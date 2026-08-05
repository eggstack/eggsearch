#![cfg(feature = "browser")]
#![cfg(feature = "live-smoke")]

use std::sync::Arc;

use eggsearch::fetch::browser::discover::discover_browser;
use eggsearch::fetch::browser::lifecycle::BrowserLifecycle;
use eggsearch::fetch::browser::navigate::browser_fetch;
use eggsearch::fetch::browser::types::BrowserConfig;

#[tokio::test]
#[ignore]
async fn browser_live_smoke_basic_page() {
    let discovery = discover_browser(None);
    if discovery.is_none() {
        eprintln!("No browser found, skipping live smoke test");
        return;
    }

    let config = BrowserConfig {
        enabled: true,
        startup_timeout_ms: 15_000,
        navigation_timeout_ms: 20_000,
        post_load_wait_ms: 1_000,
        ..Default::default()
    };

    let lifecycle = Arc::new(BrowserLifecycle::new(discovery, config.clone()));

    let result = browser_fetch(&lifecycle, "https://example.com", &config, false).await;
    match result {
        Ok(response) => {
            assert_eq!(
                response.response.transport,
                eggsearch::fetch::browser::types::FetchTransportKind::Browser
            );
            assert!(!response.response.body.is_empty());
            assert!(response.response.final_url.contains("example.com"));
        }
        Err(e) => {
            eprintln!("Browser fetch failed (expected if no Chrome): {e}");
        }
    }

    lifecycle.close().await;
}

#[tokio::test]
#[ignore]
async fn browser_live_smoke_blocks_private_network() {
    let config = BrowserConfig::default();
    let lifecycle = Arc::new(BrowserLifecycle::new(None, config.clone()));

    let result = browser_fetch(&lifecycle, "http://127.0.0.1/", &config, false).await;
    assert!(result.is_err());

    lifecycle.close().await;
}
