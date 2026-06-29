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
    "local_workspace",
];

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
) -> Option<ProviderDescriptor> {
    match id {
        "duckduckgo" => Some(ProviderDescriptor {
            id: "duckduckgo".into(),
            display_name: "DuckDuckGo".into(),
            kind: ProviderKind::HtmlScrape,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured,
            capabilities: ProviderCapabilities::none(),
        }),
        "brave" => Some(ProviderDescriptor {
            id: "brave".into(),
            display_name: "Brave".into(),
            kind: ProviderKind::HtmlScrape,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured,
            capabilities: ProviderCapabilities::none(),
        }),
        "startpage" => Some(ProviderDescriptor {
            id: "startpage".into(),
            display_name: "Startpage".into(),
            kind: ProviderKind::HtmlScrape,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured,
            capabilities: ProviderCapabilities::none(),
        }),
        "yahoo" => Some(ProviderDescriptor {
            id: "yahoo".into(),
            display_name: "Yahoo".into(),
            kind: ProviderKind::HtmlScrape,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured,
            capabilities: ProviderCapabilities::none(),
        }),
        "mojeek" => Some(ProviderDescriptor {
            id: "mojeek".into(),
            display_name: "Mojeek".into(),
            kind: ProviderKind::HtmlScrape,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured,
            capabilities: ProviderCapabilities::none(),
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
            },
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
            },
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
            },
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
            },
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
            },
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
            },
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
            },
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
            },
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
            },
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
            },
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
            },
        }),
        "osv" => Some(ProviderDescriptor {
            id: "osv".into(),
            display_name: "OSV (Open Source Vulnerabilities)".into(),
            kind: ProviderKind::JsonApi,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured: true,
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
            },
        }),
        "local_workspace" => Some(ProviderDescriptor {
            id: "local_workspace".into(),
            display_name: "Local Workspace".into(),
            kind: ProviderKind::Local,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured,
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
            },
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_provider_ids_are_all_describable() {
        for id in KNOWN_PROVIDER_IDS {
            let desc = built_in_provider_descriptor(id, true, false, true)
                .expect("known id should have descriptor");
            assert_eq!(desc.id, *id);
        }
    }

    #[test]
    fn unknown_provider_returns_none() {
        assert!(built_in_provider_descriptor("ghost", true, false, true).is_none());
    }

    #[test]
    fn capabilities_summary_basic() {
        let caps = ProviderCapabilities::none();
        assert_eq!(caps.summary(), "basic");
    }

    #[test]
    fn capabilities_summary_searxng() {
        let desc = built_in_provider_descriptor("searxng", true, false, true).unwrap();
        let summary = desc.capabilities.summary();
        // SearXNG adapter only forwards hardcoded en-US/general params;
        // no capability flags are set because none are actually passed through.
        assert_eq!(summary, "basic");
    }

    #[test]
    fn searxng_configured_false_when_disabled() {
        let desc = built_in_provider_descriptor("searxng", false, false, true).unwrap();
        assert!(!desc.configured);
    }

    #[test]
    fn searxng_configured_true_when_enabled_and_configured() {
        let desc = built_in_provider_descriptor("searxng", true, false, true).unwrap();
        assert!(desc.configured);
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
        let desc = built_in_provider_descriptor("duckduckgo", true, true, true).unwrap();
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
        let desc = built_in_provider_descriptor("brave_api", true, false, true)
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
        let desc = built_in_provider_descriptor("brave_api", false, false, true).unwrap();
        assert!(!desc.configured);
        assert!(!desc.enabled);
    }

    #[test]
    fn brave_api_descriptor_capabilities() {
        let desc = built_in_provider_descriptor("brave_api", true, false, true).unwrap();
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
        let desc = built_in_provider_descriptor("brave_api", true, false, true).unwrap();
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
        };
        assert!(caps.supports(&CapabilityOption::SafeSearch));
        assert!(!caps.supports(&CapabilityOption::Freshness));
        assert!(caps.supports(&CapabilityOption::Language));
        assert!(!caps.supports(&CapabilityOption::Region));
        assert!(caps.supports(&CapabilityOption::DomainFilters));
        assert!(!caps.supports(&CapabilityOption::News));
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
        ] {
            assert!(
                !caps.supports(&option),
                "ProviderCapabilities::none() should not support {}",
                option.display_name()
            );
        }
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
        let desc = built_in_provider_descriptor("github_issues", true, false, true).unwrap();
        // Provider-side: false. The /search/issues endpoint does not
        // accept a freshness parameter.
        assert!(!desc.capabilities.supports_freshness);
        // Client-side: true. The payload carries `updated_at` which
        // eggsearch uses for local freshness reranking.
        assert!(desc.capabilities.supports_result_timestamps);
    }

    #[test]
    fn github_releases_supports_result_timestamps_but_not_freshness() {
        let desc = built_in_provider_descriptor("github_releases", true, false, true).unwrap();
        assert!(!desc.capabilities.supports_freshness);
        assert!(desc.capabilities.supports_result_timestamps);
    }

    #[test]
    fn github_code_supports_neither_freshness_flag() {
        let desc = built_in_provider_descriptor("github_code", true, false, true).unwrap();
        assert!(!desc.capabilities.supports_freshness);
        assert!(!desc.capabilities.supports_result_timestamps);
    }

    #[test]
    fn html_scrape_providers_supports_neither_freshness_flag() {
        for id in ["duckduckgo", "brave", "startpage", "yahoo", "mojeek"] {
            let desc = built_in_provider_descriptor(id, true, false, true).unwrap();
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
}
