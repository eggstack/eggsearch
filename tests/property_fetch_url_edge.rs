use eggsearch::fetch::limits::{validate_url, FetchLimits};
use proptest::prelude::*;

fn http_scheme() -> impl Strategy<Value = String> {
    prop_oneof![Just("http"), Just("https"),].prop_map(|s| s.to_string())
}

proptest! {
    #[test]
    fn validate_url_rejects_ftp(
        host in "[a-z]{3,10}\\.[a-z]{2,3}"
    ) {
        let limits = FetchLimits::default();
        let url = format!("ftp://{host}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "ftp scheme should be rejected");
    }

    #[test]
    fn validate_url_rejects_file(
        path in "[a-z]{1,10}/[a-z]{1,10}"
    ) {
        let limits = FetchLimits::default();
        let url = format!("file:///{path}");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "file scheme should be rejected");
    }

    #[test]
    fn validate_url_rejects_ws(
        host in "[a-z]{3,10}\\.[a-z]{2,3}"
    ) {
        let limits = FetchLimits::default();
        let url = format!("ws://{host}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "ws scheme should be rejected");
    }

    #[test]
    fn validate_url_rejects_gopher(
        host in "[a-z]{3,10}\\.[a-z]{2,3}"
    ) {
        let limits = FetchLimits::default();
        let url = format!("gopher://{host}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "gopher scheme should be rejected");
    }

    #[test]
    fn validate_url_rejects_mailto(
        addr in "[a-z]{1,10}@[a-z]{3,10}\\.[a-z]{2,3}"
    ) {
        let limits = FetchLimits::default();
        let url = format!("mailto:{addr}");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "mailto scheme should be rejected");
    }

    #[test]
    fn validate_url_rejects_ssh(
        host in "[a-z]{3,10}\\.[a-z]{2,3}"
    ) {
        let limits = FetchLimits::default();
        let url = format!("ssh://{host}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "ssh scheme should be rejected");
    }

    #[test]
    fn validate_url_rejects_javascript_url(
        dummy in "[a-z]{1,3}"
    ) {
        let limits = FetchLimits::default();
        let _ = dummy;
        let result = validate_url("javascript:void(0)", &limits);
        prop_assert!(result.is_err(), "javascript URL should be rejected");
    }

    #[test]
    fn validate_url_rejects_data_url(
        dummy in "[a-z]{1,3}"
    ) {
        let limits = FetchLimits::default();
        let _ = dummy;
        let result = validate_url("data:text/html,<h1>hi</h1>", &limits);
        prop_assert!(result.is_err(), "data URL should be rejected");
    }

    #[test]
    fn validate_url_rejects_blob_url(
        dummy in "[a-z]{1,3}"
    ) {
        let limits = FetchLimits::default();
        let _ = dummy;
        let result = validate_url("blob:https://example.com/uuid", &limits);
        prop_assert!(result.is_err(), "blob URL should be rejected");
    }

    #[test]
    fn validate_url_rejects_empty_host(
        scheme in http_scheme()
    ) {
        let limits = FetchLimits::default();
        let url = format!("{scheme}:///");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "empty host should be rejected");
    }

    #[test]
    fn validate_url_rejects_excessive_url_length(
        path_len in 8193usize..20000usize
    ) {
        let limits = FetchLimits::default();
        let path = "a".repeat(path_len);
        let url = format!("https://example.com/{path}");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_err(), "URL exceeding max_url_len should be rejected");
    }

    #[test]
    fn validate_url_accepts_exactly_max_length(
        path_len in 0usize..8100usize
    ) {
        let base_len = "https://example.com/".len();
        let total = base_len + path_len;
        if total <= 8192 {
            let limits = FetchLimits::default();
            let path = "a".repeat(path_len);
            let url = format!("https://example.com/{path}");
            let result = validate_url(&url, &limits);
            prop_assert!(result.is_ok(), "URL within max_url_len should be accepted (len={})", url.len());
        }
    }

    #[test]
    fn validate_url_rejects_whitespace_only(s in "\\s{1,100}") {
        let limits = FetchLimits::default();
        let result = validate_url(&s, &limits);
        prop_assert!(result.is_err(), "whitespace-only URL should be rejected");
    }

    #[test]
    fn validate_url_accepts_percent_encoded_path(
        seg in "[a-z]{1,5}"
    ) {
        let limits = FetchLimits::default();
        let url = format!("https://example.com/path%20{seg}");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_ok(), "percent-encoded path should be accepted");
    }

    #[test]
    fn validate_url_accepts_ipv6_literal(
        groups in proptest::collection::vec("[0-9a-f]{1,4}", 8)
    ) {
        let limits = FetchLimits {
            allow_private_network: true,
            allow_localhost: true,
            ..Default::default()
        };
        let ipv6 = groups.join(":");
        let url = format!("http://[{ipv6}]/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_ok(), "IPv6 literal should be accepted: {}", url);
    }

    #[test]
    fn validate_url_accepts_port_0(
        scheme in http_scheme()
    ) {
        let limits = FetchLimits {
            allow_localhost: true,
            allow_private_network: true,
            ..Default::default()
        };
        let url = format!("{scheme}://example.com:0/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_ok(), "port 0 should be accepted");
    }

    #[test]
    fn validate_url_accepts_high_port(
        port in 49152u16..65535u16
    ) {
        let limits = FetchLimits::default();
        let url = format!("https://example.com:{port}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_ok(), "high port should be accepted");
    }

    #[test]
    fn validate_url_accepts_trailing_slash(
        path in "[a-z]{1,10}"
    ) {
        let limits = FetchLimits::default();
        let url = format!("https://example.com/{path}/");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_ok(), "trailing slash should be accepted");
    }

    #[test]
    fn validate_url_accepts_no_trailing_slash(
        path in "[a-z]{1,10}"
    ) {
        let limits = FetchLimits::default();
        let url = format!("https://example.com/{path}");
        let result = validate_url(&url, &limits);
        prop_assert!(result.is_ok(), "no trailing slash should be accepted");
    }

    #[test]
    fn validate_url_accepts_deep_path(
        segments in proptest::collection::vec("[a-z]{1,5}", 1..10)
    ) {
        let limits = FetchLimits::default();
        let path = segments.join("/");
        let url = format!("https://example.com/{path}");
        if url.len() <= 8192 {
            let result = validate_url(&url, &limits);
            prop_assert!(result.is_ok(), "deep path should be accepted");
        }
    }

}
