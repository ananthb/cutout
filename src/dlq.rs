//! Dedicated consumer for `cutout-retries-dlq`. The platform routes a
//! message here once `max_retries` is exceeded on the primary retry queue.
//! Terminal handling: load the row, send an RFC 3464 DSN to the original
//! sender, mark the row dead-lettered, and write a final error event so the
//! operator sees what happened in the live feed. The R2 object is left
//! behind for inspection / manual replay from `/manage/pending`.

use worker::*;

use crate::db;
use crate::email::send;
use crate::events::{self, EventKind};
use crate::types::RetryMsg;

pub async fn handle(batch: MessageBatch<RetryMsg>, env: Env) -> Result<()> {
    let database = env.d1("DB")?;

    for msg_result in batch.iter() {
        let msg = match msg_result {
            Ok(m) => m,
            Err(e) => {
                console_log!("dlq: malformed message body: {e}");
                continue;
            }
        };
        let id = &msg.body().id;

        let pending = match db::load_pending(&database, id).await? {
            Some(p) => p,
            None => {
                console_log!("dlq: row {id} not found; ack");
                msg.ack();
                continue;
            }
        };

        let original_error = pending
            .last_error
            .clone()
            .unwrap_or_else(|| "(no error recorded)".into());

        let dsn_outcome = send::send_dsn(
            &env,
            &pending.sender,
            &pending.recipient,
            None,
            &original_error,
        )
        .await;

        let final_error = match dsn_outcome {
            Ok(()) => format!("dead-lettered, sender notified: {original_error}"),
            Err(e) => {
                console_log!("dlq: DSN send to {} failed: {e}", pending.sender);
                format!("dead-lettered, DSN send failed ({e}): {original_error}")
            }
        };

        if let Err(e) = db::mark_dead_lettered(&database, id, Some(&final_error)).await {
            console_log!("dlq: mark_dead_lettered {id} failed: {e}");
        }

        // Surface in the live feed so the operator sees the terminal state.
        if let Ok(kv) = env.kv("KV") {
            let event = events::Event {
                ts: events::now_ms(),
                kind: EventKind::Error,
                from: pending.sender.clone(),
                to: pending.recipient.clone(),
                rule_id: pending.rule_id.clone(),
                channels: Vec::new(),
                size_bytes: 0,
                error: Some(final_error),
            };
            if let Err(e) = events::record(&env, &kv, &event).await {
                console_log!("dlq: event record failed: {e}");
            }
        }

        msg.ack();
    }
    Ok(())
}
