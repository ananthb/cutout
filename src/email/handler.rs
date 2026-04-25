//! Main email event handler — receives inbound emails, matches rules, executes actions.

use worker::d1::*;
use worker::*;

use super::{forward, mime, routing};
use crate::db;
use crate::events::EventKind;
use crate::kv;
use crate::types::*;

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
                &rule.label,
                from,
                to,
                raw_bytes,
                &database,
                &domain,
            )
            .await?;
            let event_kind = match &result {
                EmailResult::Dispatch(_) => EventKind::Forward,
                EmailResult::Drop => EventKind::Drop,
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
async fn execute_action(
    action: &Action,
    rule_label: &str,
    from: &str,
    to: &str,
    raw_bytes: &[u8],
    database: &D1Database,
    domain: &str,
) -> Result<EmailResult> {
    match action {
        Action::Drop => {
            console_log!("Dropping email from {from} to {to} (rule: {rule_label})");
            Ok(EmailResult::Drop)
        }

        Action::Forward {
            destinations,
            replace_reply_to,
        } => {
            if destinations.is_empty() {
                return Ok(EmailResult::Drop);
            }

            // Generate one reverse alias per inbound message — shared across
            // all destinations. Save to D1 for durable mapping.
            let reverse_addr = forward::generate_reverse_address(domain);
            let id = reverse_addr
                .strip_prefix("reply+")
                .and_then(|s| s.strip_suffix(&format!("@{domain}")))
                .unwrap_or(&reverse_addr);

            db::save_reverse_mapping(database, id, to, from).await?;

            // Parse the email content. We need this for all structured paths.
            let parsed = mime::parse_email(raw_bytes);

            let mut dispatch = Dispatch::default();
            let mut email_count = 0usize;

            for dest in destinations {
                match dest {
                    Destination::Email { address } => {
                        if !replace_reply_to && email_count == 0 {
                            dispatch.forward_email = Some(ForwardInstruction {
                                destination: address.clone(),
                                reply_to: reverse_addr.clone(),
                                original_from: from.to_string(),
                            });
                        } else {
                            dispatch.send_emails.push(structured_forward_email(
                                parsed.as_ref(),
                                &reverse_addr,
                                address,
                                from,
                            ));
                        }
                        email_count += 1;
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
                        });
                    }
                }
            }

            console_log!(
                "Forwarding from {from} via {to} (rule: {rule_label}): native={} send_emails={} bots={}",
                dispatch.forward_email.is_some() as u8,
                dispatch.send_emails.len(),
                dispatch.bot_forwards.len()
            );

            if dispatch.is_empty() {
                Ok(EmailResult::Drop)
            } else {
                Ok(EmailResult::Dispatch(dispatch))
            }
        }
    }
}

fn structured_forward_email(
    parsed: Option<&mime::ParsedEmail>,
    reverse_addr: &str,
    destination: &str,
    original_from: &str,
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

    let from = match (
        parsed.and_then(|p| p.from_name.as_ref()),
        parsed.and_then(|p| p.from_address.as_ref()),
    ) {
        (Some(name), Some(addr)) => format!("{} ({}) <{}>", name, addr, reverse_addr),
        (Some(name), None) => format!("{} <{}>", name, reverse_addr),
        (None, Some(addr)) => format!("{} <{}>", addr, reverse_addr),
        (None, None) => reverse_addr.to_string(),
    };

    OutboundEmail {
        from,
        to: destination.to_string(),
        subject,
        text,
        html,
        reply_to: Some(reverse_addr.to_string()),
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

        let outbound =
            structured_forward_email(Some(&parsed), reverse_addr, destination, original_from);

        // Should use display name from header + original email + the reverse alias address
        assert_eq!(
            outbound.from,
            "Alice (alice@example.org) <reply+abc@proxy.com>"
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
        );

        // Should fall back to original address as display name
        assert_eq!(outbound.from, "alice@example.org <reply@proxy.com>");
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
        );

        // Should fall back to just the address
        assert_eq!(outbound.from, "reply@proxy.com");
    }
}
