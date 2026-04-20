//! # Cutout
//!
//! Transparent email alias proxy — like SimpleLogin or addy.io, built entirely
//! on Cloudflare Workers with Email Routing and the send_email API.
//!
//! ## How it works
//!
//! 1. You own a domain (e.g. `example.com`) with Cloudflare Email Routing
//! 2. Configure routing rules via the HTMX management UI at `/manage`
//! 3. Inbound mail matching a rule is forwarded to your real address with headers rewritten
//! 4. Replies are sent back through the alias so your real address is never exposed
//!
//! ## Architecture
//!
//! - All config lives in KV — no database needed
//! - `email` — Inbound/outbound email handling and MIME rewriting
//! - `manage` — HTMX-based management UI behind Cloudflare Access

use wasm_bindgen::prelude::*;
use worker::*;

mod email;
mod helpers;
pub mod kv;
mod manage;
mod types;

// --- Email event handler via wasm_bindgen ---

#[wasm_bindgen]
extern "C" {
    pub type IncomingEmailMessage;

    #[wasm_bindgen(method, getter)]
    fn from(this: &IncomingEmailMessage) -> String;

    #[wasm_bindgen(method, getter)]
    fn to(this: &IncomingEmailMessage) -> String;

    #[wasm_bindgen(method, getter)]
    fn raw(this: &IncomingEmailMessage) -> js_sys::Promise;

    #[wasm_bindgen(method, js_name = "setReject")]
    fn set_reject(this: &IncomingEmailMessage, reason: &str);

    pub type SendEmailBinding;

    #[wasm_bindgen(method)]
    fn send(this: &SendEmailBinding, message: &JsValue) -> js_sys::Promise;
}

#[wasm_bindgen]
pub async fn email(
    message: IncomingEmailMessage,
    env: JsValue,
    _ctx: JsValue,
) -> std::result::Result<(), JsValue> {
    console_error_panic_hook::set_once();

    let from = message.from();
    let to = message.to();

    // Read the raw email bytes
    let raw_promise = message.raw();
    let raw_value = wasm_bindgen_futures::JsFuture::from(raw_promise).await?;
    let uint8 = js_sys::Uint8Array::new(&raw_value);
    let mut raw_bytes = vec![0u8; uint8.length() as usize];
    uint8.copy_to(&mut raw_bytes);

    let worker_env: Env = env.into();

    let result = email::handler::handle_email(&from, &to, &raw_bytes, &worker_env)
        .await
        .map_err(|e| JsValue::from_str(&format!("Email handler error: {e}")))?;

    match result {
        types::EmailResult::Send(emails) => {
            let email_binding = js_sys::Reflect::get(&worker_env.into(), &"EMAIL".into())
                .map_err(|_| JsValue::from_str("Missing EMAIL binding"))?;
            let send_email: SendEmailBinding = email_binding.unchecked_into();

            for outbound in emails {
                let obj = js_sys::Object::new();
                js_sys::Reflect::set(&obj, &"from".into(), &outbound.from.into())
                    .map_err(|_| JsValue::from_str("Failed to set from"))?;
                js_sys::Reflect::set(&obj, &"to".into(), &outbound.to.into())
                    .map_err(|_| JsValue::from_str("Failed to set to"))?;
                let uint8 = js_sys::Uint8Array::from(outbound.raw.as_slice());
                js_sys::Reflect::set(&obj, &"raw".into(), &uint8.into())
                    .map_err(|_| JsValue::from_str("Failed to set raw"))?;

                let send_promise = send_email.send(&obj.into());
                wasm_bindgen_futures::JsFuture::from(send_promise).await?;
            }
        }
        types::EmailResult::Reject(reason) => {
            message.set_reject(&reason);
        }
        types::EmailResult::Drop => {
            // Silently consume
        }
    }

    Ok(())
}

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    let url = req.url()?;
    let path = url.path();
    let method = req.method();

    match path {
        "/" => {
            let headers = Headers::new();
            headers.set("Location", "/manage")?;
            Ok(Response::empty()?.with_status(302).with_headers(headers))
        }
        "/health" => Response::ok("OK"),
        p if p.starts_with("/manage") => manage::handle_manage(req, env, p, method).await,
        _ => Response::error("Not Found", 404),
    }
}
