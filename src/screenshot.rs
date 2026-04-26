//! Cloudflare Browser Rendering client. The bot dispatcher hands sanitized,
//! image-inlined email HTML to [`render_email_png`]; the resulting PNG is
//! attached to the chat post (Telegram `sendPhoto`, Discord
//! `create_message_with_attachments`).
//!
//! Required environment:
//! - `CF_ACCOUNT_ID` (var) — the Cloudflare account id that owns the
//!   Browser Rendering binding.
//! - `CF_API_TOKEN` (secret) — API token with `Browser Rendering: Edit`
//!   permission (and `Account Analytics: Read` if you also want the
//!   dashboard stats panels — the same token covers both).
//!
//! When either is missing, [`render_email_png`] returns an error and the
//! caller falls back to the text + link path.

use worker::*;

const VIEWPORT_WIDTH: u32 = 800;
const VIEWPORT_HEIGHT: u32 = 1200;

/// Render `html` to a PNG screenshot via Cloudflare Browser Rendering and
/// return the raw image bytes.
pub async fn render_email_png(env: &Env, html: &str) -> Result<Vec<u8>> {
    let token = env
        .secret("CF_API_TOKEN")
        .map_err(|_| Error::from("CF_API_TOKEN secret not set"))?
        .to_string();
    let account_id = env
        .var("CF_ACCOUNT_ID")
        .map_err(|_| Error::from("CF_ACCOUNT_ID var not set"))?
        .to_string();
    if account_id.is_empty() {
        return Err(Error::from("CF_ACCOUNT_ID is empty"));
    }

    let url = format!(
        "https://api.cloudflare.com/client/v4/accounts/{account_id}/browser-rendering/screenshot"
    );
    let body = serde_json::json!({
        "html": html,
        "viewport": { "width": VIEWPORT_WIDTH, "height": VIEWPORT_HEIGHT },
        "screenshotOptions": { "type": "png", "fullPage": true }
    })
    .to_string();

    let headers = Headers::new();
    headers.set("Authorization", &format!("Bearer {token}"))?;
    headers.set("Content-Type", "application/json")?;

    let req = Request::new_with_init(
        &url,
        RequestInit::new()
            .with_method(Method::Post)
            .with_headers(headers)
            .with_body(Some(body.into())),
    )?;

    let mut resp = Fetch::Request(req).send().await?;
    let status = resp.status_code();
    if status >= 400 {
        let text = resp.text().await.unwrap_or_default();
        return Err(Error::from(format!(
            "browser rendering {status}: {}",
            text.chars().take(400).collect::<String>()
        )));
    }
    let bytes = resp.bytes().await?;
    if bytes.is_empty() {
        return Err(Error::from("browser rendering: empty response body"));
    }
    Ok(bytes)
}
