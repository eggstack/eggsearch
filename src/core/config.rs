//! Configuration model and loader for eggsearch.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::error::{CoreError, CoreResult};
use crate::core::provider::{API_PROVIDER_IDS, KNOWN_PROVIDER_IDS};

/// Returns `true` when the named credential environment variable is set to
/// a non-empty value.
///
/// A variable that is set but empty is a broken credential, not a usable
/// one: every credential path in the crate treats it as missing, so
/// diagnostics must report it the same way.
pub fn api_env_value_set(env: &str) -> bool {
    !env.is_empty() && std::env::var(env).ok().is_some_and(|v| !v.is_empty())
}

/// Returns `true` if an API provider is actually configured enough to
/// build or route requests: enabled, known, has a non-empty
/// `api_key_env`, and the referenced environment variable is present.
fn api_provider_is_configured(id: &str, cfg: &ApiProviderConfig) -> bool {
    if !cfg.enabled {
        return false;
    }
    if !API_PROVIDER_IDS.contains(&id) {
        return false;
    }
    match cfg.api_key_env.as_deref() {
        Some(env) => api_env_value_set(env),
        None => false,
    }
}

/// Resolve an optional API key for providers that work keyless.
///
/// Returns `Some(key)` only when the `[search.api.<id>]` entry is enabled,
/// names a non-empty `api_key_env`, and the referenced env var is present
/// and non-empty. All other states (absent entry, disabled entry, missing
/// env, empty value) yield `None` so the caller falls back to keyless.
/// Empty configured values fall back keyless; callers emit a precise
/// warning rather than failing configuration.
pub fn optional_api_key(
    id: &str,
    api: &std::collections::BTreeMap<String, ApiProviderConfig>,
) -> Option<String> {
    if !crate::core::provider::is_optional_api_provider(id) {
        return None;
    }
    let cfg = api.get(id)?;
    if !cfg.enabled {
        return None;
    }
    let env = cfg.api_key_env.as_deref().filter(|s| !s.is_empty())?;
    std::env::var(env).ok().filter(|v| !v.is_empty())
}

/// Whether an optional-credential entry is explicitly configured but
/// unusable (enabled with a missing or empty env value).
///
/// Used to emit a precise keyless-fallback warning without failing
/// configuration or marking the provider unroutable.
pub fn optional_api_key_misconfigured(
    id: &str,
    api: &std::collections::BTreeMap<String, ApiProviderConfig>,
) -> bool {
    if !crate::core::provider::is_optional_api_provider(id) {
        return false;
    }
    let Some(cfg) = api.get(id) else {
        return false;
    };
    if !cfg.enabled {
        return false;
    }
    match cfg.api_key_env.as_deref() {
        None | Some("") => true,
        Some(env) => std::env::var(env).ok().is_none_or(|v| v.is_empty()),
    }
}

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

#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)]
pub enum McpSchemaProfile {
    #[default]
    Full,
    Compact,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct McpSection {
    #[serde(default)]
    pub schema_profile: McpSchemaProfile,
    #[serde(default = "default_brief_max_chars")]
    pub brief_max_chars: usize,
}

fn default_brief_max_chars() -> usize {
    8_000
}

impl Default for McpSection {
    fn default() -> Self {
        Self {
            schema_profile: McpSchemaProfile::Full,
            brief_max_chars: default_brief_max_chars(),
        }
    }
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

/// Upper bound for `[search].cache_ttl_seconds` (24 hours).
const SEARCH_CACHE_TTL_MAX_SECONDS: u64 = 86_400;

/// Upper bound for `[search].cache_max_entries`.
const SEARCH_CACHE_MAX_ENTRIES_LIMIT: usize = 4096;

/// Upper bound for `[fetch].pdf_max_bytes` (256 MiB).
const PDF_MAX_BYTES_LIMIT: usize = 268_435_456;

/// Lower bound for `[fetch].pdf_max_decompressed_bytes` (1 MiB).
const PDF_MIN_DECOMPRESSED_BYTES: usize = 1_048_576;

/// Upper bound for `[fetch].pdf_max_decompressed_bytes` (1 GiB).
const PDF_MAX_DECOMPRESSED_BYTES_LIMIT: usize = 1_073_741_824;

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
    /// Time-to-live in seconds for process-local `web_search` fan-out
    /// cache entries. `0` disables the cache. Default: 300. Bounded to
    /// 0..=86400 (24 hours). The TTL is a freshness bound, not a
    /// correctness guarantee: a hit can be up to one TTL stale, and a
    /// fan-out with any provider failure or deadline interruption is
    /// never stored.
    #[serde(default = "default_search_cache_ttl_seconds")]
    pub cache_ttl_seconds: u64,
    /// Maximum number of process-local `web_search` fan-out cache
    /// entries. Default: 128. Bounded to 1..=4096. A single entry is
    /// additionally dropped when its total row count exceeds the
    /// internal per-entry row cap, so one oversized fan-out can never
    /// evict the whole cache.
    #[serde(default = "default_search_cache_max_entries")]
    pub cache_max_entries: usize,
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
        providers.insert("firecrawl_developer".to_string(), false);
        providers.insert("wikipedia".to_string(), false);
        providers.insert("arxiv".to_string(), false);
        providers.insert("pubmed".to_string(), false);
        providers.insert("hn_algolia".to_string(), false);
        providers.insert("github_repositories".to_string(), false);
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
            cache_ttl_seconds: default_search_cache_ttl_seconds(),
            cache_max_entries: default_search_cache_max_entries(),
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
    format!(
        "eggsearch/{} (+https://github.com/eggstack/eggsearch)",
        env!("CARGO_PKG_VERSION")
    )
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
fn default_search_cache_ttl_seconds() -> u64 {
    300
}
fn default_search_cache_max_entries() -> usize {
    128
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
fn default_pdf_max_bytes() -> usize {
    16_777_216
}
fn default_pdf_max_decompressed_bytes() -> usize {
    67_108_864
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
fn default_retry_max_attempts() -> usize {
    2
}
fn default_retry_base_delay_ms() -> u64 {
    250
}
fn default_retry_max_delay_ms() -> u64 {
    4000
}
fn default_origin_http_concurrency() -> usize {
    2
}
fn default_origin_browser_concurrency() -> usize {
    1
}
fn default_origin_circuit_failure_threshold() -> u8 {
    3
}
fn default_origin_circuit_duration_ms() -> u64 {
    60_000
}
fn default_cache_memory_max_entries() -> usize {
    256
}
fn default_cache_memory_max_bytes() -> usize {
    67_108_864
}
fn default_cache_derived_max_entries() -> usize {
    512
}
fn default_cache_derived_max_bytes() -> usize {
    67_108_864
}
fn default_cache_default_ttl_seconds() -> u64 {
    900
}
fn default_browser_startup_timeout_ms() -> u64 {
    10_000
}
fn default_browser_navigation_timeout_ms() -> u64 {
    20_000
}
fn default_browser_post_load_wait_ms() -> u64 {
    1_500
}
fn default_browser_verification_wait_ms() -> u64 {
    10_000
}
fn default_browser_max_requests() -> usize {
    100
}
fn default_browser_max_dom_bytes() -> usize {
    4_000_000
}

/// Default timeout for profile-scoped browser processes in milliseconds.
pub const DEFAULT_PROFILE_PROCESS_TIMEOUT_MS: u64 = 30_000;
/// Maximum timeout for profile-scoped browser processes in milliseconds.
pub const MAX_PROFILE_PROCESS_TIMEOUT_MS: u64 = 120_000;

fn default_profile_process_timeout_ms() -> u64 {
    DEFAULT_PROFILE_PROCESS_TIMEOUT_MS
}

/// Configuration for persistent browser profiles (Phase 5).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistentBrowserProfilesConfig {
    /// Whether persistent browser profiles are enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Custom profiles directory. Empty uses platform default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profiles_dir: Option<String>,
    /// Allowlist of profile names. Empty allows all.
    #[serde(default)]
    pub allowed_profiles: Vec<String>,
    /// Timeout for profile-scoped browser processes in milliseconds.
    #[serde(default = "default_profile_process_timeout_ms")]
    pub profile_process_timeout_ms: u64,
}

impl Default for PersistentBrowserProfilesConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            profiles_dir: None,
            allowed_profiles: Vec::new(),
            profile_process_timeout_ms: DEFAULT_PROFILE_PROCESS_TIMEOUT_MS,
        }
    }
}

/// The `[fetch.cache]` section of the eggsearch configuration file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FetchCacheSection {
    /// Whether the in-memory fetch cache is enabled. Default: true.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Maximum number of raw response entries in memory. Default: 256.
    #[serde(default = "default_cache_memory_max_entries")]
    pub memory_max_entries: usize,
    /// Maximum total bytes for raw cached response bodies. Default: 64MB.
    #[serde(default = "default_cache_memory_max_bytes")]
    pub memory_max_bytes: usize,
    /// Maximum number of derived (extracted document) entries. Default: 512.
    #[serde(default = "default_cache_derived_max_entries")]
    pub derived_max_entries: usize,
    /// Maximum total bytes for cached derived (extracted document)
    /// entries, independent of the raw-body byte budget. Default: 64MB.
    #[serde(default = "default_cache_derived_max_bytes")]
    pub derived_max_bytes: usize,
    /// Default TTL in seconds for cached responses without explicit
    /// `max-age` or `Expires` headers. Default: 900 (15 minutes).
    #[serde(default = "default_cache_default_ttl_seconds")]
    pub default_ttl_seconds: u64,
}

impl Default for FetchCacheSection {
    fn default() -> Self {
        Self {
            enabled: true,
            memory_max_entries: default_cache_memory_max_entries(),
            memory_max_bytes: default_cache_memory_max_bytes(),
            derived_max_entries: default_cache_derived_max_entries(),
            derived_max_bytes: default_cache_derived_max_bytes(),
            default_ttl_seconds: default_cache_default_ttl_seconds(),
        }
    }
}

#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FetchBrowserSection {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub policy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable: Option<String>,
    #[serde(default = "default_browser_startup_timeout_ms")]
    pub startup_timeout_ms: u64,
    #[serde(default = "default_browser_navigation_timeout_ms")]
    pub navigation_timeout_ms: u64,
    #[serde(default = "default_browser_post_load_wait_ms")]
    pub post_load_wait_ms: u64,
    #[serde(default = "default_browser_verification_wait_ms")]
    pub verification_wait_ms: u64,
    #[serde(default = "default_browser_max_requests")]
    pub max_requests: usize,
    #[serde(default = "default_browser_max_dom_bytes")]
    pub max_dom_bytes: usize,
    #[serde(default = "default_origin_browser_concurrency")]
    pub global_concurrency: usize,
    #[serde(default = "default_origin_browser_concurrency")]
    pub per_origin_concurrency: usize,
    #[serde(default = "default_true")]
    pub block_media: bool,
    #[serde(default)]
    pub persistent_profiles: PersistentBrowserProfilesConfig,
}

impl Default for FetchBrowserSection {
    fn default() -> Self {
        Self {
            enabled: false,
            policy: "http_only".into(),
            executable: None,
            startup_timeout_ms: default_browser_startup_timeout_ms(),
            navigation_timeout_ms: default_browser_navigation_timeout_ms(),
            post_load_wait_ms: default_browser_post_load_wait_ms(),
            verification_wait_ms: default_browser_verification_wait_ms(),
            max_requests: default_browser_max_requests(),
            max_dom_bytes: default_browser_max_dom_bytes(),
            global_concurrency: default_origin_browser_concurrency(),
            per_origin_concurrency: default_origin_browser_concurrency(),
            block_media: true,
            persistent_profiles: PersistentBrowserProfilesConfig::default(),
        }
    }
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
    /// Maximum response body size in bytes for PDF documents, applied when
    /// the response is identified as PDF by Content-Type or a `.pdf` path.
    /// Default: 16777216 (16 MiB). Must be between 1 and 268435456.
    #[serde(default = "default_pdf_max_bytes")]
    pub pdf_max_bytes: usize,
    /// Maximum bytes a PDF document may expand into once its streams are
    /// decompressed, measured over the whole document before parsing.
    /// Default: 67108864 (64 MiB). Must be between 1048576 and 1073741824.
    #[serde(default = "default_pdf_max_decompressed_bytes")]
    pub pdf_max_decompressed_bytes: usize,

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

    // --- Retry and origin control ---
    /// Maximum automatic retry attempts per request (1 = no retries). Default: 2.
    #[serde(default = "default_retry_max_attempts")]
    pub retry_max_attempts: usize,
    /// Base delay in milliseconds for exponential backoff. Default: 250.
    #[serde(default = "default_retry_base_delay_ms")]
    pub retry_base_delay_ms: u64,
    /// Maximum delay in milliseconds for exponential backoff. Default: 4000.
    #[serde(default = "default_retry_max_delay_ms")]
    pub retry_max_delay_ms: u64,
    /// Maximum concurrent HTTP requests per origin. Default: 2.
    #[serde(default = "default_origin_http_concurrency")]
    pub origin_http_concurrency: usize,
    /// Maximum concurrent browser requests per origin (Phase 4 hook). Default: 1.
    #[serde(default = "default_origin_browser_concurrency")]
    pub origin_browser_concurrency: usize,
    /// Number of consecutive retryable failures before opening the circuit. Default: 3.
    #[serde(default = "default_origin_circuit_failure_threshold")]
    pub origin_circuit_failure_threshold: u8,
    /// Circuit breaker open duration in milliseconds. Default: 60000.
    #[serde(default = "default_origin_circuit_duration_ms")]
    pub origin_circuit_duration_ms: u64,

    // --- Cache configuration ---
    /// In-memory fetch cache configuration.
    #[serde(default)]
    pub cache: FetchCacheSection,

    // --- Browser rendering configuration ---
    /// Optional browser rendering configuration (Phase 4). Disabled by
    /// default. When `enabled = true` and a Chrome/Chromium executable is
    /// discovered, `web_fetch` may escalate to browser rendering for
    /// JavaScript-heavy pages.
    #[serde(default)]
    pub browser: FetchBrowserSection,
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
            pdf_max_bytes: default_pdf_max_bytes(),
            pdf_max_decompressed_bytes: default_pdf_max_decompressed_bytes(),
            batch_max_items: default_batch_max_items(),
            batch_max_items_cap: default_batch_max_items_cap(),
            batch_max_chars_per_item: default_batch_max_chars_per_item(),
            batch_max_total_chars: default_batch_max_total_chars(),
            batch_max_total_chars_cap: default_batch_max_total_chars_cap(),
            batch_concurrency: default_batch_concurrency(),
            retry_max_attempts: default_retry_max_attempts(),
            retry_base_delay_ms: default_retry_base_delay_ms(),
            retry_max_delay_ms: default_retry_max_delay_ms(),
            origin_http_concurrency: default_origin_http_concurrency(),
            origin_browser_concurrency: default_origin_browser_concurrency(),
            origin_circuit_failure_threshold: default_origin_circuit_failure_threshold(),
            origin_circuit_duration_ms: default_origin_circuit_duration_ms(),
            cache: FetchCacheSection::default(),
            browser: FetchBrowserSection::default(),
        }
    }
}

/// Single ordered egress proxy hop with secret-indirected credentials.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EgressHopConfig {
    /// Hop scheme: `http`, `socks4`, or `socks5` (`httponly` accepted as an HTTP variant).
    pub scheme: String,
    /// Bare hostname or IP literal without userinfo, path, or port suffix.
    pub host: String,
    /// TCP port of the proxy hop.
    pub port: u16,
    /// Optional proxy username; when set, `password_env` is required.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Environment variable holding the proxy password; raw passwords are never persisted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password_env: Option<String>,
}

/// Optional `[egress]` section for listener-free outbound proxy-chain routing.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EgressSection {
    /// Whether the configured chain is used for eligible fixed-upstream paths.
    #[serde(default)]
    pub enabled: bool,
    /// Ordered proxy hops traversed before reaching the destination.
    #[serde(default)]
    pub hops: Vec<EgressHopConfig>,
}

impl EgressSection {
    /// Whether a proxy chain is configured and enabled.
    pub fn is_routed(&self) -> bool {
        self.enabled && !self.hops.is_empty()
    }

    /// Credential-safe one-line route description.
    pub fn redacted_route(&self) -> String {
        if !self.is_routed() {
            return "direct".to_string();
        }
        self.hops
            .iter()
            .map(|hop| {
                let creds = if hop.username.is_some() || hop.password_env.is_some() {
                    "****:****@"
                } else {
                    ""
                };
                format!("{}://{}{}:{}", hop.scheme, creds, hop.host, hop.port)
            })
            .collect::<Vec<_>>()
            .join("__")
    }

    fn validate(&self) -> CoreResult<()> {
        if !self.enabled {
            if self.hops.is_empty() {
                return Ok(());
            }
            for hop in &self.hops {
                validate_egress_hop(hop)?;
            }
            return Ok(());
        }
        if self.hops.is_empty() {
            return Err(CoreError::Config(
                "[egress].enabled is true but [egress].hops is empty; configure at least one http/socks hop or disable egress".to_string(),
            ));
        }
        for hop in &self.hops {
            validate_egress_hop(hop)?;
        }
        #[cfg(not(feature = "egress"))]
        {
            return Err(CoreError::Config(
                "egress route is configured but this binary was built without the `egress` feature; rebuild with --features egress or set [egress].enabled = false".to_string(),
            ));
        }
        #[cfg(feature = "egress")]
        {
            Ok(())
        }
    }
}

fn is_valid_egress_host(host: &str) -> bool {
    if host.is_empty() {
        return false;
    }
    if host.parse::<std::net::IpAddr>().is_ok() {
        return true;
    }
    if host.contains('@') || host.contains('/') || host.contains(':') {
        return false;
    }
    if host.len() > 253 {
        return false;
    }
    let mut saw_label = false;
    for label in host.split('.') {
        if label.is_empty() || label.len() > 63 {
            return false;
        }
        let Some(first) = label.chars().next() else {
            return false;
        };
        if !first.is_ascii_alphanumeric() {
            return false;
        }
        for ch in label.chars().skip(1) {
            if !(ch.is_ascii_alphanumeric() || ch == '-') {
                return false;
            }
        }
        let Some(last) = label.chars().next_back() else {
            return false;
        };
        if !last.is_ascii_alphanumeric() {
            return false;
        }
        saw_label = true;
    }
    saw_label
}

fn validate_egress_hop(hop: &EgressHopConfig) -> CoreResult<()> {
    match hop.scheme.as_str() {
        "http" | "httponly" | "socks4" | "socks5" => {}
        other => {
            return Err(CoreError::Config(format!(
                "[egress].hops scheme '{other}' is unsupported; expected one of: http, socks4, socks5 (httponly accepted as http variant)"
            )));
        }
    }
    if hop.host.is_empty() {
        return Err(CoreError::Config(
            "[egress].hops host must be non-empty".to_string(),
        ));
    }
    if !is_valid_egress_host(&hop.host) {
        return Err(CoreError::Config(format!(
            "[egress].hops host '{}' must be a bare hostname or IP literal without userinfo, path, or port suffix",
            hop.host
        )));
    }
    if hop.port == 0 {
        return Err(CoreError::Config(
            "[egress].hops port must be non-zero".to_string(),
        ));
    }
    if let Some(ref username) = hop.username {
        if username.is_empty() {
            return Err(CoreError::Config(
                "[egress].hops username must be non-empty when present".to_string(),
            ));
        }
        if username.contains(':') || username.contains('@') {
            return Err(CoreError::Config(
                "[egress].hops username must not contain ':' or '@'".to_string(),
            ));
        }
    }
    if let Some(ref env) = hop.password_env {
        if env.is_empty() {
            return Err(CoreError::Config(
                "[egress].hops password_env must be non-empty when present".to_string(),
            ));
        }
        if hop.username.as_deref().is_none_or(|u| u.is_empty()) {
            return Err(CoreError::Config(
                "[egress].hops password_env requires a non-empty username".to_string(),
            ));
        }
    }
    Ok(())
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
    /// Optional `[egress]` section for listener-free outbound proxy-chain routing.
    #[serde(default)]
    pub egress: EgressSection,
    /// Optional MCP presentation settings.
    #[serde(default)]
    pub mcp: McpSection,
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
        cfg.validate()?;
        Ok(cfg)
    }

    /// Save the config to the given path. Creates parent dirs as needed.
    ///
    /// The write is atomic (staged in a sibling temp file, then renamed) so a
    /// crash mid-write cannot leave a truncated TOML file that fails to parse
    /// on the next start.
    pub fn save(&self, path: &Path) -> CoreResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self).map_err(|e| CoreError::TomlSer(e.to_string()))?;
        use std::io::Write;
        let parent = path.parent().ok_or_else(|| {
            CoreError::Io(std::io::Error::other("config path has no parent directory"))
        })?;
        let mut staged = tempfile::NamedTempFile::new_in(parent)?;
        staged.write_all(text.as_bytes())?;
        staged.as_file().sync_all()?;
        staged
            .persist(path)
            .map(|_| ())
            .map_err(|e| CoreError::Io(e.error))
    }

    /// Resolve the effective provider list for a request.
    /// If override_list is empty, uses default_providers filtered to only enabled.
    /// If override_list is non-empty, validates explicitly-disabled providers and deduplicates.
    pub fn resolve_providers(&self, override_list: &[String]) -> CoreResult<Vec<String>> {
        let enabled_ids: Vec<String> = self.effective_provider_ids();
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
                    enabled.contains(id_str)
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

            let unavailable: Vec<String> = deduped
                .iter()
                .filter(|id| !enabled.contains(id.as_str()))
                .cloned()
                .collect();
            if !unavailable.is_empty() {
                return Err(CoreError::Config(format!(
                    "provider(s) disabled or unavailable: {}; enable them in [search].providers, or configure a usable API provider in [search].api",
                    unavailable.join(", ")
                )));
            }
            Ok(deduped)
        }
    }

    /// Validate `[search.profiles.*]`.
    ///
    /// Profile lookup goes through `SearchProfile::as_str()`, so a key that is
    /// not a canonical profile name can never be selected and would otherwise
    /// stay dead forever. Provider IDs inside a profile go through the same
    /// known/configured/api sets as `[search].default_providers`.
    fn validate_search_profiles(&self) -> CoreResult<()> {
        if self.search.profiles.is_empty() {
            return Ok(());
        }

        let known: std::collections::BTreeSet<&str> = KNOWN_PROVIDER_IDS.iter().copied().collect();
        let configured: std::collections::BTreeSet<&str> =
            self.search.providers.keys().map(|s| s.as_str()).collect();
        let api_known: std::collections::BTreeSet<&str> =
            self.search.api.keys().map(|s| s.as_str()).collect();

        let canonical = ["generic", "coding", "security", "research"];
        for (name, profile) in &self.search.profiles {
            if !canonical.contains(&name.as_str()) {
                return Err(CoreError::Config(format!(
                    "unknown [search.profiles.{name}]: valid profile names are {}",
                    canonical.join(", ")
                )));
            }
            if profile.providers.is_empty() {
                return Err(CoreError::Config(format!(
                    "[search.profiles.{name}].providers must not be empty; remove the profile to use the default provider list"
                )));
            }
            let unknown: Vec<&str> = profile
                .providers
                .iter()
                .map(|s| s.as_str())
                .filter(|id| {
                    !known.contains(id) && !configured.contains(id) && !api_known.contains(id)
                })
                .collect();
            if !unknown.is_empty() {
                return Err(CoreError::Config(format!(
                    "[search.profiles.{name}]: unknown provider id(s): {}",
                    unknown.join(", ")
                )));
            }
        }
        Ok(())
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

    /// Returns provider IDs that can be built for live search.
    pub fn effective_provider_ids(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .enabled_provider_ids()
            .into_iter()
            .filter(|id| !API_PROVIDER_IDS.contains(&id.as_str()))
            .collect();
        let mut seen: std::collections::HashSet<String> = out.iter().cloned().collect();
        for (id, cfg) in &self.search.api {
            if api_provider_is_configured(id, cfg) && seen.insert(id.clone()) {
                out.push(id.clone());
            }
        }
        out
    }

    /// Returns true when an API provider is enabled and names a credential
    /// environment variable, but that variable is absent or empty.
    ///
    /// This is the "operator asked for this provider and supplied a broken or
    /// unset credential" case, which must be reported as a missing credential
    /// rather than as a disabled provider: the repair is to set the named
    /// environment variable, not to flip a `[search.providers]` boolean.
    pub fn api_provider_credential_missing(&self, id: &str) -> Option<&str> {
        if !API_PROVIDER_IDS.contains(&id) {
            return None;
        }
        let cfg = self.search.api.get(id)?;
        if !cfg.enabled {
            return None;
        }
        let env = cfg.api_key_env.as_deref().filter(|e| !e.is_empty())?;
        match std::env::var(env) {
            Ok(value) if !value.is_empty() => None,
            _ => Some(env),
        }
    }

    /// Returns true when a provider can be routed to a built engine.
    pub fn provider_is_available(&self, id: &str) -> bool {
        if API_PROVIDER_IDS.contains(&id) {
            return self
                .search
                .api
                .get(id)
                .is_some_and(|cfg| api_provider_is_configured(id, cfg));
        }
        if id == "searxng" {
            return self
                .search
                .providers
                .get("searxng")
                .copied()
                .unwrap_or(false)
                && self.search.searxng.enabled
                && self
                    .search
                    .searxng
                    .base_url
                    .as_deref()
                    .is_some_and(|u| !u.is_empty());
        }
        self.search.providers.get(id).copied().unwrap_or(false)
    }

    /// Returns the provider ids listed in `default_providers` that are
    /// not available for live routing. These are silently filtered out
    /// by `resolve_providers`; operators should be told at startup so
    /// they can fix the config.
    pub fn misconfigured_default_providers(&self) -> Vec<String> {
        let enabled_ids = self.effective_provider_ids();
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
            pdf_max_bytes: self.fetch.pdf_max_bytes,
            pdf_max_decompressed_bytes: self.fetch.pdf_max_decompressed_bytes,
        }
    }

    /// Validate configuration invariants. Returns `Err` on the first
    /// violated invariant. The intent is to fail fast on operator
    /// misconfiguration (e.g. `max_chars_cap < max_chars_default`)
    /// rather than silently degrading behavior.
    pub fn validate(&self) -> CoreResult<()> {
        self.validate_search_profiles()?;
        if !(512..=32_000).contains(&self.mcp.brief_max_chars) {
            return Err(CoreError::Config(
                "[mcp].brief_max_chars must be between 512 and 32000".to_string(),
            ));
        }
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
        if !(1..=PDF_MAX_BYTES_LIMIT).contains(&self.fetch.pdf_max_bytes) {
            return Err(CoreError::Config(format!(
                "[fetch].pdf_max_bytes ({}) must be between 1 and {PDF_MAX_BYTES_LIMIT}",
                self.fetch.pdf_max_bytes
            )));
        }
        if !(PDF_MIN_DECOMPRESSED_BYTES..=PDF_MAX_DECOMPRESSED_BYTES_LIMIT)
            .contains(&self.fetch.pdf_max_decompressed_bytes)
        {
            return Err(CoreError::Config(format!(
                "[fetch].pdf_max_decompressed_bytes ({}) must be between {PDF_MIN_DECOMPRESSED_BYTES} and {PDF_MAX_DECOMPRESSED_BYTES_LIMIT}",
                self.fetch.pdf_max_decompressed_bytes
            )));
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
        if self.search.cache_ttl_seconds > SEARCH_CACHE_TTL_MAX_SECONDS {
            return Err(CoreError::Config(format!(
                "[search].cache_ttl_seconds ({}) must be <= {SEARCH_CACHE_TTL_MAX_SECONDS}; 0 disables the cache",
                self.search.cache_ttl_seconds
            )));
        }
        if !(1..=SEARCH_CACHE_MAX_ENTRIES_LIMIT).contains(&self.search.cache_max_entries) {
            return Err(CoreError::Config(format!(
                "[search].cache_max_entries ({}) must be between 1 and {SEARCH_CACHE_MAX_ENTRIES_LIMIT}",
                self.search.cache_max_entries
            )));
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
                Some(url) => match url::Url::parse(url) {
                    Err(_) => {
                        return Err(CoreError::Config(format!(
                            "[search].searxng.base_url is not a valid URL: {url}"
                        )));
                    }
                    Ok(parsed) if !matches!(parsed.scheme(), "http" | "https") => {
                        return Err(CoreError::Config(format!(
                            "[search].searxng.base_url must use http or https scheme, got: {}",
                            parsed.scheme()
                        )));
                    }
                    Ok(parsed) if parsed.host_str().is_none() => {
                        return Err(CoreError::Config(format!(
                            "[search].searxng.base_url must have a host: {url}"
                        )));
                    }
                    _ => {}
                },
            }
        }

        // API provider validation
        for (id, api_cfg) in &self.search.api {
            let is_optional = crate::core::provider::is_optional_api_provider(id);
            if !API_PROVIDER_IDS.contains(&id.as_str()) && !is_optional {
                tracing::warn!(
                    api_provider_id = %id,
                    "unknown API provider id in [search].api; \
                     it may be for a future provider"
                );
            }
            if api_cfg.enabled {
                if is_optional {
                    match api_cfg.api_key_env.as_deref() {
                        None | Some("") => {
                            tracing::warn!(
                                api_provider_id = %id,
                                "optional API provider is enabled without api_key_env; \
                                 continuing keyless"
                            );
                        }
                        Some(env_name) => match std::env::var(env_name) {
                            Ok(v) if !v.is_empty() => {}
                            _ => {
                                tracing::warn!(
                                    api_provider_id = %id,
                                    env_name = %env_name,
                                    "optional API provider is enabled but its api_key_env variable is missing or empty; \
                                     continuing keyless"
                                );
                            }
                        },
                    }
                } else {
                    match api_cfg.api_key_env.as_deref() {
                        None | Some("") => {
                            return Err(CoreError::Config(format!(
                                "[search].api.{id}.enabled is true but [search].api.{id}.api_key_env is missing or empty"
                            )));
                        }
                        Some(env_name) => {
                            if std::env::var(env_name).ok().is_none_or(|v| v.is_empty()) {
                                tracing::warn!(
                                    api_provider_id = %id,
                                    env_name = %env_name,
                                    "API provider is enabled but its api_key_env variable is not set; \
                                     the provider will fail at runtime"
                                );
                            }
                        }
                    }
                }
                if let Some(ref url) = api_cfg.base_url {
                    match url::Url::parse(url) {
                        Err(_) => {
                            return Err(CoreError::Config(format!(
                                "[search].api.{id}.base_url is not a valid URL: {url}"
                            )));
                        }
                        Ok(parsed) if !matches!(parsed.scheme(), "http" | "https") => {
                            return Err(CoreError::Config(format!(
                                "[search].api.{id}.base_url must use http or https scheme, got: {}",
                                parsed.scheme()
                            )));
                        }
                        Ok(parsed) if parsed.host_str().is_none() => {
                            return Err(CoreError::Config(format!(
                                "[search].api.{id}.base_url must have a host: {url}"
                            )));
                        }
                        _ => {}
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

        if self.fetch.retry_max_attempts == 0 {
            return Err(CoreError::Config(
                "[fetch].retry_max_attempts must be > 0".to_string(),
            ));
        }
        if self.fetch.retry_max_delay_ms < self.fetch.retry_base_delay_ms {
            return Err(CoreError::Config(format!(
                "[fetch].retry_max_delay_ms ({}) must be >= [fetch].retry_base_delay_ms ({})",
                self.fetch.retry_max_delay_ms, self.fetch.retry_base_delay_ms
            )));
        }
        if self.fetch.origin_http_concurrency == 0 {
            return Err(CoreError::Config(
                "[fetch].origin_http_concurrency must be > 0".to_string(),
            ));
        }
        if self.fetch.origin_browser_concurrency == 0 {
            return Err(CoreError::Config(
                "[fetch].origin_browser_concurrency must be > 0".to_string(),
            ));
        }
        if self.fetch.origin_circuit_failure_threshold == 0 {
            return Err(CoreError::Config(
                "[fetch].origin_circuit_failure_threshold must be > 0".to_string(),
            ));
        }
        if self.fetch.origin_circuit_duration_ms == 0 {
            return Err(CoreError::Config(
                "[fetch].origin_circuit_duration_ms must be > 0".to_string(),
            ));
        }
        if self.fetch.cache.memory_max_entries == 0 {
            return Err(CoreError::Config(
                "[fetch].cache.memory_max_entries must be > 0".to_string(),
            ));
        }
        if self.fetch.cache.memory_max_bytes == 0 {
            return Err(CoreError::Config(
                "[fetch].cache.memory_max_bytes must be > 0".to_string(),
            ));
        }
        if self.fetch.cache.derived_max_entries == 0 {
            return Err(CoreError::Config(
                "[fetch].cache.derived_max_entries must be > 0".to_string(),
            ));
        }
        if self.fetch.cache.derived_max_bytes == 0 {
            return Err(CoreError::Config(
                "[fetch].cache.derived_max_bytes must be > 0".to_string(),
            ));
        }

        if self.fetch.browser.startup_timeout_ms == 0 {
            return Err(CoreError::Config(
                "[fetch].browser.startup_timeout_ms must be > 0".to_string(),
            ));
        }
        if self.fetch.browser.navigation_timeout_ms == 0 {
            return Err(CoreError::Config(
                "[fetch].browser.navigation_timeout_ms must be > 0".to_string(),
            ));
        }
        if self.fetch.browser.max_requests == 0 {
            return Err(CoreError::Config(
                "[fetch].browser.max_requests must be > 0".to_string(),
            ));
        }
        if self.fetch.browser.max_dom_bytes == 0 {
            return Err(CoreError::Config(
                "[fetch].browser.max_dom_bytes must be > 0".to_string(),
            ));
        }
        if self
            .fetch
            .browser
            .persistent_profiles
            .profile_process_timeout_ms
            == 0
        {
            return Err(CoreError::Config(
                "[fetch].browser.persistent_profiles.profile_process_timeout_ms must be > 0"
                    .to_string(),
            ));
        }
        if self
            .fetch
            .browser
            .persistent_profiles
            .profile_process_timeout_ms
            > MAX_PROFILE_PROCESS_TIMEOUT_MS
        {
            return Err(CoreError::Config(format!(
                "[fetch].browser.persistent_profiles.profile_process_timeout_ms exceeds maximum {MAX_PROFILE_PROCESS_TIMEOUT_MS}"
            )));
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
            if self.local.max_parse_bytes == 0 {
                return Err(CoreError::Config(
                    "[local].max_parse_bytes must be > 0".to_string(),
                ));
            }
            if self.local.max_symbols_per_file == 0 {
                return Err(CoreError::Config(
                    "[local].max_symbols_per_file must be > 0".to_string(),
                ));
            }
            if self.local.max_structured_files == 0 {
                return Err(CoreError::Config(
                    "[local].max_structured_files must be > 0".to_string(),
                ));
            }
            if self.local.max_total_symbols == 0 {
                return Err(CoreError::Config(
                    "[local].max_total_symbols must be > 0".to_string(),
                ));
            }
            if self.local.repo_map_structure_cap == 0 {
                return Err(CoreError::Config(
                    "[local].repo_map_structure_cap must be > 0".to_string(),
                ));
            }
        }

        if self.search.mode == Mode::Live {
            let enabled_scrape_count = self
                .search
                .providers
                .iter()
                .filter(|(id, enabled)| **enabled && id.as_str() != "searxng")
                .count();
            let any_api_configured = self
                .search
                .api
                .iter()
                .any(|(id, cfg)| api_provider_is_configured(id, cfg));
            if enabled_scrape_count == 0 && !any_api_configured {
                return Err(CoreError::Config(
                    "[search].mode is 'live' but no traditional providers are enabled \
                     and no API providers are configured with resolvable credentials. \
                     Enable at least one provider in [search].providers or configure \
                     an API provider with a valid api_key_env in [search].api"
                        .to_string(),
                ));
            }
        }
        self.egress.validate()?;
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

        let enabled_ids = self.effective_provider_ids();
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
            let is_enabled = enabled.contains(id_str);

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
mod tests;
