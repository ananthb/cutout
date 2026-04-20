use worker::kv::KvStore;
use worker::Result;

use crate::types::{ReverseAlias, Rule};

const RULES_KEY: &str = "rules";
const REVERSE_PREFIX: &str = "reverse:";
/// Reverse alias TTL: 30 days in seconds.
const REVERSE_TTL: u64 = 30 * 24 * 60 * 60;

/// Load the ordered rule list from KV. Returns an empty vec if not set.
pub async fn get_rules(kv: &KvStore) -> Result<Vec<Rule>> {
    match kv.get(RULES_KEY).json::<Vec<Rule>>().await? {
        Some(rules) => Ok(rules),
        None => Ok(Vec::new()),
    }
}

/// Save the ordered rule list to KV.
pub async fn save_rules(kv: &KvStore, rules: &[Rule]) -> Result<()> {
    let json = serde_json::to_string(rules)
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
