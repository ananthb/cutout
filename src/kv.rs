use worker::kv::KvStore;
use worker::Result;

use crate::types::{ReverseAlias, Rule};

const RULES_KEY: &str = "rules";
const REVERSE_PREFIX: &str = "reverse:";
const VERIFIED_PREFIX: &str = "verified:";
const PENDING_PREFIX: &str = "pending:";
/// Reverse alias TTL: 30 days in seconds.
const REVERSE_TTL: u64 = 30 * 24 * 60 * 60;
/// Pending verification token TTL: 24 hours in seconds.
const PENDING_TTL: u64 = 24 * 60 * 60;

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

/// Check whether an email address has been verified as a forwarding destination.
pub async fn is_verified(kv: &KvStore, email: &str) -> Result<bool> {
    let key = format!("{VERIFIED_PREFIX}{}", email.to_lowercase());
    Ok(kv.get(&key).text().await?.is_some())
}

/// Mark an email address as verified. No TTL — verification is permanent until removed.
pub async fn mark_verified(kv: &KvStore, email: &str) -> Result<()> {
    let key = format!("{VERIFIED_PREFIX}{}", email.to_lowercase());
    kv.put(&key, "1")?.execute().await?;
    Ok(())
}

/// Create a pending verification token for `email`. Returns the token string.
/// The token expires after 24 hours.
pub async fn create_pending(kv: &KvStore, email: &str) -> Result<String> {
    let token = uuid::Uuid::new_v4().simple().to_string();
    let key = format!("{PENDING_PREFIX}{token}");
    kv.put(&key, email.to_lowercase())?
        .expiration_ttl(PENDING_TTL)
        .execute()
        .await?;
    Ok(token)
}

/// Consume a pending verification token. Returns the associated email if the token
/// exists (and deletes the entry), otherwise None.
pub async fn consume_pending(kv: &KvStore, token: &str) -> Result<Option<String>> {
    let key = format!("{PENDING_PREFIX}{token}");
    let email = kv.get(&key).text().await?;
    if email.is_some() {
        kv.delete(&key).await?;
    }
    Ok(email)
}
