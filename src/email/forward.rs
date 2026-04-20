/// Check if an address is a reverse alias (reply+ prefix).
pub fn is_reverse_alias(address: &str) -> bool {
    address.starts_with("reply+")
}

/// Generate a reverse alias address for reply routing.
/// Format: `reply+<uuid>@<domain>`
pub fn generate_reverse_address(domain: &str) -> String {
    let token = uuid::Uuid::new_v4().to_string().replace('-', "");
    format!("reply+{token}@{domain}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_reverse_alias() {
        assert!(is_reverse_alias("reply+abc123@proxy.example.com"));
        assert!(!is_reverse_alias("shop123@proxy.example.com"));
        assert!(!is_reverse_alias(""));
    }

    #[test]
    fn generates_valid_reverse_address() {
        let addr = generate_reverse_address("proxy.example.com");
        assert!(addr.starts_with("reply+"));
        assert!(addr.ends_with("@proxy.example.com"));
        let token = addr
            .strip_prefix("reply+")
            .unwrap()
            .strip_suffix("@proxy.example.com")
            .unwrap();
        assert_eq!(token.len(), 32);
    }

    #[test]
    fn reverse_addresses_are_unique() {
        let a = generate_reverse_address("example.com");
        let b = generate_reverse_address("example.com");
        assert_ne!(a, b);
    }
}
