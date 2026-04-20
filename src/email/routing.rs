use crate::types::Rule;

/// Case-insensitive glob matching.
/// Supports `*` (any sequence) and `?` (single char).
pub fn glob_match(pattern: &str, value: &str) -> bool {
    let pattern = pattern.to_lowercase();
    let value = value.to_lowercase();
    glob_match_bytes(pattern.as_bytes(), value.as_bytes())
}

fn glob_match_bytes(pattern: &[u8], value: &[u8]) -> bool {
    let mut pi = 0;
    let mut vi = 0;
    let mut star_pi = None;
    let mut star_vi = 0;

    while vi < value.len() {
        if pi < pattern.len() && (pattern[pi] == b'?' || pattern[pi] == value[vi]) {
            pi += 1;
            vi += 1;
        } else if pi < pattern.len() && pattern[pi] == b'*' {
            star_pi = Some(pi);
            star_vi = vi;
            pi += 1;
        } else if let Some(sp) = star_pi {
            pi = sp + 1;
            star_vi += 1;
            vi = star_vi;
        } else {
            return false;
        }
    }

    while pi < pattern.len() && pattern[pi] == b'*' {
        pi += 1;
    }

    pi == pattern.len()
}

/// Check if an email's local and domain parts match a rule's patterns.
pub fn matches_rule(rule: &Rule, local: &str, domain: &str) -> bool {
    glob_match(&rule.local_pattern, local) && glob_match(&rule.domain_pattern, domain)
}

/// Find the first matching rule (top-to-bottom evaluation).
pub fn find_matching_rule<'a>(rules: &'a [Rule], local: &str, domain: &str) -> Option<&'a Rule> {
    rules.iter().find(|r| matches_rule(r, local, domain))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Action;

    // ---- glob_match tests ----

    #[test]
    fn glob_exact_match() {
        assert!(glob_match("hello", "hello"));
        assert!(!glob_match("hello", "world"));
    }

    #[test]
    fn glob_case_insensitive() {
        assert!(glob_match("Hello", "hello"));
        assert!(glob_match("HELLO", "hello"));
        assert!(glob_match("hello", "HELLO"));
    }

    #[test]
    fn glob_star_matches_any() {
        assert!(glob_match("*", "anything"));
        assert!(glob_match("*", ""));
        assert!(glob_match("hello*", "hello world"));
        assert!(glob_match("*world", "hello world"));
        assert!(glob_match("*llo*", "hello world"));
    }

    #[test]
    fn glob_question_mark() {
        assert!(glob_match("h?llo", "hello"));
        assert!(glob_match("h?llo", "hallo"));
        assert!(!glob_match("h?llo", "hllo"));
    }

    #[test]
    fn glob_complex_patterns() {
        assert!(glob_match("support+*", "support+billing"));
        assert!(!glob_match("support+*", "info"));
    }

    #[test]
    fn glob_empty() {
        assert!(glob_match("", ""));
        assert!(!glob_match("", "something"));
        assert!(glob_match("*", ""));
    }

    // ---- rule matching tests ----

    fn make_rule(local: &str, domain: &str) -> Rule {
        Rule {
            id: format!("rule-{local}-{domain}"),
            local_pattern: local.into(),
            domain_pattern: domain.into(),
            action: Action::Drop,
            label: format!("{local}@{domain}"),
        }
    }

    #[test]
    fn catch_all_matches_everything() {
        let rule = make_rule("*", "*");
        assert!(matches_rule(&rule, "anything", "any.com"));
        assert!(matches_rule(&rule, "", ""));
    }

    #[test]
    fn exact_local_any_domain() {
        let rule = make_rule("shop", "*");
        assert!(matches_rule(&rule, "shop", "example.com"));
        assert!(matches_rule(&rule, "shop", "other.org"));
        assert!(!matches_rule(&rule, "info", "example.com"));
    }

    #[test]
    fn any_local_exact_domain() {
        let rule = make_rule("*", "example.com");
        assert!(matches_rule(&rule, "anything", "example.com"));
        assert!(!matches_rule(&rule, "anything", "other.com"));
    }

    #[test]
    fn exact_both() {
        let rule = make_rule("shop", "example.com");
        assert!(matches_rule(&rule, "shop", "example.com"));
        assert!(!matches_rule(&rule, "info", "example.com"));
        assert!(!matches_rule(&rule, "shop", "other.com"));
    }

    #[test]
    fn first_match_wins() {
        let rules = vec![
            make_rule("shop", "*"),        // rule 0: specific local
            make_rule("*", "example.com"), // rule 1: specific domain
            make_rule("*", "*"),           // rule 2: catch-all
        ];

        // "shop@example.com" matches rule 0 first
        let matched = find_matching_rule(&rules, "shop", "example.com");
        assert_eq!(matched.unwrap().id, "rule-shop-*");

        // "info@example.com" doesn't match rule 0, matches rule 1
        let matched = find_matching_rule(&rules, "info", "example.com");
        assert_eq!(matched.unwrap().id, "rule-*-example.com");

        // "info@other.com" falls through to catch-all
        let matched = find_matching_rule(&rules, "info", "other.com");
        assert_eq!(matched.unwrap().id, "rule-*-*");
    }

    #[test]
    fn no_rules_returns_none() {
        let rules: Vec<Rule> = vec![];
        assert!(find_matching_rule(&rules, "a", "b.com").is_none());
    }
}
