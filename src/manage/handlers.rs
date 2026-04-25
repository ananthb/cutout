//! CRUD handlers for routing rules plus the rule tester.

use worker::*;

use super::templates;
use crate::bots::EnabledChannels;
use crate::email::routing;
use crate::events;
use crate::helpers::generate_id;
use crate::kv;
use crate::stats;
use crate::types::*;
use crate::validation;

/// Ensure the rules list has a catch-all as the last rule.
/// If empty, creates a default catch-all with Drop action.
async fn ensure_catch_all(kv_store: &worker::kv::KvStore) -> Result<Vec<Rule>> {
    let mut rules = kv::get_rules(kv_store).await?;
    if rules.is_empty() || !rules.last().is_some_and(|r| r.is_catch_all()) {
        let catch_all = Rule {
            id: generate_id(),
            local_pattern: "*".into(),
            domain_pattern: "*".into(),
            action: Action::Drop,
            label: "Catch-all".into(),
        };
        rules.push(catch_all);
        kv::save_rules(kv_store, &rules).await?;
    }
    Ok(rules)
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
            let replace_reply_to = form
                .get("replace_reply_to")
                .and_then(|v| {
                    if v.is_boolean() {
                        v.as_bool()
                    } else {
                        v.as_str().map(|s| s == "true" || s == "on")
                    }
                })
                .unwrap_or(false);
            Ok(Action::Forward {
                destinations,
                replace_reply_to,
            })
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
    enabled: &EnabledChannels,
    selected: Option<&str>,
) -> String {
    let report = validation::validate(rules, enabled);
    let idx = templates::pick_selected_idx(rules, selected);
    let stats = stats::fetch_7d(env).await;
    templates::workbench_response(rules, &report, enabled, idx, stats.as_ref())
}

/// If validation rejects the proposed rule set, return a 400 response with a
/// human-readable error.
fn validation_error_response(report: &validation::Report) -> Result<Response> {
    let (i, msg) = report
        .first_error()
        .expect("caller checked has_errors first");
    Response::error(format!("rule {}: {}", i + 1, msg), 400)
}

/// GET /manage — list rules
pub async fn list_rules(req: Request, env: &Env, email: &str) -> Result<Response> {
    let kv_store = env.kv("KV")?;
    let rules = ensure_catch_all(&kv_store).await?;
    let enabled = EnabledChannels::from_env(env);
    let report = validation::validate(&rules, &enabled);
    let selected = selected_from_query(&req);
    let stats = stats::fetch_7d(env).await;

    Response::from_html(templates::rules_page(
        &rules,
        email,
        &report,
        &enabled,
        selected.as_deref(),
        stats.as_ref(),
    ))
}

/// GET /manage/rules/new — return the new-rule modal as an HTMX partial.
pub async fn new_rule_form(env: &Env) -> Result<Response> {
    let enabled = EnabledChannels::from_env(env);
    Response::from_html(templates::new_rule_modal(&enabled))
}

/// POST /manage/rules — create a new rule (inserted before catch-all)
pub async fn create_rule(mut req: Request, env: &Env) -> Result<Response> {
    let kv_store = env.kv("KV")?;
    let form: serde_json::Value = req.json().await?;

    let action = match parse_action(&form) {
        Ok(a) => a,
        Err(e) => return Response::error(format!("destinations: {e}"), 400),
    };

    let mut rules = ensure_catch_all(&kv_store).await?;

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
            .unwrap_or("New rule")
            .to_string(),
    };

    let new_id = rule.id.clone();
    let insert_pos = rules.len().saturating_sub(1);
    rules.insert(insert_pos, rule);

    let enabled = EnabledChannels::from_env(env);
    let report = validation::validate(&rules, &enabled);
    if report.has_errors() {
        return validation_error_response(&report);
    }

    kv::save_rules(&kv_store, &rules).await?;
    Response::from_html(render_workbench(env, &rules, &enabled, Some(&new_id)).await)
}

/// GET /manage/rules/{id}/edit — return edit form partial
pub async fn edit_form(env: &Env, rule_id: &str) -> Result<Response> {
    let kv_store = env.kv("KV")?;
    let rules = kv::get_rules(&kv_store).await?;
    let enabled = EnabledChannels::from_env(env);

    match rules.iter().find(|r| r.id == rule_id) {
        Some(rule) => Response::from_html(templates::edit_rule_form(rule, &enabled)),
        None => Response::error("Rule not found", 404),
    }
}

/// PUT /manage/rules/{id} — update a rule
pub async fn update_rule(mut req: Request, env: &Env, rule_id: &str) -> Result<Response> {
    let kv_store = env.kv("KV")?;
    let form: serde_json::Value = req.json().await?;

    let action = match parse_action(&form) {
        Ok(a) => a,
        Err(e) => return Response::error(format!("destinations: {e}"), 400),
    };

    let mut rules = kv::get_rules(&kv_store).await?;

    if let Some(existing) = rules.iter_mut().find(|r| r.id == rule_id) {
        existing.label = form
            .get("label")
            .and_then(|v| v.as_str())
            .unwrap_or(&existing.label)
            .to_string();
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
    let report = validation::validate(&rules, &enabled);
    if report.has_errors() {
        return validation_error_response(&report);
    }

    kv::save_rules(&kv_store, &rules).await?;
    Response::from_html(render_workbench(env, &rules, &enabled, Some(rule_id)).await)
}

/// DELETE /manage/rules/{id} — delete a rule (blocked for catch-all)
pub async fn delete_rule(mut req: Request, env: &Env, rule_id: &str) -> Result<Response> {
    let kv_store = env.kv("KV")?;
    let mut rules = kv::get_rules(&kv_store).await?;

    if let Some(rule) = rules.iter().find(|r| r.id == rule_id) {
        if rule.is_catch_all() {
            return Response::error("Cannot delete the catch-all rule", 400);
        }
    }

    // Best-effort selection preservation: if the deleted rule was selected,
    // fall back to whatever the form said (or default in template).
    let form: serde_json::Value = req.json().await.unwrap_or(serde_json::Value::Null);
    let selected = selected_from_form(&form)
        .filter(|s| *s != rule_id)
        .map(str::to_string);

    rules.retain(|r| r.id != rule_id);
    kv::save_rules(&kv_store, &rules).await?;
    let enabled = EnabledChannels::from_env(env);
    Response::from_html(render_workbench(env, &rules, &enabled, selected.as_deref()).await)
}

/// POST /manage/rules/reorder — move a rule up or down
pub async fn reorder_rules(mut req: Request, env: &Env) -> Result<Response> {
    let kv_store = env.kv("KV")?;
    let form: serde_json::Value = req.json().await?;

    let rule_id = form.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let direction = form.get("direction").and_then(|v| v.as_str()).unwrap_or("");
    let selected = selected_from_form(&form).map(str::to_string);

    let mut rules = kv::get_rules(&kv_store).await?;

    let enabled = EnabledChannels::from_env(env);
    if let Some(pos) = rules.iter().position(|r| r.id == rule_id) {
        if rules[pos].is_catch_all() {
            return Response::from_html(
                render_workbench(env, &rules, &enabled, selected.as_deref()).await,
            );
        }

        let catch_all_pos = rules.len().saturating_sub(1);

        match direction {
            "up" if pos > 0 => {
                rules.swap(pos, pos - 1);
            }
            "down" if pos + 1 < catch_all_pos => {
                rules.swap(pos, pos + 1);
            }
            _ => {}
        }
    }

    kv::save_rules(&kv_store, &rules).await?;
    Response::from_html(render_workbench(env, &rules, &enabled, selected.as_deref()).await)
}

/// GET /manage/test — rule tester page.
pub async fn tester_page(env: &Env, email: &str) -> Result<Response> {
    let kv_store = env.kv("KV")?;
    let rules = ensure_catch_all(&kv_store).await?;
    let enabled = EnabledChannels::from_env(env);
    Response::from_html(templates::tester_page(&rules, email, None, &enabled))
}

/// POST /manage/test — evaluate rules against the supplied `to` address.
pub async fn tester_run(mut req: Request, env: &Env, email: &str) -> Result<Response> {
    let form: serde_json::Value = req.json().await?;
    let to = form
        .get("to")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_lowercase();
    let kv_store = env.kv("KV")?;
    let rules = ensure_catch_all(&kv_store).await?;
    let enabled = EnabledChannels::from_env(env);

    let result = if let Some((local, domain)) = to.rsplit_once('@') {
        let matched = routing::find_matching_rule(&rules, local, domain);
        Some(templates::TesterResult {
            to: to.clone(),
            matched_index: matched.and_then(|m| rules.iter().position(|r| r.id == m.id)),
        })
    } else {
        Some(templates::TesterResult {
            to: to.clone(),
            matched_index: None,
        })
    };
    Response::from_html(templates::tester_page(&rules, email, result, &enabled))
}

/// GET /manage/events?since={unix_ms} — JSON tail of the event ring buffer.
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

/// GET /manage/assets/cutout-mark.svg — favicon / brand mark.
pub async fn brand_mark() -> Result<Response> {
    let mut resp = Response::ok(templates::LOGO_SVG_FILE)?;
    let headers = resp.headers_mut();
    headers.set("Content-Type", "image/svg+xml")?;
    headers.set("Cache-Control", "public, max-age=86400")?;
    Ok(resp)
}
