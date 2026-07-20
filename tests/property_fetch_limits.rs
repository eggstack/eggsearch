use eggsearch::fetch::limits::{validate_url, FetchLimits};
use proptest::prelude::*;

fn https_url_strategy() -> impl Strategy<Value = String> {
    "https://[a-z][a-z0-9-]*\\.[a-z]{2,}/[a-zA-Z0-9/_.-]{0,50}"
}

proptest! {
    #[test]
    fn validate_url_rejects_empty(s in "\\s*") {
        let limits = FetchLimits::default();
        let result = validate_url(&s, &limits);
        prop_assert!(result.is_err(), "empty URL should be rejected");
    }

    #[test]
    fn validate_url_rejects_non_http_scheme(
        scheme in prop_oneof!["ftp", "file", "ssh", "ws", "gopher", "mailto"],
        host in "[a-z]{3,10}\\.[a-z]{2,3}"
    ) {
        let limits = FetchLimits::default();
        let url = format!("{scheme}://{host}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "non-http scheme '{}' should be rejected", scheme);
    }

    #[test]
    fn validate_url_rejects_url_exceeding_max_length(
        path in "[a-z]{1,5}"
    ) {
        let limits = FetchLimits {
            max_url_len: 50,
            ..Default::default()
        };
        let base = "https://example.com/";
        let total_len = base.len() + path.len();
        if total_len > limits.max_url_len {
            let url = format!("{base}{path}");
            let result = validate_url(&url, &limits);
            prop_assert!(result.is_err(), "URL exceeding max_url_len should be rejected");
        }
    }

    #[test]
    fn validate_url_rejects_localhost_when_disallowed(port in 1u16..65535u16) {
        let limits = FetchLimits {
            allow_localhost: false,
            ..Default::default()
        };
        let url = format!("http://localhost:{port}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "localhost should be rejected when allow_localhost=false");
    }

    #[test]
    fn validate_url_rejects_loopback_ip_when_disallowed(
        a in 0u8..255u8,
        b in 0u8..255u8,
        c in 0u8..255u8
    ) {
        let limits = FetchLimits {
            allow_localhost: false,
            ..Default::default()
        };
        let url = format!("http://127.{a}.{b}.{c}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "127.x.x.x should be rejected when allow_localhost=false");
    }

    #[test]
    fn validate_url_rejects_private_ip_10_when_disallowed(
        b in 0u8..255u8,
        c in 0u8..255u8,
        d in 1u8..254u8
    ) {
        let limits = FetchLimits {
            allow_private_network: false,
            ..Default::default()
        };
        let url = format!("http://10.{b}.{c}.{d}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "10.x.x.x should be rejected when allow_private_network=false");
    }

    #[test]
    fn validate_url_rejects_private_ip_192_168_when_disallowed(
        c in 0u8..255u8,
        d in 1u8..254u8
    ) {
        let limits = FetchLimits {
            allow_private_network: false,
            ..Default::default()
        };
        let url = format!("http://192.168.{c}.{d}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "192.168.x.x should be rejected when allow_private_network=false");
    }
}

proptest! {
    #[test]
    fn validate_url_accepts_private_ip_10_when_allowed(
        b in 0u8..255u8,
        c in 0u8..255u8,
        d in 1u8..254u8
    ) {
        let limits = FetchLimits {
            allow_private_network: true,
            allow_localhost: true,
            ..Default::default()
        };
        let url = format!("http://10.{b}.{c}.{d}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_ok(), "10.x.x.x should be accepted when allow_private_network=true");
    }

    #[test]
    fn validate_url_accepts_valid_public_https(url in https_url_strategy()) {
        let limits = FetchLimits::default();
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_ok(), "valid HTTPS URL should be accepted: {}", url);
    }

    #[test]
    fn validate_url_accepts_valid_public_http(host in "[a-z][a-z0-9.-]+\\.[a-z]{2,}") {
        let limits = FetchLimits::default();
        let url = format!("http://{host}/");
        let result = validate_url(&url, &limits);
        if result.is_err() {
            let err = result.unwrap_err();
            prop_assert!(
                !matches!(err, eggsearch::fetch::types::FetchError::PrivateNetworkBlocked(_)),
                "public HTTP host '{}' should not be rejected as private: {:?}", host, err
            );
        }
    }

    #[test]
    fn validate_url_accepts_loopback_when_allowed(port in 1u16..65535u16) {
        let limits = FetchLimits {
            allow_localhost: true,
            ..Default::default()
        };
        let url = format!("http://localhost:{port}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_ok(), "localhost should be accepted when allow_localhost=true");
    }
}
