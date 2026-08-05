//! Phase 13: Schema contract, golden identity, and warning-code registry tests.
//!
//! This file groups three workstreams into a single contract test suite:
//! - **Workstream 1**: MCP schema contract tests (arg deserialization, response shape, enum stability).
//! - **Workstream 2**: Golden identity tests (deterministic ID fixtures, URL canonicalization).
//! - **Workstream 3**: Warning-code and reason-code registry tests (snake_case stability, no duplicates).

use std::collections::HashSet;

use eggsearch::core::batch_fetch::BatchFetchItem;
use eggsearch::core::evidence_bundle::{compute_bundle_id, EvidenceGapKind};
use eggsearch::core::identity::{
    batch_fetch_id, canonicalize_url, chunk_id, code_span_id, doc_id, fetch_id, locator_id,
    source_id, suggested_fetch_id,
};
use eggsearch::core::query::{Freshness, SearchIntent};
use eggsearch::core::repo_fetch::{RepoLocator, RepoLocatorKind};
use eggsearch::core::repo_search::{
    RepoResultGroupKind, RepoSearchMode, RepoSearchRequest, SearchProfile,
};
use eggsearch::core::research::{
    ResearchClaimType, ResearchDomain, ResearchSearchRequest, ResearchSourceClass, ResearchWorkflow,
};
use eggsearch::core::security::{
    RemediationCategory, SecurityResultGroupKind, SecuritySearchRequest, SeverityLevel,
};
use eggsearch::core::source_card::{RankReason, SourceKind};
use eggsearch::core::warning::{WarningCode, WarningSeverity};
use eggsearch::core::workflow::{RecipeDetail, RecipeSupport};
use eggsearch::core::WebSearchRequest;
use eggsearch::mcp::tools::{
    BatchFetchArgs, EvidenceBundleArgs, ProviderStatusArgs, RepoFetchArgs, RepoMapArgs,
    RepoSearchArgs, ResearchSearchArgs, SecuritySearchArgs, WebFetchArgs, WebSearchArgs,
};
use eggsearch::meta::fetch_ranking::FetchRankReason;

// ===========================================================================
// Workstream 1: MCP Schema Contract Tests
// ===========================================================================

#[test]
fn web_search_args_deserialize_from_valid_json() {
    let json = r#"{"query": "test query"}"#;
    let args: WebSearchArgs = serde_json::from_str(json).unwrap();
    assert_eq!(args.query, "test query");
}

#[test]
fn web_search_args_deserialize_with_all_fields() {
    let json = r#"{
        "query": "axum middleware",
        "max_results": 10,
        "providers": ["duckduckgo"],
        "safe_search": "moderate",
        "timeout_ms": 5000,
        "intent": "code",
        "freshness": "week"
    }"#;
    let args: WebSearchArgs = serde_json::from_str(json).unwrap();
    assert_eq!(args.query, "axum middleware");
    assert_eq!(args.max_results, Some(10));
    assert_eq!(args.providers, vec!["duckduckgo"]);
    assert!(args.timeout_ms.is_some());
}

#[test]
fn provider_status_args_deserialize_from_valid_json() {
    let json = r#"{"probe": false}"#;
    let args: ProviderStatusArgs = serde_json::from_str(json).unwrap();
    assert!(!args.probe);
}

#[test]
fn repo_search_args_deserialize_from_valid_json() {
    let json = r#"{"query": "tokio-rs/axum"}"#;
    let args: RepoSearchArgs = serde_json::from_str(json).unwrap();
    assert_eq!(args.query, "tokio-rs/axum");
}

#[test]
fn repo_search_args_deserialize_with_all_fields() {
    let json = r#"{
        "query": "Router::layer",
        "host": "github",
        "owner": "tokio-rs",
        "repo": "axum",
        "language": "rust",
        "symbol": "Router::layer",
        "include_docs": true,
        "profile": "coding"
    }"#;
    let args: RepoSearchArgs = serde_json::from_str(json).unwrap();
    assert_eq!(args.owner.as_deref(), Some("tokio-rs"));
    assert_eq!(args.repo.as_deref(), Some("axum"));
    assert_eq!(args.symbol.as_deref(), Some("Router::layer"));
    assert_eq!(args.profile.as_deref(), Some("coding"));
}

#[test]
fn security_search_args_deserialize_from_valid_json() {
    let json = r#"{"query": "CVE-2024-0001"}"#;
    let args: SecuritySearchArgs = serde_json::from_str(json).unwrap();
    assert_eq!(args.query.as_deref(), Some("CVE-2024-0001"));
}

#[test]
fn research_search_args_deserialize_from_valid_json() {
    let json = r#"{"query": "axum vs actix-web comparison"}"#;
    let args: ResearchSearchArgs = serde_json::from_str(json).unwrap();
    assert_eq!(args.query, "axum vs actix-web comparison");
}

#[test]
fn web_fetch_args_deserialize_from_valid_json() {
    let json = r#"{"url": "https://example.com"}"#;
    let args: WebFetchArgs = serde_json::from_str(json).unwrap();
    assert_eq!(args.url, "https://example.com");
}

#[test]
fn batch_fetch_args_deserialize_from_valid_json() {
    let json = r#"{"items": [{"type": "web", "url": "https://example.com"}]}"#;
    let args: BatchFetchArgs = serde_json::from_str(json).unwrap();
    assert!(!args.items.is_empty());
}

#[test]
fn batch_fetch_repo_host_description_lists_all_aliases() {
    let schema = schemars::schema_for!(BatchFetchItem);
    let json = serde_json::to_value(&schema).unwrap();
    let repo_schema = json
        .get("oneOf")
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            arr.iter().find(|entry| {
                entry
                    .get("properties")
                    .and_then(|p| p.get("host"))
                    .is_some()
                    && entry
                        .get("properties")
                        .and_then(|p| p.get("owner"))
                        .is_some()
                    && entry
                        .get("properties")
                        .and_then(|p| p.get("path"))
                        .is_some()
            })
        })
        .expect("Repo variant schema must include host/owner/path properties");
    let host_desc = repo_schema["properties"]["host"]["description"]
        .as_str()
        .expect("host description must be a string");
    let aliases = eggsearch::core::code_metadata::CodeHost::accepted_aliases();
    for token in [
        "github",
        "gitlab",
        "codeberg",
        "gitea",
        "forgejo",
        "workspace",
    ] {
        assert!(
            host_desc.contains(token),
            "BatchFetchItem::Repo host description must mention `{token}` (CodeHost aliases: {aliases}); got: {host_desc}"
        );
    }
}

#[test]
fn repo_fetch_args_deserialize_from_valid_json() {
    let json = r#"{"owner": "tokio-rs", "repo": "axum", "path": "src/lib.rs"}"#;
    let args: RepoFetchArgs = serde_json::from_str(json).unwrap();
    assert_eq!(args.owner, "tokio-rs");
    assert_eq!(args.repo, "axum");
    assert_eq!(args.path, "src/lib.rs");
}

#[test]
fn repo_map_args_deserialize_from_valid_json() {
    let json = r#"{"owner": "tokio-rs", "repo": "axum"}"#;
    let args: RepoMapArgs = serde_json::from_str(json).unwrap();
    assert_eq!(args.owner, "tokio-rs");
    assert_eq!(args.repo, "axum");
}

#[test]
fn evidence_bundle_args_deserialize_from_valid_json() {
    let json = r#"{"goal": "test"}"#;
    let args: EvidenceBundleArgs = serde_json::from_str(json).unwrap();
    assert_eq!(args.goal.as_deref(), Some("test"));
    assert!(args.sources.is_empty());
    assert!(args.fetches.is_empty());
}

// --- Response serialization shape tests ---

#[test]
fn web_search_request_serializes_top_level_fields() {
    let req = WebSearchRequest::new("test query");
    let json = serde_json::to_value(&req).unwrap();
    assert!(json.get("query").is_some());
    assert!(json.get("max_results").is_some());
    assert!(json.get("providers").is_some());
}

#[test]
fn repo_search_request_serializes_top_level_fields() {
    let req = RepoSearchRequest {
        query: "test".to_string(),
        mode: Some(RepoSearchMode::ExactError),
        ..Default::default()
    };
    let json = serde_json::to_value(&req).unwrap();
    assert!(json.get("query").is_some());
    assert!(json.get("mode").is_some());
    assert!(json.get("freshness").is_some());
}

#[test]
fn security_search_request_serializes_top_level_fields() {
    let req = SecuritySearchRequest {
        query: "CVE-2024-0001".to_string(),
        ..Default::default()
    };
    let json = serde_json::to_value(&req).unwrap();
    assert!(json.get("query").is_some());
    assert!(json.get("freshness").is_some());
}

#[test]
fn research_search_request_serializes_top_level_fields() {
    let req = ResearchSearchRequest {
        query: "distributed consensus".to_string(),
        ..Default::default()
    };
    let json = serde_json::to_value(&req).unwrap();
    assert!(json.get("query").is_some());
    assert!(json.get("freshness").is_some());
}

#[test]
fn evidence_bundle_request_serializes_top_level_fields() {
    use eggsearch::core::evidence_bundle::EvidenceBundleRequest;
    use eggsearch::core::evidence_bundle::EvidenceSourceInput;

    let req = EvidenceBundleRequest {
        goal: Some("test goal".to_string()),
        sources: vec![EvidenceSourceInput {
            id: None,
            url: Some("https://example.com".to_string()),
            title: None,
            snippet: None,
            providers: vec![],
            score: None,
            trust: None,
            trust_markers: None,
            metadata: None,
            quality: None,
        }],
        fetches: vec![],
        include_unfetched_sources: None,
        max_sources: None,
        max_fetched_items: None,
        max_total_chars: None,
        warnings: vec![],
        research_claims: None,
        research_conflicts: None,
    };
    let json = serde_json::to_value(&req).unwrap();
    assert!(json.get("goal").is_some());
    assert!(json.get("sources").is_some());
    // fetches is skipped when empty; goal and sources are present
    assert!(json.get("goal").unwrap().is_string());
    assert!(json.get("sources").unwrap().is_array());
}

// --- Key enum serialized name stability ---

fn assert_serde_enum<T: serde::Serialize + for<'de> serde::Deserialize<'de> + std::fmt::Debug>(
    value: &T,
    expected_json: &str,
) {
    let json = serde_json::to_string(value).unwrap();
    assert_eq!(json, expected_json, "serialize mismatch for {value:?}");
    let parsed: T = serde_json::from_str(&json).unwrap();
    assert_eq!(
        format!("{parsed:?}"),
        format!("{value:?}"),
        "roundtrip mismatch"
    );
}

#[test]
fn source_kind_serialized_names_stability() {
    assert_serde_enum(&SourceKind::Unknown, "\"unknown\"");
    assert_serde_enum(&SourceKind::OfficialDocs, "\"official_docs\"");
    assert_serde_enum(&SourceKind::PackageRegistry, "\"package_registry\"");
    assert_serde_enum(&SourceKind::SourceRepository, "\"source_repository\"");
    assert_serde_enum(&SourceKind::RepositoryRoot, "\"repository_root\"");
    assert_serde_enum(&SourceKind::SourceDirectory, "\"source_directory\"");
    assert_serde_enum(&SourceKind::SourceFile, "\"source_file\"");
    assert_serde_enum(&SourceKind::IssueThread, "\"issue_thread\"");
    assert_serde_enum(&SourceKind::PullRequest, "\"pull_request\"");
    assert_serde_enum(&SourceKind::ReleaseNotes, "\"release_notes\"");
    assert_serde_enum(&SourceKind::Tag, "\"tag\"");
    assert_serde_enum(&SourceKind::Commit, "\"commit\"");
    assert_serde_enum(&SourceKind::SecurityAdvisory, "\"security_advisory\"");
    assert_serde_enum(&SourceKind::Reference, "\"reference\"");
    assert_serde_enum(&SourceKind::News, "\"news\"");
    assert_serde_enum(&SourceKind::Tutorial, "\"tutorial\"");
    assert_serde_enum(&SourceKind::Forum, "\"forum\"");
}

#[test]
fn search_profile_serialized_names_stability() {
    assert_serde_enum(&SearchProfile::Generic, "\"generic\"");
    assert_serde_enum(&SearchProfile::Coding, "\"coding\"");
    assert_serde_enum(&SearchProfile::Security, "\"security\"");
    assert_serde_enum(&SearchProfile::Research, "\"research\"");
}

#[test]
fn repo_search_mode_serialized_names_stability() {
    assert_serde_enum(&RepoSearchMode::Normal, "\"normal\"");
    assert_serde_enum(&RepoSearchMode::ExactError, "\"exact_error\"");
}

#[test]
fn search_intent_serialized_names_stability() {
    assert_serde_enum(&SearchIntent::Web, "\"web\"");
    assert_serde_enum(&SearchIntent::Docs, "\"docs\"");
    assert_serde_enum(&SearchIntent::Code, "\"code\"");
    assert_serde_enum(&SearchIntent::Issues, "\"issues\"");
    assert_serde_enum(&SearchIntent::Releases, "\"releases\"");
    assert_serde_enum(&SearchIntent::Security, "\"security\"");
    assert_serde_enum(&SearchIntent::News, "\"news\"");
}

#[test]
fn freshness_serialized_names_stability() {
    assert_serde_enum(&Freshness::Any, "\"any\"");
    assert_serde_enum(&Freshness::Day, "\"day\"");
    assert_serde_enum(&Freshness::Week, "\"week\"");
    assert_serde_enum(&Freshness::Month, "\"month\"");
    assert_serde_enum(&Freshness::Year, "\"year\"");
}

#[test]
fn severity_level_serialized_names_stability() {
    assert_serde_enum(&SeverityLevel::Critical, "\"critical\"");
    assert_serde_enum(&SeverityLevel::High, "\"high\"");
    assert_serde_enum(&SeverityLevel::Medium, "\"medium\"");
    assert_serde_enum(&SeverityLevel::Low, "\"low\"");
    assert_serde_enum(&SeverityLevel::Unknown, "\"unknown\"");
}

#[test]
fn security_result_group_kind_serialized_names_stability() {
    assert_serde_enum(
        &SecurityResultGroupKind::AuthoritativeAdvisories,
        "\"authoritative_advisories\"",
    );
    assert_serde_enum(
        &SecurityResultGroupKind::VendorAdvisories,
        "\"vendor_advisories\"",
    );
    assert_serde_enum(
        &SecurityResultGroupKind::PackageAdvisories,
        "\"package_advisories\"",
    );
    assert_serde_enum(&SecurityResultGroupKind::KevEntries, "\"kev_entries\"");
    assert_serde_enum(
        &SecurityResultGroupKind::PatchCommitsOrReleases,
        "\"patch_commits_or_releases\"",
    );
    assert_serde_enum(
        &SecurityResultGroupKind::ExploitDiscussion,
        "\"exploit_discussion\"",
    );
    assert_serde_enum(
        &SecurityResultGroupKind::DefensiveGuidance,
        "\"defensive_guidance\"",
    );
    assert_serde_enum(
        &SecurityResultGroupKind::GeneralContext,
        "\"general_context\"",
    );
    assert_serde_enum(&SecurityResultGroupKind::Other, "\"other\"");
}

#[test]
fn research_domain_serialized_names_stability() {
    assert_serde_enum(&ResearchDomain::General, "\"general\"");
    assert_serde_enum(
        &ResearchDomain::SoftwareArchitecture,
        "\"software_architecture\"",
    );
    assert_serde_enum(&ResearchDomain::ApiDesign, "\"api_design\"");
    assert_serde_enum(
        &ResearchDomain::DistributedSystems,
        "\"distributed_systems\"",
    );
    assert_serde_enum(&ResearchDomain::Security, "\"security\"");
    assert_serde_enum(&ResearchDomain::Performance, "\"performance\"");
    assert_serde_enum(&ResearchDomain::LanguageEcosystem, "\"language_ecosystem\"");
    assert_serde_enum(&ResearchDomain::MachineLearning, "\"machine_learning\"");
    assert_serde_enum(&ResearchDomain::Infrastructure, "\"infrastructure\"");
}

#[test]
fn research_workflow_serialized_names_stability() {
    assert_serde_enum(&ResearchWorkflow::General, "\"general\"");
    assert_serde_enum(&ResearchWorkflow::ApiEvaluation, "\"api_evaluation\"");
    assert_serde_enum(
        &ResearchWorkflow::LibraryComparison,
        "\"library_comparison\"",
    );
    assert_serde_enum(
        &ResearchWorkflow::MigrationPlanning,
        "\"migration_planning\"",
    );
    assert_serde_enum(&ResearchWorkflow::SecurityReview, "\"security_review\"");
    assert_serde_enum(
        &ResearchWorkflow::PerformanceInvestigation,
        "\"performance_investigation\"",
    );
    assert_serde_enum(&ResearchWorkflow::EcosystemSurvey, "\"ecosystem_survey\"");
    assert_serde_enum(
        &ResearchWorkflow::ArchitectureDecision,
        "\"architecture_decision\"",
    );
}

#[test]
fn evidence_gap_kind_serialized_names_stability() {
    assert_serde_enum(
        &EvidenceGapKind::NoPrimarySourceFound,
        "\"no_primary_source_found\"",
    );
    assert_serde_enum(&EvidenceGapKind::ProviderDegraded, "\"provider_degraded\"");
    assert_serde_enum(
        &EvidenceGapKind::NativeRepoFilterNotEnforced,
        "\"native_repo_filter_not_enforced\"",
    );
    assert_serde_enum(
        &EvidenceGapKind::SecurityApplicabilityUnknown,
        "\"security_applicability_unknown\"",
    );
    assert_serde_enum(&EvidenceGapKind::FetchFailed, "\"fetch_failed\"");
    assert_serde_enum(&EvidenceGapKind::SourceUnfetched, "\"source_unfetched\"");
    assert_serde_enum(
        &EvidenceGapKind::AllResultsExternalUntrusted,
        "\"all_results_external_untrusted\"",
    );
    assert_serde_enum(
        &EvidenceGapKind::LocalCheckoutDirty,
        "\"local_checkout_dirty\"",
    );
    assert_serde_enum(
        &EvidenceGapKind::LocalRemoteMismatch,
        "\"local_remote_mismatch\"",
    );
    assert_serde_enum(
        &EvidenceGapKind::LocalGeneratedOrVendorOnly,
        "\"local_generated_or_vendor_only\"",
    );
    assert_serde_enum(
        &EvidenceGapKind::LocalUntrackedFile,
        "\"local_untracked_file\"",
    );
    assert_serde_enum(
        &EvidenceGapKind::LocalSourceUnfetched,
        "\"local_source_unfetched\"",
    );
    assert_serde_enum(
        &EvidenceGapKind::NativeAdvisoryUnavailable,
        "\"native_advisory_unavailable\"",
    );
    assert_serde_enum(
        &EvidenceGapKind::SymbolHintNoNativeProvider,
        "\"symbol_hint_no_native_provider\"",
    );
    assert_serde_enum(
        &EvidenceGapKind::IssueSearchNoNativeProvider,
        "\"issue_search_no_native_provider\"",
    );
    assert_serde_enum(
        &EvidenceGapKind::ReleaseSearchNoNativeProvider,
        "\"release_search_no_native_provider\"",
    );
    assert_serde_enum(
        &EvidenceGapKind::FreshnessNotEnforced,
        "\"freshness_not_enforced\"",
    );
    assert_serde_enum(
        &EvidenceGapKind::PackageResolutionFailed,
        "\"package_resolution_failed\"",
    );
    assert_serde_enum(
        &EvidenceGapKind::NoFixedVersionFound,
        "\"no_fixed_version_found\"",
    );
    assert_serde_enum(
        &EvidenceGapKind::NoCounterpointFound,
        "\"no_counterpoint_found\"",
    );
    assert_serde_enum(
        &EvidenceGapKind::NoBenchmarksFound,
        "\"no_benchmarks_found\"",
    );
    assert_serde_enum(&EvidenceGapKind::MissingTests, "\"missing_tests\"");
    assert_serde_enum(&EvidenceGapKind::MissingExamples, "\"missing_examples\"");
    assert_serde_enum(&EvidenceGapKind::MissingManifest, "\"missing_manifest\"");
    assert_serde_enum(&EvidenceGapKind::MissingChangelog, "\"missing_changelog\"");
    assert_serde_enum(
        &EvidenceGapKind::MissingSecurityPolicy,
        "\"missing_security_policy\"",
    );
}

#[test]
fn recipe_detail_serialized_names_stability() {
    assert_serde_enum(&RecipeDetail::None, "\"none\"");
    assert_serde_enum(&RecipeDetail::Summary, "\"summary\"");
    assert_serde_enum(&RecipeDetail::Full, "\"full\"");
}

#[test]
fn recipe_support_serialized_names_stability() {
    assert_serde_enum(&RecipeSupport::Available, "\"available\"");
    assert_serde_enum(&RecipeSupport::Partial, "\"partial\"");
    assert_serde_enum(&RecipeSupport::Unavailable, "\"unavailable\"");
}

#[test]
fn rank_reason_serialized_names_stability() {
    assert_serde_enum(&RankReason::RrfMultiProvider, "\"rrf_multi_provider\"");
    assert_serde_enum(&RankReason::RrfProviderRank, "\"rrf_provider_rank\"");
    assert_serde_enum(&RankReason::DomainPriorDocs, "\"domain_prior_docs\"");
    assert_serde_enum(&RankReason::DomainPriorCode, "\"domain_prior_code\"");
    assert_serde_enum(
        &RankReason::DomainPriorSecurity,
        "\"domain_prior_security\"",
    );
    assert_serde_enum(&RankReason::DomainPriorRelease, "\"domain_prior_release\"");
    assert_serde_enum(&RankReason::IntentMatch, "\"intent_match\"");
    assert_serde_enum(&RankReason::FreshnessMatch, "\"freshness_match\"");
    assert_serde_enum(&RankReason::ExactTitleMatch, "\"exact_title_match\"");
    assert_serde_enum(&RankReason::CanonicalDedup, "\"canonical_dedup\"");
    assert_serde_enum(
        &RankReason::ProviderNativeIssueSearch,
        "\"provider_native_issue_search\"",
    );
    assert_serde_enum(
        &RankReason::ProviderNativeReleaseSearch,
        "\"provider_native_release_search\"",
    );
    assert_serde_enum(
        &RankReason::ProviderNativeAdvisorySearch,
        "\"provider_native_advisory_search\"",
    );
    assert_serde_enum(&RankReason::RepoOwnerMatch, "\"repo_owner_match\"");
    assert_serde_enum(&RankReason::HintMatch, "\"hint_match\"");
    assert_serde_enum(
        &RankReason::AdvisoryIdentifierMatch,
        "\"advisory_identifier_match\"",
    );
    assert_serde_enum(&RankReason::KevMatch, "\"kev_match\"");
    assert_serde_enum(&RankReason::VendorAdvisory, "\"vendor_advisory\"");
    assert_serde_enum(&RankReason::PackageAdvisory, "\"package_advisory\"");
    assert_serde_enum(&RankReason::DefensiveGuidance, "\"defensive_guidance\"");
    assert_serde_enum(
        &RankReason::SecurityPrimarySource,
        "\"security_primary_source\"",
    );
    assert_serde_enum(
        &RankReason::SecurityMaintainerSource,
        "\"security_maintainer_source\"",
    );
    assert_serde_enum(
        &RankReason::VersionAffectedMatch,
        "\"version_affected_match\"",
    );
    assert_serde_enum(
        &RankReason::ExactErrorPhraseMatch,
        "\"exact_error_phrase_match\"",
    );
    assert_serde_enum(&RankReason::ErrorCodeMatch, "\"error_code_match\"");
    assert_serde_enum(&RankReason::ToolchainMatch, "\"toolchain_match\"");
    assert_serde_enum(&RankReason::OfficialErrorDocs, "\"official_error_docs\"");
    assert_serde_enum(
        &RankReason::MaintainerIssueMatch,
        "\"maintainer_issue_match\"",
    );
    assert_serde_enum(
        &RankReason::RegressionReleaseMatch,
        "\"regression_release_match\"",
    );
}

// --- Provider status response shape ---

#[test]
fn provider_status_args_roundtrip() {
    let args = ProviderStatusArgs {
        probe: false,
        recipe_detail: None,
    };
    let json = serde_json::to_value(&args).unwrap();
    assert!(json.get("probe").is_some());
    let restored: ProviderStatusArgs = serde_json::from_value(json).unwrap();
    assert!(!restored.probe);
    assert!(restored.recipe_detail.is_none());
}

#[test]
fn provider_status_args_with_recipe_detail() {
    let json = r#"{"probe": true, "recipe_detail": "full"}"#;
    let args: ProviderStatusArgs = serde_json::from_str(json).unwrap();
    assert!(args.probe);
    assert_eq!(args.recipe_detail, Some(RecipeDetail::Full));
}

// --- No duplicate serialized enum names ---

fn collect_enum_serialized_names<T: serde::Serialize>(variants: &[T]) -> Vec<String> {
    variants
        .iter()
        .map(|v| serde_json::to_string(v).unwrap())
        .collect()
}

fn assert_no_duplicates(names: &[String], label: &str) {
    let set: HashSet<_> = names.iter().collect();
    assert_eq!(
        set.len(),
        names.len(),
        "duplicate serialized names found in {label}"
    );
}

#[test]
fn source_kind_no_duplicate_serialized_names() {
    let variants = [
        SourceKind::Unknown,
        SourceKind::OfficialDocs,
        SourceKind::PackageRegistry,
        SourceKind::SourceRepository,
        SourceKind::RepositoryRoot,
        SourceKind::SourceDirectory,
        SourceKind::SourceFile,
        SourceKind::IssueThread,
        SourceKind::PullRequest,
        SourceKind::ReleaseNotes,
        SourceKind::Tag,
        SourceKind::Commit,
        SourceKind::SecurityAdvisory,
        SourceKind::Reference,
        SourceKind::News,
        SourceKind::Tutorial,
        SourceKind::Forum,
    ];
    let names = collect_enum_serialized_names(&variants);
    assert_no_duplicates(&names, "SourceKind");
}

#[test]
fn warning_code_no_duplicate_serialized_names() {
    let all_codes: Vec<WarningCode> = vec![
        WarningCode::UntrustedExternalContent,
        WarningCode::UntrustedLocalWorkspaceContent,
        WarningCode::PromptInjectionMarkerDetected,
        WarningCode::SafeSearchUnenforced,
        WarningCode::FreshnessUnenforced,
        WarningCode::NativeCodeSearchUnavailable,
        WarningCode::NativeIssueSearchUnavailable,
        WarningCode::NativeReleaseSearchUnavailable,
        WarningCode::NativeAdvisorySearchUnavailable,
        WarningCode::SymbolHintNoNativeProvider,
        WarningCode::RepoHintsNotEnforcedNatively,
        WarningCode::IssueSearchNoNativeProvider,
        WarningCode::ReleaseSearchNoNativeProvider,
        WarningCode::UnknownProvider,
        WarningCode::DisabledProvider,
        WarningCode::MissingApiKey,
        WarningCode::ProviderFailed,
        WarningCode::ProviderTimeout,
        WarningCode::ProviderRateLimited,
        WarningCode::ProviderCooldown,
        WarningCode::ProfileDegraded,
        WarningCode::ProfilePartial,
        WarningCode::ProfileProviderNotBuilt,
        WarningCode::ProfileProviderUnknown,
        WarningCode::ProfileProviderUnavailable,
        WarningCode::CodingProfileDegraded,
        WarningCode::LocalRepoMatch,
        WarningCode::LocalRepoDirty,
        WarningCode::LocalRepoStateUnknown,
        WarningCode::LocalSearchTimeout,
        WarningCode::LocalSearchTruncated,
        WarningCode::FetchContentTruncated,
        WarningCode::FetchLinksTruncated,
        WarningCode::FetchWarning,
        WarningCode::UnknownWarning,
        WarningCode::RequestDeadlineExceeded,
        WarningCode::SubqueryCapApplied,
        WarningCode::KevMatch,
        WarningCode::KevAbsentNotProof,
        WarningCode::KevLookupFailed,
        WarningCode::KevLookupSkipped,
        WarningCode::SeverityUnavailable,
        WarningCode::VersionMatchUnavailable,
        WarningCode::VersionMismatch,
        WarningCode::DependencyFileReadError,
        WarningCode::ApplicabilityNotExploitability,
        WarningCode::PackageSecurityNoAdvisories,
        WarningCode::PackageSecurityLookupFailed,
        WarningCode::PackageSecuritySkipped,
        WarningCode::PackageResolution,
        WarningCode::PackageResolutionFallback,
        WarningCode::NoNativeTreeProvider,
        WarningCode::GenericContextUntrusted,
        WarningCode::ProviderResolutionFailed,
        WarningCode::DefaultProviderResolutionFailed,
        WarningCode::EmptyResultGroup,
        WarningCode::CardInjectionMarkerDetected,
        WarningCode::MaxResultsClamped,
    ];
    let names: Vec<String> = all_codes.iter().map(|c| c.as_str().to_string()).collect();
    assert_no_duplicates(&names, "WarningCode");
}

#[test]
fn search_profile_no_duplicate_serialized_names() {
    let variants = [
        SearchProfile::Generic,
        SearchProfile::Coding,
        SearchProfile::Security,
        SearchProfile::Research,
    ];
    let names = collect_enum_serialized_names(&variants);
    assert_no_duplicates(&names, "SearchProfile");
}

#[test]
fn evidence_gap_kind_no_duplicate_serialized_names() {
    let variants = [
        EvidenceGapKind::NoPrimarySourceFound,
        EvidenceGapKind::ProviderDegraded,
        EvidenceGapKind::NativeRepoFilterNotEnforced,
        EvidenceGapKind::SecurityApplicabilityUnknown,
        EvidenceGapKind::FetchFailed,
        EvidenceGapKind::SourceUnfetched,
        EvidenceGapKind::AllResultsExternalUntrusted,
        EvidenceGapKind::LocalCheckoutDirty,
        EvidenceGapKind::LocalRemoteMismatch,
        EvidenceGapKind::LocalGeneratedOrVendorOnly,
        EvidenceGapKind::LocalUntrackedFile,
        EvidenceGapKind::LocalSourceUnfetched,
        EvidenceGapKind::NativeAdvisoryUnavailable,
        EvidenceGapKind::SymbolHintNoNativeProvider,
        EvidenceGapKind::IssueSearchNoNativeProvider,
        EvidenceGapKind::ReleaseSearchNoNativeProvider,
        EvidenceGapKind::FreshnessNotEnforced,
        EvidenceGapKind::PackageResolutionFailed,
        EvidenceGapKind::NoFixedVersionFound,
        EvidenceGapKind::NoCounterpointFound,
        EvidenceGapKind::NoBenchmarksFound,
        EvidenceGapKind::MissingTests,
        EvidenceGapKind::MissingExamples,
        EvidenceGapKind::MissingManifest,
        EvidenceGapKind::MissingChangelog,
        EvidenceGapKind::MissingSecurityPolicy,
    ];
    let names = collect_enum_serialized_names(&variants);
    assert_no_duplicates(&names, "EvidenceGapKind");
}

// ===========================================================================
// Workstream 2: Golden Identity Tests
// ===========================================================================

#[test]
fn golden_source_id() {
    let id = source_id(
        Some("duckduckgo"),
        Some("https://example.com/page"),
        Some("Example Page"),
        Some(SourceKind::OfficialDocs),
    );
    assert_eq!(id, "src_b7c720d1013ccb55");
}

#[test]
fn golden_fetch_id() {
    let id = fetch_id(Some("https://example.com/file.rs"), None, None, None, None);
    assert_eq!(id, "fetch_351d8b4af32d6573");
}

#[test]
fn golden_suggested_fetch_id() {
    let id = suggested_fetch_id("https://example.com/path", "OfficialDocs", 1);
    assert_eq!(id, "suggested_ad40b2173a8c41f6");
}

#[test]
fn golden_batch_fetch_id() {
    let id = batch_fetch_id("https://example.com/path", 0);
    assert_eq!(id, "batch_d85a5a3267858a74");
}

#[test]
fn golden_locator_id() {
    let loc = RepoLocator {
        kind: RepoLocatorKind::Remote,
        host: Some(eggsearch::core::CodeHost::Github),
        owner: Some("a".to_string()),
        repo: Some("r".to_string()),
        ref_name: Some("main".to_string()),
        commit_sha: None,
        path: "src/lib.rs".to_string(),
        workspace_root: None,
    };
    let id = locator_id(&loc);
    assert_eq!(id, "loc_91cbe152399f0d98");
}

#[test]
fn golden_doc_id() {
    let id = doc_id(
        Some("https://example.com/page"),
        Some("Example"),
        Some("html"),
    );
    assert_eq!(id, "doc_378ae4bb554d051c");
}

#[test]
fn golden_chunk_id() {
    let id = chunk_id("doc_aabbccdd11223344", 0, "intro");
    assert_eq!(id, "chunk_c777b483a3765f9f");
}

#[test]
fn golden_code_span_id() {
    let id = code_span_id(
        "https://example.com/src.rs",
        Some(10),
        Some(20),
        Some("main"),
    );
    assert_eq!(id, "span_2b241f6240cde0ab");
}

#[test]
fn golden_bundle_id() {
    let sources = vec!["src_aaa".to_string(), "src_bbb".to_string()];
    let fetches = vec!["fetch_ccc".to_string()];
    let id = compute_bundle_id(Some("debug error"), &sources, &fetches);
    assert_eq!(id, "bundle_06e191277c02e672");
}

#[test]
fn cross_entity_namespace_uniqueness() {
    let common_url = "https://example.com/page";
    let src = source_id(Some("p"), Some(common_url), Some("title"), None);
    let fetch = fetch_id(Some(common_url), None, None, None, None);
    let suggested = suggested_fetch_id(common_url, "g", 1);
    let batch = batch_fetch_id(common_url, 0);
    let doc = doc_id(Some(common_url), Some("title"), None);
    let chunk = chunk_id(&doc, 0, "");
    let span = code_span_id(common_url, Some(1), Some(10), None);

    assert!(src.starts_with("src_"));
    assert!(fetch.starts_with("fetch_"));
    assert!(suggested.starts_with("suggested_"));
    assert!(batch.starts_with("batch_"));
    assert!(doc.starts_with("doc_"));
    assert!(chunk.starts_with("chunk_"));
    assert!(span.starts_with("span_"));

    let all = [&src, &fetch, &suggested, &batch, &doc, &chunk, &span];
    for i in 0..all.len() {
        for j in (i + 1)..all.len() {
            assert_ne!(
                all[i], all[j],
                "IDs should differ: {} vs {}",
                all[i], all[j]
            );
        }
    }
}

// --- URL canonicalization fixtures ---

#[test]
fn canonicalize_url_strips_www_default_port_fragment() {
    let result = canonicalize_url("https://www.example.com:443/path#frag");
    assert_eq!(result, "https://example.com/path");
}

#[test]
fn canonicalize_url_strips_http_default_port() {
    let result = canonicalize_url("http://example.com:80/path");
    assert_eq!(result, "http://example.com/path");
}

#[test]
fn canonicalize_url_strips_fragment_preserves_query() {
    let result = canonicalize_url("https://example.com/path?a=1&b=2#frag");
    assert_eq!(result, "https://example.com/path?a=1&b=2");
}

#[test]
fn canonicalize_url_preserves_query_params() {
    let result = canonicalize_url("https://example.com/path?a=1&b=2");
    assert_eq!(result, "https://example.com/path?a=1&b=2");
}

#[test]
fn canonicalize_url_lowercases_scheme() {
    let result = canonicalize_url("HTTP://example.com/Path");
    assert_eq!(result, "http://example.com/Path");
}

// --- Code span ID behavior ---

#[test]
fn code_span_id_changes_with_line_range() {
    let a = code_span_id("https://a.com/f.rs", Some(10), Some(20), Some("main"));
    let b = code_span_id("https://a.com/f.rs", Some(10), Some(25), Some("main"));
    assert_ne!(a, b);
}

#[test]
fn code_span_id_changes_with_line_start() {
    let a = code_span_id("https://a.com/f.rs", Some(10), Some(20), Some("main"));
    let b = code_span_id("https://a.com/f.rs", Some(5), Some(20), Some("main"));
    assert_ne!(a, b);
}

#[test]
fn code_span_id_changes_with_symbol() {
    let a = code_span_id("https://a.com/f.rs", Some(10), Some(20), Some("main"));
    let b = code_span_id("https://a.com/f.rs", Some(10), Some(20), Some("other"));
    assert_ne!(a, b);
}

#[test]
fn code_span_id_unrelated_fields_change_id() {
    let a = code_span_id("https://a.com/f.rs", Some(10), Some(20), None);
    let b = code_span_id("https://b.com/f.rs", Some(10), Some(20), None);
    assert_ne!(a, b);
}

// ===========================================================================
// Workstream 3: Warning-Code and Reason-Code Registry Tests
// ===========================================================================

#[test]
fn all_warning_codes_serialize_to_expected_snake_case() {
    let expected: Vec<(&WarningCode, &str)> = vec![
        (
            &WarningCode::UntrustedExternalContent,
            "untrusted_external_content",
        ),
        (
            &WarningCode::UntrustedLocalWorkspaceContent,
            "untrusted_local_workspace_content",
        ),
        (
            &WarningCode::PromptInjectionMarkerDetected,
            "prompt_injection_marker_detected",
        ),
        (&WarningCode::SafeSearchUnenforced, "safe_search_unenforced"),
        (&WarningCode::FreshnessUnenforced, "freshness_unenforced"),
        (
            &WarningCode::NativeCodeSearchUnavailable,
            "native_code_search_unavailable",
        ),
        (
            &WarningCode::NativeIssueSearchUnavailable,
            "native_issue_search_unavailable",
        ),
        (
            &WarningCode::NativeReleaseSearchUnavailable,
            "native_release_search_unavailable",
        ),
        (
            &WarningCode::NativeAdvisorySearchUnavailable,
            "native_advisory_search_unavailable",
        ),
        (
            &WarningCode::SymbolHintNoNativeProvider,
            "symbol_hint_no_native_provider",
        ),
        (
            &WarningCode::RepoHintsNotEnforcedNatively,
            "repo_hints_not_enforced_natively",
        ),
        (
            &WarningCode::IssueSearchNoNativeProvider,
            "issue_search_no_native_provider",
        ),
        (
            &WarningCode::ReleaseSearchNoNativeProvider,
            "release_search_no_native_provider",
        ),
        (&WarningCode::UnknownProvider, "unknown_provider"),
        (&WarningCode::DisabledProvider, "disabled_provider"),
        (&WarningCode::MissingApiKey, "missing_api_key"),
        (&WarningCode::ProviderFailed, "provider_failed"),
        (&WarningCode::ProviderTimeout, "provider_timeout"),
        (&WarningCode::ProviderRateLimited, "provider_rate_limited"),
        (&WarningCode::ProviderCooldown, "provider_cooldown"),
        (&WarningCode::ProfileDegraded, "profile_degraded"),
        (&WarningCode::ProfilePartial, "profile_partial"),
        (
            &WarningCode::ProfileProviderNotBuilt,
            "profile_provider_not_built",
        ),
        (
            &WarningCode::ProfileProviderUnknown,
            "profile_provider_unknown",
        ),
        (
            &WarningCode::ProfileProviderUnavailable,
            "profile_provider_unavailable",
        ),
        (
            &WarningCode::CodingProfileDegraded,
            "coding_profile_degraded",
        ),
        (&WarningCode::LocalRepoMatch, "local_repo_match"),
        (&WarningCode::LocalRepoDirty, "local_repo_dirty"),
        (
            &WarningCode::LocalRepoStateUnknown,
            "local_repo_state_unknown",
        ),
        (&WarningCode::LocalSearchTimeout, "local_search_timeout"),
        (&WarningCode::LocalSearchTruncated, "local_search_truncated"),
        (
            &WarningCode::FetchContentTruncated,
            "fetch_content_truncated",
        ),
        (&WarningCode::FetchLinksTruncated, "fetch_links_truncated"),
        (&WarningCode::FetchWarning, "fetch_warning"),
        (&WarningCode::UnknownWarning, "unknown_warning"),
        (
            &WarningCode::RequestDeadlineExceeded,
            "request_deadline_exceeded",
        ),
        (&WarningCode::SubqueryCapApplied, "subquery_cap_applied"),
        (&WarningCode::KevMatch, "kev_match"),
        (&WarningCode::KevAbsentNotProof, "kev_absent_not_proof"),
        (&WarningCode::KevLookupFailed, "kev_lookup_failed"),
        (&WarningCode::KevLookupSkipped, "kev_lookup_skipped"),
        (&WarningCode::SeverityUnavailable, "severity_unavailable"),
        (
            &WarningCode::VersionMatchUnavailable,
            "version_match_unavailable",
        ),
        (&WarningCode::VersionMismatch, "version_mismatch"),
        (
            &WarningCode::DependencyFileReadError,
            "dependency_file_read_error",
        ),
        (
            &WarningCode::ApplicabilityNotExploitability,
            "applicability_not_exploitability",
        ),
        (
            &WarningCode::PackageSecurityNoAdvisories,
            "package_security_no_advisories",
        ),
        (
            &WarningCode::PackageSecurityLookupFailed,
            "package_security_lookup_failed",
        ),
        (
            &WarningCode::PackageSecuritySkipped,
            "package_security_skipped",
        ),
        (&WarningCode::PackageResolution, "package_resolution"),
        (
            &WarningCode::PackageResolutionFallback,
            "package_resolution_fallback",
        ),
        (
            &WarningCode::NoNativeTreeProvider,
            "no_native_tree_provider",
        ),
        (
            &WarningCode::GenericContextUntrusted,
            "generic_context_untrusted",
        ),
        (
            &WarningCode::ProviderResolutionFailed,
            "provider_resolution_failed",
        ),
        (
            &WarningCode::DefaultProviderResolutionFailed,
            "default_provider_resolution_failed",
        ),
        (&WarningCode::EmptyResultGroup, "empty_result_group"),
        (
            &WarningCode::CardInjectionMarkerDetected,
            "card_injection_marker_detected",
        ),
        (&WarningCode::MaxResultsClamped, "max_results_clamped"),
    ];
    for (code, expected_str) in &expected {
        let json = serde_json::to_string(code).unwrap();
        assert_eq!(
            json,
            format!("\"{expected_str}\""),
            "serialize mismatch for {code:?}"
        );
        let as_str = code.as_str();
        assert_eq!(as_str, *expected_str, "as_str mismatch for {code:?}");
    }
}

#[test]
fn all_warning_codes_have_default_severity() {
    let all_codes = [
        WarningCode::UntrustedExternalContent,
        WarningCode::UntrustedLocalWorkspaceContent,
        WarningCode::PromptInjectionMarkerDetected,
        WarningCode::SafeSearchUnenforced,
        WarningCode::FreshnessUnenforced,
        WarningCode::NativeCodeSearchUnavailable,
        WarningCode::NativeIssueSearchUnavailable,
        WarningCode::NativeReleaseSearchUnavailable,
        WarningCode::NativeAdvisorySearchUnavailable,
        WarningCode::SymbolHintNoNativeProvider,
        WarningCode::RepoHintsNotEnforcedNatively,
        WarningCode::IssueSearchNoNativeProvider,
        WarningCode::ReleaseSearchNoNativeProvider,
        WarningCode::UnknownProvider,
        WarningCode::DisabledProvider,
        WarningCode::MissingApiKey,
        WarningCode::ProviderFailed,
        WarningCode::ProviderTimeout,
        WarningCode::ProviderRateLimited,
        WarningCode::ProviderCooldown,
        WarningCode::ProfileDegraded,
        WarningCode::ProfilePartial,
        WarningCode::ProfileProviderNotBuilt,
        WarningCode::ProfileProviderUnknown,
        WarningCode::ProfileProviderUnavailable,
        WarningCode::CodingProfileDegraded,
        WarningCode::LocalRepoMatch,
        WarningCode::LocalRepoDirty,
        WarningCode::LocalRepoStateUnknown,
        WarningCode::LocalSearchTimeout,
        WarningCode::LocalSearchTruncated,
        WarningCode::FetchContentTruncated,
        WarningCode::FetchLinksTruncated,
        WarningCode::FetchWarning,
        WarningCode::UnknownWarning,
        WarningCode::RequestDeadlineExceeded,
        WarningCode::SubqueryCapApplied,
        WarningCode::KevMatch,
        WarningCode::KevAbsentNotProof,
        WarningCode::KevLookupFailed,
        WarningCode::KevLookupSkipped,
        WarningCode::SeverityUnavailable,
        WarningCode::VersionMatchUnavailable,
        WarningCode::VersionMismatch,
        WarningCode::DependencyFileReadError,
        WarningCode::ApplicabilityNotExploitability,
        WarningCode::PackageSecurityNoAdvisories,
        WarningCode::PackageSecurityLookupFailed,
        WarningCode::PackageSecuritySkipped,
        WarningCode::PackageResolution,
        WarningCode::PackageResolutionFallback,
        WarningCode::NoNativeTreeProvider,
        WarningCode::GenericContextUntrusted,
        WarningCode::ProviderResolutionFailed,
        WarningCode::DefaultProviderResolutionFailed,
        WarningCode::EmptyResultGroup,
        WarningCode::CardInjectionMarkerDetected,
        WarningCode::MaxResultsClamped,
    ];
    for code in &all_codes {
        let severity = code.default_severity();
        assert!(
            matches!(
                severity,
                WarningSeverity::Notice
                    | WarningSeverity::Warning
                    | WarningSeverity::Error
                    | WarningSeverity::Info
            ),
            "WarningCode::{code:?} returned unexpected severity {severity:?}"
        );
    }
}

// --- FetchRankReason serialization ---

#[test]
fn fetch_rank_reason_serialization_stability() {
    let all_reasons = [
        FetchRankReason::PinnedRawPermalink,
        FetchRankReason::PinnedBrowserPermalink,
        FetchRankReason::MutableRawUrl,
        FetchRankReason::MutableBrowserUrl,
        FetchRankReason::GenericWebUrl,
        FetchRankReason::SparseCodeEvidence,
        FetchRankReason::ExactConfidence,
        FetchRankReason::StrongConfidence,
        FetchRankReason::WeakConfidence,
        FetchRankReason::UnknownConfidence,
        FetchRankReason::SourceRoleImplementation,
        FetchRankReason::SourceRoleDocumentation,
        FetchRankReason::SourceRoleReadme,
        FetchRankReason::SourceRoleExample,
        FetchRankReason::SourceRoleTest,
        FetchRankReason::SourceRoleChangelog,
        FetchRankReason::SourceRoleMigration,
        FetchRankReason::SourceRoleBenchmark,
        FetchRankReason::SourceRoleConfiguration,
        FetchRankReason::KindOfficialDocs,
        FetchRankReason::KindPackageRegistry,
        FetchRankReason::KindReleaseNotes,
        FetchRankReason::KindIssueThread,
        FetchRankReason::KindPullRequest,
        FetchRankReason::KindSecurityAdvisory,
        FetchRankReason::KindSourceFile,
        FetchRankReason::SymbolHintMatch,
        FetchRankReason::PathHintMatch,
        FetchRankReason::LanguageHintMatch,
        FetchRankReason::FileHintMatch,
        FetchRankReason::ErrorContextMatch,
        FetchRankReason::VersionMigrationContext,
        FetchRankReason::PackageNameMatch,
        FetchRankReason::SourceTypeMatch,
        FetchRankReason::AuthoritativeAdvisory,
        FetchRankReason::VendorAdvisory,
        FetchRankReason::PrimaryResearchSource,
        FetchRankReason::ReferenceImplementation,
        FetchRankReason::BenchmarkSource,
        FetchRankReason::SecurityConsideration,
    ];
    let strings: Vec<String> = all_reasons.iter().map(|r| r.as_str().to_string()).collect();
    assert_no_duplicates(&strings, "FetchRankReason");
    // Verify all strings are snake_case
    for s in &strings {
        assert!(
            s.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
            "non-snake_case FetchRankReason: {s}"
        );
    }
}

// --- EvidenceGapKind snake_case ---

#[test]
fn evidence_gap_kind_all_snake_case() {
    let all_kinds = [
        EvidenceGapKind::NoPrimarySourceFound,
        EvidenceGapKind::ProviderDegraded,
        EvidenceGapKind::NativeRepoFilterNotEnforced,
        EvidenceGapKind::SecurityApplicabilityUnknown,
        EvidenceGapKind::FetchFailed,
        EvidenceGapKind::SourceUnfetched,
        EvidenceGapKind::AllResultsExternalUntrusted,
        EvidenceGapKind::LocalCheckoutDirty,
        EvidenceGapKind::LocalRemoteMismatch,
        EvidenceGapKind::LocalGeneratedOrVendorOnly,
        EvidenceGapKind::LocalUntrackedFile,
        EvidenceGapKind::LocalSourceUnfetched,
        EvidenceGapKind::NativeAdvisoryUnavailable,
        EvidenceGapKind::SymbolHintNoNativeProvider,
        EvidenceGapKind::IssueSearchNoNativeProvider,
        EvidenceGapKind::ReleaseSearchNoNativeProvider,
        EvidenceGapKind::FreshnessNotEnforced,
        EvidenceGapKind::PackageResolutionFailed,
        EvidenceGapKind::NoFixedVersionFound,
        EvidenceGapKind::NoCounterpointFound,
        EvidenceGapKind::NoBenchmarksFound,
        EvidenceGapKind::MissingTests,
        EvidenceGapKind::MissingExamples,
        EvidenceGapKind::MissingManifest,
        EvidenceGapKind::MissingChangelog,
        EvidenceGapKind::MissingSecurityPolicy,
    ];
    for kind in &all_kinds {
        let json = serde_json::to_string(kind).unwrap();
        // Remove quotes
        let s = json.trim_matches('"');
        assert!(
            s.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
            "non-snake_case EvidenceGapKind: {s}"
        );
        // Verify roundtrip
        let parsed: EvidenceGapKind = serde_json::from_str(&json).unwrap();
        assert_eq!(format!("{parsed:?}"), format!("{kind:?}"));
    }
}

// --- Security remediation categories ---

#[test]
fn remediation_category_serialized_names_stability() {
    assert_serde_enum(&RemediationCategory::Upgrade, "\"upgrade\"");
    assert_serde_enum(&RemediationCategory::Pin, "\"pin\"");
    assert_serde_enum(&RemediationCategory::Replace, "\"replace\"");
    assert_serde_enum(
        &RemediationCategory::RemoveDependency,
        "\"remove_dependency\"",
    );
    assert_serde_enum(
        &RemediationCategory::ConfigurationMitigation,
        "\"configuration_mitigation\"",
    );
    assert_serde_enum(&RemediationCategory::FeatureDisable, "\"feature_disable\"");
    assert_serde_enum(
        &RemediationCategory::VulnerableApiAvoidance,
        "\"vulnerable_api_avoidance\"",
    );
    assert_serde_enum(
        &RemediationCategory::TransitiveOverride,
        "\"transitive_override\"",
    );
    assert_serde_enum(&RemediationCategory::VendorPatch, "\"vendor_patch\"");
    assert_serde_enum(&RemediationCategory::MonitorOnly, "\"monitor_only\"");
    assert_serde_enum(&RemediationCategory::ManualReview, "\"manual_review\"");
    assert_serde_enum(
        &RemediationCategory::NoActionSupportedByEvidence,
        "\"no_action_supported_by_evidence\"",
    );
}

#[test]
fn remediation_category_no_duplicate_serialized_names() {
    let variants = [
        RemediationCategory::Upgrade,
        RemediationCategory::Pin,
        RemediationCategory::Replace,
        RemediationCategory::RemoveDependency,
        RemediationCategory::ConfigurationMitigation,
        RemediationCategory::FeatureDisable,
        RemediationCategory::VulnerableApiAvoidance,
        RemediationCategory::TransitiveOverride,
        RemediationCategory::VendorPatch,
        RemediationCategory::MonitorOnly,
        RemediationCategory::ManualReview,
        RemediationCategory::NoActionSupportedByEvidence,
    ];
    let names = collect_enum_serialized_names(&variants);
    assert_no_duplicates(&names, "RemediationCategory");
}

// --- Research source classes and claim types ---

#[test]
fn research_source_class_serialized_names_stability() {
    assert_serde_enum(&ResearchSourceClass::OfficialDocs, "\"official_docs\"");
    assert_serde_enum(&ResearchSourceClass::ReferenceDocs, "\"reference_docs\"");
    assert_serde_enum(
        &ResearchSourceClass::RepositorySource,
        "\"repository_source\"",
    );
    assert_serde_enum(
        &ResearchSourceClass::MaintainerIssue,
        "\"maintainer_issue\"",
    );
    assert_serde_enum(&ResearchSourceClass::ReleaseNotes, "\"release_notes\"");
    assert_serde_enum(&ResearchSourceClass::Benchmark, "\"benchmark\"");
    assert_serde_enum(&ResearchSourceClass::Paper, "\"paper\"");
    assert_serde_enum(&ResearchSourceClass::StandardSpec, "\"standard_spec\"");
    assert_serde_enum(
        &ResearchSourceClass::SecurityAdvisory,
        "\"security_advisory\"",
    );
    assert_serde_enum(&ResearchSourceClass::VendorBlog, "\"vendor_blog\"");
    assert_serde_enum(
        &ResearchSourceClass::EngineeringBlog,
        "\"engineering_blog\"",
    );
    assert_serde_enum(&ResearchSourceClass::ForumThread, "\"forum_thread\"");
    assert_serde_enum(&ResearchSourceClass::NewsArticle, "\"news_article\"");
    assert_serde_enum(&ResearchSourceClass::Unknown, "\"unknown\"");
}

#[test]
fn research_source_class_no_duplicate_serialized_names() {
    let variants = [
        ResearchSourceClass::OfficialDocs,
        ResearchSourceClass::ReferenceDocs,
        ResearchSourceClass::RepositorySource,
        ResearchSourceClass::MaintainerIssue,
        ResearchSourceClass::ReleaseNotes,
        ResearchSourceClass::Benchmark,
        ResearchSourceClass::Paper,
        ResearchSourceClass::StandardSpec,
        ResearchSourceClass::SecurityAdvisory,
        ResearchSourceClass::VendorBlog,
        ResearchSourceClass::EngineeringBlog,
        ResearchSourceClass::ForumThread,
        ResearchSourceClass::NewsArticle,
        ResearchSourceClass::Unknown,
    ];
    let names = collect_enum_serialized_names(&variants);
    assert_no_duplicates(&names, "ResearchSourceClass");
}

#[test]
fn research_claim_type_serialized_names_stability() {
    assert_serde_enum(&ResearchClaimType::Performance, "\"performance\"");
    assert_serde_enum(&ResearchClaimType::Security, "\"security\"");
    assert_serde_enum(&ResearchClaimType::Maintenance, "\"maintenance\"");
    assert_serde_enum(&ResearchClaimType::Compatibility, "\"compatibility\"");
    assert_serde_enum(&ResearchClaimType::Architecture, "\"architecture\"");
    assert_serde_enum(&ResearchClaimType::ApiDesign, "\"api_design\"");
    assert_serde_enum(&ResearchClaimType::Operational, "\"operational\"");
    assert_serde_enum(&ResearchClaimType::Ecosystem, "\"ecosystem\"");
    assert_serde_enum(&ResearchClaimType::Cost, "\"cost\"");
    assert_serde_enum(&ResearchClaimType::Unknown, "\"unknown\"");
}

#[test]
fn research_claim_type_no_duplicate_serialized_names() {
    let variants = [
        ResearchClaimType::Performance,
        ResearchClaimType::Security,
        ResearchClaimType::Maintenance,
        ResearchClaimType::Compatibility,
        ResearchClaimType::Architecture,
        ResearchClaimType::ApiDesign,
        ResearchClaimType::Operational,
        ResearchClaimType::Ecosystem,
        ResearchClaimType::Cost,
        ResearchClaimType::Unknown,
    ];
    let names = collect_enum_serialized_names(&variants);
    assert_no_duplicates(&names, "ResearchClaimType");
}

// --- RepoResultGroupKind stability ---

#[test]
fn repo_result_group_kind_serialized_names_stability() {
    assert_serde_enum(&RepoResultGroupKind::OfficialDocs, "\"official_docs\"");
    assert_serde_enum(
        &RepoResultGroupKind::PackageRegistry,
        "\"package_registry\"",
    );
    assert_serde_enum(&RepoResultGroupKind::Repository, "\"repository\"");
    assert_serde_enum(&RepoResultGroupKind::Readme, "\"readme\"");
    assert_serde_enum(&RepoResultGroupKind::Examples, "\"examples\"");
    assert_serde_enum(&RepoResultGroupKind::Tests, "\"tests\"");
    assert_serde_enum(&RepoResultGroupKind::SourceFiles, "\"source_files\"");
    assert_serde_enum(&RepoResultGroupKind::Issues, "\"issues\"");
    assert_serde_enum(&RepoResultGroupKind::PullRequests, "\"pull_requests\"");
    assert_serde_enum(&RepoResultGroupKind::Releases, "\"releases\"");
    assert_serde_enum(&RepoResultGroupKind::MigrationNotes, "\"migration_notes\"");
    assert_serde_enum(&RepoResultGroupKind::Changelog, "\"changelog\"");
    assert_serde_enum(
        &RepoResultGroupKind::CommunityDiscussion,
        "\"community_discussion\"",
    );
    assert_serde_enum(&RepoResultGroupKind::Other, "\"other\"");
}

#[test]
fn repo_result_group_kind_no_duplicate_serialized_names() {
    let variants = [
        RepoResultGroupKind::OfficialDocs,
        RepoResultGroupKind::PackageRegistry,
        RepoResultGroupKind::Repository,
        RepoResultGroupKind::Readme,
        RepoResultGroupKind::Examples,
        RepoResultGroupKind::Tests,
        RepoResultGroupKind::SourceFiles,
        RepoResultGroupKind::Issues,
        RepoResultGroupKind::PullRequests,
        RepoResultGroupKind::Releases,
        RepoResultGroupKind::MigrationNotes,
        RepoResultGroupKind::Changelog,
        RepoResultGroupKind::CommunityDiscussion,
        RepoResultGroupKind::Other,
    ];
    let names = collect_enum_serialized_names(&variants);
    assert_no_duplicates(&names, "RepoResultGroupKind");
}

// --- ResearchDomain and ResearchWorkflow no duplicates ---

#[test]
fn research_domain_no_duplicate_serialized_names() {
    let variants = [
        ResearchDomain::General,
        ResearchDomain::SoftwareArchitecture,
        ResearchDomain::ApiDesign,
        ResearchDomain::DistributedSystems,
        ResearchDomain::Security,
        ResearchDomain::Performance,
        ResearchDomain::LanguageEcosystem,
        ResearchDomain::MachineLearning,
        ResearchDomain::Infrastructure,
    ];
    let names = collect_enum_serialized_names(&variants);
    assert_no_duplicates(&names, "ResearchDomain");
}

#[test]
fn research_workflow_no_duplicate_serialized_names() {
    let variants = [
        ResearchWorkflow::General,
        ResearchWorkflow::ApiEvaluation,
        ResearchWorkflow::LibraryComparison,
        ResearchWorkflow::MigrationPlanning,
        ResearchWorkflow::SecurityReview,
        ResearchWorkflow::PerformanceInvestigation,
        ResearchWorkflow::EcosystemSurvey,
        ResearchWorkflow::ArchitectureDecision,
    ];
    let names = collect_enum_serialized_names(&variants);
    assert_no_duplicates(&names, "ResearchWorkflow");
}

// --- SecurityResultGroupKind no duplicates ---

#[test]
fn security_result_group_kind_no_duplicate_serialized_names() {
    let variants = [
        SecurityResultGroupKind::AuthoritativeAdvisories,
        SecurityResultGroupKind::VendorAdvisories,
        SecurityResultGroupKind::PackageAdvisories,
        SecurityResultGroupKind::KevEntries,
        SecurityResultGroupKind::PatchCommitsOrReleases,
        SecurityResultGroupKind::ExploitDiscussion,
        SecurityResultGroupKind::DefensiveGuidance,
        SecurityResultGroupKind::GeneralContext,
        SecurityResultGroupKind::Other,
    ];
    let names = collect_enum_serialized_names(&variants);
    assert_no_duplicates(&names, "SecurityResultGroupKind");
}

// --- SeverityLevel no duplicates ---

#[test]
fn severity_level_no_duplicate_serialized_names() {
    let variants = [
        SeverityLevel::Critical,
        SeverityLevel::High,
        SeverityLevel::Medium,
        SeverityLevel::Low,
        SeverityLevel::Unknown,
    ];
    let names = collect_enum_serialized_names(&variants);
    assert_no_duplicates(&names, "SeverityLevel");
}

// --- WarningSeverity no duplicates ---

#[test]
fn warning_severity_no_duplicate_serialized_names() {
    let variants = [
        WarningSeverity::Info,
        WarningSeverity::Notice,
        WarningSeverity::Warning,
        WarningSeverity::Error,
    ];
    let names = collect_enum_serialized_names(&variants);
    assert_no_duplicates(&names, "WarningSeverity");
}

// --- RecipeDetail and RecipeSupport no duplicates ---

#[test]
fn recipe_detail_no_duplicate_serialized_names() {
    let variants = [
        RecipeDetail::None,
        RecipeDetail::Summary,
        RecipeDetail::Full,
    ];
    let names = collect_enum_serialized_names(&variants);
    assert_no_duplicates(&names, "RecipeDetail");
}

#[test]
fn recipe_support_no_duplicate_serialized_names() {
    let variants = [
        RecipeSupport::Available,
        RecipeSupport::Partial,
        RecipeSupport::Unavailable,
    ];
    let names = collect_enum_serialized_names(&variants);
    assert_no_duplicates(&names, "RecipeSupport");
}

// --- RankReason no duplicates ---

#[test]
fn rank_reason_no_duplicate_serialized_names() {
    let variants = [
        RankReason::RrfMultiProvider,
        RankReason::RrfProviderRank,
        RankReason::DomainPriorDocs,
        RankReason::DomainPriorCode,
        RankReason::DomainPriorSecurity,
        RankReason::DomainPriorRelease,
        RankReason::IntentMatch,
        RankReason::FreshnessMatch,
        RankReason::ExactTitleMatch,
        RankReason::CanonicalDedup,
        RankReason::ProviderNativeIssueSearch,
        RankReason::ProviderNativeReleaseSearch,
        RankReason::ProviderNativeAdvisorySearch,
        RankReason::RepoOwnerMatch,
        RankReason::HintMatch,
        RankReason::AdvisoryIdentifierMatch,
        RankReason::KevMatch,
        RankReason::VendorAdvisory,
        RankReason::PackageAdvisory,
        RankReason::DefensiveGuidance,
        RankReason::SecurityPrimarySource,
        RankReason::SecurityMaintainerSource,
        RankReason::VersionAffectedMatch,
        RankReason::ExactErrorPhraseMatch,
        RankReason::ErrorCodeMatch,
        RankReason::ToolchainMatch,
        RankReason::OfficialErrorDocs,
        RankReason::MaintainerIssueMatch,
        RankReason::RegressionReleaseMatch,
    ];
    let names = collect_enum_serialized_names(&variants);
    assert_no_duplicates(&names, "RankReason");
}

// --- Next-action reason codes from recipe_catalog ---

#[test]
fn next_action_reason_codes_web_search() {
    let actions =
        eggsearch::meta::recipe_catalog::web_search_next_actions(&["src_1".to_string()], true);
    assert!(!actions.is_empty());
    for action in &actions {
        assert!(
            !action.reason_code.is_empty(),
            "reason_code must not be empty"
        );
        assert!(
            action
                .reason_code
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_'),
            "reason_code must be snake_case: {}",
            action.reason_code
        );
    }
}

#[test]
fn next_action_reason_codes_repo_search() {
    let actions =
        eggsearch::meta::recipe_catalog::repo_search_next_actions(&["src_1".to_string()], true);
    assert!(!actions.is_empty());
    for action in &actions {
        assert!(
            !action.reason_code.is_empty(),
            "reason_code must not be empty"
        );
        assert!(
            action
                .reason_code
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_'),
            "reason_code must be snake_case: {}",
            action.reason_code
        );
    }
}

#[test]
fn next_action_reason_codes_security_search() {
    let actions =
        eggsearch::meta::recipe_catalog::security_search_next_actions(&["src_1".to_string()], true);
    assert!(!actions.is_empty());
    for action in &actions {
        assert!(
            !action.reason_code.is_empty(),
            "reason_code must not be empty"
        );
        assert!(
            action
                .reason_code
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_'),
            "reason_code must be snake_case: {}",
            action.reason_code
        );
    }
}

#[test]
fn next_action_reason_codes_research_search() {
    let actions =
        eggsearch::meta::recipe_catalog::research_search_next_actions(&["src_1".to_string()], true);
    assert!(!actions.is_empty());
    for action in &actions {
        assert!(
            !action.reason_code.is_empty(),
            "reason_code must not be empty"
        );
        assert!(
            action
                .reason_code
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_'),
            "reason_code must be snake_case: {}",
            action.reason_code
        );
    }
}

// --- WarningCode as_str is snake_case ---

#[test]
fn warning_code_as_str_all_snake_case() {
    let all_codes = [
        WarningCode::UntrustedExternalContent,
        WarningCode::UntrustedLocalWorkspaceContent,
        WarningCode::PromptInjectionMarkerDetected,
        WarningCode::SafeSearchUnenforced,
        WarningCode::FreshnessUnenforced,
        WarningCode::NativeCodeSearchUnavailable,
        WarningCode::NativeIssueSearchUnavailable,
        WarningCode::NativeReleaseSearchUnavailable,
        WarningCode::NativeAdvisorySearchUnavailable,
        WarningCode::SymbolHintNoNativeProvider,
        WarningCode::RepoHintsNotEnforcedNatively,
        WarningCode::IssueSearchNoNativeProvider,
        WarningCode::ReleaseSearchNoNativeProvider,
        WarningCode::UnknownProvider,
        WarningCode::DisabledProvider,
        WarningCode::MissingApiKey,
        WarningCode::ProviderFailed,
        WarningCode::ProviderTimeout,
        WarningCode::ProviderRateLimited,
        WarningCode::ProviderCooldown,
        WarningCode::ProfileDegraded,
        WarningCode::ProfilePartial,
        WarningCode::ProfileProviderNotBuilt,
        WarningCode::ProfileProviderUnknown,
        WarningCode::ProfileProviderUnavailable,
        WarningCode::CodingProfileDegraded,
        WarningCode::LocalRepoMatch,
        WarningCode::LocalRepoDirty,
        WarningCode::LocalRepoStateUnknown,
        WarningCode::LocalSearchTimeout,
        WarningCode::LocalSearchTruncated,
        WarningCode::FetchContentTruncated,
        WarningCode::FetchLinksTruncated,
        WarningCode::FetchWarning,
        WarningCode::UnknownWarning,
        WarningCode::RequestDeadlineExceeded,
        WarningCode::SubqueryCapApplied,
        WarningCode::KevMatch,
        WarningCode::KevAbsentNotProof,
        WarningCode::KevLookupFailed,
        WarningCode::KevLookupSkipped,
        WarningCode::SeverityUnavailable,
        WarningCode::VersionMatchUnavailable,
        WarningCode::VersionMismatch,
        WarningCode::DependencyFileReadError,
        WarningCode::ApplicabilityNotExploitability,
        WarningCode::PackageSecurityNoAdvisories,
        WarningCode::PackageSecurityLookupFailed,
        WarningCode::PackageSecuritySkipped,
        WarningCode::PackageResolution,
        WarningCode::PackageResolutionFallback,
        WarningCode::NoNativeTreeProvider,
        WarningCode::GenericContextUntrusted,
        WarningCode::ProviderResolutionFailed,
        WarningCode::DefaultProviderResolutionFailed,
        WarningCode::EmptyResultGroup,
        WarningCode::CardInjectionMarkerDetected,
        WarningCode::MaxResultsClamped,
    ];
    for code in &all_codes {
        let s = code.as_str();
        assert!(!s.is_empty(), "as_str() returned empty for {code:?}");
        assert!(
            s.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
            "as_str() not snake_case for {code:?}: {s}"
        );
    }
}

// ===========================================================================
// ProviderSkipCode schema contract tests
// ===========================================================================

use eggsearch::core::provider::ProviderSkipCode;

#[test]
fn provider_skip_code_serialized_names_stability() {
    assert_serde_enum(&ProviderSkipCode::UnknownProvider, "\"unknown_provider\"");
    assert_serde_enum(&ProviderSkipCode::DisabledByUser, "\"disabled_by_user\"");
    assert_serde_enum(&ProviderSkipCode::MissingApiKey, "\"missing_api_key\"");
    assert_serde_enum(
        &ProviderSkipCode::MissingSearxngConfig,
        "\"missing_searxng_config\"",
    );
    assert_serde_enum(&ProviderSkipCode::MissingBaseUrl, "\"missing_base_url\"");
    assert_serde_enum(&ProviderSkipCode::InvalidBaseUrl, "\"invalid_base_url\"");
    assert_serde_enum(
        &ProviderSkipCode::MissingLocalBackend,
        "\"missing_local_backend\"",
    );
    assert_serde_enum(
        &ProviderSkipCode::CredentialNotConfigured,
        "\"credential_not_configured\"",
    );
    assert_serde_enum(
        &ProviderSkipCode::CredentialEnvMissing,
        "\"credential_env_missing\"",
    );
    assert_serde_enum(
        &ProviderSkipCode::CredentialInvalid,
        "\"credential_invalid\"",
    );
    assert_serde_enum(&ProviderSkipCode::CooldownActive, "\"cooldown_active\"");
    assert_serde_enum(&ProviderSkipCode::NotBuilt, "\"not_built\"");
    assert_serde_enum(&ProviderSkipCode::Unknown, "\"unknown\"");
}

#[test]
fn provider_skip_code_no_duplicate_serialized_names() {
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
    let names = collect_enum_serialized_names(&variants);
    assert_no_duplicates(&names, "ProviderSkipCode");
}

// ===========================================================================
// Workstream 1 (extended): raw_text metadata contract tests
// ===========================================================================

#[test]
fn web_fetch_response_raw_text_metadata_present_when_raw_text_present() {
    let resp = eggsearch::core::WebFetchResponse {
        url: "https://example.com".to_string(),
        final_url: "https://example.com".to_string(),
        stable_id: None,
        source_id: None,
        title: None,
        description: None,
        content_type: None,
        status: 200,
        fetched: true,
        truncated: false,
        trust: eggsearch::core::FetchTrust::ExternalUntrusted,
        text: Some("hello".to_string()),
        raw_text: Some("hello raw".to_string()),
        raw_text_chars_returned: Some(9),
        raw_text_truncated: false,
        raw_text_cap: Some(50000),
        links: vec![],
        links_seen: None,
        links_truncated: false,
        warnings: vec![],
        trust_markers: eggsearch::core::TrustMarkers::default(),
        document: None,
        fetch_transform: None,
        structured_warnings: vec![],
        pdf_page_metadata: None,
        pdf_document_metadata: None,
        pdf_quality_score: None,
        pdf_content_ok: None,
        cache_status: eggsearch::fetch::cache::CacheStatus::default(),
        attempt_count: None,
        retry_after_ms: None,
        origin_backoff_ms: None,
        response_headers: None,
        transport: Some("http".to_string()),
        browser_escalated: false,
        manual_interaction_required: false,
        raw_body: None,
    };
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["raw_text"], "hello raw");
    assert_eq!(json["raw_text_chars_returned"], 9);
    assert!(
        !json.as_object().unwrap().contains_key("raw_text_truncated"),
        "raw_text_truncated should be omitted when false (skip_serializing_if)"
    );
    assert_eq!(json["raw_text_cap"], 50000);
}

#[test]
fn web_fetch_response_raw_text_metadata_absent_when_raw_text_none() {
    let resp = eggsearch::core::WebFetchResponse {
        url: "https://example.com".to_string(),
        final_url: "https://example.com".to_string(),
        stable_id: None,
        source_id: None,
        title: None,
        description: None,
        content_type: None,
        status: 200,
        fetched: true,
        truncated: false,
        trust: eggsearch::core::FetchTrust::ExternalUntrusted,
        text: Some("hello".to_string()),
        raw_text: None,
        raw_text_chars_returned: None,
        raw_text_truncated: false,
        raw_text_cap: None,
        links: vec![],
        links_seen: None,
        links_truncated: false,
        warnings: vec![],
        trust_markers: eggsearch::core::TrustMarkers::default(),
        document: None,
        fetch_transform: None,
        structured_warnings: vec![],
        pdf_page_metadata: None,
        pdf_document_metadata: None,
        pdf_quality_score: None,
        pdf_content_ok: None,
        cache_status: eggsearch::fetch::cache::CacheStatus::default(),
        attempt_count: None,
        retry_after_ms: None,
        origin_backoff_ms: None,
        response_headers: None,
        transport: Some("http".to_string()),
        browser_escalated: false,
        manual_interaction_required: false,
        raw_body: None,
    };
    let json = serde_json::to_value(&resp).unwrap();
    assert!(
        !json.as_object().unwrap().contains_key("raw_text"),
        "raw_text should be absent when None"
    );
    assert!(
        !json
            .as_object()
            .unwrap()
            .contains_key("raw_text_chars_returned"),
        "raw_text_chars_returned should be absent when None"
    );
    assert!(
        !json.as_object().unwrap().contains_key("raw_text_cap"),
        "raw_text_cap should be absent when None"
    );
}

#[test]
fn web_fetch_response_raw_text_truncated_omitted_when_false() {
    let resp = eggsearch::core::WebFetchResponse {
        url: "https://example.com".to_string(),
        final_url: "https://example.com".to_string(),
        stable_id: None,
        source_id: None,
        title: None,
        description: None,
        content_type: None,
        status: 200,
        fetched: true,
        truncated: false,
        trust: eggsearch::core::FetchTrust::ExternalUntrusted,
        text: Some("hello".to_string()),
        raw_text: Some("hello raw".to_string()),
        raw_text_chars_returned: Some(9),
        raw_text_truncated: false,
        raw_text_cap: Some(50000),
        links: vec![],
        links_seen: None,
        links_truncated: false,
        warnings: vec![],
        trust_markers: eggsearch::core::TrustMarkers::default(),
        document: None,
        fetch_transform: None,
        structured_warnings: vec![],
        pdf_page_metadata: None,
        pdf_document_metadata: None,
        pdf_quality_score: None,
        pdf_content_ok: None,
        cache_status: eggsearch::fetch::cache::CacheStatus::default(),
        attempt_count: None,
        retry_after_ms: None,
        origin_backoff_ms: None,
        response_headers: None,
        transport: Some("http".to_string()),
        browser_escalated: false,
        manual_interaction_required: false,
        raw_body: None,
    };
    let json = serde_json::to_value(&resp).unwrap();
    assert!(
        !json.as_object().unwrap().contains_key("raw_text_truncated"),
        "raw_text_truncated should be omitted when false"
    );
}

#[test]
fn web_fetch_response_raw_text_truncated_present_when_true() {
    let resp = eggsearch::core::WebFetchResponse {
        url: "https://example.com".to_string(),
        final_url: "https://example.com".to_string(),
        stable_id: None,
        source_id: None,
        title: None,
        description: None,
        content_type: None,
        status: 200,
        fetched: true,
        truncated: false,
        trust: eggsearch::core::FetchTrust::ExternalUntrusted,
        text: Some("hello".to_string()),
        raw_text: Some("hello raw".to_string()),
        raw_text_chars_returned: Some(9),
        raw_text_truncated: true,
        raw_text_cap: Some(50000),
        links: vec![],
        links_seen: None,
        links_truncated: false,
        warnings: vec![],
        trust_markers: eggsearch::core::TrustMarkers::default(),
        document: None,
        fetch_transform: None,
        structured_warnings: vec![],
        pdf_page_metadata: None,
        pdf_document_metadata: None,
        pdf_quality_score: None,
        pdf_content_ok: None,
        cache_status: eggsearch::fetch::cache::CacheStatus::default(),
        attempt_count: None,
        retry_after_ms: None,
        origin_backoff_ms: None,
        response_headers: None,
        transport: Some("http".to_string()),
        browser_escalated: false,
        manual_interaction_required: false,
        raw_body: None,
    };
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["raw_text_truncated"], true);
}
