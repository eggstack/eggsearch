#![cfg(feature = "browser")]

use eggsearch::fetch::browser::classify::{classify_response, FetchDisposition};
use eggsearch::fetch::browser::discover::{browser_capability_report, discover_browser};
use eggsearch::fetch::browser::intercept::{is_request_allowed, PolicyViolation};
use eggsearch::fetch::browser::types::{BrowserConfig, BrowserFamily, BrowserSource, RenderPolicy};

#[test]
fn classify_useful_html() {
    assert_eq!(
        classify_response(
            200,
            Some("text/html"),
            Some("My Page"),
            500,
            b"<html><body>Hello</body></html>"
        ),
        FetchDisposition::UsefulContent
    );
}

#[test]
fn classify_http_401() {
    assert_eq!(
        classify_response(401, Some("text/html"), None, 0, b""),
        FetchDisposition::AuthenticationRequired
    );
}

#[test]
fn classify_http_403() {
    assert_eq!(
        classify_response(403, Some("text/html"), None, 0, b""),
        FetchDisposition::AccessDenied
    );
}

#[test]
fn classify_http_404() {
    assert_eq!(
        classify_response(404, Some("text/html"), None, 0, b""),
        FetchDisposition::AccessDenied
    );
}

#[test]
fn classify_http_429() {
    assert_eq!(
        classify_response(429, Some("text/html"), None, 0, b""),
        FetchDisposition::RateLimited
    );
}

#[test]
fn classify_http_500() {
    assert_eq!(
        classify_response(500, Some("text/html"), None, 0, b""),
        FetchDisposition::ServerError
    );
}

#[test]
fn classify_http_503() {
    assert_eq!(
        classify_response(503, Some("text/html"), None, 0, b""),
        FetchDisposition::ServerError
    );
}

#[test]
fn classify_non_html_is_useful() {
    assert_eq!(
        classify_response(200, Some("application/json"), None, 0, b"{}"),
        FetchDisposition::UsefulContent
    );
}

#[test]
fn classify_interactive_challenge_title() {
    assert_eq!(
        classify_response(
            200,
            Some("text/html"),
            Some("Access Denied"),
            200,
            b"<html></html>"
        ),
        FetchDisposition::InteractiveChallenge
    );
}

#[test]
fn classify_interactive_challenge_body() {
    let body = r#"<html><body><div class="cf-turnstile"></div></body></html>"#;
    assert_eq!(
        classify_response(
            200,
            Some("text/html"),
            Some("Just a moment"),
            200,
            body.as_bytes()
        ),
        FetchDisposition::InteractiveChallenge
    );
}

#[test]
fn classify_noninteractive_verification() {
    assert_eq!(
        classify_response(
            200,
            Some("text/html"),
            Some("Just a moment..."),
            200,
            b"<html><body>Please wait while we verify</body></html>"
        ),
        FetchDisposition::NonInteractiveVerification
    );
}

#[test]
fn classify_js_shell_empty_root() {
    let body = r#"<html><head><title>App</title></head><body><div id="root"></div><script src="a.js"></script><script src="b.js"></script><script src="c.js"></script></body></html>"#;
    assert_eq!(
        classify_response(200, Some("text/html"), Some("App"), 20, body.as_bytes()),
        FetchDisposition::JavascriptShell
    );
}

#[test]
fn policy_allows_public_https() {
    assert!(is_request_allowed("https://example.com").is_ok());
}

#[test]
fn policy_allows_public_http() {
    assert!(is_request_allowed("http://example.com").is_ok());
}

#[test]
fn policy_blocks_ftp() {
    assert!(matches!(
        is_request_allowed("ftp://example.com/file"),
        Err(PolicyViolation::UnsupportedScheme(_))
    ));
}

#[test]
fn policy_blocks_file() {
    assert!(matches!(
        is_request_allowed("file:///etc/passwd"),
        Err(PolicyViolation::UnsupportedScheme(_))
    ));
}

#[test]
fn policy_blocks_localhost() {
    assert_eq!(
        is_request_allowed("http://localhost/"),
        Err(PolicyViolation::PrivateNetworkTarget)
    );
}

#[test]
fn policy_blocks_loopback() {
    assert_eq!(
        is_request_allowed("http://127.0.0.1/"),
        Err(PolicyViolation::PrivateNetworkTarget)
    );
}

#[test]
fn policy_blocks_private_10() {
    assert_eq!(
        is_request_allowed("http://10.0.0.1/"),
        Err(PolicyViolation::PrivateNetworkTarget)
    );
}

#[test]
fn policy_blocks_private_192_168() {
    assert_eq!(
        is_request_allowed("http://192.168.1.1/"),
        Err(PolicyViolation::PrivateNetworkTarget)
    );
}

#[test]
fn policy_blocks_link_local() {
    assert_eq!(
        is_request_allowed("http://169.254.1.1/"),
        Err(PolicyViolation::PrivateNetworkTarget)
    );
}

#[test]
fn policy_blocks_embedded_credentials() {
    assert_eq!(
        is_request_allowed("https://user:pass@example.com/"),
        Err(PolicyViolation::EmbeddedCredentials)
    );
}

#[test]
fn policy_blocks_ipv6_loopback() {
    assert_eq!(
        is_request_allowed("http://[::1]/"),
        Err(PolicyViolation::PrivateNetworkTarget)
    );
}

#[test]
fn policy_blocks_metadata_endpoint() {
    assert_eq!(
        is_request_allowed("http://169.254.169.254/metadata"),
        Err(PolicyViolation::PrivateNetworkTarget)
    );
}

#[test]
fn policy_blocks_cgnat_range() {
    assert_eq!(
        is_request_allowed("http://100.64.0.1/"),
        Err(PolicyViolation::PrivateNetworkTarget)
    );
}

#[test]
fn policy_blocks_documentation_range() {
    assert_eq!(
        is_request_allowed("http://192.0.2.1/"),
        Err(PolicyViolation::PrivateNetworkTarget)
    );
}

#[test]
fn discovery_does_not_panic_with_invalid_path() {
    let _result = discover_browser(Some("/nonexistent/path/to/chrome"));
}

#[test]
fn discovery_does_not_panic_with_empty_path() {
    let _result = discover_browser(Some(""));
}

#[test]
fn discovery_does_not_panic_with_none() {
    let _result = discover_browser(None);
}

#[test]
fn capability_report_no_browser() {
    let report = browser_capability_report(true, false, None);
    assert_eq!(report["browser_feature_compiled"], true);
    assert_eq!(report["browser_enabled_in_config"], false);
    assert_eq!(report["executable_discovered"], false);
    assert_eq!(report["usable"], false);
}

#[test]
fn capability_report_with_browser() {
    use eggsearch::fetch::browser::types::BrowserDiscovery;
    use std::path::PathBuf;

    let disc = BrowserDiscovery {
        path: PathBuf::from("/usr/bin/google-chrome-stable"),
        family: BrowserFamily::Chrome,
        version: "Chrome 120.0.0.0".into(),
        source: BrowserSource::AutoDiscovered,
    };
    let report = browser_capability_report(true, true, Some(&disc));
    assert_eq!(report["browser_feature_compiled"], true);
    assert_eq!(report["browser_enabled_in_config"], true);
    assert_eq!(report["executable_discovered"], true);
    assert_eq!(report["usable"], true);
    assert_eq!(report["browser_version"], "Chrome 120.0.0.0");
}

#[test]
fn browser_config_defaults() {
    let cfg = BrowserConfig::default();
    assert!(!cfg.enabled);
    assert_eq!(cfg.policy, RenderPolicy::HttpOnly);
    assert!(cfg.executable.is_none());
    assert_eq!(cfg.startup_timeout_ms, 10_000);
    assert_eq!(cfg.navigation_timeout_ms, 20_000);
    assert_eq!(cfg.post_load_wait_ms, 1_500);
    assert_eq!(cfg.verification_wait_ms, 10_000);
    assert_eq!(cfg.max_requests, 100);
    assert_eq!(cfg.max_dom_bytes, 4_000_000);
    assert_eq!(cfg.global_concurrency, 1);
    assert_eq!(cfg.per_origin_concurrency, 1);
    assert!(cfg.block_media);
}

#[test]
fn browser_config_toml_roundtrip() {
    let cfg = BrowserConfig::default();
    let text = toml::to_string(&cfg).unwrap();
    let parsed: BrowserConfig = toml::from_str(&text).unwrap();
    assert_eq!(parsed.enabled, cfg.enabled);
    assert_eq!(parsed.policy, cfg.policy);
    assert_eq!(parsed.startup_timeout_ms, cfg.startup_timeout_ms);
}

#[test]
fn render_policy_default_is_http_only() {
    assert_eq!(RenderPolicy::default(), RenderPolicy::HttpOnly);
}

#[test]
fn render_policy_toml_roundtrip() {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct Wrapper {
        policy: RenderPolicy,
    }
    let policies = vec![
        RenderPolicy::HttpOnly,
        RenderPolicy::Auto,
        RenderPolicy::Browser,
    ];
    for policy in policies {
        let w = Wrapper {
            policy: policy.clone(),
        };
        let text = toml::to_string(&w).unwrap();
        let parsed: Wrapper = toml::from_str(&text).unwrap();
        assert_eq!(parsed.policy, policy);
    }
}

#[test]
fn fetch_disposition_all_variants() {
    let variants = vec![
        FetchDisposition::UsefulContent,
        FetchDisposition::JavascriptShell,
        FetchDisposition::NonInteractiveVerification,
        FetchDisposition::InteractiveChallenge,
        FetchDisposition::RateLimited,
        FetchDisposition::AccessDenied,
        FetchDisposition::AuthenticationRequired,
        FetchDisposition::ServerError,
        FetchDisposition::Unsupported,
    ];
    for v in &variants {
        assert!(!format!("{v:?}").is_empty());
    }
    assert_eq!(variants.len(), 9);
}
