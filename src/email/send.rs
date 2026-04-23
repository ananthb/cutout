//! Send outbound emails via the Cloudflare EMAIL binding.
//!
//! Wraps outgoing messages in the `EmailMessage` class imported from the
//! built-in `cloudflare:email` module. Passing a plain object with a `raw`
//! field is interpreted by Email Service as the new structured format and
//! rejected for missing `text`/`html` — we always have fully-formed RFC 2822
//! bytes to send, so use the legacy class path.

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use worker::*;

use crate::types::OutboundEmail;

#[wasm_bindgen(module = "cloudflare:email")]
extern "C" {
    pub type EmailMessage;

    #[wasm_bindgen(constructor)]
    fn new(from: &str, to: &str, raw: &JsValue) -> EmailMessage;
}

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

    let raw = js_sys::Uint8Array::from(outbound.raw.as_slice());
    let msg = EmailMessage::new(&outbound.from, &outbound.to, &raw.into());

    let promise = send_email.send(&msg.into());
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|e| Error::from(format!("send_email failed: {e:?}")))?;
    Ok(())
}
