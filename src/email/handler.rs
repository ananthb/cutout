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

    // Parse the email
    let parsed = mime::parse_email(raw_bytes);

    // Check if this is a reverse alias reply
    if forward::is_reverse_alias(to) {
        return handle_reverse_alias(to, &parsed, &kv_store, &domain).await;
    }

    // Load and match routing rules
    let rules = kv::get_rules(&kv_store).await?;
    let matched_rule = routing::find_matching_rule(&rules, &local, &domain);

    match matched_rule {
        Some(rule) => {
            execute_action(
                &rule.action,
                &rule.label,
                from,
                to,
                &parsed,
                &kv_store,
                &domain,
            )
            .await
        }
        None => {
            // Should not happen if catch-all exists, but handle gracefully
            console_log!("No matching rule for {to}, dropping");
            Ok(EmailResult::Drop)
        }
    }
}

/// Execute a matched action.
async fn execute_action(
    action: &Action,
    rule_label: &str,
    from: &str,
    to: &str,
    parsed: &Option<mime::ParsedEmail>,
    kv_store: &worker::kv::KvStore,
    domain: &str,
) -> Result<EmailResult> {
    match action {
        Action::Drop => {
            console_log!("Dropping email from {from} to {to} (rule: {rule_label})");
            Ok(EmailResult::Drop)
        }

        Action::Forward { destinations } => {
            let parsed = match parsed {
                Some(p) => p,
                None => return Ok(EmailResult::Reject("Failed to parse email".into())),
            };

            let mut emails = Vec::with_capacity(destinations.len());

            for destination in destinations {
                let reverse_addr = forward::generate_reverse_address(domain);

                // Save reverse alias for reply routing
                let reverse_alias = ReverseAlias {
                    alias: to.to_string(),
                    original_sender: from.to_string(),
                };
                kv::save_reverse_alias(kv_store, &reverse_addr, &reverse_alias).await?;

                let forwarded = mime::build_forwarded_email(
                    parsed,
                    &reverse_addr,
                    destination,
                    &reverse_addr,
                    from,
                    domain,
                );

                console_log!(
                    "Forwarding from {from} to {destination} via {to} (rule: {rule_label})"
                );

                emails.push(OutboundEmail {
                    from: reverse_addr,
                    to: destination.clone(),
                    raw: forwarded,
                });
            }

            if emails.is_empty() {
                Ok(EmailResult::Drop)
            } else {
                Ok(EmailResult::Send(emails))
            }
        }
    }
}

/// Handle a reply arriving at a reverse alias.
async fn handle_reverse_alias(
    to: &str,
    parsed: &Option<mime::ParsedEmail>,
    kv_store: &worker::kv::KvStore,
    domain: &str,
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

    let reply_bytes =
        mime::build_reply_email(parsed, &reverse.alias, &reverse.original_sender, domain);

    console_log!(
        "Reply routing: {} -> {}",
        reverse.alias,
        reverse.original_sender
    );

    Ok(EmailResult::Send(vec![OutboundEmail {
        from: reverse.alias,
        to: reverse.original_sender,
        raw: reply_bytes,
    }]))
}
