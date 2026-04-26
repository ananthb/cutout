//! Send outbound emails via the Cloudflare Email Service `EMAIL` binding.
//!
//! Uses the structured-message API: `{ from, to, subject, text, html, replyTo,
//! headers }`. Email Service rejects plain objects with `raw` bytes (it
//! interprets them as malformed structured messages), and the legacy
//! `EmailMessage` class from `cloudflare:email` requires bundler externals
//! that worker-build doesn't provide. Structured fields side-step both.

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use worker::*;

use crate::types::OutboundEmail;

#[wasm_bindgen]
extern "C" {
    pub type SendEmailBinding;

    #[wasm_bindgen(method)]
    fn send(this: &SendEmailBinding, message: &JsValue) -> js_sys::Promise;
}

/// Send a single outbound email through the EMAIL binding.
pub async fn send_outbound(env: &Env, outbound: &OutboundEmail) -> Result<()> {
    let env_js: JsValue = env.clone().into();
    let binding = js_sys::Reflect::get(&env_js, &"EMAIL".into())
        .map_err(|_| Error::from("Missing EMAIL binding"))?;
    let send_email: SendEmailBinding = binding.unchecked_into();

    let obj = js_sys::Object::new();
    set_str(&obj, "from", &outbound.from)?;
    set_str(&obj, "to", &outbound.to)?;
    set_str(&obj, "subject", &outbound.subject)?;
    if let Some(text) = &outbound.text {
        set_str(&obj, "text", text)?;
    }
    if let Some(html) = &outbound.html {
        set_str(&obj, "html", html)?;
    }
    if let Some(reply_to) = &outbound.reply_to {
        set_str(&obj, "replyTo", reply_to)?;
    }
    if !outbound.headers.is_empty() {
        let headers = js_sys::Object::new();
        for (name, value) in &outbound.headers {
            set_str(&headers, name, value)?;
        }
        js_sys::Reflect::set(&obj, &"headers".into(), &headers.into())
            .map_err(|_| Error::from("set headers"))?;
    }

    let promise = send_email.send(&obj.into());
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|e| Error::from(format!("send_email failed: {}", describe_js_error(&e))))?;
    Ok(())
}

fn set_str(obj: &js_sys::Object, key: &str, value: &str) -> Result<()> {
    js_sys::Reflect::set(obj, &key.into(), &JsValue::from_str(value))
        .map_err(|_| Error::from(format!("set {key}")))?;
    Ok(())
}

/// Pull a useful description out of a JsValue error: prefers `name + message`
/// (the standard Error shape), then `cause`, then falls back to `Debug`. The
/// default `format!("{e:?}")` on an Error tends to print "JsValue(Error:
/// internal server error)" which loses any structured fields the runtime
/// may have attached.
pub fn describe_js_error(e: &JsValue) -> String {
    let get = |k: &str| -> Option<String> {
        js_sys::Reflect::get(e, &k.into())
            .ok()
            .and_then(|v| v.as_string())
            .filter(|s| !s.is_empty())
    };
    let message = get("message");
    let name = get("name");
    let cause = js_sys::Reflect::get(e, &"cause".into())
        .ok()
        .filter(|v| !v.is_undefined() && !v.is_null())
        .map(|v| v.as_string().unwrap_or_else(|| format!("{v:?}")))
        .filter(|s| !s.is_empty());

    let mut parts = Vec::with_capacity(3);
    match (name, message) {
        (Some(n), Some(m)) => parts.push(format!("{n}: {m}")),
        (None, Some(m)) => parts.push(m),
        (Some(n), None) => parts.push(n),
        (None, None) => parts.push(format!("{e:?}")),
    }
    if let Some(c) = cause {
        parts.push(format!("cause: {c}"));
    }
    parts.join(" | ")
}

/// Build and send an RFC 3464 multipart/report DSN telling the original
/// sender that the message we accepted couldn't be delivered. Used by the
/// DLQ consumer when a `pending_dispatches` row is dead-lettered.
///
/// `recipient_alias` is the alias the sender originally addressed (e.g.
/// `test@kedi.dev`); the DSN's `From` is `mailer-daemon@<alias-domain>` so
/// the bounce passes SPF/DKIM for the cutout-controlled domain.
pub async fn send_dsn(
    env: &Env,
    sender: &str,
    recipient_alias: &str,
    subject_of_original: Option<&str>,
    error_text: &str,
) -> Result<()> {
    let domain = recipient_alias
        .rsplit_once('@')
        .map(|(_, d)| d)
        .unwrap_or("localhost");
    let from = format!("mailer-daemon@{domain}");
    let subject = "Mail delivery failed: returning message to sender".to_string();
    let original_subject = subject_of_original.unwrap_or("(no subject)");

    let human = format!(
        "This is the mail delivery system at {domain}.\r\n\
         \r\n\
         Your message addressed to <{recipient_alias}> could not be delivered \
         to its final destination after multiple retries. The original message \
         is preserved by the operator for inspection.\r\n\
         \r\n\
         Original subject: {original_subject}\r\n\
         Failure reason  : {error_text}\r\n",
    );

    let delivery_status = format!(
        "Reporting-MTA: dns; {domain}\r\n\
         \r\n\
         Final-Recipient: rfc822; {recipient_alias}\r\n\
         Action: failed\r\n\
         Status: 5.0.0\r\n\
         Diagnostic-Code: smtp; {error_text}\r\n",
    );

    let boundary = format!("cutout-dsn-{}", uuid::Uuid::new_v4().simple());
    let body = format!(
        "--{boundary}\r\n\
         Content-Type: text/plain; charset=us-ascii\r\n\
         \r\n\
         {human}\r\n\
         --{boundary}\r\n\
         Content-Type: message/delivery-status\r\n\
         \r\n\
         {delivery_status}\r\n\
         --{boundary}--\r\n",
    );

    let outbound = OutboundEmail {
        from,
        to: sender.to_string(),
        subject,
        text: Some(body),
        html: None,
        reply_to: None,
        headers: vec![
            ("Auto-Submitted".to_string(), "auto-replied".to_string()),
            (
                "Content-Type".to_string(),
                format!("multipart/report; report-type=delivery-status; boundary=\"{boundary}\""),
            ),
            ("X-Cutout-Forwarded".to_string(), "1".to_string()),
        ],
    };
    send_outbound(env, &outbound).await
}
