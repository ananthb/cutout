//! Rule-set validation: catches invalid destinations, empty forwards,
//! duplicates, and unreachable rules (earlier rule's pattern subsumes
//! later's). Run before saving a rule set.

use crate::bots::EnabledChannels;
use crate::types::{Action, Destination, Rule};

/// Validation problem attached to a rule by position. Errors block a save;
/// warnings (currently just "unreachable") are surfaced to the UI but don't
/// block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Issue {
    /// An error that prevents save.
    Error(String),
    /// Non-fatal warning.
    Warning(String),
}

impl Issue {
    pub fn is_error(&self) -> bool {
        matches!(self, Issue::Error(_))
    }
    pub fn message(&self) -> &str {
        match self {
            Issue::Error(m) | Issue::Warning(m) => m,
        }
    }
}

/// A validation report: per-rule issues plus a flat error list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// `issues[i]` is the list of issues attached to `rules[i]`, in the
    /// order the rules were provided to [`validate`].
    pub issues: Vec<Vec<Issue>>,
}

impl Report {
    pub fn has_errors(&self) -> bool {
        self.issues
            .iter()
            .any(|rule_issues| rule_issues.iter().any(Issue::is_error))
    }

    pub fn first_error(&self) -> Option<(usize, &str)> {
        for (i, rule_issues) in self.issues.iter().enumerate() {
            for issue in rule_issues {
                if let Issue::Error(m) = issue {
                    return Some((i, m.as_str()));
                }
            }
        }
        None
    }

    /// Flatten for simple string reporting: one line per issue with 1-based
    /// rule numbering. Used by tests and for server-side log lines.
    #[allow(dead_code)]
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (i, rule_issues) in self.issues.iter().enumerate() {
            for issue in rule_issues {
                let kind = if issue.is_error() { "error" } else { "warning" };
                out.push(format!("rule {} {kind}: {}", i + 1, issue.message()));
            }
        }
        out
    }
}

/// Validate a whole rule set in place. Always returns a [`Report`] (possibly
/// empty). Use [`Report::has_errors`] to decide whether to save.
/// `enabled` flags which chat channels currently have their bot secrets set;
/// destinations pointing at disabled channels are flagged as errors.
pub fn validate(rules: &[Rule], enabled: &EnabledChannels) -> Report {
    let mut report = Report {
        issues: vec![Vec::new(); rules.len()],
    };

    for (i, rule) in rules.iter().enumerate() {
        // Pattern can't be empty: block this, a rule with `""@example.com`
        // wouldn't match anything.
        if rule.local_pattern.is_empty() {
            report.issues[i].push(Issue::Error("local pattern is empty".into()));
        }
        if rule.domain_pattern.is_empty() {
            report.issues[i].push(Issue::Error("domain pattern is empty".into()));
        }

        // Forward action must have destinations; each destination's channel
        // must be enabled.
        if let Action::Forward {
            destinations,
            replace_reply_to: _,
        } = &rule.action
        {
            if destinations.is_empty() {
                report.issues[i].push(Issue::Error(
                    "Forward action needs at least one destination".into(),
                ));
            }
            for dest in destinations {
                match dest {
                    Destination::Telegram { .. } if !enabled.telegram => {
                        report.issues[i].push(Issue::Error(
                            "telegram destination requires TELEGRAM_BOT_TOKEN secret".into(),
                        ));
                    }
                    Destination::Discord { .. } if !enabled.discord => {
                        report.issues[i].push(Issue::Error(
                            "discord destination requires DISCORD_BOT_TOKEN + DISCORD_APP_ID + DISCORD_PUBLIC_KEY secrets".into(),
                        ));
                    }
                    _ => {}
                }
            }
        }

        // Duplicate detection: exact same (local, domain) as another rule.
        for (j, other) in rules.iter().enumerate() {
            if i == j {
                continue;
            }
            if eq_ci(&rule.local_pattern, &other.local_pattern)
                && eq_ci(&rule.domain_pattern, &other.domain_pattern)
                && j < i
            {
                report.issues[i].push(Issue::Error(format!(
                    "duplicate patterns with rule {}: {}@{}",
                    j + 1,
                    other.local_pattern,
                    other.domain_pattern
                )));
                break;
            }
        }

        // Unreachable detection: any earlier rule whose patterns subsume
        // this rule's patterns.
        for (j, earlier) in rules.iter().enumerate().take(i) {
            if glob_subsumes(&earlier.local_pattern, &rule.local_pattern)
                && glob_subsumes(&earlier.domain_pattern, &rule.domain_pattern)
                && !(eq_ci(&earlier.local_pattern, &rule.local_pattern)
                    && eq_ci(&earlier.domain_pattern, &rule.domain_pattern))
            {
                report.issues[i].push(Issue::Warning(format!(
                    "unreachable: rule {} ({}@{}) already matches everything this rule matches",
                    j + 1,
                    earlier.local_pattern,
                    earlier.domain_pattern
                )));
                break;
            }
        }
    }

    report
}

fn eq_ci(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Returns true if glob pattern `a` matches every string that glob pattern
/// `b` matches. Case-insensitive; supports `*` and `?`.
pub fn glob_subsumes(a: &str, b: &str) -> bool {
    let a = a.to_ascii_lowercase();
    let b = b.to_ascii_lowercase();
    subsumes_bytes(a.as_bytes(), b.as_bytes())
}

fn subsumes_bytes(a: &[u8], b: &[u8]) -> bool {
    match (a.first(), b.first()) {
        (None, None) => true,
        (None, Some(_)) => false,
        (Some(_), None) => a.iter().all(|&c| c == b'*'),
        (Some(&ac), Some(&bc)) => {
            if ac == b'*' {
                // Empty match of a's `*`: rest of a must subsume all of b.
                if subsumes_bytes(&a[1..], b) {
                    return true;
                }
                // b isn't empty here; consume one char from b if possible.
                // If b starts with `*`, a's `*` must absorb it too.
                if bc == b'*' {
                    return subsumes_bytes(a, &b[1..]);
                }
                return subsumes_bytes(a, &b[1..]);
            }
            if bc == b'*' {
                // a has no `*` at this position but b does. a can't subsume
                // arbitrary-length wildcards from b.
                return false;
            }
            if ac == b'?' {
                // a's `?` matches any single char. b at this position is one
                // char (literal or `?`). Advance both.
                return subsumes_bytes(&a[1..], &b[1..]);
            }
            if bc == b'?' {
                // b's `?` means b can produce any char; a must also accept
                // any char at this position. Literal a doesn't.
                return false;
            }
            if ac == bc {
                return subsumes_bytes(&a[1..], &b[1..]);
            }
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Destination, Rule};

    fn rule(id: &str, local: &str, domain: &str, action: Action) -> Rule {
        Rule {
            id: id.into(),
            local_pattern: local.into(),
            domain_pattern: domain.into(),
            action,
            label: id.into(),
        }
    }

    fn forward(dest: &str) -> Action {
        Action::Forward {
            destinations: vec![Destination::Email {
                address: dest.into(),
            }],
            replace_reply_to: false,
        }
    }

    fn all_enabled() -> EnabledChannels {
        EnabledChannels {
            telegram: true,
            discord: true,
        }
    }

    // ---- glob_subsumes ----

    #[test]
    fn star_subsumes_everything() {
        assert!(glob_subsumes("*", ""));
        assert!(glob_subsumes("*", "hello"));
        assert!(glob_subsumes("*", "*"));
        assert!(glob_subsumes("*", "hello*world"));
    }

    #[test]
    fn identical_patterns_subsume() {
        assert!(glob_subsumes("hello", "hello"));
        assert!(glob_subsumes("h?llo", "h?llo"));
        assert!(glob_subsumes("a*b", "a*b"));
    }

    #[test]
    fn case_insensitive() {
        assert!(glob_subsumes("HELLO", "hello"));
        assert!(glob_subsumes("hello", "HELLO"));
    }

    #[test]
    fn prefix_star_subsumes_literal_prefix() {
        assert!(glob_subsumes("a*", "abc"));
        assert!(glob_subsumes("shop*", "shop-newsletter"));
        assert!(!glob_subsumes("abc", "a*"));
    }

    #[test]
    fn question_mark_subsumes_single_char() {
        assert!(glob_subsumes("?", "a"));
        assert!(glob_subsumes("?", "?"));
        assert!(!glob_subsumes("?", "ab"));
        assert!(!glob_subsumes("a", "?"));
    }

    #[test]
    fn distinct_literals_do_not_subsume() {
        assert!(!glob_subsumes("abc", "abd"));
        assert!(!glob_subsumes("foo", "bar"));
    }

    #[test]
    fn nested_stars() {
        assert!(glob_subsumes("*a*", "aabb"));
        assert!(glob_subsumes("*a*", "xyaxy"));
        assert!(!glob_subsumes("*a*", "xyz"));
    }

    #[test]
    fn star_does_not_subsume_literal_char_if_pattern_has_more() {
        assert!(!glob_subsumes("a*b", "a"));
        assert!(glob_subsumes("a*b", "aXXXXb"));
        assert!(glob_subsumes("a*b", "ab"));
    }

    // ---- validate ----

    #[test]
    fn valid_ruleset_has_no_issues() {
        let rules = vec![
            rule("a", "shop", "example.com", forward("me@x.com")),
            rule("b", "*", "*", Action::Drop),
        ];
        let report = validate(&rules, &all_enabled());
        assert!(!report.has_errors(), "{:?}", report.lines());
        assert!(report.issues.iter().all(|r| r.is_empty()));
    }

    #[test]
    fn empty_forward_destinations_is_error() {
        let rules = vec![rule(
            "a",
            "x",
            "y",
            Action::Forward {
                destinations: vec![],
                replace_reply_to: false,
            },
        )];
        let report = validate(&rules, &all_enabled());
        assert!(report.has_errors());
        assert!(report.first_error().unwrap().1.contains("Forward action"));
    }

    #[test]
    fn empty_pattern_is_error() {
        let rules = vec![rule("a", "", "example.com", Action::Drop)];
        let report = validate(&rules, &all_enabled());
        assert!(report.has_errors());
    }

    #[test]
    fn duplicate_patterns_error_on_second_occurrence() {
        let rules = vec![
            rule("a", "shop", "example.com", forward("a@x")),
            rule("b", "SHOP", "Example.com", forward("b@x")),
        ];
        let report = validate(&rules, &all_enabled());
        assert!(report.has_errors());
        let (idx, msg) = report.first_error().unwrap();
        assert_eq!(idx, 1, "error should attach to second rule");
        assert!(msg.contains("duplicate"));
    }

    #[test]
    fn unreachable_warned_not_errored() {
        let rules = vec![
            rule("catch", "*", "*", Action::Drop),
            rule("specific", "shop", "example.com", forward("x@y")),
        ];
        let report = validate(&rules, &all_enabled());
        assert!(
            !report.has_errors(),
            "unreachable is a warning, not an error"
        );
        let warnings: Vec<_> = report.issues[1].iter().collect();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message().contains("unreachable"));
    }

    #[test]
    fn unreachable_via_partial_subsumption() {
        let rules = vec![
            rule("first", "shop*", "*", forward("x@y")),
            rule("second", "shop-foo", "example.com", forward("y@z")),
        ];
        let report = validate(&rules, &all_enabled());
        assert!(!report.has_errors());
        assert!(
            !report.issues[1].is_empty(),
            "second rule should be unreachable"
        );
    }

    #[test]
    fn not_unreachable_when_earlier_is_narrower() {
        let rules = vec![
            rule("first", "shop", "example.com", forward("x@y")),
            rule("catch-all", "*", "*", Action::Drop),
        ];
        let report = validate(&rules, &all_enabled());
        assert!(report.issues.iter().all(|r| r.is_empty()));
    }

    #[test]
    fn telegram_destination_requires_telegram_enabled() {
        let rules = vec![rule(
            "a",
            "shop",
            "x.com",
            Action::Forward {
                destinations: vec![Destination::Telegram {
                    chat_id: "-100123".into(),
                    link_auth: Default::default(),
                }],
                replace_reply_to: false,
            },
        )];
        let disabled = EnabledChannels {
            telegram: false,
            discord: true,
        };
        let report = validate(&rules, &disabled);
        assert!(report.has_errors());
        assert!(report
            .first_error()
            .unwrap()
            .1
            .contains("TELEGRAM_BOT_TOKEN"));
    }

    #[test]
    fn discord_destination_requires_discord_enabled() {
        let rules = vec![rule(
            "a",
            "shop",
            "x.com",
            Action::Forward {
                destinations: vec![Destination::Discord {
                    channel_id: "42".into(),
                    link_auth: Default::default(),
                }],
                replace_reply_to: false,
            },
        )];
        let disabled = EnabledChannels {
            telegram: true,
            discord: false,
        };
        let report = validate(&rules, &disabled);
        assert!(report.has_errors());
        assert!(report
            .first_error()
            .unwrap()
            .1
            .contains("DISCORD_BOT_TOKEN"));
    }

    #[test]
    fn chat_destinations_pass_when_enabled() {
        let rules = vec![rule(
            "a",
            "shop",
            "x.com",
            Action::Forward {
                destinations: vec![
                    Destination::Telegram {
                        chat_id: "-100".into(),
                        link_auth: Default::default(),
                    },
                    Destination::Discord {
                        channel_id: "555".into(),
                        link_auth: Default::default(),
                    },
                ],
                replace_reply_to: false,
            },
        )];
        let report = validate(&rules, &all_enabled());
        assert!(!report.has_errors(), "{:?}", report.lines());
    }

    #[test]
    fn email_destination_ignores_channel_flags() {
        let rules = vec![rule("a", "shop", "x.com", forward("me@x.com"))];
        let disabled = EnabledChannels {
            telegram: false,
            discord: false,
        };
        let report = validate(&rules, &disabled);
        assert!(!report.has_errors());
    }
}
