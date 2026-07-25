//! Core types, source card model, configuration, and error types
//! for eggsearch. This module is intentionally independent of any MCP,
//! HTTP, or search-engine implementation.

pub mod batch_fetch;
pub mod code_context;
pub mod code_evidence;
pub mod code_host_fetch;
pub mod code_metadata;
pub mod config;
/// Deterministic conflict detection metadata for contradictory source evidence.
pub mod conflict;
pub mod document;
pub mod error;
/// Deterministic error-message parser and subquery generator for exact-error search mode.
pub mod error_query;
/// Evidence bundle types for multi-agent handoff.
pub mod evidence_bundle;
/// Post-processing stage for evidence roles, conflicts, retrieval status, and workflow coverage.
pub mod evidence_postprocess;
/// Unified evidence role taxonomy mapping across source kinds, roles, classes, and tiers.
pub mod evidence_role;
pub mod fetch;
/// Deterministic cross-tool identity model for stable source/fetch/suggested IDs.
pub mod identity;
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
/// Response-level distinctions between evidence absence and retrieval failure.
pub mod retrieval_status;
pub mod sanitize;
pub mod security;
pub mod security_applicability;
pub mod source_card;
/// Structured warning model with stable codes, severity, and deduplication.
pub mod warning;
/// Agent workflow recipes and next-action hints for tool sequencing guidance.
pub mod workflow;
/// Deterministic coverage structures for workflow evidence models.
pub mod workflow_coverage;

pub use crate::fetch::span::SelectedSpan;
pub use batch_fetch::{BatchFetchItem, BatchFetchItemType, BatchFetchResponse, BatchFetchResult};
pub use code_context::{
    detect_language, detect_language_str, extract_code_context, extract_imports,
    find_enclosing_symbol, CodeContext, ExtractionLanguage,
};
pub use code_evidence::{
    build_code_evidence, infer_source_role, CodeEvidence, CodeEvidenceReason, EvidenceConfidence,
    SourceRole, SymbolKind,
};
pub use code_host_fetch::{resolve_code_host_fetch_target, CodeHostFetchTarget};
pub use code_metadata::{CodeHost, CodeMetadata};
pub use config::{AppConfig, LiveConfig, Mode, SearchSection};
pub use conflict::{
    detect_benchmark_conflicts, detect_date_conflicts, detect_mutable_vs_pinned,
    detect_provider_metadata_conflicts, detect_version_range_conflicts, ConflictClass,
    ConflictDetector, ConflictResolution, ConflictSeverity, EvidenceConflict,
};
pub use document::{
    BlockKind, DocumentChunk, DocumentKind, DocumentOutlineEntry, FetchDocument,
    FetchRenderMetadata, RenderFormat, RenderedBlock,
};
pub use error::{CoreError, CoreResult};
pub use error_query::{
    ErrorCode, ErrorQueryParts, ErrorSearchContext, ErrorSubquery, ExactErrorConfig, StackFrameHint,
};
pub use evidence_bundle::{
    compute_bundle_id, EvidenceBundle, EvidenceBundleFetchedItem, EvidenceBundleLimits,
    EvidenceBundleLink, EvidenceBundleLinkReason, EvidenceBundleRequest, EvidenceBundleSource,
    EvidenceFetchInput, EvidenceGap, EvidenceGapKind, EvidenceProviderCount,
    EvidenceProviderSummary, EvidenceSourceInput, EvidenceTrustSummary, DEFAULT_MAX_FETCHED_ITEMS,
    DEFAULT_MAX_SOURCES, DEFAULT_MAX_TOTAL_CHARS,
};
pub use evidence_postprocess::{
    assign_evidence_role, compute_evidence_role_summary, detect_structured_conflicts,
    materialize_evidence_roles, postprocess, resolve_workflow_model, EvidencePostprocessResult,
    EvidenceRoleSummary, RoleCount,
};
pub use evidence_role::EvidenceRole;
pub use fetch::{
    ExtractMode, ExtractedLink, FetchTransform, FetchTransformKind, FetchTrust, WebFetchRequest,
    WebFetchResponse,
};
pub use identity::{
    batch_fetch_id, canonicalize_url, chunk_id, code_span_id, compute_batch_fetch_id,
    compute_chunk_id, compute_code_span_id, compute_doc_id, compute_fetch_id, compute_locator_id,
    compute_source_id, compute_suggested_fetch_id, doc_id, fetch_id, locator_id, source_id,
    suggested_fetch_id, BatchFetchKey, CodeSpanKey, DocChunkKey, DocKey, FetchKey, RepoLocatorKey,
    SourceKey, SuggestedFetchKey,
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
    gitlab_raw_url, CodeSpanEvidence, RepoFetchRequest, RepoFetchResponse, RepoFetchedLine,
    RepoLocator,
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
    EvidenceQuality, ResearchClaim, ResearchClaimType, ResearchConflict, ResearchCoverage,
    ResearchDepth, ResearchDimension, ResearchDomain, ResearchEvidenceGap, ResearchEvidenceGapKind,
    ResearchGap, ResearchGapKind, ResearchQualitySignal, ResearchResultGroup,
    ResearchResultGroupKind, ResearchSearchRequest, ResearchSearchResponse, ResearchSourceClass,
    ResearchSourceQuality, ResearchSourceType, ResearchSubquery, ResearchSuggestedFetch,
    ResearchTelemetry, ResearchWorkflow, ResearchWorkflowContext,
};
pub use result::{SearchWarning, TrustLevel};
pub use retrieval_status::{
    absent_roles, attempt_outcome_to_dimension_state, attempts_to_failures, classify_absence,
    failed_providers, has_indeterminate, is_absence_only, is_failure_only, summarize_retrieval,
    summarize_retrieval_with_attempts, validate_attempt_ledger, AttemptLedgerViolation,
    AttemptSummaryCounts, EvidenceAbsenceKind, ResponseRetrievalSummary, RetrievalAttempt,
    RetrievalAttemptOutcome, RetrievalDimensionState, RetrievalDimensionStatus,
    RetrievalOperationIdentity, TruncationEvidence,
};
pub use sanitize::{
    bound_text, frame, scan_injection_markers, strip_control_chars, MarkerHit, TrustMarkers,
    SNIPPET_MAX_CHARS, TITLE_MAX_CHARS,
};
pub use security::{
    assess_source_quality, build_identifier_list, classify_query_kind, classify_source_tier,
    AffectedPackageSummary, CompactSecurityContext, DefensiveGuidance, DefensiveGuidanceCategory,
    KevMetadata, RemediationCategory, SecurityContext, SecurityEvidenceSummary, SecurityIdentifier,
    SecurityIdentifierKind, SecurityIdentifiers, SecurityQueryKind, SecurityRankReason,
    SecurityRemediation, SecurityResultGroup, SecurityResultGroupKind, SecuritySearchRequest,
    SecuritySearchResponse, SecuritySourceClass, SecuritySourceQuality, SecuritySourceTier,
    SecuritySuggestedFetch, SeverityLevel, TextSafetyCategory, TextSafetyWarning,
    VulnerabilityMetadata, VulnerabilityReference, VulnerabilitySource, VulnerabilitySummary,
};
pub use security_applicability::{
    AdvisoryRange, ApplicabilityAssessment, ApplicabilityConfidence, ApplicabilityStatus,
    DependencyFinding, DependencyRelation, DependencySource,
};
pub use source_card::{RankReason, SourceCard, SourceKind, SourceMetadata};
pub use warning::{
    convert_warnings, AgentWarning, WarningAccumulator, WarningCode, WarningSeverity,
};
pub use workflow::{
    AgentNextAction, AgentWorkflowFallback, AgentWorkflowRecipe, AgentWorkflowStep, RecipeDetail,
    RecipeSupport, MAX_NEXT_ACTIONS,
};
pub use workflow_coverage::{
    CoverageStatus, ResolutionSource, RetrievalFailure, RetrievalFailureKind,
    WorkflowCoverageModel, WorkflowCoverageRequest, WorkflowCoverageResult, WorkflowKind,
    WorkflowResolutionContext,
};
