//! Telegram and Discord bot integration: outbound posts with reply-context
//! storage, plus inbound webhook handlers that route replies back via email.

use worker::*;

use botrelay::discord::{
    ActionRow, Attachment, Component, CreateMessage, DiscordBot, InteractionResponse,
};
use botrelay::reply::ReplyContext;
use botrelay::telegram::{SendMessage, SendPhoto, TelegramBot};

use crate::db;
use crate::email::send;
use crate::manage::viewer;
use crate::screenshot;
use crate::types::{BotChannel, BotForward, OutboundEmail, ViewerAuth};

/// Which chat destinations are currently available, based on whether the
/// relevant bot secrets are present.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EnabledChannels {
    pub telegram: bool,
    pub discord: bool,
}

impl EnabledChannels {
    /// Detect which bot channels are configured. A channel is enabled iff
    /// all secrets required to both post and verify its webhook are set.
    pub fn from_env(env: &Env) -> Self {
        Self {
            telegram: env.secret("TELEGRAM_BOT_TOKEN").is_ok(),
            discord: env.secret("DISCORD_BOT_TOKEN").is_ok()
                && env.secret("DISCORD_APP_ID").is_ok()
                && env.secret("DISCORD_PUBLIC_KEY").is_ok(),
        }
    }
}

fn telegram_bot(env: &Env) -> Option<TelegramBot> {
    env.secret("TELEGRAM_BOT_TOKEN")
        .ok()
        .map(|s| TelegramBot::new(s.to_string()))
}

fn discord_bot(env: &Env) -> Option<DiscordBot> {
    let token = env.secret("DISCORD_BOT_TOKEN").ok()?.to_string();
    let app_id = env.secret("DISCORD_APP_ID").ok()?.to_string();
    let pub_key = env.secret("DISCORD_PUBLIC_KEY").ok()?.to_string();
    Some(DiscordBot::new(token, app_id, pub_key))
}

/// Execute a [`BotForward`]: post the content to the right channel and save
/// a [`ReplyContext`] for the webhook side to pick up.
///
/// When the forward carries an HTML body and Browser Rendering is
/// configured, the post includes a screenshot of the email; otherwise it
/// falls back to text only. Either way the body ends with a link to the
/// stored email's viewer URL (when one is available).
pub async fn dispatch(env: &Env, forward: &BotForward) -> Result<()> {
    let database = env.d1("DB")?;
    let ctx = ReplyContext {
        alias: forward.alias.clone(),
        original_sender: forward.original_sender.clone(),
        subject: forward.subject.clone(),
    };
    let viewer_url = build_viewer_url(env, forward);
    let screenshot = render_screenshot_or_log(env, forward).await;

    match &forward.channel {
        BotChannel::Telegram { chat_id } => {
            let bot = match telegram_bot(env) {
                Some(b) => b,
                None => {
                    console_log!("Telegram bot not configured; skipping chat_id={chat_id}");
                    return Ok(());
                }
            };
            let msg = match screenshot {
                Some(png) => {
                    let caption = render_body(forward, viewer_url.as_deref(), TG_CAPTION_MAX);
                    bot.send_photo(SendPhoto {
                        chat_id: chat_id.clone(),
                        photo: png,
                        photo_filename: "email.png".into(),
                        photo_content_type: "image/png".into(),
                        caption: Some(caption),
                        ..Default::default()
                    })
                    .await?
                }
                None => {
                    let text = render_body(forward, viewer_url.as_deref(), TG_TEXT_MAX);
                    bot.send_message(SendMessage {
                        chat_id: chat_id.clone(),
                        text,
                        disable_preview: Some(true),
                        ..Default::default()
                    })
                    .await?
                }
            };
            let key = ReplyContext::telegram_key(chat_id, msg.message_id);
            db::save_bot_ctx(&database, &key, &ctx).await?;
            Ok(())
        }
        BotChannel::Discord { channel_id } => {
            let bot = match discord_bot(env) {
                Some(b) => b,
                None => {
                    console_log!("Discord bot not configured; skipping channel_id={channel_id}");
                    return Ok(());
                }
            };
            let body = render_body(forward, viewer_url.as_deref(), DC_TEXT_MAX);
            let params = CreateMessage {
                content: body,
                components: vec![ActionRow::new(vec![Component::primary_button(
                    format!("reply:dc:{channel_id}"),
                    "Reply",
                )])],
                ..Default::default()
            };
            let msg = match screenshot {
                Some(png) => {
                    let attachments = vec![Attachment {
                        filename: "email.png".into(),
                        content_type: "image/png".into(),
                        bytes: png,
                    }];
                    bot.create_message_with_attachments(channel_id, params, &attachments)
                        .await?
                }
                None => bot.create_message(channel_id, params).await?,
            };
            let key = ReplyContext::discord_key(channel_id, &msg.id);
            db::save_bot_ctx(&database, &key, &ctx).await?;
            Ok(())
        }
    }
}

const TG_TEXT_MAX: usize = 3500; // Telegram message hard limit is 4096; leave headroom.
const TG_CAPTION_MAX: usize = 1024; // Telegram photo caption hard limit.
const DC_TEXT_MAX: usize = 1900; // Discord message hard limit is 2000.

/// Construct the chat-post body: header lines + truncated text snippet +
/// optional viewer link line. `max` caps the total character count so the
/// caller can pick a value appropriate to the destination's hard limit.
fn render_body(forward: &BotForward, viewer_url: Option<&str>, max: usize) -> String {
    let header = format!(
        "From: {}\nTo: {}\nSubject: {}",
        forward.original_sender, forward.alias, forward.subject
    );
    let link_line = viewer_url
        .map(|url| format!("\n\nView full email → {url}"))
        .unwrap_or_default();

    // Reserve space for the header and link; the snippet uses what's left.
    let header_len = header.chars().count();
    let link_len = link_line.chars().count();
    // 2 chars for the blank line between header and body.
    let snippet_budget = max
        .saturating_sub(header_len)
        .saturating_sub(link_len)
        .saturating_sub(2);

    let mut snippet: String = forward.text.chars().take(snippet_budget).collect();
    if forward.text.chars().count() > snippet_budget {
        // Trim back to leave room for the truncation marker.
        let marker = "… (truncated)";
        let marker_len = marker.chars().count();
        if snippet.chars().count() > marker_len {
            let keep = snippet.chars().count() - marker_len;
            snippet = snippet.chars().take(keep).collect::<String>() + marker;
        }
    }

    format!("{header}\n\n{snippet}{link_line}")
}

/// Resolve the viewer URL for a forward, or `None` when one can't be built
/// (no stored message id, or token mode requested but no signing key).
fn build_viewer_url(env: &Env, forward: &BotForward) -> Option<String> {
    if forward.message_id.is_empty() {
        return None;
    }
    let base = env
        .var("PUBLIC_BASE_URL")
        .ok()
        .map(|s| s.to_string())
        .unwrap_or_default();
    let base = base.trim_end_matches('/');
    if base.is_empty() {
        return None;
    }
    match forward.link_auth {
        ViewerAuth::Access => Some(format!("{base}/manage/m/{}", forward.message_id)),
        ViewerAuth::Token => {
            let key = env.secret("VIEWER_HMAC_KEY").ok()?.to_string();
            let token = viewer::sign_id(&key, &forward.message_id);
            Some(format!("{base}/m/{}?t={token}", forward.message_id))
        }
    }
}

/// Best-effort screenshot. Returns `None` (and logs) on any failure so the
/// dispatcher falls back to a text-only post rather than dropping the email.
async fn render_screenshot_or_log(env: &Env, forward: &BotForward) -> Option<Vec<u8>> {
    let html = forward.html.as_deref()?;
    if html.is_empty() {
        return None;
    }
    let sanitized = crate::sanitize::sanitize_email_html(html);
    match screenshot::render_email_png(env, &sanitized).await {
        Ok(bytes) => Some(bytes),
        Err(e) => {
            console_log!("screenshot render failed (falling back to text): {e}");
            None
        }
    }
}

// ========================================================================
// Inbound webhooks
// ========================================================================

/// Handle `POST /telegram/webhook`. Checks the secret-token header if one
/// is configured, parses the update, and if it's a reply to a message we
/// remember, sends the reply back via email.
pub async fn handle_telegram_webhook(mut req: Request, env: Env) -> Result<Response> {
    if let Ok(expected) = env.secret("TELEGRAM_WEBHOOK_SECRET") {
        let got = req
            .headers()
            .get("X-Telegram-Bot-Api-Secret-Token")
            .ok()
            .flatten();
        if got.as_deref() != Some(&expected.to_string()) {
            console_log!("telegram webhook: bad secret token");
            return Response::error("Forbidden", 403);
        }
    }

    let body = req.bytes().await?;
    let update = botrelay::telegram::parse_update(&body)?;
    let msg = match update.any_message() {
        Some(m) => m,
        None => return Response::ok("ignored"),
    };

    // Only reply-to-message updates trigger routing.
    let reply_to = match &msg.reply_to_message {
        Some(r) => r,
        None => return Response::ok("ignored"),
    };

    let database = env.d1("DB")?;
    let key = ReplyContext::telegram_key(&msg.chat.id.to_string(), reply_to.message_id);
    let ctx = match db::get_bot_ctx(&database, &key).await? {
        Some(c) => c,
        None => {
            console_log!("telegram: no context for {key}");
            return Response::ok("no context");
        }
    };

    let body_text = msg.text.clone().unwrap_or_default();
    if body_text.trim().is_empty() {
        return Response::ok("empty reply");
    }

    send_reply_email(&env, &ctx, &body_text).await?;
    Response::ok("ok")
}

/// Handle `POST /discord/interactions`. Verifies the Ed25519 signature,
/// answers pings, shows a modal on button click, and on modal submit routes
/// the reply back via email.
pub async fn handle_discord_interaction(mut req: Request, env: Env) -> Result<Response> {
    let bot = match discord_bot(&env) {
        Some(b) => b,
        None => return Response::error("Discord not configured", 503),
    };

    let signature = req
        .headers()
        .get("X-Signature-Ed25519")
        .ok()
        .flatten()
        .unwrap_or_default();
    let timestamp = req
        .headers()
        .get("X-Signature-Timestamp")
        .ok()
        .flatten()
        .unwrap_or_default();
    let body_str = req.text().await?;

    if !bot
        .verify_interaction(&signature, &timestamp, &body_str)
        .await?
    {
        return Response::error("Bad signature", 401);
    }

    let interaction = botrelay::discord::parse_interaction(body_str.as_bytes())?;

    if interaction.is_ping() {
        return Response::from_json(&InteractionResponse::pong());
    }

    // Button click → open a modal so the user can type the reply.
    if interaction.is_component_click() {
        let message_id = interaction
            .message
            .as_ref()
            .map(|m| m.id.clone())
            .unwrap_or_default();
        let channel_id = interaction.channel_id.clone().unwrap_or_default();
        let custom_id = format!("reply:dc:{channel_id}:{message_id}");
        let resp = InteractionResponse::modal(
            custom_id,
            "Reply",
            vec![ActionRow::new(vec![Component::paragraph_input(
                "reply_text",
                "Your reply",
            )])],
        );
        return Response::from_json(&resp);
    }

    // Modal submit → send the email.
    if interaction.is_modal_submit() {
        let cid = interaction
            .data
            .as_ref()
            .and_then(|d| d.custom_id.clone())
            .unwrap_or_default();
        let parts: Vec<&str> = cid.split(':').collect();
        // Expect "reply:dc:<channel_id>:<message_id>"
        if parts.len() != 4 || parts[0] != "reply" || parts[1] != "dc" {
            return Response::from_json(&InteractionResponse::ephemeral_message(
                "Unknown reply context",
            ));
        }
        let channel_id = parts[2];
        let message_id = parts[3];
        let text = interaction.modal_text("reply_text").unwrap_or_default();
        if text.trim().is_empty() {
            return Response::from_json(&InteractionResponse::ephemeral_message("Reply was empty"));
        }

        let database = env.d1("DB")?;
        let key = ReplyContext::discord_key(channel_id, message_id);
        let ctx = match db::get_bot_ctx(&database, &key).await? {
            Some(c) => c,
            None => {
                return Response::from_json(&InteractionResponse::ephemeral_message(
                    "Reply context not found",
                ));
            }
        };

        send_reply_email(&env, &ctx, text).await?;
        return Response::from_json(&InteractionResponse::ephemeral_message("Reply sent."));
    }

    Response::from_json(&InteractionResponse::ephemeral_message(
        "Unsupported interaction",
    ))
}

async fn send_reply_email(env: &Env, ctx: &ReplyContext, text: &str) -> Result<()> {
    let outbound = OutboundEmail {
        from: ctx.alias.clone(),
        to: ctx.original_sender.clone(),
        subject: format!("Re: {}", ctx.subject),
        text: Some(text.to_string()),
        html: None,
        reply_to: Some(ctx.alias.clone()),
        headers: vec![("X-Cutout-Forwarded".to_string(), "1".to_string())],
    };
    send::send_outbound(env, &outbound).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{BotChannel, BotForward};

    #[test]
    fn render_body_truncates() {
        let huge = "x".repeat(10_000);
        let f = BotForward {
            channel: BotChannel::Telegram {
                chat_id: "1".into(),
            },
            original_sender: "s@x".into(),
            alias: "a@d".into(),
            subject: "S".into(),
            text: huge,
            message_id: String::new(),
            html: None,
            link_auth: Default::default(),
        };
        let out = render_body(&f, None, TG_TEXT_MAX);
        assert!(out.contains("truncated"));
        assert!(out.chars().count() <= TG_TEXT_MAX);
    }

    #[test]
    fn render_body_preserves_header_lines() {
        let f = BotForward {
            channel: BotChannel::Discord {
                channel_id: "1".into(),
            },
            original_sender: "alice@example.org".into(),
            alias: "shop@kedi.dev".into(),
            subject: "Order".into(),
            text: "Body".into(),
            message_id: String::new(),
            html: None,
            link_auth: Default::default(),
        };
        let out = render_body(&f, None, TG_TEXT_MAX);
        assert!(out.starts_with("From: alice@example.org\n"));
        assert!(out.contains("To: shop@kedi.dev\n"));
        assert!(out.contains("Subject: Order\n\nBody"));
    }

    #[test]
    fn render_body_appends_viewer_link_when_provided() {
        let f = BotForward {
            channel: BotChannel::Telegram {
                chat_id: "1".into(),
            },
            original_sender: "s@x".into(),
            alias: "a@d".into(),
            subject: "S".into(),
            text: "Body".into(),
            message_id: "abc".into(),
            html: None,
            link_auth: Default::default(),
        };
        let out = render_body(&f, Some("https://x.test/m/abc"), TG_TEXT_MAX);
        assert!(out.ends_with("View full email → https://x.test/m/abc"));
    }

    #[test]
    fn render_body_caption_budget_respected() {
        let huge = "y".repeat(5_000);
        let f = BotForward {
            channel: BotChannel::Telegram {
                chat_id: "1".into(),
            },
            original_sender: "s@x".into(),
            alias: "a@d".into(),
            subject: "S".into(),
            text: huge,
            message_id: "abc".into(),
            html: None,
            link_auth: Default::default(),
        };
        let url = "https://x.test/m/abc?t=tok";
        let out = render_body(&f, Some(url), TG_CAPTION_MAX);
        assert!(
            out.chars().count() <= TG_CAPTION_MAX,
            "len {}",
            out.chars().count()
        );
        assert!(out.ends_with(url));
        assert!(out.contains("truncated"));
    }
}
