use serde::{Deserialize, Serialize};
use serde_json::Value;
use worker::kv::KvStore;
use worker::Result;

use crate::types::{ReverseAlias, Rule};

const RULES_KEY: &str = "rules";
const REVERSE_PREFIX: &str = "reverse:";
/// Reverse alias TTL: 30 days in seconds.
const REVERSE_TTL: u64 = 30 * 24 * 60 * 60;

/// Versioned envelope for the rules list, used to give the manage UI
/// optimistic-concurrency semantics (every editor session reads the
/// version, sends it back on save, and gets a 409 if another operator
/// has bumped it). Backward-compatible: if the KV value is a bare
/// `Vec<Rule>` (pre-versioning shape) it parses as version 0 and the
/// next save migrates the value forward.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuleSet {
    pub version: u64,
    pub rules: Vec<Rule>,
}

impl RuleSet {
    pub fn empty() -> Self {
        Self {
            version: 0,
            rules: Vec::new(),
        }
    }
}

/// Outcome of a [`save_rule_set_if_unchanged`] attempt.
#[derive(Debug)]
pub enum SaveOutcome {
    /// The save committed; the new version (always `expected + 1`) is
    /// returned so the UI can keep editing without an extra round-trip.
    Saved { new_version: u64 },
    /// Another writer bumped the version since the editor loaded.
    /// The current version is returned so the UI can prompt a refresh.
    Conflict { current_version: u64 },
}

/// Load the rule set + version from KV. Accepts both the new envelope
/// shape and the legacy bare `Vec<Rule>` shape (version assumed 0).
pub async fn get_rule_set(kv: &KvStore) -> Result<RuleSet> {
    let raw = match kv.get(RULES_KEY).json::<Value>().await? {
        Some(v) => v,
        None => return Ok(RuleSet::empty()),
    };
    if raw.is_array() {
        let rules: Vec<Rule> = serde_json::from_value(raw)
            .map_err(|e| worker::Error::from(format!("decode legacy rules: {e}")))?;
        return Ok(RuleSet { version: 0, rules });
    }
    serde_json::from_value::<RuleSet>(raw)
        .map_err(|e| worker::Error::from(format!("decode rule set: {e}")))
}

/// Load just the ordered rule list. Used by the email hot path which
/// doesn't care about the version. Equivalent to `get_rule_set` but
/// drops the version on the floor.
pub async fn get_rules(kv: &KvStore) -> Result<Vec<Rule>> {
    Ok(get_rule_set(kv).await?.rules)
}

/// Compare-and-swap save: only writes if the current stored version
/// matches `expected_version`. Returns [`SaveOutcome::Conflict`] with
/// the current version when it doesn't, so the caller can surface a
/// "another operator just saved" message.
///
/// KV has no native CAS; the read-then-write window is small (a few
/// hundred ms in the worst case) but not zero. Two requests racing
/// inside that window can both pass the check and both write. For the
/// expected use (a small shared admin team) this collapses the race
/// window from "duration of an edit" to "duration of one request,"
/// which is the intended outcome.
pub async fn save_rule_set_if_unchanged(
    kv: &KvStore,
    rules: &[Rule],
    expected_version: u64,
) -> Result<SaveOutcome> {
    let current = get_rule_set(kv).await?;
    if current.version != expected_version {
        return Ok(SaveOutcome::Conflict {
            current_version: current.version,
        });
    }
    let next = RuleSet {
        version: expected_version + 1,
        rules: rules.to_vec(),
    };
    let json = serde_json::to_string(&next)
        .map_err(|e| worker::Error::from(format!("JSON serialize: {e}")))?;
    kv.put(RULES_KEY, json)?.execute().await?;
    Ok(SaveOutcome::Saved {
        new_version: next.version,
    })
}

/// Force-save the rule set without checking the current version. Use
/// only for bootstrap operations (e.g. first-time catch-all insert) or
/// when the caller has explicitly decided to override the conflict.
pub async fn save_rule_set_force(kv: &KvStore, rules: &[Rule], version: u64) -> Result<()> {
    let envelope = RuleSet {
        version,
        rules: rules.to_vec(),
    };
    let json = serde_json::to_string(&envelope)
        .map_err(|e| worker::Error::from(format!("JSON serialize: {e}")))?;
    kv.put(RULES_KEY, json)?.execute().await?;
    Ok(())
}

/// Look up a reverse alias by its full address (e.g. `reply+uuid@domain`).
pub async fn get_reverse_alias(kv: &KvStore, address: &str) -> Result<Option<ReverseAlias>> {
    let key = format!("{REVERSE_PREFIX}{address}");
    Ok(kv.get(&key).json::<ReverseAlias>().await?)
}

/// Save a reverse alias with a 30-day TTL.
pub async fn save_reverse_alias(kv: &KvStore, address: &str, alias: &ReverseAlias) -> Result<()> {
    let key = format!("{REVERSE_PREFIX}{address}");
    let json = serde_json::to_string(alias)
        .map_err(|e| worker::Error::from(format!("JSON serialize: {e}")))?;
    kv.put(&key, json)?
        .expiration_ttl(REVERSE_TTL)
        .execute()
        .await?;
    Ok(())
}
