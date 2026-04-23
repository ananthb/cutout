//! Management panel — routing rules UI gated by Cloudflare Access.

pub mod access;
pub mod handlers;
pub mod templates;

use worker::*;

/// Handle /manage/* routes. Requires Cloudflare Access.
pub async fn handle_manage(req: Request, env: Env, path: &str, method: Method) -> Result<Response> {
    let email = match access::verify_access(&req, &env).await {
        Some(e) => e,
        None => return Response::error("Forbidden: Cloudflare Access required", 403),
    };

    let sub = path
        .strip_prefix("/manage")
        .unwrap_or("")
        .trim_start_matches('/');

    let parts: Vec<&str> = sub.split('/').filter(|s| !s.is_empty()).collect();

    match (method, parts.as_slice()) {
        // GET /manage — list rules
        (Method::Get, []) => handlers::list_rules(&env, &email).await,

        // POST /manage/rules — create rule
        (Method::Post, ["rules"]) => handlers::create_rule(req, &env).await,

        // POST /manage/rules/reorder — move rule up/down
        (Method::Post, ["rules", "reorder"]) => handlers::reorder_rules(req, &env).await,

        // POST /manage/verify/resend — resend a verification email
        (Method::Post, ["verify", "resend"]) => handlers::resend_verification(req, &env).await,

        // GET /manage/rules/{id}/edit — edit form partial
        (Method::Get, ["rules", id, "edit"]) => handlers::edit_form(&env, id).await,

        // PUT /manage/rules/{id} — update rule
        (Method::Put, ["rules", id]) => handlers::update_rule(req, &env, id).await,

        // DELETE /manage/rules/{id} — delete rule
        (Method::Delete, ["rules", id]) => handlers::delete_rule(&env, id).await,

        _ => Response::error("Not Found", 404),
    }
}
