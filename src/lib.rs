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
//! - `verify` — Destination email verification (public `/verify/{token}` route)
//!
//! ## Routes
//!
//! Only `/manage/*` is protected by the Cloudflare Access application.
//! Everything else (`/`, `/health`, `/verify/{token}`) is public.

use wasm_bindgen::prelude::*;
use worker::*;

mod email;
mod helpers;
pub mod kv;
mod manage;
mod types;
mod verify;

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
            for outbound in &emails {
                email::send::send_outbound(&worker_env, outbound)
                    .await
                    .map_err(|e| JsValue::from_str(&format!("send_outbound: {e}")))?;
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
        // Public — the Access application is scoped to /manage/* only.
        p if p.starts_with("/verify/") => {
            let token = p.strip_prefix("/verify/").unwrap_or("");
            if token.is_empty() || token.contains('/') {
                return Response::error("Not Found", 404);
            }
            verify::handle_verify(&env, token).await
        }
        p if p.starts_with("/manage") => manage::handle_manage(req, env, p, method).await,
        _ => Response::error("Not Found", 404),
    }
}
