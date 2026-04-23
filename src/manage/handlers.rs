//! CRUD handlers for routing rules.

use std::collections::HashSet;

use worker::*;

use super::templates;
use crate::helpers::generate_id;
use crate::kv;
use crate::types::*;
use crate::verify;

/// Collect all Forward destinations across rules and look up their verified status.
async fn verified_set(kv_store: &worker::kv::KvStore, rules: &[Rule]) -> HashSet<String> {
    let mut to_check: HashSet<String> = HashSet::new();
    for rule in rules {
        if let Action::Forward { destinations } = &rule.action {
            for d in destinations {
                to_check.insert(d.clone());
            }
        }
    }
    let mut verified = HashSet::new();
    for dest in to_check {
        if kv::is_verified(kv_store, &dest).await.unwrap_or(false) {
            verified.insert(dest);
        }
    }
    verified
}

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

/// Trigger verification for each destination that is not already verified.
/// Failures are logged and swallowed — the rule save is not rolled back.
async fn send_verification_emails(env: &Env, host: &str, destinations: &[String]) {
    if host.is_empty() {
        console_log!("no host available, skipping verification sends");
        return;
    }
    for dest in destinations {
        if let Err(e) = verify::send_verification(env, host, dest).await {
            console_log!("verification send failed for {dest}: {e}");
        }
    }
}

/// Parse an Action from form JSON. Email addresses are lowercased.
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
                .map(|s| s.trim().to_lowercase())
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
    let verified = verified_set(&kv_store, &rules).await;
    Response::from_html(templates::rules_page(&rules, email, &verified))
}

/// POST /manage/rules — create a new rule (inserted before catch-all)
pub async fn create_rule(mut req: Request, env: &Env) -> Result<Response> {
    let host = req.url()?.host_str().unwrap_or("").to_string();
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

    let verify_targets: Vec<String> = match &rule.action {
        Action::Forward { destinations } => destinations.clone(),
        _ => Vec::new(),
    };

    // Insert before the catch-all (last element)
    let insert_pos = if !rules.is_empty() {
        rules.len() - 1
    } else {
        0
    };
    rules.insert(insert_pos, rule);

    kv::save_rules(&kv_store, &rules).await?;

    send_verification_emails(env, &host, &verify_targets).await;

    let verified = verified_set(&kv_store, &rules).await;
    Response::from_html(templates::rules_list_partial(&rules, &verified))
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
    let host = req.url()?.host_str().unwrap_or("").to_string();
    let kv_store = env.kv("KV")?;
    let form: serde_json::Value = req.json().await?;

    let mut rules = kv::get_rules(&kv_store).await?;

    let mut verify_targets: Vec<String> = Vec::new();
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
        if let Action::Forward { destinations } = &existing.action {
            verify_targets = destinations.clone();
        }
    }

    kv::save_rules(&kv_store, &rules).await?;

    send_verification_emails(env, &host, &verify_targets).await;

    let verified = verified_set(&kv_store, &rules).await;
    Response::from_html(templates::rules_list_partial(&rules, &verified))
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
    let verified = verified_set(&kv_store, &rules).await;
    Response::from_html(templates::rules_list_partial(&rules, &verified))
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
            let verified = verified_set(&kv_store, &rules).await;
            return Response::from_html(templates::rules_list_partial(&rules, &verified));
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
    let verified = verified_set(&kv_store, &rules).await;
    Response::from_html(templates::rules_list_partial(&rules, &verified))
}

/// POST /manage/verify/resend — resend a verification email for a single address.
/// Returns a toast snippet (success or error) for swap into #toast.
pub async fn resend_verification(mut req: Request, env: &Env) -> Result<Response> {
    let host = req.url()?.host_str().unwrap_or("").to_string();
    let form: serde_json::Value = req.json().await?;
    let email = form
        .get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_lowercase();
    if email.is_empty() {
        return Response::from_html(templates::toast("error", "Email address required."));
    }
    if host.is_empty() {
        return Response::from_html(templates::toast(
            "error",
            "No host in request URL; cannot build verification link.",
        ));
    }

    let kv_store = env.kv("KV")?;
    if kv::is_verified(&kv_store, &email).await.unwrap_or(false) {
        return Response::from_html(templates::toast(
            "success",
            &format!("{email} is already verified."),
        ));
    }

    match verify::send_verification(env, &host, &email).await {
        Ok(()) => Response::from_html(templates::toast(
            "success",
            &format!("Verification email sent to {email}."),
        )),
        Err(e) => {
            console_log!("resend verification failed for {email}: {e}");
            Response::from_html(templates::toast(
                "error",
                &format!("Could not send verification to {email}: {e}"),
            ))
        }
    }
}
