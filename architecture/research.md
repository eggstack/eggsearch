# Research Subsystem Deep Dive

**Path:** `src/core/research.rs`, `src/meta/research_planner.rs`, `src/meta/research_workflow.rs`, `src/meta/research_grouping.rs`, `src/meta/research_evidence_analysis.rs`, `src/meta/research_suggested_fetches.rs`, `src/meta/adapter/research.rs`, `src/mcp/tools/research_search.rs`
**Purpose:** Multi-source evidence discovery with typed subquery planning, workflow scaffolding, deterministic grouping, claim/conflict/gap analysis, and ranked fetch suggestions.

---

## Overview

`research_search` fans out one user query into a bounded set of role-annotated
subqueries, aggregates provider results with RRF, groups cards into evidence
kinds, then derives claims, conflicts, source quality, and evidence gaps. The
orchestrator is `MetadataSearchAdapter::research_search`
(`src/meta/adapter/research.rs`); domain policy lives in the typed
`research_*` modules. Shared mechanics (attempt ledger, fetch candidates) come
from `src/meta/workflow.rs` and `src/meta/fetch_ranking.rs` — the research
modules keep their own planner, grouping, and builders and are never flattened
into a generic workflow.

---

## Core Types (`src/core/research.rs`)

`ResearchDomain` (9 variants: `General` default, `SoftwareArchitecture`,
`ApiDesign`, `DistributedSystems`, `Security`, `Performance`,
`LanguageEcosystem`, `MachineLearning`, `Infrastructure`) with
`ResearchDomain::parse()` accepting aliases (`architecture`, `api`,
`distributed`, `ml`, `infra`, `ecosystem`).

`ResearchSourceType` (13 variants): `PrimarySources`, `OfficialDocs`,
`Specifications`, `ReferenceImplementations`, `DesignDiscussions`,
`Benchmarks`, `SecurityConsiderations`, `IssueThreads`, `ReleaseNotes`,
`AcademicOrFormalSources`, `RecentNews`, `CommunityDiscussion`,
`Counterpoints`. `ResearchSourceType::parse()` accepts short aliases (`docs`,
`specs`, `reference`, `issues`, `releases`, `academic`, `news`, `community`,
`counterpoint`, …).

`ResearchWorkflow` (8 variants, default `General`): `ApiEvaluation`,
`LibraryComparison`, `MigrationPlanning`, `SecurityReview`,
`PerformanceInvestigation`, `EcosystemSurvey`, `ArchitectureDecision`,
`General`. `ResearchDepth` (`Quick`, `Standard` default, `Deep`).

`ResearchSearchRequest` carries `query`, `research_domain`,
`desired_source_types` (validated to at most 12 entries),
`include_counterpoints` / `include_primary_sources` /
`include_recent_discussion` / `include_security_considerations`,
`max_results` / `max_groups` / `max_per_group`, `freshness`, `timeout_ms`,
`providers`, plus workflow fields `workflow`, `depth`, `compare_targets`,
`constraints`, `known_context`. Helpers `effective_workflow()` and
`effective_depth()` default to `General` / `Standard`.

`ResearchSearchResponse` carries `query`, `mode`
(`"research_metasearch"`), `research_domain`, `subqueries`, `groups`,
`suggested_fetches`, `providers_queried`, `providers_failed`, `warnings`,
`trust_markers`, optional `workflow_context` and `telemetry`, plus the
analysis block (`claims`, `conflicts`, `source_quality`, `evidence_gaps`) and
the postprocessing block (`workflow_coverage`, `retrieval_summary`,
`conflict_metadata`, `evidence_role_summary`).

---

## Query Planning (`src/meta/research_planner.rs`)

`build_research_search_plan()` returns `ResearchSearchPlan { domain,
subqueries }`:

1. Empty `desired_source_types` falls back to `default_source_types()` — 6
   entries: `PrimarySources`, `OfficialDocs`, `ReferenceImplementations`,
   `DesignDiscussions`, `SecurityConsiderations`, `Counterpoints`.
2. `include_*` flags append their source type when `Some(true)`, without
   duplicating an already-listed type; `dedup()` runs after expansion.
3. More than `MAX_SUBQUERIES` (8) entries are reduced by
   `prioritize_source_types()` using `PRIORITY_ORDER` (`PrimarySources`,
   `OfficialDocs`, `Specifications`, `ReferenceImplementations`,
   `SecurityConsiderations`, `DesignDiscussions`, `Counterpoints`), then
   truncated. Reaching 8 subqueries emits a `subquery_cap_applied` warning
   downstream.
4. Each entry becomes a `ResearchSubquery` with sequential id `rq_{i}`,
   rewritten query, mapped `SearchIntent`, and the request `freshness`.

`intent_for_source_type()` maps source types to `SearchIntent`: docs-ish
types to `Docs`, `ReferenceImplementations` to `Code`, `DesignDiscussions`
and `IssueThreads` to `Issues`, `ReleaseNotes` to `Releases`,
`SecurityConsiderations` to `Security`, `RecentNews` to `News`, and
`Benchmarks` / `CommunityDiscussion` / `Counterpoints` to `Web`.

`build_query_for_source_type()` appends intent-specific suffixes, e.g.
`PrimarySources` → `"official docs source repository maintainer"`,
`Benchmarks` → `"benchmark performance latency throughput comparison"`,
`Counterpoints` → `"drawbacks limitations tradeoffs criticism
alternatives"`, `ReleaseNotes` → `"release notes changelog migration
breaking changes"`.

---

## Semantic Roles

Every planned subquery carries typed intended roles. The adapter maps each
`ResearchSubquery` into a `PlannedSubquery` (`label`, `query`, `priority`
from `research_subquery_priority()`, `intended_roles` from
`intended_roles_for_research_source_type()`, `repo_scope: None`,
`excerpt_count: 0`), where the roles vector is
`EvidenceRole::from_research_source_type(st)`. Roles flow into
`dispatch_subqueries()` and into each `RetrievalAttempt`, so the retrieval
summary can account per evidence role instead of per raw query string.

---

## Depth Control (`src/meta/research_workflow.rs`)

`max_subqueries_for_depth()` bounds total dimension subqueries: `Quick` → 4,
`Standard` → 8, `Deep` → 12. `build_workflow_dimensions()` generates the
deterministic per-workflow `ResearchDimension { name, purpose, source_types,
subqueries }` sets, then drops trailing dimensions (and finally truncates the
last dimension) until the total fits the depth budget.

Dimension builders and their literal names:

- `architecture_decision_dimensions` — 7 dimensions (`Official Docs &
  Specs`, `Reference Implementations`, `Design Discussions`, `Benchmarks &
  Performance`, `Security & Failure Modes`, `Migration & Adoption`,
  `Counterpoints & Tradeoffs`).
- `api_evaluation_dimensions` — 6 (`Official API Documentation`, `Examples
  & Tutorials`, `Source Implementation`, `Issues & Known Pitfalls`, `Version
  & Release Notes`, `Security & Compatibility`).
- `library_comparison_dimensions(query, compare_targets)` — per-target docs
  subqueries plus `Benchmarks`, `Maintenance & Release Cadence`, `Security
  Advisories`, `Migration & Interoperability`, and a bounded (max 3)
  `Per-Target Deep Dives` dimension when more than one target is given.
- `migration_planning_dimensions` — 5 (`Migration Guides`, `Changelogs &
  Breaking Changes`, `Breaking-Change Issues`, `Before/After Examples`,
  `Security Changes`).
- `security_review_dimensions` — 5 (`Security Advisories`, `Threat
  Modeling`, `Hardening Guides`, `Issue Discussion`, `Community Analysis`).
- `performance_investigation_dimensions` — 4 (`Benchmarks`, `Profiling &
  Optimization`, `Performance Issues`, `Comparative Analysis`).
- `ecosystem_survey_dimensions` — 5 (`Ecosystem Overview`, `Popular
  Libraries`, `Community Sentiment`, `Recent Developments`, `Security
  Landscape`).
- `general_dimensions` — 5 (`Official Documentation`, `Implementation
  Evidence`, `Design Discussions`, `Security Considerations`,
  `Counterpoints`).

`build_interpreted_question()` prefixes the query per workflow
(`LibraryComparison` renders `"Compare a vs b regarding: …"`).
`build_workflow_context()` assembles `ResearchWorkflowContext` (workflow,
interpreted question, dimensions, coverage, gaps, first 5 suggested fetches
as `recommended_next_fetches`, gap-derived warnings).
`build_research_telemetry()` records workflow, depth, dimensions/subqueries
generated, diversity caps applied, coverage gaps, and later the routing
decision plus `CapabilityEnforcementTelemetry::for_research_search()`.

---

## Shared Primitives, Typed Policy

`src/meta/workflow.rs` owns domain-free mechanics shared by repo, research,
and security: `PlannedLane`, `RetrievalAttemptSet` (with `record`,
`extend`, `as_slice`, `From<Vec<RetrievalAttempt>>`), `NormalizedLaneResults`,
`EvidenceGroup<K>`, `CoverageSummary`, `FetchCandidateSet`, and
`WorkflowExecution` (`new(scope)`, `record_attempt` / `extend_attempts`,
`push_cards`, `coverage()`).

Research uses these without adopting domain policy: the adapter wraps
dispatch attempts via `RetrievalAttemptSet::from(dispatch.attempts.clone())`
and passes `attempt_set.as_slice()` to `provider_failures()`,
`build_retrieval_failures()`, and `postprocess()`. Suggested fetches are
built with `FetchCandidateBuilder::from_card()` and ranked through
`rank_and_select()` in `FetchRankMode::Research` mode. Planners
(`build_research_search_plan`, `build_workflow_dimensions`), grouping
(`classify_research_group`, `group_research_results`), and fetch builders
(`generate_research_suggested_fetches`) stay typed per domain — shared
helpers never absorb source-type taxonomy, quality signals, or reason codes.

---

## Result Grouping (`src/meta/research_grouping.rs`)

`classify_research_group()` honors an explicit `ResearchSourceType` hint via
`source_type_to_group_kind()`; otherwise it classifies by `SourceKind` plus
URL/title/snippet heuristics (RFC/spec hosts → `Specifications`, arxiv/acm →
`AcademicOrFormalSources`, benchmark keywords → `Benchmarks`, drawback /
limitation / tradeoff / criticism → `Counterpoints`, PRs and RFC-like issues
→ `DesignDiscussions`, plain issues → `IssueThreads`).

`group_research_results()` delegates to the shared `build_card_groups()`
with `CANONICAL_GROUP_ORDER` (13 kinds plus `Unknown`), per-kind labels from
`research_group_label()`, `max_per_group`, and `max_groups`; each card lands
in exactly one group and empty groups are omitted.

`classify_evidence_quality()` produces `EvidenceQuality` (12 variants:
`OfficialPrimary`, `MaintainerPrimary`, `StandardsOrSpecification`,
`VendorPrimary`, `PackageRegistry`, `AcademicOrFormal`, `BenchmarkOrMeasurement`,
`SecurityAdvisory`, `CommunityDiscussion`, `NewsOrPress`, `BlogOrTutorial`,
`Unknown`) from `SourceKind`, domain priors, and provider id.

---

## Claims, Gaps, Conflicts (`src/meta/research_evidence_analysis.rs`)

`analyze_research_evidence()` orchestrates `compute_source_qualities()`,
`extract_claims()`, `detect_conflicts()`, and `detect_evidence_gaps()`.

- `classify_source_class()` → `ResearchSourceClass` (14 variants:
  `OfficialDocs`, `ReferenceDocs`, `RepositorySource`, `MaintainerIssue`,
  `ReleaseNotes`, `Benchmark`, `Paper`, `StandardSpec`, `SecurityAdvisory`,
  `VendorBlog`, `EngineeringBlog`, `ForumThread`, `NewsArticle`, `Unknown`).
- `classify_quality_signals()` → `ResearchQualitySignal` (12 variants:
  `PrimarySource`, `MaintainedCurrent`, `VersionSpecific`, `CommitPinned`,
  `ReproducibleBenchmark`, `PeerReviewed`, `StandardSpecSource`,
  `MaintainerAuthored`, `StaleSource`, `SecondarySource`, `AnecdotalSource`,
  `MarketingSource`); stale detection uses year patterns in the URL.
- `extract_claims()` (bounded by `MAX_CLAIMS = 10`) emits one claim per
  group with ≥ 2 results; `Counterpoints` groups produce challenge text and
  their ids are appended to the last non-counterpoint claim's
  `conflicting_source_ids`. Confidence comes from the group's quality
  summary via `compute_claim_confidence()`.
- `detect_conflicts()` (bounded by `MAX_CONFLICTS = 5`) emits
  `conflict_counterpoints_0` when a counterpoint group exists, plus
  `conflict_quality_{kind}` entries for groups mixing high-tier
  (`OfficialDocs`, `StandardSpec`, `SecurityAdvisory`) and low-tier
  (`ForumThread`, `NewsArticle`) classes.
- Workflow gaps (`detect_gaps()` → `ResearchGapKind`, 8 variants:
  `NoPrimarySources`, `NoRecentSources`, `NoCounterpoints`,
  `NoImplementationEvidence`, `NoBenchmarks`, `NoSecurityDiscussion`,
  `NoMigrationDocs`, `ProviderCoverageLimited`) are workflow- and
  flag-conditional. Evidence gaps (`detect_evidence_gaps()` → bounded by
  `MAX_GAPS`, `ResearchEvidenceGapKind`, 8 variants: `NoPrimarySource`,
  `NoRecentSource`, `NoBenchmarkSource`, `NoSecuritySource`,
  `NoMigrationChangelog`, `OnlySecondarySources`,
  `ConflictingEvidenceUnresolved`, `VersionContextMissing`) each carry
  `AgentNextAction` follow-ups. Do not mix the two vocabularies.

`compute_coverage()` folds groups into `ResearchCoverage`
(`primary_sources_found`, `official_docs_found`,
`implementation_sources_found`, `benchmark_sources_found`,
`security_sources_found`, `counterpoints_found`, `recent_sources_found`).

---

## Suggested Fetches (`src/meta/research_suggested_fetches.rs`)

`generate_research_suggested_fetches()` takes the first card of each
non-empty group, builds a `FetchCandidate` via `FetchCandidateBuilder`
(`group`, `expected_kind` from `expected_kind_for_group()`,
`recommended_extract_mode` from `recommended_extract_mode_for_group()`
— `None` for `ReferenceImplementations`, `Markdown` otherwise — plus
`source_role` / `evidence_confidence` carried from code evidence), and ranks
with `DiversityConfig { max_per_domain: 2, max_per_group: 0, total_cap: 8 }`.
Output `ResearchSuggestedFetch` entries carry sequential `priority`,
`score`, `rank_reasons`, `information_gain`, `source_class`,
`reason_code` (from `resolve_reason_code()`: `commit_pinned_evidence`,
`primary_advisory`, `official_docs`, `maintainer_source`,
`symbol_hint_match`, or per-group `counterpoint_evidence` /
`primary_source_evidence` / `benchmark_evidence` / `security_evidence` /
`release_notes_evidence` / `suggested_evidence`), and a `batch_item` via
`url_to_batch_web_item()`.

---

## RRF and Postprocessing (`src/meta/adapter/research.rs`)

Dispatch uses `candidate_pool_size(final_max, max_results_cap)` and
`dispatch_subqueries()`; `aggregate_source_cards()` merges engine rows with
RRF (`aggregate_rrf()`, `RRF_K = 60.0`) into sanitized `SourceCard`s. After
grouping and `apply_diversity_caps()`, each group's cards pass through
`materialize_evidence_roles()`, and the full card set plus
`providers_failed`, `queried_ids`, the resolved workflow model (via
`resolve_workflow_model_with_context()` mapping research workflows and
research domains to `WorkflowKind`), and `attempt_set.as_slice()` go through
`evidence_postprocess::postprocess()`, yielding `workflow_coverage`,
`retrieval_summary`, `conflict_metadata`, and `evidence_role_summary`.
Gap-driven next actions come from `generate_gap_driven_next_actions()`; the
tool (`run_research_search`) falls back to `research_search_next_actions()`
only when the adapter produced none. Warnings stay explicit:
`subquery_cap_applied`, `freshness_unenforced`,
`diversity_cap: group … capped …`, `generic_context_untrusted`.

---

**Back to:** [overview.md](overview.md)
