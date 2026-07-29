# Evidence & Workflow Deep Dive

**Path:** `src/core/evidence_bundle.rs`, `src/core/evidence_role.rs`, `src/core/evidence_postprocess.rs`, `src/core/workflow.rs`, `src/core/workflow_coverage.rs`, `src/core/conflict.rs`, `src/core/retrieval_status.rs`
**Purpose:** Evidence packaging, role taxonomy, workflow guidance, conflict detection, and retrieval tracking.

---

## Evidence Roles (`src/core/evidence_role.rs`)

18-variant taxonomy mapping across source kinds, roles, classes, and tiers:

| Role | Meaning |
|------|---------|
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

Conversion methods map from `SourceKind`, `SourceRole`, `ResearchSourceType`, `SecuritySourceTier`.

---

## Evidence Bundle (`src/core/evidence_bundle.rs`)

Deterministic non-summarizing container for multi-agent handoff:

```
EvidenceBundle
  ├── bundle_id: String (FNV-1a, prefix: "bundle_")
  ├── goal: String
  ├── created_at: String
  ├── sources: Vec<EvidenceBundleSource>
  ├── fetched_items: Vec<EvidenceBundleFetchedItem>
  ├── source_links: Vec<EvidenceBundleLink>
  ├── trust_summary: EvidenceTrustSummary
  ├── provider_summary: Vec<EvidenceProviderSummary>
  ├── gaps: Vec<EvidenceGap>
  ├── warnings: Vec<String>
  ├── research_claims: Vec<ResearchClaim>
  ├── research_conflicts: Vec<ResearchConflict>
  └── limits: EvidenceBundleLimits
```

### Caps

| Limit | Default |
|-------|---------|
| `max_sources` | 50 |
| `max_fetched_items` | 20 |
| `max_total_chars` | 100,000 |

### Evidence Gap Kinds (25+)

`EvidenceGapKind` includes: `NoPrimarySource`, `NoSecuritySource`, `NoBenchmarkSource`, `NoRecentSource`, `OnlySecondarySources`, `ConflictingEvidenceUnresolved`, `VersionContextMissing`, `NoMigrationChangelog`, `ProviderFailed`, `ProviderSkipped`, etc.

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
  ├── suitable_when / avoid_when: String
  ├── required_capabilities: Vec<String>
  ├── optional_capabilities: Vec<String>
  ├── steps: Vec<AgentWorkflowStep>
  ├── fallbacks: Vec<AgentWorkflowFallback>
  ├── expected_outputs: Vec<String>
  └── support: RecipeSupport
```

Each step includes: order, tool, purpose, input_hints, inspect_fields, next_action_rule, evidence_roles.

`AgentNextAction` (max 5 per response): tool, reason_code, priority (1-5), input_template, source_ids.

---

## Workflow Coverage (`src/core/workflow_coverage.rs`)

### Workflow Models (10)

| Model | Required Roles | Recommended Roles |
|-------|---------------|-------------------|
| `ApiComprehension` | Interface/ApiDefinition, OfficialDocumentation | UsageExample, PrimaryImplementation |
| `RepositoryArchitecture` | PrimaryImplementation, ArchitectureOrDesignDocument | ConfigurationOrFeatureGate, ManifestOrDependencyMetadata |
| `ErrorInvestigation` | OfficialDocumentation, IssueOrIncidentDiscussion | PrimaryImplementation, CommunityDiscussion |
| `VersionMigration` | MigrationGuidance, ReleaseNoteOrChangelog | PrimaryImplementation, OfficialDocumentation |
| `SecurityReview` | AuthoritativeSecurityAdvisory, PrimaryImplementation | OfficialDocumentation, IndependentCorroboration |
| `DependencyEvaluation` | ManifestOrDependencyMetadata, AuthoritativeSecurityAdvisory | OfficialDocumentation, ReleaseNoteOrChangelog |
| `PerformanceInvestigation` | BenchmarkOrPerformanceEvidence, PrimaryImplementation | OfficialDocumentation, IndependentCorroboration |
| `ComparativeResearch` | CounterpointOrConflictingEvidence, PrimaryImplementation | OfficialDocumentation, BenchmarkOrPerformanceEvidence |
| `PreChangeEvidence` | PrimaryImplementation, TestOrBehavioralSpecification | OfficialDocumentation, ConfigurationOrFeatureGate |
| `PostChangeReview` | TestOrBehavioralSpecification, PrimaryImplementation | OfficialDocumentation, BenchmarkOrPerformanceEvidence |

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

### Dimension State

`RetrievalDimensionState` (8 states):
- `Satisfied`, `CompletedNoMatch`, `Failed`, `SkippedByPolicy`
- `CapabilityUnavailable`, `Interrupted`, `Partial`, `NotApplicable`

### Summary Invariants

- `attempted_job_count == completed + failed + policy_skipped + capability_skipped`
- `attempted_dimension_count == completed_dimension_count + failed_dimension_count + not_applicable_count`
- Dimension-only summaries return `None` for job counters

### Debug Validation

`debug_validate_attempt_ledger()` panics in debug/test builds on:
- Empty provider IDs
- Duplicate (provider, operation) tuples
- Mismatched result counts

---

**Back to:** [overview.md](overview.md)
