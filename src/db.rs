use crate::types::{PendingAction, PendingDispatch, ReverseAlias};
use botrelay::reply::ReplyContext;
use serde::Deserialize;
use worker::d1::*;
use worker::*;

pub async fn save_reverse_mapping(
    db: &D1Database,
    id: &str,
    alias: &str,
    original_sender: &str,
) -> Result<()> {
    db.prepare(
        "INSERT INTO reverse_mappings (id, alias_address, original_sender) VALUES (?, ?, ?)",
    )
    .bind(&[id.into(), alias.into(), original_sender.into()])?
    .run()
    .await?;
    Ok(())
}

pub async fn get_reverse_mapping(db: &D1Database, id: &str) -> Result<Option<ReverseAlias>> {
    let result = db
        .prepare("SELECT alias_address, original_sender FROM reverse_mappings WHERE id = ?")
        .bind(&[id.into()])?
        .first::<ReverseMappingRow>(None)
        .await?;

    Ok(result.map(|r| ReverseAlias {
        alias: r.alias_address,
        original_sender: r.original_sender,
    }))
}

#[derive(Deserialize)]
struct ReverseMappingRow {
    alias_address: String,
    original_sender: String,
}

pub async fn save_bot_ctx(db: &D1Database, key: &str, ctx: &ReplyContext) -> Result<()> {
    db.prepare("INSERT INTO bot_reply_contexts (key, alias_address, original_sender, subject) VALUES (?, ?, ?, ?) ON CONFLICT(key) DO UPDATE SET alias_address=excluded.alias_address")
        .bind(&[key.into(), ctx.alias.clone().into(), ctx.original_sender.clone().into(), ctx.subject.clone().into()])?
        .run()
        .await?;
    Ok(())
}

pub async fn get_bot_ctx(db: &D1Database, key: &str) -> Result<Option<ReplyContext>> {
    let row = db
        .prepare(
            "SELECT alias_address, original_sender, subject FROM bot_reply_contexts WHERE key = ?",
        )
        .bind(&[key.into()])?
        .first::<ReplyContextRow>(None)
        .await?;
    Ok(row.map(|r| ReplyContext {
        alias: r.alias_address,
        original_sender: r.original_sender,
        subject: r.subject.unwrap_or_default(),
    }))
}

/// Insert a metadata row pointing at the R2 object that holds the body.
/// The body itself must already have been uploaded to R2 at `r2_key` (see
/// [`crate::r2::message_key`]).
pub async fn save_message(
    db: &D1Database,
    id: &str,
    sender: &str,
    recipient: &str,
    subject: &str,
    r2_key: &str,
) -> Result<()> {
    db.prepare(
        "INSERT INTO messages (id, sender, recipient, subject, r2_key) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&[
        id.into(),
        sender.into(),
        recipient.into(),
        subject.into(),
        r2_key.into(),
    ])?
    .run()
    .await?;
    Ok(())
}

#[derive(Deserialize)]
struct ReplyContextRow {
    alias_address: String,
    original_sender: String,
    subject: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────
// pending_dispatches: failed dispatches awaiting retry via Cloudflare Queues.

#[derive(Deserialize)]
struct PendingRow {
    id: String,
    sender: String,
    recipient: String,
    rule_id: Option<String>,
    r2_key: String,
    pending_actions: String,
    attempts: u32,
    last_error: Option<String>,
    dead_lettered: u32,
}

fn row_to_pending(row: PendingRow) -> Result<PendingDispatch> {
    let actions: Vec<PendingAction> = serde_json::from_str(&row.pending_actions)
        .map_err(|e| Error::from(format!("decode pending_actions: {e}")))?;
    Ok(PendingDispatch {
        id: row.id,
        sender: row.sender,
        recipient: row.recipient,
        rule_id: row.rule_id,
        r2_key: row.r2_key,
        pending_actions: actions,
        attempts: row.attempts,
        last_error: row.last_error,
        dead_lettered: row.dead_lettered != 0,
    })
}

pub async fn insert_pending(db: &D1Database, p: &PendingDispatch) -> Result<()> {
    let actions_json = serde_json::to_string(&p.pending_actions)
        .map_err(|e| Error::from(format!("encode pending_actions: {e}")))?;
    db.prepare(
        "INSERT INTO pending_dispatches \
         (id, sender, recipient, rule_id, r2_key, pending_actions, attempts, last_error, dead_lettered) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&[
        p.id.clone().into(),
        p.sender.clone().into(),
        p.recipient.clone().into(),
        p.rule_id.clone().into(),
        p.r2_key.clone().into(),
        actions_json.into(),
        (p.attempts as i64).into(),
        p.last_error.clone().into(),
        (p.dead_lettered as i64).into(),
    ])?
    .run()
    .await?;
    Ok(())
}

pub async fn load_pending(db: &D1Database, id: &str) -> Result<Option<PendingDispatch>> {
    let row = db
        .prepare(
            "SELECT id, sender, recipient, rule_id, r2_key, pending_actions, attempts, last_error, dead_lettered \
             FROM pending_dispatches WHERE id = ?",
        )
        .bind(&[id.into()])?
        .first::<PendingRow>(None)
        .await?;
    row.map(row_to_pending).transpose()
}

/// Update the still-failing actions and bump attempts/last_error in one shot.
pub async fn update_pending_after_attempt(
    db: &D1Database,
    id: &str,
    pending_actions: &[PendingAction],
    last_error: Option<&str>,
) -> Result<()> {
    let actions_json = serde_json::to_string(pending_actions)
        .map_err(|e| Error::from(format!("encode pending_actions: {e}")))?;
    db.prepare(
        "UPDATE pending_dispatches \
         SET pending_actions = ?, attempts = attempts + 1, last_error = ?, updated_at = CURRENT_TIMESTAMP \
         WHERE id = ?",
    )
    .bind(&[actions_json.into(), last_error.into(), id.into()])?
    .run()
    .await?;
    Ok(())
}

pub async fn delete_pending(db: &D1Database, id: &str) -> Result<()> {
    db.prepare("DELETE FROM pending_dispatches WHERE id = ?")
        .bind(&[id.into()])?
        .run()
        .await?;
    Ok(())
}

pub async fn mark_dead_lettered(db: &D1Database, id: &str, last_error: Option<&str>) -> Result<()> {
    db.prepare(
        "UPDATE pending_dispatches \
         SET dead_lettered = 1, last_error = COALESCE(?, last_error), updated_at = CURRENT_TIMESTAMP \
         WHERE id = ?",
    )
    .bind(&[last_error.into(), id.into()])?
    .run()
    .await?;
    Ok(())
}

#[derive(Deserialize)]
struct CountRow {
    pending: i64,
    dead: i64,
}

/// Returns `(queued_count, dead_lettered_count)`. Single-query so the live
/// dashboard widget doesn't pay for two D1 round-trips per refresh.
pub async fn count_pending(db: &D1Database) -> Result<(u64, u64)> {
    let row = db
        .prepare(
            "SELECT \
                SUM(CASE WHEN dead_lettered = 0 THEN 1 ELSE 0 END) AS pending, \
                SUM(CASE WHEN dead_lettered = 1 THEN 1 ELSE 0 END) AS dead \
             FROM pending_dispatches",
        )
        .first::<CountRow>(None)
        .await?
        .unwrap_or(CountRow {
            pending: 0,
            dead: 0,
        });
    Ok((row.pending.max(0) as u64, row.dead.max(0) as u64))
}

pub async fn list_pending(db: &D1Database, limit: u32) -> Result<Vec<PendingDispatch>> {
    let result = db
        .prepare(
            "SELECT id, sender, recipient, rule_id, r2_key, pending_actions, attempts, last_error, dead_lettered \
             FROM pending_dispatches \
             ORDER BY dead_lettered DESC, updated_at DESC \
             LIMIT ?",
        )
        .bind(&[(limit as i64).into()])?
        .all()
        .await?;
    let rows: Vec<PendingRow> = result.results()?;
    rows.into_iter().map(row_to_pending).collect()
}
