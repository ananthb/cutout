//! Queue consumer for `cutout-retries`. Each message carries a
//! `pending_dispatches` row id; we re-run the still-failing actions and
//! either delete the row (full success), update it (partial success), or
//! retry (total failure). Per-message retry uses an exponential backoff
//! schedule so escalating failures don't pummel the upstream API.

use worker::*;

use crate::db;
use crate::email::handler;
use crate::r2;
use crate::types::RetryMsg;

/// Backoff sequence indexed by current `attempts`. After 8 attempts the
/// platform moves the message to `cutout-retries-dlq`.
const BACKOFF_SECONDS: &[u32] = &[60, 120, 300, 900, 3600, 10800, 21600, 43200];

pub async fn handle(batch: MessageBatch<RetryMsg>, env: Env) -> Result<()> {
    let database = env.d1("DB")?;

    for msg_result in batch.iter() {
        let msg = match msg_result {
            Ok(m) => m,
            Err(e) => {
                console_log!("retries: malformed message body: {e}");
                continue;
            }
        };
        let id = &msg.body().id;

        let pending = match db::load_pending(&database, id).await? {
            Some(p) if !p.dead_lettered => p,
            Some(_) => {
                // Already dead-lettered; ack and move on.
                msg.ack();
                continue;
            }
            None => {
                // Already deleted — operator may have discarded it. Ack.
                msg.ack();
                continue;
            }
        };

        let (still_failing, errors) =
            handler::execute_pending_actions(&env, &pending.pending_actions).await;

        if still_failing.is_empty() {
            // All destinations succeeded on retry. Delete the row + R2 object.
            if let Err(e) = db::delete_pending(&database, id).await {
                console_log!("retries: delete_pending {id} failed: {e}");
            }
            r2::delete(&env, &pending.r2_key).await.ok();
            msg.ack();
            console_log!(
                "retries: {id} succeeded after {} attempts",
                pending.attempts + 1
            );
            continue;
        }

        // Persist what's still failing and bump attempts.
        let last_error = errors.join("; ");
        if let Err(e) =
            db::update_pending_after_attempt(&database, id, &still_failing, Some(&last_error)).await
        {
            console_log!("retries: update_pending {id} failed: {e}");
        }

        let next_attempt = (pending.attempts + 1) as usize;
        let delay = BACKOFF_SECONDS
            .get(next_attempt.min(BACKOFF_SECONDS.len() - 1))
            .copied()
            .unwrap_or(*BACKOFF_SECONDS.last().unwrap());
        let opts = QueueRetryOptionsBuilder::new()
            .with_delay_seconds(delay)
            .build();
        msg.retry_with_options(&opts);
        console_log!(
            "retries: {id} attempt {} failed ({last_error}); retrying in {delay}s",
            pending.attempts + 1
        );
    }
    Ok(())
}
