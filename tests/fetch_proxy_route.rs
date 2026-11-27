use eggsearch::core::batch_fetch::BatchFetchItem;
use eggsearch::core::config::{AppConfig, FetchProxyMode};
use eggsearch::core::fetch::FetchCachePolicy;
use eggsearch::core::workflow::RecipeDetail;
use eggsearch::mcp::projection::ResponseDetail;
use eggsearch::mcp::state::ServerState;
use eggsearch::mcp::tools::{
    run_batch_fetch, run_provider_status, run_web_fetch, BatchFetchArgs, ProviderStatusArgs,
    WebFetchArgs,
};
use httpmock::prelude::*;
use std::sync::Arc;

fn proxy_state(
    route: FetchProxyMode,
    proxy_url: String,
    no_proxy: Vec<String>,
) -> Arc<ServerState> {
    let mut config = AppConfig::default();
    config.fetch.allow_localhost = true;
    config.fetch.allow_private_network = true;
    config.fetch.browser.enabled = false;
    config.fetch.proxy.route = route;
    config.fetch.proxy.url = Some(proxy_url);
    config.fetch.proxy.no_proxy = no_proxy;
    Arc::new(ServerState::build(config).expect("proxy state builds"))
}

fn web_item(url: String) -> BatchFetchItem {
    BatchFetchItem::Web {
        url,
        extract_mode: None,
        include_links: None,
        include_selectors: None,
        exclude_selectors: None,
        attributes: None,
        max_chars: None,
        cache_policy: None,
        max_cache_age_seconds: None,
        focus: None,
        focus_max_chunks: None,
        focus_max_chars: None,
        pdf: None,
        render: None,
        browser_profile: None,
        snapshot: None,
        include_screenshot: None,
    }
}

fn payload_error_code(error: &eggsearch::mcp::tools::ToolError) -> serde_json::Value {
    error.error_payload()["data"]["code"].clone()
}

#[tokio::test]
async fn proxy_no_proxy_route_is_reported_for_fetch_and_batch_items() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/page");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body("<html><head><title>Route</title></head><body><p>Route test content.</p></body></html>");
    });
    let state = proxy_state(
        FetchProxyMode::Proxy,
        "http://127.0.0.1:1".into(),
        vec!["127.0.0.1".into()],
    );
    let url = server.url("/page");
    let fetched = run_web_fetch(
        state.clone(),
        WebFetchArgs {
            include_selectors: None,
            exclude_selectors: None,
            attributes: None,
            url: url.clone(),
            response_detail: Some(ResponseDetail::Diagnostic),
            ..Default::default()
        },
    )
    .await
    .expect("no_proxy fetch stays direct");
    assert_eq!(fetched["route"]["kind"], "direct");
    assert_eq!(fetched["route"]["reason"], "no_proxy");
    assert!(fetched["route"]["scheme"].is_null());

    let batch = run_batch_fetch(
        state,
        BatchFetchArgs {
            items: vec![web_item(url)],
            response_detail: Some(ResponseDetail::Diagnostic),
            ..Default::default()
        },
    )
    .await
    .expect("batch web item stays direct under no_proxy");
    assert_eq!(batch["results"][0]["response"]["route"]["kind"], "direct");
    assert_eq!(
        batch["results"][0]["response"]["route"]["reason"],
        "no_proxy"
    );
    assert_eq!(mock.hits(), 1);
}

#[tokio::test]
async fn direct_route_omits_route_metadata() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/direct");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body("<html><body>Direct route.</body></html>");
    });
    let mut config = AppConfig::default();
    config.fetch.allow_localhost = true;
    config.fetch.allow_private_network = true;
    config.fetch.browser.enabled = false;
    let state = Arc::new(ServerState::build(config).expect("direct state builds"));
    let result = run_web_fetch(
        state,
        WebFetchArgs {
            include_selectors: None,
            exclude_selectors: None,
            attributes: None,
            url: server.url("/direct"),
            response_detail: Some(ResponseDetail::Diagnostic),
            ..Default::default()
        },
    )
    .await
    .expect("direct fetch succeeds");
    assert!(result.get("route").is_none());
}

#[tokio::test]
async fn proxy_required_http_targets_fail_before_any_direct_request() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET);
        then.status(200).body("unexpected direct request");
    });
    let state = proxy_state(
        FetchProxyMode::ProxyRequired,
        "http://127.0.0.1:1".into(),
        Vec::new(),
    );
    let error = run_web_fetch(
        state,
        WebFetchArgs {
            include_selectors: None,
            exclude_selectors: None,
            attributes: None,
            url: server.url("/plaintext"),
            ..Default::default()
        },
    )
    .await
    .expect_err("plaintext target cannot carry the pin through a forward proxy");
    assert_eq!(payload_error_code(&error), "proxy_target_unsupported");
    assert_eq!(mock.hits(), 0);
}

#[tokio::test]
async fn unavailable_proxy_fails_closed_with_a_typed_error() {
    let proxy = std::net::TcpListener::bind("127.0.0.1:0").expect("proxy reservation");
    let proxy_address = proxy.local_addr().expect("proxy address");
    drop(proxy);
    let target = std::net::TcpListener::bind("127.0.0.1:0").expect("target reservation");
    let target_port = target.local_addr().expect("target address").port();
    drop(target);
    let state = proxy_state(
        FetchProxyMode::ProxyRequired,
        format!("http://{proxy_address}"),
        Vec::new(),
    );
    let error = run_web_fetch(
        state,
        WebFetchArgs {
            include_selectors: None,
            exclude_selectors: None,
            attributes: None,
            url: format!("https://127.0.0.1:{target_port}/"),
            ..Default::default()
        },
    )
    .await
    .expect_err("unavailable proxy must not fall back to the origin");
    assert_eq!(payload_error_code(&error), "proxy_connect_failed");
}

#[tokio::test]
async fn missing_proxy_credentials_fail_before_target_lookup() {
    let username_env = format!("EGGSEARCH_PROXY_MISSING_USER_{}", std::process::id());
    let password_env = format!("EGGSEARCH_PROXY_MISSING_PASSWORD_{}", std::process::id());
    std::env::remove_var(&username_env);
    std::env::remove_var(&password_env);
    let mut config = AppConfig::default();
    config.fetch.proxy.route = FetchProxyMode::ProxyRequired;
    config.fetch.proxy.url = Some("http://127.0.0.1:1".into());
    config.fetch.proxy.username_env = Some(username_env);
    config.fetch.proxy.password_env = Some(password_env);
    let state = Arc::new(ServerState::build(config).expect("proxy state builds"));
    let error = run_web_fetch(
        state,
        WebFetchArgs {
            include_selectors: None,
            exclude_selectors: None,
            attributes: None,
            url: "https://example.com/".into(),
            ..Default::default()
        },
    )
    .await
    .expect_err("missing credentials fail before target lookup");
    assert_eq!(
        payload_error_code(&error),
        "proxy_credentials_missing",
        "payload: {}",
        error.error_payload()
    );
}

#[tokio::test]
async fn cache_only_under_proxy_route_does_not_access_the_network() {
    let state = proxy_state(
        FetchProxyMode::ProxyRequired,
        "http://127.0.0.1:1".into(),
        Vec::new(),
    );
    let error = run_web_fetch(
        state,
        WebFetchArgs {
            include_selectors: None,
            exclude_selectors: None,
            attributes: None,
            url: "https://cache-only.invalid/".into(),
            cache_policy: Some(FetchCachePolicy::CacheOnly),
            ..Default::default()
        },
    )
    .await
    .expect_err("an empty route-specific cache must miss without network access");
    assert_eq!(payload_error_code(&error), "cache_only_miss");
}

#[test]
fn provider_status_reports_proxy_summary_without_environment_names() {
    let username_env = format!("EGGSEARCH_PROXY_DIAGNOSTICS_USER_{}", std::process::id());
    let password_env = format!(
        "EGGSEARCH_PROXY_DIAGNOSTICS_PASSWORD_{}",
        std::process::id()
    );
    let username = "proxy-diagnostic-user-secret";
    let password = "proxy-diagnostic-password-secret";
    std::env::remove_var(&username_env);
    std::env::remove_var(&password_env);
    let mut config = AppConfig::default();
    config.fetch.proxy.route = FetchProxyMode::ProxyRequired;
    config.fetch.proxy.url = Some("socks5://proxy.example:1080".into());
    config.fetch.proxy.username_env = Some(username_env.clone());
    config.fetch.proxy.password_env = Some(password_env.clone());
    let state = Arc::new(ServerState::build(config).expect("diagnostics state builds"));
    let missing = run_provider_status(
        state.clone(),
        ProviderStatusArgs {
            probe: false,
            recipe_detail: Some(RecipeDetail::None),
            response_detail: None,
        },
    )
    .expect("provider status succeeds without credentials");
    assert_eq!(missing["fetch_proxy"]["credentials_present"], false);
    assert_eq!(
        missing["fetch_proxy"]["validation_state"],
        "credentials_missing"
    );

    std::env::set_var(&username_env, username);
    std::env::set_var(&password_env, password);
    let status = run_provider_status(
        state,
        ProviderStatusArgs {
            probe: false,
            recipe_detail: Some(RecipeDetail::None),
            response_detail: None,
        },
    )
    .expect("provider status succeeds with credentials");
    assert_eq!(status["fetch_proxy"]["route_class"], "proxy_required");
    assert_eq!(status["fetch_proxy"]["scheme"], "socks5");
    assert_eq!(status["fetch_proxy"]["host"], "proxy.example");
    assert_eq!(status["fetch_proxy"]["port"], 1080);
    assert_eq!(status["fetch_proxy"]["credentials_present"], true);
    assert_eq!(status["fetch_proxy"]["validation_state"], "valid");
    let serialized = serde_json::to_string(&status).expect("status serializes");
    assert!(!serialized.contains(&username_env));
    assert!(!serialized.contains(&password_env));
    assert!(!serialized.contains(username));
    assert!(!serialized.contains(password));
    std::env::remove_var(&username_env);
    std::env::remove_var(&password_env);
}
