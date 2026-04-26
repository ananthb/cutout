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
mod dlq;
mod email;
mod events;
mod helpers;
pub mod kv;
mod manage;
mod r2;
mod retries;
mod sanitize;
mod screenshot;
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

    let mut succeeded_channels: Vec<String> = Vec::new();
    let mut failed_actions: Vec<types::PendingAction> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    let mut event_kind = outcome.event_kind;
    let rule_id = outcome.matched_rule_id.clone();

    match outcome.result {
        types::EmailResult::Dispatch(dispatch) => {
            // Native EmailMessage.forward() — synchronously, only available here.
            if let Some(instr) = &dispatch.forward_email {
                match try_native_forward(&message, instr).await {
                    Ok(()) => push_unique(&mut succeeded_channels, "email"),
                    Err(e) => {
                        errors.push(format!("native forward to {}: {e}", instr.destination));
                        // Convert to a structured send for retry; we do not
                        // have an IncomingEmailMessage handle on the queue
                        // consumer side.
                        let parsed = email::mime::parse_email(&raw_bytes);
                        let structured =
                            email::handler::structured_from_native(parsed.as_ref(), instr);
                        failed_actions.push(types::PendingAction::SendEmail(structured));
                    }
                }
            }

            for outbound in &dispatch.send_emails {
                match email::send::send_outbound(&worker_env, outbound).await {
                    Ok(()) => push_unique(&mut succeeded_channels, "email"),
                    Err(e) => {
                        errors.push(format!("send_email to {}: {e}", outbound.to));
                        failed_actions.push(types::PendingAction::SendEmail(outbound.clone()));
                    }
                }
            }

            for forward in &dispatch.bot_forwards {
                let label = match &forward.channel {
                    types::BotChannel::Telegram { .. } => "telegram",
                    types::BotChannel::Discord { .. } => "discord",
                };
                match bots::dispatch(&worker_env, forward).await {
                    Ok(()) => push_unique(&mut succeeded_channels, label),
                    Err(e) => {
                        errors.push(format!("bot {label}: {e}"));
                        failed_actions.push(types::PendingAction::Bot(forward.clone()));
                    }
                }
            }

            if !failed_actions.is_empty() {
                event_kind = events::EventKind::Error;
                if let Err(e) = enqueue_for_retry(
                    &worker_env,
                    &raw_bytes,
                    &from,
                    &to,
                    rule_id.as_deref(),
                    &failed_actions,
                    &errors.join("; "),
                )
                .await
                {
                    // If we couldn't even persist the retry row, we have no
                    // durable record. Log loudly so the operator knows the
                    // email is genuinely lost rather than queued.
                    console_log!("CRITICAL: enqueue_for_retry failed: {e}; failed_actions lost");
                    errors.push(format!("enqueue failed: {e}"));
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

    let error_text = if errors.is_empty() {
        None
    } else {
        Some(errors.join("; "))
    };
    let event = events::Event {
        ts: events::now_ms(),
        kind: event_kind,
        from: from.clone(),
        to: to.clone(),
        rule_id,
        channels: succeeded_channels,
        size_bytes,
        error: error_text,
    };
    if let Ok(kv) = worker_env.kv("KV") {
        if let Err(e) = events::record(&worker_env, &kv, &event).await {
            console_log!("event record failed: {e}");
        }
    }

    Ok(())
}

fn push_unique(channels: &mut Vec<String>, label: &str) {
    if !channels.iter().any(|c| c == label) {
        channels.push(label.into());
    }
}

async fn try_native_forward(
    message: &IncomingEmailMessage,
    instr: &types::ForwardInstruction,
) -> std::result::Result<(), String> {
    let headers = web_sys::Headers::new().map_err(|e| format!("Headers::new: {e:?}"))?;
    headers
        .set("Reply-To", &instr.reply_to)
        .map_err(|e| format!("set Reply-To: {e:?}"))?;
    headers
        .set("X-Cutout-Forwarded", "1")
        .map_err(|e| format!("set X-Cutout-Forwarded: {e:?}"))?;
    headers
        .set("X-Original-From", &instr.original_from)
        .map_err(|e| format!("set X-Original-From: {e:?}"))?;
    let promise = message.forward(&instr.destination, &headers);
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|e| email::send::describe_js_error(&e))?;
    Ok(())
}

/// Persist `raw_bytes` to R2, write a `pending_dispatches` row carrying the
/// still-failing actions, and publish the row id to the `cutout-retries`
/// queue. Returns `Err` only if the operation truly couldn't be durably
/// recorded, in which case the caller logs and the inbound is lost.
async fn enqueue_for_retry(
    env: &Env,
    raw_bytes: &[u8],
    sender: &str,
    recipient: &str,
    rule_id: Option<&str>,
    failed_actions: &[types::PendingAction],
    last_error: &str,
) -> Result<()> {
    let id = uuid::Uuid::new_v4().to_string();
    let r2_key = r2::pending_key(&id);
    r2::put(env, &r2_key, raw_bytes).await?;

    let database = env.d1("DB")?;
    let pending = types::PendingDispatch {
        id: id.clone(),
        sender: sender.to_string(),
        recipient: recipient.to_string(),
        rule_id: rule_id.map(str::to_string),
        r2_key,
        pending_actions: failed_actions.to_vec(),
        attempts: 0,
        last_error: Some(last_error.to_string()),
        dead_lettered: false,
    };
    db::insert_pending(&database, &pending).await?;

    let queue = env.queue("RETRIES")?;
    queue.send(&types::RetryMsg { id }).await?;

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
        p if p.starts_with("/m/") && method == Method::Get => {
            handle_public_viewer(req, env, p).await
        }
        _ => Response::error("Not Found", 404),
    }
}

/// Public viewer route `/m/{id}?t={hmac}`. Validates the token is the
/// HMAC of `id` under `VIEWER_HMAC_KEY` and renders the same viewer used by
/// the Access-protected `/manage/m/{id}` route.
async fn handle_public_viewer(req: Request, env: Env, path: &str) -> Result<Response> {
    let id = match path.strip_prefix("/m/") {
        Some(rest) if !rest.is_empty() && !rest.contains('/') => rest,
        _ => return Response::error("Not Found", 404),
    };

    let url = req.url()?;
    let token = url
        .query_pairs()
        .find(|(k, _)| k == "t")
        .map(|(_, v)| v.into_owned())
        .unwrap_or_default();
    if token.is_empty() {
        return Response::error("Forbidden", 403);
    }

    let key = match env.secret("VIEWER_HMAC_KEY") {
        Ok(s) => s.to_string(),
        Err(_) => {
            console_log!("/m/: VIEWER_HMAC_KEY not configured; refusing token-mode viewer");
            return Response::error("Viewer not configured", 503);
        }
    };

    if !manage::viewer::verify_signed_id(&key, id, &token) {
        return Response::error("Forbidden", 403);
    }

    manage::viewer::render(&env, id).await
}

/// Queue consumer dispatcher. Two queues land on the same worker:
/// - `cutout-retries`     → re-run a previously-failed dispatch.
/// - `cutout-retries-dlq` → terminal handling: send DSN to original sender,
///   mark the row dead-lettered.
#[event(queue)]
async fn queue(batch: MessageBatch<types::RetryMsg>, env: Env, _ctx: Context) -> Result<()> {
    console_error_panic_hook::set_once();
    match batch.queue().as_str() {
        "cutout-retries" => retries::handle(batch, env).await,
        "cutout-retries-dlq" => dlq::handle(batch, env).await,
        other => {
            console_log!("queue handler: unknown queue {other}");
            Ok(())
        }
    }
}
