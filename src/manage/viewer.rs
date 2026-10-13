//! Stored-email rendering for the inbox API: fetch, parse and sanitize a
//! stored message into the parts the frontend reads.

use worker::*;

use crate::db;
use crate::email::mime;
use crate::r2;
use crate::sanitize;

/// Header + body parts for a stored email, produced by [`build_rendered`] and
/// shipped by the JSON API, which hosts the sanitized HTML in a sandboxed
/// iframe or shows the text.
pub struct RenderedEmail {
    pub subject: String,
    pub from: String,
    pub to: String,
    /// The `created_at` timestamp, when known.
    pub date: Option<String>,
    /// Sanitized HTML body, hosted by the frontend in a sandboxed iframe.
    pub body_html: Option<String>,
    /// Plain-text body, used when there is no HTML part.
    pub body_text: Option<String>,
}

/// Fetch + parse + sanitize a stored email. Returns `None` when the row or
/// its R2 object is missing.
pub async fn build_rendered(env: &Env, id: &str) -> Result<Option<RenderedEmail>> {
    let database = env.d1("DB")?;
    let meta = match db::get_message_meta(&database, id).await? {
        Some(m) => m,
        None => return Ok(None),
    };

    let raw = match r2::get(env, &meta.r2_key).await? {
        Some(bytes) => bytes,
        None => return Ok(None),
    };

    let parsed = mime::parse_email(&raw);

    let body_html = match mime::inlined_html(&raw) {
        Some(html) => sanitize::sanitize_email_html(&html),
        None => String::new(),
    };
    let body_text = parsed.as_ref().and_then(|p| p.text_body.clone());

    let from = parsed
        .as_ref()
        .map(display_from)
        .unwrap_or_else(|| meta.sender.clone());
    let subject = parsed
        .as_ref()
        .map(|p| p.subject.clone())
        .unwrap_or_else(|| meta.subject.clone());

    Ok(Some(RenderedEmail {
        subject,
        from,
        to: meta.recipient,
        date: meta.created_at,
        body_html: (!body_html.is_empty()).then_some(body_html),
        body_text,
    }))
}

fn display_from(p: &mime::ParsedEmail) -> String {
    match (p.from_name.as_deref(), p.from_address.as_deref()) {
        (Some(name), Some(addr)) if !name.is_empty() => format!("{name} <{addr}>"),
        (_, Some(addr)) => addr.to_string(),
        (Some(name), None) => name.to_string(),
        (None, None) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_from_prefers_name_with_address() {
        let p = mime::ParsedEmail {
            from_name: Some("Alice".into()),
            from_address: Some("alice@example.org".into()),
            subject: String::new(),
            message_id: None,
            references: None,
            text_body: None,
            html_body: None,
        };
        assert_eq!(display_from(&p), "Alice <alice@example.org>");
    }

    #[test]
    fn display_from_falls_back_to_address() {
        let p = mime::ParsedEmail {
            from_name: None,
            from_address: Some("bob@x".into()),
            subject: String::new(),
            message_id: None,
            references: None,
            text_body: None,
            html_body: None,
        };
        assert_eq!(display_from(&p), "bob@x");
    }
}
