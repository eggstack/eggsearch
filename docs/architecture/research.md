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
- `PrimarySources` — official documentation, specifications
- `OfficialDocs` — vendor documentation
- `Specifications` — standards, RFCs
- `Benchmarks` — performance data
- `Counterpoints` — contradicting evidence
- `CaseStudies` — real-world usage
- `Tutorials` — how-to guides
- `MigrationGuides` — version upgrade paths
- `CommunityDiscussion` — forum/issue discussions
- `AcademicPapers` — peer-reviewed research
- `SecurityAnalysis` — vulnerability assessments
- `EcosystemSurveys` — landscape analysis
- `ArchitectureDecisions` — ADRs, design documents

### Research Workflows

`ResearchWorkflow` (8 variants) determines which dimensions to probe:
- `General`, `ApiEvaluation`, `LibraryComparison`, `MigrationPlanning`
- `SecurityReview`, `PerformanceInvestigation`, `EcosystemSurvey`, `ArchitectureDecision`

### Research Depth

`ResearchDepth` controls subquery count and breadth:
- `Quick` — 3-4 subqueries, focused
- `Standard` — 5-6 subqueries, balanced
- `Deep` — 7-8 subqueries, comprehensive

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
  ├── source_quality: ResearchSourceQuality
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
- `PrimarySources` → `"{query} official documentation specification"`
- `Benchmarks` → `"{query} benchmark performance comparison"`
- `Counterpoints` → `"{query} limitations drawbacks alternative"`

---

## Workflow Scaffolding (`src/meta/research_workflow.rs`)

### Dimension Generation

`build_workflow_dimensions()` creates deterministic `ResearchDimension` sets per workflow:

| Workflow | Required Dimensions |
|----------|-------------------|
| `ApiEvaluation` | API design, documentation quality, community adoption |
| `LibraryComparison` | Feature parity, performance, maintenance status |
| `ArchitectureDecision` | Trade-offs, constraints, precedent |
| `SecurityReview` | Vulnerability history, security practices |
| `PerformanceInvestigation` | Benchmarks, profiling data, optimization guidance |
| `EcosystemSurvey` | Package landscape, maturity, alternatives |
| `MigrationPlanning` | Breaking changes, upgrade path, compatibility |
| `General` | Broad evidence across all dimensions |

### Coverage Computation

`compute_coverage()` evaluates found vs. required dimensions:
- `Sufficient` — all required roles satisfied
- `UsableWithGaps` — some recommended roles missing
- `Insufficient` — required roles not satisfied
- `IndeterminateDueToFailures` — provider failures prevent assessment

### Gap Detection

`detect_gaps()` identifies `ResearchGapKind`:
- `NoPrimarySources`, `NoCounterpoints`, `NoBenchmarks`
- `NoSecurityAnalysis`, `NoRecentSource`, `OnlySecondarySources`
- `ConflictingEvidenceUnresolved`, `VersionContextMissing`
- `NoMigrationChangelog`

### Diversity Caps

`apply_diversity_caps()` prevents over-representation:
- Max 2 results per domain
- Max 3 results per source type
- Balanced coverage across dimensions

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

`detect_conflicts()` finds:
- Counterpoint groups (sources with opposing positions)
- Quality disagreements (different assessments of same topic)
- Version-specific conflicts (different behavior across versions)

### Quality Classification

`classify_source_class()` and `classify_quality_signals()`:
- `PrimarySource`, `SecondarySource`, `AnecdotalSource`, `MarketingSource`
- Quality signals: `maintained_current`, `version_specific`, `commit_pinned`, `reproducible_benchmark`, `peer_reviewed`

---

## Result Grouping (`src/meta/research_grouping.rs`)

Groups results into `ResearchResultGroupKind`:

| Group | Content |
|-------|---------|
| `OfficialDocumentation` | Vendor docs, specifications |
| `BenchmarksAndPerformance` | Performance data, benchmarks |
| `SecurityAnalysis` | Security research, advisories |
| `CommunityDiscussion` | Forum posts, issue discussions |
| `AcademicResearch` | Papers, studies |
| `CaseStudies` | Real-world implementations |
| `MigrationGuidance` | Upgrade paths, changelogs |
| `Counterpoints` | Contradicting evidence |
| `TutorialsAndGuides` | How-to content |
| `EcosystemLandscape` | Package surveys, comparisons |
| `Other` | Unclassified results |

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
