//! Provider capability model and descriptors.
//!
//! Each built-in search engine is described by a [`ProviderDescriptor`]
//! that captures its kind, configuration state, and feature capabilities.
//! MCP `provider_status` and the `eggsearch providers` CLI both
//! serialize these descriptors.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The set of provider ids that ship with the vendored engine
/// implementations.
pub const KNOWN_PROVIDER_IDS: &[&str] = &[
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
];

/// Provider ids that require an operator-supplied API key via
/// `[search].api.<id>.api_key_env`.
pub const API_PROVIDER_IDS: &[&str] = &[
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
    "github_advisory",
    "semantic_scholar",
    "sourcegraph",
];

/// Returns `true` if `id` is a known API-key provider.
pub fn is_api_provider(id: &str) -> bool {
    API_PROVIDER_IDS.contains(&id)
}

/// Return the provider-specific readiness signal used by status
/// surfaces before the generic `enabled` gate is applied.
pub fn provider_configured_state(
    id: &str,
    searxng_configured: bool,
    api_configured: bool,
    local_backend_available: bool,
) -> bool {
    match id {
        "searxng" => searxng_configured,
        "local_workspace" => local_backend_available,
        _ if is_api_provider(id) => api_configured,
        _ => true,
    }
}

/// Whether the provider scrapes HTML or speaks a JSON API, or
/// requires an API key.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// HTML scraping (DuckDuckGo, Brave, Startpage, Yahoo, Mojeek).
    HtmlScrape,
    /// JSON API (SearXNG).
    JsonApi,
    /// Requires an operator-supplied API key (reserved for future use).
    ApiKey,
    /// Local filesystem search backend.
    Local,
}

/// Feature capabilities that a provider may or may not support.
///
/// Two distinct flags cover time-related behaviour and the distinction
/// matters for downstream freshness-aware reranking:
///
/// - [`ProviderCapabilities::supports_freshness`] is a **provider-side**
///   flag: the upstream engine accepts a freshness / time-range
///   parameter and applies it on the server before returning results.
///   When this is `false`, eggsearch does not pass a freshness hint to
///   the upstream request at all.
/// - [`ProviderCapabilities::supports_result_timestamps`] is a
///   **client-side** flag: the provider's result payloads include
///   per-result timestamp evidence (e.g. `updated_at` on issues,
///   `published_at` on releases). eggsearch uses these timestamps
///   locally, after retrieval, to apply bounded freshness reranking.
///   When this is `true`, `FreshnessMatch` may be emitted even when
///   `supports_freshness` is `false`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ProviderCapabilities {
    /// Provider enforces safe-search filtering.
    pub supports_safe_search: bool,
    /// Provider supports a provider-side freshness / time-range
    /// request parameter. See the type-level docs for the distinction
    /// from `supports_result_timestamps`.
    pub supports_freshness: bool,
    /// Provider supports a language parameter.
    pub supports_language: bool,
    /// Provider supports a region / locale parameter.
    pub supports_region: bool,
    /// Provider supports domain include/exclude filters.
    pub supports_domain_filters: bool,
    /// Provider supports a news-specific category.
    pub supports_news: bool,
    /// Provider supports code/file search.
    pub supports_code_search: bool,
    /// Provider supports repo filter (e.g. `repo:owner/name`).
    pub supports_repo_filter: bool,
    /// Provider supports org filter (e.g. `org:name`).
    pub supports_org_filter: bool,
    /// Provider supports path filter (e.g. `path:src/`).
    pub supports_path_filter: bool,
    /// Provider supports language filter (e.g. `language:rust`).
    pub supports_language_filter: bool,
    /// Provider supports symbol hints (best-effort free-text).
    pub supports_symbol_hint: bool,
    /// Provider supports issue search.
    pub supports_issue_search: bool,
    /// Provider supports release search.
    pub supports_release_search: bool,
    /// Provider returns result-level timestamps usable for local
    /// freshness reranking. See the type-level docs for the
    /// distinction from `supports_freshness`.
    pub supports_result_timestamps: bool,
    /// Provider supports native security advisory search.
    pub supports_security_search: bool,
    /// Provider can look up package metadata (name, version, repo URL,
    /// homepage, license, changelog URL, deprecation status).
    pub supports_package_metadata: bool,
    /// Provider can look up a specific advisory by CVE/GHSA/RustSec ID.
    pub supports_advisory_lookup_by_id: bool,
    /// Provider can query advisories by package name/version.
    pub supports_advisory_lookup_by_package: bool,
    /// Provider provides CISA KEV or exploit-in-the-wild status.
    pub supports_exploit_kev_status: bool,
    /// Provider can search scholarly/academic papers.
    pub supports_scholarly_search: bool,
    /// Provider can look up a paper by DOI.
    pub supports_doi_lookup: bool,
    /// Provider provides repository-level indexing (file tree, symbol search).
    pub supports_repo_indexing: bool,
    /// Provider returns structured changelog/release metadata.
    pub supports_structured_changelog: bool,
}

impl ProviderCapabilities {
    /// A capabilities record where every field is `false`.
    pub fn none() -> Self {
        Self {
            supports_safe_search: false,
            supports_freshness: false,
            supports_language: false,
            supports_region: false,
            supports_domain_filters: false,
            supports_news: false,
            supports_code_search: false,
            supports_repo_filter: false,
            supports_org_filter: false,
            supports_path_filter: false,
            supports_language_filter: false,
            supports_symbol_hint: false,
            supports_issue_search: false,
            supports_release_search: false,
            supports_result_timestamps: false,
            supports_security_search: false,
            supports_package_metadata: false,
            supports_advisory_lookup_by_id: false,
            supports_advisory_lookup_by_package: false,
            supports_exploit_kev_status: false,
            supports_scholarly_search: false,
            supports_doi_lookup: false,
            supports_repo_indexing: false,
            supports_structured_changelog: false,
        }
    }

    /// Return a comma-separated list of enabled capability names.
    pub fn summary(&self) -> String {
        let mut caps = Vec::new();
        if self.supports_safe_search {
            caps.push("safe_search");
        }
        if self.supports_freshness {
            caps.push("freshness");
        }
        if self.supports_language {
            caps.push("language");
        }
        if self.supports_region {
            caps.push("region");
        }
        if self.supports_domain_filters {
            caps.push("domain_filters");
        }
        if self.supports_news {
            caps.push("news");
        }
        if self.supports_code_search {
            caps.push("code_search");
        }
        if self.supports_repo_filter {
            caps.push("repo_filter");
        }
        if self.supports_org_filter {
            caps.push("org_filter");
        }
        if self.supports_path_filter {
            caps.push("path_filter");
        }
        if self.supports_language_filter {
            caps.push("language_filter");
        }
        if self.supports_symbol_hint {
            caps.push("symbol_hint");
        }
        if self.supports_issue_search {
            caps.push("issue_search");
        }
        if self.supports_release_search {
            caps.push("release_search");
        }
        if self.supports_result_timestamps {
            caps.push("result_timestamps");
        }
        if self.supports_security_search {
            caps.push("security_search");
        }
        if self.supports_package_metadata {
            caps.push("package_metadata");
        }
        if self.supports_advisory_lookup_by_id {
            caps.push("advisory_lookup_by_id");
        }
        if self.supports_advisory_lookup_by_package {
            caps.push("advisory_lookup_by_package");
        }
        if self.supports_exploit_kev_status {
            caps.push("exploit_kev_status");
        }
        if self.supports_scholarly_search {
            caps.push("scholarly_search");
        }
        if self.supports_doi_lookup {
            caps.push("doi_lookup");
        }
        if self.supports_repo_indexing {
            caps.push("repo_indexing");
        }
        if self.supports_structured_changelog {
            caps.push("structured_changelog");
        }
        if caps.is_empty() {
            "basic".to_string()
        } else {
            caps.join(", ")
        }
    }

    /// Check if a specific option is supported by this provider.
    pub fn supports(&self, option: &CapabilityOption) -> bool {
        match option {
            CapabilityOption::SafeSearch => self.supports_safe_search,
            CapabilityOption::Freshness => self.supports_freshness,
            CapabilityOption::Language => self.supports_language,
            CapabilityOption::Region => self.supports_region,
            CapabilityOption::DomainFilters => self.supports_domain_filters,
            CapabilityOption::News => self.supports_news,
            CapabilityOption::CodeSearch => self.supports_code_search,
            CapabilityOption::RepoFilter => self.supports_repo_filter,
            CapabilityOption::OrgFilter => self.supports_org_filter,
            CapabilityOption::PathFilter => self.supports_path_filter,
            CapabilityOption::LanguageFilter => self.supports_language_filter,
            CapabilityOption::SymbolHint => self.supports_symbol_hint,
            CapabilityOption::IssueSearch => self.supports_issue_search,
            CapabilityOption::ReleaseSearch => self.supports_release_search,
            CapabilityOption::ResultTimestamps => self.supports_result_timestamps,
            CapabilityOption::SecuritySearch => self.supports_security_search,
            CapabilityOption::PackageMetadata => self.supports_package_metadata,
            CapabilityOption::AdvisoryLookupById => self.supports_advisory_lookup_by_id,
            CapabilityOption::AdvisoryLookupByPackage => self.supports_advisory_lookup_by_package,
            CapabilityOption::ExploitKevStatus => self.supports_exploit_kev_status,
            CapabilityOption::ScholarlySearch => self.supports_scholarly_search,
            CapabilityOption::DoiLookup => self.supports_doi_lookup,
            CapabilityOption::RepoIndexing => self.supports_repo_indexing,
            CapabilityOption::StructuredChangelog => self.supports_structured_changelog,
        }
    }
}

/// Options that can be checked against provider capabilities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CapabilityOption {
    /// Safe-search filtering.
    SafeSearch,
    /// Freshness / time-range filtering.
    Freshness,
    /// Language parameter.
    Language,
    /// Region / locale parameter.
    Region,
    /// Domain include/exclude filters.
    DomainFilters,
    /// News-specific category.
    News,
    /// Code/file search.
    CodeSearch,
    /// Repo filter.
    RepoFilter,
    /// Org filter.
    OrgFilter,
    /// Path filter.
    PathFilter,
    /// Language filter.
    LanguageFilter,
    /// Symbol hint.
    SymbolHint,
    /// Issue search.
    IssueSearch,
    /// Release search.
    ReleaseSearch,
    /// Result timestamps.
    ResultTimestamps,
    /// Security advisory search.
    SecuritySearch,
    /// Package metadata lookup.
    PackageMetadata,
    /// Advisory lookup by CVE/GHSA/RustSec ID.
    AdvisoryLookupById,
    /// Advisory lookup by package name/version.
    AdvisoryLookupByPackage,
    /// CISA KEV or exploit-in-the-wild status.
    ExploitKevStatus,
    /// Scholarly/academic paper search.
    ScholarlySearch,
    /// DOI lookup.
    DoiLookup,
    /// Repository-level indexing (file tree, symbol search).
    RepoIndexing,
    /// Structured changelog/release metadata.
    StructuredChangelog,
}

impl CapabilityOption {
    /// Human-readable name for warning messages.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::SafeSearch => "safe_search",
            Self::Freshness => "freshness",
            Self::Language => "language",
            Self::Region => "region",
            Self::DomainFilters => "domain_filters",
            Self::News => "news",
            Self::CodeSearch => "code_search",
            Self::RepoFilter => "repo_filter",
            Self::OrgFilter => "org_filter",
            Self::PathFilter => "path_filter",
            Self::LanguageFilter => "language_filter",
            Self::SymbolHint => "symbol_hint",
            Self::IssueSearch => "issue_search",
            Self::ReleaseSearch => "release_search",
            Self::ResultTimestamps => "result_timestamps",
            Self::SecuritySearch => "security_search",
            Self::PackageMetadata => "package_metadata",
            Self::AdvisoryLookupById => "advisory_lookup_by_id",
            Self::AdvisoryLookupByPackage => "advisory_lookup_by_package",
            Self::ExploitKevStatus => "exploit_kev_status",
            Self::ScholarlySearch => "scholarly_search",
            Self::DoiLookup => "doi_lookup",
            Self::RepoIndexing => "repo_indexing",
            Self::StructuredChangelog => "structured_changelog",
        }
    }
}

/// Machine-actionable skip code for programmatic handling of
/// non-routable providers. Stable across versions — agents can match
/// on these.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSkipCode {
    /// Provider id is not in `KNOWN_PROVIDER_IDS`.
    UnknownProvider,
    /// Provider is disabled in config.
    DisabledByUser,
    /// API-key provider missing a configured API key.
    MissingApiKey,
    /// SearXNG provider missing base_url config.
    MissingSearxngConfig,
    /// Provider missing a required base URL.
    MissingBaseUrl,
    /// Provider has an invalid base URL.
    InvalidBaseUrl,
    /// Local workspace provider missing backend availability.
    MissingLocalBackend,
    /// Credential present but not configured.
    CredentialNotConfigured,
    /// Credential environment variable not set.
    CredentialEnvMissing,
    /// Credential present but invalid.
    CredentialInvalid,
    /// Provider is in cooldown after repeated failures.
    CooldownActive,
    /// Provider was not built (feature-gated or compiled out).
    NotBuilt,
    /// Catch-all for unrecognized skip conditions.
    Unknown,
}

impl ProviderSkipCode {
    /// Stable snake-case string for programmatic matching.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UnknownProvider => "unknown_provider",
            Self::DisabledByUser => "disabled_by_user",
            Self::MissingApiKey => "missing_api_key",
            Self::MissingSearxngConfig => "missing_searxng_config",
            Self::MissingBaseUrl => "missing_base_url",
            Self::InvalidBaseUrl => "invalid_base_url",
            Self::MissingLocalBackend => "missing_local_backend",
            Self::CredentialNotConfigured => "credential_not_configured",
            Self::CredentialEnvMissing => "credential_env_missing",
            Self::CredentialInvalid => "credential_invalid",
            Self::CooldownActive => "cooldown_active",
            Self::NotBuilt => "not_built",
            Self::Unknown => "unknown",
        }
    }

    /// Human-readable display name for CLI / logging.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::UnknownProvider => "Unknown provider",
            Self::DisabledByUser => "Disabled by user",
            Self::MissingApiKey => "Missing API key",
            Self::MissingSearxngConfig => "SearXNG not configured",
            Self::MissingBaseUrl => "Missing base URL",
            Self::InvalidBaseUrl => "Invalid base URL",
            Self::MissingLocalBackend => "Local backend not available",
            Self::CredentialNotConfigured => "Credential not configured",
            Self::CredentialEnvMissing => "Credential environment variable not set",
            Self::CredentialInvalid => "Credential invalid (empty)",
            Self::CooldownActive => "Cooldown active",
            Self::NotBuilt => "Not built",
            Self::Unknown => "Unknown",
        }
    }
}

/// Full descriptor for a built-in provider, returned by
/// `provider_status` and the `eggsearch providers` CLI.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ProviderDescriptor {
    /// Stable provider id, e.g. `"duckduckgo"`.
    pub id: String,
    /// Human-readable display name, e.g. `"DuckDuckGo"`.
    pub display_name: String,
    /// Kind of engine.
    pub kind: ProviderKind,
    /// Whether the provider is enabled in the server's effective config.
    pub enabled: bool,
    /// Whether the provider appears in the server's `default_providers`.
    pub default: bool,
    /// Whether the provider requires an API key.
    pub requires_api_key: bool,
    /// Whether the provider is fully configured (e.g. SearXNG has a
    /// non-empty `base_url`). Disabled providers are always reported as
    /// `configured: false`.
    pub configured: bool,
    /// Feature capabilities.
    pub capabilities: ProviderCapabilities,
    /// Whether this provider can actually be used right now (enabled +
    /// configured and not in cooldown).
    #[serde(default)]
    pub routable: bool,
    /// Human-readable reason if not routable.
    #[serde(default)]
    pub skip_reason: Option<String>,
    /// Machine-actionable skip code for programmatic handling.
    #[serde(default)]
    pub skip_code: Option<ProviderSkipCode>,
}

/// Build a [`ProviderDescriptor`] for a known provider id.
///
/// Returns `None` for unknown ids. The `enabled`, `is_default`, and
/// `configured` flags are caller-supplied so the descriptor reflects
/// the actual server config state.
pub fn built_in_provider_descriptor(
    id: &str,
    enabled: bool,
    is_default: bool,
    configured: bool,
    routable: bool,
    skip_reason: Option<String>,
    skip_code: Option<ProviderSkipCode>,
) -> Option<ProviderDescriptor> {
    match id {
        "duckduckgo" => Some(ProviderDescriptor {
            id: "duckduckgo".into(),
            display_name: "DuckDuckGo".into(),
            kind: ProviderKind::HtmlScrape,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured: configured && enabled,
            capabilities: ProviderCapabilities::none(),
            routable,
            skip_reason,
            skip_code,
        }),
        "brave" => Some(ProviderDescriptor {
            id: "brave".into(),
            display_name: "Brave".into(),
            kind: ProviderKind::HtmlScrape,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured: configured && enabled,
            capabilities: ProviderCapabilities::none(),
            routable,
            skip_reason,
            skip_code,
        }),
        "startpage" => Some(ProviderDescriptor {
            id: "startpage".into(),
            display_name: "Startpage".into(),
            kind: ProviderKind::HtmlScrape,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured: configured && enabled,
            capabilities: ProviderCapabilities::none(),
            routable,
            skip_reason,
            skip_code,
        }),
        "yahoo" => Some(ProviderDescriptor {
            id: "yahoo".into(),
            display_name: "Yahoo".into(),
            kind: ProviderKind::HtmlScrape,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured: configured && enabled,
            capabilities: ProviderCapabilities::none(),
            routable,
            skip_reason,
            skip_code,
        }),
        "mojeek" => Some(ProviderDescriptor {
            id: "mojeek".into(),
            display_name: "Mojeek".into(),
            kind: ProviderKind::HtmlScrape,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured: configured && enabled,
            capabilities: ProviderCapabilities::none(),
            routable,
            skip_reason,
            skip_code,
        }),
        "searxng" => Some(ProviderDescriptor {
            id: "searxng".into(),
            display_name: "SearXNG".into(),
            kind: ProviderKind::JsonApi,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: false,
                supports_repo_filter: false,
                supports_org_filter: false,
                supports_path_filter: false,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: false,
                supports_release_search: false,
                supports_result_timestamps: false,
                supports_security_search: false,
                supports_package_metadata: false,
                supports_advisory_lookup_by_id: false,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: false,
                supports_structured_changelog: false,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "brave_api" => Some(ProviderDescriptor {
            id: "brave_api".into(),
            display_name: "Brave Search API".into(),
            kind: ProviderKind::ApiKey,
            enabled,
            default: is_default,
            requires_api_key: true,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: false,
                supports_repo_filter: false,
                supports_org_filter: false,
                supports_path_filter: false,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: false,
                supports_release_search: false,
                supports_result_timestamps: false,
                supports_security_search: false,
                supports_package_metadata: false,
                supports_advisory_lookup_by_id: false,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: false,
                supports_structured_changelog: false,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "github_code" => Some(ProviderDescriptor {
            id: "github_code".into(),
            display_name: "GitHub Code Search".into(),
            kind: ProviderKind::ApiKey,
            enabled,
            default: is_default,
            requires_api_key: true,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: true,
                supports_repo_filter: true,
                supports_org_filter: true,
                supports_path_filter: true,
                supports_language_filter: true,
                supports_symbol_hint: true,
                supports_issue_search: false,
                supports_release_search: false,
                supports_result_timestamps: false,
                supports_security_search: false,
                supports_package_metadata: false,
                supports_advisory_lookup_by_id: false,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: false,
                supports_structured_changelog: false,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "github_issues" => Some(ProviderDescriptor {
            id: "github_issues".into(),
            display_name: "GitHub Issues Search".into(),
            kind: ProviderKind::ApiKey,
            enabled,
            default: is_default,
            requires_api_key: true,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: false,
                supports_repo_filter: true,
                supports_org_filter: true,
                supports_path_filter: false,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: true,
                supports_release_search: false,
                supports_result_timestamps: true,
                supports_security_search: false,
                supports_package_metadata: false,
                supports_advisory_lookup_by_id: false,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: false,
                supports_structured_changelog: false,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "github_releases" => Some(ProviderDescriptor {
            id: "github_releases".into(),
            display_name: "GitHub Releases".into(),
            kind: ProviderKind::ApiKey,
            enabled,
            default: is_default,
            requires_api_key: true,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: false,
                supports_repo_filter: true,
                supports_org_filter: false,
                supports_path_filter: false,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: false,
                supports_release_search: true,
                supports_result_timestamps: true,
                supports_security_search: false,
                supports_package_metadata: false,
                supports_advisory_lookup_by_id: false,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: false,
                supports_structured_changelog: false,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "gitlab_code" => Some(ProviderDescriptor {
            id: "gitlab_code".into(),
            display_name: "GitLab Code Search".into(),
            kind: ProviderKind::ApiKey,
            enabled,
            default: is_default,
            requires_api_key: true,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: true,
                supports_repo_filter: true,
                supports_org_filter: true,
                supports_path_filter: true,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: false,
                supports_release_search: false,
                supports_result_timestamps: false,
                supports_security_search: false,
                supports_package_metadata: false,
                supports_advisory_lookup_by_id: false,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: false,
                supports_structured_changelog: false,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "gitlab_issues" => Some(ProviderDescriptor {
            id: "gitlab_issues".into(),
            display_name: "GitLab Issues Search".into(),
            kind: ProviderKind::ApiKey,
            enabled,
            default: is_default,
            requires_api_key: true,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: false,
                supports_repo_filter: true,
                supports_org_filter: true,
                supports_path_filter: false,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: true,
                supports_release_search: false,
                supports_result_timestamps: true,
                supports_security_search: false,
                supports_package_metadata: false,
                supports_advisory_lookup_by_id: false,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: false,
                supports_structured_changelog: false,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "gitlab_releases" => Some(ProviderDescriptor {
            id: "gitlab_releases".into(),
            display_name: "GitLab Releases".into(),
            kind: ProviderKind::ApiKey,
            enabled,
            default: is_default,
            requires_api_key: true,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: false,
                supports_repo_filter: true,
                supports_org_filter: false,
                supports_path_filter: false,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: false,
                supports_release_search: true,
                supports_result_timestamps: true,
                supports_security_search: false,
                supports_package_metadata: false,
                supports_advisory_lookup_by_id: false,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: false,
                supports_structured_changelog: false,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "gitea_code" => Some(ProviderDescriptor {
            id: "gitea_code".into(),
            display_name: "Gitea/Forgejo Code Search".into(),
            kind: ProviderKind::ApiKey,
            enabled,
            default: is_default,
            requires_api_key: true,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: true,
                supports_repo_filter: false,
                supports_org_filter: false,
                supports_path_filter: false,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: false,
                supports_release_search: false,
                supports_result_timestamps: false,
                supports_security_search: false,
                supports_package_metadata: false,
                supports_advisory_lookup_by_id: false,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: false,
                supports_structured_changelog: false,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "gitea_issues" => Some(ProviderDescriptor {
            id: "gitea_issues".into(),
            display_name: "Gitea/Forgejo Issues".into(),
            kind: ProviderKind::ApiKey,
            enabled,
            default: is_default,
            requires_api_key: true,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: false,
                supports_repo_filter: false,
                supports_org_filter: false,
                supports_path_filter: false,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: true,
                supports_release_search: false,
                supports_result_timestamps: true,
                supports_security_search: false,
                supports_package_metadata: false,
                supports_advisory_lookup_by_id: false,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: false,
                supports_structured_changelog: false,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "gitea_releases" => Some(ProviderDescriptor {
            id: "gitea_releases".into(),
            display_name: "Gitea/Forgejo Releases".into(),
            kind: ProviderKind::ApiKey,
            enabled,
            default: is_default,
            requires_api_key: true,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: false,
                supports_repo_filter: false,
                supports_org_filter: false,
                supports_path_filter: false,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: false,
                supports_release_search: true,
                supports_result_timestamps: true,
                supports_security_search: false,
                supports_package_metadata: false,
                supports_advisory_lookup_by_id: false,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: false,
                supports_structured_changelog: false,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "osv" => Some(ProviderDescriptor {
            id: "osv".into(),
            display_name: "OSV (Open Source Vulnerabilities)".into(),
            kind: ProviderKind::JsonApi,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: false,
                supports_repo_filter: false,
                supports_org_filter: false,
                supports_path_filter: false,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: false,
                supports_release_search: false,
                supports_result_timestamps: false,
                supports_security_search: true,
                supports_package_metadata: false,
                supports_advisory_lookup_by_id: false,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: false,
                supports_structured_changelog: false,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "github_advisory" => Some(ProviderDescriptor {
            id: "github_advisory".into(),
            display_name: "GitHub Security Advisories".into(),
            kind: ProviderKind::ApiKey,
            enabled,
            default: is_default,
            requires_api_key: true,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: false,
                supports_repo_filter: false,
                supports_org_filter: false,
                supports_path_filter: false,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: false,
                supports_release_search: false,
                supports_result_timestamps: false,
                supports_security_search: true,
                supports_package_metadata: false,
                supports_advisory_lookup_by_id: true,
                supports_advisory_lookup_by_package: true,
                supports_exploit_kev_status: false,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: false,
                supports_structured_changelog: false,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "nvd" => Some(ProviderDescriptor {
            id: "nvd".into(),
            display_name: "NIST National Vulnerability Database".into(),
            kind: ProviderKind::JsonApi,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: true,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: false,
                supports_repo_filter: false,
                supports_org_filter: false,
                supports_path_filter: false,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: false,
                supports_release_search: false,
                supports_result_timestamps: false,
                supports_security_search: true,
                supports_package_metadata: false,
                supports_advisory_lookup_by_id: true,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: false,
                supports_structured_changelog: false,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "cisa_kev" => Some(ProviderDescriptor {
            id: "cisa_kev".into(),
            display_name: "CISA Known Exploited Vulnerabilities".into(),
            kind: ProviderKind::JsonApi,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: false,
                supports_repo_filter: false,
                supports_org_filter: false,
                supports_path_filter: false,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: false,
                supports_release_search: false,
                supports_result_timestamps: false,
                supports_security_search: false,
                supports_package_metadata: false,
                supports_advisory_lookup_by_id: true,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: true,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: false,
                supports_structured_changelog: false,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "rustsec" => Some(ProviderDescriptor {
            id: "rustsec".into(),
            display_name: "RustSec Advisory Database".into(),
            kind: ProviderKind::JsonApi,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: false,
                supports_repo_filter: false,
                supports_org_filter: false,
                supports_path_filter: false,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: false,
                supports_release_search: false,
                supports_result_timestamps: false,
                supports_security_search: true,
                supports_package_metadata: false,
                supports_advisory_lookup_by_id: true,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: false,
                supports_structured_changelog: false,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "crates_io" => Some(ProviderDescriptor {
            id: "crates_io".into(),
            display_name: "crates.io".into(),
            kind: ProviderKind::JsonApi,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: false,
                supports_repo_filter: false,
                supports_org_filter: false,
                supports_path_filter: false,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: false,
                supports_release_search: false,
                supports_result_timestamps: false,
                supports_security_search: false,
                supports_package_metadata: true,
                supports_advisory_lookup_by_id: false,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: false,
                supports_structured_changelog: true,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "pypi" => Some(ProviderDescriptor {
            id: "pypi".into(),
            display_name: "PyPI".into(),
            kind: ProviderKind::JsonApi,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: false,
                supports_repo_filter: false,
                supports_org_filter: false,
                supports_path_filter: false,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: false,
                supports_release_search: false,
                supports_result_timestamps: false,
                supports_security_search: false,
                supports_package_metadata: true,
                supports_advisory_lookup_by_id: false,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: false,
                supports_structured_changelog: true,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "npm_registry" => Some(ProviderDescriptor {
            id: "npm_registry".into(),
            display_name: "npm".into(),
            kind: ProviderKind::JsonApi,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: false,
                supports_repo_filter: false,
                supports_org_filter: false,
                supports_path_filter: false,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: false,
                supports_release_search: false,
                supports_result_timestamps: false,
                supports_security_search: false,
                supports_package_metadata: true,
                supports_advisory_lookup_by_id: false,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: false,
                supports_structured_changelog: true,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "go_pkg" => Some(ProviderDescriptor {
            id: "go_pkg".into(),
            display_name: "Go Proxy".into(),
            kind: ProviderKind::JsonApi,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: false,
                supports_repo_filter: false,
                supports_org_filter: false,
                supports_path_filter: false,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: false,
                supports_release_search: false,
                supports_result_timestamps: false,
                supports_security_search: false,
                supports_package_metadata: true,
                supports_advisory_lookup_by_id: false,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: false,
                supports_structured_changelog: true,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "maven_central" => Some(ProviderDescriptor {
            id: "maven_central".into(),
            display_name: "Maven Central".into(),
            kind: ProviderKind::JsonApi,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: false,
                supports_repo_filter: false,
                supports_org_filter: false,
                supports_path_filter: false,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: false,
                supports_release_search: false,
                supports_result_timestamps: false,
                supports_security_search: false,
                supports_package_metadata: true,
                supports_advisory_lookup_by_id: false,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: false,
                supports_structured_changelog: true,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "nuget" => Some(ProviderDescriptor {
            id: "nuget".into(),
            display_name: "NuGet".into(),
            kind: ProviderKind::JsonApi,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: false,
                supports_repo_filter: false,
                supports_org_filter: false,
                supports_path_filter: false,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: false,
                supports_release_search: false,
                supports_result_timestamps: false,
                supports_security_search: false,
                supports_package_metadata: true,
                supports_advisory_lookup_by_id: false,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: false,
                supports_structured_changelog: true,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "rubygems" => Some(ProviderDescriptor {
            id: "rubygems".into(),
            display_name: "RubyGems".into(),
            kind: ProviderKind::JsonApi,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: false,
                supports_repo_filter: false,
                supports_org_filter: false,
                supports_path_filter: false,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: false,
                supports_release_search: false,
                supports_result_timestamps: false,
                supports_security_search: false,
                supports_package_metadata: true,
                supports_advisory_lookup_by_id: false,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: false,
                supports_structured_changelog: true,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "packagist" => Some(ProviderDescriptor {
            id: "packagist".into(),
            display_name: "Packagist".into(),
            kind: ProviderKind::JsonApi,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: false,
                supports_repo_filter: false,
                supports_org_filter: false,
                supports_path_filter: false,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: false,
                supports_release_search: false,
                supports_result_timestamps: false,
                supports_security_search: false,
                supports_package_metadata: true,
                supports_advisory_lookup_by_id: false,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: false,
                supports_structured_changelog: true,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "local_workspace" => Some(ProviderDescriptor {
            id: "local_workspace".into(),
            display_name: "Local Workspace".into(),
            kind: ProviderKind::Local,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: true,
                supports_repo_filter: false,
                supports_org_filter: false,
                supports_path_filter: true,
                supports_language_filter: true,
                supports_symbol_hint: false,
                supports_issue_search: false,
                supports_release_search: false,
                supports_result_timestamps: false,
                supports_security_search: false,
                supports_package_metadata: false,
                supports_advisory_lookup_by_id: false,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: false,
                supports_structured_changelog: false,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "openalex" => Some(ProviderDescriptor {
            id: "openalex".into(),
            display_name: "OpenAlex".into(),
            kind: ProviderKind::JsonApi,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: false,
                supports_repo_filter: false,
                supports_org_filter: false,
                supports_path_filter: false,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: false,
                supports_release_search: false,
                supports_result_timestamps: true,
                supports_security_search: false,
                supports_package_metadata: false,
                supports_advisory_lookup_by_id: false,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: true,
                supports_doi_lookup: true,
                supports_repo_indexing: false,
                supports_structured_changelog: false,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "crossref" => Some(ProviderDescriptor {
            id: "crossref".into(),
            display_name: "Crossref".into(),
            kind: ProviderKind::JsonApi,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: false,
                supports_repo_filter: false,
                supports_org_filter: false,
                supports_path_filter: false,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: false,
                supports_release_search: false,
                supports_result_timestamps: false,
                supports_security_search: false,
                supports_package_metadata: false,
                supports_advisory_lookup_by_id: false,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: true,
                supports_doi_lookup: true,
                supports_repo_indexing: false,
                supports_structured_changelog: false,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "semantic_scholar" => Some(ProviderDescriptor {
            id: "semantic_scholar".into(),
            display_name: "Semantic Scholar".into(),
            kind: ProviderKind::ApiKey,
            enabled,
            default: is_default,
            requires_api_key: true,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: false,
                supports_repo_filter: false,
                supports_org_filter: false,
                supports_path_filter: false,
                supports_language_filter: false,
                supports_symbol_hint: false,
                supports_issue_search: false,
                supports_release_search: false,
                supports_result_timestamps: false,
                supports_security_search: false,
                supports_package_metadata: false,
                supports_advisory_lookup_by_id: false,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: true,
                supports_doi_lookup: true,
                supports_repo_indexing: false,
                supports_structured_changelog: false,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        "sourcegraph" => Some(ProviderDescriptor {
            id: "sourcegraph".into(),
            display_name: "Sourcegraph".into(),
            kind: ProviderKind::ApiKey,
            enabled,
            default: is_default,
            requires_api_key: true,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: false,
                supports_freshness: false,
                supports_language: false,
                supports_region: false,
                supports_domain_filters: false,
                supports_news: false,
                supports_code_search: true,
                supports_repo_filter: false,
                supports_org_filter: false,
                supports_path_filter: true,
                supports_language_filter: true,
                supports_symbol_hint: false,
                supports_issue_search: false,
                supports_release_search: false,
                supports_result_timestamps: false,
                supports_security_search: false,
                supports_package_metadata: false,
                supports_advisory_lookup_by_id: false,
                supports_advisory_lookup_by_package: false,
                supports_exploit_kev_status: false,
                supports_scholarly_search: false,
                supports_doi_lookup: false,
                supports_repo_indexing: true,
                supports_structured_changelog: false,
            },
            routable,
            skip_reason,
            skip_code,
        }),
        _ => None,
    }
}

/// Compute the correct [`ProviderSkipCode`] for a non-routable provider.
///
/// Returns `None` when `routable` is true.
pub fn provider_skip_code(
    id: &str,
    kind: ProviderKind,
    is_known: bool,
    is_enabled: bool,
    configured: bool,
    searxng_configured: bool,
    routable: bool,
) -> Option<ProviderSkipCode> {
    if routable {
        return None;
    }
    if !is_known {
        return Some(ProviderSkipCode::NotBuilt);
    }
    if !is_enabled {
        return Some(ProviderSkipCode::DisabledByUser);
    }
    if !configured {
        match kind {
            ProviderKind::Local => {
                return Some(ProviderSkipCode::MissingLocalBackend);
            }
            ProviderKind::ApiKey => {
                return Some(ProviderSkipCode::MissingApiKey);
            }
            _ => {
                if id.contains("searxng") && !searxng_configured {
                    return Some(ProviderSkipCode::MissingSearxngConfig);
                }
                if is_api_provider(id) {
                    return Some(ProviderSkipCode::MissingApiKey);
                }
                return Some(ProviderSkipCode::Unknown);
            }
        }
    }
    Some(ProviderSkipCode::Unknown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_provider_ids_are_all_describable() {
        for id in KNOWN_PROVIDER_IDS {
            let desc = built_in_provider_descriptor(id, true, false, true, false, None, None)
                .expect("known id should have descriptor");
            assert_eq!(desc.id, *id);
        }
    }

    #[test]
    fn unknown_provider_returns_none() {
        assert!(
            built_in_provider_descriptor("ghost", true, false, true, false, None, None).is_none()
        );
    }

    #[test]
    fn capabilities_summary_basic() {
        let caps = ProviderCapabilities::none();
        assert_eq!(caps.summary(), "basic");
    }

    #[test]
    fn capabilities_summary_searxng() {
        let desc =
            built_in_provider_descriptor("searxng", true, false, true, false, None, None).unwrap();
        let summary = desc.capabilities.summary();
        // SearXNG adapter only forwards hardcoded en-US/general params;
        // no capability flags are set because none are actually passed through.
        assert_eq!(summary, "basic");
    }

    #[test]
    fn searxng_configured_false_when_disabled() {
        let desc =
            built_in_provider_descriptor("searxng", false, false, true, false, None, None).unwrap();
        assert!(!desc.configured);
    }

    #[test]
    fn searxng_configured_true_when_enabled_and_configured() {
        let desc =
            built_in_provider_descriptor("searxng", true, false, true, false, None, None).unwrap();
        assert!(desc.configured);
    }

    #[test]
    fn duckduckgo_descriptor_configured_false_when_disabled() {
        let desc =
            built_in_provider_descriptor("duckduckgo", false, false, true, false, None, None)
                .unwrap();
        assert!(!desc.configured);
        assert!(!desc.enabled);
    }

    #[test]
    fn osv_descriptor_configured_false_when_disabled() {
        let desc =
            built_in_provider_descriptor("osv", false, false, true, false, None, None).unwrap();
        assert!(!desc.configured);
        assert!(!desc.enabled);
    }

    #[test]
    fn provider_configured_state_matches_provider_kind() {
        assert!(provider_configured_state("duckduckgo", false, false, false));
        assert!(provider_configured_state("osv", false, false, false));
        assert!(provider_configured_state("searxng", true, false, false));
        assert!(!provider_configured_state("searxng", false, true, false));
        assert!(provider_configured_state("brave_api", false, true, false));
        assert!(!provider_configured_state("brave_api", false, false, false));
        assert!(provider_configured_state(
            "local_workspace",
            false,
            false,
            true
        ));
        assert!(!provider_configured_state(
            "local_workspace",
            false,
            false,
            false
        ));
    }

    #[test]
    fn provider_kind_serde_roundtrip() {
        let kind = ProviderKind::HtmlScrape;
        let json = serde_json::to_string(&kind).unwrap();
        let parsed: ProviderKind = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, kind);
    }

    #[test]
    fn provider_descriptor_serde_roundtrip() {
        let desc = built_in_provider_descriptor("duckduckgo", true, true, true, false, None, None)
            .unwrap();
        let json = serde_json::to_string(&desc).unwrap();
        let parsed: ProviderDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, desc.id);
        assert_eq!(parsed.kind, desc.kind);
        assert_eq!(parsed.enabled, desc.enabled);
        assert_eq!(parsed.default, desc.default);
        assert_eq!(parsed.capabilities, desc.capabilities);
    }

    #[test]
    fn brave_api_descriptor_is_api_key_kind() {
        let desc = built_in_provider_descriptor("brave_api", true, false, true, false, None, None)
            .expect("brave_api should have descriptor");
        assert_eq!(desc.id, "brave_api");
        assert_eq!(desc.display_name, "Brave Search API");
        assert_eq!(desc.kind, ProviderKind::ApiKey);
        assert!(desc.requires_api_key);
        assert!(desc.configured);
        assert!(desc.enabled);
        assert!(!desc.default);
    }

    #[test]
    fn brave_api_descriptor_configured_false_when_disabled() {
        let desc = built_in_provider_descriptor("brave_api", false, false, true, false, None, None)
            .unwrap();
        assert!(!desc.configured);
        assert!(!desc.enabled);
    }

    #[test]
    fn brave_api_descriptor_capabilities() {
        let desc = built_in_provider_descriptor("brave_api", true, false, true, false, None, None)
            .unwrap();
        // Brave API adapter only forwards q and count; safe_search,
        // freshness, language, and region are not passed through.
        assert!(!desc.capabilities.supports_safe_search);
        assert!(!desc.capabilities.supports_freshness);
        assert!(!desc.capabilities.supports_language);
        assert!(!desc.capabilities.supports_region);
        assert!(!desc.capabilities.supports_domain_filters);
        assert!(!desc.capabilities.supports_news);
    }

    #[test]
    fn brave_api_capabilities_summary() {
        let desc = built_in_provider_descriptor("brave_api", true, false, true, false, None, None)
            .unwrap();
        let summary = desc.capabilities.summary();
        assert_eq!(summary, "basic");
    }

    #[test]
    fn capability_option_supports_method() {
        let caps = ProviderCapabilities {
            supports_safe_search: true,
            supports_freshness: false,
            supports_language: true,
            supports_region: false,
            supports_domain_filters: true,
            supports_news: false,
            supports_code_search: false,
            supports_repo_filter: false,
            supports_org_filter: false,
            supports_path_filter: false,
            supports_language_filter: false,
            supports_symbol_hint: false,
            supports_issue_search: false,
            supports_release_search: false,
            supports_result_timestamps: false,
            supports_security_search: false,
            supports_package_metadata: false,
            supports_advisory_lookup_by_id: false,
            supports_advisory_lookup_by_package: false,
            supports_exploit_kev_status: false,
            supports_scholarly_search: false,
            supports_doi_lookup: false,
            supports_repo_indexing: false,
            supports_structured_changelog: false,
        };
        assert!(caps.supports(&CapabilityOption::SafeSearch));
        assert!(!caps.supports(&CapabilityOption::Freshness));
        assert!(caps.supports(&CapabilityOption::Language));
        assert!(!caps.supports(&CapabilityOption::Region));
        assert!(caps.supports(&CapabilityOption::DomainFilters));
        assert!(!caps.supports(&CapabilityOption::News));
        assert!(!caps.supports(&CapabilityOption::PackageMetadata));
        assert!(!caps.supports(&CapabilityOption::AdvisoryLookupById));
        assert!(!caps.supports(&CapabilityOption::AdvisoryLookupByPackage));
        assert!(!caps.supports(&CapabilityOption::ExploitKevStatus));
        assert!(!caps.supports(&CapabilityOption::ScholarlySearch));
        assert!(!caps.supports(&CapabilityOption::DoiLookup));
        assert!(!caps.supports(&CapabilityOption::RepoIndexing));
        assert!(!caps.supports(&CapabilityOption::StructuredChangelog));
    }

    #[test]
    fn capability_option_display_names() {
        assert_eq!(CapabilityOption::SafeSearch.display_name(), "safe_search");
        assert_eq!(CapabilityOption::Freshness.display_name(), "freshness");
        assert_eq!(CapabilityOption::Language.display_name(), "language");
        assert_eq!(CapabilityOption::Region.display_name(), "region");
        assert_eq!(
            CapabilityOption::DomainFilters.display_name(),
            "domain_filters"
        );
        assert_eq!(CapabilityOption::News.display_name(), "news");
        assert_eq!(
            CapabilityOption::PackageMetadata.display_name(),
            "package_metadata"
        );
        assert_eq!(
            CapabilityOption::AdvisoryLookupById.display_name(),
            "advisory_lookup_by_id"
        );
        assert_eq!(
            CapabilityOption::AdvisoryLookupByPackage.display_name(),
            "advisory_lookup_by_package"
        );
        assert_eq!(
            CapabilityOption::ExploitKevStatus.display_name(),
            "exploit_kev_status"
        );
        assert_eq!(
            CapabilityOption::ScholarlySearch.display_name(),
            "scholarly_search"
        );
        assert_eq!(CapabilityOption::DoiLookup.display_name(), "doi_lookup");
        assert_eq!(
            CapabilityOption::RepoIndexing.display_name(),
            "repo_indexing"
        );
        assert_eq!(
            CapabilityOption::StructuredChangelog.display_name(),
            "structured_changelog"
        );
    }

    #[test]
    fn capability_option_supports_none() {
        let caps = ProviderCapabilities::none();
        for option in [
            CapabilityOption::SafeSearch,
            CapabilityOption::Freshness,
            CapabilityOption::Language,
            CapabilityOption::Region,
            CapabilityOption::DomainFilters,
            CapabilityOption::News,
            CapabilityOption::CodeSearch,
            CapabilityOption::RepoFilter,
            CapabilityOption::OrgFilter,
            CapabilityOption::PathFilter,
            CapabilityOption::LanguageFilter,
            CapabilityOption::SymbolHint,
            CapabilityOption::IssueSearch,
            CapabilityOption::ReleaseSearch,
            CapabilityOption::ResultTimestamps,
            CapabilityOption::SecuritySearch,
            CapabilityOption::PackageMetadata,
            CapabilityOption::AdvisoryLookupById,
            CapabilityOption::AdvisoryLookupByPackage,
            CapabilityOption::ExploitKevStatus,
            CapabilityOption::ScholarlySearch,
            CapabilityOption::DoiLookup,
            CapabilityOption::RepoIndexing,
            CapabilityOption::StructuredChangelog,
        ] {
            assert!(
                !caps.supports(&option),
                "ProviderCapabilities::none() should not support {}",
                option.display_name()
            );
        }
    }

    #[test]
    fn new_capability_flags_default_false() {
        let caps = ProviderCapabilities::none();
        assert!(!caps.supports_package_metadata);
        assert!(!caps.supports_advisory_lookup_by_id);
        assert!(!caps.supports_advisory_lookup_by_package);
        assert!(!caps.supports_exploit_kev_status);
        assert!(!caps.supports_scholarly_search);
        assert!(!caps.supports_doi_lookup);
        assert!(!caps.supports_repo_indexing);
        assert!(!caps.supports_structured_changelog);
    }

    // ------------------------------------------------------------------
    // Freshness vs result_timestamps semantics.
    //
    // `supports_freshness` is the provider-side flag: the upstream
    // engine accepts a freshness/time-range parameter and applies it
    // server-side.
    //
    // `supports_result_timestamps` is the client-side flag: the
    // provider's result payloads include per-result timestamps that
    // eggsearch uses for local freshness reranking.
    //
    // GitHub issues/releases currently use the client-side model only
    // (they do not pass a freshness hint upstream). These tests pin
    // the (false, true) shape so the distinction cannot drift.
    // ------------------------------------------------------------------

    #[test]
    fn github_issues_supports_result_timestamps_but_not_freshness() {
        let desc =
            built_in_provider_descriptor("github_issues", true, false, true, false, None, None)
                .unwrap();
        // Provider-side: false. The /search/issues endpoint does not
        // accept a freshness parameter.
        assert!(!desc.capabilities.supports_freshness);
        // Client-side: true. The payload carries `updated_at` which
        // eggsearch uses for local freshness reranking.
        assert!(desc.capabilities.supports_result_timestamps);
    }

    #[test]
    fn github_releases_supports_result_timestamps_but_not_freshness() {
        let desc =
            built_in_provider_descriptor("github_releases", true, false, true, false, None, None)
                .unwrap();
        assert!(!desc.capabilities.supports_freshness);
        assert!(desc.capabilities.supports_result_timestamps);
    }

    #[test]
    fn github_code_supports_neither_freshness_flag() {
        let desc =
            built_in_provider_descriptor("github_code", true, false, true, false, None, None)
                .unwrap();
        assert!(!desc.capabilities.supports_freshness);
        assert!(!desc.capabilities.supports_result_timestamps);
    }

    #[test]
    fn html_scrape_providers_supports_neither_freshness_flag() {
        for id in ["duckduckgo", "brave", "startpage", "yahoo", "mojeek"] {
            let desc =
                built_in_provider_descriptor(id, true, false, true, false, None, None).unwrap();
            assert!(
                !desc.capabilities.supports_freshness,
                "{id} should not advertise provider-side freshness"
            );
            assert!(
                !desc.capabilities.supports_result_timestamps,
                "{id} should not advertise result timestamps"
            );
        }
    }

    // --- Gitea/Forgejo capability flag audit ---
    //
    // These tests pin the capability flags for Gitea/Forgejo providers so
    // they cannot drift into overclaiming features that are not wired.

    #[test]
    fn gitea_code_capabilities_are_conservative() {
        let desc = built_in_provider_descriptor("gitea_code", true, false, true, false, None, None)
            .unwrap();
        assert!(desc.capabilities.supports_code_search);
        // Gitea global search API does not support repo/path/language filters.
        assert!(!desc.capabilities.supports_repo_filter);
        assert!(!desc.capabilities.supports_path_filter);
        assert!(!desc.capabilities.supports_language_filter);
        assert!(!desc.capabilities.supports_symbol_hint);
        assert!(!desc.capabilities.supports_issue_search);
        assert!(!desc.capabilities.supports_release_search);
        assert!(!desc.capabilities.supports_result_timestamps);
        assert!(!desc.capabilities.supports_security_search);
    }

    #[test]
    fn gitea_issues_capabilities_are_conservative() {
        let desc =
            built_in_provider_descriptor("gitea_issues", true, false, true, false, None, None)
                .unwrap();
        assert!(desc.capabilities.supports_issue_search);
        assert!(desc.capabilities.supports_result_timestamps);
        // Gitea issues search does not support repo/language filters.
        assert!(!desc.capabilities.supports_repo_filter);
        assert!(!desc.capabilities.supports_path_filter);
        assert!(!desc.capabilities.supports_language_filter);
        assert!(!desc.capabilities.supports_code_search);
        assert!(!desc.capabilities.supports_release_search);
        assert!(!desc.capabilities.supports_security_search);
    }

    #[test]
    fn gitea_releases_capabilities_are_conservative() {
        let desc =
            built_in_provider_descriptor("gitea_releases", true, false, true, false, None, None)
                .unwrap();
        assert!(desc.capabilities.supports_release_search);
        assert!(desc.capabilities.supports_result_timestamps);
        // Gitea releases API does not support repo/language/code search.
        assert!(!desc.capabilities.supports_repo_filter);
        assert!(!desc.capabilities.supports_path_filter);
        assert!(!desc.capabilities.supports_language_filter);
        assert!(!desc.capabilities.supports_code_search);
        assert!(!desc.capabilities.supports_issue_search);
        assert!(!desc.capabilities.supports_security_search);
    }

    #[test]
    fn forgejo_providers_share_gitea_descriptors() {
        let code = built_in_provider_descriptor("gitea_code", true, false, true, false, None, None)
            .unwrap();
        assert!(code.capabilities.supports_code_search);
        let issues =
            built_in_provider_descriptor("gitea_issues", true, false, true, false, None, None)
                .unwrap();
        assert!(issues.capabilities.supports_issue_search);
        let releases =
            built_in_provider_descriptor("gitea_releases", true, false, true, false, None, None)
                .unwrap();
        assert!(releases.capabilities.supports_release_search);
    }

    #[test]
    fn gitea_providers_do_not_claim_tree_or_repo_map() {
        for id in ["gitea_code", "gitea_issues", "gitea_releases"] {
            let desc =
                built_in_provider_descriptor(id, true, false, true, false, None, None).unwrap();
            // None of these flags exist in ProviderCapabilities currently,
            // but if they are added, they must not be claimed for Gitea.
            assert!(
                !desc.capabilities.supports_security_search,
                "{id} should not claim security search"
            );
        }
    }

    #[test]
    fn api_provider_ids_are_all_known() {
        for id in API_PROVIDER_IDS {
            assert!(
                KNOWN_PROVIDER_IDS.contains(id),
                "API_PROVIDER_IDS entry {id} must also be in KNOWN_PROVIDER_IDS"
            );
        }
    }

    #[test]
    fn api_provider_ids_match_requires_api_key() {
        for id in API_PROVIDER_IDS {
            let desc = built_in_provider_descriptor(id, true, false, true, false, None, None)
                .expect("known API provider");
            assert!(
                desc.requires_api_key,
                "API_PROVIDER_IDS entry {id} should have requires_api_key=true"
            );
        }
    }

    #[test]
    fn is_api_provider_matches_api_provider_ids() {
        for id in KNOWN_PROVIDER_IDS {
            assert_eq!(
                is_api_provider(id),
                API_PROVIDER_IDS.contains(id),
                "is_api_provider({id}) should match API_PROVIDER_IDS membership"
            );
        }
    }

    #[test]
    fn non_api_provider_returns_false() {
        assert!(!is_api_provider("duckduckgo"));
        assert!(!is_api_provider("searxng"));
        assert!(!is_api_provider("local_workspace"));
        assert!(!is_api_provider("nonexistent"));
    }

    #[test]
    fn provider_skip_code_serde_roundtrip() {
        let variants = [
            ProviderSkipCode::UnknownProvider,
            ProviderSkipCode::DisabledByUser,
            ProviderSkipCode::MissingApiKey,
            ProviderSkipCode::MissingSearxngConfig,
            ProviderSkipCode::MissingBaseUrl,
            ProviderSkipCode::InvalidBaseUrl,
            ProviderSkipCode::MissingLocalBackend,
            ProviderSkipCode::CredentialNotConfigured,
            ProviderSkipCode::CredentialEnvMissing,
            ProviderSkipCode::CredentialInvalid,
            ProviderSkipCode::CooldownActive,
            ProviderSkipCode::NotBuilt,
            ProviderSkipCode::Unknown,
        ];
        for variant in &variants {
            let json = serde_json::to_string(variant).unwrap();
            let parsed: ProviderSkipCode = serde_json::from_str(&json).unwrap();
            assert_eq!(&parsed, variant, "roundtrip failed for {json}");
        }
    }

    #[test]
    fn provider_skip_code_as_str_matches_serialized_form() {
        let variants = [
            ProviderSkipCode::UnknownProvider,
            ProviderSkipCode::DisabledByUser,
            ProviderSkipCode::MissingApiKey,
            ProviderSkipCode::MissingSearxngConfig,
            ProviderSkipCode::MissingBaseUrl,
            ProviderSkipCode::InvalidBaseUrl,
            ProviderSkipCode::MissingLocalBackend,
            ProviderSkipCode::CredentialNotConfigured,
            ProviderSkipCode::CredentialEnvMissing,
            ProviderSkipCode::CredentialInvalid,
            ProviderSkipCode::CooldownActive,
            ProviderSkipCode::NotBuilt,
            ProviderSkipCode::Unknown,
        ];
        for variant in &variants {
            let json = serde_json::to_string(variant).unwrap();
            let expected = format!("\"{}\"", variant.as_str());
            assert_eq!(json, expected, "as_str mismatch for {variant:?}");
        }
    }

    #[test]
    fn provider_skip_code_display_name_non_empty() {
        let variants = [
            ProviderSkipCode::UnknownProvider,
            ProviderSkipCode::DisabledByUser,
            ProviderSkipCode::MissingApiKey,
            ProviderSkipCode::MissingSearxngConfig,
            ProviderSkipCode::MissingBaseUrl,
            ProviderSkipCode::InvalidBaseUrl,
            ProviderSkipCode::MissingLocalBackend,
            ProviderSkipCode::CredentialNotConfigured,
            ProviderSkipCode::CredentialEnvMissing,
            ProviderSkipCode::CredentialInvalid,
            ProviderSkipCode::CooldownActive,
            ProviderSkipCode::NotBuilt,
            ProviderSkipCode::Unknown,
        ];
        for variant in &variants {
            let name = variant.display_name();
            assert!(!name.is_empty(), "display_name is empty for {variant:?}");
        }
    }

    #[test]
    fn provider_skip_code_routable_returns_none() {
        let result = provider_skip_code(
            "duckduckgo",
            ProviderKind::HtmlScrape,
            true,
            true,
            true,
            false,
            true,
        );
        assert_eq!(result, None);
    }

    #[test]
    fn provider_skip_code_unknown_provider_not_built() {
        let result = provider_skip_code(
            "nonexistent",
            ProviderKind::HtmlScrape,
            false,
            true,
            true,
            false,
            false,
        );
        assert_eq!(result, Some(ProviderSkipCode::NotBuilt));
    }

    #[test]
    fn provider_skip_code_disabled_by_user() {
        let result = provider_skip_code(
            "duckduckgo",
            ProviderKind::HtmlScrape,
            true,
            false,
            true,
            false,
            false,
        );
        assert_eq!(result, Some(ProviderSkipCode::DisabledByUser));
    }

    #[test]
    fn provider_skip_code_missing_searxng_config() {
        let result = provider_skip_code(
            "searxng",
            ProviderKind::JsonApi,
            true,
            true,
            false,
            false,
            false,
        );
        assert_eq!(result, Some(ProviderSkipCode::MissingSearxngConfig));
    }

    #[test]
    fn provider_skip_code_missing_api_key() {
        let result = provider_skip_code(
            "brave_api",
            ProviderKind::ApiKey,
            true,
            true,
            false,
            false,
            false,
        );
        assert_eq!(result, Some(ProviderSkipCode::MissingApiKey));
    }

    #[test]
    fn provider_skip_code_missing_local_backend() {
        let result = provider_skip_code(
            "local_workspace",
            ProviderKind::Local,
            true,
            true,
            false,
            false,
            false,
        );
        assert_eq!(result, Some(ProviderSkipCode::MissingLocalBackend));
    }

    #[test]
    fn provider_descriptor_with_skip_code() {
        let desc = built_in_provider_descriptor(
            "duckduckgo",
            true,
            true,
            true,
            true,
            None,
            Some(ProviderSkipCode::DisabledByUser),
        )
        .unwrap();
        assert_eq!(desc.skip_code, Some(ProviderSkipCode::DisabledByUser));
        assert!(desc.routable);
    }
}
