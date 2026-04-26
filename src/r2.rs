//! Thin wrapper around the `EMAILS` R2 bucket binding. Centralises key
//! conventions and stream-to-bytes plumbing so callers in `lib.rs`,
//! `email::handler`, `retries`, and `dlq` all speak the same vocabulary.
//!
//! Key prefixes:
//! - `pending/<id>` — raw bytes of an inbound email queued for retry.
//! - `messages/<id>` — raw bytes saved by the `Store` action.

use worker::*;

const BINDING: &str = "EMAILS";

/// Build the R2 key for a pending-retry blob.
pub fn pending_key(id: &str) -> String {
    format!("pending/{id}")
}

/// Build the R2 key for a Store-action blob.
pub fn message_key(id: &str) -> String {
    format!("messages/{id}")
}

/// Upload `bytes` to `key` in the EMAILS bucket. Strongly consistent: once
/// this resolves, every subsequent `get` will see it.
pub async fn put(env: &Env, key: &str, bytes: &[u8]) -> Result<()> {
    let bucket = env.bucket(BINDING)?;
    bucket.put(key, bytes.to_vec()).execute().await?;
    Ok(())
}

/// Read an object's body to a byte vector. Returns `None` when the key
/// doesn't exist; errors only on infrastructure failure (binding missing,
/// network, etc.).
#[allow(dead_code)]
pub async fn get(env: &Env, key: &str) -> Result<Option<Vec<u8>>> {
    let bucket = env.bucket(BINDING)?;
    let object = match bucket.get(key).execute().await? {
        Some(o) => o,
        None => return Ok(None),
    };
    let body = match object.body() {
        Some(b) => b,
        None => return Ok(None),
    };
    let bytes = body.bytes().await?;
    Ok(Some(bytes))
}

/// Best-effort delete. Logs and returns `Ok(())` on failure: the row that
/// pointed at this key is already being cleaned up; an orphaned R2 object
/// is recoverable, but a propagated error here would block the caller's
/// own cleanup.
pub async fn delete(env: &Env, key: &str) -> Result<()> {
    let bucket = env.bucket(BINDING)?;
    if let Err(e) = bucket.delete(key).await {
        console_log!("R2 delete {key} failed (ignored): {e}");
    }
    Ok(())
}
