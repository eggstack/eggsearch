use super::*;
use crate::core::config::AppConfig;
use crate::mcp::state::ServerState;
use std::sync::Arc;

#[cfg(all(feature = "mock", feature = "browser"))]
fn synthetic_available_browser_state(
    browser_fetch_override: crate::mcp::state::BrowserFetchOverride,
) -> Arc<ServerState> {
    use crate::fetch::browser::{
        BrowserConfig, BrowserDiscovery, BrowserDiscoveryState, BrowserFamily, BrowserLifecycle,
        BrowserSource, RenderPolicy,
    };

    let mut config = AppConfig::default();
    config.fetch.allow_localhost = true;
    config.fetch.allow_private_network = true;
    config.fetch.browser.enabled = true;
    let mut state = Arc::new(ServerState::build(config).expect("state builds"));
    let discovery = BrowserDiscovery {
        path: std::path::PathBuf::from("/synthetic/chrome"),
        family: BrowserFamily::Chrome,
        version: "Chrome 120.0.0.0".to_string(),
        source: BrowserSource::AutoDiscovered,
    };
    let browser_config = BrowserConfig {
        enabled: true,
        policy: RenderPolicy::Browser,
        ..BrowserConfig::default()
    };
    let state_mut = Arc::get_mut(&mut state).expect("unique state");
    state_mut.browser_discovery_state = BrowserDiscoveryState::Available(discovery.clone());
    state_mut.browser_lifecycle = Some(Arc::new(BrowserLifecycle::new(
        Some(discovery),
        browser_config,
    )));
    state_mut.browser_fetch_override = Some(browser_fetch_override);
    state
}

#[cfg(all(feature = "mock", feature = "browser"))]
#[tokio::test]
async fn browser_absent_mcp_search_fetch_and_status_degrade() {
    use crate::meta::mock::{mock_engines, MockEngine, MockResult};
    use httpmock::prelude::*;

    let server = MockServer::start();
    let _ordinary = server.mock(|when, then| {
        when.method(GET).path("/ordinary");
        then.status(200).header("content-type", "text/html").body(
            "<html><body>Ordinary page with readable content and no render markers.</body></html>",
        );
    });
    let _verification = server.mock(|when, then| {
        when.method(GET).path("/verification");
        then.status(200)
            .header("content-type", "text/html")
            .body("<html><head><title>Checking your browser</title></head><body>Please wait while we verify.</body></html>");
    });
    let mut config = AppConfig::default();
    config.fetch.allow_localhost = true;
    config.fetch.allow_private_network = true;
    config.fetch.browser.enabled = true;
    config.fetch.browser.executable = Some("/synthetic/missing/chrome".to_string());
    let engine = MockEngine::success(
        "duckduckgo",
        vec![MockResult::new(
            "Example result",
            "https://example.com/result",
            "duckduckgo",
        )],
    );
    let adapter = crate::meta::MetadataSearchAdapter::from_engines(
        mock_engines(vec![engine]),
        std::time::Duration::from_millis(config.search.timeout_ms),
    );
    let mut state = Arc::new(ServerState::build(config).expect("state builds"));
    Arc::get_mut(&mut state).expect("unique state").adapter = Arc::new(adapter);

    let search = run_web_search(
        state.clone(),
        WebSearchArgs {
            query: "test query".to_string(),
            max_results: Some(5),
            providers: vec![],
            safe_search: None,
            timeout_ms: None,
            intent: None,
            freshness: None,
            date_range: None,
            include_domains: Vec::new(),
            exclude_domains: Vec::new(),
            language: None,
            region: None,
            excerpt_count: None,
            response_detail: None,
        },
    )
    .await
    .expect("search does not require Chrome");
    assert!(search.to_string().contains("Example result"));

    let ordinary = run_web_fetch(
        state.clone(),
        serde_json::from_value(serde_json::json!({
            "url": server.url("/ordinary"),
            "render": "auto"
        }))
        .unwrap(),
    )
    .await
    .expect("ordinary HTTP fetch does not require Chrome");
    assert!(!ordinary["structured_warnings"]
        .to_string()
        .contains("browser_unavailable"));

    let verification = run_web_fetch(
        state.clone(),
        serde_json::from_value(serde_json::json!({
            "url": server.url("/verification"),
            "render": "auto"
        }))
        .unwrap(),
    )
    .await
    .expect("auto fetch keeps the HTTP response without Chrome");
    assert!(verification["structured_warnings"]
        .to_string()
        .contains("browser_unavailable"));
    assert!(
        verification["structured_warnings"]
            .to_string()
            .contains("/synthetic/missing/chrome"),
        "{verification}"
    );

    let explicit = run_web_fetch(
        state.clone(),
        serde_json::from_value(serde_json::json!({
            "url": "https://example.test/explicit-browser",
            "render": "browser"
        }))
        .unwrap(),
    )
    .await
    .expect_err("explicit browser request must report unavailability");
    assert_eq!(
        explicit.error_payload()["data"]["code"],
        "browser_unavailable"
    );
    assert!(explicit
        .error_payload()
        .to_string()
        .contains("/synthetic/missing/chrome"));

    let status = super::provider_status::run_provider_status(
        state,
        super::provider_status::ProviderStatusArgs {
            probe: false,
            recipe_detail: None,
            response_detail: None,
        },
    )
    .expect("provider status remains available without Chrome");
    let capabilities = &status["browser_capabilities"];
    assert_eq!(capabilities["usable"], false);
    assert_eq!(capabilities["discovered"], false);
    assert!(capabilities["reason"]
        .as_str()
        .unwrap_or_default()
        .contains("/synthetic/missing/chrome"));
    for field in [
        "compiled",
        "configured",
        "discovered",
        "usable",
        "reason",
        "lifecycle",
    ] {
        assert!(capabilities.get(field).is_some(), "missing field {field}");
    }
}

#[cfg(all(feature = "mock", feature = "browser"))]
#[tokio::test]
async fn browser_absent_batch_and_unified_fetch_preserve_specific_error_code() {
    let mut config = AppConfig::default();
    config.fetch.browser.enabled = true;
    config.fetch.browser.executable = Some("/synthetic/missing/chrome".to_string());
    let state = Arc::new(ServerState::build(config).expect("state builds"));

    let batch = super::run_batch_fetch(
        state.clone(),
        serde_json::from_value(serde_json::json!({
            "items": [{
                "type": "web",
                "url": "https://example.test/batch-browser",
                "render": "browser"
            }]
        }))
        .expect("batch arguments deserialize"),
    )
    .await
    .expect("batch preserves the item failure");
    let batch_error = batch["results"][0]["error"]
        .as_str()
        .expect("batch error string");
    assert!(
        batch_error.starts_with("browser_unavailable:"),
        "{batch_error}"
    );
    assert!(
        batch_error.contains("/synthetic/missing/chrome"),
        "{batch_error}"
    );

    let unified = super::run_fetch(
        state,
        serde_json::from_value(serde_json::json!({
            "items": {
                "type": "web",
                "url": "https://example.test/unified-browser",
                "render": "browser"
            }
        }))
        .expect("unified fetch arguments deserialize"),
    )
    .await
    .expect("unified fetch preserves the item failure");
    let unified_error = unified["results"][0]["error"]
        .as_str()
        .expect("unified fetch error string");
    assert!(
        unified_error.starts_with("browser_unavailable:"),
        "{unified_error}"
    );
    assert!(
        unified_error.contains("/synthetic/missing/chrome"),
        "{unified_error}"
    );
}

#[cfg(all(feature = "mock", feature = "browser"))]
#[tokio::test]
async fn browser_launch_failure_is_named_and_does_not_repeat_http() {
    use httpmock::prelude::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/verification");
        then.status(200)
            .header("content-type", "text/html")
            .body("<html><head><title>Checking your browser</title></head><body>Please wait while we verify.</body></html>");
    });
    let browser_calls = Arc::new(AtomicUsize::new(0));
    let seen = browser_calls.clone();
    let state = synthetic_available_browser_state(Arc::new(
        move |_request: crate::mcp::state::MockBrowserRequest| {
            seen.fetch_add(1, Ordering::SeqCst);
            Err(crate::fetch::browser::BrowserFetchError::LaunchFailed(
                "container sandbox rejected Chrome launch".to_string(),
            ))
        },
    ));
    let response = run_web_fetch(
        state.clone(),
        serde_json::from_value(serde_json::json!({
            "url": server.url("/verification"),
            "render": "auto"
        }))
        .unwrap(),
    )
    .await
    .expect("auto render failure retains the HTTP document");
    assert!(response["structured_warnings"]
        .to_string()
        .contains("browser_unavailable"));
    assert!(response["structured_warnings"]
        .to_string()
        .contains("container sandbox rejected Chrome launch"));
    assert_eq!(browser_calls.load(Ordering::SeqCst), 1);
    mock.assert_calls(1);

    let explicit = run_web_fetch(
        state,
        serde_json::from_value(serde_json::json!({
            "url": "https://example.test/explicit-launch-failure",
            "render": "browser"
        }))
        .unwrap(),
    )
    .await
    .expect_err("explicit launch failure must remain a structured browser error");
    assert_eq!(
        explicit.error_payload()["data"]["code"],
        "browser_unavailable"
    );
    assert!(explicit
        .error_payload()
        .to_string()
        .contains("browser launch failed: container sandbox rejected Chrome launch"));
    assert_eq!(browser_calls.load(Ordering::SeqCst), 2);
    mock.assert_calls(1);
}
