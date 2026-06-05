//! Tests for the config-driven provider registry.

use eggsearch_core::config::{AppConfig, ProviderConfig};
use eggsearch_meta::registry::DiagnosticStatus;
use eggsearch_meta::ProviderRegistry;
use std::collections::BTreeMap;

fn providers_map() -> BTreeMap<String, ProviderConfig> {
    let mut m = BTreeMap::new();
    m.insert("duckduckgo_html".into(), ProviderConfig { enabled: true, ..Default::default() });
    m.insert("wikipedia".into(), ProviderConfig { enabled: true, ..Default::default() });
    m.insert("crates_io".into(), ProviderConfig { enabled: true, ..Default::default() });
    m.insert("docs_rs".into(), ProviderConfig { enabled: true, ..Default::default() });
    m.insert(
        "searxng".into(),
        ProviderConfig {
            enabled: true,
            base_url: Some("http://127.0.0.1:8080".into()),
            ..Default::default()
        },
    );
    m.insert(
        "brave".into(),
        ProviderConfig {
            enabled: true,
            api_key_env: Some("EGGSEARCH_TEST_BRAVE_DISABLED".into()),
            ..Default::default()
        },
    );
    m.insert(
        "tavily".into(),
        ProviderConfig {
            enabled: false,
            ..Default::default()
        },
    );
    m.insert("exa".into(), ProviderConfig { enabled: false, ..Default::default() });
    m
}

#[test]
fn from_config_registers_enabled_and_skips_disabled() {
    let cfg = AppConfig { search: eggsearch_core::config::SearchSection { providers: providers_map(), ..Default::default() } };
    let (reg, diag) = ProviderRegistry::from_config(&cfg, false);

    // The four MVP providers plus SearXNG should load.
    let ids: Vec<&str> = reg.ids().into_iter().collect();
    for required in ["duckduckgo_html", "wikipedia", "crates_io", "docs_rs", "searxng"] {
        assert!(ids.contains(&required), "expected {required} in registry, got {ids:?}");
    }
    // Brave should be misconfigured (env var unset) and skipped from registry.
    assert!(!ids.contains(&"brave"), "brave should be skipped when env var missing");
    // Tavily/Exa disabled, not registered.
    assert!(!ids.contains(&"tavily"));
    assert!(!ids.contains(&"exa"));

    // Diagnostics: 8 providers total.
    assert_eq!(diag.diagnostics.len(), 8);
    let brave = diag.diagnostics.iter().find(|d| d.id == "brave").unwrap();
    assert_eq!(brave.status, DiagnosticStatus::Misconfigured);
    assert!(brave.message.as_deref().unwrap().contains("EGGSEARCH_TEST_BRAVE_DISABLED"));
    let tavily = diag.diagnostics.iter().find(|d| d.id == "tavily").unwrap();
    assert_eq!(tavily.status, DiagnosticStatus::Disabled);
    let searxng = diag.diagnostics.iter().find(|d| d.id == "searxng").unwrap();
    assert_eq!(searxng.status, DiagnosticStatus::Loaded);
    assert!(!diag.healthy(), "registry with misconfigured brave should not be healthy");
}

#[test]
fn from_config_searxng_missing_base_url_is_misconfigured() {
    let mut m = BTreeMap::new();
    m.insert(
        "searxng".into(),
        ProviderConfig { enabled: true, ..Default::default() },
    );
    let cfg = AppConfig { search: eggsearch_core::config::SearchSection { providers: m, ..Default::default() } };
    let (reg, diag) = ProviderRegistry::from_config(&cfg, false);
    assert!(!reg.ids().contains(&"searxng"));
    let searxng = diag.diagnostics.iter().find(|d| d.id == "searxng").unwrap();
    assert_eq!(searxng.status, DiagnosticStatus::Misconfigured);
    let msg = searxng.message.as_deref().unwrap();
    assert!(msg.contains("base_url"), "expected base_url mention, got: {msg}");
}

#[test]
fn from_config_unknown_provider_id_is_misconfigured() {
    let mut m = BTreeMap::new();
    m.insert("nonesuch".into(), ProviderConfig { enabled: true, ..Default::default() });
    let cfg = AppConfig { search: eggsearch_core::config::SearchSection { providers: m, ..Default::default() } };
    let (_reg, diag) = ProviderRegistry::from_config(&cfg, false);
    let entry = diag.diagnostics.iter().find(|d| d.id == "nonesuch").unwrap();
    assert_eq!(entry.status, DiagnosticStatus::Misconfigured);
    assert!(entry.message.as_deref().unwrap().contains("unknown provider"));
}

#[test]
fn from_config_include_mock_adds_mock() {
    let cfg = AppConfig::default();
    let (reg_with, _) = ProviderRegistry::from_config(&cfg, true);
    let (reg_without, _) = ProviderRegistry::from_config(&cfg, false);
    assert!(reg_with.ids().contains(&"mock"));
    assert!(!reg_without.ids().contains(&"mock"));
}

#[test]
fn from_config_with_brave_key_loads_brave() {
    std::env::set_var("EGGSEARCH_TEST_BRAVE_OK", "abc1234567");
    let mut m = BTreeMap::new();
    m.insert(
        "brave".into(),
        ProviderConfig {
            enabled: true,
            api_key_env: Some("EGGSEARCH_TEST_BRAVE_OK".into()),
            ..Default::default()
        },
    );
    let cfg = AppConfig { search: eggsearch_core::config::SearchSection { providers: m, ..Default::default() } };
    let (reg, diag) = ProviderRegistry::from_config(&cfg, false);
    assert!(reg.ids().contains(&"brave"));
    let brave = diag.diagnostics.iter().find(|d| d.id == "brave").unwrap();
    assert_eq!(brave.status, DiagnosticStatus::Loaded);
    std::env::remove_var("EGGSEARCH_TEST_BRAVE_OK");
}
