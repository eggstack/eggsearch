use super::*;

#[test]
fn mode_parsing() {
    assert_eq!(Mode::parse("off").unwrap(), Mode::Off);
    assert_eq!(Mode::parse("live").unwrap(), Mode::Live);
    assert!(Mode::parse("nope").is_err());
}

#[test]
fn mode_parsing_rejects_documented_aliases() {
    // The previous build accepted "ask", "local_only", "localonly",
    // and "local" as aliases for Live. The current build is strict
    // and only accepts "off" and "live".
    for alias in ["ask", "local_only", "localonly", "local"] {
        assert!(
            Mode::parse(alias).is_err(),
            "{alias} should be rejected, was accepted as a Live alias"
        );
    }
}

#[test]
fn default_config_loads() {
    let c = AppConfig::default();
    assert!(c.search.default_max_results > 0);
    assert!(!c.search.default_providers.is_empty());
}

#[test]
fn mcp_excerpt_budget_defaults_agree_and_validate() {
    assert_eq!(
        McpSection::default().excerpt_budget_chars,
        DEFAULT_EXCERPT_BUDGET_CHARS
    );
    let parsed: AppConfig = toml::from_str("[mcp]\nbrief_max_chars = 8000\n").expect("parse");
    assert_eq!(
        parsed.mcp.excerpt_budget_chars,
        DEFAULT_EXCERPT_BUDGET_CHARS
    );
    for value in [MIN_EXCERPT_BUDGET_CHARS, MAX_EXCERPT_BUDGET_CHARS] {
        let mut c = AppConfig::default();
        c.mcp.excerpt_budget_chars = value;
        assert!(c.validate().is_ok(), "{value}");
    }
    for value in [MIN_EXCERPT_BUDGET_CHARS - 1, MAX_EXCERPT_BUDGET_CHARS + 1] {
        let mut c = AppConfig::default();
        c.mcp.excerpt_budget_chars = value;
        let err = c.validate().expect_err("expected excerpt budget failure");
        assert!(
            err.to_string().contains("excerpt_budget_chars"),
            "got: {err}"
        );
    }
}

#[test]
fn mcp_excerpt_budget_is_independent_of_brief_floor() {
    let mut c = AppConfig::default();
    c.mcp.brief_max_chars = 512;
    assert!(c.validate().is_ok());
    assert_eq!(c.mcp.excerpt_budget_chars, DEFAULT_EXCERPT_BUDGET_CHARS);
}

#[test]
fn default_providers_lists_known_engines() {
    let c = AppConfig::default();
    for expected in [
        "duckduckgo",
        "brave",
        "startpage",
        "yahoo",
        "mojeek",
        "searxng",
    ] {
        assert!(
            c.search.providers.contains_key(expected),
            "missing default provider: {expected}"
        );
    }
}

#[test]
fn default_searxng_is_disabled() {
    let c = AppConfig::default();
    assert!(!c.search.searxng.enabled);
    assert!(c.search.searxng.base_url.is_none());
    assert_eq!(c.search.providers.get("searxng"), Some(&false));
}

#[test]
fn searxng_provider_is_available_requires_enabled_and_base_url() {
    let mut c = AppConfig::default();
    c.search.providers.insert("searxng".to_string(), true);
    assert!(!c.provider_is_available("searxng"));

    c.search.searxng.enabled = true;
    assert!(!c.provider_is_available("searxng"));

    c.search.searxng.base_url = Some(String::new());
    assert!(!c.provider_is_available("searxng"));

    c.search.searxng.base_url = Some("https://searx.example.org".to_string());
    assert!(c.provider_is_available("searxng"));
}

#[test]
fn default_mojeek_is_disabled() {
    let c = AppConfig::default();
    assert_eq!(c.search.providers.get("mojeek"), Some(&false));
}

#[test]
fn round_trip_toml() {
    let c = AppConfig::default();
    let text = toml::to_string(&c).unwrap();
    let parsed: AppConfig = toml::from_str(&text).unwrap();
    assert_eq!(
        parsed.search.default_max_results,
        c.search.default_max_results
    );
}

#[test]
fn resolve_providers_uses_default_when_empty() {
    let c = AppConfig::default();
    let out = c.resolve_providers(&[]).unwrap();
    assert_eq!(out, c.search.default_providers);
}

#[test]
fn resolve_providers_dedupes_override() {
    let c = AppConfig::default();
    let out = c
        .resolve_providers(&["brave".into(), "brave".into(), "duckduckgo".into()])
        .unwrap();
    assert_eq!(out, vec!["brave".to_string(), "duckduckgo".to_string()]);
}

#[test]
fn resolve_providers_filters_to_enabled() {
    let mut c = AppConfig::default();
    c.search.providers.insert("duckduckgo".to_string(), true);
    c.search.providers.insert("brave".to_string(), false);
    c.search.default_providers = vec!["duckduckgo".to_string(), "brave".to_string()];

    let out = c.resolve_providers(&[]).unwrap();
    assert_eq!(out, vec!["duckduckgo".to_string()]);
}

#[test]
fn resolve_providers_rejects_disabled_in_explicit_list() {
    let mut c = AppConfig::default();
    c.search.providers.insert("duckduckgo".to_string(), true);
    c.search.providers.insert("brave".to_string(), false);

    let result = c.resolve_providers(&["brave".to_string()]);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("disabled"));
}

#[test]
fn resolve_providers_empty_when_all_disabled() {
    let mut c = AppConfig::default();
    let keys: Vec<_> = c.search.providers.keys().cloned().collect();
    for key in keys {
        c.search.providers.insert(key, false);
    }

    let result = c.resolve_providers(&[]);
    assert!(result.is_err());
}

#[test]
fn resolve_providers_preserves_order() {
    let c = AppConfig::default();
    let out = c
        .resolve_providers(&["yahoo".into(), "duckduckgo".into()])
        .unwrap();
    assert_eq!(out, vec!["yahoo".to_string(), "duckduckgo".to_string()]);
}

#[test]
fn resolve_providers_dedups() {
    let c = AppConfig::default();
    let out = c
        .resolve_providers(&["brave".into(), "brave".into(), "brave".into()])
        .unwrap();
    assert_eq!(out, vec!["brave".to_string()]);
}

#[test]
fn resolve_providers_validates_enabled() {
    let mut c = AppConfig::default();
    c.search.providers.insert("duckduckgo".to_string(), true);
    c.search.providers.insert("brave".to_string(), false);

    let out = c.resolve_providers(&["brave".to_string()]);
    assert!(out.is_err());
    assert!(out.unwrap_err().to_string().contains("disabled"));
}

#[test]
fn save_load_round_trip_through_filesystem() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let c = AppConfig::default();
    c.save(&path).unwrap();
    let loaded = AppConfig::load(&path).unwrap();
    assert_eq!(
        loaded.search.default_max_results,
        c.search.default_max_results
    );
    assert_eq!(loaded.search.mode, c.search.mode);
    assert_eq!(loaded.search.default_providers, c.search.default_providers);
}

#[test]
fn load_malformed_toml_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.toml");
    std::fs::write(&path, "this is not [valid toml").unwrap();
    let err = AppConfig::load(&path);
    assert!(err.is_err(), "expected error for malformed TOML");
}

#[test]
fn load_missing_file_returns_default() {
    let path = std::path::Path::new("/nonexistent/path/config.toml");
    let cfg = AppConfig::load(path).unwrap();
    assert_eq!(cfg.search.mode, Mode::default());
}

#[test]
fn enabled_provider_ids_returns_only_enabled() {
    let mut c = AppConfig::default();
    c.search.providers.insert("duckduckgo".to_string(), true);
    c.search.providers.insert("brave".to_string(), false);
    c.search.providers.insert("startpage".to_string(), true);

    let ids = c.enabled_provider_ids();
    assert!(ids.contains(&"duckduckgo".to_string()));
    assert!(!ids.contains(&"brave".to_string()));
    assert!(ids.contains(&"startpage".to_string()));
}

#[test]
fn effective_provider_ids_include_configured_api_provider() {
    let mut c = AppConfig::default();
    let env = "EGGSEARCH_TEST_EFFECTIVE_PROVIDER_KEY";
    std::env::set_var(env, "test_key");
    c.search.api.insert(
        "brave_api".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: Some(env.to_string()),
            base_url: None,
        },
    );

    let ids = c.effective_provider_ids();
    std::env::remove_var(env);
    assert!(ids.contains(&"brave_api".to_string()), "got: {ids:?}");
}

#[test]
fn misconfigured_default_providers_lists_disabled() {
    let mut c = AppConfig::default();
    c.search.providers.insert("duckduckgo".to_string(), true);
    c.search.providers.insert("brave".to_string(), false);
    c.search.providers.insert("startpage".to_string(), true);
    c.search.providers.insert("yahoo".to_string(), false);
    c.search.default_providers = vec![
        "duckduckgo".to_string(),
        "brave".to_string(),
        "yahoo".to_string(),
        "ghost".to_string(), // not in the providers map at all
    ];

    let misconfigured = c.misconfigured_default_providers();
    assert!(
        misconfigured.contains(&"brave".to_string()),
        "got: {misconfigured:?}"
    );
    assert!(
        misconfigured.contains(&"yahoo".to_string()),
        "got: {misconfigured:?}"
    );
    assert!(
        misconfigured.contains(&"ghost".to_string()),
        "got: {misconfigured:?}"
    );
    assert!(
        !misconfigured.contains(&"duckduckgo".to_string()),
        "got: {misconfigured:?}"
    );
    assert_eq!(misconfigured.len(), 3, "got: {misconfigured:?}");
}

#[test]
fn misconfigured_default_providers_empty_when_all_enabled() {
    let c = AppConfig::default();
    assert!(c.misconfigured_default_providers().is_empty());
}

#[test]
fn validate_accepts_defaults() {
    let c = AppConfig::default();
    assert!(
        c.validate().is_ok(),
        "default config should validate: {:?}",
        c.validate().err()
    );
}

#[test]
fn validate_rejects_cap_below_default() {
    let mut c = AppConfig::default();
    c.fetch.max_chars_cap = 100;
    c.fetch.max_chars_default = 12_000;
    let err = c.validate().expect_err("expected validation failure");
    assert!(err.to_string().contains("max_chars_cap"), "got: {err}");
}

#[test]
fn validate_rejects_zero_max_bytes() {
    let mut c = AppConfig::default();
    c.fetch.max_bytes = 0;
    let err = c.validate().expect_err("expected validation failure");
    assert!(err.to_string().contains("max_bytes"), "got: {err}");
}

#[test]
fn validate_rejects_zero_timeouts() {
    let mut c = AppConfig::default();
    c.fetch.timeout_ms = 0;
    let err = c.validate().expect_err("expected fetch timeout failure");
    assert!(err.to_string().contains("[fetch].timeout_ms"), "got: {err}");

    let mut c2 = AppConfig::default();
    c2.search.timeout_ms = 0;
    let err2 = c2.validate().expect_err("expected search timeout failure");
    assert!(
        err2.to_string().contains("[search].timeout_ms"),
        "got: {err2}"
    );
}

#[test]
fn validate_rejects_excessive_profile_process_timeout() {
    let mut c = AppConfig::default();
    c.fetch
        .browser
        .persistent_profiles
        .profile_process_timeout_ms = MAX_PROFILE_PROCESS_TIMEOUT_MS + 1;
    let err = c
        .validate()
        .expect_err("expected profile process timeout cap failure");
    assert!(
        err.to_string()
            .contains("profile_process_timeout_ms exceeds maximum"),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_zero_default_max_results() {
    let mut c = AppConfig::default();
    c.search.default_max_results = 0;
    let err = c
        .validate()
        .expect_err("expected default_max_results failure");
    assert!(
        err.to_string().contains("default_max_results"),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_max_results_cap_below_default_max_results() {
    let mut c = AppConfig::default();
    c.search.default_max_results = 50;
    c.search.max_results_cap = 10;
    let err = c.validate().expect_err("expected cap failure");
    assert!(
        err.to_string().contains("default_max_results"),
        "got: {err}"
    );
    assert!(err.to_string().contains("max_results_cap"), "got: {err}");
}

#[test]
fn validate_rejects_zero_max_results_cap() {
    let mut c = AppConfig::default();
    c.search.max_results_cap = 0;
    let err = c.validate().expect_err("expected max_results_cap failure");
    assert!(err.to_string().contains("max_results_cap"), "got: {err}");
}

#[test]
fn validate_rejects_zero_max_query_chars() {
    let mut c = AppConfig::default();
    c.search.max_query_chars = 0;
    let err = c.validate().expect_err("expected max_query_chars failure");
    assert!(err.to_string().contains("max_query_chars"), "got: {err}");
}

#[test]
fn default_search_section_has_sanitize_output_true() {
    let c = AppConfig::default();
    assert!(c.search.sanitize_output);
}

#[test]
fn default_fetch_section_has_sanitize_output_true() {
    let c = AppConfig::default();
    assert!(c.fetch.sanitize_output);
}

#[test]
fn validate_rejects_no_providers_enabled_in_live_mode() {
    let mut c = AppConfig::default();
    c.search.mode = Mode::Live;
    // Disable ALL providers
    let keys: Vec<_> = c.search.providers.keys().cloned().collect();
    for key in keys {
        c.search.providers.insert(key, false);
    }
    let err = c.validate().expect_err("expected no-providers failure");
    assert!(
        err.to_string()
            .contains("no traditional providers are enabled"),
        "got: {err}"
    );
}

#[test]
fn validate_allows_no_providers_in_off_mode() {
    let mut c = AppConfig::default();
    c.search.mode = Mode::Off;
    let keys: Vec<_> = c.search.providers.keys().cloned().collect();
    for key in keys {
        c.search.providers.insert(key, false);
    }
    // mode=off with no providers is fine - no search is attempted
    assert!(c.validate().is_ok());
}

#[test]
fn validate_allows_api_only_deployment_in_live_mode() {
    use super::ApiProviderConfig;

    let mut c = AppConfig::default();
    c.search.mode = Mode::Live;
    // Disable ALL traditional providers
    let keys: Vec<_> = c.search.providers.keys().cloned().collect();
    for key in keys {
        c.search.providers.insert(key, false);
    }
    // Enable an API provider so live mode is valid
    c.search.api.insert(
        "brave_api".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: Some("BRAVE_API_KEY".to_string()),
            base_url: None,
        },
    );
    // Setting env var so validation passes
    std::env::set_var("BRAVE_API_KEY", "test_key");
    assert!(
        c.validate().is_ok(),
        "API-only deployment should be valid in live mode"
    );
}

#[test]
fn validate_rejects_live_mode_no_providers_no_api() {
    let mut c = AppConfig::default();
    c.search.mode = Mode::Live;
    // Disable ALL providers
    let keys: Vec<_> = c.search.providers.keys().cloned().collect();
    for key in keys {
        c.search.providers.insert(key, false);
    }
    let err = c.validate().expect_err("expected no-providers failure");
    assert!(
        err.to_string().contains("no API providers are configured"),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_unknown_default_provider() {
    let mut c = AppConfig::default();
    c.search
        .default_providers
        .push("ghost_provider".to_string());
    let err = c
        .validate()
        .expect_err("expected unknown default provider failure");
    assert!(
        err.to_string().contains("unknown provider: ghost_provider"),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_unknown_provider_in_providers_map() {
    let mut c = AppConfig::default();
    c.search
        .providers
        .insert("ghost_provider".to_string(), true);
    let err = c
        .validate()
        .expect_err("expected unknown providers map key failure");
    assert!(
        err.to_string().contains("unknown provider: ghost_provider"),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_searxng_enabled_without_base_url() {
    let mut c = AppConfig::default();
    c.search.searxng.enabled = true;
    c.search.searxng.base_url = None;
    let err = c
        .validate()
        .expect_err("expected searxng without base_url failure");
    assert!(
        err.to_string().contains("base_url is missing or empty"),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_searxng_enabled_with_empty_base_url() {
    let mut c = AppConfig::default();
    c.search.searxng.enabled = true;
    c.search.searxng.base_url = Some(String::new());
    let err = c
        .validate()
        .expect_err("expected searxng with empty base_url failure");
    assert!(
        err.to_string().contains("base_url is missing or empty"),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_searxng_enabled_with_invalid_url() {
    let mut c = AppConfig::default();
    c.search.searxng.enabled = true;
    c.search.searxng.base_url = Some("not a url".to_string());
    let err = c
        .validate()
        .expect_err("expected searxng with invalid URL failure");
    assert!(err.to_string().contains("not a valid URL"), "got: {err}");
}

#[test]
fn validate_accepts_searxng_enabled_with_valid_url() {
    let mut c = AppConfig::default();
    c.search.searxng.enabled = true;
    c.search.searxng.base_url = Some("https://searx.example.org".to_string());
    assert!(c.validate().is_ok());
}

#[test]
fn resolve_providers_rejects_unknown_in_explicit_list() {
    let c = AppConfig::default();
    let result = c.resolve_providers(&["ghost_provider".to_string()]);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("unknown provider"),);
}

#[test]
fn resolve_providers_filters_disabled_default_providers() {
    let mut c = AppConfig::default();
    c.search.default_providers = vec!["duckduckgo".to_string(), "brave".to_string()];
    c.search.providers.insert("brave".to_string(), false);

    let out = c.resolve_providers(&[]).unwrap();
    assert_eq!(out, vec!["duckduckgo".to_string()]);
}

#[test]
fn resolve_providers_filters_unconfigured_api_default_provider() {
    let mut c = AppConfig::default();
    c.search.default_providers = vec!["brave_api".to_string()];
    c.search.api.insert(
        "brave_api".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: Some("EGGSEARCH_TEST_MISSING_DEFAULT_API_KEY".to_string()),
            base_url: None,
        },
    );

    let err = c.resolve_providers(&[]).unwrap_err();
    assert!(
        err.to_string().contains("no default providers are enabled"),
        "got: {err}"
    );
}

// -----------------------------------------------------------------------
// API provider config tests
// -----------------------------------------------------------------------

#[test]
fn default_api_map_is_empty() {
    let c = AppConfig::default();
    assert!(c.search.api.is_empty());
}

#[test]
fn api_provider_config_serde_roundtrip() {
    let mut api = std::collections::BTreeMap::new();
    api.insert(
        "brave_api".to_string(),
        ApiProviderConfig {
            enabled: false,
            api_key_env: Some("BRAVE_SEARCH_API_KEY".to_string()),
            base_url: None,
        },
    );
    let mut c = AppConfig::default();
    c.search.api = api;

    let text = toml::to_string(&c).unwrap();
    let parsed: AppConfig = toml::from_str(&text).unwrap();
    assert_eq!(parsed.search.api.len(), 1);
    let cfg = parsed.search.api.get("brave_api").unwrap();
    assert!(!cfg.enabled);
    assert_eq!(cfg.api_key_env.as_deref(), Some("BRAVE_SEARCH_API_KEY"));
}

#[test]
fn validate_rejects_api_provider_enabled_without_key_env() {
    let mut c = AppConfig::default();
    c.search.api.insert(
        "brave_api".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: None,
            base_url: None,
        },
    );
    let err = c
        .validate()
        .expect_err("expected api_key_env missing failure");
    assert!(
        err.to_string().contains("api_key_env is missing or empty"),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_api_provider_enabled_with_empty_key_env() {
    let mut c = AppConfig::default();
    c.search.api.insert(
        "brave_api".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: Some(String::new()),
            base_url: None,
        },
    );
    let err = c
        .validate()
        .expect_err("expected api_key_env empty failure");
    assert!(
        err.to_string().contains("api_key_env is missing or empty"),
        "got: {err}"
    );
}

#[test]
fn validate_accepts_api_provider_disabled_without_key_env() {
    let mut c = AppConfig::default();
    c.search.api.insert(
        "brave_api".to_string(),
        ApiProviderConfig {
            enabled: false,
            api_key_env: None,
            base_url: None,
        },
    );
    // Disabled providers don't need a key env
    assert!(c.validate().is_ok());
}

#[test]
fn validate_rejects_api_provider_with_invalid_base_url() {
    let mut c = AppConfig::default();
    c.search.api.insert(
        "brave_api".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: Some("BRAVE_SEARCH_API_KEY".to_string()),
            base_url: Some("not a url".to_string()),
        },
    );
    let err = c.validate().expect_err("expected invalid base_url failure");
    assert!(err.to_string().contains("not a valid URL"), "got: {err}");
}

#[test]
fn validate_accepts_api_provider_with_valid_config() {
    let mut c = AppConfig::default();
    c.search.api.insert(
        "brave_api".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: Some("BRAVE_SEARCH_API_KEY".to_string()),
            base_url: Some("https://api.search.brave.com/res/v1/web/search".to_string()),
        },
    );
    // Even if the env var isn't set, validate() only warns (doesn't error)
    assert!(c.validate().is_ok());
}

#[test]
fn resolve_providers_accepts_configured_brave_api_in_explicit_list() {
    let mut c = AppConfig::default();
    let env = "EGGSEARCH_TEST_EXPLICIT_BRAVE_API_KEY";
    std::env::set_var(env, "test_key");
    c.search.api.insert(
        "brave_api".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: Some(env.to_string()),
            base_url: None,
        },
    );

    let result = c.resolve_providers(&["brave_api".to_string()]);
    std::env::remove_var(env);
    assert!(
        result.is_ok(),
        "brave_api should be accepted: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap(), vec!["brave_api".to_string()]);
}

#[test]
fn resolve_providers_rejects_unconfigured_api_in_explicit_list() {
    let mut c = AppConfig::default();
    c.search.api.insert(
        "brave_api".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: Some("EGGSEARCH_TEST_MISSING_EXPLICIT_API_KEY".to_string()),
            base_url: None,
        },
    );

    let err = c.resolve_providers(&["brave_api".to_string()]).unwrap_err();
    assert!(
        err.to_string().contains("disabled or unavailable"),
        "got: {err}"
    );
}

#[test]
fn resolve_providers_rejects_api_enabled_only_in_provider_map() {
    let mut c = AppConfig::default();
    c.search.providers.insert("brave_api".to_string(), true);

    let err = c.resolve_providers(&["brave_api".to_string()]).unwrap_err();
    assert!(
        err.to_string().contains("disabled or unavailable"),
        "got: {err}"
    );
}

#[test]
fn api_provider_config_default_values() {
    let cfg = ApiProviderConfig {
        enabled: false,
        api_key_env: None,
        base_url: None,
    };
    assert!(!cfg.enabled);
    assert!(cfg.api_key_env.is_none());
    assert!(cfg.base_url.is_none());
}

// -----------------------------------------------------------------------
// api_provider_is_configured helper tests
// -----------------------------------------------------------------------

#[test]
fn live_mode_valid_with_configured_api_provider() {
    use super::ApiProviderConfig;

    let mut c = AppConfig::default();
    c.search.mode = Mode::Live;
    // Disable ALL traditional providers
    let keys: Vec<_> = c.search.providers.keys().cloned().collect();
    for key in keys {
        c.search.providers.insert(key, false);
    }
    // Enable brave_api with env var set
    c.search.api.insert(
        "brave_api".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: Some("EGGSEARCH_TEST_LIVE_MODE_KEY_12345".to_string()),
            base_url: None,
        },
    );
    std::env::set_var("EGGSEARCH_TEST_LIVE_MODE_KEY_12345", "test_key");
    assert!(
        c.validate().is_ok(),
        "API-only deployment with configured env var should be valid"
    );
}

#[test]
fn live_mode_invalid_with_missing_api_env() {
    use super::ApiProviderConfig;

    let mut c = AppConfig::default();
    c.search.mode = Mode::Live;
    // Disable ALL traditional providers
    let keys: Vec<_> = c.search.providers.keys().cloned().collect();
    for key in keys {
        c.search.providers.insert(key, false);
    }
    // Enable brave_api but env var is NOT set
    c.search.api.insert(
        "brave_api".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: Some("EGGSEARCH_TEST_MISSING_KEY_98765".to_string()),
            base_url: None,
        },
    );
    let err = c.validate().expect_err("expected missing env var failure");
    assert!(
        err.to_string().contains("no API providers are configured"),
        "got: {err}"
    );
}

#[test]
fn live_mode_invalid_with_empty_api_key_env() {
    use super::ApiProviderConfig;

    let mut c = AppConfig::default();
    c.search.mode = Mode::Live;
    // Disable ALL traditional providers
    let keys: Vec<_> = c.search.providers.keys().cloned().collect();
    for key in keys {
        c.search.providers.insert(key, false);
    }
    // Enabled API provider with empty api_key_env → rejected by
    // per-field validation before the live-mode check
    c.search.api.insert(
        "brave_api".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: Some(String::new()),
            base_url: None,
        },
    );
    let err = c
        .validate()
        .expect_err("expected empty api_key_env failure");
    assert!(
        err.to_string().contains("api_key_env is missing or empty"),
        "got: {err}"
    );
}

#[test]
fn live_mode_invalid_with_unknown_api_provider() {
    use super::ApiProviderConfig;

    let mut c = AppConfig::default();
    c.search.mode = Mode::Live;
    // Disable ALL traditional providers
    let keys: Vec<_> = c.search.providers.keys().cloned().collect();
    for key in keys {
        c.search.providers.insert(key, false);
    }
    // Enable an unknown API provider (not in API_PROVIDER_IDS)
    c.search.api.insert(
        "unknown_api".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: Some("SOME_KEY".to_string()),
            base_url: None,
        },
    );
    std::env::set_var("SOME_KEY", "test_key");
    // Unknown API provider passes per-field validation (it logs a warning
    // but does not error) and is not in API_PROVIDER_IDS, so
    // api_provider_is_configured returns false. Live mode should reject.
    let err = c
        .validate()
        .expect_err("expected unknown api provider failure");
    assert!(
        err.to_string().contains("no API providers are configured"),
        "got: {err}"
    );
}

// -----------------------------------------------------------------------
// Profile config tests
// -----------------------------------------------------------------------

#[test]
fn default_profiles_map_is_empty() {
    let c = AppConfig::default();
    assert!(c.search.profiles.is_empty());
}

#[test]
fn validate_accepts_canonical_profile_names() {
    let mut c = AppConfig::default();
    c.search.profiles.insert(
        "research".to_string(),
        ProfileConfig {
            providers: vec!["duckduckgo".to_string()],
        },
    );
    assert!(c.validate().is_ok());
}

#[test]
fn validate_rejects_unknown_profile_name() {
    let mut c = AppConfig::default();
    // Lookup goes through `SearchProfile::as_str()`, so a typo'd key can
    // never be selected and would otherwise stay dead.
    c.search.profiles.insert(
        "reserach".to_string(),
        ProfileConfig {
            providers: vec!["duckduckgo".to_string()],
        },
    );
    let err = c.validate().expect_err("typo'd profile must be rejected");
    assert!(err.to_string().contains("reserach"), "{err}");
}

#[test]
fn validate_rejects_empty_profile_provider_list() {
    let mut c = AppConfig::default();
    c.search
        .profiles
        .insert("coding".to_string(), ProfileConfig { providers: vec![] });
    let err = c
        .validate()
        .expect_err("empty provider list must be rejected");
    assert!(err.to_string().contains("must not be empty"), "{err}");
}

#[test]
fn validate_rejects_unknown_provider_in_profile() {
    let mut c = AppConfig::default();
    c.search.profiles.insert(
        "coding".to_string(),
        ProfileConfig {
            providers: vec!["not_a_provider".to_string()],
        },
    );
    let err = c.validate().expect_err("unknown provider must be rejected");
    assert!(err.to_string().contains("not_a_provider"), "{err}");
}

#[test]
fn profile_config_serde_roundtrip() {
    let mut profiles = std::collections::BTreeMap::new();
    profiles.insert(
        "coding".to_string(),
        ProfileConfig {
            providers: vec![
                "github_code".to_string(),
                "github_issues".to_string(),
                "duckduckgo".to_string(),
            ],
        },
    );
    let mut c = AppConfig::default();
    c.search.profiles = profiles;

    let text = toml::to_string(&c).unwrap();
    let parsed: AppConfig = toml::from_str(&text).unwrap();
    assert_eq!(parsed.search.profiles.len(), 1);
    let coding = parsed.search.profiles.get("coding").unwrap();
    assert_eq!(coding.providers.len(), 3);
    assert_eq!(coding.providers[0], "github_code");
}

#[test]
fn resolve_profile_providers_no_profile_no_explicit() {
    let c = AppConfig::default();
    let (providers, degraded, warnings) = c.resolve_profile_providers(None, &[]);
    assert!(!providers.is_empty());
    assert!(!degraded);
    assert!(warnings.is_empty());
    assert_eq!(providers, c.search.default_providers);
}

#[test]
fn resolve_profile_providers_explicit_overrides_profile() {
    let c = AppConfig::default();
    let (providers, degraded, warnings) = c.resolve_profile_providers(
        Some(crate::core::repo_search::SearchProfile::Coding),
        &["duckduckgo".to_string()],
    );
    assert_eq!(providers, vec!["duckduckgo".to_string()]);
    assert!(!degraded);
    assert!(warnings.is_empty());
}

#[test]
fn resolve_profile_providers_coding_profile_uses_builtins() {
    let c = AppConfig::default();
    let (providers, _degraded, warnings) =
        c.resolve_profile_providers(Some(crate::core::repo_search::SearchProfile::Coding), &[]);
    // Coding profile built-in defaults include github_code, github_issues, etc.
    // Most won't be enabled in default config, so some will be skipped with warnings
    assert!(!providers.is_empty());
    // Should have warnings about unavailable providers
    assert!(!warnings.is_empty());
    // Available providers should be a subset of the enabled set. A built-in
    // profile is not required to be a subset of default_providers: `coding`
    // lists `startpage`, which stays enabled and selectable even though it is
    // no longer default-routed.
    let enabled = c.effective_provider_ids();
    for p in &providers {
        assert!(
            enabled.contains(p),
            "resolved provider {p} should be enabled"
        );
    }
}

#[test]
fn resolve_profile_providers_configured_profile_used() {
    let mut c = AppConfig::default();
    c.search.profiles.insert(
        "coding".to_string(),
        ProfileConfig {
            providers: vec!["duckduckgo".to_string(), "startpage".to_string()],
        },
    );
    let (providers, degraded, _warnings) =
        c.resolve_profile_providers(Some(crate::core::repo_search::SearchProfile::Coding), &[]);
    assert_eq!(providers, vec!["duckduckgo", "startpage"]);
    assert!(!degraded);
}

#[test]
fn resolve_profile_providers_generic_falls_back_to_defaults() {
    let c = AppConfig::default();
    let (providers, degraded, _warnings) =
        c.resolve_profile_providers(Some(crate::core::repo_search::SearchProfile::Generic), &[]);
    assert_eq!(providers, c.search.default_providers);
    assert!(!degraded);
}

#[test]
fn resolve_profile_providers_unknown_explicit_returns_warning() {
    let c = AppConfig::default();
    let (_providers, degraded, warnings) =
        c.resolve_profile_providers(None, &["ghost_provider".to_string()]);
    assert!(degraded);
    assert!(warnings
        .iter()
        .any(|w| w.message.contains("provider_resolution_failed")));
}

#[test]
fn profile_config_toml_parse() {
    let toml = r#"
[search]
mode = "live"
default_max_results = 10
max_results_cap = 50
max_query_chars = 512
timeout_ms = 8000
default_providers = ["duckduckgo"]

[search.providers]
duckduckgo = true

[search.profiles.coding]
providers = ["github_code", "duckduckgo"]

[search.profiles.security]
providers = ["osv", "duckduckgo"]
"#;
    let c: AppConfig = toml::from_str(toml).unwrap();
    assert_eq!(c.search.profiles.len(), 2);
    let coding = c.search.profiles.get("coding").unwrap();
    assert_eq!(coding.providers, vec!["github_code", "duckduckgo"]);
    let security = c.search.profiles.get("security").unwrap();
    assert_eq!(security.providers, vec!["osv", "duckduckgo"]);
}

#[test]
fn search_profile_parse_returns_none_for_unknown() {
    use crate::core::repo_search::SearchProfile;
    assert_eq!(SearchProfile::parse("unknown"), None);
    assert_eq!(SearchProfile::parse(""), None);
}

#[test]
fn search_profile_parse_case_insensitive() {
    use crate::core::repo_search::SearchProfile;
    assert_eq!(SearchProfile::parse("Coding"), Some(SearchProfile::Coding));
    assert_eq!(
        SearchProfile::parse("SECURITY"),
        Some(SearchProfile::Security)
    );
    assert_eq!(
        SearchProfile::parse("Research"),
        Some(SearchProfile::Research)
    );
}

// -----------------------------------------------------------------------
// Batch fetch config tests
// -----------------------------------------------------------------------

#[test]
fn default_batch_config_values() {
    let c = AppConfig::default();
    assert_eq!(c.fetch.batch_max_items, 8);
    assert_eq!(c.fetch.batch_max_items_cap, 20);
    assert_eq!(c.fetch.batch_max_chars_per_item, 12000);
    assert_eq!(c.fetch.batch_max_total_chars, 50000);
    assert_eq!(c.fetch.batch_max_total_chars_cap, 120000);
    assert_eq!(c.fetch.batch_concurrency, 4);
}

#[test]
fn validate_accepts_default_batch_config() {
    let c = AppConfig::default();
    assert!(c.validate().is_ok());
}

#[test]
fn validate_rejects_zero_batch_max_items() {
    let mut c = AppConfig::default();
    c.fetch.batch_max_items = 0;
    let err = c.validate().expect_err("expected batch_max_items failure");
    assert!(err.to_string().contains("batch_max_items"), "got: {err}");
}

#[test]
fn validate_rejects_zero_batch_max_items_cap() {
    let mut c = AppConfig::default();
    c.fetch.batch_max_items_cap = 0;
    let err = c
        .validate()
        .expect_err("expected batch_max_items_cap failure");
    assert!(
        err.to_string().contains("batch_max_items_cap"),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_batch_max_items_cap_below_batch_max_items() {
    let mut c = AppConfig::default();
    c.fetch.batch_max_items = 10;
    c.fetch.batch_max_items_cap = 5;
    let err = c
        .validate()
        .expect_err("expected cap below default failure");
    assert!(
        err.to_string().contains("batch_max_items_cap"),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_zero_batch_max_total_chars() {
    let mut c = AppConfig::default();
    c.fetch.batch_max_total_chars = 0;
    let err = c
        .validate()
        .expect_err("expected batch_max_total_chars failure");
    assert!(
        err.to_string().contains("batch_max_total_chars"),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_batch_max_total_chars_cap_below_batch_max_total_chars() {
    let mut c = AppConfig::default();
    c.fetch.batch_max_total_chars = 50000;
    c.fetch.batch_max_total_chars_cap = 10000;
    let err = c.validate().expect_err("expected total chars cap failure");
    assert!(
        err.to_string().contains("batch_max_total_chars_cap"),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_zero_batch_concurrency() {
    let mut c = AppConfig::default();
    c.fetch.batch_concurrency = 0;
    let err = c
        .validate()
        .expect_err("expected batch_concurrency failure");
    assert!(err.to_string().contains("batch_concurrency"), "got: {err}");
}

#[test]
fn batch_config_toml_roundtrip() {
    let c = AppConfig::default();
    let text = toml::to_string(&c).unwrap();
    let parsed: AppConfig = toml::from_str(&text).unwrap();
    assert_eq!(parsed.fetch.batch_max_items, c.fetch.batch_max_items);
    assert_eq!(
        parsed.fetch.batch_max_items_cap,
        c.fetch.batch_max_items_cap
    );
    assert_eq!(
        parsed.fetch.batch_max_chars_per_item,
        c.fetch.batch_max_chars_per_item
    );
    assert_eq!(
        parsed.fetch.batch_max_total_chars,
        c.fetch.batch_max_total_chars
    );
    assert_eq!(
        parsed.fetch.batch_max_total_chars_cap,
        c.fetch.batch_max_total_chars_cap
    );
    assert_eq!(parsed.fetch.batch_concurrency, c.fetch.batch_concurrency);
}

#[test]
fn challenge_defaults_agree_across_sites() {
    let defaulted = ChallengeSection::default();
    let parsed: AppConfig = toml::from_str("").expect("empty config parses");
    let from_search = AppConfig::default().search.challenge;
    assert_eq!(parsed.search.challenge.enabled, defaulted.enabled);
    assert_eq!(
        parsed.search.challenge.max_difficulty,
        defaulted.max_difficulty
    );
    assert_eq!(parsed.search.challenge.max_attempts, defaulted.max_attempts);
    assert_eq!(parsed.search.challenge.timeout_ms, defaulted.timeout_ms);
    assert_eq!(
        parsed.search.challenge.cookie_ttl_seconds,
        defaulted.cookie_ttl_seconds
    );
    assert_eq!(from_search.enabled, defaulted.enabled);
    assert_eq!(from_search.max_difficulty, defaulted.max_difficulty);
    assert_eq!(from_search.max_attempts, defaulted.max_attempts);
    assert_eq!(from_search.timeout_ms, defaulted.timeout_ms);
    assert_eq!(from_search.cookie_ttl_seconds, defaulted.cookie_ttl_seconds);
}

#[test]
fn challenge_defaults_are_disabled_and_bounded() {
    let challenge = ChallengeSection::default();
    assert!(!challenge.enabled);
    assert_eq!(challenge.max_difficulty, 5);
    assert_eq!(challenge.max_attempts, 2_000_000);
    assert_eq!(challenge.timeout_ms, 2000);
    assert_eq!(challenge.cookie_ttl_seconds, 240);
}

#[test]
fn config_without_challenge_table_keeps_today_behaviour() {
    let parsed: AppConfig = toml::from_str("").expect("empty config parses");
    assert!(!parsed.search.challenge.enabled);
    assert_eq!(parsed.search.challenge.max_attempts, 2_000_000);
    assert_eq!(parsed.search.challenge.timeout_ms, 2000);
    assert_eq!(parsed.search.challenge.cookie_ttl_seconds, 240);
    parsed.validate().expect("defaults validate");
}

#[test]
fn challenge_table_parses_and_validates() {
    let mut value: toml::Value =
        toml::from_str(&toml::to_string(&AppConfig::default()).expect("serialize"))
            .expect("toml value");
    let challenge = value
        .get_mut("search")
        .and_then(|search| search.get_mut("challenge"))
        .and_then(toml::Value::as_table_mut)
        .expect("challenge table present");
    challenge.insert("enabled".to_string(), toml::Value::Boolean(true));
    challenge.insert("max_difficulty".to_string(), toml::Value::Integer(4));
    challenge.insert("max_attempts".to_string(), toml::Value::Integer(1000));
    challenge.insert("timeout_ms".to_string(), toml::Value::Integer(500));
    challenge.insert("cookie_ttl_seconds".to_string(), toml::Value::Integer(0));

    let parsed: AppConfig = value.try_into().expect("config parses");
    assert!(parsed.search.challenge.enabled);
    assert_eq!(parsed.search.challenge.max_difficulty, 4);
    assert_eq!(parsed.search.challenge.max_attempts, 1000);
    assert_eq!(parsed.search.challenge.timeout_ms, 500);
    assert_eq!(parsed.search.challenge.cookie_ttl_seconds, 0);
    parsed.validate().expect("explicit values validate");
}

#[test]
fn validate_rejects_challenge_max_difficulty_out_of_range() {
    for value in [0u8, 11] {
        let mut c = AppConfig::default();
        c.search.challenge.max_difficulty = value;
        let err = c.validate().expect_err("expected max_difficulty failure");
        assert!(
            err.to_string()
                .contains("[search.challenge].max_difficulty"),
            "got: {err}"
        );
    }
}

#[test]
fn validate_rejects_challenge_max_attempts_out_of_range() {
    for value in [0u64, 100_000_001] {
        let mut c = AppConfig::default();
        c.search.challenge.max_attempts = value;
        let err = c.validate().expect_err("expected max_attempts failure");
        assert!(
            err.to_string().contains("[search.challenge].max_attempts"),
            "got: {err}"
        );
    }
}

#[test]
fn validate_rejects_challenge_timeout_out_of_range() {
    for value in [99u64, 30_001] {
        let mut c = AppConfig::default();
        c.search.challenge.timeout_ms = value;
        let err = c.validate().expect_err("expected timeout_ms failure");
        assert!(
            err.to_string().contains("[search.challenge].timeout_ms"),
            "got: {err}"
        );
    }
}

#[test]
fn validate_rejects_challenge_cookie_ttl_out_of_range() {
    let mut c = AppConfig::default();
    c.search.challenge.cookie_ttl_seconds = 3601;
    let err = c
        .validate()
        .expect_err("expected cookie_ttl_seconds failure");
    assert!(
        err.to_string()
            .contains("[search.challenge].cookie_ttl_seconds"),
        "got: {err}"
    );
}

#[test]
fn challenge_config_toml_roundtrip() {
    let mut c = AppConfig::default();
    c.search.challenge.enabled = true;
    c.search.challenge.max_difficulty = 7;
    c.search.challenge.max_attempts = 1234;
    c.search.challenge.timeout_ms = 900;
    c.search.challenge.cookie_ttl_seconds = 30;
    let text = toml::to_string(&c).expect("serialize");
    let parsed: AppConfig = toml::from_str(&text).expect("parse");
    assert!(parsed.search.challenge.enabled);
    assert_eq!(parsed.search.challenge.max_difficulty, 7);
    assert_eq!(parsed.search.challenge.max_attempts, 1234);
    assert_eq!(parsed.search.challenge.timeout_ms, 900);
    assert_eq!(parsed.search.challenge.cookie_ttl_seconds, 30);
}
