//! Outbound webhook destination: POST the parsed email as JSON.

use worker::*;

use crate::types::WebhookForward;

/// Longest error snippet taken from a failed endpoint's response body. Enough
/// to identify the problem, short enough to fit in `pending_dispatches.last_error`.
const BODY_SNIPPET: usize = 200;

/// POST `forward` to its URL as JSON. Any non-2xx response is an error, so the
/// caller can queue the same [`WebhookForward`] for the retry consumer.
pub async fn dispatch(_env: &Env, forward: &WebhookForward) -> Result<()> {
    let payload = serde_json::json!({
        "from": forward.from,
        "to": forward.to,
        "subject": forward.subject,
        "text": forward.text,
        "html": forward.html,
        "message_id": forward.message_id,
        "rule_id": forward.rule_id,
        "received_at": forward.received_at,
    })
    .to_string();

    let headers = Headers::new();
    headers.set("Content-Type", "application/json")?;

    let req = Request::new_with_init(
        &forward.url,
        RequestInit::new()
            .with_method(Method::Post)
            .with_headers(headers)
            .with_body(Some(payload.into())),
    )?;

    let mut resp = Fetch::Request(req).send().await?;
    let status = resp.status_code();
    if !(200..300).contains(&status) {
        let text = resp.text().await.unwrap_or_default();
        return Err(Error::from(format!(
            "webhook {} returned {status}: {}",
            forward.url,
            text.chars().take(BODY_SNIPPET).collect::<String>()
        )));
    }
    Ok(())
}
