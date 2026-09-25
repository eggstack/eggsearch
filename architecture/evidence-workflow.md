# Evidence & Workflow Deep Dive

**Path:** `src/core/evidence_bundle.rs`, `src/core/evidence_role.rs`, `src/core/workflow_coverage.rs`, `src/core/conflict.rs`, `src/core/retrieval_status.rs`, `src/core/evidence_postprocess.rs`, `src/meta/evidence_bundle.rs`, `src/meta/workflow.rs`, `src/meta/fetch_ranking.rs`, `src/mcp/tools/evidence_bundle.rs`
**Purpose:** Evidence packaging, role taxonomy, workflow guidance, conflict detection, retrieval tracking, and suggested-fetch ranking.

---

## Non-Summarizing Portable Bundles

An `EvidenceBundle` (`src/core/evidence_bundle.rs`) is a
deterministic container for multi-agent handoff. It packages
already-selected evidence for reuse without repeating search and
fetch work. The bundle is not a conclusion: it never summarizes or
reinterprets untrusted content.

```
EvidenceBundle
  ├── bundle_id: String (FNV-1a, prefix "bundle_")
  ├── goal: Option<String>
  ├── created_at: String (RFC 3339)
  ├── sources: Vec<EvidenceBundleSource>
  ├── fetched_items: Vec<EvidenceBundleFetchedItem>
  ├── source_links: Vec<EvidenceBundleLink>
  ├── trust_summary: EvidenceTrustSummary
  ├── provider_summary: EvidenceProviderSummary
  ├── gaps: Vec<EvidenceGap>
  ├── warnings: Vec<SearchWarning>
  ├── structured_warnings: Vec<AgentWarning>
  ├── limits: EvidenceBundleLimits
  ├── research_claims: Option<Vec<ResearchClaim>>
  └── research_conflicts: Option<Vec<ResearchConflict>>
```

`EvidenceBundleSource` preserves source identity: deterministic
`source_id` (`src_<hash>`), `original_id`, `url`, `title`,
`source_kind`, `source_role`, `provider_id`, `rank`, `score`,
`rank_reasons`, `trust`, `trust_markers`, `quality`, `stable`, a
structured repo-fetch locator, full `metadata`, and
`evidence_role`. `EvidenceBundleFetchedItem` preserves fetch
identity: `fetch_id` (`fetch_<hash>`), `source_id`, `url`, `locator`,
`fetched`, `content_type`, `language`, `selected_span`,
`code_span_id`, `line_start`, `line_end`, bounded `text`,
`truncated`, `trust`, `trust_markers`, `warnings`.

Request caps (`EvidenceBundleRequest`): `max_sources` (default
`DEFAULT_MAX_SOURCES = 50`), `max_fetched_items` (default
`DEFAULT_MAX_FETCHED_ITEMS = 20`), `max_total_chars` (default
`DEFAULT_MAX_TOTAL_CHARS = 100_000`), clamped to server caps
`MAX_SOURCES_CAP = 200`, `MAX_FETCHED_ITEMS_CAP = 100`,
`MAX_TOTAL_CHARS_CAP = 500_000`. `include_unfetched_sources`
defaults to `true`. `EvidenceBundleLimits` records the applied caps
plus `sources_truncated`, `fetched_items_truncated`, and
`total_chars_exceeded`.

`EvidenceGapKind` (26 variants): `NoPrimarySourceFound`,
`ProviderDegraded`, `NativeRepoFilterNotEnforced`,
`SecurityApplicabilityUnknown`, `FetchFailed`, `SourceUnfetched`,
`AllResultsExternalUntrusted`, `LocalCheckoutDirty`,
`LocalRemoteMismatch`, `LocalGeneratedOrVendorOnly`,
`LocalUntrackedFile`, `LocalSourceUnfetched`,
`NativeAdvisoryUnavailable`, `SymbolHintNoNativeProvider`,
`IssueSearchNoNativeProvider`, `ReleaseSearchNoNativeProvider`,
`FreshnessNotEnforced`, `PackageResolutionFailed`,
`NoFixedVersionFound`, `NoCounterpointFound`, `NoBenchmarksFound`,
`MissingTests`, `MissingExamples`, `MissingManifest`,
`MissingChangelog`, `MissingSecurityPolicy`. Each `EvidenceGap`
carries `kind`, `message`, optional `source_id` / `provider_id`,
`affected_source_ids`, and optional `evidence_role`.

Source-to-fetch links (`EvidenceBundleLink`) use one of four
`EvidenceBundleLinkReason` values: `SourceIdMatch` (explicit caller
link, highest priority), `UrlMatch` (trailing-slash and case
normalized), `LocatorMatch` (host/owner/repo/path comparison), or
`Explicit`.

---

## 19-Role Taxonomy

`EvidenceRole` (`src/core/evidence_role.rs`) defaults to
`UnknownOrWeakContext`. The remaining 18 variants:
`PrimaryImplementation`, `InterfaceOrApiDefinition`, `UsageExample`,
`TestOrBehavioralSpecification`, `ConfigurationOrFeatureGate`,
`ManifestOrDependencyMetadata`, `OfficialDocumentation`,
`ArchitectureOrDesignDocument`, `ReleaseNoteOrChangelog`,
`MigrationGuidance`, `BenchmarkOrPerformanceEvidence`,
`IssueOrIncidentDiscussion`, `PullRequestOrDesignReview`,
`AuthoritativeSecurityAdvisory`, `VendorSecurityGuidance`,
`IndependentCorroboration`, `CounterpointOrConflictingEvidence`,
`CommunityDiscussion`. Serialization is `snake_case`
(`"primary_implementation"`); `label()` gives the display form.

Five deterministic converters map domain taxonomies onto the unified
role: `from_source_kind()` (`SourceKind`), `from_source_role()`
(`SourceRole`: `Implementation`/`Generated` collapse to
`PrimaryImplementation`; `Vendor`/`Unknown` collapse to
`UnknownOrWeakContext`), `from_research_source_class()`
(`ResearchSourceClass`: `Paper` maps to `IndependentCorroboration`,
`VendorBlog` to `VendorSecurityGuidance`),
`from_security_source_tier()` (`SecuritySourceTier`:
`PrimaryAdvisory`/`PackageRegistryAdvisory` collapse to
`AuthoritativeSecurityAdvisory`), and `from_research_source_type()`
(`ResearchSourceType`; only `Counterpoints` maps to
`CounterpointOrConflictingEvidence`).

`assign_evidence_role()` prefers the card's explicit
`metadata.evidence_role`, then code-evidence `source_role`, then
`source_kind`. `materialize_evidence_roles()` backfills every card;
`compute_evidence_role_summary()` counts roles (sorted by count, ties
by label, truncated to 30 entries) and derives a generic
coverage status from `PrimaryImplementation` /
`OfficialDocumentation` presence.

---

## Coverage Models per `WorkflowKind`

`WorkflowKind` (10 variants, `as_str()` snake-case, `parse()`
accepting short aliases like `"api"`, `"migration"`, `"security"`):
`ApiComprehension`, `RepositoryArchitecture`, `ErrorInvestigation`,
`VersionMigration`, `SecurityReview`, `DependencyEvaluation`,
`PerformanceInvestigation`, `ComparativeResearch`,
`PreChangeEvidence`, `PostChangeReview`. `to_model()` returns the
corresponding `WorkflowCoverageModel` (`workflow_id`, `title`,
`required`, `recommended`, `optional`).

| Model | Required | Recommended |
|-------|----------|-------------|
| `api_comprehension` | InterfaceOrApiDefinition, PrimaryImplementation | OfficialDocumentation, UsageExample, TestOrBehavioralSpecification |
| `repo_architecture` | PrimaryImplementation, ArchitectureOrDesignDocument | OfficialDocumentation, ConfigurationOrFeatureGate, ManifestOrDependencyMetadata |
| `error_investigation` | IssueOrIncidentDiscussion, PrimaryImplementation | OfficialDocumentation, TestOrBehavioralSpecification |
| `version_migration` | ReleaseNoteOrChangelog, MigrationGuidance | OfficialDocumentation, IssueOrIncidentDiscussion |
| `security_review` | AuthoritativeSecurityAdvisory, VendorSecurityGuidance | PrimaryImplementation, ConfigurationOrFeatureGate, ManifestOrDependencyMetadata |
| `dependency_evaluation` | ManifestOrDependencyMetadata | OfficialDocumentation, ReleaseNoteOrChangelog, AuthoritativeSecurityAdvisory |
| `performance_investigation` | BenchmarkOrPerformanceEvidence | PrimaryImplementation, OfficialDocumentation, IndependentCorroboration |
| `comparative_research` | OfficialDocumentation, PrimaryImplementation | BenchmarkOrPerformanceEvidence, IndependentCorroboration, CounterpointOrConflictingEvidence |
| `pre_change_evidence` | PrimaryImplementation, TestOrBehavioralSpecification | OfficialDocumentation, ConfigurationOrFeatureGate |
| `post_change_review` | TestOrBehavioralSpecification | PrimaryImplementation, OfficialDocumentation, ConfigurationOrFeatureGate |

`CoverageStatus`: `Sufficient` (required + recommended found),
`UsableWithGaps` (required found, recommended missing),
`Insufficient` (required missing), `IndeterminateDueToFailures`
(required role hit a capability/policy skip or an indeterminate
retrieval). `RetrievalFailureKind` (9 variants) distinguishes
`NoMatchingEvidenceFound`, `ProviderCapabilityUnavailable`,
`ProviderSkippedByPolicy`, `ProviderFailed`,
`DeadlinePreventedCompletion`, `ResultTruncatedByCap`,
`EvidenceRoleNotRequested`, `EvidenceRoleRequestedButNotFound`, and
`EvidenceRoleIndeterminateBecauseRetrievalFailed`.
`compute_coverage()` evaluates a model against found roles and
failures; `generate_gap_driven_next_actions()` turns missing roles
into `AgentNextAction` hints via `role_to_next_tool()`.

Model selection carries its precedence layer in
`ResolutionSource`: `ExplicitWorkflow`, `Profile`, `Mode`,
`Domain`, `Default`. `resolve_workflow_model_with_context()` takes a
`WorkflowResolutionContext` (tool, explicit workflow, profile,
research domain, exact-error flag); the legacy
`resolve_workflow_model()` maps `repo_search` (exact-error forces
error investigation; `security`/`research` profiles select their
models), `research_search` (by research domain), `security_search`
(always security review), and `web_search` (no model).

---

## Conflict Detection Scoped to Canonical Entities

`ConflictClass` (8 variants): `DifferingVersionRanges`,
`ConflictingReleaseDates`, `MutuallyExclusiveStatusFields`,
`DivergentBenchmarkNumbers`, `DocumentationImplementationMismatch`,
`MutableVsCommitPinnedContent`, `DifferentProviderMetadata`,
`Unknown` (default). `ConflictSeverity`: `Critical`, `High`,
`Medium`, `Low`, `Informational` (default). `ConflictResolution`:
`PreferCommitPinned`, `PreferAuthoritativeSource`, `PreferNewerDate`,
`PreferHigherVersion`, `ManualReviewRequired`, `NoRecommendation`
(default). Each `EvidenceConflict` carries a deterministic
`conflict_<hash>` id, `source_ids`, `conflict_class`,
`compared_fields`, `values`, `directly_comparable`, `severity`,
`resolution`, and `message`.

Comparison is entity-scoped so unrelated sources never conflict.
`ConflictEntityType`: `Vulnerability`, `Package`, `Benchmark`,
`Repository`, `Documentation`. `ConflictEntityKey` is
(entity_type, `canonical_id`, `field`); `extract_entity_key()`
derives it from card metadata — vulnerability cards via the first
CVE, GHSA, or OSV id; repository cards via `code.repo`. Cards without
an identifiable entity return `None` and are skipped.
`detect_entity_scoped_conflicts()` groups cards by (entity type,
canonical id), requires at least two cards with `stable_id`s per
group, and only emits conflicts for directly comparable values.
`detect_mutable_vs_pinned()` fires when one repo key has both mutable
branch and commit-pinned (`commit_sha`) sources. `postprocess()`
combines both paths through `detect_structured_conflicts()`, capped
at `MAX_CONFLICTS = 20`.

---

## Retrieval Ledger

`RetrievalAttempt` records one terminal provider operation:
`provider_id`, `subquery_id`, `operation_id` (a
`RetrievalOperationIdentity::stable_id()` — never raw query text),
`intended_roles`, `outcome`, `result_count`, `error_class`,
`deadline_interrupted`, `truncated`, `truncation_evidence`,
`query_fingerprint` (`fp_<hash>`, non-recoverable), `duration_ms`.

`RetrievalAttemptOutcome` (10 variants): `SuccessWithResults`,
`SuccessZeroResults`, `Failed`, `TimedOut`, `RateLimited`,
`SkippedByPolicy`, `SkippedCapabilityUnavailable`, `NotApplicable`,
`InterruptedByDeadline`, `TruncatedAfterPartialSuccess`.
`TruncationEvidence`: `None` (default), `LimitReachedUnknown`,
`ConfirmedByEggsearch`, `ConfirmedByProvider`;
`effective_truncation_evidence()` infers confirmation from the
`truncated` flag or partial-success outcome.
`RetrievalOperationIdentity`: `SearchSubquery`,
`AdvisoryLookupById`, `AdvisoryQueryByPackage`, `KevLookup`, rendered
as `search:{id}`, `advisory-id:{fp}`,
`advisory-package:{eco}:{fp}:{fp|none}`, `kev:{fp}`.

Uniqueness invariant: at most one terminal attempt per
(`provider_id`, operation identity, evidence role) tuple.
`validate_attempt_ledger()` enforces it plus cross-field invariants
(`AttemptLedgerViolation`, 9 variants:
`DuplicateProviderOperationRole`, `EmptyProviderId`,
`DuplicateRoleInAttempt`, `ResultCountWithFailure`,
`ZeroResultWithCount`, `SuccessWithZeroResults`,
`DeadlineWithoutFlag`, `TruncationWithoutSuccess`,
`CapabilitySkipWithEmptyRoles`); `debug_validate_attempt_ledger()`
panics on violation in debug/test builds and is a no-op in release.
`map_provider_to_intended_roles()` derives intended roles from the
subquery label (`advisory`, `vendor`, `source`, `issues`,
`releases`, `docs`, `benchmarks`, `research`, `error_*`, …) with a
provider-id fallback.

The authoritative coarse state is `RetrievalDimensionState` (8
states): `Satisfied`, `CompletedNoMatch`, `Failed`,
`SkippedByPolicy`, `CapabilityUnavailable`, `Interrupted`, `Partial`,
`NotApplicable`. `attempt_outcome_to_dimension_state()` maps outcomes
to states; confirmed truncation takes precedence and maps to
`Partial`, while `LimitReachedUnknown` stays `Satisfied`.
`EvidenceAbsenceKind` (10 variants) carries the legacy absence
context alongside each dimension. State-aware helpers:
`summarize_retrieval()` (dimension-level), `summarize_retrieval_with_attempts()`
(authoritative path: job counts from attempts, dimension counts from
role-expanded dimensions), `is_absence_only()`,
`is_failure_only()`, `has_indeterminate()`, `absent_roles()`,
`failed_providers()`, `classify_absence()`,
`attempts_to_failures()` / `to_retrieval_failures()` (attempts to
`RetrievalFailure` records for coverage input).

Count invariants: `attempted_job_count == completed + failed +
policy_skipped + capability_skipped`; `attempted_dimension_count ==
dimensions.len() >= attempted_job_count`. `not_applicable` is counted
at both levels (`not_applicable_count` for dimensions,
`not_applicable_job_count` for attempts) and folds into completed
counts, never failure counts.

---

## Suggested-Fetch Ranking

`FetchCandidate` (internal, converted to public suggested-fetch types
by the caller) carries `url`, a structured repo-fetch flag,
`group`, kinds (`expected_kind`, `source_kind`, `source_role`,
`evidence_confidence`), stability flags (`is_pinned_permalink`,
`is_raw_url`, `is_browser_url`, `stable`, `domain`), `score`,
`reasons`, `information_gain`, `original_order`, and
`source_card_stable_id` linking back to the originating card.

`FetchCandidateBuilder` is shared by repo, research, and security
paths: `new(url)` for synthetic candidates (derives domain and
stability flags from the URL), `from_card(card, url)` for
card-derived candidates (derives role, confidence, and card linkage).
Callers set `group`, kinds, order, and structured locators
explicitly; `build()` applies ranking defaults (score 0, stable
false). `score_candidate()` layers provenance scoring, confidence
scoring, a mode pass (`FetchRankMode::Normal`, `ExactError`,
`PackageMigration`, `Security`, `Research`), and query-context
scoring. `rank_and_select()` scores all candidates, stable-sorts by
descending score (original order breaks ties deterministically),
applies diversity caps (`DiversityConfig` default: `max_per_domain =
2`, `max_per_group = 2`, `total_cap = 8`), and assigns
`information_gain` (1.0 first-seen domain+group, 0.7 near-duplicate,
0.4 otherwise).

Shared execution substrate (`src/meta/workflow.rs`) keeps domain
policy typed while mechanics stay common: `PlannedLane` (label,
query, priority, `intended_roles`, repo scope, excerpt demand),
`RetrievalAttemptSet` (deterministic ledger wrapper),
`NormalizedLaneResults` (insertion-ordered cards; RRF dedup stays in
the aggregation path), `EvidenceGroup`, `CoverageSummary`,
`FetchCandidateSet`, and `WorkflowExecution` (one call's ledger plus
cards). Domain planners, grouping, and builders stay typed per
workflow and are never flattened into one generic path.

---

## `build_evidence_bundle`: Sync, Local-Only, Identity-Preserving

`run_build_evidence_bundle` (`src/mcp/tools/evidence_bundle.rs`)
validates caps (rejects `max_sources > 200`, `max_fetched_items >
100`, `max_total_chars > 500_000`), requires at least one source or
fetch input, and calls the pure sync builder
`meta::evidence_bundle::build_evidence_bundle()` — no I/O, no
provider dispatch, no summarization. The 11-phase pipeline:
convert inputs with deterministic ids, dedupe sources by URL (richer
metadata wins), apply the source cap, convert fetches, link
(`SourceIdMatch` → `UrlMatch` → `LocatorMatch`), optionally drop
unfetched sources, apply fetch caps and the char budget, compute
trust/provider summaries and deterministic gaps, and derive
`compute_bundle_id(goal, source_ids, fetch_ids)`.

Identity is preserved end to end via `src/core/identity.rs` FNV-1a
ids: `compute_source_id` (provider + url + title + kind),
`compute_fetch_id` (url/locator + line range + text prefix),
`compute_bundle_id` (goal + source ids + fetch ids). The tool accepts
`response_detail` but projection is a passthrough for
`build_evidence_bundle`: canonical content is identical in all modes.

---

**Back to:** [overview.md](overview.md)
