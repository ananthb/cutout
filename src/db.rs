use crate::types::ReverseAlias;
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

pub async fn save_message(
    db: &D1Database,
    sender: &str,
    recipient: &str,
    subject: &str,
    text_body: Option<&str>,
    html_body: Option<&str>,
) -> Result<()> {
    db.prepare(
        "INSERT INTO messages (sender, recipient, subject, text_body, html_body) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&[
        sender.into(),
        recipient.into(),
        subject.into(),
        text_body.into(),
        html_body.into(),
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
