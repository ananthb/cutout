//! Management surface: Cloudflare Access verification and the stored-email
//! viewer. The JSON API lives in `crate::api`; the SPA shell in `crate::shell`.

pub mod access;
pub mod viewer;

use worker::*;

/// Serve the SPA shell for a route the Elm app owns (`/manage`,
/// `/manage/pending`, `/inbox`). Not cached: the shell is cheap to regenerate
/// and caching it would pin a stale set of asset integrity hashes across a
/// deploy.
pub fn serve_app_shell() -> Result<Response> {
    let headers = Headers::new();
    headers.set("Content-Type", "text/html; charset=utf-8")?;
    headers.set("Cache-Control", "no-store")?;
    Ok(Response::ok(crate::shell::html())?.with_headers(headers))
}
