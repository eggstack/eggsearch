//! Types for the research-oriented structured search (research_search) tool.

use crate::core::fetch::ExtractMode;
use crate::core::query::{resolve_max_results, Freshness, SearchIntent};
use crate::core::result::SearchWarning;
use crate::core::sanitize::TrustMarkers;
use crate::core::source_card::{SourceCard, SourceKind};
use crate::meta::response::ProviderFailure;
use serde::{Deserialize, Serialize};

/// Research domain classification.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ResearchDomain {
    /// General-purpose research (default).
    #[default]
    General,
    /// Software architecture patterns and system design.
    SoftwareArchitecture,
    /// API design patterns and conventions.
    ApiDesign,
    /// Distributed systems topics.
    DistributedSystems,
    /// Security vulnerabilities, advisories, and hardening.
    Security,
    /// Performance tuning and benchmarking.
    Performance,
    /// Language ecosystem discovery and comparison.
    LanguageEcosystem,
    /// Machine learning frameworks, models, and pipelines.
    MachineLearning,
    /// Infrastructure, deployment, and DevOps.
    Infrastructure,
}

impl ResearchDomain {
    /// Parse a research-domain string, accepting common aliases used by MCP callers.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "general" => Some(Self::General),
            "software_architecture" | "architecture" => Some(Self::SoftwareArchitecture),
            "api_design" | "api" => Some(Self::ApiDesign),
            "distributed_systems" | "distributed" => Some(Self::DistributedSystems),
            "security" => Some(Self::Security),
            "performance" => Some(Self::Performance),
            "language_ecosystem" | "ecosystem" => Some(Self::LanguageEcosystem),
            "machine_learning" | "ml" => Some(Self::MachineLearning),
            "infrastructure" | "infra" => Some(Self::Infrastructure),
            _ => None,
        }
    }
}

/// Classification of source types sought in a research query.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResearchSourceType {
    /// Peer-reviewed papers, standards-track documents, or formal specifications.
    PrimarySources,
    /// Official documentation sites or README files.
    OfficialDocs,
    /// Specifications and protocol definitions.
    Specifications,
    /// Reference implementations or canonical codebases.
    ReferenceImplementations,
    /// Design discussions, RFCs, and architecture decision records.
    DesignDiscussions,
    /// Benchmarks, performance reports, and measurements.
    Benchmarks,
    /// Security advisories, CVEs, and hardening guides.
    SecurityConsiderations,
    /// Issue threads and bug reports.
    IssueThreads,
    /// Release notes, changelogs, and migration guides.
    ReleaseNotes,
    /// Academic papers, theses, or formal verification results.
    AcademicOrFormalSources,
    /// Recent news articles and press releases.
    RecentNews,
    /// Community discussions, forum threads, and Stack Overflow.
    CommunityDiscussion,
    /// Counterpoints, criticism, or alternative viewpoints.
    Counterpoints,
}

impl ResearchSourceType {
    /// Parse a research source-type string, accepting stable names and short aliases.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "primary_sources" | "primary" => Some(Self::PrimarySources),
            "official_docs" | "docs" => Some(Self::OfficialDocs),
            "specifications" | "specs" => Some(Self::Specifications),
            "reference_implementations" | "reference" | "implementations" => {
                Some(Self::ReferenceImplementations)
            }
            "design_discussions" | "design" => Some(Self::DesignDiscussions),
            "benchmarks" | "benchmark" => Some(Self::Benchmarks),
            "security_considerations" | "security" => Some(Self::SecurityConsiderations),
            "issue_threads" | "issues" => Some(Self::IssueThreads),
            "release_notes" | "releases" => Some(Self::ReleaseNotes),
            "academic_or_formal_sources" | "academic" | "formal" => {
                Some(Self::AcademicOrFormalSources)
            }
            "recent_news" | "news" => Some(Self::RecentNews),
            "community_discussion" | "community" => Some(Self::CommunityDiscussion),
            "counterpoints" | "counterpoint" => Some(Self::Counterpoints),
            _ => None,
        }
    }
}

/// Quality tier of an evidence source.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceQuality {
    /// Official primary source from the project or standard body.
    OfficialPrimary,
    /// Primary source from a maintainer (blog post, talk, etc.).
    MaintainerPrimary,
    /// Standards-track or specification document.
    StandardsOrSpecification,
    /// Primary source from a vendor or platform.
    VendorPrimary,
    /// Package registry listing or metadata.
    PackageRegistry,
    /// Academic paper, thesis, or formal document.
    AcademicOrFormal,
    /// Benchmark or measurement result.
    BenchmarkOrMeasurement,
    /// Security advisory or vulnerability disclosure.
    SecurityAdvisory,
    /// Community discussion or forum thread.
    CommunityDiscussion,
    /// News article or press coverage.
    NewsOrPress,
    /// Blog post or tutorial.
    BlogOrTutorial,
    /// Unknown or unclassifiable source.
    Unknown,
}

/// Classification for research result groups.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ResearchResultGroupKind {
    /// Peer-reviewed papers, standards-track documents, or formal specifications.
    PrimarySources,
    /// Official documentation sites or README files.
    OfficialDocs,
    /// Specifications and protocol definitions.
    Specifications,
    /// Reference implementations or canonical codebases.
    ReferenceImplementations,
    /// Design discussions, RFCs, and architecture decision records.
    DesignDiscussions,
    /// Benchmarks, performance reports, and measurements.
    Benchmarks,
    /// Security advisories, CVEs, and hardening guides.
    SecurityConsiderations,
    /// Issue threads and bug reports.
    IssueThreads,
    /// Release notes, changelogs, and migration guides.
    ReleaseNotes,
    /// Academic papers, theses, or formal verification results.
    AcademicOrFormalSources,
    /// Recent news articles and press releases.
    RecentNews,
    /// Community discussions, forum threads, and Stack Overflow.
    CommunityDiscussion,
    /// Counterpoints, criticism, or alternative viewpoints.
    Counterpoints,
    /// Unclassified results.
    #[default]
    Unknown,
}

/// Research workflow classification for structured research scaffolding.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ResearchWorkflow {
    /// General-purpose research workflow.
    #[default]
    General,
    /// Evaluate an API or library for adoption.
    ApiEvaluation,
    /// Compare two or more libraries or frameworks.
    LibraryComparison,
    /// Plan a migration between versions or systems.
    MigrationPlanning,
    /// Security-focused review (advisories, threat models, hardening).
    SecurityReview,
    /// Performance investigation (benchmarks, profiling, tuning).
    PerformanceInvestigation,
    /// Broad ecosystem survey across multiple tools or libraries.
    EcosystemSurvey,
    /// Architecture decision research (patterns, tradeoffs, RFCs).
    ArchitectureDecision,
}

impl ResearchWorkflow {
    /// Parse a research workflow string, accepting stable names and short aliases.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "general" => Some(Self::General),
            "architecture_decision" | "architecture" => Some(Self::ArchitectureDecision),
            "api_evaluation" | "api" => Some(Self::ApiEvaluation),
            "library_comparison" | "comparison" => Some(Self::LibraryComparison),
            "migration_planning" | "migration" => Some(Self::MigrationPlanning),
            "security_review" | "security" => Some(Self::SecurityReview),
            "performance_investigation" | "performance" => Some(Self::PerformanceInvestigation),
            "ecosystem_survey" | "ecosystem" => Some(Self::EcosystemSurvey),
            _ => None,
        }
    }
}

/// Research depth controls source diversity and subquery breadth.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ResearchDepth {
    /// Quick scan: fewer subqueries, limited source diversity.
    Quick,
    /// Standard depth (default): balanced breadth and diversity.
    #[default]
    Standard,
    /// Deep dive: maximum subqueries and source diversity.
    Deep,
}

impl ResearchDepth {
    /// Parse a research depth string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "quick" => Some(Self::Quick),
            "standard" => Some(Self::Standard),
            "deep" => Some(Self::Deep),
            _ => None,
        }
    }
}

/// A research dimension — a named aspect of the research question.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResearchDimension {
    /// Short name for this dimension (e.g. "Official Docs").
    pub name: String,
    /// Purpose of this dimension in the research workflow.
    pub purpose: String,
    /// Source types that contribute to this dimension.
    pub source_types: Vec<ResearchSourceType>,
    /// Subqueries generated for this dimension.
    pub subqueries: Vec<String>,
}

/// Aggregate coverage counts across source types.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResearchCoverage {
    /// Number of primary/official source results found.
    pub primary_sources_found: usize,
    /// Number of official documentation results found.
    pub official_docs_found: usize,
    /// Number of implementation/reference source results found.
    pub implementation_sources_found: usize,
    /// Number of benchmark results found.
    pub benchmark_sources_found: usize,
    /// Number of security-related results found.
    pub security_sources_found: usize,
    /// Number of counterpoint/alternative viewpoint results found.
    pub counterpoints_found: usize,
    /// Number of recent/fresh results found.
    pub recent_sources_found: usize,
}

/// Kind of coverage gap detected.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResearchGapKind {
    /// No primary or official sources found.
    NoPrimarySources,
    /// No recent sources found.
    NoRecentSources,
    /// No counterpoints found when requested.
    NoCounterpoints,
    /// No implementation evidence found.
    NoImplementationEvidence,
    /// No benchmark results found.
    NoBenchmarks,
    /// No security discussion found.
    NoSecurityDiscussion,
    /// No migration docs found.
    NoMigrationDocs,
    /// Limited provider coverage.
    ProviderCoverageLimited,
    /// Ambiguous or underspecified research question.
    AmbiguousQuestion,
}

/// A coverage gap with guidance for the calling agent.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResearchGap {
    /// Kind of gap detected.
    pub kind: ResearchGapKind,
    /// Human-readable description of the gap.
    pub message: String,
    /// Suggested query to fill the gap, if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_query: Option<String>,
}

/// Workflow context block returned when workflow mode is active.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResearchWorkflowContext {
    /// The workflow type used.
    pub workflow: ResearchWorkflow,
    /// Interpreted research question.
    pub interpreted_question: String,
    /// Research dimensions explored.
    pub dimensions: Vec<ResearchDimension>,
    /// Aggregate coverage counts.
    pub coverage: ResearchCoverage,
    /// Coverage gaps detected.
    pub gaps: Vec<ResearchGap>,
    /// Recommended next fetches for the calling agent.
    pub recommended_next_fetches: Vec<ResearchSuggestedFetch>,
    /// Workflow-specific warnings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// Telemetry for the research workflow.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResearchTelemetry {
    /// Workflow type used, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow: Option<ResearchWorkflow>,
    /// Research depth used.
    pub depth: ResearchDepth,
    /// Number of dimensions generated.
    pub dimensions_generated: usize,
    /// Number of subqueries generated.
    pub subqueries_generated: usize,
    /// Diversity caps that were applied.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_diversity_caps_applied: Vec<String>,
    /// Coverage gap kinds detected.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coverage_gaps: Vec<ResearchGapKind>,
    /// Capability enforcement telemetry for this request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_enforcement:
        Option<crate::meta::provider_diagnostics::CapabilityEnforcementTelemetry>,
    /// Provider routing decision for this request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routing_decision: Option<crate::meta::provider_diagnostics::ProviderRoutingDecision>,
}

/// Structured request for research-oriented bundle search.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResearchSearchRequest {
    /// Required. Free-text research query.
    pub query: String,
    /// Optional. Research domain to scope the search.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub research_domain: Option<ResearchDomain>,
    /// Optional. Source types to include in the search.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub desired_source_types: Vec<ResearchSourceType>,
    /// Optional. Include counterpoints and alternative viewpoints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_counterpoints: Option<bool>,
    /// Optional. Prioritize primary sources over secondary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_primary_sources: Option<bool>,
    /// Optional. Include recent discussion and news.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_recent_discussion: Option<bool>,
    /// Optional. Include security-related considerations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_security_considerations: Option<bool>,
    /// Optional. Maximum total results to return.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_results: Option<usize>,
    /// Optional. Maximum result groups to return.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_groups: Option<usize>,
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

    /// Optional. Research workflow type for structured scaffolding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow: Option<ResearchWorkflow>,
    /// Optional. Research depth (quick, standard, deep).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<ResearchDepth>,
    /// Optional. Compare targets for library comparison workflows.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compare_targets: Vec<String>,
    /// Optional. Constraints or requirements for the research.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<String>,
    /// Optional. Known context the caller already has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub known_context: Option<String>,
}

impl ResearchSearchRequest {
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
        if let Some(0) = self.max_groups {
            return Err("max_groups must be > 0".to_string());
        }
        if let Some(0) = self.max_per_group {
            return Err("max_per_group must be > 0".to_string());
        }
        if self.desired_source_types.len() > 12 {
            return Err("desired_source_types must have <= 12 entries".to_string());
        }
        Ok(())
    }

    /// Effective max_results, defaulting to the given default.
    pub fn effective_max_results(&self, default: usize, cap: usize) -> usize {
        resolve_max_results(self.max_results, default, cap).effective
    }

    /// Effective max_groups, defaulting to the given default.
    pub fn effective_max_groups(&self, default: usize) -> usize {
        self.max_groups.unwrap_or(default).max(1)
    }

    /// Effective max_per_group, defaulting to the given default.
    pub fn effective_max_per_group(&self, default: usize) -> usize {
        self.max_per_group.unwrap_or(default).max(1)
    }

    /// Effective research depth, defaulting to Standard.
    pub fn effective_depth(&self) -> ResearchDepth {
        self.depth.unwrap_or_default()
    }

    /// Effective research workflow, defaulting to General.
    pub fn effective_workflow(&self) -> ResearchWorkflow {
        self.workflow.unwrap_or_default()
    }
}

/// A subquery generated for a specific source type.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResearchSubquery {
    /// Unique identifier for this subquery.
    pub id: String,
    /// Source type this subquery targets.
    pub source_type: ResearchSourceType,
    /// The rewritten query string for this source type.
    pub query: String,
    /// Search intent used for this subquery.
    pub intent: SearchIntent,
    /// Freshness filter applied to this subquery.
    pub freshness: Freshness,
}

/// A group of source cards sharing a classification.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResearchResultGroup {
    /// The classification kind for this group.
    pub kind: ResearchResultGroupKind,
    /// Human-readable label for the group.
    pub label: String,
    /// Source cards in this group.
    pub results: Vec<SourceCard>,
    /// Whether additional results were truncated.
    pub truncated: bool,
    /// Aggregate quality summary for this group's results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality_summary: Option<crate::core::quality::GroupQualitySummary>,
}

/// A suggested URL for the caller to fetch.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResearchSuggestedFetch {
    /// The URL to fetch.
    pub url: String,
    /// Which result group this fetch belongs to.
    pub group: ResearchResultGroupKind,
    /// Expected content kind (e.g. "documentation", "source").
    pub expected_kind: SourceKind,
    /// Evidence quality tier of the source.
    pub evidence_quality: EvidenceQuality,
    /// Why this URL is suggested.
    pub reason: String,
    /// Recommended extract mode for the fetch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_extract_mode: Option<ExtractMode>,
    /// Priority (lower is higher priority).
    pub priority: u8,
    /// Deterministic, content-derived identifier stable across runs.
    /// Format: `suggested_<16hex>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stable_id: Option<String>,
    /// Deterministic source card ID linking this suggested fetch back
    /// to the source card that produced it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    /// Deterministic score for this suggestion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<i32>,
    /// Rank reasons explaining why this fetch was scored as it was.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rank_reasons: Vec<String>,
    /// Information gain estimate (0.0 to 1.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub information_gain: Option<f32>,
}

/// Response from research_search.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResearchSearchResponse {
    /// The original query string.
    pub query: String,
    /// Search mode used.
    pub mode: String,
    /// Resolved research domain.
    pub research_domain: ResearchDomain,
    /// Subqueries generated for this research request.
    pub subqueries: Vec<ResearchSubquery>,
    /// Grouped results.
    pub groups: Vec<ResearchResultGroup>,
    /// Suggested URLs to fetch next.
    pub suggested_fetches: Vec<ResearchSuggestedFetch>,
    /// Provider IDs that were queried.
    pub providers_queried: Vec<String>,
    /// Per-provider failures, if any.
    pub providers_failed: Vec<ProviderFailure>,
    /// Aggregated warnings.
    pub warnings: Vec<SearchWarning>,
    /// Aggregate trust markers across all results.
    pub trust_markers: TrustMarkers,
    /// Workflow context block (present when workflow mode is active).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_context: Option<ResearchWorkflowContext>,
    /// Research telemetry for diagnostics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telemetry: Option<ResearchTelemetry>,
    /// Structured warnings with stable machine-readable codes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub structured_warnings: Vec<crate::core::warning::AgentWarning>,
    /// Suggested next actions for the agent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub next_actions: Vec<crate::core::workflow::AgentNextAction>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_empty_query() {
        let req = ResearchSearchRequest {
            query: "   ".to_string(),
            ..Default::default()
        };
        assert!(req.validate(512).is_err());
    }

    #[test]
    fn validate_rejects_oversized_query() {
        let req = ResearchSearchRequest {
            query: "a".repeat(1000),
            ..Default::default()
        };
        assert!(req.validate(512).is_err());
    }

    #[test]
    fn validate_rejects_zero_max_results() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            max_results: Some(0),
            ..Default::default()
        };
        assert!(req.validate(512).is_err());
    }

    #[test]
    fn validate_rejects_zero_max_groups() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            max_groups: Some(0),
            ..Default::default()
        };
        assert!(req.validate(512).is_err());
    }

    #[test]
    fn validate_rejects_zero_max_per_group() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            max_per_group: Some(0),
            ..Default::default()
        };
        assert!(req.validate(512).is_err());
    }

    #[test]
    fn validate_rejects_too_many_source_types() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            desired_source_types: vec![
                ResearchSourceType::OfficialDocs,
                ResearchSourceType::Specifications,
                ResearchSourceType::ReferenceImplementations,
                ResearchSourceType::DesignDiscussions,
                ResearchSourceType::Benchmarks,
                ResearchSourceType::SecurityConsiderations,
                ResearchSourceType::IssueThreads,
                ResearchSourceType::ReleaseNotes,
                ResearchSourceType::AcademicOrFormalSources,
                ResearchSourceType::RecentNews,
                ResearchSourceType::CommunityDiscussion,
                ResearchSourceType::Counterpoints,
                ResearchSourceType::PrimarySources,
            ],
            ..Default::default()
        };
        assert!(req.validate(512).is_err());
    }

    #[test]
    fn validate_accepts_valid_query() {
        let req = ResearchSearchRequest {
            query: "distributed consensus algorithms".to_string(),
            ..Default::default()
        };
        assert!(req.validate(512).is_ok());
    }

    #[test]
    fn effective_max_results_defaults() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        assert_eq!(req.effective_max_results(10, 50), 10);
    }

    #[test]
    fn effective_max_results_clamps_to_cap() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            max_results: Some(100),
            ..Default::default()
        };
        assert_eq!(req.effective_max_results(10, 50), 50);
    }

    #[test]
    fn effective_max_groups_defaults() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        assert_eq!(req.effective_max_groups(8), 8);
    }

    #[test]
    fn effective_max_groups_min_one() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            max_groups: Some(0),
            ..Default::default()
        };
        assert_eq!(req.effective_max_groups(8), 1);
    }

    #[test]
    fn effective_max_per_group_defaults() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        assert_eq!(req.effective_max_per_group(5), 5);
    }

    #[test]
    fn effective_max_per_group_min_one() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            max_per_group: Some(0),
            ..Default::default()
        };
        assert_eq!(req.effective_max_per_group(5), 1);
    }

    #[test]
    fn research_domain_default() {
        assert_eq!(ResearchDomain::default(), ResearchDomain::General);
    }

    #[test]
    fn research_enum_parsers_accept_documented_aliases() {
        assert_eq!(
            ResearchDomain::parse("architecture"),
            Some(ResearchDomain::SoftwareArchitecture)
        );
        assert_eq!(
            ResearchDomain::parse("ml"),
            Some(ResearchDomain::MachineLearning)
        );

        assert_eq!(
            ResearchSourceType::parse("docs"),
            Some(ResearchSourceType::OfficialDocs)
        );
        assert_eq!(
            ResearchSourceType::parse("releases"),
            Some(ResearchSourceType::ReleaseNotes)
        );

        assert_eq!(
            ResearchWorkflow::parse("comparison"),
            Some(ResearchWorkflow::LibraryComparison)
        );
        assert_eq!(ResearchDepth::parse("deep"), Some(ResearchDepth::Deep));
    }

    #[test]
    fn research_enum_parsers_reject_unknown_values() {
        assert_eq!(ResearchDomain::parse("literature"), None);
        assert_eq!(ResearchSourceType::parse("podcasts"), None);
        assert_eq!(ResearchWorkflow::parse("brainstorm"), None);
        assert_eq!(ResearchDepth::parse("exhaustive"), None);
    }

    #[test]
    fn research_result_group_kind_default() {
        assert_eq!(
            ResearchResultGroupKind::default(),
            ResearchResultGroupKind::Unknown
        );
    }

    #[test]
    fn serde_roundtrip_request() {
        let req = ResearchSearchRequest {
            query: "raft consensus".to_string(),
            research_domain: Some(ResearchDomain::DistributedSystems),
            desired_source_types: vec![
                ResearchSourceType::AcademicOrFormalSources,
                ResearchSourceType::ReferenceImplementations,
            ],
            include_counterpoints: Some(true),
            max_results: Some(20),
            ..Default::default()
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: ResearchSearchRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.query, req.query);
        assert_eq!(parsed.research_domain, req.research_domain);
        assert_eq!(parsed.desired_source_types, req.desired_source_types);
        assert_eq!(parsed.include_counterpoints, req.include_counterpoints);
        assert_eq!(parsed.max_results, req.max_results);
    }

    #[test]
    fn serde_roundtrip_response() {
        let resp = ResearchSearchResponse {
            query: "test".to_string(),
            mode: "live".to_string(),
            research_domain: ResearchDomain::Security,
            subqueries: vec![],
            groups: vec![],
            suggested_fetches: vec![],
            providers_queried: vec!["duckduckgo".to_string()],
            providers_failed: vec![],
            warnings: vec![],
            trust_markers: TrustMarkers::default(),
            workflow_context: None,
            telemetry: None,
            structured_warnings: vec![],
            next_actions: vec![],
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: ResearchSearchResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.query, resp.query);
        assert_eq!(parsed.research_domain, resp.research_domain);
    }

    #[test]
    fn workflow_default_is_general() {
        assert_eq!(ResearchWorkflow::default(), ResearchWorkflow::General);
    }

    #[test]
    fn depth_default_is_standard() {
        assert_eq!(ResearchDepth::default(), ResearchDepth::Standard);
    }

    #[test]
    fn effective_depth_defaults_to_standard() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        assert_eq!(req.effective_depth(), ResearchDepth::Standard);
    }

    #[test]
    fn effective_depth_from_request() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            depth: Some(ResearchDepth::Deep),
            ..Default::default()
        };
        assert_eq!(req.effective_depth(), ResearchDepth::Deep);
    }

    #[test]
    fn effective_workflow_defaults_to_general() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        assert_eq!(req.effective_workflow(), ResearchWorkflow::General);
    }

    #[test]
    fn effective_workflow_from_request() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            workflow: Some(ResearchWorkflow::LibraryComparison),
            ..Default::default()
        };
        assert_eq!(
            req.effective_workflow(),
            ResearchWorkflow::LibraryComparison
        );
    }

    #[test]
    fn serde_roundtrip_request_with_workflow() {
        let req = ResearchSearchRequest {
            query: "compare axum vs actix".to_string(),
            workflow: Some(ResearchWorkflow::LibraryComparison),
            depth: Some(ResearchDepth::Deep),
            compare_targets: vec!["axum".to_string(), "actix-web".to_string()],
            constraints: vec!["must support HTTP/2".to_string()],
            known_context: Some("already evaluated rocket".to_string()),
            ..Default::default()
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: ResearchSearchRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.workflow, Some(ResearchWorkflow::LibraryComparison));
        assert_eq!(parsed.depth, Some(ResearchDepth::Deep));
        assert_eq!(parsed.compare_targets, vec!["axum", "actix-web"]);
        assert_eq!(parsed.constraints, vec!["must support HTTP/2"]);
        assert_eq!(
            parsed.known_context,
            Some("already evaluated rocket".to_string())
        );
    }

    #[test]
    fn serde_roundtrip_response_with_workflow_context() {
        let resp = ResearchSearchResponse {
            query: "test".to_string(),
            mode: "research_metasearch".to_string(),
            research_domain: ResearchDomain::General,
            subqueries: vec![],
            groups: vec![],
            suggested_fetches: vec![],
            providers_queried: vec![],
            providers_failed: vec![],
            warnings: vec![],
            trust_markers: TrustMarkers::default(),
            workflow_context: Some(ResearchWorkflowContext {
                workflow: ResearchWorkflow::ArchitectureDecision,
                interpreted_question: "test question".to_string(),
                dimensions: vec![],
                coverage: ResearchCoverage::default(),
                gaps: vec![],
                recommended_next_fetches: vec![],
                warnings: vec![],
            }),
            telemetry: Some(ResearchTelemetry {
                workflow: Some(ResearchWorkflow::ArchitectureDecision),
                depth: ResearchDepth::Standard,
                dimensions_generated: 3,
                subqueries_generated: 6,
                source_diversity_caps_applied: vec![],
                coverage_gaps: vec![ResearchGapKind::NoPrimarySources],
                capability_enforcement: None,
                routing_decision: None,
            }),
            structured_warnings: vec![],
            next_actions: vec![],
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: ResearchSearchResponse = serde_json::from_str(&json).unwrap();
        let ctx = parsed.workflow_context.unwrap();
        assert_eq!(ctx.workflow, ResearchWorkflow::ArchitectureDecision);
        let telem = parsed.telemetry.unwrap();
        assert_eq!(telem.dimensions_generated, 3);
        assert_eq!(telem.coverage_gaps, vec![ResearchGapKind::NoPrimarySources]);
    }
}
