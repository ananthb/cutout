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
/// Cloudflare Email Service expects structured fields, not raw RFC 2822 bytes.
pub struct OutboundEmail {
    pub from: String,
    pub to: String,
    pub subject: String,
    pub text: Option<String>,
    pub html: Option<String>,
    pub reply_to: Option<String>,
    /// Extra headers to set on the outbound message (e.g. In-Reply-To, References,
    /// X-Cutout-Forwarded). Iterated in order so duplicate names are preserved.
    pub headers: Vec<(String, String)>,
}

/// Instruction to Cloudflare to forward the inbound message via the native
/// `EmailMessage.forward()` call. This preserves the original From/To/DKIM
/// and everything else — exactly what CF's built-in email forwarder does.
/// The destination must be verified in the zone's Email Routing
/// "Destination Addresses" list.
pub struct ForwardInstruction {
    pub destination: String,
    /// Reply-To to overlay so recipient replies route back through the proxy.
    pub reply_to: String,
}

/// Result of email processing — drives action in the wasm_bindgen email() export.
pub enum EmailResult {
    /// Silently drop the email.
    Drop,
    /// Reject the email with an SMTP error message.
    Reject(String),
    /// Hand the inbound message to Cloudflare's native forwarder.
    Forward(ForwardInstruction),
    /// Send one or more new emails via the send_email binding (used by the
    /// reverse-alias reply path, where there's no inbound bytes to preserve).
    Send(Vec<OutboundEmail>),
}
