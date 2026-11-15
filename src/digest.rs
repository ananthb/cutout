//! Scheduled digest sender.
//!
//! Email destinations in digest mode are never forwarded inline: the email
//! handler stores each message and records a `digest_items` row per pending
//! destination. The Cron trigger (`0 * * * *`) wakes this module hourly, and
//! it sends one email per destination once that destination's schedule is
//! due.
//!
//! Due-ness is measured from the last successful send, or from the oldest
//! pending item when nothing has been sent yet. That makes the first digest
//! wait a full interval after the first message instead of firing the moment
//! one arrives.

use worker::*;

use crate::db;
use crate::email::send;
use crate::types::{Action, Destination, DigestItem, DigestSchedule, OutboundEmail, Rule};

pub async fn handle(env: Env) -> Result<()> {
    let database = env.d1("DB")?;
    let rules = db::get_rules(&database).await?;
    let now = crate::events::now_ms();
    let base = env
        .var("PUBLIC_BASE_URL")
        .ok()
        .map(|v| v.to_string())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim_end_matches('/').to_string());

    for rule in &rules {
        let destinations = match &rule.action {
            Action::Forward { destinations } | Action::ForwardStore { destinations } => {
                destinations
            }
            _ => continue,
        };

        for destination in destinations {
            let Destination::Email {
                address,
                digest: Some(schedule),
                ..
            } = destination
            else {
                continue;
            };

            // A transient D1 failure for one destination must not skip every
            // destination after it this tick; log and move on.
            if let Err(e) = process_destination(
                &env,
                &database,
                rule,
                address,
                schedule,
                now,
                base.as_deref(),
            )
            .await
            {
                console_log!("digest: rule {} destination {address} failed: {e}", rule.id);
            }
        }
    }

    Ok(())
}

/// Send one destination's digest if its schedule is due. Errors are reported
/// to the caller, which logs them and continues with the next destination.
async fn process_destination(
    env: &Env,
    database: &D1Database,
    rule: &Rule,
    address: &str,
    schedule: &DigestSchedule,
    now: i64,
    base: Option<&str>,
) -> Result<()> {
    let Some((oldest, _)) = db::digest_pending_summary(database, &rule.id, address).await? else {
        return Ok(());
    };
    let reference = db::digest_last_sent(database, &rule.id, address)
        .await?
        .unwrap_or(oldest);
    if now - reference < schedule.interval_ms() {
        return Ok(());
    }

    let items = db::list_digest_items(database, &rule.id, address).await?;
    if items.is_empty() {
        return Ok(());
    }

    // Bound the post-send delete to the rows we actually sent: anything
    // enqueued while the send was in flight has a larger id and must survive
    // for the next digest.
    let up_to = digest_up_to(&items);
    let outbound = build_digest_email(&rule.display_label(), address, &items, base);
    match send::send_outbound(env, &outbound).await {
        Ok(()) => {
            db::delete_digest_items(database, &rule.id, address, up_to).await?;
            db::set_digest_last_sent(database, &rule.id, address, now).await?;
        }
        Err(e) => console_log!("digest: send to {address} failed: {e}"),
    }
    Ok(())
}

/// Highest row id in a listed snapshot; the delete is bounded by it.
fn digest_up_to(items: &[DigestItem]) -> i64 {
    items.iter().map(|item| item.id).max().unwrap_or(0)
}

/// Build the single summary email for one destination.
///
/// `from` is the alias the stored messages were addressed to (the newest
/// item's `recipient`): Cloudflare's structured send requires a sender on an
/// onboarded Cutout domain, so a digest reuses the alias rather than inventing
/// a new address.
fn build_digest_email(
    rule_label: &str,
    to_address: &str,
    items: &[DigestItem],
    base: Option<&str>,
) -> OutboundEmail {
    let count = items.len();
    let from = items
        .first()
        .map(|item| item.recipient.clone())
        .unwrap_or_default();
    let plural = if count == 1 { "" } else { "s" };
    let subject = format!("Digest · {count} new email{plural} · {rule_label}");

    let mut text_lines = Vec::with_capacity(count);
    for item in items {
        let line = match base {
            Some(base) => format!(
                "{} — {} — {base}/inbox/{}",
                item.sender, item.subject, item.message_id
            ),
            None => format!("{} — {}", item.sender, item.subject),
        };
        text_lines.push(line);
    }
    let text = text_lines.join("\n");

    let mut html = String::from("<ul>");
    for item in items {
        let link = match base {
            Some(base) => format!(
                "<a href=\"{}/inbox/{}\">{}</a>",
                escape_html(base),
                escape_html(&item.message_id),
                escape_html(&item.subject)
            ),
            None => escape_html(&item.subject),
        };
        html.push_str(&format!("<li>{link} — {}</li>", escape_html(&item.sender)));
    }
    html.push_str("</ul>");

    OutboundEmail {
        from,
        to: to_address.to_string(),
        subject,
        text: Some(text),
        html: Some(html),
        reply_to: None,
        headers: vec![("X-Cutout-Digest".to_string(), "1".to_string())],
    }
}

fn escape_html(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(message_id: &str, sender: &str, subject: &str) -> DigestItem {
        DigestItem {
            id: 0,
            message_id: message_id.into(),
            sender: sender.into(),
            recipient: "alias@cutout.test".into(),
            subject: subject.into(),
            created_at: 1,
        }
    }

    fn item_with_id(id: i64, message_id: &str) -> DigestItem {
        DigestItem {
            id,
            message_id: message_id.into(),
            sender: "a@example.com".into(),
            recipient: "alias@cutout.test".into(),
            subject: "Subject".into(),
            created_at: 1,
        }
    }

    #[test]
    fn digest_up_to_is_the_snapshot_max_id() {
        let items = vec![
            item_with_id(3, "m3"),
            item_with_id(7, "m7"),
            item_with_id(5, "m5"),
        ];
        assert_eq!(digest_up_to(&items), 7);
        assert_eq!(digest_up_to(&[]), 0);
    }

    #[test]
    fn build_digest_email_links_each_item() {
        let items = vec![
            item("m2", "bob@example.com", "Second"),
            item("m1", "alice@example.com", "First"),
        ];
        let outbound = build_digest_email(
            "Newsletter",
            "me@home.test",
            &items,
            Some("https://cutout.test"),
        );

        assert_eq!(outbound.from, "alias@cutout.test");
        assert_eq!(outbound.to, "me@home.test");
        assert!(
            outbound.subject.contains("2 new emails"),
            "{}",
            outbound.subject
        );
        assert!(outbound.subject.contains("Newsletter"));
        assert!(outbound.reply_to.is_none());
        assert_eq!(
            outbound.headers,
            vec![("X-Cutout-Digest".to_string(), "1".to_string())]
        );

        let text = outbound.text.unwrap();
        assert!(text.contains("https://cutout.test/inbox/m1"));
        assert!(text.contains("https://cutout.test/inbox/m2"));
        assert!(text.contains("alice@example.com"));
        assert!(text.contains("First"));

        let html = outbound.html.unwrap();
        assert!(html.contains("href=\"https://cutout.test/inbox/m1\""));
        assert!(html.contains("href=\"https://cutout.test/inbox/m2\""));
        assert!(html.contains("Second"));
    }

    #[test]
    fn build_digest_email_without_base_has_no_links() {
        let items = vec![item("m1", "alice@example.com", "First")];
        let outbound = build_digest_email("Newsletter", "me@home.test", &items, None);

        assert!(
            outbound.subject.contains("1 new email "),
            "{}",
            outbound.subject
        );
        let text = outbound.text.unwrap();
        assert!(!text.contains("/inbox/"));
        assert!(text.contains("alice@example.com"));
        let html = outbound.html.unwrap();
        assert!(!html.contains("href="));
        assert!(html.contains("First"));
    }

    #[test]
    fn build_digest_email_escapes_html() {
        let items = vec![item("m&1", "a<b@example.com", "<script>")];
        let outbound =
            build_digest_email("News", "me@home.test", &items, Some("https://cutout.test"));
        let html = outbound.html.unwrap();
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("a&lt;b@example.com"));
        assert!(html.contains("m&amp;1"));
    }

    #[test]
    fn build_digest_email_escapes_base_url_in_href() {
        let items = vec![item("m1", "a@example.com", "First")];
        let outbound = build_digest_email(
            "News",
            "me@home.test",
            &items,
            Some("https://cutout.test/?a=1&b=\"q\""),
        );
        let html = outbound.html.unwrap();
        assert!(
            html.contains("href=\"https://cutout.test/?a=1&amp;b=&quot;q&quot;/inbox/m1\""),
            "{html}"
        );
    }
}
