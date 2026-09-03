//! Keyless-core runtime contract tests.
//!
//! Verifies that a clean installation with no configuration file and no
//! provider credential environment variables starts successfully and
//! provides a useful keyless MCP search/fetch service.

use std::sync::Arc;

use eggsearch::core::config::AppConfig;
use eggsearch::core::provider::API_PROVIDER_IDS;
use eggsearch::core::repo_search::SearchProfile;
use eggsearch::mcp::state::ServerState;

#[cfg(feature = "mock")]
use eggsearch::meta::mock::{mock_engines, MockEngine, MockResult};
#[cfg(feature = "mock")]
use eggsearch::meta::MetadataSearchAdapter;

fn scrubbed_config() -> AppConfig {
    let mut cfg = AppConfig::default();
    cfg.search.api.clear();
    cfg
}

#[test]
fn e1_no_config_no_keys_state_builds() {
    let cfg = scrubbed_config();
    let state = ServerState::build(cfg);
    assert!(state.is_ok(), "state build failed: {:?}", state.err());
}

#[test]
fn e2_no_config_no_keys_default_providers_are_keyless() {
    let cfg = scrubbed_config();
    let state = ServerState::build(cfg).unwrap();
    let status = state.adapter.provider_status();
    for desc in &status {
        if desc.default && desc.enabled {
            assert!(
                !desc.requires_api_key,
                "default provider {} requires API key",
                desc.id
            );
        }
    }
}

#[test]
fn e3_no_keys_provider_status_succeeds() {
    let cfg = scrubbed_config();
    let state = Arc::new(ServerState::build(cfg).unwrap());
    let result = eggsearch::mcp::tools::run_provider_status(
        state.clone(),
        eggsearch::mcp::tools::ProviderStatusArgs {
            probe: false,
            recipe_detail: None,
        },
    );
    assert!(result.is_ok(), "provider_status failed: {:?}", result.err());
}

#[test]
fn e4_no_keys_credentialed_providers_non_routable() {
    let cfg = scrubbed_config();
    let state = ServerState::build(cfg).unwrap();
    let status = state.adapter.provider_status();
    for desc in &status {
        if desc.requires_api_key && API_PROVIDER_IDS.contains(&desc.id.as_str()) {
            assert!(
                !desc.routable,
                "credentialed provider {} should not be routable without key",
                desc.id
            );
        }
    }
}

#[test]
fn e5_coding_profile_has_keyless_path() {
    let cfg = scrubbed_config();
    let state = ServerState::build(cfg).unwrap();
    let (providers, degraded, _warnings) = state
        .config
        .resolve_profile_providers(Some(SearchProfile::Coding), &[]);
    assert!(
        !providers.is_empty(),
        "coding profile should have keyless providers, degraded={degraded}"
    );
}

#[test]
fn e6_security_profile_has_keyless_path() {
    let cfg = scrubbed_config();
    let state = ServerState::build(cfg).unwrap();
    let (providers, degraded, _warnings) = state
        .config
        .resolve_profile_providers(Some(SearchProfile::Security), &[]);
    assert!(
        !providers.is_empty(),
        "security profile should have keyless providers, degraded={degraded}"
    );
}

#[test]
fn e7_research_profile_has_keyless_path() {
    let cfg = scrubbed_config();
    let state = ServerState::build(cfg).unwrap();
    let (providers, degraded, _warnings) = state
        .config
        .resolve_profile_providers(Some(SearchProfile::Research), &[]);
    assert!(
        !providers.is_empty(),
        "research profile should have keyless providers, degraded={degraded}"
    );
}

#[test]
fn e8_missing_github_token_does_not_fail_startup() {
    let cfg = scrubbed_config();
    let result = ServerState::build(cfg);
    assert!(result.is_ok(), "startup failed: {:?}", result.err());
}

#[test]
fn e9_no_credential_values_in_serialized_status() {
    let cfg = scrubbed_config();
    let state = Arc::new(ServerState::build(cfg).unwrap());
    let result = eggsearch::mcp::tools::run_provider_status(
        state.clone(),
        eggsearch::mcp::tools::ProviderStatusArgs {
            probe: false,
            recipe_detail: None,
        },
    )
    .unwrap();
    let json = serde_json::to_string(&result).unwrap();
    let lower = json.to_lowercase();
    assert!(
        !lower.contains("ghp_") && !lower.contains("gho_"),
        "GitHub token leaked in status"
    );
    assert!(!lower.contains("glpat-"), "GitLab token leaked in status");
    assert!(
        !lower.contains("brave_api_key"),
        "Brave API key leaked in status"
    );
    assert!(
        !lower.contains("sourcegraph_api_key"),
        "Sourcegraph key leaked in status"
    );
}

#[test]
fn e10_server_health_independent_of_optional_adapters() {
    let cfg = scrubbed_config();
    let state = ServerState::build(cfg).unwrap();
    let status = state.adapter.provider_status();
    let keyless_count = status
        .iter()
        .filter(|d| d.routable && !d.requires_api_key)
        .count();
    assert!(
        keyless_count > 0,
        "should have at least one routable keyless provider"
    );
}

#[test]
fn e11_default_provider_ids_are_all_known() {
    let cfg = scrubbed_config();
    let state = ServerState::build(cfg).unwrap();
    let ids = state.adapter.provider_ids();
    for id in ids {
        assert!(
            eggsearch::core::provider::KNOWN_PROVIDER_IDS.contains(&id.as_str()),
            "unknown provider id: {id}"
        );
    }
}

#[test]
fn e12_keyless_default_providers_are_html_scrape() {
    use eggsearch::core::provider::ProviderKind;
    let cfg = scrubbed_config();
    let state = ServerState::build(cfg).unwrap();
    let status = state.adapter.provider_status();
    for desc in &status {
        if desc.default && desc.enabled {
            assert_eq!(
                desc.kind,
                ProviderKind::HtmlScrape,
                "default provider {} should be HtmlScrape (keyless)",
                desc.id
            );
        }
    }
}

#[cfg(feature = "mock")]
fn mock_state(engines: Vec<MockEngine>) -> Arc<ServerState> {
    let cfg = scrubbed_config();
    let adapter = MetadataSearchAdapter::from_engines(
        mock_engines(engines),
        std::time::Duration::from_secs(5),
    );
    Arc::new(ServerState::with_adapter(cfg, Arc::new(adapter)))
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn e13_keyless_web_search_dispatches_through_mock_engines() {
    use eggsearch::mcp::tools::{run_web_search, WebSearchArgs};
    let engines = vec![MockEngine::success(
        "duckduckgo",
        vec![MockResult::new(
            "Test Result",
            "https://example.com/test",
            "duckduckgo",
        )],
    )];
    let state = mock_state(engines);
    let result = run_web_search(
        state,
        WebSearchArgs {
            query: "test query".into(),
            max_results: None,
            providers: vec!["duckduckgo".into()],
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
        },
    )
    .await;
    assert!(
        result.is_ok(),
        "keyless web search failed: {:?}",
        result.err()
    );
    let v = result.unwrap();
    let results = v["results"].as_array().expect("results array");
    assert!(
        !results.is_empty(),
        "keyless web search should return results"
    );
}

#[test]
fn e14_no_keys_missing_gitlab_token_does_not_fail_startup() {
    let cfg = scrubbed_config();
    let result = ServerState::build(cfg);
    assert!(
        result.is_ok(),
        "startup failed with missing GITLAB_TOKEN: {:?}",
        result.err()
    );
}

#[test]
fn e15_no_keys_missing_gitea_token_does_not_fail_startup() {
    let cfg = scrubbed_config();
    let result = ServerState::build(cfg);
    assert!(
        result.is_ok(),
        "startup failed with missing GITEA/FORGEJO_TOKEN: {:?}",
        result.err()
    );
}

#[test]
fn e16_no_keys_missing_sourcegraph_key_does_not_fail_startup() {
    let cfg = scrubbed_config();
    let result = ServerState::build(cfg);
    assert!(
        result.is_ok(),
        "startup failed with missing SOURCEGRAPH_API_KEY: {:?}",
        result.err()
    );
}

#[test]
fn e17_no_keys_missing_semantic_scholar_key_does_not_fail_startup() {
    let cfg = scrubbed_config();
    let result = ServerState::build(cfg);
    assert!(
        result.is_ok(),
        "startup failed with missing SEMANTIC_SCHOLAR_API_KEY: {:?}",
        result.err()
    );
}

#[test]
fn e18_no_keys_missing_brave_key_does_not_fail_startup() {
    let cfg = scrubbed_config();
    let result = ServerState::build(cfg);
    assert!(
        result.is_ok(),
        "startup failed with missing BRAVE_API_KEY: {:?}",
        result.err()
    );
}

#[test]
fn e19_searxng_absence_does_not_fail_startup() {
    let cfg = scrubbed_config();
    let state = ServerState::build(cfg).unwrap();
    assert!(
        !state.adapter.searxng_configured(),
        "SearXNG should not be configured in scrubbed environment"
    );
}

#[test]
fn e20_local_workspace_disabled_does_not_fail_startup() {
    let mut cfg = scrubbed_config();
    cfg.local.enabled = false;
    let result = ServerState::build(cfg);
    assert!(
        result.is_ok(),
        "startup failed with local workspace disabled: {:?}",
        result.err()
    );
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn e21_keyless_web_fetch_succeeds_without_credentials() {
    use eggsearch::mcp::tools::{run_web_fetch, WebFetchArgs};
    let server = httpmock::MockServer::start();
    server.mock(|when, then| {
        when.path("/get");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body("<html><body><p>keyless fetch ok</p></body></html>");
    });
    let engines: Vec<MockEngine> = vec![];
    let mut cfg = scrubbed_config();
    cfg.fetch.allow_localhost = true;
    cfg.fetch.allow_private_network = true;
    let adapter = MetadataSearchAdapter::from_engines(
        mock_engines(engines),
        std::time::Duration::from_secs(5),
    );
    let state = Arc::new(ServerState::with_adapter(cfg, Arc::new(adapter)));
    let result = run_web_fetch(
        state,
        WebFetchArgs {
            url: server.url("/get"),
            max_chars: None,
            timeout_ms: None,
            extract_mode: None,
            include_links: None,
            pdf: None,
            cache_policy: None,
            render: None,
            browser_profile: None,
            max_cache_age_seconds: None,
            focus: None,
            focus_max_chunks: None,
            focus_max_chars: None,
        },
    )
    .await;
    let v = result.expect("web_fetch should succeed against a loopback URL without credentials");
    assert_eq!(v["status"], 200);
    let text = v["text"].as_str().expect("text should be a string");
    assert!(
        text.contains("keyless fetch ok"),
        "fetched body should contain served content: {text}"
    );
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn e22_mixed_providers_keyless_result_survives_credentialed_skip() {
    use eggsearch::mcp::tools::{run_web_search, WebSearchArgs};
    let engines = vec![MockEngine::success(
        "duckduckgo",
        vec![MockResult::new(
            "Keyless Hit",
            "https://example.com/keyless",
            "duckduckgo",
        )],
    )];
    let state = mock_state(engines);
    let result = run_web_search(
        state.clone(),
        WebSearchArgs {
            query: "test".into(),
            max_results: None,
            providers: vec!["duckduckgo".into()],
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
        },
    )
    .await;
    assert!(
        result.is_ok(),
        "keyless-only request failed: {:?}",
        result.err()
    );
    let v = result.unwrap();
    let results = v["results"].as_array().expect("results array");
    assert!(!results.is_empty(), "keyless results should be present");
    let status = state.adapter.provider_status();
    let github = status.iter().find(|d| d.id == "github_code");
    if let Some(g) = github {
        assert!(
            !g.routable,
            "github_code should be non-routable without credentials"
        );
    }
}

#[test]
fn e23_provider_status_distinguishes_server_health_from_adapter_availability() {
    let cfg = scrubbed_config();
    let state = ServerState::build(cfg).unwrap();
    let status = state.adapter.provider_status();
    let keyless_routable = status
        .iter()
        .filter(|d| d.routable && !d.requires_api_key)
        .count();
    let credentialed_non_routable = status
        .iter()
        .filter(|d| !d.routable && d.requires_api_key)
        .count();
    assert!(
        keyless_routable > 0,
        "server should have routable keyless providers for baseline health"
    );
    assert!(
        credentialed_non_routable > 0 || !status.iter().any(|d| d.requires_api_key),
        "credentialed providers should be non-routable, or none should require keys"
    );
}
