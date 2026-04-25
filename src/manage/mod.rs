//! Management panel — routing rules UI gated by Cloudflare Access.

pub mod access;
pub mod handlers;
pub mod templates;

use worker::*;

/// Handle /manage/* routes. Requires Cloudflare Access (except for
/// `/manage/assets/*`, which is just static branding).
pub async fn handle_manage(req: Request, env: Env, path: &str, method: Method) -> Result<Response> {
    let sub = path
        .strip_prefix("/manage")
        .unwrap_or("")
        .trim_start_matches('/');

    let parts: Vec<&str> = sub.split('/').filter(|s| !s.is_empty()).collect();

    // Public asset routes — no Access challenge so the favicon loads on the
    // login redirect page too.
    if let (Method::Get, ["assets", "cutout-mark.svg"]) = (&method, parts.as_slice()) {
        return handlers::brand_mark().await;
    }

    let email = match access::verify_access(&req, &env).await {
        Some(e) => e,
        None => return Response::error("Forbidden: Cloudflare Access required", 403),
    };

    match (method, parts.as_slice()) {
        // GET /manage — list rules
        (Method::Get, []) => handlers::list_rules(req, &env, &email).await,

        // GET /manage/rules/new — new-rule modal partial
        (Method::Get, ["rules", "new"]) => handlers::new_rule_form(&env).await,

        // POST /manage/rules — create rule
        (Method::Post, ["rules"]) => handlers::create_rule(req, &env).await,

        // POST /manage/rules/reorder — move rule up/down
        (Method::Post, ["rules", "reorder"]) => handlers::reorder_rules(req, &env).await,

        // GET /manage/rules/{id}/edit — edit form partial
        (Method::Get, ["rules", id, "edit"]) => handlers::edit_form(&env, id).await,

        // PUT /manage/rules/{id} — update rule
        (Method::Put, ["rules", id]) => handlers::update_rule(req, &env, id).await,

        // DELETE /manage/rules/{id} — delete rule
        (Method::Delete, ["rules", id]) => handlers::delete_rule(req, &env, id).await,

        // GET /manage/test — rule tester page
        (Method::Get, ["test"]) => handlers::tester_page(&env, &email).await,
        // POST /manage/test — run the tester
        (Method::Post, ["test"]) => handlers::tester_run(req, &env, &email).await,

        // GET /manage/events — JSON tail for the live feed
        (Method::Get, ["events"]) => handlers::list_events(req, &env).await,

        _ => Response::error("Not Found", 404),
    }
}
