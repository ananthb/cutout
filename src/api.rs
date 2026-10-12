//! The JSON API the Elm frontend talks to.
//!
//! Everything here is behind Cloudflare Access: [`handle`] verifies the Access
//! JWT before dispatching, and answers 401 with the same `{"error":{…}}`
//! envelope every other failure uses, so the frontend renders one shape.
//!
//! The API is the *only* interface the frontend has. The Worker never renders
//! management HTML any more — it serves the SPA shell (`src/shell.rs`) and this
//! module, and the stored-email viewer (`src/manage/viewer.rs`), which is
//! sandboxed HTML that Elm could not host as safely.
//!
//! # Concurrency
//!
//! Rule writes are compare-and-swap on a monotonic `rules_version`. A write
//! whose version is stale gets a 409 with code `conflict`; the frontend refetches
//! and shows a banner rather than clobbering another operator's change.

use serde_json::{json, Value};
use worker::*;

use crate::bots::EnabledChannels;
use crate::db;
use crate::events;
use crate::helpers::generate_id;
use crate::kv;
use crate::manage::access;
use crate::manage::viewer;
use crate::stats;
use crate::types::{Action, PendingDispatch, Rule};
use crate::validation;

/// Page size for the per-rule stored-email list. Mirrored by the frontend's
/// "load more" behaviour: a full page means there may be more.
const RULE_MESSAGES_PAGE_SIZE: u32 = 25;

/// Page size for the `/inbox` list, matching the per-rule list so "load more"
/// behaves identically on both surfaces.
const INBOX_PAGE_SIZE: u32 = 25;

// ---------------------------------------------------------------------------
// Responses
// ---------------------------------------------------------------------------

fn json_ok(value: &Value) -> Result<Response> {
    let mut resp = Response::from_json(value)?;
    resp.headers_mut().set("Cache-Control", "no-store")?;
    Ok(resp)
}

fn json_error(status: u16, code: &str, message: &str) -> Result<Response> {
    let body = json!({ "error": { "code": code, "message": message } });
    let mut resp = Response::from_json(&body)?.with_status(status);
    resp.headers_mut().set("Cache-Control", "no-store")?;
    Ok(resp)
}

fn unauthorized() -> Result<Response> {
    json_error(
        401,
        "unauthenticated",
        "Cloudflare Access required. Sign in and try again.",
    )
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Handle `/api/*`. `path` is the full request path, e.g. `/api/rules/abc`.
pub async fn handle(mut req: Request, env: Env, path: &str, method: Method) -> Result<Response> {
    let operator = match access::verify_access(&req, &env).await {
        Some(email) => email,
        None => return unauthorized(),
    };

    let rest = path.strip_prefix("/api").unwrap_or("");
    let parts: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();

    // Every arm answers `Result<Response>`; an internal failure (a D1/R2 error,
    // a decoder mismatch) becomes a JSON 500 the frontend can render, rather
    // than a bare Cloudflare error page with no detail.
    let result: Result<Response> = match (method, parts.as_slice()) {
        (Method::Get, ["bootstrap"]) => state(&env, &operator).await,

        (Method::Get, ["events"]) => list_events(req, &env).await,

        (Method::Get, ["pending"]) => list_pending(&env).await,
        (Method::Get, ["pending", "count"]) => pending_count(&env).await,
        (Method::Post, ["pending", "retry-all"]) => retry_all_pending(&env).await,
        (Method::Post, ["pending", "discard-all"]) => discard_all_pending(&env).await,
        (Method::Post, ["pending", id, "retry"]) => retry_pending(&env, id).await,
        (Method::Post, ["pending", id, "discard"]) => discard_pending(&env, id).await,

        (Method::Post, ["rules"]) => create_rule(&mut req, &env, &operator).await,
        (Method::Post, ["rules", "reorder"]) => reorder_rules(&mut req, &env, &operator).await,
        (Method::Put, ["rules", id]) => update_rule(&mut req, &env, &operator, id).await,
        (Method::Delete, ["rules", id]) => delete_rule(&mut req, &env, &operator, id).await,
        (Method::Get, ["rules", id, "messages"]) => list_rule_messages(req, &env, id).await,

        (Method::Get, ["messages", id]) => message(&env, id).await,

        (Method::Get, ["inbox"]) => inbox(req, &env, &operator).await,
        (Method::Get, ["inbox", "messages", id]) => inbox_message(&env, &operator, id).await,

        (Method::Get, ["telegram-chats"]) => telegram_chats(&env).await,

        _ => json_error(404, "not_found", "No such endpoint."),
    };

    match result {
        Ok(resp) => Ok(resp),
        Err(e) => {
            console_log!("api {path} failed: {e}");
            json_error(500, "internal", &format!("{e}"))
        }
    }
}

// ---------------------------------------------------------------------------
// State (bootstrap + every mutation response)
// ---------------------------------------------------------------------------

/// Ensure the rules list ends with a Drop catch-all, inserting one on first
/// deploy. Returns the loaded set with its current version.
async fn ensure_catch_all(kv_store: &worker::kv::KvStore) -> Result<kv::RuleSet> {
    let mut set = kv::get_rule_set(kv_store).await?;
    if set.rules.is_empty() || !set.rules.last().is_some_and(|r| r.is_catch_all()) {
        set.rules.push(Rule {
            id: generate_id(),
            local_pattern: "*".into(),
            domain_pattern: "*".into(),
            action: Action::Drop,
            label: "Catch-all".into(),
            inbox_readers: Vec::new(),
        });
        kv::save_rule_set_force(kv_store, &set.rules, set.version).await?;
    }
    Ok(set)
}

/// The full workbench state: everything the first frame needs, and the same
/// shape every rule mutation returns so the frontend can just replace it.
async fn state(env: &Env, operator: &str) -> Result<Response> {
    let kv_store = env.kv("KV")?;
    let set = ensure_catch_all(&kv_store).await?;
    let enabled = EnabledChannels::from_env(env);
    let report = validation::validate(&set.rules, &enabled);
    let stats7d = stats::fetch_7d(env).await;
    let pending = pending_json(env).await;

    let rules: Vec<Value> = set
        .rules
        .iter()
        .enumerate()
        .map(|(i, rule)| {
            let issues = report.issues.get(i).map(Vec::as_slice).unwrap_or(&[]);
            rule_json(rule, issues, stats7d.as_ref())
        })
        .collect();

    json_ok(&json!({
        "operator": operator,
        "enabled": { "telegram": enabled.telegram, "discord": enabled.discord },
        "rules_version": set.version,
        "rules": rules,
        "stats": stats7d,
        "pending": pending,
    }))
}

fn rule_json(
    rule: &Rule,
    issues: &[validation::Issue],
    stats_data: Option<&stats::Stats7d>,
) -> Value {
    let action = serde_json::to_value(&rule.action).unwrap_or(json!({ "type": "drop" }));
    let rule_stats = stats_data.and_then(|s| s.by_rule.get(&rule.id));
    let issues_json: Vec<Value> = issues
        .iter()
        .map(|i| {
            json!({
                "level": if i.is_error() { "error" } else { "warning" },
                "message": i.message(),
            })
        })
        .collect();

    json!({
        "id": rule.id,
        "label": rule.label,
        "display_label": rule.display_label(),
        "local_pattern": rule.local_pattern,
        "domain_pattern": rule.domain_pattern,
        "is_catch_all": rule.is_catch_all(),
        "action": action,
        "inbox_readers": rule.inbox_readers,
        "readers": rule.effective_readers(),
        "issues": issues_json,
        "matches_7d": rule_stats.map(|r| r.matches),
        "last_match_s": rule_stats.and_then(|r| r.last_match_s),
    })
}

// ---------------------------------------------------------------------------
// Rules CRUD
// ---------------------------------------------------------------------------

/// Parse an [`Action`] from the request's `action` object. The shape matches
/// the native serde form (`{"type":"forward","destinations":[…]}`), so the
/// frontend encodes exactly what it decoded.
fn action_from_form(form: &Value) -> std::result::Result<Action, String> {
    let action = form
        .get("action")
        .ok_or_else(|| "missing action".to_string())?;
    serde_json::from_value::<Action>(action.clone()).map_err(|e| format!("action: {e}"))
}

fn field(form: &Value, key: &str) -> String {
    form.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn rules_version_from_form(form: &Value) -> u64 {
    form.get("rules_version")
        .and_then(|v| match v {
            Value::String(s) => s.parse::<u64>().ok(),
            Value::Number(n) => n.as_u64(),
            _ => None,
        })
        .unwrap_or(0)
}

/// Read a JSON array of strings from `form[key]`, trimming, lower-casing and
/// de-duplicating. `None` when the key is absent (an older client), so callers
/// can tell "no readers" apart from "this client doesn't know the field".
fn opt_string_list(form: &Value, key: &str) -> Option<Vec<String>> {
    let arr = form.get(key)?.as_array()?;
    let mut out: Vec<String> = Vec::new();
    for value in arr {
        if let Some(s) = value.as_str() {
            let s = s.trim().to_lowercase();
            if !s.is_empty() && !out.contains(&s) {
                out.push(s);
            }
        }
    }
    Some(out)
}

fn conflict(current_version: u64) -> Result<Response> {
    json_error(
        409,
        "conflict",
        &format!(
            "Another operator just saved (rules are now version {current_version}). \
             Reload to pick up their changes."
        ),
    )
}

fn validation_error(report: &validation::Report) -> Result<Response> {
    let (i, msg) = report
        .first_error()
        .expect("caller checked has_errors first");
    json_error(400, "invalid", &format!("Rule {}: {}", i + 1, msg))
}

async fn create_rule(req: &mut Request, env: &Env, operator: &str) -> Result<Response> {
    let form: Value = req.json().await?;
    let action = match action_from_form(&form) {
        Ok(a) => a,
        Err(e) => return json_error(400, "invalid", &e),
    };

    let kv_store = env.kv("KV")?;
    let expected = rules_version_from_form(&form);
    let mut set = ensure_catch_all(&kv_store).await?;
    if set.version != expected {
        return conflict(set.version);
    }

    let rule = Rule {
        id: generate_id(),
        local_pattern: field(&form, "local_pattern"),
        domain_pattern: field(&form, "domain_pattern"),
        action,
        label: field(&form, "label").trim().to_string(),
        inbox_readers: opt_string_list(&form, "inbox_readers").unwrap_or_default(),
    };

    // Insert before the catch-all, so the pinned rule stays last.
    let insert_pos = set.rules.len().saturating_sub(1);
    set.rules.insert(insert_pos, rule);

    let enabled = EnabledChannels::from_env(env);
    let report = validation::validate(&set.rules, &enabled);
    if report.has_errors() {
        return validation_error(&report);
    }

    match kv::save_rule_set_if_unchanged(&kv_store, &set.rules, set.version).await? {
        kv::SaveOutcome::Saved { .. } => state(env, operator).await,
        kv::SaveOutcome::Conflict { current_version } => conflict(current_version),
    }
}

async fn update_rule(
    req: &mut Request,
    env: &Env,
    operator: &str,
    rule_id: &str,
) -> Result<Response> {
    let form: Value = req.json().await?;
    let action = match action_from_form(&form) {
        Ok(a) => a,
        Err(e) => return json_error(400, "invalid", &e),
    };

    let kv_store = env.kv("KV")?;
    let expected = rules_version_from_form(&form);
    let mut set = kv::get_rule_set(&kv_store).await?;
    if set.version != expected {
        return conflict(set.version);
    }

    let Some(existing) = set.rules.iter_mut().find(|r| r.id == rule_id) else {
        return json_error(404, "not_found", "No rule with that id.");
    };
    existing.label = field(&form, "label").trim().to_string();
    existing.local_pattern = field(&form, "local_pattern");
    existing.domain_pattern = field(&form, "domain_pattern");
    existing.action = action;
    // Preserve existing readers when the client doesn't send the field at all
    // (an older bundle), rather than silently clearing them.
    if let Some(readers) = opt_string_list(&form, "inbox_readers") {
        existing.inbox_readers = readers;
    }

    let enabled = EnabledChannels::from_env(env);
    let report = validation::validate(&set.rules, &enabled);
    if report.has_errors() {
        return validation_error(&report);
    }

    match kv::save_rule_set_if_unchanged(&kv_store, &set.rules, set.version).await? {
        kv::SaveOutcome::Saved { .. } => state(env, operator).await,
        kv::SaveOutcome::Conflict { current_version } => conflict(current_version),
    }
}

async fn delete_rule(
    req: &mut Request,
    env: &Env,
    operator: &str,
    rule_id: &str,
) -> Result<Response> {
    let form: Value = req.json().await.unwrap_or(Value::Null);

    let kv_store = env.kv("KV")?;
    let expected = rules_version_from_form(&form);
    let mut set = kv::get_rule_set(&kv_store).await?;
    if set.version != expected {
        return conflict(set.version);
    }
    if set
        .rules
        .iter()
        .find(|r| r.id == rule_id)
        .is_some_and(Rule::is_catch_all)
    {
        return json_error(400, "invalid", "The catch-all rule can't be deleted.");
    }

    set.rules.retain(|r| r.id != rule_id);
    match kv::save_rule_set_if_unchanged(&kv_store, &set.rules, set.version).await? {
        kv::SaveOutcome::Saved { .. } => state(env, operator).await,
        kv::SaveOutcome::Conflict { current_version } => conflict(current_version),
    }
}

async fn reorder_rules(req: &mut Request, env: &Env, operator: &str) -> Result<Response> {
    let form: Value = req.json().await?;
    let rule_id = form.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let direction = form.get("direction").and_then(|v| v.as_str()).unwrap_or("");

    let kv_store = env.kv("KV")?;
    let expected = rules_version_from_form(&form);
    let mut set = kv::get_rule_set(&kv_store).await?;
    if set.version != expected {
        return conflict(set.version);
    }

    let catch_all_pos = set.rules.len().saturating_sub(1);
    if let Some(pos) = set.rules.iter().position(|r| r.id == rule_id) {
        if set.rules[pos].is_catch_all() {
            // The pinned rule never moves; answer with the current state.
            return state(env, operator).await;
        }
        match direction {
            "up" if pos > 0 => {
                set.rules.swap(pos, pos - 1);
            }
            "down" if pos + 1 < catch_all_pos => {
                set.rules.swap(pos, pos + 1);
            }
            _ => {}
        }
    }

    match kv::save_rule_set_if_unchanged(&kv_store, &set.rules, set.version).await? {
        kv::SaveOutcome::Saved { .. } => state(env, operator).await,
        kv::SaveOutcome::Conflict { current_version } => conflict(current_version),
    }
}

// ---------------------------------------------------------------------------
// Events + pending
// ---------------------------------------------------------------------------

async fn list_events(req: Request, env: &Env) -> Result<Response> {
    let kv_store = env.kv("KV")?;
    let since = req.url().ok().and_then(|u| {
        u.query_pairs()
            .find(|(k, _)| k == "since")
            .map(|(_, v)| v.into_owned())
            .and_then(|s| s.parse::<i64>().ok())
    });
    let events = events::recent(&kv_store, since).await?;
    json_ok(&json!({ "events": events, "now": events::now_ms() }))
}

async fn pending_json(env: &Env) -> Value {
    let counts = match env.d1("DB") {
        Ok(database) => db::count_pending(&database).await.ok(),
        Err(_) => None,
    };
    let (queued, dead) = counts.unwrap_or((0, 0));
    json!({ "queued": queued, "dead_lettered": dead })
}

async fn pending_count(env: &Env) -> Result<Response> {
    json_ok(&pending_json(env).await)
}

fn pending_row_json(p: &PendingDispatch) -> Value {
    json!({
        "id": p.id,
        "sender": p.sender,
        "recipient": p.recipient,
        "rule_id": p.rule_id,
        "attempts": p.attempts,
        "last_error": p.last_error,
        "dead_lettered": p.dead_lettered,
    })
}

async fn list_pending(env: &Env) -> Result<Response> {
    let database = env.d1("DB")?;
    let rows = db::list_pending(&database, 200).await?;
    let rows: Vec<Value> = rows.iter().map(pending_row_json).collect();
    json_ok(&json!({ "rows": rows }))
}

async fn retry_pending(env: &Env, id: &str) -> Result<Response> {
    let database = env.d1("DB")?;
    let Some(pending) = db::load_pending(&database, id).await? else {
        return json_error(404, "not_found", "No pending dispatch with that id.");
    };
    // A dead-lettered row gets a clean second life: clear the flag and reset
    // attempts so the backoff schedule starts over.
    if pending.dead_lettered {
        database
            .prepare(
                "UPDATE pending_dispatches \
                 SET dead_lettered = 0, attempts = 0, updated_at = CURRENT_TIMESTAMP \
                 WHERE id = ?",
            )
            .bind(&[id.into()])?
            .run()
            .await?;
    }
    let queue = env.queue("RETRIES")?;
    queue
        .send(&crate::types::RetryMsg { id: id.to_string() })
        .await?;
    json_ok(&json!({ "ok": true }))
}

async fn discard_pending(env: &Env, id: &str) -> Result<Response> {
    let database = env.d1("DB")?;
    let Some(pending) = db::load_pending(&database, id).await? else {
        return json_error(404, "not_found", "No pending dispatch with that id.");
    };
    db::delete_pending(&database, id).await?;
    crate::r2::delete(env, &pending.r2_key).await.ok();
    json_ok(&json!({ "ok": true }))
}

/// Re-queue every pending row at once. Dead-lettered rows get a clean second
/// life (flag cleared, attempts reset) exactly as the single-row retry does.
async fn retry_all_pending(env: &Env) -> Result<Response> {
    let database = env.d1("DB")?;
    let rows = db::list_pending(&database, 500).await?;
    let queue = env.queue("RETRIES")?;
    let mut count = 0u32;
    for row in &rows {
        if row.dead_lettered {
            database
                .prepare(
                    "UPDATE pending_dispatches \
                     SET dead_lettered = 0, attempts = 0, updated_at = CURRENT_TIMESTAMP \
                     WHERE id = ?",
                )
                .bind(&[row.id.clone().into()])?
                .run()
                .await?;
        }
        queue
            .send(&crate::types::RetryMsg { id: row.id.clone() })
            .await?;
        count += 1;
    }
    json_ok(&json!({ "ok": true, "count": count }))
}

/// Delete every pending row (and its stored body). Terminal — there is no
/// undo, so the frontend confirms first.
async fn discard_all_pending(env: &Env) -> Result<Response> {
    let database = env.d1("DB")?;
    let rows = db::list_pending(&database, 500).await?;
    let mut count = 0u32;
    for row in &rows {
        db::delete_pending(&database, &row.id).await?;
        crate::r2::delete(env, &row.r2_key).await.ok();
        count += 1;
    }
    json_ok(&json!({ "ok": true, "count": count }))
}

// ---------------------------------------------------------------------------
// Stored emails
// ---------------------------------------------------------------------------

async fn list_rule_messages(req: Request, env: &Env, rule_id: &str) -> Result<Response> {
    let database = env.d1("DB")?;
    let before = req.url().ok().and_then(|u| {
        u.query_pairs()
            .find(|(k, _)| k == "before")
            .map(|(_, v)| v.into_owned())
            .filter(|s| !s.is_empty())
    });
    let items = db::list_messages_for_rule(
        &database,
        rule_id,
        RULE_MESSAGES_PAGE_SIZE,
        before.as_deref(),
    )
    .await?;
    let has_more = items.len() as u32 == RULE_MESSAGES_PAGE_SIZE;
    let next_before = items.last().and_then(|m| m.created_at.clone());
    let items: Vec<Value> = items
        .iter()
        .map(|m| {
            json!({
                "id": m.id,
                "sender": m.sender,
                "subject": m.subject,
                "created_at": m.created_at,
            })
        })
        .collect();
    json_ok(&json!({ "messages": items, "has_more": has_more, "next_before": next_before }))
}

/// One stored email, rendered for the inline inspector. The body comes back
/// either as sanitized HTML (hosted in a sandboxed iframe by the frontend) or
/// as plain text, because the frontend can't sanitize HTML itself.
async fn message(env: &Env, id: &str) -> Result<Response> {
    let Some(rendered) = viewer::build_rendered(env, id).await? else {
        return json_error(404, "not_found", "No stored email with that id.");
    };
    json_ok(&rendered_json(rendered))
}

fn rendered_json(rendered: viewer::RenderedEmail) -> Value {
    let body = match (rendered.body_html, rendered.body_text) {
        (Some(html), _) => json!({ "kind": "html", "html": html }),
        (None, Some(text)) => json!({ "kind": "text", "text": text }),
        _ => json!({ "kind": "none" }),
    };
    json!({
        "subject": rendered.subject,
        "from": rendered.from,
        "to": rendered.to,
        "date": rendered.date,
        "body": body,
    })
}

// ---------------------------------------------------------------------------
// Inbox: stored mail readable by the signed-in Access identity
// ---------------------------------------------------------------------------

/// Stored emails the signed-in person is allowed to read. A person reads a
/// rule's stored mail when they're in that rule's effective reader set: its
/// email destinations (implicit recipients) plus any extra `inbox_readers`.
/// The rule config lives in KV, so this loads it, resolves the allowed rule
/// ids, then pages D1.
async fn inbox(req: Request, env: &Env, operator: &str) -> Result<Response> {
    let kv_store = env.kv("KV")?;
    let set = kv::get_rule_set(&kv_store).await?;

    let allowed: Vec<&Rule> = set.rules.iter().filter(|r| r.is_reader(operator)).collect();
    let rule_ids: Vec<String> = allowed.iter().map(|r| r.id.clone()).collect();
    let labels: std::collections::HashMap<&str, String> = allowed
        .iter()
        .map(|r| (r.id.as_str(), r.display_label()))
        .collect();

    let before = req.url().ok().and_then(|u| {
        u.query_pairs()
            .find(|(k, _)| k == "before")
            .map(|(_, v)| v.into_owned())
            .filter(|s| !s.is_empty())
    });

    let database = env.d1("DB")?;
    let items =
        db::list_messages_for_rules(&database, &rule_ids, INBOX_PAGE_SIZE, before.as_deref())
            .await?;
    let has_more = items.len() as u32 == INBOX_PAGE_SIZE;
    let next_before = items.last().and_then(|m| m.created_at.clone());
    let messages: Vec<Value> = items
        .iter()
        .map(|m| {
            let rule_label = m.rule_id.as_deref().and_then(|id| labels.get(id)).cloned();
            json!({
                "id": m.id,
                "sender": m.sender,
                "subject": m.subject,
                "rule_id": m.rule_id,
                "rule_label": rule_label,
                "created_at": m.created_at,
            })
        })
        .collect();

    json_ok(&json!({
        "operator": operator,
        "messages": messages,
        "has_more": has_more,
        "next_before": next_before,
    }))
}

/// Fetch one inbox email. Unlike the operator-facing [`message`], this checks
/// the signed-in person is in the reader set of the email's rule; a non-reader
/// gets a 403 rather than someone else's mail.
async fn inbox_message(env: &Env, operator: &str, id: &str) -> Result<Response> {
    let database = env.d1("DB")?;
    let Some(meta) = db::get_message_meta(&database, id).await? else {
        return json_error(404, "not_found", "No stored email with that id.");
    };
    let Some(rule_id) = meta.rule_id.as_deref() else {
        return json_error(
            403,
            "forbidden",
            "That email isn't attached to a rule, so it has no readers.",
        );
    };

    let kv_store = env.kv("KV")?;
    let set = kv::get_rule_set(&kv_store).await?;
    let rule = set.rules.iter().find(|r| r.id == rule_id);
    let allowed = rule.is_some_and(|r| r.is_reader(operator));
    if !allowed {
        return json_error(403, "forbidden", "You aren't a reader for this email.");
    }

    let Some(rendered) = viewer::build_rendered(env, id).await? else {
        return json_error(404, "not_found", "No stored email with that id.");
    };
    json_ok(&rendered_json(rendered))
}

async fn telegram_chats(env: &Env) -> Result<Response> {
    let database = env.d1("DB")?;
    let chats = db::list_recent_telegram_chats(&database, 20).await?;
    let chats: Vec<Value> = chats
        .iter()
        .map(|c| json!({ "chat_id": c.chat_id, "label": c.label }))
        .collect();
    json_ok(&json!({ "chats": chats }))
}
