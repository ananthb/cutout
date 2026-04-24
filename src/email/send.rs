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
        .map_err(|e| Error::from(format!("send_email failed: {e:?}")))?;
    Ok(())
}

fn set_str(obj: &js_sys::Object, key: &str, value: &str) -> Result<()> {
    js_sys::Reflect::set(obj, &key.into(), &JsValue::from_str(value))
        .map_err(|_| Error::from(format!("set {key}")))?;
    Ok(())
}
