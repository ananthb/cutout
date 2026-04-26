//! Aggregated stats for the dashboard, sourced from Analytics Engine.
//!
//! The worker can't query its own AE dataset directly; querying happens via
//! Cloudflare's REST SQL API. We need an account id and an API token with
//! "Account Analytics: Read" scope. If either is missing the dashboard
//! degrades gracefully (no stats shown, no error).
//!
//! Results are cached in `caches.default` for 5 minutes, keyed by a synthetic
//! URL. AE has 2-5 min ingest lag anyway, so this cache never serves data
//! significantly staler than the underlying source.

use std::collections::HashMap;

use serde_json::Value;
use worker::{Cache, Env, Headers, Method, Request, RequestInit, Response};

const CACHE_KEY: &str = "https://stats.cutout.local/7d";
const CACHE_TTL_S: u32 = 300;

const SQL_AGGREGATES: &str = r#"SELECT
  index1 AS rule_id,
  blob1 AS event_type,
  SUM(_sample_interval) AS n,
  toUnixTimestamp(MAX(timestamp)) AS last_ts
FROM cutout_events
WHERE timestamp > NOW() - INTERVAL '7' DAY
GROUP BY index1, blob1
FORMAT JSON"#;

const SQL_TOP_SENDERS: &str = r#"SELECT
  blob2 AS sender,
  SUM(_sample_interval) AS n
FROM cutout_events
WHERE blob1 = 'forward'
  AND timestamp > NOW() - INTERVAL '7' DAY
GROUP BY blob2
ORDER BY n DESC
LIMIT 10
FORMAT JSON"#;

/// 7-day rollup feeding the workbench top bar, inspector, and top-senders card.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct Stats7d {
    pub forwarded_total: u64,
    pub dropped_total: u64,
    pub stored_total: u64,
    pub by_rule: HashMap<String, RuleStats>,
    pub top_senders: Vec<TopSender>,
    /// Unix-millisecond timestamp for when these numbers were computed.
    /// The dashboard uses this to render a "stats from N min ago" hint.
    pub generated_at: i64,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct RuleStats {
    pub matches: u64,
    /// Last forward+drop seen for this rule, unix seconds.
    pub last_match_s: Option<i64>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TopSender {
    pub address: String,
    pub n: u64,
}

/// Fetch the 7-day rollup. Returns `None` if AE credentials aren't
/// configured or the API call fails, so callers should render the
/// dashboard without stats in that case.
pub async fn fetch_7d(env: &Env) -> Option<Stats7d> {
    if let Some(cached) = read_cache().await {
        return Some(cached);
    }
    let stats = query_ae(env).await?;
    write_cache(&stats).await;
    Some(stats)
}

async fn read_cache() -> Option<Stats7d> {
    let cache = Cache::default();
    let mut resp = cache.get(CACHE_KEY, false).await.ok().flatten()?;
    let body: Stats7d = resp.json().await.ok()?;
    Some(body)
}

async fn write_cache(stats: &Stats7d) {
    let cache = Cache::default();
    let body = match serde_json::to_string(stats) {
        Ok(s) => s,
        Err(_) => return,
    };
    let resp = match Response::ok(body) {
        Ok(r) => r,
        Err(_) => return,
    };
    let h = Headers::new();
    let _ = h.set(
        "Cache-Control",
        &format!("public, max-age={CACHE_TTL_S}, s-maxage={CACHE_TTL_S}"),
    );
    let _ = h.set("Content-Type", "application/json");
    let resp = resp.with_headers(h);
    let _ = cache.put(CACHE_KEY, resp).await;
}

async fn query_ae(env: &Env) -> Option<Stats7d> {
    let account_id = env
        .var("CF_ACCOUNT_ID")
        .map(|v| v.to_string())
        .ok()
        .filter(|s| !s.is_empty())?;
    let api_token = env
        .secret("CF_API_TOKEN")
        .map(|v| v.to_string())
        .ok()
        .filter(|s| !s.is_empty())?;

    let agg = run_sql(&account_id, &api_token, SQL_AGGREGATES)
        .await
        .ok()?;
    let senders = run_sql(&account_id, &api_token, SQL_TOP_SENDERS)
        .await
        .ok()?;

    let mut stats = Stats7d {
        generated_at: crate::events::now_ms(),
        ..Default::default()
    };
    parse_aggregates(&agg, &mut stats);
    stats.top_senders = parse_top_senders(&senders);
    Some(stats)
}

async fn run_sql(account_id: &str, api_token: &str, sql: &str) -> worker::Result<Value> {
    let url =
        format!("https://api.cloudflare.com/client/v4/accounts/{account_id}/analytics_engine/sql");
    let headers = Headers::new();
    headers.set("Authorization", &format!("Bearer {api_token}"))?;
    headers.set("Content-Type", "text/plain")?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(sql.to_string().into()));

    let req = Request::new_with_init(&url, &init)?;
    let mut resp = worker::Fetch::Request(req).send().await?;
    if resp.status_code() < 200 || resp.status_code() >= 300 {
        return Err(worker::Error::from(format!(
            "AE SQL {} {}",
            resp.status_code(),
            resp.text().await.unwrap_or_default()
        )));
    }
    let json: Value = resp.json().await?;
    Ok(json)
}

/// AE / ClickHouse `FORMAT JSON` quotes UInt64 values as strings so JS
/// callers don't lose precision. We may also see plain numbers (UInt32)
/// or floats (sampled aggregates), so accept any of the three.
fn value_to_u64(v: &Value) -> Option<u64> {
    if let Some(n) = v.as_u64() {
        return Some(n);
    }
    if let Some(f) = v.as_f64() {
        return Some(f.max(0.0) as u64);
    }
    v.as_str().and_then(|s| s.parse().ok())
}

fn value_to_i64(v: &Value) -> Option<i64> {
    if let Some(n) = v.as_i64() {
        return Some(n);
    }
    if let Some(f) = v.as_f64() {
        return Some(f as i64);
    }
    v.as_str().and_then(|s| s.parse().ok())
}

fn parse_aggregates(json: &Value, stats: &mut Stats7d) {
    let rows = match json.get("data").and_then(|d| d.as_array()) {
        Some(r) => r,
        None => return,
    };
    for row in rows {
        let rule_id = row
            .get("rule_id")
            .and_then(|v| v.as_str())
            .unwrap_or("-")
            .to_string();
        let event_type = match row.get("event_type").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => continue,
        };
        let n = row.get("n").and_then(value_to_u64).unwrap_or(0);
        let last_ts = row.get("last_ts").and_then(value_to_i64).unwrap_or(0);
        match event_type {
            "forward" => stats.forwarded_total += n,
            "drop" => stats.dropped_total += n,
            "store" => stats.stored_total += n,
            _ => {}
        }
        // Per-rule rollup counts every event where the rule fired, regardless
        // of dispatch outcome: forward (all destinations OK), drop, store, and
        // error (rule matched but at least one destination failed). Reply and
        // reject have no rule_id so they're filtered by the `!= "-"` guard.
        if matches!(event_type, "forward" | "drop" | "store" | "error") && rule_id != "-" {
            let entry = stats.by_rule.entry(rule_id).or_default();
            entry.matches += n;
            entry.last_match_s = Some(match entry.last_match_s {
                Some(prev) => prev.max(last_ts),
                None => last_ts,
            });
        }
    }
}

fn parse_top_senders(json: &Value) -> Vec<TopSender> {
    let rows = match json.get("data").and_then(|d| d.as_array()) {
        Some(r) => r,
        None => return Vec::new(),
    };
    rows.iter()
        .filter_map(|row| {
            let sender = row.get("sender").and_then(|v| v.as_str())?.to_string();
            if sender.is_empty() {
                return None;
            }
            let n = row.get("n").and_then(value_to_u64).unwrap_or(0);
            Some(TopSender { address: sender, n })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregates_split_forward_and_drop() {
        let json: Value = serde_json::from_str(
            r#"{
                "data": [
                    {"rule_id":"r1","event_type":"forward","n":10,"last_ts":1700},
                    {"rule_id":"r1","event_type":"drop","n":5,"last_ts":1800},
                    {"rule_id":"r2","event_type":"forward","n":7,"last_ts":1900},
                    {"rule_id":"-","event_type":"drop","n":3,"last_ts":2000}
                ]
            }"#,
        )
        .unwrap();
        let mut s = Stats7d::default();
        parse_aggregates(&json, &mut s);
        assert_eq!(s.forwarded_total, 17);
        assert_eq!(s.dropped_total, 8);
        assert_eq!(s.by_rule.get("r1").unwrap().matches, 15);
        assert_eq!(s.by_rule.get("r1").unwrap().last_match_s, Some(1800));
        assert_eq!(s.by_rule.get("r2").unwrap().matches, 7);
        // No-match drops aren't attributed to any rule
        assert!(s.by_rule.get("-").is_none());
    }

    #[test]
    fn aggregates_count_partial_failures_in_per_rule_matches() {
        // A rule whose dispatch had at least one destination fail is recorded
        // as event_type=error. Those still need to show up in matches /
        // last_match for the rule (it did fire), but not in the global
        // forwarded/dropped/stored totals.
        let json: Value = serde_json::from_str(
            r#"{
                "data": [
                    {"rule_id":"r1","event_type":"forward","n":2,"last_ts":1700},
                    {"rule_id":"r1","event_type":"error","n":3,"last_ts":1900}
                ]
            }"#,
        )
        .unwrap();
        let mut s = Stats7d::default();
        parse_aggregates(&json, &mut s);
        assert_eq!(s.forwarded_total, 2);
        assert_eq!(s.dropped_total, 0);
        let r1 = s.by_rule.get("r1").unwrap();
        assert_eq!(r1.matches, 5);
        assert_eq!(r1.last_match_s, Some(1900));
    }

    #[test]
    fn top_senders_filters_empty() {
        let json: Value = serde_json::from_str(
            r#"{
                "data": [
                    {"sender":"alice@a.com","n":50},
                    {"sender":"","n":100},
                    {"sender":"bob@b.com","n":3}
                ]
            }"#,
        )
        .unwrap();
        let v = parse_top_senders(&json);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].address, "alice@a.com");
        assert_eq!(v[1].address, "bob@b.com");
    }

    #[test]
    fn aggregates_handles_uint64_strings() {
        // CF AE / ClickHouse FORMAT JSON serializes UInt64 as strings to
        // preserve precision. Make sure we still parse those.
        let json: Value = serde_json::from_str(
            r#"{
                "data": [
                    {"rule_id":"r1","event_type":"forward","n":"42","last_ts":"1700"},
                    {"rule_id":"r1","event_type":"drop","n":"3","last_ts":"1800"}
                ]
            }"#,
        )
        .unwrap();
        let mut s = Stats7d::default();
        parse_aggregates(&json, &mut s);
        assert_eq!(s.forwarded_total, 42);
        assert_eq!(s.dropped_total, 3);
        assert_eq!(s.by_rule.get("r1").unwrap().matches, 45);
        assert_eq!(s.by_rule.get("r1").unwrap().last_match_s, Some(1800));
    }

    #[test]
    fn top_senders_handles_uint64_strings() {
        let json: Value =
            serde_json::from_str(r#"{"data":[{"sender":"alice@a.com","n":"123456"}]}"#).unwrap();
        let v = parse_top_senders(&json);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].n, 123456);
    }

    #[test]
    fn aggregates_empty_data_array_is_noop() {
        let json: Value = serde_json::from_str(r#"{"data":[]}"#).unwrap();
        let mut s = Stats7d::default();
        parse_aggregates(&json, &mut s);
        assert_eq!(s.forwarded_total, 0);
        assert!(s.by_rule.is_empty());
    }
}
