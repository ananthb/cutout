//! CRUD handlers for routing rules plus the rule tester.

use worker::*;

use super::templates;
use super::viewer;
use crate::bots::EnabledChannels;
use crate::db;
use crate::events;
use crate::helpers::generate_id;
use crate::kv;
use crate::r2;
use crate::stats;
use crate::types::*;
use crate::validation;

/// Page size for the per-rule "Stored emails" list (and the cursor that
/// drives "Load more" pagination).
const RULE_MESSAGES_PAGE_SIZE: u32 = 25;

/// Fetch the first page of stored emails for `selected` (when set), so the
/// inspector can render the list inline without a second round-trip.
async fn fetch_messages_for_selected(
    env: &Env,
    selected: Option<&str>,
) -> Option<Vec<db::MessageListItem>> {
    let rule_id = selected?;
    let database = env.d1("DB").ok()?;
    db::list_messages_for_rule(&database, rule_id, RULE_MESSAGES_PAGE_SIZE, None)
        .await
        .ok()
}

/// Ensure the rules list has a catch-all as the last rule. If the
/// stored set is missing one (only happens on first deploy), inserts a
/// default Drop catch-all and force-saves it. Returns the loaded set
/// with its current version so callers can thread it through CAS saves.
async fn ensure_catch_all(kv_store: &worker::kv::KvStore) -> Result<kv::RuleSet> {
    let mut set = kv::get_rule_set(kv_store).await?;
    if set.rules.is_empty() || !set.rules.last().is_some_and(|r| r.is_catch_all()) {
        let catch_all = Rule {
            id: generate_id(),
            local_pattern: "*".into(),
            domain_pattern: "*".into(),
            action: Action::Drop,
            label: "Catch-all".into(),
        };
        set.rules.push(catch_all);
        kv::save_rule_set_force(kv_store, &set.rules, set.version).await?;
    }
    Ok(set)
}

/// Read the editor's expected `rules_version` from the form. A missing
/// or unparseable field is taken as version 0, which only ever matches
/// a freshly-bootstrapped store, so any concurrent change will already
/// produce a conflict.
fn version_from_form(form: &serde_json::Value) -> u64 {
    form.get("rules_version")
        .and_then(|v| match v {
            serde_json::Value::String(s) => s.parse::<u64>().ok(),
            serde_json::Value::Number(n) => n.as_u64(),
            _ => None,
        })
        .unwrap_or(0)
}

/// HTMX-friendly 409 for the "another operator saved while you were
/// editing" case. Emits an `HX-Trigger: rule-conflict` event header so
/// a global Alpine listener can show a sticky banner; the body itself
/// is a fallback message in case the event isn't wired up.
fn conflict_response(current_version: u64) -> Result<Response> {
    let body = format!(
        "Rules version conflict: another operator saved (now version {current_version}). Refresh and try again."
    );
    let mut resp = Response::ok(body)?.with_status(409);
    let headers = resp.headers_mut();
    headers.set(
        "HX-Trigger",
        &format!(r#"{{"rule-conflict":{{"current_version":{current_version}}}}}"#),
    )?;
    headers.set("HX-Reswap", "none")?;
    Ok(resp)
}

/// Parse an Action from form JSON. The `destinations` field is a
/// newline-separated list of `kind:value` lines (see [`Destination::parse_list`]).
fn parse_action(form: &serde_json::Value) -> std::result::Result<Action, String> {
    let action_type = form
        .get("action_type")
        .and_then(|v| v.as_str())
        .unwrap_or("drop");

    match action_type {
        "forward" => {
            let raw = form
                .get("destinations")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let destinations = Destination::parse_list(raw)?;
            Ok(Action::Forward { destinations })
        }
        "store" => {
            let persist = form
                .get("persist")
                .and_then(|v| {
                    if v.is_boolean() {
                        v.as_bool()
                    } else {
                        v.as_str().map(|s| s == "true" || s == "on")
                    }
                })
                .unwrap_or(false);
            Ok(Action::Store { persist })
        }
        _ => Ok(Action::Drop),
    }
}

/// Read the requested "selected rule" from a form payload.
fn selected_from_form(form: &serde_json::Value) -> Option<&str> {
    form.get("selected")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
}

/// Read `?rule=…` from the request URL.
fn selected_from_query(req: &Request) -> Option<String> {
    let url = req.url().ok()?;
    url.query_pairs()
        .find(|(k, _)| k == "rule")
        .map(|(_, v)| v.into_owned())
        .filter(|s| !s.is_empty())
}

async fn render_workbench(
    env: &Env,
    rules: &[Rule],
    version: u64,
    enabled: &EnabledChannels,
    selected: Option<&str>,
) -> String {
    let report = validation::validate(rules, enabled);
    let idx = templates::pick_selected_idx(rules, selected);
    let stats = stats::fetch_7d(env).await;
    let messages = fetch_messages_for_selected(env, selected).await;
    templates::workbench_response(
        rules,
        version,
        &report,
        enabled,
        idx,
        stats.as_ref(),
        messages.as_deref(),
    )
}

/// If validation rejects the proposed rule set, return a 400 response with a
/// human-readable error.
fn validation_error_response(report: &validation::Report) -> Result<Response> {
    let (i, msg) = report
        .first_error()
        .expect("caller checked has_errors first");
    Response::error(format!("rule {}: {}", i + 1, msg), 400)
}

/// GET /manage: list rules
pub async fn list_rules(req: Request, env: &Env, email: &str) -> Result<Response> {
    let kv_store = env.kv("KV")?;
    let set = ensure_catch_all(&kv_store).await?;
    let enabled = EnabledChannels::from_env(env);
    let report = validation::validate(&set.rules, &enabled);
    let selected = selected_from_query(&req);
    let stats = stats::fetch_7d(env).await;
    let messages = fetch_messages_for_selected(env, selected.as_deref()).await;

    Response::from_html(templates::rules_page(
        &set.rules,
        set.version,
        email,
        &report,
        &enabled,
        selected.as_deref(),
        stats.as_ref(),
        messages.as_deref(),
    ))
}

/// GET /manage/rules/new: return the new-rule modal as an HTMX partial.
pub async fn new_rule_form(env: &Env) -> Result<Response> {
    let enabled = EnabledChannels::from_env(env);
    Response::from_html(templates::new_rule_modal(&enabled))
}

/// POST /manage/rules: create a new rule (inserted before catch-all)
pub async fn create_rule(mut req: Request, env: &Env) -> Result<Response> {
    let kv_store = env.kv("KV")?;
    let form: serde_json::Value = req.json().await?;

    let action = match parse_action(&form) {
        Ok(a) => a,
        Err(e) => return Response::error(format!("destinations: {e}"), 400),
    };

    let expected_version = version_from_form(&form);
    let mut set = ensure_catch_all(&kv_store).await?;
    if set.version != expected_version {
        return conflict_response(set.version);
    }

    let rule = Rule {
        id: generate_id(),
        local_pattern: form
            .get("local_pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("*")
            .to_string(),
        domain_pattern: form
            .get("domain_pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("*")
            .to_string(),
        action,
        label: form
            .get("label")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string(),
    };

    let new_id = rule.id.clone();
    let insert_pos = set.rules.len().saturating_sub(1);
    set.rules.insert(insert_pos, rule);

    let enabled = EnabledChannels::from_env(env);
    let report = validation::validate(&set.rules, &enabled);
    if report.has_errors() {
        return validation_error_response(&report);
    }

    let new_version = match kv::save_rule_set_if_unchanged(&kv_store, &set.rules, set.version)
        .await?
    {
        kv::SaveOutcome::Saved { new_version } => new_version,
        kv::SaveOutcome::Conflict { current_version } => return conflict_response(current_version),
    };
    Response::from_html(
        render_workbench(env, &set.rules, new_version, &enabled, Some(&new_id)).await,
    )
}

/// GET /manage/rules/{id}/edit: return edit form partial
pub async fn edit_form(env: &Env, rule_id: &str) -> Result<Response> {
    let kv_store = env.kv("KV")?;
    let rules = kv::get_rules(&kv_store).await?;
    let enabled = EnabledChannels::from_env(env);

    match rules.iter().find(|r| r.id == rule_id) {
        Some(rule) => Response::from_html(templates::edit_rule_form(rule, &enabled)),
        None => Response::error("Rule not found", 404),
    }
}

/// PUT /manage/rules/{id}: update a rule
pub async fn update_rule(mut req: Request, env: &Env, rule_id: &str) -> Result<Response> {
    let kv_store = env.kv("KV")?;
    let form: serde_json::Value = req.json().await?;

    let action = match parse_action(&form) {
        Ok(a) => a,
        Err(e) => return Response::error(format!("destinations: {e}"), 400),
    };

    let expected_version = version_from_form(&form);
    let mut set = kv::get_rule_set(&kv_store).await?;
    if set.version != expected_version {
        return conflict_response(set.version);
    }

    if let Some(existing) = set.rules.iter_mut().find(|r| r.id == rule_id) {
        existing.label = form
            .get("label")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| existing.label.clone());
        existing.local_pattern = form
            .get("local_pattern")
            .and_then(|v| v.as_str())
            .unwrap_or(&existing.local_pattern)
            .to_string();
        existing.domain_pattern = form
            .get("domain_pattern")
            .and_then(|v| v.as_str())
            .unwrap_or(&existing.domain_pattern)
            .to_string();
        existing.action = action;
    }

    let enabled = EnabledChannels::from_env(env);
    let report = validation::validate(&set.rules, &enabled);
    if report.has_errors() {
        return validation_error_response(&report);
    }

    let new_version = match kv::save_rule_set_if_unchanged(&kv_store, &set.rules, set.version)
        .await?
    {
        kv::SaveOutcome::Saved { new_version } => new_version,
        kv::SaveOutcome::Conflict { current_version } => return conflict_response(current_version),
    };
    Response::from_html(
        render_workbench(env, &set.rules, new_version, &enabled, Some(rule_id)).await,
    )
}

/// DELETE /manage/rules/{id}: delete a rule (blocked for catch-all)
pub async fn delete_rule(mut req: Request, env: &Env, rule_id: &str) -> Result<Response> {
    let kv_store = env.kv("KV")?;
    let form: serde_json::Value = req.json().await.unwrap_or(serde_json::Value::Null);

    let expected_version = version_from_form(&form);
    let mut set = kv::get_rule_set(&kv_store).await?;
    if set.version != expected_version {
        return conflict_response(set.version);
    }

    if let Some(rule) = set.rules.iter().find(|r| r.id == rule_id) {
        if rule.is_catch_all() {
            return Response::error("Cannot delete the catch-all rule", 400);
        }
    }

    // Best-effort selection preservation: if the deleted rule was selected,
    // fall back to whatever the form said (or default in template).
    let selected = selected_from_form(&form)
        .filter(|s| *s != rule_id)
        .map(str::to_string);

    set.rules.retain(|r| r.id != rule_id);
    let new_version = match kv::save_rule_set_if_unchanged(&kv_store, &set.rules, set.version)
        .await?
    {
        kv::SaveOutcome::Saved { new_version } => new_version,
        kv::SaveOutcome::Conflict { current_version } => return conflict_response(current_version),
    };
    let enabled = EnabledChannels::from_env(env);
    Response::from_html(
        render_workbench(env, &set.rules, new_version, &enabled, selected.as_deref()).await,
    )
}

/// POST /manage/rules/reorder: move a rule up or down
pub async fn reorder_rules(mut req: Request, env: &Env) -> Result<Response> {
    let kv_store = env.kv("KV")?;
    let form: serde_json::Value = req.json().await?;

    let rule_id = form.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let direction = form.get("direction").and_then(|v| v.as_str()).unwrap_or("");
    let selected = selected_from_form(&form).map(str::to_string);

    let expected_version = version_from_form(&form);
    let mut set = kv::get_rule_set(&kv_store).await?;
    if set.version != expected_version {
        return conflict_response(set.version);
    }

    let enabled = EnabledChannels::from_env(env);
    if let Some(pos) = set.rules.iter().position(|r| r.id == rule_id) {
        if set.rules[pos].is_catch_all() {
            return Response::from_html(
                render_workbench(env, &set.rules, set.version, &enabled, selected.as_deref()).await,
            );
        }

        let catch_all_pos = set.rules.len().saturating_sub(1);

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

    let new_version = match kv::save_rule_set_if_unchanged(&kv_store, &set.rules, set.version)
        .await?
    {
        kv::SaveOutcome::Saved { new_version } => new_version,
        kv::SaveOutcome::Conflict { current_version } => return conflict_response(current_version),
    };
    Response::from_html(
        render_workbench(env, &set.rules, new_version, &enabled, selected.as_deref()).await,
    )
}

/// GET /manage/events?since={unix_ms}: JSON tail of the event ring buffer.
/// Returns `{events: [...], now: <unix_ms>}`. Polled by the dashboard.
pub async fn list_events(req: Request, env: &Env) -> Result<Response> {
    let kv_store = env.kv("KV")?;
    let since = req
        .url()
        .ok()
        .and_then(|u| {
            u.query_pairs()
                .find(|(k, _)| k == "since")
                .map(|(_, v)| v.into_owned())
        })
        .and_then(|s| s.parse::<i64>().ok());

    let events = events::recent(&kv_store, since).await?;
    let body = serde_json::json!({
        "events": events,
        "now": events::now_ms(),
    });
    let mut resp = Response::from_json(&body)?;
    let headers = resp.headers_mut();
    headers.set("Cache-Control", "no-store")?;
    Ok(resp)
}

/// GET /manage/api/recent-telegram-chats: JSON list of chats that have sent
/// `/start` to the bot, newest first. Capped at 20 entries; the rule editor's
/// destination autofill consumes this.
pub async fn recent_telegram_chats(env: &Env) -> Result<Response> {
    let database = env.d1("DB")?;
    let chats = db::list_recent_telegram_chats(&database, 20).await?;
    let mut resp = Response::from_json(&chats)?;
    resp.headers_mut().set("Cache-Control", "no-store")?;
    Ok(resp)
}

/// GET /manage/assets/cutout-mark.svg: favicon / brand mark.
pub async fn brand_mark() -> Result<Response> {
    let mut resp = Response::ok(templates::LOGO_SVG_FILE)?;
    let headers = resp.headers_mut();
    headers.set("Content-Type", "image/svg+xml")?;
    headers.set("Cache-Control", "public, max-age=86400")?;
    Ok(resp)
}

/// GET /manage/pending: full HTML page listing queued + dead-lettered rows.
pub async fn list_pending(env: &Env) -> Result<Response> {
    let database = env.d1("DB")?;
    let rows = db::list_pending(&database, 200).await?;
    let html = templates::pending_page(&rows);
    Response::from_html(html)
}

/// GET /manage/pending/count: tiny JSON for the live-feed widget. Kept
/// separate from `/manage/events` so the widget poll doesn't have to parse
/// the whole event buffer.
pub async fn pending_count(env: &Env) -> Result<Response> {
    let database = env.d1("DB")?;
    let (queued, dead) = db::count_pending(&database).await?;
    let body = serde_json::json!({ "queued": queued, "dead_lettered": dead });
    let mut resp = Response::from_json(&body)?;
    resp.headers_mut().set("Cache-Control", "no-store")?;
    Ok(resp)
}

/// POST /manage/pending/{id}/retry: re-publish to `cutout-retries`. The
/// consumer will pick it up on the next batch and re-run the dispatch.
/// Redirects back to /manage/pending so the operator sees the row's new
/// state instead of a bare "requeued" string.
pub async fn retry_pending(env: &Env, id: &str) -> Result<Response> {
    let database = env.d1("DB")?;
    let pending = match db::load_pending(&database, id).await? {
        Some(p) => p,
        None => return Response::error("Not Found", 404),
    };
    // If the row was previously dead-lettered we want a clean second life:
    // clear the flag and reset attempts so the backoff schedule starts over.
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
    queue.send(&RetryMsg { id: id.to_string() }).await?;
    let headers = Headers::new();
    headers.set("Location", "/manage/pending")?;
    Ok(Response::empty()?.with_status(303).with_headers(headers))
}

/// GET /manage/rules/{id}/messages?before={ts}: HTML fragment listing the
/// next page of stored emails for a rule (newest first). Used by the
/// inspector's "Load more" button (HTMX appends rows into the list).
pub async fn list_rule_messages(req: Request, env: &Env, rule_id: &str) -> Result<Response> {
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
    Response::from_html(templates::rule_messages_list(rule_id, &items, true))
}

/// GET /manage/rules/{id}/messages/{msg_id}: HTML fragment with one
/// stored email's expanded body. Verifies the message belongs to the
/// rule (defense in depth: Cloudflare Access already gates the route).
pub async fn rule_message_fragment(env: &Env, rule_id: &str, msg_id: &str) -> Result<Response> {
    let database = env.d1("DB")?;
    let meta = match db::get_message_meta(&database, msg_id).await? {
        Some(m) => m,
        None => return Response::error("Not Found", 404),
    };
    if meta.rule_id.as_deref() != Some(rule_id) {
        return Response::error("Not Found", 404);
    }
    let rendered = match viewer::build_rendered(env, msg_id).await? {
        Some(r) => r,
        None => return Response::error("Not Found", 404),
    };
    let mut resp = Response::from_html(templates::rule_message_fragment(msg_id, &rendered))?;
    let headers = resp.headers_mut();
    headers.set("Cache-Control", "private, no-store")?;
    headers.set("X-Content-Type-Options", "nosniff")?;
    Ok(resp)
}

/// POST /manage/pending/{id}/discard: delete the row + its R2 object. Used
/// to cut a dead-lettered email loose; the operator has decided not to
/// re-attempt it. Redirects back to /manage/pending so the operator sees
/// the row gone instead of a bare "discarded" string.
pub async fn discard_pending(env: &Env, id: &str) -> Result<Response> {
    let database = env.d1("DB")?;
    let pending = match db::load_pending(&database, id).await? {
        Some(p) => p,
        None => return Response::error("Not Found", 404),
    };
    db::delete_pending(&database, id).await?;
    r2::delete(env, &pending.r2_key).await.ok();
    let headers = Headers::new();
    headers.set("Location", "/manage/pending")?;
    Ok(Response::empty()?.with_status(303).with_headers(headers))
}
