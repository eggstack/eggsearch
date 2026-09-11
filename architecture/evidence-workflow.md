# Evidence & Workflow Deep Dive

**Path:** `src/core/evidence_bundle.rs`, `src/core/evidence_role.rs`, `src/core/evidence_postprocess.rs`, `src/core/workflow.rs`, `src/core/workflow_coverage.rs`, `src/core/conflict.rs`, `src/core/retrieval_status.rs`
**Purpose:** Evidence packaging, role taxonomy, workflow guidance, conflict detection, and retrieval tracking.

---

## Evidence Roles (`src/core/evidence_role.rs`)

19-variant taxonomy mapping across source kinds, roles, classes, and tiers:

| Role | Meaning |
|------|---------|
| `UnknownOrWeakContext` | Default for unclassified or weak-context sources |
| `PrimaryImplementation` | Core implementation code |
| `InterfaceOrApiDefinition` | API surface, public interface |
| `UsageExample` | Example code, usage patterns |
| `TestOrBehavioralSpecification` | Test code, behavioral specs |
| `ConfigurationOrFeatureGate` | Config files, feature flags |
| `ManifestOrDependencyMetadata` | Package manifests, lock files |
| `OfficialDocumentation` | Vendor documentation |
| `ArchitectureOrDesignDocument` | ADRs, design docs |
| `ReleaseNoteOrChangelog` | Release notes, changelogs |
| `MigrationGuidance` | Upgrade paths, migration guides |
| `BenchmarkOrPerformanceEvidence` | Performance data |
| `IssueOrIncidentDiscussion` | Bug reports, incidents |
| `PullRequestOrDesignReview` | PRs, code reviews |
| `AuthoritativeSecurityAdvisory` | Primary security advisory |
| `VendorSecurityGuidance` | Vendor security bulletin |
| `IndependentCorroboration` | Third-party analysis |
| `CounterpointOrConflictingEvidence` | Contradicting evidence |
| `CommunityDiscussion` | Forum, discussion |

Conversion methods map from `SourceKind`, `SourceRole`, `ResearchSourceType`, `ResearchSourceClass`, `SecuritySourceTier`.

---

## Evidence Bundle (`src/core/evidence_bundle.rs`)

Deterministic non-summarizing container for multi-agent handoff:

```
EvidenceBundle
  ├── bundle_id: String (FNV-1a, prefix: "bundle_")
  ├── goal: Option<String>
  ├── created_at: String
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

### Caps

| Limit | Default |
|-------|---------|
| `max_sources` | 50 |
| `max_fetched_items` | 20 |
| `max_total_chars` | 100,000 |

### Evidence Gap Kinds (26 variants)

`EvidenceGapKind` includes: `NoPrimarySourceFound`, `ProviderDegraded`, `NativeRepoFilterNotEnforced`, `SecurityApplicabilityUnknown`, `FetchFailed`, `SourceUnfetched`, `AllResultsExternalUntrusted`, `LocalCheckoutDirty`, `LocalRemoteMismatch`, `LocalGeneratedOrVendorOnly`, `LocalUntrackedFile`, `LocalSourceUnfetched`, `NativeAdvisoryUnavailable`, `SymbolHintNoNativeProvider`, `IssueSearchNoNativeProvider`, `ReleaseSearchNoNativeProvider`, `FreshnessNotEnforced`, `PackageResolutionFailed`, `NoFixedVersionFound`, `NoCounterpointFound`, `NoBenchmarksFound`, `MissingTests`, `MissingExamples`, `MissingManifest`, `MissingChangelog`, `MissingSecurityPolicy`.

---

## Evidence Postprocessing (`src/core/evidence_postprocess.rs`)

Phase 5 response integration applied to all result conversion paths:

### Functions

- `assign_evidence_role()` — maps card metadata or source_kind to `EvidenceRole`
- `materialize_evidence_roles()` — populates evidence roles on all source cards
- `compute_evidence_role_summary()` — counts roles, assesses coverage
- `build_retrieval_summary_for_search()` — constructs retrieval summary from provider results
- `build_retrieval_summary_from_attempts()` — constructs from attempt ledger
- `detect_structured_conflicts()` — entity-scoped + mutable-vs-pinned conflicts
- `resolve_workflow_model()` — maps tool/profile/domain to workflow model

### Output

```
EvidencePostprocessResult
  ├── workflow_coverage: Option<WorkflowCoverageResult>
  ├── retrieval_summary: ResponseRetrievalSummary
  ├── conflict_metadata: Vec<EvidenceConflict>
  └── evidence_role_summary: EvidenceRoleSummary
```

---

## Workflow Recipes (`src/core/workflow.rs`)

Machine-readable recipes for agent guidance:

```
AgentWorkflowRecipe
  ├── id: String
  ├── title: String
  ├── goal: String
  ├── suitable_when / avoid_when: Vec<String>
  ├── required_capabilities: Vec<String>
  ├── optional_capabilities: Vec<String>
  ├── steps: Vec<AgentWorkflowStep>
  ├── fallbacks: Vec<AgentWorkflowFallback>
  ├── expected_outputs: Vec<String>
  ├── trust_notes: Vec<String>
  └── support: RecipeSupport
```

Each step includes: order, tool, purpose, input_hints, inspect_fields, next_action_rule, evidence_roles.

`AgentNextAction` (max 5 per response): tool, reason_code, priority (1-5), input_template, source_ids.

Multi-source responses may recommend one focused `batch_fetch` (`fetch_multiple_focused` with per-item `focus`) instead of serial `web_fetch` calls when candidates share safety prerequisites; mixed remote/workspace locators must use separate fetches. Suggested fetches carry `batch_item` for direct batch handoff.

---

## Workflow Coverage (`src/core/workflow_coverage.rs`)

### Workflow Models (10)

| Model | Required Roles | Recommended Roles |
|-------|---------------|-------------------|
| `ApiComprehension` | InterfaceOrApiDefinition, PrimaryImplementation | OfficialDocumentation, UsageExample, TestOrBehavioralSpecification |
| `RepositoryArchitecture` | PrimaryImplementation, ArchitectureOrDesignDocument | OfficialDocumentation, ConfigurationOrFeatureGate, ManifestOrDependencyMetadata |
| `ErrorInvestigation` | IssueOrIncidentDiscussion, PrimaryImplementation | OfficialDocumentation, TestOrBehavioralSpecification |
| `VersionMigration` | ReleaseNoteOrChangelog, MigrationGuidance | OfficialDocumentation, IssueOrIncidentDiscussion |
| `SecurityReview` | AuthoritativeSecurityAdvisory, VendorSecurityGuidance | PrimaryImplementation, ConfigurationOrFeatureGate, ManifestOrDependencyMetadata |
| `DependencyEvaluation` | ManifestOrDependencyMetadata | OfficialDocumentation, ReleaseNoteOrChangelog, AuthoritativeSecurityAdvisory |
| `PerformanceInvestigation` | BenchmarkOrPerformanceEvidence | PrimaryImplementation, OfficialDocumentation, IndependentCorroboration |
| `ComparativeResearch` | OfficialDocumentation, PrimaryImplementation | BenchmarkOrPerformanceEvidence, IndependentCorroboration, CounterpointOrConflictingEvidence |
| `PreChangeEvidence` | PrimaryImplementation, TestOrBehavioralSpecification | OfficialDocumentation, ConfigurationOrFeatureGate |
| `PostChangeReview` | TestOrBehavioralSpecification | PrimaryImplementation, OfficialDocumentation, ConfigurationOrFeatureGate |

### Coverage Status

`CoverageStatus`: `Sufficient`, `UsableWithGaps`, `Insufficient`, `IndeterminateDueToFailures`

### Gap-Driven Next Actions

`generate_gap_driven_next_actions()` produces `AgentNextAction` hints for missing roles.

---

## Conflict Detection (`src/core/conflict.rs`)

### Conflict Classes

| Class | Example |
|-------|---------|
| `DifferingVersionRanges` | Two sources disagree on affected versions |
| `ConflictingReleaseDates` | Different release dates for same event |
| `MutualExclusiveStatusFields` | Contradictory status claims |
| `DivergentBenchmarkNumbers` | Different performance numbers |
| `DocumentationImplementationMismatch` | Docs don't match code |
| `MutableVsCommitPinnedContent` | Mutable URL vs permalink |
| `DifferentProviderMetadata` | Provider disagreement on metadata |
| `Unknown` | Default for unclassified conflicts |

### Severity & Resolution

`ConflictSeverity`: `Critical`, `High`, `Medium`, `Low`, `Informational`

`ConflictResolution`: `PreferCommitPinned`, `PreferAuthoritativeSource`, `PreferNewerDate`, `PreferHigherVersion`, `ManualReviewRequired`, `NoRecommendation`

### Entity-Scoped Detection

`ConflictEntityKey` (entity_type + canonical_id + field) prevents unrelated sources from being compared. Only directly comparable values produce conflicts.

---

## Retrieval Status (`src/core/retrieval_status.rs`)

### Attempt Ledger

`RetrievalAttempt` tracks per-provider outcomes:
- `provider_id`, `subquery_id`, `operation_id`
- `intended_roles` (typed, from planner)
- `outcome`: 10 variants (SuccessWithResults, SuccessZeroResults, Failed, TimedOut, RateLimited, SkippedByPolicy, SkippedCapabilityUnavailable, NotApplicable, InterruptedByDeadline, TruncatedAfterPartialSuccess)
- `result_count`, `error_class`, `truncated`, `truncation_evidence`
- `deadline_interrupted`, `query_fingerprint`, `duration_ms`

### Dimension State

`RetrievalDimensionState` (8 states):
- `Satisfied`, `CompletedNoMatch`, `Failed`, `SkippedByPolicy`
- `CapabilityUnavailable`, `Interrupted`, `Partial`, `NotApplicable`

### Summary Invariants

- `attempted_job_count == completed + failed + policy_skipped + capability_skipped`
- `attempted_dimension_count == completed_dimension_count + failed_dimension_count + not_applicable_count`
- Dimension-only summaries return `None` for job counters

### Debug Validation

`debug_validate_attempt_ledger()` panics in debug/test builds on ledger violations:
- Empty provider IDs
- Duplicate (provider, operation) tuples
- Mismatched result counts
- Duplicate roles within an attempt, zero-result success mismatches
- Deadline flags without interruption, truncation flags without success
- Capability skips with empty role sets (see `AttemptLedgerViolation` for the full set)

---

**Back to:** [overview.md](overview.md)
