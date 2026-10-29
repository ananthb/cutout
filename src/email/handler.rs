//! Main email event handler: receives inbound emails, matches rules, executes actions.

use worker::d1::*;
use worker::*;

use super::{forward, mime, routing, send};
use crate::db;
use crate::events::EventKind;
use crate::kv;
use crate::r2;
use crate::types::*;
use crate::{bots, types};

/// What `handle_email` decided to do with an inbound message, plus the
/// metadata needed to record an event after the dispatch executes.
pub struct EmailOutcome {
    pub result: EmailResult,
    /// `id` of the rule that fired (if any).
    pub matched_rule_id: Option<String>,
    /// Event category for analytics. Channels are derived by the caller
    /// from the realised `Dispatch`.
    pub event_kind: EventKind,
}

/// Handle an incoming email. Called from the wasm_bindgen email() export.
pub async fn handle_email(
    from: &str,
    to: &str,
    raw_bytes: &[u8],
    env: &Env,
) -> Result<EmailOutcome> {
    let kv_store = env.kv("KV")?;
    let database = env.d1("DB")?;
    let env_for_store = env;

    // Split recipient into local@domain
    let (local, domain) = match to.rsplit_once('@') {
        Some((l, d)) => (l.to_lowercase(), d.to_lowercase()),
        None => {
            return Ok(EmailOutcome {
                result: EmailResult::Reject("Invalid recipient".into()),
                matched_rule_id: None,
                event_kind: EventKind::Reject,
            });
        }
    };

    // Loop detection: check for our forwarding header in the raw email headers.
    let header_end = raw_bytes
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .unwrap_or(raw_bytes.len().min(8192));
    let header_section = &raw_bytes[..header_end];
    let header_lower: Vec<u8> = header_section
        .iter()
        .map(|b| b.to_ascii_lowercase())
        .collect();
    if header_lower
        .windows(b"x-cutout-forwarded:".len())
        .any(|w| w == b"x-cutout-forwarded:")
    {
        console_log!("Loop detected: {from} -> {to}");
        return Ok(EmailOutcome {
            result: EmailResult::Reject("Forwarding loop detected".into()),
            matched_rule_id: None,
            event_kind: EventKind::Reject,
        });
    }

    // Check if this is a reverse alias reply (parse email only for this path).
    if forward::is_reverse_alias(to) {
        let parsed = mime::parse_email(raw_bytes);
        let result = handle_reverse_alias(to, &parsed, &database).await?;
        let event_kind = match &result {
            EmailResult::Reject(_) => EventKind::Reject,
            _ => EventKind::Reply,
        };
        return Ok(EmailOutcome {
            result,
            matched_rule_id: None,
            event_kind,
        });
    }

    // Load and match routing rules from KV.
    let rules = kv::get_rules(&kv_store).await?;
    let matched_rule = routing::find_matching_rule(&rules, &local, &domain);

    match matched_rule {
        Some(rule) => {
            let result = execute_action(
                &rule.action,
                &rule.id,
                &rule.display_label(),
                from,
                to,
                raw_bytes,
                &database,
                env_for_store,
                &domain,
            )
            .await?;
            let event_kind = match &result {
                EmailResult::Dispatch(_) => EventKind::Forward,
                EmailResult::Drop => {
                    if matches!(rule.action, Action::Store { .. }) {
                        EventKind::Store
                    } else {
                        EventKind::Drop
                    }
                }
                EmailResult::Reject(_) => EventKind::Reject,
            };
            Ok(EmailOutcome {
                result,
                matched_rule_id: Some(rule.id.clone()),
                event_kind,
            })
        }
        None => {
            console_log!("No matching rule for {to}, dropping");
            Ok(EmailOutcome {
                result: EmailResult::Drop,
                matched_rule_id: None,
                event_kind: EventKind::Drop,
            })
        }
    }
}

/// Execute a matched action, producing a [`Dispatch`] for the top-level
/// handler to fan out into `message.forward()`, `send_email`, or bot posts.
// Wide by nature: an action needs the whole matched-rule context.
#[allow(clippy::too_many_arguments)]
async fn execute_action(
    action: &Action,
    rule_id: &str,
    rule_label: &str,
    from: &str,
    to: &str,
    raw_bytes: &[u8],
    database: &D1Database,
    env: &Env,
    domain: &str,
) -> Result<EmailResult> {
    match action {
        Action::Drop => {
            console_log!("Dropping email from {from} to {to} (rule: {rule_label})");
            Ok(EmailResult::Drop)
        }

        Action::Forward { destinations } | Action::ForwardStore { destinations } => {
            if destinations.is_empty() {
                return Ok(EmailResult::Drop);
            }

            // Store & Forward persists a copy of the message in addition to
            // forwarding it.
            let persist = matches!(action, Action::ForwardStore { .. });

            // Generate one reverse alias per inbound message: shared across
            // all destinations. Save to D1 for durable mapping.
            let reverse_addr = forward::generate_reverse_address(domain);
            let id = reverse_addr
                .strip_prefix("reply+")
                .and_then(|s| s.strip_suffix(&format!("@{domain}")))
                .unwrap_or(&reverse_addr);

            db::save_reverse_mapping(database, id, to, from).await?;

            // Parse the email content. We need this for all structured paths.
            let parsed = mime::parse_email(raw_bytes);

            // Persist raw bytes + metadata when at least one destination is a
            // chat channel (the bot post embeds a "View full email" link to
            // /inbox/{id}) or when Store & Forward was asked for.
            let has_chat_destination = destinations.iter().any(|d| {
                matches!(
                    d,
                    Destination::Telegram { .. } | Destination::Discord { .. }
                )
            });
            let stored_message_id = if has_chat_destination || persist {
                let id = uuid::Uuid::new_v4().to_string();
                let key = r2::message_key(&id);
                r2::put(env, &key, raw_bytes).await?;
                let subject = parsed
                    .as_ref()
                    .map(|p| p.subject.clone())
                    .unwrap_or_default();
                db::save_message(database, &id, from, to, &subject, &key, Some(rule_id)).await?;
                Some(id)
            } else {
                None
            };
            // Inline cid: image references once; both bot dispatch (for the
            // screenshot) and the viewer get the same payload.
            let inlined_html = if has_chat_destination {
                mime::inlined_html(raw_bytes)
            } else {
                None
            };

            // Proxy destinations are re-originated through `send_email`;
            // direct destinations keep Cloudflare's native forward so the
            // original From survives. The choice is per destination.
            let mut dispatch = Dispatch::default();

            for dest in destinations {
                match dest {
                    Destination::Email { address, proxy } => {
                        if *proxy {
                            // Cloudflare's structured send requires `From` to
                            // be on an onboarded domain. Keep the original
                            // sender visible in the display name, and route
                            // replies back through the reverse alias.
                            dispatch.send_emails.push(structured_forward_email(
                                parsed.as_ref(),
                                to,
                                address,
                                from,
                                Some(&reverse_addr),
                            ));
                        } else {
                            // Native forward preserves the original From and
                            // Reply-To. Only X- headers can be added.
                            dispatch.forward_emails.push(ForwardInstruction {
                                destination: address.clone(),
                                alias: to.to_string(),
                                original_from: from.to_string(),
                            });
                        }
                    }
                    Destination::Telegram { chat_id } => {
                        dispatch.bot_forwards.push(BotForward {
                            channel: BotChannel::Telegram {
                                chat_id: chat_id.clone(),
                            },
                            original_sender: from.to_string(),
                            alias: to.to_string(),
                            subject: parsed
                                .as_ref()
                                .map(|p| p.subject.clone())
                                .unwrap_or_default(),
                            text: parsed
                                .as_ref()
                                .and_then(|p| p.text_body.clone())
                                .unwrap_or_default(),
                            message_id: stored_message_id.clone().unwrap_or_default(),
                            html: inlined_html.clone(),
                            inbound_message_id: parsed.as_ref().and_then(|p| p.message_id.clone()),
                            inbound_references: parsed.as_ref().and_then(|p| p.references.clone()),
                        });
                    }
                    Destination::Discord { channel_id } => {
                        dispatch.bot_forwards.push(BotForward {
                            channel: BotChannel::Discord {
                                channel_id: channel_id.clone(),
                            },
                            original_sender: from.to_string(),
                            alias: to.to_string(),
                            subject: parsed
                                .as_ref()
                                .map(|p| p.subject.clone())
                                .unwrap_or_default(),
                            text: parsed
                                .as_ref()
                                .and_then(|p| p.text_body.clone())
                                .unwrap_or_default(),
                            message_id: stored_message_id.clone().unwrap_or_default(),
                            html: inlined_html.clone(),
                            inbound_message_id: parsed.as_ref().and_then(|p| p.message_id.clone()),
                            inbound_references: parsed.as_ref().and_then(|p| p.references.clone()),
                        });
                    }
                }
            }

            console_log!(
                "Forwarding from {from} via {to} (rule: {rule_label}): native={} send_emails={} bots={}",
                dispatch.forward_emails.len(),
                dispatch.send_emails.len(),
                dispatch.bot_forwards.len()
            );

            if dispatch.is_empty() {
                Ok(EmailResult::Drop)
            } else {
                Ok(EmailResult::Dispatch(dispatch))
            }
        }

        Action::Store { persist } => {
            console_log!(
                "Storing email from {from} to {to} (rule: {rule_label}, persist: {persist})"
            );
            if *persist {
                let parsed = mime::parse_email(raw_bytes);
                let subject = parsed
                    .as_ref()
                    .map(|p| p.subject.clone())
                    .unwrap_or_default();
                let id = uuid::Uuid::new_v4().to_string();
                let key = r2::message_key(&id);
                r2::put(env, &key, raw_bytes).await?;
                db::save_message(database, &id, from, to, &subject, &key, Some(rule_id)).await?;
            }
            Ok(EmailResult::Drop)
        }
    }
}

fn structured_forward_email(
    parsed: Option<&mime::ParsedEmail>,
    from_addr: &str,
    destination: &str,
    original_from: &str,
    reply_to: Option<&str>,
) -> OutboundEmail {
    let mut headers: Vec<(String, String)> = vec![
        ("X-Cutout-Forwarded".to_string(), "1".to_string()),
        ("X-Original-From".to_string(), original_from.to_string()),
    ];
    if let Some(parsed) = parsed {
        if let Some(msg_id) = &parsed.message_id {
            headers.push(("In-Reply-To".to_string(), msg_id.clone()));
        }
        if let Some(refs) = &parsed.references {
            headers.push(("References".to_string(), refs.clone()));
        }
    }

    let subject = parsed.map(|p| p.subject.clone()).unwrap_or_default();
    let text = parsed.and_then(|p| p.text_body.clone());
    let html = parsed.and_then(|p| p.html_body.clone());

    // Cloudflare's structured send rejects a `From` outside an onboarded
    // sending domain, so `from_addr` is always one of ours. Keep the original
    // sender visible in a quoted display name: RFC 5322 permits any qtext
    // (including `<`, `>` and `@`) inside a quoted-string, and the full
    // address is what the recipient needs in order to recognise the sender.
    // `X-Original-From` remains the machine-readable copy.
    let display = parsed.and_then(|p| match (&p.from_name, &p.from_address) {
        (Some(name), Some(addr)) if !name.is_empty() && name != addr => {
            Some(format!("{name} <{addr}>"))
        }
        (_, Some(addr)) if !addr.is_empty() => Some(addr.clone()),
        (Some(name), _) if !name.is_empty() => Some(name.clone()),
        _ => None,
    });
    let from = match display {
        Some(name) => format!("\"{}\" <{}>", quote_display(&name), from_addr),
        None => from_addr.to_string(),
    };

    OutboundEmail {
        from,
        to: destination.to_string(),
        subject,
        text,
        html,
        reply_to: reply_to.map(str::to_string),
        headers,
    }
}

async fn handle_reverse_alias(
    to: &str,
    parsed: &Option<mime::ParsedEmail>,
    database: &D1Database,
) -> Result<EmailResult> {
    // Extract ID from reply+<id>@domain
    let id = to
        .split('+')
        .nth(1)
        .and_then(|s| s.split('@').next())
        .unwrap_or(to);

    let reverse = match db::get_reverse_mapping(database, id).await? {
        Some(r) => r,
        None => {
            console_log!("Unknown reverse alias: {to}");
            return Ok(EmailResult::Reject("Unknown reverse alias".into()));
        }
    };

    let parsed = match parsed {
        Some(p) => p,
        None => return Ok(EmailResult::Reject("Failed to parse reply".into())),
    };

    console_log!(
        "Reply routing: {} -> {}",
        reverse.alias,
        reverse.original_sender
    );

    let mut headers: Vec<(String, String)> =
        vec![("X-Cutout-Forwarded".to_string(), "1".to_string())];
    if let Some(msg_id) = &parsed.message_id {
        headers.push(("In-Reply-To".to_string(), msg_id.clone()));
    }
    if let Some(refs) = &parsed.references {
        headers.push(("References".to_string(), refs.clone()));
    }

    let mut dispatch = Dispatch::default();
    dispatch.send_emails.push(OutboundEmail {
        from: reverse.alias.clone(),
        to: reverse.original_sender,
        subject: parsed.subject.clone(),
        text: parsed.text_body.clone(),
        html: parsed.html_body.clone(),
        reply_to: Some(reverse.alias),
        headers,
    });
    Ok(EmailResult::Dispatch(dispatch))
}

/// Escape `"` and `\` inside an RFC 5322 quoted-string display name. This
/// is a deliberately conservative subset: no folding-whitespace handling,
/// no unicode encoding (RFC 2047 is the next step if we hit headers with
/// non-ASCII names — `send_email` accepts UTF-8 directly today).
fn quote_display(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch == '\\' || ch == '"' {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// Convert a still-failing [`ForwardInstruction`] into the equivalent
/// [`OutboundEmail`]: the queue consumer cannot replay native
/// `EmailMessage.forward()` because there is no `IncomingEmailMessage`
/// handle on the consumer side, so the only durable retry is via
/// structured `send_email`.
///
/// The fallback cannot preserve the original `From` (Cloudflare requires a
/// sender on an onboarded domain), so it uses the alias the message was
/// addressed to and leaves `Reply-To` alone; direct mode stays direct.
pub fn structured_from_native(
    parsed: Option<&mime::ParsedEmail>,
    instr: &ForwardInstruction,
) -> OutboundEmail {
    structured_forward_email(
        parsed,
        &instr.alias,
        &instr.destination,
        &instr.original_from,
        None,
    )
}

/// Replay a list of pending actions. Returns `(still_failing, errors)`.
/// Used by the `cutout-retries` queue consumer.
pub async fn execute_pending_actions(
    env: &Env,
    actions: &[PendingAction],
) -> (Vec<PendingAction>, Vec<String>) {
    let mut still_failing = Vec::new();
    let mut errors = Vec::new();
    for action in actions {
        match action {
            PendingAction::SendEmail(out) => {
                if let Err(e) = send::send_outbound(env, out).await {
                    errors.push(format!("send_email to {}: {}", out.to, e));
                    still_failing.push(action.clone());
                }
            }
            PendingAction::Bot(forward) => {
                let label = match &forward.channel {
                    types::BotChannel::Telegram { .. } => "telegram",
                    types::BotChannel::Discord { .. } => "discord",
                };
                if let Err(e) = bots::dispatch(env, forward).await {
                    errors.push(format!("bot {label}: {e}"));
                    still_failing.push(action.clone());
                }
            }
        }
    }
    (still_failing, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_structured_forward_display_name() {
        let parsed = mime::ParsedEmail {
            from_name: Some("Alice".to_string()),
            from_address: Some("alice@example.org".to_string()),
            subject: "Hello".to_string(),
            message_id: Some("msg123".to_string()),
            references: None,
            text_body: Some("body".to_string()),
            html_body: None,
        };
        let reverse_addr = "reply+abc@proxy.com";
        let destination = "me@home.com";
        let original_from = "alice@example.org";

        let outbound = structured_forward_email(
            Some(&parsed),
            "newsletter@proxy.com",
            destination,
            original_from,
            Some(reverse_addr),
        );

        // The From address must be ours; the original identity (name and
        // address) lives in the quoted display name.
        assert_eq!(
            outbound.from,
            "\"Alice <alice@example.org>\" <newsletter@proxy.com>"
        );
        assert_eq!(outbound.reply_to, Some(reverse_addr.to_string()));

        // Should include the original sender in headers
        assert!(outbound
            .headers
            .iter()
            .any(|(n, v)| n == "X-Original-From" && v == "alice@example.org"));
    }

    #[test]
    fn test_structured_forward_no_name() {
        let parsed = mime::ParsedEmail {
            from_name: None,
            from_address: Some("alice@example.org".to_string()),
            subject: "Hello".to_string(),
            message_id: None,
            references: None,
            text_body: None,
            html_body: None,
        };
        let outbound = structured_forward_email(
            Some(&parsed),
            "reply@proxy.com",
            "me@home.com",
            "alice@example.org",
            None,
        );

        // Should fall back to the original address as the display name.
        assert_eq!(outbound.from, "\"alice@example.org\" <reply@proxy.com>");
        assert_eq!(outbound.reply_to, None);
    }

    #[test]
    fn test_structured_forward_no_metadata() {
        let parsed = mime::ParsedEmail {
            from_name: None,
            from_address: None,
            subject: "Hello".to_string(),
            message_id: None,
            references: None,
            text_body: None,
            html_body: None,
        };
        let outbound = structured_forward_email(
            Some(&parsed),
            "reply@proxy.com",
            "me@home.com",
            "alice@example.org",
            None,
        );

        // Should fall back to just the address
        assert_eq!(outbound.from, "reply@proxy.com");
    }
}
