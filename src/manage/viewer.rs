//! Stored-email viewer. Shared between the Access-protected
//! `/manage/m/{id}` route and the public token-signed `/m/{id}?t=…` route:
//! both call [`render`] to produce the response.

use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use worker::*;

use crate::db;
use crate::email::mime;
use crate::helpers::html_escape;
use crate::r2;
use crate::sanitize;

/// HMAC-SHA256 the message id with `key` and return URL-safe base64 (no
/// padding). The result is the `t=` value embedded in token-mode viewer
/// URLs.
pub fn sign_id(key: &str, id: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes()).expect("HMAC accepts any key");
    mac.update(id.as_bytes());
    let bytes = mac.finalize().into_bytes();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Verify a token produced by [`sign_id`]. Constant-time compare.
pub fn verify_signed_id(key: &str, id: &str, token: &str) -> bool {
    let expected = sign_id(key, id);
    expected.as_bytes().ct_eq(token.as_bytes()).into()
}

/// Render the viewer for `id`. Returns:
/// - 200 + HTML when the message exists and has a body to render
/// - 404 when the row or its R2 object is missing
pub async fn render(env: &Env, id: &str) -> Result<Response> {
    let database = env.d1("DB")?;
    let meta = match db::get_message_meta(&database, id).await? {
        Some(m) => m,
        None => return Response::error("Not Found", 404),
    };

    let raw = match r2::get(env, &meta.r2_key).await? {
        Some(bytes) => bytes,
        None => return Response::error("Body not found", 404),
    };

    let parsed = mime::parse_email(&raw);

    let body_html = match mime::inlined_html(&raw) {
        Some(html) => sanitize::sanitize_email_html(&html),
        None => String::new(),
    };
    let body_text = parsed.as_ref().and_then(|p| p.text_body.clone());

    let from = parsed
        .as_ref()
        .map(|p| display_from(p))
        .unwrap_or_else(|| meta.sender.clone());
    let subject = parsed
        .as_ref()
        .map(|p| p.subject.clone())
        .unwrap_or_else(|| meta.subject.clone());
    let date_line = meta
        .created_at
        .as_deref()
        .map(|ts| {
            format!(
                "<div class=\"row\"><span class=\"k\">Date</span><span>{}</span></div>",
                html_escape(ts)
            )
        })
        .unwrap_or_default();

    let body_section = if !body_html.is_empty() {
        format!(
            "<iframe class=\"email-body\" sandbox=\"allow-popups allow-popups-to-escape-sandbox\" srcdoc=\"{}\"></iframe>",
            srcdoc_attr_escape(&body_html)
        )
    } else if let Some(text) = body_text {
        format!("<pre class=\"email-text\">{}</pre>", html_escape(&text))
    } else {
        "<p class=\"empty\">No renderable body.</p>".to_string()
    };

    let page = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>{subject_html} — cutout</title>
<style>
  body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif; margin: 0; background: #f5f5f7; color: #1d1d1f; }}
  .wrap {{ max-width: 920px; margin: 24px auto; padding: 0 16px; }}
  .header {{ background: #fff; border: 1px solid #d2d2d7; border-radius: 12px; padding: 16px 20px; margin-bottom: 16px; }}
  .header h1 {{ margin: 0 0 12px; font-size: 18px; line-height: 1.4; word-break: break-word; }}
  .row {{ display: flex; gap: 12px; font-size: 13px; line-height: 1.6; }}
  .row .k {{ width: 64px; color: #6e6e73; text-transform: uppercase; letter-spacing: 0.4px; font-size: 11px; padding-top: 2px; }}
  .row span:not(.k) {{ word-break: break-all; }}
  .body-card {{ background: #fff; border: 1px solid #d2d2d7; border-radius: 12px; overflow: hidden; }}
  .email-body {{ width: 100%; min-height: 70vh; border: 0; background: #fff; }}
  .email-text {{ margin: 0; padding: 16px 20px; white-space: pre-wrap; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 13px; line-height: 1.55; }}
  .empty {{ padding: 24px; text-align: center; color: #6e6e73; }}
</style>
</head>
<body>
<div class="wrap">
  <div class="header">
    <h1>{subject_html}</h1>
    <div class="row"><span class="k">From</span><span>{from_html}</span></div>
    <div class="row"><span class="k">To</span><span>{to_html}</span></div>
    {date_line}
  </div>
  <div class="body-card">
    {body_section}
  </div>
</div>
</body>
</html>"#,
        subject_html = html_escape(&subject),
        from_html = html_escape(&from),
        to_html = html_escape(&meta.recipient),
    );

    let mut resp = Response::from_html(page)?;
    let headers = resp.headers_mut();
    headers.set("Cache-Control", "private, no-store")?;
    headers.set("X-Content-Type-Options", "nosniff")?;
    Ok(resp)
}

fn display_from(p: &mime::ParsedEmail) -> String {
    match (p.from_name.as_deref(), p.from_address.as_deref()) {
        (Some(name), Some(addr)) if !name.is_empty() => format!("{name} <{addr}>"),
        (_, Some(addr)) => addr.to_string(),
        (Some(name), None) => name.to_string(),
        (None, None) => String::new(),
    }
}

/// Escape a string for safe embedding inside a `srcdoc="…"` attribute.
/// Only `&` and `"` need encoding inside a double-quoted attribute value.
fn srcdoc_attr_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srcdoc_escape_handles_quotes_and_amps() {
        let s = srcdoc_attr_escape("<a href=\"x\">x &amp; y</a>");
        assert_eq!(s, "<a href=&quot;x&quot;>x &amp;amp; y</a>");
    }

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
    fn sign_id_is_stable_for_same_inputs() {
        let a = sign_id("secret", "abc123");
        let b = sign_id("secret", "abc123");
        assert_eq!(a, b);
    }

    #[test]
    fn sign_id_differs_when_key_or_id_changes() {
        let base = sign_id("k1", "id1");
        assert_ne!(base, sign_id("k2", "id1"));
        assert_ne!(base, sign_id("k1", "id2"));
    }

    #[test]
    fn verify_signed_id_round_trips() {
        let key = "test-key";
        let id = "msg-123";
        let token = sign_id(key, id);
        assert!(verify_signed_id(key, id, &token));
    }

    #[test]
    fn verify_signed_id_rejects_tamper() {
        let key = "test-key";
        let id = "msg-123";
        let mut token = sign_id(key, id);
        // Flip a character.
        let last = token.pop().unwrap();
        token.push(if last == 'A' { 'B' } else { 'A' });
        assert!(!verify_signed_id(key, id, &token));
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
