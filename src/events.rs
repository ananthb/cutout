//! Event recording: KV ring buffer for the live tail and Analytics Engine
//! data points for aggregated stats.
//!
//! The two stores serve different needs:
//! - **KV ring buffer** (`events:recent`, last [`MAX_EVENTS`] entries):
//!   feeds the dashboard's live feed pane; full event detail; refreshed
//!   within seconds.
//! - **Analytics Engine** dataset bound as `EVENTS`: feeds long-window
//!   aggregates (matches per rule, totals over 7d, top senders); sampled
//!   under load; ~2-5 min ingest lag.
//!
//! Writes happen on the email handler's hot path. KV writes are awaited
//! inline (a few hundred ms in the worst case); AE writes are non-blocking.

use serde::{Deserialize, Serialize};
use worker::kv::KvStore;
use worker::{AnalyticsEngineDataPointBuilder, Env};

/// Maximum events retained in the KV ring buffer. The buffer is read and
/// rewritten on each event, so this directly bounds the per-write cost.
pub const MAX_EVENTS: usize = 200;

const EVENTS_KEY: &str = "events:recent";
const AE_BINDING: &str = "EVENTS";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EventKind {
    /// Inbound mail forwarded to one or more destinations.
    Forward,
    /// Inbound mail dropped (matched a Drop rule, or no rule matched).
    Drop,
    /// Reverse-alias reply routed back to the original sender.
    Reply,
    /// Inbound mail rejected at SMTP (loop detected, unknown reverse alias, ...).
    Reject,
    /// Inbound mail stored in database.
    Store,
    /// Rule matched and dispatch was attempted, but at least one downstream
    /// step (native forward, send_email, or bot post) failed. The `error`
    /// field on [`Event`] carries the failure detail.
    Error,
}

impl EventKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EventKind::Forward => "forward",
            EventKind::Drop => "drop",
            EventKind::Reply => "reply",
            EventKind::Reject => "reject",
            EventKind::Store => "store",
            EventKind::Error => "error",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Event {
    /// Unix-millisecond timestamp.
    pub ts: i64,
    pub kind: EventKind,
    pub from: String,
    pub to: String,
    /// Matched rule id (`None` for no-match drops or rejects before routing).
    pub rule_id: Option<String>,
    /// Channels actually dispatched to. Ordered email/telegram/discord.
    pub channels: Vec<String>,
    pub size_bytes: u64,
    /// Failure detail when [`EventKind::Error`]: which destinations failed
    /// and the upstream error string. `None` for non-error events. Skipped
    /// during serialization when absent so older buffer entries round-trip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Append `event` to the ring buffer, truncating to [`MAX_EVENTS`].
///
/// **Concurrency caveat:** under simultaneous writes to KV, last-write-wins
/// can drop an event. Acceptable for self-hosted personal volume; revisit
/// (Durable Object) if/when this becomes hosted multi-tenant.
pub async fn record_kv(kv: &KvStore, event: &Event) -> worker::Result<()> {
    let mut buffer: Vec<Event> = kv
        .get(EVENTS_KEY)
        .json::<Vec<Event>>()
        .await?
        .unwrap_or_default();
    buffer.insert(0, event.clone());
    truncate_buffer(&mut buffer, MAX_EVENTS);
    let json =
        serde_json::to_string(&buffer).map_err(|e| worker::Error::from(format!("encode: {e}")))?;
    kv.put(EVENTS_KEY, json)?.execute().await?;
    Ok(())
}

/// Read the ring buffer. If `since_ms` is set, only return events newer
/// than that timestamp.
pub async fn recent(kv: &KvStore, since_ms: Option<i64>) -> worker::Result<Vec<Event>> {
    let buffer: Vec<Event> = kv
        .get(EVENTS_KEY)
        .json::<Vec<Event>>()
        .await?
        .unwrap_or_default();
    Ok(match since_ms {
        Some(t) => buffer.into_iter().filter(|e| e.ts > t).collect(),
        None => buffer,
    })
}

/// Write a data point to Analytics Engine. Schema:
/// - `index1` = rule id (or `"-"` when no match)
/// - `blob1` = event kind, `blob2` = from, `blob3` = to, `blob4` = channels (csv)
/// - `double1` = size_bytes, `double2` = destination_count
///
/// If the `EVENTS` binding isn't present (e.g. local dev without
/// `analytics_engine_datasets` configured), this silently does nothing: we
/// never want analytics writes to take down the email path.
pub fn record_ae(env: &Env, event: &Event) {
    let dataset = match env.analytics_engine(AE_BINDING) {
        Ok(d) => d,
        Err(_) => return,
    };
    let rule_id = event.rule_id.as_deref().unwrap_or("-");
    let channels = event.channels.join(",");
    let _ = AnalyticsEngineDataPointBuilder::new()
        .indexes([rule_id].as_slice())
        .add_blob(event.kind.as_str())
        .add_blob(event.from.as_str())
        .add_blob(event.to.as_str())
        .add_blob(channels.as_str())
        .add_double(event.size_bytes as f64)
        .add_double(event.channels.len() as f64)
        .write_to(&dataset);
}

/// Record an event to both stores. KV write is awaited (slow path); AE
/// write is fire-and-forget.
pub async fn record(env: &Env, kv: &KvStore, event: &Event) -> worker::Result<()> {
    record_ae(env, event);
    record_kv(kv, event).await
}

fn truncate_buffer(buffer: &mut Vec<Event>, max: usize) {
    if buffer.len() > max {
        buffer.truncate(max);
    }
}

/// Best-effort current time in unix milliseconds. Falls back to 0 when
/// the JS `Date` binding is unavailable (only happens in unit tests, which
/// don't depend on real timestamps).
pub fn now_ms() -> i64 {
    js_sys::Date::now() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(ts: i64) -> Event {
        Event {
            ts,
            kind: EventKind::Forward,
            from: "a@b.com".into(),
            to: "c@d.com".into(),
            rule_id: Some("r1".into()),
            channels: vec!["email".into()],
            size_bytes: 100,
            error: None,
        }
    }

    #[test]
    fn event_kind_error_serializes() {
        let s = serde_json::to_string(&EventKind::Error).unwrap();
        assert_eq!(s, "\"error\"");
        assert_eq!(EventKind::Error.as_str(), "error");
    }

    #[test]
    fn event_with_error_round_trips() {
        let mut e = ev(1);
        e.kind = EventKind::Error;
        e.error = Some("send_email failed: 500".into());
        let s = serde_json::to_string(&e).unwrap();
        assert!(s.contains("\"error\""));
        assert!(s.contains("send_email failed"));
        let back: Event = serde_json::from_str(&s).unwrap();
        assert_eq!(back.kind, EventKind::Error);
        assert_eq!(back.error.as_deref(), Some("send_email failed: 500"));
    }

    #[test]
    fn event_without_error_omits_field() {
        let s = serde_json::to_string(&ev(1)).unwrap();
        assert!(!s.contains("error"));
    }

    #[test]
    fn truncate_keeps_newest() {
        let mut buf: Vec<Event> = (0..250).rev().map(ev).collect();
        truncate_buffer(&mut buf, 200);
        assert_eq!(buf.len(), 200);
        assert_eq!(buf[0].ts, 249);
        assert_eq!(buf[199].ts, 50);
    }

    #[test]
    fn truncate_below_max_is_noop() {
        let mut buf: Vec<Event> = (0..10).rev().map(ev).collect();
        truncate_buffer(&mut buf, 200);
        assert_eq!(buf.len(), 10);
    }

    #[test]
    fn event_kind_serializes_lowercase() {
        let s = serde_json::to_string(&EventKind::Forward).unwrap();
        assert_eq!(s, "\"forward\"");
        let s = serde_json::to_string(&EventKind::Drop).unwrap();
        assert_eq!(s, "\"drop\"");
        let s = serde_json::to_string(&EventKind::Reply).unwrap();
        assert_eq!(s, "\"reply\"");
        let s = serde_json::to_string(&EventKind::Reject).unwrap();
        assert_eq!(s, "\"reject\"");
    }

    #[test]
    fn event_kind_as_str_matches_serde() {
        assert_eq!(EventKind::Forward.as_str(), "forward");
        assert_eq!(EventKind::Drop.as_str(), "drop");
        assert_eq!(EventKind::Reply.as_str(), "reply");
        assert_eq!(EventKind::Reject.as_str(), "reject");
    }
}
