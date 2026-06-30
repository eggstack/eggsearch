//! Configuration model and loader for eggsearch.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::error::{CoreError, CoreResult};
use crate::core::provider::KNOWN_PROVIDER_IDS;

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

/// Configuration for an API-key backed search provider (e.g. Brave
/// Search API). The key itself is stored in an environment variable;
/// the config only references the env var name.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ApiProviderConfig {
    /// Whether this API provider is enabled.
    pub enabled: bool,
    /// Name of the environment variable that holds the API key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    /// Base URL for the API endpoint. When `None`, a provider-specific
    /// default is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

/// Configuration for a named search profile.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ProfileConfig {
    /// Ordered list of provider IDs for this profile.
    /// Providers are tried in order; unavailable providers are
    /// skipped with warnings.
    pub providers: Vec<String>,
}

/// The `[search]` section of the eggsearch configuration file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchSection {
    /// Server mode: `off` or `live`. Defaults to `live`.
    pub mode: Mode,
    /// Default number of results for `web_search` when not specified.
    #[serde(alias = "max_results")]
    pub default_max_results: usize,
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
    /// API-key backed provider configurations. Keys are provider ids
    /// (e.g. `brave_api`). Each entry specifies whether the provider
    /// is enabled and how to obtain its API key.
    #[serde(default)]
    pub api: std::collections::BTreeMap<String, ApiProviderConfig>,
    /// Live network configuration. Most fields are reserved for future
    /// use; see `LiveConfig` docs.
    #[serde(default)]
    pub live: LiveConfig,
    /// Whether to wrap untrusted search-result text (titles,
    /// snippets) in `<<<EXTERNAL_UNTRUSTED ...>>>` framing
    /// delimiters and emit per-response prompt-injection warnings.
    /// Tier 1 (control-char stripping + length bounding) is always on;
    /// this flag gates Tier 2 (framing) and Tier 3 (marker scan).
    /// Default: `true`.
    #[serde(default = "default_sanitize_output")]
    pub sanitize_output: bool,
    /// Named search profiles. Keys are profile names ("generic",
    /// "coding", "security", "research"). When a profile is not
    /// configured, built-in defaults are used.
    #[serde(default)]
    pub profiles: std::collections::BTreeMap<String, ProfileConfig>,
    /// Exact-error mode configuration for compiler/runtime error search.
    #[serde(default)]
    pub exact_error: crate::core::error_query::ExactErrorConfig,
    /// Maximum total in-flight (subquery, provider) jobs during
    /// parallel dispatch for multi-subquery searches (repo_search,
    /// research_search, security_search). Computed as
    /// `subqueries.clamp(1,8) * engines.clamp(1,4)` when not set.
    #[serde(default = "default_multiquery_concurrency")]
    pub multiquery_concurrency: usize,
    /// Maximum concurrent jobs for any single provider during parallel
    /// dispatch. Prevents one fast provider from being overwhelmed when
    /// many subqueries target it simultaneously.
    #[serde(default = "default_multiquery_provider_concurrency")]
    pub multiquery_provider_concurrency: usize,
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
        providers.insert("osv".to_string(), true);
        Self {
            mode: Mode::default(),
            default_max_results: 10,
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
            api: std::collections::BTreeMap::new(),
            live: LiveConfig::default(),
            sanitize_output: default_sanitize_output(),
            profiles: std::collections::BTreeMap::new(),
            exact_error: crate::core::error_query::ExactErrorConfig::default(),
            multiquery_concurrency: default_multiquery_concurrency(),
            multiquery_provider_concurrency: default_multiquery_provider_concurrency(),
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

fn default_multiquery_concurrency() -> usize {
    8
}

fn default_multiquery_provider_concurrency() -> usize {
    2
}
fn default_pdf_max_pages() -> usize {
    25
}
fn default_pdf_max_chars_per_page() -> usize {
    12000
}
fn default_pdf_max_total_chars() -> usize {
    50000
}
fn default_batch_max_items() -> usize {
    8
}
fn default_batch_max_items_cap() -> usize {
    20
}
fn default_batch_max_chars_per_item() -> usize {
    12000
}
fn default_batch_max_total_chars() -> usize {
    50000
}
fn default_batch_max_total_chars_cap() -> usize {
    120000
}
fn default_batch_concurrency() -> usize {
    4
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
    /// Whether PDF text extraction is enabled. Requires the `pdf`
    /// Cargo feature to be compiled in. When disabled, PDF
    /// responses are rejected with a clear message. Default: `false`.
    #[serde(default)]
    pub pdf_enabled: bool,
    /// Maximum number of PDF pages to attempt extracting text from.
    /// Default: 25.
    #[serde(default = "default_pdf_max_pages")]
    pub pdf_max_pages: usize,
    /// Maximum characters to extract per PDF page. Default: 12000.
    #[serde(default = "default_pdf_max_chars_per_page")]
    pub pdf_max_chars_per_page: usize,
    /// Maximum total characters to extract across all PDF pages.
    /// Default: 50000.
    #[serde(default = "default_pdf_max_total_chars")]
    pub pdf_max_total_chars: usize,

    // --- Batch fetch configuration ---
    /// Default maximum number of items in a batch fetch request.
    /// Callers can request fewer items but not more than `batch_max_items_cap`.
    /// Default: 8.
    #[serde(default = "default_batch_max_items")]
    pub batch_max_items: usize,
    /// Hard cap on the number of items in a batch fetch request.
    /// Requests exceeding this are rejected. Default: 20.
    #[serde(default = "default_batch_max_items_cap")]
    pub batch_max_items_cap: usize,
    /// Default per-item character extraction cap for batch fetch.
    /// Default: 12000.
    #[serde(default = "default_batch_max_chars_per_item")]
    pub batch_max_chars_per_item: usize,
    /// Default total character budget across all batch fetch items.
    /// Default: 50000.
    #[serde(default = "default_batch_max_total_chars")]
    pub batch_max_total_chars: usize,
    /// Hard cap on total characters across all batch fetch items.
    /// Requests exceeding this are rejected. Default: 120000.
    #[serde(default = "default_batch_max_total_chars_cap")]
    pub batch_max_total_chars_cap: usize,
    /// Maximum concurrent fetches in a batch request. Default: 4.
    #[serde(default = "default_batch_concurrency")]
    pub batch_concurrency: usize,
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
            pdf_enabled: false,
            pdf_max_pages: default_pdf_max_pages(),
            pdf_max_chars_per_page: default_pdf_max_chars_per_page(),
            pdf_max_total_chars: default_pdf_max_total_chars(),
            batch_max_items: default_batch_max_items(),
            batch_max_items_cap: default_batch_max_items_cap(),
            batch_max_chars_per_item: default_batch_max_chars_per_item(),
            batch_max_total_chars: default_batch_max_total_chars(),
            batch_max_total_chars_cap: default_batch_max_total_chars_cap(),
            batch_concurrency: default_batch_concurrency(),
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
    /// Optional `[local]` section for local workspace search.
    #[serde(default)]
    pub local: crate::core::local::LocalConfig,
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
        let known: std::collections::BTreeSet<&str> = KNOWN_PROVIDER_IDS.iter().copied().collect();
        let configured: std::collections::BTreeSet<&str> =
            self.search.providers.keys().map(|s| s.as_str()).collect();
        let api_known: std::collections::BTreeSet<&str> =
            self.search.api.keys().map(|s| s.as_str()).collect();

        if override_list.is_empty() {
            let defaults: Vec<String> = self
                .search
                .default_providers
                .iter()
                .filter(|id| {
                    let id_str = id.as_str();
                    if !known.contains(id_str)
                        && !configured.contains(id_str)
                        && !api_known.contains(id_str)
                    {
                        return false;
                    }
                    enabled.contains(id_str) || api_known.contains(id_str)
                })
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

            // Check for unknown provider ids (not in KNOWN_PROVIDER_IDS
            // and not in the config's providers or api map)
            let unknown: Vec<String> = deduped
                .iter()
                .filter(|id| {
                    let id_str = id.as_str();
                    !known.contains(id_str)
                        && !configured.contains(id_str)
                        && !api_known.contains(id_str)
                })
                .cloned()
                .collect();
            if !unknown.is_empty() {
                return Err(CoreError::Config(format!(
                    "unknown provider id(s): {}",
                    unknown.join(", ")
                )));
            }

            // Check for explicitly DISABLED providers (config key exists with value false)
            let explicitly_disabled: Vec<String> = deduped
                .iter()
                .filter(|id| self.search.providers.get(*id).is_some_and(|v| !*v))
                .cloned()
                .collect();
            if !explicitly_disabled.is_empty() {
                return Err(CoreError::Config(format!(
                    "provider(s) disabled: {}; enable them in [search].providers or remove them from request",
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
            pdf_enabled: self.fetch.pdf_enabled,
            pdf_max_pages: self.fetch.pdf_max_pages,
            pdf_max_chars_per_page: self.fetch.pdf_max_chars_per_page,
            pdf_max_total_chars: self.fetch.pdf_max_total_chars,
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
        if self.search.default_max_results == 0 {
            return Err(CoreError::Config(
                "[search].default_max_results must be > 0".to_string(),
            ));
        }
        if self.search.max_results_cap == 0 {
            return Err(CoreError::Config(
                "[search].max_results_cap must be > 0".to_string(),
            ));
        }
        if self.search.max_results_cap < self.search.default_max_results {
            return Err(CoreError::Config(format!(
                "[search].default_max_results ({}) must be <= [search].max_results_cap ({})",
                self.search.default_max_results, self.search.max_results_cap
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

        // Provider validation
        let known: std::collections::BTreeSet<&str> = KNOWN_PROVIDER_IDS.iter().copied().collect();

        // Every default provider must be a known id
        for id in &self.search.default_providers {
            if !known.contains(id.as_str()) {
                return Err(CoreError::Config(format!(
                    "[search].default_providers contains unknown provider: {id}"
                )));
            }
        }

        // Every key in the providers map must be a known id
        for id in self.search.providers.keys() {
            if !known.contains(id.as_str()) {
                return Err(CoreError::Config(format!(
                    "[search].providers contains unknown provider: {id}"
                )));
            }
        }

        // SearXNG validation: if enabled, base_url must be non-empty
        if self.search.searxng.enabled {
            match self.search.searxng.base_url.as_deref() {
                None | Some("") => {
                    return Err(CoreError::Config(
                        "[search].searxng.enabled is true but [search].searxng.base_url is missing or empty".to_string(),
                    ));
                }
                Some(url) => {
                    if url::Url::parse(url).is_err() {
                        return Err(CoreError::Config(format!(
                            "[search].searxng.base_url is not a valid URL: {url}"
                        )));
                    }
                }
            }
        }

        // API provider validation
        let known_api: std::collections::BTreeSet<&str> = [
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
        ]
        .into_iter()
        .collect();
        for (id, api_cfg) in &self.search.api {
            if !known_api.contains(id.as_str()) {
                tracing::warn!(
                    api_provider_id = %id,
                    "unknown API provider id in [search].api; \
                     it may be for a future provider"
                );
            }
            if api_cfg.enabled {
                match api_cfg.api_key_env.as_deref() {
                    None | Some("") => {
                        return Err(CoreError::Config(format!(
                            "[search].api.{id}.enabled is true but [search].api.{id}.api_key_env is missing or empty"
                        )));
                    }
                    Some(env_name) => {
                        if std::env::var(env_name).is_err() {
                            tracing::warn!(
                                api_provider_id = %id,
                                env_name = %env_name,
                                "API provider is enabled but its api_key_env variable is not set; \
                                 the provider will fail at runtime"
                            );
                        }
                    }
                }
                if let Some(ref url) = api_cfg.base_url {
                    if url::Url::parse(url).is_err() {
                        return Err(CoreError::Config(format!(
                            "[search].api.{id}.base_url is not a valid URL: {url}"
                        )));
                    }
                }
            }
        }

        // Batch fetch config validation
        if self.fetch.batch_max_items == 0 {
            return Err(CoreError::Config(
                "[fetch].batch_max_items must be > 0".to_string(),
            ));
        }
        if self.fetch.batch_max_items_cap == 0 {
            return Err(CoreError::Config(
                "[fetch].batch_max_items_cap must be > 0".to_string(),
            ));
        }
        if self.fetch.batch_max_items_cap < self.fetch.batch_max_items {
            return Err(CoreError::Config(format!(
                "[fetch].batch_max_items_cap ({}) must be >= [fetch].batch_max_items ({})",
                self.fetch.batch_max_items_cap, self.fetch.batch_max_items
            )));
        }
        if self.fetch.batch_max_total_chars == 0 {
            return Err(CoreError::Config(
                "[fetch].batch_max_total_chars must be > 0".to_string(),
            ));
        }
        if self.fetch.batch_max_total_chars_cap < self.fetch.batch_max_total_chars {
            return Err(CoreError::Config(format!(
                "[fetch].batch_max_total_chars_cap ({}) must be >= [fetch].batch_max_total_chars ({})",
                self.fetch.batch_max_total_chars_cap, self.fetch.batch_max_total_chars
            )));
        }
        if self.fetch.batch_concurrency == 0 {
            return Err(CoreError::Config(
                "[fetch].batch_concurrency must be > 0".to_string(),
            ));
        }

        // Local config validation
        if self.local.enabled {
            if self.local.roots.is_empty() {
                return Err(CoreError::Config(
                    "[local].enabled is true but [local].roots is empty; at least one root is required".to_string(),
                ));
            }
            for root in &self.local.roots {
                if !root.exists() {
                    return Err(CoreError::Config(format!(
                        "[local].root '{}' does not exist",
                        root.display()
                    )));
                }
                if !root.is_dir() {
                    return Err(CoreError::Config(format!(
                        "[local].root '{}' is not a directory",
                        root.display()
                    )));
                }
            }
            if self.local.max_file_bytes == 0 {
                return Err(CoreError::Config(
                    "[local].max_file_bytes must be > 0".to_string(),
                ));
            }
            if self.local.max_indexed_files == 0 {
                return Err(CoreError::Config(
                    "[local].max_indexed_files must be > 0".to_string(),
                ));
            }
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

    /// Resolve providers for a search profile. Returns the provider list,
    /// whether the profile degraded to generic providers, and any warnings
    /// about unavailable providers.
    ///
    /// Resolution order:
    /// 1. If `explicit_providers` is non-empty, validate and use it exactly.
    /// 2. Else if `profile` is supplied, derive provider list from configured
    ///    profile or built-in defaults.
    /// 3. Else use existing `default_providers` behavior.
    pub fn resolve_profile_providers(
        &self,
        profile: Option<crate::core::repo_search::SearchProfile>,
        explicit_providers: &[String],
    ) -> (Vec<String>, bool, Vec<crate::core::result::SearchWarning>) {
        use crate::core::repo_search::SearchProfile;

        // Explicit providers always win
        if !explicit_providers.is_empty() {
            match self.resolve_providers(explicit_providers) {
                Ok(providers) => return (providers, false, vec![]),
                Err(e) => {
                    return (
                        vec![],
                        true,
                        vec![crate::core::result::SearchWarning::new(
                            "_system",
                            format!("provider_resolution_failed: {e}"),
                        )],
                    );
                }
            }
        }

        let profile = profile.unwrap_or_default();

        // Try configured profile first
        let profile_providers = self.search.profiles.get(profile.as_str());

        // Built-in defaults per profile
        let built_in = match profile {
            SearchProfile::Generic => vec![],
            SearchProfile::Coding => vec![
                "github_code".to_string(),
                "github_issues".to_string(),
                "github_releases".to_string(),
                "gitlab_code".to_string(),
                "gitlab_issues".to_string(),
                "gitlab_releases".to_string(),
                "gitea_code".to_string(),
                "gitea_issues".to_string(),
                "gitea_releases".to_string(),
                "brave_api".to_string(),
                "searxng".to_string(),
                "duckduckgo".to_string(),
                "startpage".to_string(),
            ],
            SearchProfile::Security => vec![
                "osv".to_string(),
                "github_issues".to_string(),
                "brave_api".to_string(),
                "duckduckgo".to_string(),
                "startpage".to_string(),
            ],
            SearchProfile::Research => vec![
                "brave_api".to_string(),
                "searxng".to_string(),
                "duckduckgo".to_string(),
                "startpage".to_string(),
                "mojeek".to_string(),
            ],
        };

        let candidate_list = profile_providers
            .map(|pc| pc.providers.clone())
            .unwrap_or(built_in);

        // If the profile is Generic and has no configured providers, fall back to defaults
        if candidate_list.is_empty() && profile == SearchProfile::Generic {
            match self.resolve_providers(&[]) {
                Ok(providers) => return (providers, false, vec![]),
                Err(e) => {
                    return (
                        vec![],
                        true,
                        vec![crate::core::result::SearchWarning::new(
                            "_system",
                            format!("default_provider_resolution_failed: {e}"),
                        )],
                    );
                }
            }
        }

        let enabled_ids = self.enabled_provider_ids();
        let enabled: std::collections::BTreeSet<&str> =
            enabled_ids.iter().map(|s| s.as_str()).collect();
        let known: std::collections::BTreeSet<&str> = KNOWN_PROVIDER_IDS.iter().copied().collect();
        let configured: std::collections::BTreeSet<&str> =
            self.search.providers.keys().map(|s| s.as_str()).collect();
        let api_known: std::collections::BTreeSet<&str> =
            self.search.api.keys().map(|s| s.as_str()).collect();

        let mut warnings = Vec::new();
        let mut resolved = Vec::new();
        let mut any_available = false;

        for id in &candidate_list {
            let id_str = id.as_str();
            let is_known =
                known.contains(id_str) || configured.contains(id_str) || api_known.contains(id_str);
            let is_enabled = enabled.contains(id_str) || api_known.contains(id_str);

            if !is_known {
                // Unknown provider IDs are skipped with warning
                warnings.push(crate::core::result::SearchWarning::new(
                    "_system",
                    format!("profile_provider_unknown: {id} is in {profile} profile but is not a known provider"),
                ));
                continue;
            }

            if !is_enabled {
                warnings.push(crate::core::result::SearchWarning::new(
                    "_system",
                    format!("profile_provider_unavailable: {id} is in {profile} profile but not configured or enabled"),
                ));
                continue;
            }

            resolved.push(id.clone());
            any_available = true;
        }

        if resolved.is_empty() && !candidate_list.is_empty() {
            warnings.push(crate::core::result::SearchWarning::new(
                "_system",
                format!("profile_degraded: {profile} profile fell back to default providers"),
            ));
            // Fall back to default_providers
            match self.resolve_providers(&[]) {
                Ok(defaults) => return (defaults, true, warnings),
                Err(e) => {
                    warnings.push(crate::core::result::SearchWarning::new(
                        "_system",
                        format!("default_provider_resolution_failed: {e}"),
                    ));
                    return (vec![], true, warnings);
                }
            }
        }

        if !any_available {
            warnings.push(crate::core::result::SearchWarning::new(
                "_system",
                format!("profile_degraded: {profile} profile fell back to default providers"),
            ));
            match self.resolve_providers(&[]) {
                Ok(defaults) => return (defaults, true, warnings),
                Err(e) => {
                    warnings.push(crate::core::result::SearchWarning::new(
                        "_system",
                        format!("default_provider_resolution_failed: {e}"),
                    ));
                    return (vec![], true, warnings);
                }
            }
        }

        (resolved, false, warnings)
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
        assert!(c.search.default_max_results > 0);
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
    fn resolve_providers_accepts_brave_api_in_explicit_list() {
        let c = AppConfig::default();
        // brave_api is in KNOWN_PROVIDER_IDS, so it should be accepted
        // (not rejected as "unknown provider"). It is NOT in the default
        // providers map, so it passes through successfully.
        let result = c.resolve_providers(&["brave_api".to_string()]);
        assert!(
            result.is_ok(),
            "brave_api should be accepted: {:?}",
            result.err()
        );
        assert_eq!(result.unwrap(), vec!["brave_api".to_string()]);
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
    // Profile config tests
    // -----------------------------------------------------------------------

    #[test]
    fn default_profiles_map_is_empty() {
        let c = AppConfig::default();
        assert!(c.search.profiles.is_empty());
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
        // Available providers should be a subset of default_providers
        // (the non-github ones that are enabled)
        for p in &providers {
            assert!(
                c.search.default_providers.contains(p),
                "resolved provider {p} should be from default_providers"
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
        let (providers, degraded, _warnings) = c
            .resolve_profile_providers(Some(crate::core::repo_search::SearchProfile::Generic), &[]);
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
}
