use eggsearch::core::config::{ApiProviderConfig, AppConfig, Mode};
use eggsearch::core::provider::KNOWN_PROVIDER_IDS;

fn default_config() -> AppConfig {
    AppConfig::default()
}

#[test]
fn unknown_toml_top_level_key_is_silently_ignored() {
    let toml = r#"
[unknown_section]
foo = "bar"

[search]
mode = "live"
default_max_results = 10
max_results_cap = 50
max_query_chars = 512
timeout_ms = 8000
default_providers = ["duckduckgo"]
"#;
    let cfg: AppConfig = toml::from_str(toml).unwrap();
    assert_eq!(cfg.search.mode, Mode::Live);
}

#[test]
fn unknown_toml_search_key_is_silently_ignored() {
    let toml = r#"
[search]
mode = "live"
default_max_results = 10
max_results_cap = 50
max_query_chars = 512
timeout_ms = 8000
default_providers = ["duckduckgo"]
bogus_field = 42
"#;
    let cfg: AppConfig = toml::from_str(toml).unwrap();
    assert_eq!(cfg.search.timeout_ms, 8000);
}

#[test]
fn invalid_duration_string_in_timeout_ms_rejected() {
    let toml = r#"
[search]
mode = "live"
default_max_results = 10
max_results_cap = 50
max_query_chars = 512
timeout_ms = "30s"
default_providers = ["duckduckgo"]
"#;
    let err = toml::from_str::<AppConfig>(toml).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("timeout_ms") || msg.contains("invalid type") || msg.contains("string"),
        "expected type mismatch for timeout_ms, got: {msg}"
    );
}

#[test]
fn invalid_duration_string_in_fetch_timeout_ms_rejected() {
    let toml = r#"
[fetch]
timeout_ms = "5m"
"#;
    let err = toml::from_str::<AppConfig>(toml).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("timeout_ms") || msg.contains("invalid type") || msg.contains("string"),
        "expected type mismatch for fetch timeout_ms, got: {msg}"
    );
}

#[test]
fn negative_number_in_unsigned_field_rejected() {
    let toml = r#"
[search]
mode = "live"
default_max_results = -5
max_results_cap = 50
max_query_chars = 512
timeout_ms = 8000
default_providers = ["duckduckgo"]
"#;
    let err = toml::from_str::<AppConfig>(toml).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("default_max_results")
            || msg.contains("invalid type")
            || msg.contains("number"),
        "expected error for negative default_max_results, got: {msg}"
    );
}

#[test]
fn negative_timeout_ms_rejected() {
    let toml = r#"
[search]
mode = "live"
default_max_results = 10
max_results_cap = 50
max_query_chars = 512
timeout_ms = -1000
default_providers = ["duckduckgo"]
"#;
    let err = toml::from_str::<AppConfig>(toml).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("timeout_ms") || msg.contains("invalid type") || msg.contains("number"),
        "expected error for negative timeout_ms, got: {msg}"
    );
}

#[test]
fn negative_fetch_max_bytes_rejected() {
    let toml = r#"
[fetch]
max_bytes = -1
"#;
    let err = toml::from_str::<AppConfig>(toml).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("max_bytes") || msg.contains("invalid type") || msg.contains("number"),
        "expected error for negative max_bytes, got: {msg}"
    );
}

#[test]
fn negative_fetch_timeout_ms_rejected() {
    let toml = r#"
[fetch]
timeout_ms = -42
"#;
    let err = toml::from_str::<AppConfig>(toml).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("timeout_ms") || msg.contains("invalid type") || msg.contains("number"),
        "expected error for negative fetch timeout_ms, got: {msg}"
    );
}

#[test]
fn unknown_provider_id_in_default_providers_rejected_by_validate() {
    let mut cfg = default_config();
    cfg.search
        .default_providers
        .push("nonexistent_provider".to_string());
    let err = cfg.validate().unwrap_err();
    assert!(
        err.to_string()
            .contains("unknown provider: nonexistent_provider"),
        "got: {err}"
    );
}

#[test]
fn unknown_provider_id_in_providers_map_rejected_by_validate() {
    let mut cfg = default_config();
    cfg.search
        .providers
        .insert("nonexistent_provider".to_string(), true);
    let err = cfg.validate().unwrap_err();
    assert!(
        err.to_string()
            .contains("unknown provider: nonexistent_provider"),
        "got: {err}"
    );
}

#[test]
fn unknown_provider_id_in_default_providers_serde_accepts() {
    let toml = r#"
[search]
mode = "live"
default_max_results = 10
max_results_cap = 50
max_query_chars = 512
timeout_ms = 8000
default_providers = ["duckduckgo", "ghost_provider"]

[search.providers]
duckduckgo = true
"#;
    let cfg: AppConfig = toml::from_str(toml).unwrap();
    assert!(cfg
        .search
        .default_providers
        .contains(&"ghost_provider".to_string()));
    let err = cfg.validate().unwrap_err();
    assert!(err.to_string().contains("ghost_provider"));
}

#[test]
fn unknown_provider_id_in_providers_map_serde_accepts() {
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
ghost_provider = true
"#;
    let cfg: AppConfig = toml::from_str(toml).unwrap();
    assert!(cfg.search.providers.contains_key("ghost_provider"));
    let err = cfg.validate().unwrap_err();
    assert!(err.to_string().contains("ghost_provider"));
}

#[test]
fn resolve_providers_rejects_unknown_in_explicit_override() {
    let cfg = default_config();
    let err = cfg
        .resolve_providers(&["nonexistent_provider".to_string()])
        .unwrap_err();
    assert!(err.to_string().contains("unknown provider"), "got: {err}");
}

#[test]
fn resolve_providers_rejects_disabled_provider_in_explicit_override() {
    let mut cfg = default_config();
    cfg.search.providers.insert("mojeek".to_string(), false);
    let err = cfg.resolve_providers(&["mojeek".to_string()]).unwrap_err();
    assert!(err.to_string().contains("disabled"), "got: {err}");
}

#[test]
fn validate_rejects_zero_fetch_timeout_ms() {
    let mut cfg = default_config();
    cfg.fetch.timeout_ms = 0;
    let err = cfg.validate().unwrap_err();
    assert!(err.to_string().contains("timeout_ms"));
}

#[test]
fn validate_rejects_zero_search_timeout_ms() {
    let mut cfg = default_config();
    cfg.search.timeout_ms = 0;
    let err = cfg.validate().unwrap_err();
    assert!(err.to_string().contains("timeout_ms"));
}

#[test]
fn validate_rejects_zero_max_bytes() {
    let mut cfg = default_config();
    cfg.fetch.max_bytes = 0;
    let err = cfg.validate().unwrap_err();
    assert!(err.to_string().contains("max_bytes"));
}

#[test]
fn validate_rejects_max_chars_cap_below_default() {
    let mut cfg = default_config();
    cfg.fetch.max_chars_cap = 100;
    cfg.fetch.max_chars_default = 12_000;
    let err = cfg.validate().unwrap_err();
    assert!(err.to_string().contains("max_chars_cap"));
}

#[test]
fn validate_rejects_max_results_cap_below_default() {
    let mut cfg = default_config();
    cfg.search.default_max_results = 50;
    cfg.search.max_results_cap = 10;
    let err = cfg.validate().unwrap_err();
    assert!(err.to_string().contains("default_max_results"));
}

#[test]
fn validate_rejects_zero_default_max_results() {
    let mut cfg = default_config();
    cfg.search.default_max_results = 0;
    let err = cfg.validate().unwrap_err();
    assert!(err.to_string().contains("default_max_results"));
}

#[test]
fn validate_rejects_zero_max_results_cap() {
    let mut cfg = default_config();
    cfg.search.max_results_cap = 0;
    let err = cfg.validate().unwrap_err();
    assert!(err.to_string().contains("max_results_cap"));
}

#[test]
fn validate_rejects_zero_max_query_chars() {
    let mut cfg = default_config();
    cfg.search.max_query_chars = 0;
    let err = cfg.validate().unwrap_err();
    assert!(err.to_string().contains("max_query_chars"));
}

#[test]
fn validate_accepts_defaults() {
    let cfg = default_config();
    assert!(cfg.validate().is_ok());
}

#[test]
fn validate_rejects_live_mode_with_no_providers_and_no_api() {
    let mut cfg = default_config();
    cfg.search.mode = Mode::Live;
    for key in cfg.search.providers.keys().cloned().collect::<Vec<_>>() {
        cfg.search.providers.insert(key, false);
    }
    let err = cfg.validate().unwrap_err();
    assert!(
        err.to_string().contains("no traditional providers")
            || err.to_string().contains("no API providers"),
        "got: {err}"
    );
}

#[test]
fn validate_allows_off_mode_with_no_providers() {
    let mut cfg = default_config();
    cfg.search.mode = Mode::Off;
    for key in cfg.search.providers.keys().cloned().collect::<Vec<_>>() {
        cfg.search.providers.insert(key, false);
    }
    assert!(cfg.validate().is_ok());
}

#[test]
fn default_config_has_expected_values() {
    let cfg = default_config();
    assert_eq!(cfg.search.mode, Mode::Live);
    assert_eq!(cfg.search.default_max_results, 10);
    assert_eq!(cfg.search.max_results_cap, 50);
    assert_eq!(cfg.search.max_query_chars, 512);
    assert_eq!(cfg.search.timeout_ms, 8000);
    assert!(cfg.search.sanitize_output);
    assert!(cfg.fetch.enabled);
    assert_eq!(cfg.fetch.timeout_ms, 8000);
    assert_eq!(cfg.fetch.max_bytes, 2_000_000);
    assert_eq!(cfg.fetch.max_chars_default, 12_000);
    assert_eq!(cfg.fetch.max_chars_cap, 50_000);
    assert_eq!(cfg.fetch.redirect_limit, 5);
    assert!(!cfg.fetch.allow_private_network);
    assert!(!cfg.fetch.allow_localhost);
    assert!(cfg.fetch.sanitize_output);
    assert!(!cfg.fetch.pdf_enabled);
    assert!(!cfg.local.enabled);
    assert!(cfg.local.roots.is_empty());
}

#[test]
fn default_search_section_has_all_expected_providers() {
    let cfg = default_config();
    assert_eq!(cfg.search.providers.get("duckduckgo"), Some(&true));
    assert_eq!(cfg.search.providers.get("brave"), Some(&true));
    assert_eq!(cfg.search.providers.get("startpage"), Some(&true));
    assert_eq!(cfg.search.providers.get("yahoo"), Some(&true));
    assert_eq!(cfg.search.providers.get("mojeek"), Some(&false));
    assert_eq!(cfg.search.providers.get("searxng"), Some(&false));
}

#[test]
fn default_providers_match_docs() {
    let cfg = default_config();
    assert_eq!(
        cfg.search.default_providers,
        vec!["duckduckgo", "startpage", "yahoo"]
    );
}

#[test]
fn round_trip_preserves_all_fields() {
    let cfg = default_config();
    let text = toml::to_string(&cfg).unwrap();
    let parsed: AppConfig = toml::from_str(&text).unwrap();
    assert_eq!(parsed.search.mode, cfg.search.mode);
    assert_eq!(
        parsed.search.default_max_results,
        cfg.search.default_max_results
    );
    assert_eq!(parsed.search.max_results_cap, cfg.search.max_results_cap);
    assert_eq!(parsed.search.max_query_chars, cfg.search.max_query_chars);
    assert_eq!(parsed.search.timeout_ms, cfg.search.timeout_ms);
    assert_eq!(
        parsed.search.default_providers,
        cfg.search.default_providers
    );
    assert_eq!(parsed.search.sanitize_output, cfg.search.sanitize_output);
    assert_eq!(parsed.fetch.enabled, cfg.fetch.enabled);
    assert_eq!(parsed.fetch.timeout_ms, cfg.fetch.timeout_ms);
    assert_eq!(parsed.fetch.max_bytes, cfg.fetch.max_bytes);
    assert_eq!(parsed.fetch.max_chars_default, cfg.fetch.max_chars_default);
    assert_eq!(parsed.fetch.max_chars_cap, cfg.fetch.max_chars_cap);
    assert_eq!(parsed.fetch.redirect_limit, cfg.fetch.redirect_limit);
    assert_eq!(
        parsed.fetch.allow_private_network,
        cfg.fetch.allow_private_network
    );
    assert_eq!(parsed.fetch.allow_localhost, cfg.fetch.allow_localhost);
    assert_eq!(parsed.fetch.sanitize_output, cfg.fetch.sanitize_output);
    assert_eq!(parsed.fetch.pdf_enabled, cfg.fetch.pdf_enabled);
    assert_eq!(parsed.local.enabled, cfg.local.enabled);
}

#[test]
fn toml_parse_minimal_config() {
    let toml = r#"
[search]
mode = "live"
default_max_results = 5
max_results_cap = 20
max_query_chars = 256
timeout_ms = 4000
default_providers = ["duckduckgo"]

[search.providers]
duckduckgo = true
"#;
    let cfg: AppConfig = toml::from_str(toml).unwrap();
    assert_eq!(cfg.search.default_max_results, 5);
    assert_eq!(cfg.search.max_results_cap, 20);
    assert_eq!(cfg.search.timeout_ms, 4000);
    assert_eq!(cfg.search.default_providers, vec!["duckduckgo"]);
}

#[test]
fn toml_parse_fetch_overrides() {
    let toml = r#"
[fetch]
enabled = false
timeout_ms = 15000
max_bytes = 5000000
max_chars_default = 20000
max_chars_cap = 100000
redirect_limit = 10
allow_private_network = true
allow_localhost = true
sanitize_output = false
pdf_enabled = true
pdf_max_pages = 50
pdf_max_chars_per_page = 24000
pdf_max_total_chars = 100000
"#;
    let cfg: AppConfig = toml::from_str(toml).unwrap();
    assert!(!cfg.fetch.enabled);
    assert_eq!(cfg.fetch.timeout_ms, 15000);
    assert_eq!(cfg.fetch.max_bytes, 5_000_000);
    assert_eq!(cfg.fetch.max_chars_default, 20_000);
    assert_eq!(cfg.fetch.max_chars_cap, 100_000);
    assert_eq!(cfg.fetch.redirect_limit, 10);
    assert!(cfg.fetch.allow_private_network);
    assert!(cfg.fetch.allow_localhost);
    assert!(!cfg.fetch.sanitize_output);
    assert!(cfg.fetch.pdf_enabled);
    assert_eq!(cfg.fetch.pdf_max_pages, 50);
    assert_eq!(cfg.fetch.pdf_max_chars_per_page, 24_000);
    assert_eq!(cfg.fetch.pdf_max_total_chars, 100_000);
}

#[test]
fn toml_parse_local_workspace() {
    let toml = r#"
[local]
enabled = true
roots = ["/tmp/workspace"]
max_file_bytes = 2097152
max_indexed_files = 100000
include_hidden = true
respect_gitignore = false
follow_symlinks = true
"#;
    let cfg: AppConfig = toml::from_str(toml).unwrap();
    assert!(cfg.local.enabled);
    assert_eq!(cfg.local.roots.len(), 1);
    assert_eq!(cfg.local.max_file_bytes, 2_097_152);
    assert_eq!(cfg.local.max_indexed_files, 100_000);
    assert!(cfg.local.include_hidden);
    assert!(!cfg.local.respect_gitignore);
    assert!(cfg.local.follow_symlinks);
}

#[test]
fn validate_rejects_local_enabled_without_roots() {
    let mut cfg = default_config();
    cfg.local.enabled = true;
    cfg.local.roots.clear();
    let err = cfg.validate().unwrap_err();
    assert!(
        err.to_string().contains("[local]") && err.to_string().contains("roots"),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_zero_local_max_file_bytes() {
    let mut cfg = default_config();
    cfg.local.enabled = true;
    cfg.local.roots = vec!["/tmp".into()];
    cfg.local.max_file_bytes = 0;
    let err = cfg.validate().unwrap_err();
    assert!(err.to_string().contains("max_file_bytes"), "got: {err}");
}

#[test]
fn validate_rejects_zero_local_max_indexed_files() {
    let mut cfg = default_config();
    cfg.local.enabled = true;
    cfg.local.roots = vec!["/tmp".into()];
    cfg.local.max_indexed_files = 0;
    let err = cfg.validate().unwrap_err();
    assert!(err.to_string().contains("max_indexed_files"), "got: {err}");
}

#[test]
fn validate_rejects_batch_max_items_cap_below_default() {
    let mut cfg = default_config();
    cfg.fetch.batch_max_items = 10;
    cfg.fetch.batch_max_items_cap = 5;
    let err = cfg.validate().unwrap_err();
    assert!(
        err.to_string().contains("batch_max_items_cap"),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_batch_max_total_chars_cap_below_default() {
    let mut cfg = default_config();
    cfg.fetch.batch_max_total_chars = 50000;
    cfg.fetch.batch_max_total_chars_cap = 10000;
    let err = cfg.validate().unwrap_err();
    assert!(
        err.to_string().contains("batch_max_total_chars_cap"),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_zero_batch_max_items() {
    let mut cfg = default_config();
    cfg.fetch.batch_max_items = 0;
    let err = cfg.validate().unwrap_err();
    assert!(err.to_string().contains("batch_max_items"), "got: {err}");
}

#[test]
fn validate_rejects_zero_batch_concurrency() {
    let mut cfg = default_config();
    cfg.fetch.batch_concurrency = 0;
    let err = cfg.validate().unwrap_err();
    assert!(err.to_string().contains("batch_concurrency"), "got: {err}");
}

#[test]
fn api_provider_rejects_missing_key_env_when_enabled() {
    let mut cfg = default_config();
    cfg.search.api.insert(
        "brave_api".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: None,
            base_url: None,
        },
    );
    let err = cfg.validate().unwrap_err();
    assert!(err.to_string().contains("api_key_env"), "got: {err}");
}

#[test]
fn api_provider_rejects_empty_key_env_when_enabled() {
    let mut cfg = default_config();
    cfg.search.api.insert(
        "brave_api".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: Some(String::new()),
            base_url: None,
        },
    );
    let err = cfg.validate().unwrap_err();
    assert!(err.to_string().contains("api_key_env"), "got: {err}");
}

#[test]
fn api_provider_accepts_disabled_without_key_env() {
    let mut cfg = default_config();
    cfg.search.api.insert(
        "brave_api".to_string(),
        ApiProviderConfig {
            enabled: false,
            api_key_env: None,
            base_url: None,
        },
    );
    assert!(cfg.validate().is_ok());
}

#[test]
fn api_provider_rejects_invalid_base_url() {
    let mut cfg = default_config();
    cfg.search.api.insert(
        "brave_api".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: Some("BRAVE_KEY".to_string()),
            base_url: Some("not a url".to_string()),
        },
    );
    let err = cfg.validate().unwrap_err();
    assert!(err.to_string().contains("not a valid URL"), "got: {err}");
}

#[test]
fn searxng_rejects_enabled_without_base_url() {
    let mut cfg = default_config();
    cfg.search.searxng.enabled = true;
    cfg.search.searxng.base_url = None;
    let err = cfg.validate().unwrap_err();
    assert!(err.to_string().contains("base_url"), "got: {err}");
}

#[test]
fn searxng_rejects_enabled_with_empty_base_url() {
    let mut cfg = default_config();
    cfg.search.searxng.enabled = true;
    cfg.search.searxng.base_url = Some(String::new());
    let err = cfg.validate().unwrap_err();
    assert!(err.to_string().contains("base_url"), "got: {err}");
}

#[test]
fn searxng_rejects_invalid_base_url() {
    let mut cfg = default_config();
    cfg.search.searxng.enabled = true;
    cfg.search.searxng.base_url = Some("not a url".to_string());
    let err = cfg.validate().unwrap_err();
    assert!(err.to_string().contains("not a valid URL"), "got: {err}");
}

#[test]
fn searxng_accepts_valid_config() {
    let mut cfg = default_config();
    cfg.search.searxng.enabled = true;
    cfg.search.searxng.base_url = Some("https://searx.example.org".to_string());
    assert!(cfg.validate().is_ok());
}

#[test]
fn mode_parse_only_off_and_live_accepted() {
    assert_eq!(Mode::parse("off").unwrap(), Mode::Off);
    assert_eq!(Mode::parse("live").unwrap(), Mode::Live);
    assert!(Mode::parse("ask").is_err());
    assert!(Mode::parse("local").is_err());
    assert!(Mode::parse("LOCAL_ONLY").is_err());
    assert!(Mode::parse("invalid").is_err());
}

#[test]
fn all_known_provider_ids_are_in_known_list() {
    for id in &[
        "duckduckgo",
        "brave",
        "startpage",
        "yahoo",
        "mojeek",
        "searxng",
        "brave_api",
        "github_code",
        "github_issues",
        "github_releases",
        "gitlab_code",
        "gitlab_issues",
        "gitlab_releases",
        "gitea_code",
        "gitea_issues",
        "gitea_releases",
        "osv",
        "github_advisory",
        "nvd",
        "cisa_kev",
        "rustsec",
        "local_workspace",
        "crates_io",
        "pypi",
        "npm_registry",
        "go_pkg",
        "maven_central",
        "nuget",
        "rubygems",
        "packagist",
        "openalex",
        "crossref",
        "semantic_scholar",
        "sourcegraph",
    ] {
        assert!(
            KNOWN_PROVIDER_IDS.contains(id),
            "provider '{id}' not in KNOWN_PROVIDER_IDS"
        );
    }
}

#[test]
fn validate_accepts_extreme_but_valid_config() {
    let mut cfg = default_config();
    cfg.search.default_max_results = 1;
    cfg.search.max_results_cap = 1;
    cfg.search.max_query_chars = 1;
    cfg.search.timeout_ms = 1;
    cfg.fetch.timeout_ms = 1;
    cfg.fetch.max_bytes = 1;
    cfg.fetch.max_chars_default = 1;
    cfg.fetch.max_chars_cap = 1;
    cfg.fetch.redirect_limit = 0;
    cfg.fetch.batch_max_items = 1;
    cfg.fetch.batch_max_items_cap = 1;
    cfg.fetch.batch_max_chars_per_item = 1;
    cfg.fetch.batch_max_total_chars = 1;
    cfg.fetch.batch_max_total_chars_cap = 1;
    cfg.fetch.batch_concurrency = 1;
    assert!(cfg.validate().is_ok());
}

#[test]
fn resolve_providers_deduplicates_explicit_override() {
    let cfg = default_config();
    let result = cfg.resolve_providers(&[
        "duckduckgo".to_string(),
        "duckduckgo".to_string(),
        "startpage".to_string(),
    ]);
    assert_eq!(
        result.unwrap(),
        vec!["duckduckgo".to_string(), "startpage".to_string()]
    );
}

#[test]
fn resolve_providers_preserves_explicit_order() {
    let cfg = default_config();
    let result = cfg
        .resolve_providers(&["yahoo".to_string(), "duckduckgo".to_string()])
        .unwrap();
    assert_eq!(result, vec!["yahoo".to_string(), "duckduckgo".to_string()]);
}

#[test]
fn resolve_providers_returns_defaults_when_empty() {
    let cfg = default_config();
    let result = cfg.resolve_providers(&[]).unwrap();
    assert_eq!(result, cfg.search.default_providers);
}

#[test]
fn fetch_limits_derived_from_config() {
    let mut cfg = default_config();
    cfg.fetch.max_bytes = 1_000_000;
    cfg.fetch.max_chars_default = 5_000;
    cfg.fetch.max_chars_cap = 20_000;
    cfg.fetch.timeout_ms = 10_000;
    cfg.fetch.redirect_limit = 3;
    cfg.fetch.allow_private_network = true;
    cfg.fetch.allow_localhost = true;
    cfg.fetch.pdf_enabled = true;
    cfg.fetch.pdf_max_pages = 10;
    cfg.fetch.pdf_max_chars_per_page = 6_000;
    cfg.fetch.pdf_max_total_chars = 30_000;
    let limits = cfg.fetch_limits();
    assert_eq!(limits.max_bytes, 1_000_000);
    assert_eq!(limits.max_chars_default, 5_000);
    assert_eq!(limits.max_chars_cap, 20_000);
    assert_eq!(limits.timeout_ms, 10_000);
    assert_eq!(limits.redirect_limit, 3);
    assert!(limits.allow_private_network);
    assert!(limits.allow_localhost);
    assert!(limits.pdf_enabled);
    assert_eq!(limits.pdf_max_pages, 10);
    assert_eq!(limits.pdf_max_chars_per_page, 6_000);
    assert_eq!(limits.pdf_max_total_chars, 30_000);
}

#[test]
fn malformed_toml_returns_error() {
    let err = toml::from_str::<AppConfig>("this is not valid toml {{{").unwrap_err();
    assert!(!err.to_string().is_empty());
}

#[test]
fn empty_toml_uses_defaults() {
    let cfg: AppConfig = toml::from_str("").unwrap();
    assert_eq!(cfg.search.mode, Mode::default());
    assert_eq!(cfg.search.default_max_results, 10);
    assert_eq!(cfg.fetch.timeout_ms, 8000);
    assert!(!cfg.local.enabled);
}

#[test]
fn empty_providers_list_rejected_by_validate_in_live_mode() {
    let mut cfg = default_config();
    cfg.search.mode = Mode::Live;
    cfg.search.default_providers.clear();
    for key in cfg.search.providers.keys().cloned().collect::<Vec<_>>() {
        cfg.search.providers.insert(key, false);
    }
    let err = cfg.validate().unwrap_err();
    assert!(
        err.to_string().contains("no traditional providers")
            || err.to_string().contains("no API providers"),
        "got: {err}"
    );
}

#[test]
fn provider_is_available_for_enabled_provider() {
    let cfg = default_config();
    assert!(cfg.provider_is_available("duckduckgo"));
    assert!(cfg.provider_is_available("brave"));
    assert!(cfg.provider_is_available("startpage"));
    assert!(cfg.provider_is_available("yahoo"));
}

#[test]
fn provider_not_available_for_disabled_provider() {
    let cfg = default_config();
    assert!(!cfg.provider_is_available("mojeek"));
    assert!(!cfg.provider_is_available("searxng"));
}

#[test]
fn provider_not_available_for_unknown_id() {
    let cfg = default_config();
    assert!(!cfg.provider_is_available("nonexistent"));
}

#[test]
fn enabled_provider_ids_excludes_disabled() {
    let mut cfg = default_config();
    cfg.search.providers.insert("brave".to_string(), false);
    let ids = cfg.enabled_provider_ids();
    assert!(!ids.contains(&"brave".to_string()));
    assert!(ids.contains(&"duckduckgo".to_string()));
}

#[test]
fn misconfigured_default_providers_lists_unknown() {
    let mut cfg = default_config();
    cfg.search.default_providers = vec!["duckduckgo".to_string(), "ghost_provider".to_string()];
    let mis = cfg.misconfigured_default_providers();
    assert!(mis.contains(&"ghost_provider".to_string()));
}

#[test]
fn fetch_user_agent_from_config() {
    let cfg = default_config();
    let ua = cfg.fetch_user_agent();
    assert!(ua.contains("eggsearch"));
}

#[test]
fn save_and_load_roundtrip_through_filesystem() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let cfg = default_config();
    cfg.save(&path).unwrap();
    let loaded = AppConfig::load(&path).unwrap();
    assert_eq!(loaded.search.mode, cfg.search.mode);
    assert_eq!(
        loaded.search.default_max_results,
        cfg.search.default_max_results
    );
    assert_eq!(loaded.fetch.timeout_ms, cfg.fetch.timeout_ms);
}

#[test]
fn load_missing_file_returns_defaults() {
    let path = std::path::Path::new("/nonexistent/eggsearch_test_config.toml");
    let cfg = AppConfig::load(path).unwrap();
    assert_eq!(cfg.search.mode, Mode::default());
}
