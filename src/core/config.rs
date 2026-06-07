//! Configuration model and loader for eggsearch.
//!
//! The changeover configuration is intentionally minimal: the default
//! server is a live metasearch-only MCP server. Tantivy and web_fetch
//! are deferred behind feature flags.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::error::{CoreError, CoreResult};

/// Server operating mode.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// All tools disabled.
    Off,
    /// Live metasearch is allowed.
    #[default]
    Live,
}

impl Mode {
    /// Parse a mode string. Only `"off"` and `"live"` are accepted; `"ask"`
    /// is a host-level policy and is not a valid value at this layer.
    pub fn parse(s: &str) -> CoreResult<Self> {
        match s.to_lowercase().as_str() {
            "off" => Ok(Self::Off),
            "live" => Ok(Self::Live),
            other => Err(CoreError::Config(format!("unknown mode: {other}"))),
        }
    }
}

/// Live network configuration. Both fields are NO-OPs in the current
/// build: they are parsed from TOML so that operator configs continue
/// to load, but they are not read by the runtime. Setting them will
/// emit a `tracing::warn!` at startup and otherwise have no effect.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LiveConfig {
    /// **NO-OP in the current build.** Reserved for future use. The
    /// vendored HTML engines hard-code a browser-like user agent that
    /// the upstream providers expect; the operator cannot override it
    /// from config yet. A warning is logged at startup if this is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    /// **NO-OP in the current build.** Reserved for future use. The
    /// `web_fetch` tool does not consult robots.txt; setting this to
    /// `true` has no effect on fetching behavior. A warning is logged
    /// at startup if this is set to `true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub respect_robots_txt: Option<bool>,
}

/// Configuration for the optional SearXNG upstream adapter. SearXNG is
/// disabled by default; when `enabled = true` and `base_url` is set, the
/// `searxng` provider id becomes available and points at the operator's
/// self-hosted instance.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SearxngConfig {
    /// Whether the `searxng` provider is enabled. The provider is only
    /// built when both this flag and `[search].providers.searxng = true`
    /// are set and `base_url` is non-empty.
    #[serde(default)]
    pub enabled: bool,
    /// Base URL of the SearXNG instance, e.g. `https://searx.example.org`.
    /// The trailing slash is optional. The engine appends `/search`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

/// The `[search]` section of the eggsearch configuration file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchSection {
    /// Server mode: `off` or `live`. Defaults to `live`.
    pub mode: Mode,
    /// Default `max_results` for `web_search` when not specified.
    pub max_results: usize,
    /// Hard cap on `max_results` from clients.
    pub max_results_cap: usize,
    /// Maximum accepted query length in characters.
    pub max_query_chars: usize,
    /// Default per-request timeout in milliseconds.
    pub timeout_ms: u64,
    /// Default providers to query when none are specified.
    pub default_providers: Vec<String>,
    /// Per-provider enable/disable flags. Keys are provider ids
    /// (`duckduckgo`, `brave`, `startpage`, `yahoo`, `mojeek`, `searxng`).
    #[serde(default)]
    pub providers: std::collections::BTreeMap<String, bool>,
    /// Optional SearXNG upstream configuration. Only consulted when
    /// the `searxng` provider is also enabled in `providers`.
    #[serde(default)]
    pub searxng: SearxngConfig,
    /// Live network configuration. Most fields are reserved for future
    /// use; see `LiveConfig` docs.
    pub live: LiveConfig,
    /// Whether to wrap untrusted search-result text (titles,
    /// snippets) in `<<<EXTERNAL_UNTRUSTED ...>>>` framing
    /// delimiters and emit per-response prompt-injection warnings.
    /// Tier 1 (control-char stripping + length bounding) is always on;
    /// this flag gates Tier 2 (framing) and Tier 3 (marker scan).
    /// Default: `true`.
    #[serde(default = "default_sanitize_output")]
    pub sanitize_output: bool,
}

impl Default for SearchSection {
    fn default() -> Self {
        let mut providers = std::collections::BTreeMap::new();
        providers.insert("duckduckgo".to_string(), true);
        providers.insert("brave".to_string(), true);
        providers.insert("startpage".to_string(), true);
        providers.insert("yahoo".to_string(), true);
        providers.insert("mojeek".to_string(), false);
        providers.insert("searxng".to_string(), false);
        Self {
            mode: Mode::default(),
            max_results: 10,
            max_results_cap: 50,
            max_query_chars: 512,
            timeout_ms: 8000,
            default_providers: vec![
                "duckduckgo".to_string(),
                "startpage".to_string(),
                "yahoo".to_string(),
            ],
            providers,
            searxng: SearxngConfig::default(),
            live: LiveConfig::default(),
            sanitize_output: default_sanitize_output(),
        }
    }
}

fn default_true() -> bool {
    true
}
fn default_fetch_timeout() -> u64 {
    8000
}
fn default_max_bytes() -> usize {
    2_000_000
}
fn default_max_chars_default() -> usize {
    12000
}
fn default_max_chars_cap() -> usize {
    50000
}
fn default_redirect_limit() -> usize {
    5
}
fn default_user_agent() -> String {
    "eggsearch/0.1 (+https://github.com/eggstack/eggsearch)".to_string()
}
fn default_sanitize_output() -> bool {
    true
}

/// The `[fetch]` section of the eggsearch configuration file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FetchSection {
    /// Whether fetch is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Fetch timeout in milliseconds.
    #[serde(default = "default_fetch_timeout")]
    pub timeout_ms: u64,
    /// Maximum content size in bytes.
    #[serde(default = "default_max_bytes")]
    pub max_bytes: usize,
    /// Default maximum characters to extract.
    #[serde(default = "default_max_chars_default")]
    pub max_chars_default: usize,
    /// Maximum character extraction cap.
    #[serde(default = "default_max_chars_cap")]
    pub max_chars_cap: usize,
    /// Maximum number of redirects.
    #[serde(default = "default_redirect_limit")]
    pub redirect_limit: usize,
    /// Whether to allow private network access.
    #[serde(default)]
    pub allow_private_network: bool,
    /// Whether to allow localhost access.
    #[serde(default)]
    pub allow_localhost: bool,
    /// Default for whether to include links.
    #[serde(default)]
    pub include_links_default: bool,
    /// User agent string for HTTP requests.
    #[serde(default = "default_user_agent")]
    pub user_agent: String,
    /// Whether to wrap untrusted fetched text (title, description,
    /// body) in `<<<EXTERNAL_UNTRUSTED ...>>>` framing delimiters
    /// and emit per-response prompt-injection warnings. Tier 1
    /// (control-char stripping + length bounding) is always on; this
    /// flag gates Tier 2 (framing) and Tier 3 (marker scan).
    /// Default: `true`.
    #[serde(default = "default_sanitize_output")]
    pub sanitize_output: bool,
}

impl Default for FetchSection {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout_ms: default_fetch_timeout(),
            max_bytes: default_max_bytes(),
            max_chars_default: default_max_chars_default(),
            max_chars_cap: default_max_chars_cap(),
            redirect_limit: default_redirect_limit(),
            allow_private_network: false,
            allow_localhost: false,
            include_links_default: false,
            user_agent: default_user_agent(),
            sanitize_output: default_sanitize_output(),
        }
    }
}

/// Root configuration type. Mirrors the structure of the TOML file
/// loaded from [`default_config_path`] or a user-supplied path.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AppConfig {
    /// The `[search]` section.
    #[serde(default)]
    pub search: SearchSection,
    /// The `[fetch]` section.
    #[serde(default)]
    pub fetch: FetchSection,
}

impl AppConfig {
    /// Load a config from the given TOML file path, falling back to defaults
    /// for any missing sections.
    ///
    /// # Examples
    ///
    /// ```
    /// use eggsearch::core::AppConfig;
    ///
    /// // If the file does not exist, defaults are returned silently.
    /// let cfg = AppConfig::load(std::path::Path::new("/nonexistent/config.toml"))
    ///     .expect("missing file returns defaults");
    /// assert_eq!(cfg.search.timeout_ms, 8_000);
    /// assert!(!cfg.search.default_providers.is_empty());
    /// ```
    pub fn load(path: &Path) -> CoreResult<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)?;
        let cfg: Self = toml::from_str(&text)?;
        Ok(cfg)
    }

    /// Save the config to the given path. Creates parent dirs as needed.
    pub fn save(&self, path: &Path) -> CoreResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self).map_err(|e| CoreError::TomlSer(e.to_string()))?;
        std::fs::write(path, text)?;
        Ok(())
    }

    /// Resolve the effective provider list for a request.
    /// If override_list is empty, uses default_providers filtered to only enabled.
    /// If override_list is non-empty, validates explicitly-disabled providers and deduplicates.
    pub fn resolve_providers(&self, override_list: &[String]) -> CoreResult<Vec<String>> {
        let enabled_ids: Vec<String> = self.enabled_provider_ids();
        let enabled: std::collections::BTreeSet<&str> =
            enabled_ids.iter().map(|s| s.as_str()).collect();

        if override_list.is_empty() {
            let defaults: Vec<String> = self
                .search
                .default_providers
                .iter()
                .filter(|id| enabled.contains(id.as_str()))
                .cloned()
                .collect();
            if defaults.is_empty() {
                return Err(CoreError::Config(
                    "no default providers are enabled; check [search].providers".into(),
                ));
            }
            Ok(defaults)
        } else {
            // Dedupe while preserving order
            let mut seen = std::collections::HashSet::new();
            let mut deduped = Vec::new();
            for p in override_list {
                if seen.insert(p.clone()) {
                    deduped.push(p.clone());
                }
            }

            // Check for explicitly DISABLED providers (config key exists with value false)
            let explicitly_disabled: Vec<String> = deduped
                .iter()
                .filter(|id| self.search.providers.get(*id).is_some_and(|v| !*v))
                .cloned()
                .collect();
            if !explicitly_disabled.is_empty() {
                return Err(CoreError::Config(format!(
                    "provider(s) not enabled: {}; enable them in [search].providers or remove them from request",
                    explicitly_disabled.join(", ")
                )));
            }
            Ok(deduped)
        }
    }

    /// Returns provider IDs that are explicitly enabled (value = true) in the providers map.
    pub fn enabled_provider_ids(&self) -> Vec<String> {
        self.search
            .providers
            .iter()
            .filter(|(_, enabled)| **enabled)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Returns the provider ids listed in `default_providers` that are
    /// not enabled in `search.providers`. These are silently filtered
    /// out by `resolve_providers`; operators should be told at startup
    /// so they can fix the config.
    pub fn misconfigured_default_providers(&self) -> Vec<String> {
        let enabled_ids = self.enabled_provider_ids();
        let enabled: std::collections::BTreeSet<&str> =
            enabled_ids.iter().map(|s| s.as_str()).collect();
        self.search
            .default_providers
            .iter()
            .filter(|id| !enabled.contains(id.as_str()))
            .cloned()
            .collect()
    }

    /// Returns fetch limits based on config.
    pub fn fetch_limits(&self) -> crate::fetch::limits::FetchLimits {
        crate::fetch::limits::FetchLimits {
            max_url_len: 8192,
            max_bytes: self.fetch.max_bytes,
            max_chars_default: self.fetch.max_chars_default,
            max_chars_cap: self.fetch.max_chars_cap,
            timeout_ms: self.fetch.timeout_ms,
            redirect_limit: self.fetch.redirect_limit,
            allow_private_network: self.fetch.allow_private_network,
            allow_localhost: self.fetch.allow_localhost,
        }
    }

    /// Validate configuration invariants. Returns `Err` on the first
    /// violated invariant. The intent is to fail fast on operator
    /// misconfiguration (e.g. `max_chars_cap < max_chars_default`)
    /// rather than silently degrading behavior.
    pub fn validate(&self) -> CoreResult<()> {
        if self.fetch.max_chars_cap < self.fetch.max_chars_default {
            return Err(CoreError::Config(format!(
                "[fetch].max_chars_cap ({}) must be >= [fetch].max_chars_default ({})",
                self.fetch.max_chars_cap, self.fetch.max_chars_default
            )));
        }
        if self.fetch.max_bytes == 0 {
            return Err(CoreError::Config(
                "[fetch].max_bytes must be > 0".to_string(),
            ));
        }
        if self.fetch.timeout_ms == 0 {
            return Err(CoreError::Config(
                "[fetch].timeout_ms must be > 0".to_string(),
            ));
        }
        if self.search.max_results == 0 {
            return Err(CoreError::Config(
                "[search].max_results must be > 0".to_string(),
            ));
        }
        if self.search.max_results_cap < self.search.max_results {
            return Err(CoreError::Config(format!(
                "[search].max_results_cap ({}) must be >= [search].max_results ({})",
                self.search.max_results_cap, self.search.max_results
            )));
        }
        if self.search.timeout_ms == 0 {
            return Err(CoreError::Config(
                "[search].timeout_ms must be > 0".to_string(),
            ));
        }
        if self.search.max_query_chars == 0 {
            return Err(CoreError::Config(
                "[search].max_query_chars must be > 0".to_string(),
            ));
        }
        if self.search.mode == Mode::Live {
            let enabled_count = self.search.providers.values().filter(|v| **v).count();
            if enabled_count == 0 {
                return Err(CoreError::Config(
                    "[search].mode is 'live' but no providers are enabled in [search].providers"
                        .to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Returns the configured user agent for fetch.
    pub fn fetch_user_agent(&self) -> String {
        self.fetch.user_agent.clone()
    }
}

/// Resolve the platform-specific default config path.
///
/// Honors `$XDG_CONFIG_HOME` on Linux, `~/Library/Application Support`
/// on macOS, and `%APPDATA%` on Windows. Falls back to the literal
/// string `eggsearch.toml` in the current working directory if no
/// platform config dir is available.
pub fn default_config_path() -> PathBuf {
    if let Some(dir) = dirs::config_dir() {
        return dir.join("eggsearch").join("config.toml");
    }
    PathBuf::from("eggsearch.toml")
}

#[cfg(test)]
mod tests {
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
        assert!(c.search.max_results > 0);
        assert!(!c.search.default_providers.is_empty());
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
    fn default_mojeek_is_disabled() {
        let c = AppConfig::default();
        assert_eq!(c.search.providers.get("mojeek"), Some(&false));
    }

    #[test]
    fn round_trip_toml() {
        let c = AppConfig::default();
        let text = toml::to_string(&c).unwrap();
        let parsed: AppConfig = toml::from_str(&text).unwrap();
        assert_eq!(parsed.search.max_results, c.search.max_results);
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
        assert!(result.unwrap_err().to_string().contains("not enabled"));
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
        assert!(out.unwrap_err().to_string().contains("not enabled"));
    }

    #[test]
    fn save_load_round_trip_through_filesystem() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let c = AppConfig::default();
        c.save(&path).unwrap();
        let loaded = AppConfig::load(&path).unwrap();
        assert_eq!(loaded.search.max_results, c.search.max_results);
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
    fn validate_rejects_zero_max_results() {
        let mut c = AppConfig::default();
        c.search.max_results = 0;
        let err = c.validate().expect_err("expected max_results failure");
        assert!(err.to_string().contains("max_results"), "got: {err}");
    }

    #[test]
    fn validate_rejects_max_results_cap_below_max_results() {
        let mut c = AppConfig::default();
        c.search.max_results = 50;
        c.search.max_results_cap = 10;
        let err = c.validate().expect_err("expected cap failure");
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
            err.to_string().contains("no providers are enabled"),
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
}
