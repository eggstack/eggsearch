//! Types for the repo-oriented structured search (repo_search) tool.

use crate::core::code_metadata::CodeHost;
use crate::core::fetch::ExtractMode;
use crate::core::query::{resolve_max_results, Freshness};
use crate::core::repo_query::RepoQueryHints;
use crate::core::result::SearchWarning;
use crate::core::sanitize::TrustMarkers;
use crate::core::source_card::{SourceCard, SourceKind};
use crate::meta::response::ProviderFailure;
use serde::{Deserialize, Serialize};

/// Named search profile that controls provider selection behavior.
///
/// Profiles are advisory: they influence which providers are selected
/// when no explicit `providers` list is given, and they emit telemetry
/// so callers can observe degradation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SearchProfile {
    /// Default behavior: use configured default providers.
    #[default]
    Generic,
    /// Prefer native code/issues/releases providers when configured,
    /// then API/web providers, then generic HTML fallback.
    Coding,
    /// Prefer OSV and security-capable providers, then authoritative
    /// advisory domains through generic search.
    Security,
    /// Prefer diverse source discovery and broad web/API providers.
    Research,
}

impl SearchProfile {
    /// Parse a profile string, accepting common aliases.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "generic" | "default" | "web" => Some(Self::Generic),
            "coding" | "code" | "repo" | "repository" | "project" => Some(Self::Coding),
            "security" | "vuln" | "vulnerability" | "advisory" => Some(Self::Security),
            "research" | "deep" | "thorough" => Some(Self::Research),
            _ => None,
        }
    }

    /// Stable snake-case string form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::Coding => "coding",
            Self::Security => "security",
            Self::Research => "research",
        }
    }
}

impl std::fmt::Display for SearchProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Telemetry for provider selection in a search response.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProviderSelectionTelemetry {
    /// The profile that was requested by the caller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_requested: Option<SearchProfile>,
    /// The profile that was actually applied (may differ if the
    /// requested profile was unavailable or fell back).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_applied: Option<SearchProfile>,
    /// Whether the profile fell back to generic providers.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub degraded: bool,
    /// Human-readable reason for the provider selection or degradation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Telemetry for a single subquery in a repo search.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RepoSearchSubqueryTelemetry {
    /// Label for this subquery (e.g. "docs", "source", "issues").
    pub label: String,
    /// The query text that was sent to providers.
    pub query: String,
    /// Which result group this subquery targets.
    pub intended_group: Option<String>,
    /// Required native capability for this subquery (e.g. "code_search",
    /// "issue_search", "release_search"), if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_capability: Option<String>,
    /// Provider IDs that were attempted for this subquery.
    pub providers_attempted: Vec<String>,
}

/// Telemetry for a repo search response.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RepoSearchTelemetry {
    /// Provider selection telemetry.
    pub provider_selection: ProviderSelectionTelemetry,
    /// Subqueries that were generated and dispatched.
    pub subqueries: Vec<RepoSearchSubqueryTelemetry>,
    /// Whether the request-level deadline was exceeded.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deadline_exceeded: bool,
    /// Number of subqueries that were interrupted by deadline.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub subqueries_interrupted: usize,
    /// Number of subqueries that were skipped (never started).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub subqueries_skipped: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// Structured request for repo-oriented bundle search.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RepoSearchRequest {
    /// Required. Free-text query. May contain repo hints (repo:owner/name, etc.).
    pub query: String,
    /// Optional. Code host to target (github, gitlab, codeberg).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<CodeHost>,
    /// Optional. Repository owner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Optional. Repository name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    /// Optional. Organization filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub org: Option<String>,
    /// Optional. Path hint for source files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Optional. File hint for source files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Optional. Language filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Optional. Symbol hint for code search.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// Optional. Include official documentation results. Default true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_docs: Option<bool>,
    /// Optional. Include package registry results. Default true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_registry: Option<bool>,
    /// Optional. Include issue results. Default true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_issues: Option<bool>,
    /// Optional. Include release results. Default true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_releases: Option<bool>,
    /// Optional. Include example results. Default true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_examples: Option<bool>,
    /// Optional. Include pull request results. Default true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_pull_requests: Option<bool>,
    /// Optional. Maximum total results to return.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_results: Option<usize>,
    /// Optional. Maximum results per group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_per_group: Option<usize>,
    /// Optional. Freshness hint for results.
    #[serde(default)]
    pub freshness: Freshness,
    /// Optional. Per-request timeout override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// Optional. Explicit provider ID list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers: Vec<String>,
    /// Optional. Search profile for provider selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<SearchProfile>,
}

impl RepoSearchRequest {
    /// Validate the request, returning an error if invalid.
    pub fn validate(&self, max_query_chars: usize) -> Result<(), String> {
        if self.query.trim().is_empty() {
            return Err("query must not be empty".to_string());
        }
        if self.query.chars().count() > max_query_chars {
            return Err(format!("query must be <= {max_query_chars} characters"));
        }
        if let Some(0) = self.max_results {
            return Err("max_results must be > 0".to_string());
        }
        Ok(())
    }

    /// Effective max_results, defaulting to the given default.
    pub fn effective_max_results(&self, default: usize, cap: usize) -> usize {
        resolve_max_results(self.max_results, default, cap).effective
    }

    /// Merge explicit fields with parsed hints from the query string.
    /// Explicit fields always override parsed query hints.
    pub fn resolved_hints(&self) -> RepoQueryHints {
        let mut hints = RepoQueryHints::parse(&self.query);
        if self.host.is_some() {
            hints.host = self.host;
        }
        if self.owner.is_some() {
            hints.owner = self.owner.clone();
        }
        if self.repo.is_some() {
            hints.repo = self.repo.clone();
        }
        if self.org.is_some() {
            hints.org = self.org.clone();
        }
        if self.path.is_some() {
            hints.path = self.path.clone();
        }
        if self.file.is_some() {
            hints.file = self.file.clone();
        }
        if self.language.is_some() {
            hints.language = self.language.clone().map(|s| s.to_lowercase());
        }
        if self.symbol.is_some() {
            hints.symbol = self.symbol.clone();
        }
        hints
    }

    /// Whether official docs results are included (default true).
    pub fn include_docs_enabled(&self) -> bool {
        self.include_docs.unwrap_or(true)
    }

    /// Whether package registry results are included (default true).
    pub fn include_registry_enabled(&self) -> bool {
        self.include_registry.unwrap_or(true)
    }

    /// Whether issue results are included (default true).
    pub fn include_issues_enabled(&self) -> bool {
        self.include_issues.unwrap_or(true)
    }

    /// Whether release results are included (default true).
    pub fn include_releases_enabled(&self) -> bool {
        self.include_releases.unwrap_or(true)
    }

    /// Whether example results are included (default true).
    pub fn include_examples_enabled(&self) -> bool {
        self.include_examples.unwrap_or(true)
    }

    /// Whether pull request results are included (default true).
    pub fn include_pull_requests_enabled(&self) -> bool {
        self.include_pull_requests.unwrap_or(true)
    }
}

/// Classification for result groups.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RepoResultGroupKind {
    /// Official documentation (docs.rs, MDN, etc.).
    #[default]
    OfficialDocs,
    /// Package registry listing (crates.io, npm, PyPI, etc.).
    PackageRegistry,
    /// Repository root or general repository reference.
    Repository,
    /// README file.
    Readme,
    /// Example code or demo projects.
    Examples,
    /// Test files and test suites.
    Tests,
    /// Individual source files.
    SourceFiles,
    /// Issue tracker entries.
    Issues,
    /// Pull requests.
    PullRequests,
    /// Releases, tags, and security advisories.
    Releases,
    /// Migration guides and upgrade notes.
    MigrationNotes,
    /// Changelog entries.
    Changelog,
    /// Community discussions, tutorials, and forums.
    CommunityDiscussion,
    /// Unclassified results.
    Other,
}

/// A group of source cards sharing a classification.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RepoResultGroup {
    /// The classification kind for this group.
    pub kind: RepoResultGroupKind,
    /// Human-readable label for the group.
    pub label: String,
    /// Source cards in this group.
    pub results: Vec<SourceCard>,
    /// Whether additional results were truncated.
    pub truncated: bool,
}

/// A suggested URL for the caller to fetch.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RepoSuggestedFetch {
    /// The URL to fetch.
    pub url: String,
    /// Why this URL is suggested.
    pub reason: String,
    /// Which result group this fetch belongs to.
    pub group: RepoResultGroupKind,
    /// Expected content kind (e.g. "documentation", "source").
    pub expected_kind: SourceKind,
    /// Recommended extract mode for the fetch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_extract_mode: Option<ExtractMode>,
    /// Priority (lower is higher priority).
    pub priority: u8,
    /// Optional structured repo_fetch request. When present, the
    /// caller can use `repo_fetch` with these locator fields instead
    /// of constructing a raw URL. Only populated for source-file
    /// results with code evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_repo_fetch: Option<super::repo_fetch::RepoFetchRequest>,
}

/// Response from repo_search.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RepoSearchResponse {
    /// The original query string.
    pub query: String,
    /// Search mode used.
    pub mode: String,
    /// Resolved hints merged from explicit fields and query tokens.
    pub resolved_hints: RepoQueryHints,
    /// Human-readable summary of resolved hints.
    pub resolved_hints_summary: String,
    /// Grouped results.
    pub groups: Vec<RepoResultGroup>,
    /// Suggested URLs to fetch next.
    pub suggested_fetches: Vec<RepoSuggestedFetch>,
    /// Provider IDs that were queried.
    pub providers_queried: Vec<String>,
    /// Per-provider failures, if any.
    pub providers_failed: Vec<ProviderFailure>,
    /// Aggregated warnings.
    pub warnings: Vec<SearchWarning>,
    /// Aggregate trust markers across all results.
    pub trust_markers: TrustMarkers,
    /// Telemetry for provider selection, subqueries, and deadline status.
    pub telemetry: RepoSearchTelemetry,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_empty_query() {
        let req = RepoSearchRequest {
            query: "   ".to_string(),
            ..Default::default()
        };
        assert!(req.validate(512).is_err());
    }

    #[test]
    fn validate_rejects_oversized_query() {
        let req = RepoSearchRequest {
            query: "a".repeat(1000),
            ..Default::default()
        };
        assert!(req.validate(512).is_err());
    }

    #[test]
    fn validate_rejects_zero_max_results() {
        let req = RepoSearchRequest {
            query: "test".to_string(),
            max_results: Some(0),
            ..Default::default()
        };
        assert!(req.validate(512).is_err());
    }

    #[test]
    fn validate_accepts_valid_query() {
        let req = RepoSearchRequest {
            query: "axum middleware".to_string(),
            ..Default::default()
        };
        assert!(req.validate(512).is_ok());
    }

    #[test]
    fn effective_max_results_defaults() {
        let req = RepoSearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        assert_eq!(req.effective_max_results(10, 50), 10);
    }

    #[test]
    fn effective_max_results_clamps_to_cap() {
        let req = RepoSearchRequest {
            query: "test".to_string(),
            max_results: Some(100),
            ..Default::default()
        };
        assert_eq!(req.effective_max_results(10, 50), 50);
    }

    #[test]
    fn default_include_flags_all_true() {
        let req = RepoSearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        assert!(req.include_docs_enabled());
        assert!(req.include_registry_enabled());
        assert!(req.include_issues_enabled());
        assert!(req.include_releases_enabled());
        assert!(req.include_examples_enabled());
        assert!(req.include_pull_requests_enabled());
    }

    #[test]
    fn explicit_false_flags_disable_includes() {
        let req = RepoSearchRequest {
            query: "test".to_string(),
            include_docs: Some(false),
            include_registry: Some(false),
            include_issues: Some(false),
            include_releases: Some(false),
            include_examples: Some(false),
            include_pull_requests: Some(false),
            ..Default::default()
        };
        assert!(!req.include_docs_enabled());
        assert!(!req.include_registry_enabled());
        assert!(!req.include_issues_enabled());
        assert!(!req.include_releases_enabled());
        assert!(!req.include_examples_enabled());
        assert!(!req.include_pull_requests_enabled());
    }

    #[test]
    fn resolved_hints_merges_explicit_fields() {
        let req = RepoSearchRequest {
            query: "router middleware".to_string(),
            owner: Some("tokio-rs".to_string()),
            repo: Some("axum".to_string()),
            host: Some(CodeHost::Github),
            ..Default::default()
        };
        let hints = req.resolved_hints();
        assert_eq!(hints.owner.as_deref(), Some("tokio-rs"));
        assert_eq!(hints.repo.as_deref(), Some("axum"));
        assert_eq!(hints.host, Some(CodeHost::Github));
        assert_eq!(hints.residual_query, "router middleware");
    }

    #[test]
    fn resolved_hints_explicit_fields_override_query() {
        let req = RepoSearchRequest {
            query: "repo:serde-rs/serde serializer".to_string(),
            owner: Some("other-owner".to_string()),
            repo: Some("other-repo".to_string()),
            ..Default::default()
        };
        let hints = req.resolved_hints();
        assert_eq!(hints.owner.as_deref(), Some("other-owner"));
        assert_eq!(hints.repo.as_deref(), Some("other-repo"));
    }

    #[test]
    fn resolved_hints_lowercases_language() {
        let req = RepoSearchRequest {
            query: "middleware".to_string(),
            language: Some("Python".to_string()),
            ..Default::default()
        };
        let hints = req.resolved_hints();
        assert_eq!(hints.language.as_deref(), Some("python"));
    }

    #[test]
    fn repo_result_group_kind_default() {
        assert_eq!(
            RepoResultGroupKind::default(),
            RepoResultGroupKind::OfficialDocs
        );
    }

    #[test]
    fn serde_roundtrip_request() {
        let req = RepoSearchRequest {
            query: "test".to_string(),
            host: Some(CodeHost::Github),
            owner: Some("tokio-rs".to_string()),
            ..Default::default()
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: RepoSearchRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.query, req.query);
        assert_eq!(parsed.host, req.host);
        assert_eq!(parsed.owner, req.owner);
    }

    #[test]
    fn search_profile_parse_aliases() {
        assert_eq!(SearchProfile::parse("generic"), Some(SearchProfile::Generic));
        assert_eq!(SearchProfile::parse("default"), Some(SearchProfile::Generic));
        assert_eq!(SearchProfile::parse("web"), Some(SearchProfile::Generic));
        assert_eq!(SearchProfile::parse("coding"), Some(SearchProfile::Coding));
        assert_eq!(SearchProfile::parse("code"), Some(SearchProfile::Coding));
        assert_eq!(SearchProfile::parse("repo"), Some(SearchProfile::Coding));
        assert_eq!(SearchProfile::parse("repository"), Some(SearchProfile::Coding));
        assert_eq!(SearchProfile::parse("project"), Some(SearchProfile::Coding));
        assert_eq!(
            SearchProfile::parse("security"),
            Some(SearchProfile::Security)
        );
        assert_eq!(SearchProfile::parse("vuln"), Some(SearchProfile::Security));
        assert_eq!(
            SearchProfile::parse("advisory"),
            Some(SearchProfile::Security)
        );
        assert_eq!(
            SearchProfile::parse("research"),
            Some(SearchProfile::Research)
        );
        assert_eq!(SearchProfile::parse("deep"), Some(SearchProfile::Research));
        assert_eq!(
            SearchProfile::parse("thorough"),
            Some(SearchProfile::Research)
        );
        assert_eq!(SearchProfile::parse("unknown"), None);
        assert_eq!(SearchProfile::parse(""), None);
    }

    #[test]
    fn search_profile_as_str() {
        assert_eq!(SearchProfile::Generic.as_str(), "generic");
        assert_eq!(SearchProfile::Coding.as_str(), "coding");
        assert_eq!(SearchProfile::Security.as_str(), "security");
        assert_eq!(SearchProfile::Research.as_str(), "research");
    }

    #[test]
    fn search_profile_display() {
        assert_eq!(SearchProfile::Generic.to_string(), "generic");
        assert_eq!(SearchProfile::Coding.to_string(), "coding");
    }

    #[test]
    fn search_profile_default_is_generic() {
        assert_eq!(SearchProfile::default(), SearchProfile::Generic);
    }

    #[test]
    fn search_profile_serde_roundtrip() {
        let profiles = vec![
            SearchProfile::Generic,
            SearchProfile::Coding,
            SearchProfile::Security,
            SearchProfile::Research,
        ];
        for p in profiles {
            let json = serde_json::to_string(&p).unwrap();
            let parsed: SearchProfile = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, p);
        }
    }

    #[test]
    fn search_profile_serde_snake_case() {
        let json = serde_json::to_string(&SearchProfile::Coding).unwrap();
        assert_eq!(json, "\"coding\"");
        let json = serde_json::to_string(&SearchProfile::Security).unwrap();
        assert_eq!(json, "\"security\"");
    }

    #[test]
    fn request_with_profile_roundtrips() {
        let req = RepoSearchRequest {
            query: "test".to_string(),
            profile: Some(SearchProfile::Coding),
            ..Default::default()
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: RepoSearchRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.profile, Some(SearchProfile::Coding));
    }

    #[test]
    fn request_without_profile_omits_in_json() {
        let req = RepoSearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("profile"));
    }

    #[test]
    fn telemetry_roundtrip() {
        let telemetry = RepoSearchTelemetry {
            provider_selection: ProviderSelectionTelemetry {
                profile_requested: Some(SearchProfile::Coding),
                profile_applied: Some(SearchProfile::Coding),
                degraded: false,
                reason: Some("using coding profile providers".to_string()),
            },
            subqueries: vec![RepoSearchSubqueryTelemetry {
                label: "docs".to_string(),
                query: "tokio-rs/axum docs".to_string(),
                intended_group: Some("official_docs".to_string()),
                required_capability: None,
                providers_attempted: vec!["duckduckgo".to_string()],
            }],
            deadline_exceeded: false,
            subqueries_interrupted: 0,
            subqueries_skipped: 0,
        };
        let json = serde_json::to_string(&telemetry).unwrap();
        let parsed: RepoSearchTelemetry = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed.provider_selection.profile_requested,
            Some(SearchProfile::Coding)
        );
        assert_eq!(parsed.subqueries.len(), 1);
        assert_eq!(parsed.subqueries[0].label, "docs");
        assert!(!parsed.deadline_exceeded);
    }

    #[test]
    fn response_with_profile_includes_telemetry() {
        let response = RepoSearchResponse {
            query: "test".to_string(),
            mode: "repo_metasearch".to_string(),
            ..Default::default()
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("telemetry"));
        assert!(json.contains("provider_selection"));
    }
}
