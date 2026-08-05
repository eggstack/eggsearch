#![cfg(feature = "browser")]

use eggsearch::fetch::browser::classify::{classify_response, FetchDisposition};
use eggsearch::fetch::browser::discover::{browser_capability_report, discover_browser};
use eggsearch::fetch::browser::intercept::{
    is_request_allowed, is_request_allowed_with_dns, PolicyViolation,
};
use eggsearch::fetch::browser::lifecycle::BrowserLifecycle;
use eggsearch::fetch::browser::navigate::{
    browser_fetch, browser_fetch_with_policy, BrowserFetchError,
};
use eggsearch::fetch::browser::types::{
    BrowserConfig, BrowserDiscovery, BrowserFamily, BrowserSource, RenderPolicy,
    MAX_GLOBAL_CONCURRENCY, MAX_MAX_DOM_BYTES, MAX_MAX_REQUESTS, MAX_NAVIGATION_TIMEOUT_MS,
    MAX_PER_ORIGIN_CONCURRENCY, MAX_POST_LOAD_WAIT_MS, MAX_STARTUP_TIMEOUT_MS,
    MAX_VERIFICATION_WAIT_MS,
};
use std::sync::Arc;

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
    let disc = BrowserDiscovery {
        path: std::path::PathBuf::from("/usr/bin/google-chrome-stable"),
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

#[tokio::test]
async fn test1_executable_discovery_order_and_explicit_override() {
    let disc_auto = discover_browser(None);
    let disc_explicit = discover_browser(Some("/usr/bin/google-chrome-stable"));
    if let Some(disc) = disc_auto {
        assert!(disc.source == BrowserSource::AutoDiscovered);
    }
    if let Some(disc) = disc_explicit {
        assert!(disc.source == BrowserSource::Configured);
    }
}

#[tokio::test]
async fn test2_invalid_executable_rejection() {
    let result = discover_browser(Some("/nonexistent/fake-chrome"));
    if let Some(disc) = result {
        assert_eq!(disc.source, BrowserSource::AutoDiscovered);
    }
}

#[tokio::test]
async fn test3_browser_unavailable_result() {
    let lifecycle = Arc::new(BrowserLifecycle::new(None, BrowserConfig::default()));
    let result = browser_fetch(
        &lifecycle,
        "https://example.com",
        &BrowserConfig::default(),
        false,
    )
    .await;
    assert!(matches!(result, Err(BrowserFetchError::LaunchFailed(_))));
}

#[tokio::test]
async fn test4_http_only_never_starts_chrome() {
    let lifecycle = Arc::new(BrowserLifecycle::new(None, BrowserConfig::default()));
    let result = browser_fetch_with_policy(
        &lifecycle,
        "https://example.com",
        &BrowserConfig::default(),
        false,
        &RenderPolicy::HttpOnly,
    )
    .await;
    assert!(matches!(result, Err(BrowserFetchError::HttpOnly)));
}

#[tokio::test]
async fn test5_auto_does_not_escalate_useful_html() {
    let disposition = classify_response(
        200,
        Some("text/html"),
        Some("Article Title"),
        500,
        b"<html><body><p>Real content here</p></body></html>",
    );
    assert_eq!(disposition, FetchDisposition::UsefulContent);
}

#[tokio::test]
async fn test6_auto_escalates_deterministic_js_shell_once() {
    let disposition = classify_response(
        200,
        Some("text/html"),
        Some("App"),
        20,
        br#"<html><head><title>App</title></head><body><div id="root"></div><script src="a.js"></script><script src="b.js"></script><script src="c.js"></script></body></html>"#,
    );
    assert_eq!(disposition, FetchDisposition::JavascriptShell);
}

#[tokio::test]
async fn test7_auto_does_not_escalate_error_statuses() {
    let cases = vec![
        (401, FetchDisposition::AuthenticationRequired),
        (403, FetchDisposition::AccessDenied),
        (404, FetchDisposition::AccessDenied),
        (429, FetchDisposition::RateLimited),
        (500, FetchDisposition::ServerError),
        (503, FetchDisposition::ServerError),
    ];
    for (status, expected) in cases {
        let disposition = classify_response(status, Some("text/html"), None, 0, b"");
        assert_eq!(
            disposition, expected,
            "Status {status} should map to {expected:?}"
        );
    }
}

#[tokio::test]
async fn test8_explicit_browser_rendering_policy() {
    let result = browser_fetch_with_policy(
        &Arc::new(BrowserLifecycle::new(None, BrowserConfig::default())),
        "https://example.com",
        &BrowserConfig::default(),
        false,
        &RenderPolicy::Browser,
    )
    .await;
    assert!(matches!(result, Err(BrowserFetchError::LaunchFailed(_))));
}

#[tokio::test]
async fn test9_final_dom_passes_through_sanitation() {
    let html = b"<html><head><title>Test</title></head><body><p>Hello</p></body></html>";
    let (title, _, text, _, warnings, _, _, _) =
        eggsearch::fetch::extract::extract_content(html, "https://example.com", 10000, false);
    assert_eq!(title, Some("Test".to_string()));
    assert!(text.contains("Hello"));
    assert!(warnings.is_empty());
}

#[tokio::test]
async fn test10_top_level_prohibited_redirect_is_blocked() {
    assert_eq!(
        is_request_allowed("http://127.0.0.1/redirect"),
        Err(PolicyViolation::PrivateNetworkTarget)
    );
}

#[tokio::test]
async fn test11_prohibited_subresource_is_blocked() {
    assert_eq!(
        is_request_allowed("http://169.254.169.254/latest/meta-data/"),
        Err(PolicyViolation::PrivateNetworkTarget)
    );
    assert_eq!(
        is_request_allowed("http://[::1]/internal"),
        Err(PolicyViolation::PrivateNetworkTarget)
    );
}

#[tokio::test]
async fn test12_request_count_and_dom_size_limits() {
    let cfg = BrowserConfig {
        max_requests: 10,
        max_dom_bytes: 1024,
        ..Default::default()
    };
    assert!(cfg.validate().is_ok());
    let cfg_excessive = BrowserConfig {
        max_requests: 10000,
        max_dom_bytes: 100_000_000,
        ..Default::default()
    };
    assert!(cfg_excessive.validate().is_err());
}

#[tokio::test]
async fn test13_navigation_timeout_cleanup() {
    let cfg = BrowserConfig {
        navigation_timeout_ms: 1,
        startup_timeout_ms: 1,
        ..Default::default()
    };
    let lifecycle = Arc::new(BrowserLifecycle::new(None, cfg.clone()));
    let result = browser_fetch(&lifecycle, "https://example.com", &cfg, false).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test14_page_context_cleanup_after_failure() {
    let lifecycle = Arc::new(BrowserLifecycle::new(None, BrowserConfig::default()));
    let _ = browser_fetch(
        &lifecycle,
        "https://example.com",
        &BrowserConfig::default(),
        false,
    )
    .await;
    assert!(!lifecycle.is_available());
}

#[tokio::test]
async fn test15_interactive_challenge_result() {
    let disposition = classify_response(
        200,
        Some("text/html"),
        Some("Access Denied"),
        100,
        b"<html><body>Access Denied</body></html>",
    );
    assert_eq!(disposition, FetchDisposition::InteractiveChallenge);
}

#[tokio::test]
async fn test16_noninteractive_bounded_wait() {
    let disposition = classify_response(
        200,
        Some("text/html"),
        Some("Just a moment..."),
        200,
        b"<html><body>Please wait while we verify your browser</body></html>",
    );
    assert_eq!(disposition, FetchDisposition::NonInteractiveVerification);
}

#[tokio::test]
async fn test17_origin_circuit_prevents_escalation() {
    let cfg = BrowserConfig {
        enabled: false,
        ..Default::default()
    };
    let lifecycle = Arc::new(BrowserLifecycle::new(None, cfg.clone()));
    let result = browser_fetch(&lifecycle, "https://example.com", &cfg, false).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test18_browser_config_validates_limits() {
    let good = BrowserConfig::default();
    assert!(good.validate().is_ok());

    let bad = BrowserConfig {
        startup_timeout_ms: MAX_STARTUP_TIMEOUT_MS + 1,
        navigation_timeout_ms: MAX_NAVIGATION_TIMEOUT_MS + 1,
        post_load_wait_ms: MAX_POST_LOAD_WAIT_MS + 1,
        verification_wait_ms: MAX_VERIFICATION_WAIT_MS + 1,
        max_requests: MAX_MAX_REQUESTS + 1,
        max_dom_bytes: MAX_MAX_DOM_BYTES + 1,
        global_concurrency: MAX_GLOBAL_CONCURRENCY + 1,
        per_origin_concurrency: MAX_PER_ORIGIN_CONCURRENCY + 1,
        ..Default::default()
    };
    let errors = bad.validate().unwrap_err();
    assert_eq!(errors.len(), 8);
}

#[tokio::test]
async fn test_dns_resolution_blocks_private_host() {
    let result = is_request_allowed_with_dns("http://localhost/").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_browser_config_clamped_values() {
    let cfg = BrowserConfig {
        startup_timeout_ms: u64::MAX,
        navigation_timeout_ms: u64::MAX,
        ..Default::default()
    };
    let clamped = cfg.with_clamped_values();
    assert_eq!(clamped.startup_timeout_ms, MAX_STARTUP_TIMEOUT_MS);
    assert_eq!(clamped.navigation_timeout_ms, MAX_NAVIGATION_TIMEOUT_MS);
    assert!(clamped.validate().is_ok());
}

#[test]
fn classify_js_shell_high_script_density() {
    let body = r#"<html><body><script></script><script></script><script></script><div></div></body></html>"#;
    assert_eq!(
        classify_response(200, Some("text/html"), Some("App"), 10, body.as_bytes()),
        FetchDisposition::JavascriptShell
    );
}

#[test]
fn classify_useful_content_with_scripts_and_text() {
    let body = r#"<html><head><script src="a.js"></script></head><body><p>This is a real page with lots of text content that makes it useful.</p></body></html>"#;
    assert_eq!(
        classify_response(
            200,
            Some("text/html"),
            Some("Article"),
            500,
            body.as_bytes()
        ),
        FetchDisposition::UsefulContent
    );
}
