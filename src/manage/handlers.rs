//! CRUD handlers for routing rules.

use worker::*;

use super::templates;
use crate::helpers::generate_id;
use crate::kv;
use crate::types::*;

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

/// Parse an Action from form JSON.
fn parse_action(form: &serde_json::Value) -> Action {
    let action_type = form
        .get("action_type")
        .and_then(|v| v.as_str())
        .unwrap_or("drop");

    match action_type {
        "forward" => {
            let destinations: Vec<String> = form
                .get("destinations")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            Action::Forward { destinations }
        }
        _ => Action::Drop,
    }
}

/// GET /manage — list rules
pub async fn list_rules(env: &Env, email: &str) -> Result<Response> {
    let kv_store = env.kv("KV")?;
    let rules = ensure_catch_all(&kv_store).await?;
    Response::from_html(templates::rules_page(&rules, email))
}

/// POST /manage/rules — create a new rule (inserted before catch-all)
pub async fn create_rule(mut req: Request, env: &Env) -> Result<Response> {
    let kv_store = env.kv("KV")?;
    let form: serde_json::Value = req.json().await?;

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
        action: parse_action(&form),
        label: form
            .get("label")
            .and_then(|v| v.as_str())
            .unwrap_or("New rule")
            .to_string(),
    };

    // Insert before the catch-all (last element)
    let insert_pos = if !rules.is_empty() {
        rules.len() - 1
    } else {
        0
    };
    rules.insert(insert_pos, rule);

    kv::save_rules(&kv_store, &rules).await?;
    Response::from_html(templates::rules_list_partial(&rules))
}

/// GET /manage/rules/{id}/edit — return edit form partial
pub async fn edit_form(env: &Env, rule_id: &str) -> Result<Response> {
    let kv_store = env.kv("KV")?;
    let rules = kv::get_rules(&kv_store).await?;

    match rules.iter().find(|r| r.id == rule_id) {
        Some(rule) => Response::from_html(templates::edit_rule_form(rule)),
        None => Response::error("Rule not found", 404),
    }
}

/// PUT /manage/rules/{id} — update a rule
pub async fn update_rule(mut req: Request, env: &Env, rule_id: &str) -> Result<Response> {
    let kv_store = env.kv("KV")?;
    let form: serde_json::Value = req.json().await?;

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
        existing.action = parse_action(&form);
    }

    kv::save_rules(&kv_store, &rules).await?;
    Response::from_html(templates::rules_list_partial(&rules))
}

/// DELETE /manage/rules/{id} — delete a rule (blocked for catch-all)
pub async fn delete_rule(env: &Env, rule_id: &str) -> Result<Response> {
    let kv_store = env.kv("KV")?;
    let mut rules = kv::get_rules(&kv_store).await?;

    // Block deletion of the catch-all
    if let Some(rule) = rules.iter().find(|r| r.id == rule_id) {
        if rule.is_catch_all() {
            return Response::error("Cannot delete the catch-all rule", 400);
        }
    }

    rules.retain(|r| r.id != rule_id);
    kv::save_rules(&kv_store, &rules).await?;
    Response::from_html(templates::rules_list_partial(&rules))
}

/// POST /manage/rules/reorder — move a rule up or down
pub async fn reorder_rules(mut req: Request, env: &Env) -> Result<Response> {
    let kv_store = env.kv("KV")?;
    let form: serde_json::Value = req.json().await?;

    let rule_id = form.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let direction = form.get("direction").and_then(|v| v.as_str()).unwrap_or("");

    let mut rules = kv::get_rules(&kv_store).await?;

    if let Some(pos) = rules.iter().position(|r| r.id == rule_id) {
        // Don't move the catch-all
        if rules[pos].is_catch_all() {
            return Response::from_html(templates::rules_list_partial(&rules));
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
    Response::from_html(templates::rules_list_partial(&rules))
}
