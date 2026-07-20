use eggsearch::fetch::limits::{validate_url, FetchLimits};
use proptest::prelude::*;

proptest! {
    #[test]
    fn validate_url_rejects_private_tld_internal(
        name in "[a-z]{2,10}"
    ) {
        let limits = FetchLimits::default();
        let url = format!("http://{name}.internal/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "host ending in .internal should be rejected");
    }

    #[test]
    fn validate_url_rejects_private_tld_private(
        name in "[a-z]{2,10}"
    ) {
        let limits = FetchLimits::default();
        let url = format!("http://{name}.private/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "host ending in .private should be rejected");
    }

    #[test]
    fn validate_url_rejects_private_tld_local(
        name in "[a-z]{2,10}"
    ) {
        let limits = FetchLimits::default();
        let url = format!("http://{name}.local/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "host ending in .local should be rejected");
    }

    #[test]
    fn validate_url_rejects_private_tld_lan(
        name in "[a-z]{2,10}"
    ) {
        let limits = FetchLimits::default();
        let url = format!("http://{name}.lan.example/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "host containing .lan. should be rejected");
    }

    #[test]
    fn validate_url_accepts_private_tld_when_allowed(
        tld in prop_oneof!["internal", "private", "local"],
        name in "[a-z]{2,10}"
    ) {
        let limits = FetchLimits {
            allow_private_network: true,
            ..Default::default()
        };
        let url = format!("http://{name}.{tld}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_ok(), "private TLD should be accepted when allowed");
    }

    #[test]
    fn validate_url_rejects_172_16_private_range(
        second in 16u8..32u8,
        third in 0u8..255u8,
        fourth in 1u8..254u8
    ) {
        let limits = FetchLimits::default();
        let url = format!("http://172.{second}.{third}.{fourth}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "172.16-31.x.x should be rejected");
    }

    #[test]
    fn validate_url_rejects_cgnat_100_range(
        second in 64u8..128u8,
        third in 0u8..255u8,
        fourth in 1u8..254u8
    ) {
        let limits = FetchLimits::default();
        let url = format!("http://100.{second}.{third}.{fourth}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "100.64-127.x.x should be rejected as CGNAT/private");
    }

    #[test]
    fn validate_url_accepts_public_100_0_to_63(
        second in 0u8..64u8,
        third in 0u8..255u8,
        fourth in 1u8..254u8
    ) {
        let limits = FetchLimits::default();
        let url = format!("http://100.{second}.{third}.{fourth}/");
        let result = validate_url(&url, &limits);
        if result.is_err() {
            let err = result.unwrap_err();
            prop_assert!(
                !matches!(err, eggsearch::fetch::types::FetchError::PrivateNetworkBlocked(_)),
                "100.0-63.x.x should not be rejected as private"
            );
        }
    }

    #[test]
    fn validate_url_accepts_public_100_128_to_100_255(
        third in 0u8..255u8,
        fourth in 1u8..254u8
    ) {
        let limits = FetchLimits::default();
        let url = format!("http://100.128.{third}.{fourth}/");
        let result = validate_url(&url, &limits);
        if result.is_err() {
            let err = result.unwrap_err();
            prop_assert!(
                !matches!(err, eggsearch::fetch::types::FetchError::PrivateNetworkBlocked(_)),
                "100.128-255.x.x should not be rejected as private"
            );
        }
    }

    #[test]
    fn validate_url_rejects_link_local_v4_range(
        third in 0u8..255u8,
        fourth in 1u8..254u8
    ) {
        let limits = FetchLimits::default();
        let url = format!("http://169.254.{third}.{fourth}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "169.254.x.x should be rejected");
    }

    #[test]
    fn validate_url_port_boundary(
        port in 1u16..65535u16
    ) {
        let limits = FetchLimits {
            allow_localhost: true,
            ..Default::default()
        };
        let url = format!("http://localhost:{port}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_ok(), "localhost with any port should be accepted when allowed");
    }

    #[test]
    fn validate_url_fragment_stripped_from_identity(
        path in "[a-z]{1,10}",
        fragment in "[a-z]{1,10}"
    ) {
        let limits = FetchLimits::default();
        let url1 = format!("https://example.com/{path}#{fragment}");
        let url2 = format!("https://example.com/{path}");
        let r1 = validate_url(&url1, &limits);
        let r2 = validate_url(&url2, &limits);
        prop_assert!(r1.is_ok() && r2.is_ok(), "both URLs should be valid");
    }

    #[test]
    fn validate_url_query_does_not_affect_acceptance(
        path in "[a-z]{1,10}",
        key in "[a-z]{1,5}",
        value in "[a-z]{1,5}"
    ) {
        let limits = FetchLimits::default();
        let url_with = format!("https://example.com/{path}?{key}={value}");
        let url_without = format!("https://example.com/{path}");
        let r1 = validate_url(&url_with, &limits);
        let r2 = validate_url(&url_without, &limits);
        prop_assert_eq!(r1.is_ok(), r2.is_ok(), "query params should not affect acceptance");
    }

    #[test]
    fn validate_url_www_subdomain_accepted(
        name in "[a-z]{2,10}"
    ) {
        let limits = FetchLimits::default();
        let url = format!("https://www.{name}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_ok(), "www subdomain should be accepted");
    }

    #[test]
    fn validate_url_deep_subdomain_accepted(
        sub1 in "[a-z]{2,8}",
        sub2 in "[a-z]{2,8}",
        name in "[a-z]{2,8}"
    ) {
        let limits = FetchLimits::default();
        let url = format!("https://{sub1}.{sub2}.{name}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_ok(), "deep subdomain should be accepted");
    }

    #[test]
    fn validate_url_numeric_subdomain_accepted(
        name in "[a-z]{2,8}"
    ) {
        let limits = FetchLimits::default();
        let url = format!("https://123.{name}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_ok(), "numeric subdomain should be accepted");
    }

    #[test]
    fn validate_url_rejects_non_http_schemes(
        scheme in prop_oneof!["ftp", "ssh", "file", "data", "javascript", "mailto", "tel", "ws"]
    ) {
        let limits = FetchLimits::default();
        let url = format!("{scheme}://example.com/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "non-http scheme '{}' should be rejected", scheme);
    }

    #[test]
    fn validate_url_rejects_oversized_urls(
        path_len in 200usize..8000usize
    ) {
        let limits = FetchLimits {
            max_url_len: 200,
            ..Default::default()
        };
        let path = "a".repeat(path_len);
        let url = format!("https://example.com/{path}");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "URL exceeding max_url_len should be rejected");
    }

    #[test]
    fn validate_url_accepts_within_length_limit(
        path in "[a-z]{1,100}"
    ) {
        let limits = FetchLimits {
            max_url_len: 200,
            ..Default::default()
        };
        let url = format!("https://example.com/{path}");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_ok(), "URL within length limit should be accepted");
    }

    #[test]
    fn validate_url_rejects_malformed_urls(
        prefix in "[a-z]{1,5}",
        garbage in "[^a-zA-Z0-9]{1,20}"
    ) {
        let limits = FetchLimits::default();
        let url = format!("{prefix}://{garbage} ");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "malformed URL should be rejected");
    }

    #[test]
    fn validate_url_localhost_rejected_by_default(
        port in 1u16..65535u16
    ) {
        let limits = FetchLimits {
            allow_localhost: false,
            ..Default::default()
        };
        let url = format!("http://localhost:{port}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "localhost should be rejected when disallowed");
    }

    #[test]
    fn validate_url_10_private_rejected(
        second in 0u8..255u8,
        third in 0u8..255u8,
        fourth in 1u8..254u8
    ) {
        let limits = FetchLimits::default();
        let url = format!("http://10.{second}.{third}.{fourth}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "10.x.x.x should be rejected as private");
    }

    #[test]
    fn validate_url_192_168_private_rejected(
        third in 0u8..255u8,
        fourth in 1u8..254u8
    ) {
        let limits = FetchLimits::default();
        let url = format!("http://192.168.{third}.{fourth}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "192.168.x.x should be rejected as private");
    }

    #[test]
    fn validate_url_multicast_rejected(
        first in 224u8..240u8,
        second in 0u8..255u8,
        third in 0u8..255u8,
        fourth in 1u8..254u8
    ) {
        let limits = FetchLimits::default();
        let url = format!("http://{first}.{second}.{third}.{fourth}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "multicast address should be rejected");
    }

    #[test]
    fn validate_url_loopback_rejected(
        second in 0u8..255u8,
        third in 0u8..255u8,
        fourth in 1u8..254u8
    ) {
        let limits = FetchLimits::default();
        let url = format!("http://127.{second}.{third}.{fourth}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "loopback 127.x.x.x should be rejected");
    }
}

#[test]
fn validate_url_rejects_empty_url() {
    let limits = FetchLimits::default();
    let result = validate_url("", &limits);
    assert!(result.is_err(), "empty URL should be rejected");
}

#[test]
fn validate_url_rejects_whitespace_only_url() {
    let limits = FetchLimits::default();
    let result = validate_url("   ", &limits);
    assert!(result.is_err(), "whitespace-only URL should be rejected");
}
