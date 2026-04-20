use serde::{Deserialize, Serialize};

/// A routing rule. Rules are stored as an ordered `Vec<Rule>` in KV.
/// Evaluated top-to-bottom; first match wins.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Rule {
    pub id: String,
    /// Glob pattern for the local part (before @). `*` matches everything.
    pub local_pattern: String,
    /// Glob pattern for the domain part (after @). `*` matches everything.
    pub domain_pattern: String,
    pub action: Action,
    /// Human-readable label, e.g. "Newsletter drop" or "Catch-all forward".
    pub label: String,
}

impl Rule {
    /// Returns true if this is the catch-all rule (`*@*`).
    pub fn is_catch_all(&self) -> bool {
        self.local_pattern == "*" && self.domain_pattern == "*"
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Forward {
        /// One or more destination email addresses.
        destinations: Vec<String>,
    },
    Drop,
}

/// Stored in KV under `reverse:{reply+uuid@domain}` with 30-day TTL.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ReverseAlias {
    /// The alias address the original email was sent to.
    pub alias: String,
    /// The external sender's address.
    pub original_sender: String,
}

/// A single outbound email to send via the EMAIL binding.
pub struct OutboundEmail {
    pub from: String,
    pub to: String,
    pub raw: Vec<u8>,
}

/// Result of email processing — drives action in the wasm_bindgen email() export.
pub enum EmailResult {
    /// Silently drop the email.
    Drop,
    /// Reject the email with an SMTP error message.
    Reject(String),
    /// Send one or more emails via the send_email binding.
    Send(Vec<OutboundEmail>),
}
