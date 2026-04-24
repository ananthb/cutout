//! Main email event handler — receives inbound emails, matches rules, executes actions.

use worker::*;

use super::{forward, mime, routing};
use crate::kv;
use crate::types::*;

/// Handle an incoming email. Called from the wasm_bindgen email() export.
pub async fn handle_email(
    from: &str,
    to: &str,
    raw_bytes: &[u8],
    env: &Env,
) -> Result<EmailResult> {
    let kv_store = env.kv("KV")?;

    // Split recipient into local@domain
    let (local, domain) = match to.rsplit_once('@') {
        Some((l, d)) => (l.to_lowercase(), d.to_lowercase()),
        None => return Ok(EmailResult::Reject("Invalid recipient".into())),
    };

    // Loop detection: check for our forwarding header in the raw email headers.
    // Only search up to the first blank line (header/body boundary).
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
        return Ok(EmailResult::Reject("Forwarding loop detected".into()));
    }

    // Check if this is a reverse alias reply (parse email only for this path).
    if forward::is_reverse_alias(to) {
        let parsed = mime::parse_email(raw_bytes);
        return handle_reverse_alias(to, &parsed, &kv_store).await;
    }

    // Load and match routing rules.
    let rules = kv::get_rules(&kv_store).await?;
    let matched_rule = routing::find_matching_rule(&rules, &local, &domain);

    match matched_rule {
        Some(rule) => {
            execute_action(
                &rule.action,
                &rule.label,
                from,
                to,
                raw_bytes,
                &kv_store,
                &domain,
            )
            .await
        }
        None => {
            console_log!("No matching rule for {to}, dropping");
            Ok(EmailResult::Drop)
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
    kv_store: &worker::kv::KvStore,
    domain: &str,
) -> Result<EmailResult> {
    match action {
        Action::Drop => {
            console_log!("Dropping email from {from} to {to} (rule: {rule_label})");
            Ok(EmailResult::Drop)
        }

        Action::Forward { destinations } => {
            if destinations.is_empty() {
                return Ok(EmailResult::Drop);
            }

            // Generate one reverse alias per inbound message — shared across
            // all destinations so any reply routes back to the same original
            // sender. Save the mapping up front.
            let reverse_addr = forward::generate_reverse_address(domain);
            let reverse_alias = ReverseAlias {
                alias: to.to_string(),
                original_sender: from.to_string(),
            };
            kv::save_reverse_alias(kv_store, &reverse_addr, &reverse_alias).await?;

            // Parse the email content. We need this for all structured paths.
            let parsed = mime::parse_email(raw_bytes);

            let mut dispatch = Dispatch::default();

            for dest in destinations {
                match dest {
                    Destination::Email { address } => {
                        // Always use structured send_email for email destinations.
                        // CF's message.forward() only allows X- headers and often
                        // fails to override an existing Reply-To from the original
                        // message. structured_forward_email ensures our reverse-alias
                        // Reply-To is set correctly.
                        dispatch.send_emails.push(structured_forward_email(
                            parsed.as_ref(),
                            &reverse_addr,
                            address,
                            from,
                        ));
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
                "Forwarding from {from} via {to} (rule: {rule_label}): send_emails={} bots={}",
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

/// Build a structured outbound email for destinations beyond the first.
/// Used when a rule has multiple email destinations — CF's `forward()` can
/// only fire once per message, so extras take the `send_email` path (which
/// rewrites the `From` to the reverse-alias).
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

    // Use the original sender's name as a display name so the inbox shows
    // "Alice <reply+uuid@domain.com>" instead of just the alias.
    let from = match parsed.and_then(|p| p.from_name.as_ref()) {
        Some(name) => format!("{} <{}>", name, reverse_addr),
        None => reverse_addr.to_string(),
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

/// Handle a reply arriving at a reverse alias.
async fn handle_reverse_alias(
    to: &str,
    parsed: &Option<mime::ParsedEmail>,
    kv_store: &worker::kv::KvStore,
) -> Result<EmailResult> {
    let reverse = match kv::get_reverse_alias(kv_store, to).await? {
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

        // Should use display name from header + the reverse alias address
        assert_eq!(outbound.from, "Alice <reply+abc@proxy.com>");
        assert_eq!(outbound.reply_to, Some(reverse_addr.to_string()));
    }

    #[test]
    fn test_structured_forward_no_name() {
        let parsed = mime::ParsedEmail {
            from_name: None,
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

        // Should fall back to just the address if no display name is present
        assert_eq!(outbound.from, "reply@proxy.com");
    }
}
