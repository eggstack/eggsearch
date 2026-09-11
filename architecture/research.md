# Research Subsystem Deep Dive

**Path:** `src/core/research.rs`, `src/meta/research_planner.rs`, `src/meta/research_workflow.rs`, `src/meta/research_grouping.rs`, `src/meta/research_evidence_analysis.rs`, `src/meta/research_suggested_fetches.rs`
**Purpose:** Research-oriented multi-source evidence discovery with claims, conflicts, gaps, and depth control.

---

## Overview

The research subsystem provides structured evidence gathering for comparative analysis, library evaluation, architecture decisions, and migration planning. It goes beyond simple search by producing research claims, detecting conflicts between sources, identifying evidence gaps, and computing workflow coverage.

---

## Core Types (`src/core/research.rs`)

### Research Domain

`ResearchDomain` classifies the subject area:
- `General`, `SoftwareArchitecture`, `ApiDesign`, `DistributedSystems`
- `Security`, `Performance`, `LanguageEcosystem`, `MachineLearning`, `Infrastructure`

### Research Source Types

`ResearchSourceType` (13 variants) maps to specific evidence roles:
- `PrimarySources` — peer-reviewed papers, standards-track documents, formal specs
- `OfficialDocs` — official documentation sites, READMEs
- `Specifications` — specifications and protocol definitions
- `ReferenceImplementations` — canonical codebases
- `DesignDiscussions` — RFCs, ADRs, design discussions
- `Benchmarks` — performance data and measurements
- `SecurityConsiderations` — advisories, CVEs, hardening guides
- `IssueThreads` — issue threads and bug reports
- `ReleaseNotes` — release notes, changelogs, migration guides
- `AcademicOrFormalSources` — theses, formal verification results
- `RecentNews` — news articles and press releases
- `CommunityDiscussion` — forum threads, Stack Overflow
- `Counterpoints` — criticism and alternative viewpoints

### Research Workflows

`ResearchWorkflow` (8 variants) determines which dimensions to probe:
- `General`, `ApiEvaluation`, `LibraryComparison`, `MigrationPlanning`
- `SecurityReview`, `PerformanceInvestigation`, `EcosystemSurvey`, `ArchitectureDecision`

### Research Depth

`ResearchDepth` controls subquery count and breadth (`max_subqueries_for_depth()`):
- `Quick` — 4 subqueries, focused
- `Standard` — 8 subqueries, balanced
- `Deep` — 12 subqueries, comprehensive

### Request/Response

```
ResearchSearchRequest
  ├── query: String
  ├── research_domain: Option<ResearchDomain>
  ├── desired_source_types: Vec<ResearchSourceType>
  ├── include_*: bool flags
  ├── workflow: Option<ResearchWorkflow>
  ├── depth: Option<ResearchDepth>
  ├── compare_targets: Vec<String>
  ├── constraints: Vec<String>
  └── known_context: Option<String>

ResearchSearchResponse
  ├── groups: Vec<ResearchResultGroup>
  ├── suggested_fetches: Vec<ResearchSuggestedFetch>
  ├── workflow_context: Option<ResearchWorkflowContext>
  ├── claims: Vec<ResearchClaim>
  ├── conflicts: Vec<ResearchConflict>
  ├── source_quality: Vec<ResearchSourceQuality>
  ├── evidence_gaps: Vec<ResearchEvidenceGap>
  ├── workflow_coverage: Option<WorkflowCoverageResult>
  ├── retrieval_summary: ResponseRetrievalSummary
  └── conflict_metadata: Vec<EvidenceConflict>
```

---

## Query Planning (`src/meta/research_planner.rs`)

`build_research_search_plan()` generates up to 8 `ResearchSubquery` values:

1. Maps `desired_source_types` to query strings with intent-specific suffixes
2. Assigns priorities based on source type importance
3. Each subquery carries typed `intended_roles` derived from `ResearchSourceType`
4. Generic fallback subquery if no source types specified

Example mapping:
- `PrimarySources` → `"{query} official docs source repository maintainer"`
- `Benchmarks` → `"{query} benchmark performance latency throughput comparison"`
- `Counterpoints` → `"{query} drawbacks limitations tradeoffs criticism alternatives"`

---

## Workflow Scaffolding (`src/meta/research_workflow.rs`)

### Dimension Generation

`build_workflow_dimensions()` creates deterministic `ResearchDimension` sets per workflow.
Dimensions are generated per workflow from source (see `research_workflow.rs`);
examples include "Official API Documentation", "Examples & Tutorials",
"Source Implementation", "Issues & Known Pitfalls", "Version & Release Notes",
and "Security & Compatibility" for API-evaluation-style workflows. The table
below summarizes intent, not literal dimension names — read the source for exact
strings:

| Workflow | Focus |
|----------|-------|
| `ApiEvaluation` | API design, documentation quality, community adoption |
| `LibraryComparison` | Feature parity, performance, maintenance status |
| `ArchitectureDecision` | Trade-offs, constraints, precedent |
| `SecurityReview` | Vulnerability history, security practices |
| `PerformanceInvestigation` | Benchmarks, profiling data, optimization guidance |
| `EcosystemSurvey` | Package landscape, maturity, alternatives |
| `MigrationPlanning` | Breaking changes, upgrade path, compatibility |
| `General` | Broad evidence across all dimensions |

### Coverage Computation

`compute_coverage()` evaluates grouped results into a `ResearchCoverage` struct
with count fields; workflow-level sufficiency (`Sufficient` / `UsableWithGaps` /
`Insufficient` / `IndeterminateDueToFailures`) is the `WorkflowCoverageResult`
vocabulary shared with `core/workflow_coverage.rs`:
- `Sufficient` — all required roles satisfied
- `UsableWithGaps` — some recommended roles missing
- `Insufficient` — required roles not satisfied
- `IndeterminateDueToFailures` — provider failures prevent assessment

### Gap Detection

Two gap vocabularies — do not mix them:

- Workflow gaps (`detect_gaps()` → `ResearchGapKind`, 8 variants):
  `NoPrimarySources`, `NoRecentSources`, `NoCounterpoints`,
  `NoImplementationEvidence`, `NoBenchmarks`, `NoSecurityDiscussion`,
  `NoMigrationDocs`, `ProviderCoverageLimited`
- Evidence gaps (`detect_evidence_gaps()` → `ResearchEvidenceGapKind`, 8 variants):
  `NoPrimarySource`, `NoRecentSource`, `NoBenchmarkSource`, `NoSecuritySource`,
  `NoMigrationChangelog`, `OnlySecondarySources`,
  `ConflictingEvidenceUnresolved`, `VersionContextMissing`

### Diversity Caps

`apply_diversity_caps()` prevents over-representation:
- Max 2 results per domain (`DiversityConfig { max_per_domain: 2, total_cap: 8 }`)
- Balanced coverage across dimensions
- (There is no per-source-type cap of 3.)

---

## Evidence Analysis (`src/meta/research_evidence_analysis.rs`)

### Claim Extraction

`extract_claims()` (bounded at 10) identifies:
- Text claims from source cards
- Claim type classification (performance, security, compatibility, etc.)
- Confidence level (high/medium/low)
- Supporting and conflicting source IDs
- Missing evidence notes

### Conflict Detection

`detect_conflicts()` finds two conflict shapes (bounded by `MAX_CONFLICTS`):
- Counterpoint groups (sources with opposing positions)
- Quality disagreements (mixed high/low quality tiers within one group)

### Quality Classification

`classify_source_class()` returns `ResearchSourceClass` (14 variants:
`OfficialDocs`, `ReferenceDocs`, `RepositorySource`, `MaintainerIssue`,
`ReleaseNotes`, `Benchmark`, `Paper`, `StandardSpec`, `SecurityAdvisory`,
`VendorBlog`, `EngineeringBlog`, `ForumThread`, `NewsArticle`, `Unknown`).
`classify_quality_signals()` returns `Vec<ResearchQualitySignal>` (12 variants):
`PrimarySource`, `MaintainedCurrent`, `VersionSpecific`, `CommitPinned`,
`ReproducibleBenchmark`, `PeerReviewed`, `StandardSpecSource`,
`MaintainerAuthored`, `StaleSource`, `SecondarySource`, `AnecdotalSource`,
`MarketingSource`.

---

## Result Grouping (`src/meta/research_grouping.rs`)

Groups results into `ResearchResultGroupKind` (14 variants + `Unknown` default):

| Group | Content |
|-------|---------|
| `PrimarySources` | Peer-reviewed papers, standards-track documents |
| `OfficialDocs` | Vendor docs, READMEs |
| `Specifications` | Standards, protocol definitions |
| `ReferenceImplementations` | Canonical codebases |
| `DesignDiscussions` | RFCs, ADRs, design threads |
| `Benchmarks` | Performance data, measurements |
| `SecurityConsiderations` | Advisories, CVEs, hardening guides |
| `IssueThreads` | Issue threads, bug reports |
| `ReleaseNotes` | Release notes, changelogs, migration guides |
| `AcademicOrFormalSources` | Papers, theses, formal results |
| `RecentNews` | News articles, press releases |
| `CommunityDiscussion` | Forum posts, Stack Overflow |
| `Counterpoints` | Contradicting evidence |
| `Unknown` | Unclassified results |

Each group carries `EvidenceQuality` classification.

---

## Suggested Fetches (`src/meta/research_suggested_fetches.rs`)

Generates `ResearchSuggestedFetch` from grouped results using `fetch_ranking` pipeline in `FetchRankMode::Research` mode.

Diversity caps:
- Max 2 per domain
- Max 8 total

Prioritizes:
1. Primary sources over secondary
2. Official docs over blog posts
3. Commit-pinned URLs over mutable content
4. Counterpoint sources (for balanced evidence)

---

## Workflow Context

`ResearchWorkflowContext` includes:
- Active workflow type
- Coverage status and confidence
- Missing required/recommended dimensions
- Next-action hints for gap filling
- Retrieval failure attribution

---

**Back to:** [overview.md](overview.md)
