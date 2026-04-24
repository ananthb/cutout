//! Destination address verification flow.
//!
//! When a rule is configured to forward to an address that hasn't been
//! verified yet, we send a one-time link to that address. Clicking the link
//! marks the address as verified and enables forwarding.

use worker::*;

use crate::email::send;
use crate::helpers::html_escape;
use crate::kv;
use crate::types::OutboundEmail;

/// If `email` is not already verified, create a pending token and send a
/// verification email. No-op if already verified.
pub async fn send_verification(env: &Env, proxy_domain: &str, email: &str) -> Result<()> {
    let kv_store = env.kv("KV")?;
    if kv::is_verified(&kv_store, email).await? {
        return Ok(());
    }
    let token = kv::create_pending(&kv_store, email).await?;
    send::send_outbound(env, &verification_email(email, proxy_domain, &token)).await
}

/// Handle GET /verify/{token}. Marks the associated email as verified.
pub async fn handle_verify(env: &Env, token: &str) -> Result<Response> {
    let kv_store = env.kv("KV")?;
    match kv::consume_pending(&kv_store, token).await? {
        Some(email) => {
            kv::mark_verified(&kv_store, &email).await?;
            Response::from_html(success_page(&email))
        }
        None => {
            let resp = Response::from_html(invalid_page())?;
            Ok(resp.with_status(404))
        }
    }
}

fn verification_email(to: &str, proxy_domain: &str, token: &str) -> OutboundEmail {
    let body = format!(
        "Hi,\n\n\
        Someone added this address as a forwarding destination on {proxy_domain}.\n\n\
        Click the link below to confirm:\n\n\
        https://{proxy_domain}/verify/{token}\n\n\
        This link expires in 24 hours. If you did not expect this, ignore this email.\n"
    );

    OutboundEmail {
        from: format!("verify@{proxy_domain}"),
        to: to.to_string(),
        subject: "Confirm your forwarding address".to_string(),
        text: Some(body),
        html: None,
        reply_to: None,
        headers: Vec::new(),
    }
}

fn success_page(email: &str) -> String {
    format!(
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\">\
        <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
        <title>Verified — Cutout</title></head>\
        <body style=\"font-family:system-ui,sans-serif;max-width:480px;margin:4rem auto;padding:2rem;text-align:center;line-height:1.5\">\
        <h1 style=\"font-size:1.5rem\">Email verified</h1>\
        <p><code>{}</code> is now confirmed as a forwarding destination.</p>\
        <p style=\"color:#666;font-size:0.9rem\">You can close this tab.</p>\
        </body></html>",
        html_escape(email)
    )
}

fn invalid_page() -> String {
    "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\">\
    <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
    <title>Invalid link — Cutout</title></head>\
    <body style=\"font-family:system-ui,sans-serif;max-width:480px;margin:4rem auto;padding:2rem;text-align:center;line-height:1.5\">\
    <h1 style=\"font-size:1.5rem\">Invalid or expired link</h1>\
    <p style=\"color:#666\">This verification link is no longer valid. Open the management UI and resend.</p>\
    </body></html>"
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verification_email_has_link() {
        let msg = verification_email("me@example.com", "proxy.example.com", "tok123");
        assert_eq!(msg.from, "verify@proxy.example.com");
        assert_eq!(msg.to, "me@example.com");
        assert_eq!(msg.subject, "Confirm your forwarding address");
        let text = msg.text.expect("text body");
        assert!(text.contains("https://proxy.example.com/verify/tok123"));
        assert!(msg.html.is_none());
    }

    #[test]
    fn success_page_escapes_email() {
        let html = success_page("a<b>@example.com");
        assert!(html.contains("a&lt;b&gt;@example.com"));
        assert!(!html.contains("a<b>@example.com"));
    }
}
