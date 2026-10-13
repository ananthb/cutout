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

/// Which outbound backend to use, read from `OUTBOUND_EMAIL_BACKEND`.
///
/// Cloudflare has two ways to send: Email Routing (free, but only to
/// destination addresses verified in the dashboard) and Email Service (paid,
/// any recipient, needs the sending domain onboarded). The binding is the
/// same; this only decides how the message is sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutboundBackend {
    /// Free. Verified destinations only. The first non-proxy email
    /// destination uses native `EmailMessage.forward()`, which preserves the
    /// original message; the rest go through `send_email`.
    Routing,
    /// Paid. Any recipient. Every email destination goes through
    /// `send_email`, so nothing is limited to a verified address.
    Service,
}

impl OutboundBackend {
    pub fn from_env(env: &Env) -> Self {
        match env
            .var("OUTBOUND_EMAIL_BACKEND")
            .ok()
            .map(|v| v.to_string().trim().to_ascii_lowercase())
            .as_deref()
        {
            Some("service" | "email_service") => OutboundBackend::Service,
            _ => OutboundBackend::Routing,
        }
    }

    /// Whether the first non-proxy email destination should use native
    /// `EmailMessage.forward()`. Only the free Email Routing path does.
    pub fn uses_native_forward(self) -> bool {
        matches!(self, OutboundBackend::Routing)
    }
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
            // Email Service allowlists custom headers (platform-controlled
            // ones like Content-Type are rejected) and rejects the *whole*
            // send if any header is disallowed. Drop and log rather than
            // letting one bad header take down the message.
            if !is_allowed_header(name) {
                console_log!("send_outbound: dropping disallowed header {name:?}");
                continue;
            }
            set_str(&headers, name, value)?;
        }
        js_sys::Reflect::set(&obj, &"headers".into(), &headers.into())
            .map_err(|_| Error::from("set headers"))?;
    }

    let promise = send_email.send(&obj.into());
    wasm_bindgen_futures::JsFuture::from(promise).await.map_err(|e| {
        let detail = describe_js_error(&e);
        if is_unverified_recipient(&detail) {
            // The free Email Routing backend only delivers to addresses
            // verified in the dashboard. Make that obvious in the logs and
            // point at the fix, because the raw error says nothing about it.
            console_log!(
                "send_outbound: {} is not a verified Email Routing destination. \
                 Verify it under Email Routing > Destination Addresses, or onboard the \
                 sending domain to Email Service (paid) and set OUTBOUND_EMAIL_BACKEND=service. \
                 See https://developers.cloudflare.com/email-service/",
                outbound.to
            );
            Error::from(format!(
                "{} is not a verified address. Verify it, or switch to Email Service \
                 (https://developers.cloudflare.com/email-service/)",
                outbound.to
            ))
        } else {
            Error::from(format!("send_email failed: {detail}"))
        }
    })?;
    Ok(())
}

/// True when the runtime rejected the recipient for not being a verified
/// Email Routing destination address.
fn is_unverified_recipient(detail: &str) -> bool {
    detail.to_ascii_lowercase().contains("not a verified address")
}

/// The custom headers Cloudflare Email Service accepts. Anything else — and
/// anything corresponding to a first-class API field (`From`, `To`, `Subject`,
/// `Reply-To`, …) — is rejected with `E_HEADER_NOT_ALLOWED`, failing the whole
/// send. See <https://developers.cloudflare.com/email-service/reference/headers/>.
fn is_allowed_header(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if lower.starts_with("x-") {
        return true;
    }
    matches!(
        lower.as_str(),
        "in-reply-to"
            | "references"
            | "thread-index"
            | "thread-topic"
            | "list-unsubscribe"
            | "list-unsubscribe-post"
            | "list-id"
            | "list-archive"
            | "list-help"
            | "list-owner"
            | "list-post"
            | "list-subscribe"
            | "precedence"
            | "auto-submitted"
            | "content-language"
            | "keywords"
            | "comments"
            | "importance"
            | "priority"
            | "sensitivity"
            | "organization"
            | "require-recipient-valid-since"
            | "expires"
            | "reply-by"
            | "archived-at"
    )
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

/// Build and send a DSN telling the original sender that the message we
/// accepted couldn't be delivered. Used by the DLQ consumer when a
/// `pending_dispatches` row is dead-lettered.
///
/// `recipient_alias` is the alias the sender originally addressed (e.g.
/// `test@kedi.dev`); the DSN's `From` is `mailer-daemon@<alias-domain>` so
/// the bounce passes SPF/DKIM for the cutout-controlled domain.
///
/// The body is plain text: Cloudflare's `send_email` owns `Content-Type`
/// (it is platform-controlled and a custom one fails the whole send), so an
/// RFC 3464 `multipart/report` can't be assembled through the structured API.
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

    let body = format!(
        "This is the mail delivery system at {domain}.\n\
         \n\
         Your message addressed to <{recipient_alias}> could not be delivered to its \
         final destination after multiple retries. The original message is preserved \
         by the operator for inspection.\n\
         \n\
         Original subject: {original_subject}\n\
         Failure reason  : {error_text}\n",
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
            ("X-Cutout-Forwarded".to_string(), "1".to_string()),
        ],
    };
    send_outbound(env, &outbound).await
}
