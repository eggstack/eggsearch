//! Warning prefix tables that map adapter message prefixes to codes.

use super::WarningCode;

/// Known prefix patterns emitted by the adapter. The prefix is the text
/// before the first `: ` separator in the message.
pub(super) const KNOWN_PREFIXES: &[(&str, WarningCode)] = &[
    ("safe_search_unenforced", WarningCode::SafeSearchUnenforced),
    ("freshness_unenforced", WarningCode::FreshnessUnenforced),
    ("date_range_unenforced", WarningCode::DateRangeUnenforced),
    ("language_unenforced", WarningCode::LanguageUnenforced),
    ("region_unenforced", WarningCode::RegionUnenforced),
    ("domain_filters_local", WarningCode::DomainFiltersLocal),
    (
        "native_code_search_unavailable",
        WarningCode::NativeCodeSearchUnavailable,
    ),
    (
        "native_issue_search_unavailable",
        WarningCode::NativeIssueSearchUnavailable,
    ),
    (
        "native_release_search_unavailable",
        WarningCode::NativeReleaseSearchUnavailable,
    ),
    (
        "native_advisory_search_unavailable",
        WarningCode::NativeAdvisorySearchUnavailable,
    ),
    (
        "intent_routing_capability_excluded",
        WarningCode::IntentRoutingExcluded,
    ),
    (
        "symbol_hint_no_native_provider",
        WarningCode::SymbolHintNoNativeProvider,
    ),
    (
        "repo_hints_not_enforced_natively",
        WarningCode::RepoHintsNotEnforcedNatively,
    ),
    (
        "issue_search_no_native_provider",
        WarningCode::IssueSearchNoNativeProvider,
    ),
    (
        "release_search_no_native_provider",
        WarningCode::ReleaseSearchNoNativeProvider,
    ),
    (
        "coding_profile_degraded",
        WarningCode::CodingProfileDegraded,
    ),
    ("profile_degraded", WarningCode::ProfileDegraded),
    ("profile_partial", WarningCode::ProfilePartial),
    (
        "profile_provider_not_built",
        WarningCode::ProfileProviderNotBuilt,
    ),
    (
        "profile_provider_unknown",
        WarningCode::ProfileProviderUnknown,
    ),
    (
        "profile_provider_unavailable",
        WarningCode::ProfileProviderUnavailable,
    ),
    ("provider_cooldown", WarningCode::ProviderCooldown),
    (
        "generic_context_untrusted",
        WarningCode::GenericContextUntrusted,
    ),
    ("local_repo_match", WarningCode::LocalRepoMatch),
    ("local_repo_dirty", WarningCode::LocalRepoDirty),
    (
        "local_repo_state_unknown",
        WarningCode::LocalRepoStateUnknown,
    ),
    ("local_search_timeout", WarningCode::LocalSearchTimeout),
    ("local_search_truncated", WarningCode::LocalSearchTruncated),
    (
        "request_deadline_exceeded",
        WarningCode::RequestDeadlineExceeded,
    ),
    ("kev_match", WarningCode::KevMatch),
    ("kev_absent_not_proof", WarningCode::KevAbsentNotProof),
    ("kev_lookup_failed", WarningCode::KevLookupFailed),
    ("kev_lookup_skipped", WarningCode::KevLookupSkipped),
    ("severity_unavailable", WarningCode::SeverityUnavailable),
    (
        "version_match_unavailable",
        WarningCode::VersionMatchUnavailable,
    ),
    ("version_mismatch", WarningCode::VersionMismatch),
    (
        "dependency_file_read_error",
        WarningCode::DependencyFileReadError,
    ),
    (
        "dependency_parse_malformed",
        WarningCode::DependencyParseMalformed,
    ),
    (
        "dependency_format_unsupported",
        WarningCode::DependencyFormatUnsupported,
    ),
    (
        "dependency_parse_partial",
        WarningCode::DependencyParsePartial,
    ),
    (
        "dependency_file_budget_exceeded",
        WarningCode::DependencyFileBudgetExceeded,
    ),
    (
        "dependency_finding_budget_exceeded",
        WarningCode::DependencyFindingBudgetExceeded,
    ),
    (
        "weak_evidence_ignored_for_exact_applicability",
        WarningCode::WeakEvidenceIgnoredForExactApplicability,
    ),
    (
        "dependency_provenance_unverified_for_advisory",
        WarningCode::DependencyProvenanceUnverifiedForAdvisory,
    ),
    (
        "request_ecosystem_unassessed",
        WarningCode::RequestEcosystemUnassessed,
    ),
    (
        "applicability_not_exploitability",
        WarningCode::ApplicabilityNotExploitability,
    ),
    (
        "package_security_no_advisories",
        WarningCode::PackageSecurityNoAdvisories,
    ),
    (
        "package_security_lookup_failed",
        WarningCode::PackageSecurityLookupFailed,
    ),
    (
        "package_security_skipped",
        WarningCode::PackageSecuritySkipped,
    ),
    (
        "package_resolution_fallback",
        WarningCode::PackageResolutionFallback,
    ),
    ("no_native_tree_provider", WarningCode::NoNativeTreeProvider),
    ("repo_ref_not_found", WarningCode::RepoRefNotFound),
    ("forge_rate_limited", WarningCode::ForgeRateLimited),
    ("forge_auth_required", WarningCode::ForgeAuthRequired),
    ("forge_tree_truncated", WarningCode::ForgeTreeTruncated),
    (
        "forge_tree_unsupported_host",
        WarningCode::ForgeTreeUnsupportedHost,
    ),
    (
        "provider_resolution_failed",
        WarningCode::ProviderResolutionFailed,
    ),
    (
        "default_provider_resolution_failed",
        WarningCode::DefaultProviderResolutionFailed,
    ),
    ("subquery_cap_applied", WarningCode::SubqueryCapApplied),
];

/// Known fetch warning prefix patterns. Matches the text emitted by
/// `FetchClient` and the MCP tool handlers. The prefix is the text
/// before the first `: ` separator.
pub(super) const FETCH_WARNING_PREFIXES: &[(&str, WarningCode)] = &[
    (
        "fetch_content_truncated",
        WarningCode::FetchContentTruncated,
    ),
    ("fetch_links_truncated", WarningCode::FetchLinksTruncated),
    (
        "batch_item_count_truncated",
        WarningCode::FetchContentTruncated,
    ),
    (
        "batch_total_budget_exhausted",
        WarningCode::FetchContentTruncated,
    ),
    (
        "local_content_marker_warning",
        WarningCode::PromptInjectionMarkerDetected,
    ),
    (
        "workspace_fetch_truncated_by_max_chars",
        WarningCode::FetchContentTruncated,
    ),
];
