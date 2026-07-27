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
