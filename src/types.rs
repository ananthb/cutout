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
    /// Forward inbound mail to one or more destinations (mixed-channel).
    Forward {
        destinations: Vec<Destination>,
        #[serde(default)]
        replace_reply_to: bool,
    },
    /// Silently drop.
    Drop,
}

/// A single forward target. Multiple destinations of different kinds may be
/// attached to one rule.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Destination {
    /// Forward to an email address via Cloudflare's `EmailMessage.forward()`.
    /// Recipient must be in the zone's Email Routing Destination Addresses.
    Email { address: String },
    /// Forward to a Telegram chat via bot `sendMessage`.
    Telegram { chat_id: String },
    /// Forward to a Discord channel via bot `createMessage`.
    Discord { channel_id: String },
}

impl Destination {
    /// Short kind label for logs / UI.
    pub fn kind_label(&self) -> &'static str {
        match self {
            Destination::Email { .. } => "email",
            Destination::Telegram { .. } => "telegram",
            Destination::Discord { .. } => "discord",
        }
    }

    /// The address/id value as a display string.
    pub fn value(&self) -> &str {
        match self {
            Destination::Email { address } => address,
            Destination::Telegram { chat_id } => chat_id,
            Destination::Discord { channel_id } => channel_id,
        }
    }

    /// Parse a single line of the form `kind:value` (e.g. `email:a@b.com`,
    /// `telegram:-100123`, `discord:987654321`). Returns `None` for blank
    /// lines. Returns an error for any non-blank malformed line.
    pub fn parse_line(line: &str) -> Result<Option<Destination>, &'static str> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        let (kind, value) = trimmed
            .split_once(':')
            .ok_or("expected 'kind:value' (e.g. email:you@example.com)")?;
        let value = value.trim().to_string();
        if value.is_empty() {
            return Err("value missing after ':'");
        }
        match kind.trim().to_lowercase().as_str() {
            "email" => {
                if !value.contains('@') || value.starts_with('@') || value.ends_with('@') {
                    return Err("email address must contain '@'");
                }
                Ok(Some(Destination::Email {
                    address: value.to_lowercase(),
                }))
            }
            "telegram" | "tg" => {
                // Telegram chat_ids are integers (optionally negative for groups/channels).
                if !value
                    .trim_start_matches('-')
                    .chars()
                    .all(|c| c.is_ascii_digit())
                {
                    return Err("telegram chat_id must be an integer");
                }
                Ok(Some(Destination::Telegram { chat_id: value }))
            }
            "discord" | "dc" => {
                // Discord channel IDs are snowflakes (positive integers).
                if !value.chars().all(|c| c.is_ascii_digit()) {
                    return Err("discord channel_id must be a positive integer");
                }
                Ok(Some(Destination::Discord { channel_id: value }))
            }
            other => Err(stringify_kind_err(other)),
        }
    }

    /// Parse a newline-separated list of `kind:value` entries. Blank lines
    /// are skipped. Returns the first parse error (with 1-based line number)
    /// if any.
    pub fn parse_list(text: &str) -> Result<Vec<Destination>, String> {
        let mut out = Vec::new();
        for (i, line) in text.lines().enumerate() {
            match Destination::parse_line(line) {
                Ok(Some(d)) => out.push(d),
                Ok(None) => {}
                Err(e) => return Err(format!("line {}: {}", i + 1, e)),
            }
        }
        Ok(out)
    }

    /// Format a list of destinations as newline-separated `kind:value` lines,
    /// suitable for round-tripping through [`Destination::parse_list`].
    pub fn format_list(destinations: &[Destination]) -> String {
        destinations
            .iter()
            .map(|d| format!("{}:{}", d.kind_label(), d.value()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// `stringify_kind_err` has to allocate the message, but we want a static
// error string for call sites. Keep a small wrapper that returns a fixed
// fallback — callers shouldn't rely on the echoed kind in the error.
fn stringify_kind_err(_kind: &str) -> &'static str {
    "unknown destination kind (use email, telegram, or discord)"
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
/// `EmailMessage.forward()` call. Preserves original From/To/DKIM. The
/// destination must be verified in the zone's Email Routing Destination
/// Addresses list.
pub struct ForwardInstruction {
    pub destination: String,
    /// Reply-To to overlay so recipient replies route back through the proxy.
    pub reply_to: String,
    /// Original sender for archival headers.
    pub original_from: String,
}

/// Bot-channel forward: send the parsed email content to a chat via the
/// relevant bot API. The caller saves a [`botrelay::ReplyContext`] keyed by
/// the returned message id so replies route back.
pub struct BotForward {
    pub channel: BotChannel,
    pub original_sender: String,
    pub alias: String,
    pub subject: String,
    pub text: String,
}

#[derive(Clone, Debug)]
pub enum BotChannel {
    Telegram { chat_id: String },
    Discord { channel_id: String },
}

/// Result of email processing — drives action in the wasm_bindgen email() export.
pub enum EmailResult {
    /// Silently drop the email.
    Drop,
    /// Reject the email with an SMTP error message.
    Reject(String),
    /// Execute a fan-out of forwards and/or new messages.
    Dispatch(Dispatch),
}

/// Everything the top-level handler needs to do for one inbound email.
/// Email destinations are sent via structured `send_email` to ensure
/// Reply-To and other headers are set correctly.
/// Bot forwards fan out to Telegram/Discord.
#[derive(Default)]
pub struct Dispatch {
    /// At most one — assigned to the first email destination if replace_reply_to
    /// is false, ensuring high fidelity (PGP/attachments preserved).
    pub forward_email: Option<ForwardInstruction>,
    /// Email destinations sent via structured `send_email`.
    pub send_emails: Vec<OutboundEmail>,
    /// Telegram + Discord bot posts.
    pub bot_forwards: Vec<BotForward>,
}

impl Dispatch {
    pub fn is_empty(&self) -> bool {
        self.forward_email.is_none() && self.send_emails.is_empty() && self.bot_forwards.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_email_destination() {
        let d = Destination::parse_line("email:Foo@Bar.com")
            .unwrap()
            .unwrap();
        assert_eq!(
            d,
            Destination::Email {
                address: "foo@bar.com".into()
            }
        );
    }

    #[test]
    fn parse_telegram_destination() {
        let d = Destination::parse_line("telegram:-100123")
            .unwrap()
            .unwrap();
        assert_eq!(
            d,
            Destination::Telegram {
                chat_id: "-100123".into()
            }
        );
        let d = Destination::parse_line("tg:42").unwrap().unwrap();
        assert_eq!(
            d,
            Destination::Telegram {
                chat_id: "42".into()
            }
        );
    }

    #[test]
    fn parse_discord_destination() {
        let d = Destination::parse_line("discord:987654321")
            .unwrap()
            .unwrap();
        assert_eq!(
            d,
            Destination::Discord {
                channel_id: "987654321".into()
            }
        );
    }

    #[test]
    fn blank_line_is_none() {
        assert!(Destination::parse_line("").unwrap().is_none());
        assert!(Destination::parse_line("   ").unwrap().is_none());
    }

    #[test]
    fn rejects_unknown_kind() {
        let err = Destination::parse_line("slack:abc").unwrap_err();
        assert!(err.contains("unknown destination kind"), "got: {err}");
    }

    #[test]
    fn rejects_missing_value() {
        assert!(Destination::parse_line("email:").is_err());
    }

    #[test]
    fn rejects_missing_at_in_email() {
        assert!(Destination::parse_line("email:not-an-email").is_err());
    }

    #[test]
    fn rejects_non_numeric_telegram() {
        assert!(Destination::parse_line("telegram:abc").is_err());
    }

    #[test]
    fn rejects_negative_discord() {
        // Discord snowflakes are positive; "-123" should be rejected.
        assert!(Destination::parse_line("discord:-123").is_err());
    }

    #[test]
    fn rejects_line_without_colon() {
        let err = Destination::parse_line("foo@bar.com").unwrap_err();
        assert!(err.contains("kind:value"), "got: {err}");
    }

    #[test]
    fn parse_list_round_trips_through_format_list() {
        let input = "email:a@b.com\n\ntelegram:-100\ndiscord:555";
        let parsed = Destination::parse_list(input).unwrap();
        assert_eq!(parsed.len(), 3);
        let formatted = Destination::format_list(&parsed);
        assert_eq!(formatted, "email:a@b.com\ntelegram:-100\ndiscord:555");
    }

    #[test]
    fn parse_list_reports_line_number_on_error() {
        let err = Destination::parse_list("email:a@b.com\nslack:c").unwrap_err();
        assert!(err.starts_with("line 2:"), "got: {err}");
    }

    #[test]
    fn action_serializes_with_tag() {
        let a = Action::Forward {
            destinations: vec![Destination::Email {
                address: "a@b".into(),
            }],
            replace_reply_to: true,
        };
        let j = serde_json::to_value(&a).unwrap();
        assert_eq!(j["type"], "forward");
        assert_eq!(j["destinations"][0]["kind"], "email");
        assert_eq!(j["destinations"][0]["address"], "a@b");
        assert_eq!(j["replace_reply_to"], true);
    }

    #[test]
    fn drop_action_serializes() {
        let j = serde_json::to_value(&Action::Drop).unwrap();
        assert_eq!(j, serde_json::json!({"type": "drop"}));
    }
}
