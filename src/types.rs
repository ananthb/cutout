use serde::{Deserialize, Deserializer, Serialize};

/// A routing rule. Rules are stored as an ordered `Vec<Rule>` in D1 (`rules_state`).
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
    /// Extra inbox readers named by the operator, over and above the rule's
    /// own email destinations. Every email destination is an implicit reader
    /// (see [`Rule::effective_readers`]); the `/inbox` page shows the stored
    /// mail of a rule to exactly the addresses in that effective set.
    #[serde(default)]
    pub inbox_readers: Vec<String>,
}

impl Rule {
    /// Returns true if this is the catch-all rule (`*@*`).
    pub fn is_catch_all(&self) -> bool {
        self.local_pattern == "*" && self.domain_pattern == "*"
    }

    /// User-provided label if non-empty, otherwise an auto-generated one
    /// derived from pattern + action.
    pub fn display_label(&self) -> String {
        let trimmed = self.label.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
        self.auto_label()
    }

    /// Generate a semantic label from the rule's pattern and action.
    /// Examples: "Catch-all", "*@example.com → Telegram",
    /// "news@*.org → Email + Discord", "*@example.com → drop".
    pub fn auto_label(&self) -> String {
        let is_catch = self.is_catch_all();
        match (&self.action, is_catch) {
            (Action::Drop, true) => "Catch-all".to_string(),
            (action, _) => {
                let prefix = if is_catch {
                    "Catch-all".to_string()
                } else {
                    format!("{}@{}", self.local_pattern, self.domain_pattern)
                };
                let suffix = match action {
                    Action::Drop => "drop".to_string(),
                    Action::Store { .. } => "store".to_string(),
                    Action::ForwardStore { destinations } => {
                        if destinations.is_empty() {
                            "store & forward (no destinations)".to_string()
                        } else {
                            format!("{} · stored", format_channels(destinations))
                        }
                    }
                    Action::Forward { destinations } => {
                        if destinations.is_empty() {
                            "forward (no destinations)".to_string()
                        } else {
                            format_channels(destinations)
                        }
                    }
                };
                format!("{prefix} → {suffix}")
            }
        }
    }

    /// Every address allowed to read this rule's stored mail: the rule's email
    /// destinations (implicit recipients) followed by any extra
    /// `inbox_readers`. Lower-cased and de-duplicated, order preserved.
    pub fn effective_readers(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        if let Action::Forward { destinations } | Action::ForwardStore { destinations } =
            &self.action
        {
            for d in destinations {
                if let Destination::Email { address, .. } = d {
                    push_unique_addr(&mut out, address);
                }
            }
        }
        for reader in &self.inbox_readers {
            push_unique_addr(&mut out, reader);
        }
        out
    }

    /// Whether `email` is in this rule's effective reader set (case-insensitive).
    pub fn is_reader(&self, email: &str) -> bool {
        let wanted = email.trim().to_lowercase();
        !wanted.is_empty() && self.effective_readers().contains(&wanted)
    }
}

/// Lower-case `addr`, drop blanks, and append to `out` unless already present.
fn push_unique_addr(out: &mut Vec<String>, addr: &str) {
    let addr = addr.trim().to_lowercase();
    if !addr.is_empty() && !out.contains(&addr) {
        out.push(addr);
    }
}

/// Render a destination list as a compact channel summary.
/// "Email", "Email + Telegram", "Email ×3 + Discord", etc.
fn format_channels(destinations: &[Destination]) -> String {
    let (mut email, mut telegram, mut discord, mut webhook) = (0usize, 0usize, 0usize, 0usize);
    for d in destinations {
        match d {
            Destination::Email { .. } => email += 1,
            Destination::Telegram { .. } => telegram += 1,
            Destination::Discord { .. } => discord += 1,
            Destination::Webhook { .. } => webhook += 1,
        }
    }
    let mut parts: Vec<String> = Vec::new();
    let push = |parts: &mut Vec<String>, n: usize, name: &str| {
        if n == 1 {
            parts.push(name.to_string());
        } else if n > 1 {
            parts.push(format!("{name} ×{n}"));
        }
    };
    push(&mut parts, email, "Email");
    push(&mut parts, telegram, "Telegram");
    push(&mut parts, discord, "Discord");
    push(&mut parts, webhook, "Webhook");
    parts.join(" + ")
}

/// A pragmatic email-address check: exactly one `@`, a non-empty local part
/// with no whitespace, and a dotted domain whose labels are alphanumeric or
/// hyphenated and whose TLD is alphabetic.
///
/// Deliberately stricter than RFC 5322 — it exists to catch typos (a stray
/// `=`, a missing dot) before they reach Cloudflare's sending API, which
/// otherwise fails the send with an opaque "Invalid email domain" only after
/// the message has been accepted and queued.
pub fn valid_email_address(addr: &str) -> bool {
    if addr.is_empty() || addr.len() > 254 || addr.chars().any(char::is_whitespace) {
        return false;
    }
    let mut parts = addr.split('@');
    let (local, domain) = match (parts.next(), parts.next(), parts.next()) {
        (Some(local), Some(domain), None) => (local, domain),
        _ => return false,
    };
    if local.is_empty()
        || local.len() > 64
        || !local
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "!#$%&'*+-/=?^_`{|}~.".contains(c))
    {
        return false;
    }
    let labels: Vec<&str> = domain.split('.').collect();
    if labels.len() < 2 {
        return false;
    }
    labels.iter().all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    }) && labels
        .last()
        .is_some_and(|tld| tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()))
}

/// A pragmatic webhook-URL check: an `http://` or `https://` scheme
/// (case-insensitive) followed by a non-empty remainder with no whitespace.
///
/// A scheme-only URL like `https://` passes a bare prefix test but can never
/// deliver, so it is rejected here rather than at send time.
pub fn valid_webhook_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    let rest = if let Some(rest) = lower.strip_prefix("https://") {
        rest
    } else if let Some(rest) = lower.strip_prefix("http://") {
        rest
    } else {
        return false;
    };
    !rest.is_empty() && !rest.chars().any(|c| c.is_ascii_whitespace())
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Forward inbound mail to one or more destinations (mixed-channel).
    /// Reply-to rewriting is per-destination (see [`Destination::Email::proxy`]).
    Forward { destinations: Vec<Destination> },
    /// Forward *and* persist a copy of the message (Store & Forward), so it
    /// shows up in the rule's stored-email list and the viewer.
    ForwardStore { destinations: Vec<Destination> },
    /// Silently drop.
    Drop,
    /// Store message for later retrieval.
    Store {
        #[serde(default)]
        persist: bool,
    },
}

// Wire format for `Action`, including the legacy `replace_reply_to` field.
// Older KV blobs may carry `replace_reply_to: true` at the action level; on
// load we fan that out into per-`Email` `proxy` flags and drop the field.
// Newer writes go directly through `Action`'s `Serialize` impl, which never
// emits this field.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ActionWire {
    Forward {
        destinations: Vec<Destination>,
        #[serde(default)]
        replace_reply_to: bool,
    },
    ForwardStore {
        destinations: Vec<Destination>,
    },
    Drop,
    Store {
        #[serde(default)]
        persist: bool,
    },
}

impl<'de> Deserialize<'de> for Action {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        Ok(match ActionWire::deserialize(de)? {
            ActionWire::Forward {
                mut destinations,
                replace_reply_to,
            } => {
                if replace_reply_to {
                    for d in destinations.iter_mut() {
                        if let Destination::Email { proxy, .. } = d {
                            *proxy = true;
                        }
                    }
                }
                Action::Forward { destinations }
            }
            ActionWire::ForwardStore { destinations } => Action::ForwardStore { destinations },
            ActionWire::Drop => Action::Drop,
            ActionWire::Store { persist } => Action::Store { persist },
        })
    }
}

/// Proxy is the default for email destinations; direct mode is opt-in.
fn proxy_default() -> bool {
    true
}

/// How often an email destination in digest mode receives its batch.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum DigestSchedule {
    #[serde(rename = "6h")]
    SixHours,
    #[serde(rename = "daily")]
    Daily,
    #[serde(rename = "weekly")]
    Weekly,
}

impl DigestSchedule {
    pub fn as_str(&self) -> &'static str {
        match self {
            DigestSchedule::SixHours => "6h",
            DigestSchedule::Daily => "daily",
            DigestSchedule::Weekly => "weekly",
        }
    }

    pub fn interval_ms(&self) -> i64 {
        const HOUR_MS: i64 = 60 * 60 * 1000;
        match self {
            DigestSchedule::SixHours => 6 * HOUR_MS,
            DigestSchedule::Daily => 24 * HOUR_MS,
            DigestSchedule::Weekly => 7 * 24 * HOUR_MS,
        }
    }

    pub fn parse(s: &str) -> Option<DigestSchedule> {
        let lower = s.trim().to_ascii_lowercase();
        match lower.as_str() {
            "6h" => Some(DigestSchedule::SixHours),
            "daily" => Some(DigestSchedule::Daily),
            "weekly" => Some(DigestSchedule::Weekly),
            _ => None,
        }
    }
}

/// A single panel entry in a pending digest: enough to render the summary
/// line and link back to the stored message.
#[derive(Clone, Debug)]
pub struct DigestItem {
    /// Row id in `digest_items`. Monotonic, so a snapshot's max id bounds the
    /// post-send delete to exactly the rows that were included.
    pub id: i64,
    pub message_id: String,
    pub sender: String,
    pub recipient: String,
    pub subject: String,
    pub created_at: i64,
}

impl Action {
    /// True when at least one email destination asks for a periodic digest.
    /// Digest destinations are not forwarded inline, so the email handler
    /// uses this to decide whether to persist the message and whether a
    /// `Drop` result actually stored mail.
    pub fn has_digest_destination(&self) -> bool {
        match self {
            Action::Forward { destinations } | Action::ForwardStore { destinations } => {
                destinations.iter().any(|d| {
                    matches!(
                        d,
                        Destination::Email {
                            digest: Some(_),
                            ..
                        }
                    )
                })
            }
            _ => false,
        }
    }
}

/// A single forward target. Multiple destinations of different kinds may be
/// attached to one rule.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Destination {
    /// Forward to an email address.
    ///
    /// Proxy mode (`proxy = true`, the default) re-originates the message
    /// through the structured `send_email` API. Cloudflare requires the `From`
    /// address to be on an onboarded sending domain, so the address becomes
    /// the alias the message was sent to; the original sender is carried in
    /// the display name and in `X-Original-From`, and `Reply-To` is the
    /// reverse alias so replies route back through Cutout.
    ///
    /// Direct mode (`proxy = false`) is the explicit opt-in to native
    /// `EmailMessage.forward()`, which preserves the original `From` and
    /// `Reply-To`. Replies then go straight to the sender and reveal this
    /// destination's address to them. The destination must be verified under
    /// Email Routing > Destination Addresses; an unverified one falls back to
    /// structured send, which replaces `From` with the alias.
    Email {
        address: String,
        #[serde(default = "proxy_default")]
        proxy: bool,
        /// Opt into a periodic digest instead of one forward per message.
        /// Digest destinations are stored (never forwarded inline) and the
        /// scheduled handler sends one email linking to each stored message.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        digest: Option<DigestSchedule>,
    },
    /// Forward to a Telegram chat via bot `sendMessage`.
    Telegram { chat_id: String },
    /// Forward to a Discord channel via bot `createMessage`.
    Discord { channel_id: String },
    /// POST the parsed email as JSON to an HTTP endpoint.
    Webhook { url: String },
}

impl Destination {
    /// Short kind label for logs / UI.
    pub fn kind_label(&self) -> &'static str {
        match self {
            Destination::Email { .. } => "email",
            Destination::Telegram { .. } => "telegram",
            Destination::Discord { .. } => "discord",
            Destination::Webhook { .. } => "webhook",
        }
    }

    /// The address/id value as a display string.
    pub fn value(&self) -> &str {
        match self {
            Destination::Email { address, .. } => address,
            Destination::Telegram { chat_id, .. } => chat_id,
            Destination::Discord { channel_id, .. } => channel_id,
            Destination::Webhook { url, .. } => url,
        }
    }

    /// Parse a single line of the form `kind:value` (e.g. `email:a@b.com`,
    /// `telegram:-100123`, `discord:987654321`). Returns `None` for blank
    /// lines.
    pub fn parse_line(line: &str) -> Result<Option<Destination>, &'static str> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        let (kind, rest) = trimmed
            .split_once(':')
            .ok_or("expected 'kind:value' (e.g. email:you@example.com)")?;
        let rest = rest.trim();
        if rest.is_empty() {
            return Err("value missing after ':'");
        }
        match kind.trim().to_lowercase().as_str() {
            "email" => {
                // `email:<address>[:modifier[:modifier...]]`. Modifiers are
                // order-independent: `direct`/`noproxy` opt out of proxy mode,
                // `proxy` (the default) opts back in, and `6h`/`daily`/`weekly`
                // select a digest schedule.
                let mut parts = rest.split(':');
                let value = parts.next().unwrap_or("").trim().to_string();
                let mut proxy = true;
                let mut digest = None;
                for raw in parts {
                    let modifier = raw.trim();
                    match modifier.to_ascii_lowercase().as_str() {
                        "proxy" => proxy = true,
                        "direct" | "noproxy" => proxy = false,
                        _ => match DigestSchedule::parse(modifier) {
                            Some(schedule) => digest = Some(schedule),
                            None => {
                                return Err(
                                    "email modifier must be 'direct', 'proxy', '6h', 'daily', or 'weekly'",
                                )
                            }
                        },
                    }
                }
                if !valid_email_address(&value) {
                    return Err("email address looks invalid (expected name@example.com)");
                }
                Ok(Some(Destination::Email {
                    address: value.to_lowercase(),
                    proxy,
                    digest,
                }))
            }
            "telegram" | "tg" => {
                let value = rest.to_string();
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
                let value = rest.to_string();
                if !value.chars().all(|c| c.is_ascii_digit()) {
                    return Err("discord channel_id must be a positive integer");
                }
                Ok(Some(Destination::Discord { channel_id: value }))
            }
            "webhook" | "wh" => {
                let value = rest.to_string();
                if !valid_webhook_url(&value) {
                    return Err("webhook url must start with http:// or https://");
                }
                Ok(Some(Destination::Webhook { url: value }))
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
    /// suitable for round-tripping through [`Destination::parse_list`]. An
    /// email destination appends `:direct` when it opts out of the default
    /// proxy mode and `:<schedule>` when it is in digest mode.
    pub fn format_list(destinations: &[Destination]) -> String {
        destinations
            .iter()
            .map(|d| match d {
                Destination::Email {
                    address,
                    proxy,
                    digest,
                } => {
                    let mut line = format!("email:{address}");
                    if !proxy {
                        line.push_str(":direct");
                    }
                    if let Some(schedule) = digest {
                        line.push(':');
                        line.push_str(schedule.as_str());
                    }
                    line
                }
                Destination::Telegram { chat_id } => format!("telegram:{chat_id}"),
                Destination::Discord { channel_id } => format!("discord:{channel_id}"),
                Destination::Webhook { url } => format!("webhook:{url}"),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// `stringify_kind_err` has to allocate the message, but we want a static
// error string for call sites. Keep a small wrapper that returns a fixed
// fallback: callers shouldn't rely on the echoed kind in the error.
fn stringify_kind_err(_kind: &str) -> &'static str {
    "unknown destination kind (use email, telegram, discord, or webhook)"
}

/// Reverse alias mapping stored in D1 (`reverse_mappings`).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ReverseAlias {
    /// The alias address the original email was sent to.
    pub alias: String,
    /// The external sender's address.
    pub original_sender: String,
}

/// A single outbound email to send via the EMAIL binding.
/// Cloudflare Email Service expects structured fields, not raw RFC 2822 bytes.
#[derive(Serialize, Deserialize, Clone, Debug)]
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
/// `EmailMessage.forward()` call. Preserves original From/To/DKIM, and is
/// therefore only used in direct mode. Cloudflare strips every non-`X-`
/// header on `forward()`, so no `Reply-To` is overlaid here.
pub struct ForwardInstruction {
    pub destination: String,
    /// The alias the inbound message was addressed to. Used as the `From`
    /// address if the native forward fails and has to be replayed through the
    /// structured API, which requires a Cutout-domain sender.
    pub alias: String,
    /// Original sender for archival headers.
    pub original_from: String,
}

/// Bot-channel forward: send the parsed email content to a chat via the
/// relevant bot API. The caller saves a [`botrelay::ReplyContext`] keyed by
/// the returned message id so replies route back.
///
/// `message_id` drives the "View full email" link composed into the chat post.
/// `html` is the inline-image-rewritten HTML body, used to render the in-chat
/// screenshot via Cloudflare Browser Rendering.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BotForward {
    pub channel: BotChannel,
    pub original_sender: String,
    pub alias: String,
    pub subject: String,
    pub text: String,
    /// UUID of the stored email; matches the `messages.id` row and
    /// `messages/{id}` R2 key. Used to build the inbox URL.
    #[serde(default)]
    pub message_id: String,
    /// Sanitized HTML body with `cid:` images inlined as `data:` URIs.
    /// `None` when the source email has no HTML part.
    #[serde(default)]
    pub html: Option<String>,
    /// RFC 5322 `Message-ID` of the inbound email, used to thread replies
    /// back into the original conversation in the recipient's mail client.
    #[serde(default)]
    pub inbound_message_id: Option<String>,
    /// RFC 5322 `References` of the inbound email, prepended to the new
    /// `References` chain on the outbound reply.
    #[serde(default)]
    pub inbound_references: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BotChannel {
    Telegram { chat_id: String },
    Discord { channel_id: String },
}

/// Webhook-channel forward: POST the parsed email as JSON to an operator-supplied
/// URL. Unlike bots, there is no reply path back through the worker.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WebhookForward {
    pub url: String,
    pub from: String,
    pub to: String,
    pub subject: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub html: Option<String>,
    #[serde(default)]
    pub message_id: String,
    pub rule_id: Option<String>,
    #[serde(default)]
    pub received_at: i64,
}

/// One outstanding destination on a queued retry. We do NOT carry the native
/// `EmailMessage.forward()` path here because the queue consumer has no
/// `IncomingEmailMessage` handle: instead, the original handler converts a
/// failed native forward into a structured [`OutboundEmail`] before queueing.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PendingAction {
    SendEmail(OutboundEmail),
    Bot(BotForward),
    Webhook(WebhookForward),
}

/// Queue payload for `cutout-retries` and `cutout-retries-dlq`. Carries only
/// the row id; the consumer hydrates the full state from D1 + R2.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RetryMsg {
    pub id: String,
}

/// Hydrated `pending_dispatches` row.
#[derive(Clone, Debug)]
pub struct PendingDispatch {
    pub id: String,
    pub sender: String,
    pub recipient: String,
    pub rule_id: Option<String>,
    pub r2_key: String,
    pub pending_actions: Vec<PendingAction>,
    pub attempts: u32,
    pub last_error: Option<String>,
    pub dead_lettered: bool,
}

/// Result of email processing: drives action in the wasm_bindgen email() export.
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
    /// Direct-mode destinations. Each one is forwarded with native
    /// `EmailMessage.forward()` so the original `From` survives; Cloudflare
    /// applies ARC so the forward stays authenticated. Proxy destinations go
    /// through `send_emails` instead.
    pub forward_emails: Vec<ForwardInstruction>,
    /// Proxy-mode email destinations sent via structured `send_email`.
    pub send_emails: Vec<OutboundEmail>,
    /// Telegram + Discord bot posts.
    pub bot_forwards: Vec<BotForward>,
    /// Webhook POSTs carrying the parsed email as JSON.
    pub webhook_forwards: Vec<WebhookForward>,
}

impl Dispatch {
    pub fn is_empty(&self) -> bool {
        self.forward_emails.is_empty()
            && self.send_emails.is_empty()
            && self.bot_forwards.is_empty()
            && self.webhook_forwards.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_email_destination_defaults_to_proxy() {
        let d = Destination::parse_line("email:Foo@Bar.com")
            .unwrap()
            .unwrap();
        assert_eq!(
            d,
            Destination::Email {
                address: "foo@bar.com".into(),
                proxy: true,
                digest: None,
            }
        );
    }

    #[test]
    fn parse_email_direct_modifier() {
        for input in ["email:foo@bar.com:direct", "email:foo@bar.com:noproxy"] {
            let d = Destination::parse_line(input).unwrap().unwrap();
            assert_eq!(
                d,
                Destination::Email {
                    address: "foo@bar.com".into(),
                    proxy: false,
                    digest: None,
                }
            );
        }
    }

    #[test]
    fn parse_email_with_proxy() {
        let d = Destination::parse_line("email:foo@bar.com:proxy")
            .unwrap()
            .unwrap();
        assert_eq!(
            d,
            Destination::Email {
                address: "foo@bar.com".into(),
                proxy: true,
                digest: None,
            }
        );
    }

    #[test]
    fn rejects_unknown_email_modifier() {
        let err = Destination::parse_line("email:foo@bar.com:token").unwrap_err();
        assert!(err.contains("email modifier"), "got: {err}");
    }

    #[test]
    fn parse_email_digest_modifiers() {
        for (input, schedule) in [
            ("email:foo@bar.com:6h", DigestSchedule::SixHours),
            ("email:foo@bar.com:daily", DigestSchedule::Daily),
            ("email:foo@bar.com:weekly", DigestSchedule::Weekly),
            ("email:foo@bar.com:WEEKLY", DigestSchedule::Weekly),
        ] {
            let d = Destination::parse_line(input).unwrap().unwrap();
            assert_eq!(
                d,
                Destination::Email {
                    address: "foo@bar.com".into(),
                    proxy: true,
                    digest: Some(schedule),
                }
            );
        }
    }

    #[test]
    fn email_modifiers_are_order_independent() {
        let d = Destination::parse_line("email:foo@bar.com:daily:direct")
            .unwrap()
            .unwrap();
        assert_eq!(
            d,
            Destination::Email {
                address: "foo@bar.com".into(),
                proxy: false,
                digest: Some(DigestSchedule::Daily),
            }
        );
        let d = Destination::parse_line("email:foo@bar.com:direct:daily")
            .unwrap()
            .unwrap();
        assert_eq!(
            d,
            Destination::Email {
                address: "foo@bar.com".into(),
                proxy: false,
                digest: Some(DigestSchedule::Daily),
            }
        );
    }

    #[test]
    fn digest_schedule_helpers() {
        assert_eq!(DigestSchedule::parse("6h"), Some(DigestSchedule::SixHours));
        assert_eq!(DigestSchedule::parse("daily"), Some(DigestSchedule::Daily));
        assert_eq!(
            DigestSchedule::parse("weekly"),
            Some(DigestSchedule::Weekly)
        );
        assert_eq!(DigestSchedule::parse("monthly"), None);
        assert_eq!(DigestSchedule::SixHours.as_str(), "6h");
        assert_eq!(DigestSchedule::SixHours.interval_ms(), 6 * 60 * 60 * 1000);
        assert_eq!(DigestSchedule::Daily.interval_ms(), 24 * 60 * 60 * 1000);
        assert_eq!(
            DigestSchedule::Weekly.interval_ms(),
            7 * 24 * 60 * 60 * 1000
        );
    }

    #[test]
    fn format_round_trips_digest_destinations() {
        let dests = vec![
            Destination::Email {
                address: "a@b.com".into(),
                proxy: true,
                digest: Some(DigestSchedule::Daily),
            },
            Destination::Email {
                address: "c@d.com".into(),
                proxy: false,
                digest: Some(DigestSchedule::SixHours),
            },
        ];
        assert_eq!(
            Destination::format_list(&dests),
            "email:a@b.com:daily\nemail:c@d.com:direct:6h"
        );
        let parsed = Destination::parse_list(&Destination::format_list(&dests)).unwrap();
        assert_eq!(parsed, dests);
    }

    #[test]
    fn has_digest_destination_only_for_email_digests() {
        let digest = Action::Forward {
            destinations: vec![Destination::Email {
                address: "a@b.com".into(),
                proxy: true,
                digest: Some(DigestSchedule::Weekly),
            }],
        };
        assert!(digest.has_digest_destination());
        let plain = Action::ForwardStore {
            destinations: vec![Destination::Email {
                address: "a@b.com".into(),
                proxy: true,
                digest: None,
            }],
        };
        assert!(!plain.has_digest_destination());
        assert!(!Action::Store { persist: true }.has_digest_destination());
        assert!(!Action::Drop.has_digest_destination());
    }

    #[test]
    fn parse_telegram_destination() {
        let d = Destination::parse_line("telegram:-100123")
            .unwrap()
            .unwrap();
        assert_eq!(
            d,
            Destination::Telegram {
                chat_id: "-100123".into(),
            }
        );
        let d = Destination::parse_line("tg:42").unwrap().unwrap();
        assert_eq!(
            d,
            Destination::Telegram {
                chat_id: "42".into(),
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
                channel_id: "987654321".into(),
            }
        );
    }

    #[test]
    fn parse_webhook_destination() {
        for input in [
            "webhook:https://example.com/hook",
            "wh:https://example.com/hook",
            "WEBHOOK:https://example.com/hook",
        ] {
            let d = Destination::parse_line(input).unwrap().unwrap();
            assert_eq!(
                d,
                Destination::Webhook {
                    url: "https://example.com/hook".into(),
                }
            );
        }
        // Plain http and an uppercase scheme are both accepted.
        let d = Destination::parse_line("webhook:HTTP://example.com/hook")
            .unwrap()
            .unwrap();
        assert_eq!(
            d,
            Destination::Webhook {
                url: "HTTP://example.com/hook".into(),
            }
        );
    }

    #[test]
    fn valid_webhook_url_requires_host_after_scheme() {
        assert!(valid_webhook_url("https://example.com/hook"));
        assert!(valid_webhook_url("http://example.com"));
        assert!(valid_webhook_url("HTTPS://example.com"));
        assert!(!valid_webhook_url("https://"));
        assert!(!valid_webhook_url("http://"));
        assert!(!valid_webhook_url("ftp://x"));
        assert!(!valid_webhook_url("https://a b"));
        assert!(!valid_webhook_url("example.com"));
        assert!(!valid_webhook_url(""));
    }

    #[test]
    fn rejects_scheme_only_webhook_destination() {
        for input in ["webhook:https://", "wh:http://", "webhook:https://a b"] {
            let err = Destination::parse_line(input).unwrap_err();
            assert!(err.contains("webhook url"), "got: {err}");
        }
    }

    #[test]
    fn rejects_webhook_without_http_scheme() {
        for input in [
            "webhook:example.com/hook",
            "webhook:ftp://example.com/hook",
            "wh:not-a-url",
        ] {
            let err = Destination::parse_line(input).unwrap_err();
            assert!(err.contains("webhook url"), "got: {err}");
        }
    }

    #[test]
    fn format_round_trips_chat_destinations() {
        let dests = vec![
            Destination::Telegram {
                chat_id: "-100".into(),
            },
            Destination::Discord {
                channel_id: "555".into(),
            },
            Destination::Webhook {
                url: "https://example.com/hook".into(),
            },
        ];
        let formatted = Destination::format_list(&dests);
        assert_eq!(
            formatted,
            "telegram:-100\ndiscord:555\nwebhook:https://example.com/hook"
        );
        let parsed = Destination::parse_list(&formatted).unwrap();
        assert_eq!(parsed, dests);
    }

    #[test]
    fn auto_label_counts_webhooks() {
        let rule = Rule {
            id: "r".into(),
            local_pattern: "news".into(),
            domain_pattern: "*.org".into(),
            action: Action::Forward {
                destinations: vec![
                    Destination::Email {
                        address: "a@b.com".into(),
                        proxy: true,
                        digest: None,
                    },
                    Destination::Webhook {
                        url: "https://example.com/hook".into(),
                    },
                    Destination::Webhook {
                        url: "https://example.com/other".into(),
                    },
                ],
            },
            label: String::new(),
            inbox_readers: vec![],
        };
        assert_eq!(rule.auto_label(), "news@*.org → Email + Webhook ×2");
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
    fn webhook_destination_serializes_with_kind_tag() {
        let j = serde_json::to_value(&Destination::Webhook {
            url: "https://example.com/hook".into(),
        })
        .unwrap();
        assert_eq!(
            j,
            serde_json::json!({"kind": "webhook", "url": "https://example.com/hook"})
        );
        let back: Destination = serde_json::from_value(j).unwrap();
        assert_eq!(
            back,
            Destination::Webhook {
                url: "https://example.com/hook".into(),
            }
        );
    }

    #[test]
    fn action_serializes_with_tag() {
        let a = Action::Forward {
            destinations: vec![Destination::Email {
                address: "a@b".into(),
                proxy: true,
                digest: None,
            }],
        };
        let j = serde_json::to_value(&a).unwrap();
        assert_eq!(j["type"], "forward");
        assert_eq!(j["destinations"][0]["kind"], "email");
        assert_eq!(j["destinations"][0]["address"], "a@b");
        assert_eq!(j["destinations"][0]["proxy"], true);
        assert!(j.get("replace_reply_to").is_none());
    }

    #[test]
    fn legacy_replace_reply_to_fans_out_to_proxy() {
        // KV blobs written before the per-destination modifier landed carry
        // the action-level `replace_reply_to` flag. Loading them must promote
        // every email destination to `proxy = true`.
        let legacy = serde_json::json!({
            "type": "forward",
            "destinations": [
                {"kind": "email", "address": "a@b.com"},
                {"kind": "email", "address": "c@d.com"},
                {"kind": "telegram", "chat_id": "42"},
            ],
            "replace_reply_to": true,
        });
        let a: Action = serde_json::from_value(legacy).unwrap();
        match a {
            Action::Forward { destinations } => {
                assert_eq!(destinations.len(), 3);
                assert_eq!(
                    destinations[0],
                    Destination::Email {
                        address: "a@b.com".into(),
                        proxy: true,
                        digest: None,
                    }
                );
                assert_eq!(
                    destinations[1],
                    Destination::Email {
                        address: "c@d.com".into(),
                        proxy: true,
                        digest: None,
                    }
                );
                assert!(matches!(destinations[2], Destination::Telegram { .. }));
            }
            _ => panic!("expected forward"),
        }
    }

    #[test]
    fn legacy_without_proxy_field_defaults_to_proxy() {
        // Blobs written before the per-destination modifier existed have no
        // `proxy` field. Proxy is now the default, so they load as proxy.
        let legacy = serde_json::json!({
            "type": "forward",
            "destinations": [
                {"kind": "email", "address": "a@b.com"},
            ],
        });
        let a: Action = serde_json::from_value(legacy).unwrap();
        match a {
            Action::Forward { destinations } => assert_eq!(
                destinations[0],
                Destination::Email {
                    address: "a@b.com".into(),
                    proxy: true,
                    digest: None,
                }
            ),
            _ => panic!("expected forward"),
        }
    }

    #[test]
    fn format_emits_direct_modifier() {
        let dests = vec![
            Destination::Email {
                address: "a@b.com".into(),
                proxy: true,
                digest: None,
            },
            Destination::Email {
                address: "c@d.com".into(),
                proxy: false,
                digest: None,
            },
        ];
        assert_eq!(
            Destination::format_list(&dests),
            "email:a@b.com\nemail:c@d.com:direct"
        );
        let parsed = Destination::parse_list(&Destination::format_list(&dests)).unwrap();
        assert_eq!(parsed, dests);
    }

    #[test]
    fn drop_action_serializes() {
        let j = serde_json::to_value(&Action::Drop).unwrap();
        assert_eq!(j, serde_json::json!({"type": "drop"}));
    }

    #[test]
    fn accepts_valid_email_addresses() {
        assert!(valid_email_address("a@b.com"));
        assert!(valid_email_address("first.last+tag@sub.example.co.uk"));
        assert!(valid_email_address("Foo@Bar.com"));
    }

    #[test]
    fn rejects_malformed_email_addresses() {
        // A trailing `=` (the reported case) must not reach the sending API.
        assert!(!valid_email_address("arulpriya93@gmail.com="));
        assert!(!valid_email_address("a@b"));
        assert!(!valid_email_address("a b@c.com"));
        assert!(!valid_email_address("a@@c.com"));
        assert!(!valid_email_address("@c.com"));
        assert!(!valid_email_address("a@.com"));
        assert!(!valid_email_address("a@c."));
        assert!(!valid_email_address("a@c.c"));
        assert!(!valid_email_address(""));
    }

    #[test]
    fn effective_readers_unions_destinations_and_explicit() {
        let rule = Rule {
            id: "r".into(),
            local_pattern: "*".into(),
            domain_pattern: "*".into(),
            action: Action::Forward {
                destinations: vec![
                    Destination::Email {
                        address: "Alice@Example.com".into(),
                        proxy: false,
                        digest: None,
                    },
                    Destination::Telegram {
                        chat_id: "42".into(),
                    },
                ],
            },
            label: String::new(),
            inbox_readers: vec!["bob@x.com".into(), "ALICE@Example.com".into(), "  ".into()],
        };
        assert_eq!(
            rule.effective_readers(),
            vec!["alice@example.com", "bob@x.com"]
        );
        assert!(rule.is_reader("alice@example.com"));
        assert!(rule.is_reader("BOB@X.com"));
        assert!(!rule.is_reader("carol@x.com"));
        assert!(!rule.is_reader(""));
    }

    #[test]
    fn effective_readers_empty_for_store_rule_without_explicit() {
        let rule = Rule {
            id: "r".into(),
            local_pattern: "*".into(),
            domain_pattern: "*".into(),
            action: Action::Store { persist: true },
            label: String::new(),
            inbox_readers: vec![],
        };
        assert!(rule.effective_readers().is_empty());
        assert!(!rule.is_reader("anyone@example.com"));
    }
}
