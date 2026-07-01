//! Core types, source card model, configuration, and error types
//! for eggsearch. This module is intentionally independent of any MCP,
//! HTTP, or search-engine implementation.

pub mod batch_fetch;
pub mod code_evidence;
pub mod code_host_fetch;
pub mod code_metadata;
pub mod config;
pub mod document;
pub mod error;
/// Deterministic error-message parser and subquery generator for exact-error search mode.
pub mod error_query;
pub mod fetch;
/// Local workspace search types: config, file entries, and search requests.
pub mod local;
/// Package coordinate types and ecosystem resolution for repo_search.
pub mod package;
pub mod provider;
/// Deterministic result quality and uncertainty metadata.
pub mod quality;
pub mod query;
/// Structured repository fetch request/response types and validation.
pub mod repo_fetch;
/// Repository map types for structured repo discovery.
pub mod repo_map;
/// Repo-oriented query hint parser for structured search.
pub mod repo_query;
pub mod repo_search;
pub mod research;
pub mod result;
pub mod sanitize;
pub mod security;
pub mod security_applicability;
pub mod source_card;

pub use crate::fetch::span::SelectedSpan;
pub use batch_fetch::{BatchFetchItem, BatchFetchItemType, BatchFetchResponse, BatchFetchResult};
pub use code_evidence::{
    build_code_evidence, infer_source_role, CodeEvidence, CodeEvidenceReason, EvidenceConfidence,
    SourceRole, SymbolKind,
};
pub use code_host_fetch::{resolve_code_host_fetch_target, CodeHostFetchTarget};
pub use code_metadata::{CodeHost, CodeMetadata};
pub use config::{AppConfig, LiveConfig, Mode, SearchSection};
pub use document::{
    BlockKind, DocumentChunk, DocumentKind, DocumentOutlineEntry, FetchDocument,
    FetchRenderMetadata, RenderFormat, RenderedBlock,
};
pub use error::{CoreError, CoreResult};
pub use error_query::{
    ErrorCode, ErrorQueryParts, ErrorSearchContext, ErrorSubquery, ExactErrorConfig, StackFrameHint,
};
pub use fetch::{
    ExtractMode, ExtractedLink, FetchTransform, FetchTransformKind, FetchTrust, WebFetchRequest,
    WebFetchResponse,
};
pub use local::{LocalConfig, LocalFileEntry, LocalMatch, LocalSearchRequest, LocalSearchResult};
pub use package::{
    ecosystem_to_osv, user_ecosystem_to_osv, PackageCoordinate, PackageEcosystem, PackageResolution,
};
pub use provider::{
    built_in_provider_descriptor, ProviderCapabilities, ProviderDescriptor, ProviderKind,
    KNOWN_PROVIDER_IDS,
};
pub use quality::{
    compute_card_quality, compute_group_quality, AuthorityEstimate, EvidenceStrength,
    FreshnessEstimate, GroupQualitySummary, QualityReason, RelevanceEstimate, ResultConfidence,
    ResultQuality, SearchUncertaintySummary, UncertaintyReason,
};
pub use query::{
    resolve_max_results, Freshness, MaxResultsResolution, SafeSearch, SearchIntent,
    WebSearchRequest,
};
pub use repo_fetch::{
    apply_line_range, github_browser_url, github_permalink_url, github_raw_url, gitlab_browser_url,
    gitlab_raw_url, RepoFetchRequest, RepoFetchResponse, RepoFetchedLine, RepoLocator,
};
pub use repo_map::{
    classify_important_directory, classify_important_file, ImportantDirKind, ImportantFileKind,
    RepoImportantDirectory, RepoImportantFile, RepoMapEntry, RepoMapEntryKind, RepoMapMode,
    RepoMapRequest, RepoMapResponse, RepoMapSuggestedFetch, RepoPathSummary,
};
pub use repo_query::RepoQueryHints;
pub use repo_search::{
    ProviderSelectionTelemetry, RepoIdentitySource, RepoResultGroup, RepoResultGroupKind,
    RepoSearchMode, RepoSearchRequest, RepoSearchResponse, RepoSearchSubqueryTelemetry,
    RepoSearchTelemetry, RepoSuggestedFetch, ResolvedRepoIdentity, SearchProfile,
};
pub use research::{
    EvidenceQuality, ResearchCoverage, ResearchDepth, ResearchDimension, ResearchDomain,
    ResearchGap, ResearchGapKind, ResearchResultGroup, ResearchResultGroupKind,
    ResearchSearchRequest, ResearchSearchResponse, ResearchSourceType, ResearchSubquery,
    ResearchSuggestedFetch, ResearchTelemetry, ResearchWorkflow, ResearchWorkflowContext,
};
pub use result::{SearchWarning, TrustLevel};
pub use sanitize::{
    bound_text, frame, scan_injection_markers, strip_control_chars, MarkerHit, TrustMarkers,
    SNIPPET_MAX_CHARS, TITLE_MAX_CHARS,
};
pub use security::{
    assess_source_quality, build_identifier_list, classify_query_kind, classify_source_tier,
    AffectedPackageSummary, CompactSecurityContext, DefensiveGuidance, DefensiveGuidanceCategory,
    KevMetadata, SecurityContext, SecurityIdentifier, SecurityIdentifierKind, SecurityIdentifiers,
    SecurityQueryKind, SecurityResultGroup, SecurityResultGroupKind, SecuritySearchRequest,
    SecuritySearchResponse, SecuritySourceQuality, SecuritySourceTier, SecuritySuggestedFetch,
    SeverityLevel, VulnerabilityMetadata, VulnerabilityReference, VulnerabilitySource,
    VulnerabilitySummary,
};
pub use security_applicability::{
    AdvisoryRange, ApplicabilityAssessment, ApplicabilityConfidence, ApplicabilityStatus,
    DependencyFinding, DependencySource,
};
pub use source_card::{RankReason, SourceCard, SourceKind, SourceMetadata};
