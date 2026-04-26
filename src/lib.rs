//! # Cutout
//!
//! Transparent email alias proxy: similar to SimpleLogin or addy.io, built entirely
//! on Cloudflare Workers with Email Routing and the send_email API.
//!
//! ## How it works
//!
//! 1. You own a domain (e.g. `example.com`) with Cloudflare Email Routing.
//! 2. Configure routing rules via the HTMX management UI at `/manage`.
//! 3. Inbound mail matching a rule is forwarded to your destinations
//!    (email, Discord, or Telegram) with headers rewritten.
//! 4. Replies route back through the worker and are re-sent as email from
//!    the alias, so your real address is never exposed.
//!
//! ## Architecture
//!
//! - `kv`: routing rule storage in Cloudflare KV.
//! - `db`: reverse-alias mappings and bot reply contexts in Cloudflare D1
//!   (durable, no expiry).
//! - `email`: inbound `EmailMessage.forward()` and outbound `send_email()`.
//! - `bots`: Telegram and Discord destinations plus reply webhooks.
//! - `manage`: HTMX-based management UI behind Cloudflare Access.
//! - `events`, `stats`: per-event log and Analytics Engine aggregations.
//!
//! ## Destination verification
//!
//! Email destinations must be verified via Cloudflare Email Routing's
//! "Destination Addresses" list (dashboard > Email > Email Routing >
//! Destination Addresses). `message.forward()` enforces this at runtime.
//! Cutout itself doesn't run a verification flow.
//!
//! ## Routes
//!
//! - `/` redirects to `/manage`.
//! - `/manage/*` is protected by the Cloudflare Access application.
//! - `/health` returns 200 OK.
//! - `/telegram/webhook` and `/discord/interactions` are public; they
//!   authenticate via secret token and Ed25519 signature respectively.

use wasm_bindgen::prelude::*;
use worker::*;

mod bots;
mod db;
mod email;
mod events;
mod helpers;
pub mod kv;
mod manage;
mod stats;
mod types;
mod validation;

// --- Email event handler via wasm_bindgen ---

#[wasm_bindgen]
extern "C" {
    pub type IncomingEmailMessage;

    #[wasm_bindgen(method, getter)]
    fn from(this: &IncomingEmailMessage) -> String;

    #[wasm_bindgen(method, getter)]
    fn to(this: &IncomingEmailMessage) -> String;

    /// `raw` is a ReadableStream of the message's RFC 2822 bytes, not a Promise.
    /// Treating it as a Promise (awaiting it) hangs forever.
    #[wasm_bindgen(method, getter)]
    fn raw(this: &IncomingEmailMessage) -> web_sys::ReadableStream;

    #[wasm_bindgen(method, js_name = "setReject")]
    fn set_reject(this: &IncomingEmailMessage, reason: &str);

    /// `forward(rcptTo, headers)` hands the inbound message to Cloudflare's
    /// native forwarder, preserving original From/To/DKIM/attachments.
    /// Optional `headers` are overlaid (e.g. Reply-To for proxied replies).
    /// The destination must be verified in the zone's Email Routing
    /// "Destination Addresses" list.
    #[wasm_bindgen(method)]
    fn forward(
        this: &IncomingEmailMessage,
        rcpt_to: &str,
        headers: &web_sys::Headers,
    ) -> js_sys::Promise;
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

    // `message.raw` is a ReadableStream. Wrap it in a Response to consume the
    // bytes: `Response#arrayBuffer()` reads the stream to completion.
    let raw_stream = message.raw();
    let response = web_sys::Response::new_with_opt_readable_stream(Some(&raw_stream))
        .map_err(|e| JsValue::from_str(&format!("Response from raw stream: {e:?}")))?;
    let buf_promise = response
        .array_buffer()
        .map_err(|e| JsValue::from_str(&format!("arrayBuffer: {e:?}")))?;
    let buf_value = wasm_bindgen_futures::JsFuture::from(buf_promise).await?;
    let uint8 = js_sys::Uint8Array::new(&buf_value);
    let mut raw_bytes = vec![0u8; uint8.length() as usize];
    uint8.copy_to(&mut raw_bytes);

    let worker_env: Env = env.into();
    let size_bytes = raw_bytes.len() as u64;

    let outcome = email::handler::handle_email(&from, &to, &raw_bytes, &worker_env)
        .await
        .map_err(|e| JsValue::from_str(&format!("Email handler error: {e}")))?;

    let mut channels: Vec<String> = Vec::new();

    match outcome.result {
        types::EmailResult::Dispatch(dispatch) => {
            if dispatch.forward_email.is_some() || !dispatch.send_emails.is_empty() {
                channels.push("email".into());
            }
            for f in &dispatch.bot_forwards {
                let label = match &f.channel {
                    types::BotChannel::Telegram { .. } => "telegram",
                    types::BotChannel::Discord { .. } => "discord",
                };
                if !channels.iter().any(|c| c == label) {
                    channels.push(label.into());
                }
            }

            if let Some(instr) = &dispatch.forward_email {
                let headers = web_sys::Headers::new()
                    .map_err(|e| JsValue::from_str(&format!("Headers::new: {e:?}")))?;
                headers
                    .set("Reply-To", &instr.reply_to)
                    .map_err(|e| JsValue::from_str(&format!("set Reply-To: {e:?}")))?;
                headers
                    .set("X-Cutout-Forwarded", "1")
                    .map_err(|e| JsValue::from_str(&format!("set X-Cutout-Forwarded: {e:?}")))?;
                headers
                    .set("X-Original-From", &instr.original_from)
                    .map_err(|e| JsValue::from_str(&format!("set X-Original-From: {e:?}")))?;

                let promise = message.forward(&instr.destination, &headers);
                wasm_bindgen_futures::JsFuture::from(promise).await?;
            }
            for outbound in &dispatch.send_emails {
                email::send::send_outbound(&worker_env, outbound)
                    .await
                    .map_err(|e| JsValue::from_str(&format!("send_outbound: {e}")))?;
            }
            for forward in &dispatch.bot_forwards {
                if let Err(e) = bots::dispatch(&worker_env, forward).await {
                    console_log!("bot dispatch failed: {e}");
                }
            }
        }
        types::EmailResult::Reject(reason) => {
            message.set_reject(&reason);
        }
        types::EmailResult::Drop => {
            // Silently consume
        }
    }

    // Record event after dispatch executes so channel/size reflect what
    // actually happened. Failures in the analytics path must not affect
    // the email's outcome.
    let event = events::Event {
        ts: events::now_ms(),
        kind: outcome.event_kind,
        from: from.clone(),
        to: to.clone(),
        rule_id: outcome.matched_rule_id,
        channels,
        size_bytes,
    };
    if let Ok(kv) = worker_env.kv("KV") {
        if let Err(e) = events::record(&worker_env, &kv, &event).await {
            console_log!("event record failed: {e}");
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
        "/telegram/webhook" if method == Method::Post => {
            bots::handle_telegram_webhook(req, env).await
        }
        "/discord/interactions" if method == Method::Post => {
            bots::handle_discord_interaction(req, env).await
        }
        p if p.starts_with("/manage") => manage::handle_manage(req, env, p, method).await,
        _ => Response::error("Not Found", 404),
    }
}
