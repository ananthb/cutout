use serde::{Deserialize, Serialize};
use worker::d1::D1Database;
use worker::Result;

use crate::types::Rule;

/// Versioned envelope for the rules list, used to give the manage UI
/// optimistic-concurrency semantics (every editor session reads the
/// version, sends it back on save, and gets a 409 if another operator
/// has bumped it).
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

#[derive(Deserialize)]
struct RulesStateRow {
    version: u64,
    rules_json: String,
}

#[derive(Deserialize)]
struct VersionRow {
    #[allow(dead_code)]
    version: u64,
}

/// Load the rule set + version from D1.
pub async fn get_rule_set(db: &D1Database) -> Result<RuleSet> {
    let stmt = db.prepare("SELECT version, rules_json FROM rules_state WHERE id = 1");
    let row = stmt.first::<RulesStateRow>(None).await?;
    match row {
        None => Ok(RuleSet::empty()),
        Some(row) => {
            let rules: Vec<Rule> = serde_json::from_str(&row.rules_json)
                .map_err(|e| worker::Error::from(format!("decode rules_json: {e}")))?;
            Ok(RuleSet {
                version: row.version,
                rules,
            })
        }
    }
}

/// Load just the ordered rule list. Used by the email hot path which
/// doesn't care about the version.
pub async fn get_rules(db: &D1Database) -> Result<Vec<Rule>> {
    Ok(get_rule_set(db).await?.rules)
}

/// Atomic compare-and-swap save in D1: only writes if the stored version
/// matches `expected_version`. Returns [`SaveOutcome::Conflict`] with
/// the current version when it doesn't, so the caller can surface a
/// "another operator just saved" message.
pub async fn save_rule_set_if_unchanged(
    db: &D1Database,
    rules: &[Rule],
    expected_version: u64,
) -> Result<SaveOutcome> {
    let rules_json = serde_json::to_string(rules)
        .map_err(|e| worker::Error::from(format!("JSON serialize: {e}")))?;
    let next_version = expected_version + 1;

    let updated = if expected_version == 0 {
        let stmt = db.prepare(
            "INSERT INTO rules_state (id, version, rules_json, updated_at)
             VALUES (1, 1, ?1, CURRENT_TIMESTAMP)
             ON CONFLICT(id) DO UPDATE SET
                 version = 1,
                 rules_json = excluded.rules_json,
                 updated_at = CURRENT_TIMESTAMP
             WHERE rules_state.version = 0
             RETURNING version",
        );
        stmt.bind(&[rules_json.into()])?
            .first::<VersionRow>(None)
            .await?
    } else {
        let stmt = db.prepare(
            "UPDATE rules_state
             SET version = ?1, rules_json = ?2, updated_at = CURRENT_TIMESTAMP
             WHERE id = 1 AND version = ?3
             RETURNING version",
        );
        stmt.bind(&[
            (next_version as f64).into(),
            rules_json.into(),
            (expected_version as f64).into(),
        ])?
        .first::<VersionRow>(None)
        .await?
    };

    if updated.is_some() {
        Ok(SaveOutcome::Saved {
            new_version: next_version,
        })
    } else {
        let current = get_rule_set(db).await?;
        Ok(SaveOutcome::Conflict {
            current_version: current.version,
        })
    }
}

/// Force-save the rule set without checking the current version. Use
/// only for bootstrap operations (e.g. first-time catch-all insert) or
/// when the caller has explicitly decided to override the conflict.
pub async fn save_rule_set_force(db: &D1Database, rules: &[Rule], version: u64) -> Result<()> {
    let rules_json = serde_json::to_string(rules)
        .map_err(|e| worker::Error::from(format!("JSON serialize: {e}")))?;
    let stmt = db.prepare(
        "INSERT INTO rules_state (id, version, rules_json, updated_at)
         VALUES (1, ?1, ?2, CURRENT_TIMESTAMP)
         ON CONFLICT(id) DO UPDATE SET
             version = excluded.version,
             rules_json = excluded.rules_json,
             updated_at = CURRENT_TIMESTAMP",
    );
    stmt.bind(&[(version as f64).into(), rules_json.into()])?
        .run()
        .await?;
    Ok(())
}
