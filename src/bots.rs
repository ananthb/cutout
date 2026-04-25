//! Telegram and Discord bot integration: outbound posts with reply-context
//! storage, plus inbound webhook handlers that route replies back via email.

use worker::*;

use botrelay::discord::{ActionRow, Component, CreateMessage, DiscordBot, InteractionResponse};
use botrelay::reply::ReplyContext;
use botrelay::telegram::{ParseMode, SendMessage, TelegramBot};

use crate::db;
use crate::email::send;
use crate::types::{BotChannel, BotForward, OutboundEmail};

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

/// Execute a [`BotForward`] — post the content to the right channel and save
/// a [`ReplyContext`] for the webhook side to pick up.
pub async fn dispatch(env: &Env, forward: &BotForward) -> Result<()> {
    let database = env.d1("DB")?;
    let ctx = ReplyContext {
        alias: forward.alias.clone(),
        original_sender: forward.original_sender.clone(),
        subject: forward.subject.clone(),
    };
    let body = render_body(forward);

    match &forward.channel {
        BotChannel::Telegram { chat_id } => {
            let bot = match telegram_bot(env) {
                Some(b) => b,
                None => {
                    console_log!("Telegram bot not configured; skipping chat_id={chat_id}");
                    return Ok(());
                }
            };
            let msg = bot
                .send_message(SendMessage {
                    chat_id: chat_id.clone(),
                    text: body,
                    parse_mode: Some(ParseMode::Html),
                    disable_preview: Some(true),
                    ..Default::default()
                })
                .await?;
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
            let msg = bot
                .create_message(
                    channel_id,
                    CreateMessage {
                        content: body,
                        components: vec![ActionRow::new(vec![Component::primary_button(
                            format!("reply:dc:{channel_id}"),
                            "Reply",
                        )])],
                        ..Default::default()
                    },
                )
                .await?;
            let key = ReplyContext::discord_key(channel_id, &msg.id);
            db::save_bot_ctx(&database, &key, &ctx).await?;
            Ok(())
        }
    }
}

/// Render the body we post to a chat. Keep it simple: who from, subject,
/// then the text body (truncated if huge).
fn render_body(forward: &BotForward) -> String {
    const MAX: usize = 3500; // Telegram limits messages to 4096 chars; keep headroom.
    let mut text = forward.text.clone();
    if text.chars().count() > MAX {
        text.truncate(MAX);
        text.push_str("\n… (truncated)");
    }
    format!(
        "From: {}\nTo: {}\nSubject: {}\n\n{}",
        forward.original_sender, forward.alias, forward.subject, text
    )
}

// ========================================================================
// Inbound webhooks
// ========================================================================

/// Handle `POST /telegram/webhook`. Checks the secret-token header if one
/// is configured, parses the update, and if it's a reply to a message we
/// remember — sends the reply back via email.
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
        };
        let out = render_body(&f);
        assert!(out.contains("truncated"));
        assert!(out.chars().count() < 4096);
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
        };
        let out = render_body(&f);
        assert!(out.starts_with("From: alice@example.org\n"));
        assert!(out.contains("To: shop@kedi.dev\n"));
        assert!(out.contains("Subject: Order\n\nBody"));
    }
}
