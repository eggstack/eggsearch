# Metasearch Adapter Deep Dive

**Location:** `src/meta/` (adapter + dispatch + planners + grouping + resolvers)
**Purpose:** Central orchestrator for all search. MCP tools never touch engines directly; everything flows through `MetadataSearchAdapter`.

Related: [overview.md](overview.md) (system map) · [engines.md](engines.md) (providers) · [evidence-workflow.md](evidence-workflow.md) (roles/coverage/conflicts) · [research.md](research.md) · [security.md](security.md) · [core.md](core.md) (stable types) · [mcp.md](mcp.md) (tool wiring).

---

## 1. Adapter pattern: tools never touch engines

`src/meta/mod.rs` re-exports the boundary: `MetadataSearchAdapter`, `build_search_plan`, `build_repo_search_plan`, `build_research_search_plan`, `WebSearchResponse`/`ProviderFailure`, health/telemetry types. Engines (`src/meta/engines/`) implement `SearchEngine::search(&EngineSearchRequest)` and stay behind the adapter.

`src/meta/adapter/mod.rs` owns construction and selection:

- `new()` / `new_with_egress()` builds engines via `adapter/builders.rs`, records `SkippedProvider` entries, computes `searxng_configured` / `api_configured`, and fails closed when zero engines build.
- `from_engines()` (plus `mock`-gated `from_engines_with_sanitize()`) injects test doubles.
- `select_engines()` / `selected_engines()` resolves explicit `providers` lists; unknown IDs are returned for the caller to reject, never silently dropped.
- `provider_ids()`, `health()`, `effective_timeout()` expose routing state. Per-request `timeout_ms` is clamped by the global timeout.
- `PlannedSubquery` (label, query, priority, intended roles, `RepoScope`, `excerpt_count`) is the internal handoff from planners to dispatch.

Behavior splits by file with stable `adapter::X` paths:

| File | Responsibility |
|------|----------------|
| `adapter/web.rs` | `web_search()` single-query fan-out |
| `adapter/repo.rs` | `repo_search()` bundle search + local merge |
| `adapter/research.rs` | `research_search()` multi-source evidence search |
| `adapter/security.rs` | `security_search_subqueries()` generic security lanes |
| `adapter/advisory.rs` | `lookup_advisory_scoped()`, `query_advisories_by_package_scoped()` native advisory ops |
| `adapter/execution.rs` | `dispatch_subqueries()`, failure/warning helpers |
| `adapter/normalization.rs` | RRF, `SourceCard` conversion, reranking, domain filter |
| `adapter/builders.rs` | `build_default_engines()` engine construction |
| `adapter/status.rs` | `record_provider_health()`, `provider_status()` |
| `adapter/error.rs` | `ErrorClass`, `classify()`, skip-code helpers |

Full orchestration for `security_search` (web lanes + advisory synthesis + KEV enrichment) lives in `src/meta/security_search.rs`, which calls back into the adapter paths above.

---

## 2. Canonical flow: plan → dispatch → RRF → SourceCard → evidence

```
MCP tool (src/mcp/tools/) validates request
  → planner builds subqueries (planner / repo_planner / research_planner / error_planner)
  → adapter/execution.rs builds (subquery, provider) jobs
  → dispatch/dispatch_parallel() bounded execution
  → engines return SearchResult lists (+ EngineSearchBatch metadata)
  → adapter/normalization.rs aggregate_rrf() + convert_aggregated()
  → grouping.rs / repo_grouping.rs / research_grouping.rs / security_grouping.rs
  → suggested_fetches* via fetch_ranking.rs pipeline
  → core evidence postprocessing (roles, coverage, conflicts, ledger)
  → response.rs / domain response types back through MCP
```

Partial failure is soft throughout: `ProviderFailure { id, error_class, message }` plus `SearchWarning` entries. The adapter always returns a response; `map_tool_result` in `src/mcp/` decides the MCP envelope.

### Module map

| File(s) | Responsibility |
|---------|----------------|
| `mod.rs` | Crate boundary + re-exports |
| `adapter/` | `MetadataSearchAdapter` split by behavior (table above) |
| `dispatch/` | Bounded `(subquery, provider)` executor |
| `planner.rs`, `repo_planner.rs`, `research_planner.rs`, `error_planner.rs` | Domain query planners (pure) |
| `grouping.rs`, `repo_grouping.rs`, `research_grouping.rs`, `security_grouping.rs` | Shared bucket helper + domain classifiers |
| `workflow.rs` | `PlannedLane`, `WorkflowExecution`, `RetrievalAttemptSet`, `FetchCandidateSet` |
| `fetch_ranking.rs`, `suggested_fetches.rs`, `research_suggested_fetches.rs`, `security_suggested_fetches.rs` | Deterministic fetch-candidate pipeline + domain builders |
| `security_search.rs` | `security_search` orchestration (lanes + advisories + KEV) |
| `forge_adapter.rs`, `package_resolver.rs` | Tree retrieval + registry resolution |
| `probe.rs`, `provider_diagnostics.rs`, `recipe_catalog.rs` | Liveness, health/telemetry, recipes |
| `response.rs` | `WebSearchResponse`, `ProviderFailure` |
| `engines/`, `local_*.rs`, `local/` | Vendored providers; local backend (see [engines.md](engines.md), [local-workspace.md](local-workspace.md)) |

---

## 3. Planners per domain

Each planner turns one tool request into bounded subqueries; dispatch never invents queries.

- `src/meta/planner.rs` — `build_search_plan()` / `SearchPlan`. Parses `RepoQueryHints`, rewrites `generic_query` per `SearchIntent` (code/issues/releases get repo-aware rewrites; web/docs/security/news pass through), populates `provider_queries` overrides, and preserves `date_range`, `include/exclude_domains`, `language`, `region`, `safe_search` for native parameters vs local enforcement.
- `src/meta/repo_planner.rs` — `build_repo_search_plan[_with_package]()` / `RepoSearchPlan` / `RepoSubquery`. Merges explicit fields with query tokens, folds in verified `PackageResolution` (repo URL → owner/repo hints), emits up to 8 labeled lanes (`docs`, `registry`, `source`, `examples`, `issues`, `releases`, `changelog`, …) with `target_groups`.
- `src/meta/research_planner.rs` — `build_research_search_plan()` / `ResearchSearchPlan`. Resolves `ResearchDomain`, expands `desired_source_types` plus `include_*` flags, dedups, caps at 8 via priority order, emits `rq_N` subqueries with per-type intent and freshness.
- `src/meta/error_planner.rs` — `build_error_plan()` / `to_repo_subqueries()`. Exact-error mode: `parse_error_query`, optional redaction, `generate_error_subqueries`, label remap (`exact_phrase` → `error_exact`, `error_code`, `error_package`, `error_docs/issues/releases`). Used by `repo_search` when `RepoSearchMode::ExactError`.

Priorities are assigned at the adapter layer (`repo_subquery_priority`, `research_subquery_priority`, `security_subquery_priority`): lower number runs first; ties break by subquery then provider order.

### Planner input/output contract

| Planner | Consumes | Emits | Cap |
|---------|----------|-------|-----|
| `planner.rs` | `WebSearchRequest` + selected provider IDs | `SearchPlan { generic_query, provider_queries, hints, date_range, domains, language, region, safe_search }` | 1 generic query + per-provider overrides |
| `repo_planner.rs` | `RepoSearchRequest` + optional `PackageResolution` | `RepoSearchPlan { hints, subqueries: RepoSubquery { label, query, target_groups } }` | 8 subqueries |
| `research_planner.rs` | `ResearchSearchRequest` | `ResearchSearchPlan { domain, subqueries: ResearchSubquery { id, source_type, query, intent, freshness } }` | 8 subqueries (`MAX_SUBQUERIES`) |
| `error_planner.rs` | query string + `ExactErrorConfig` | `ErrorPlan { parts, subqueries, warnings }` → `RepoSubquery` list | `max_subqueries` from config |

Planners are pure: no network, no engines, no health writes. Redaction, stack-frame truncation, and missing-signal warnings (`no_exact_phrase`, `no_structured_signals`) are planner output, not dispatch behavior.

---

## 4. Bounded dispatch

`src/meta/dispatch/` is the only parallel executor. `dispatch/types.rs` owns `DispatchJob`, `DispatchConfig`, `DispatchOutput`, `RequestDeadlineStats`, `CapabilityDisposition`, and `partition_roles_for_engine()`. `dispatch/execution.rs` owns `dispatch_parallel()`; `dispatch/mod.rs` is the seam with stable `dispatch::X` re-exports.

- **Priority queue.** Jobs sort by `(priority, subquery_order, provider_order)`. The executor starts eligible jobs in order and rotates blocked ones to the back.
- **Two caps.** `max_concurrent_jobs` bounds global in-flight work; `max_concurrent_per_provider` bounds per-provider pressure. Config defaults live in `DispatchConfig::default()`; the adapter passes `multiquery_concurrency` / `multiquery_provider_concurrency`.
- **Global deadline.** One `overall_deadline` governs the call. Expired jobs return `EngineError::Timeout`; never-started and aborted jobs synthesize `InterruptedByDeadline` attempts. `RequestDeadlineStats` tracks completed / skipped / interrupted / partially completed counts, surfaced as `request_deadline_exceeded` warnings.
- **Panic recovery.** Each task wraps in `catch_unwind`; panics (and `JoinSet` join errors) become `NetworkError { reason: "task panicked during dispatch" }`, classified as `ErrorClass::Panic`, with concurrency counters released via `ActiveTaskInfo`.
- **Deterministic output.** Results, failures, attempts, and `EngineRetrievalMetadata` sort by stable order before returning, so completion order never affects ranking.
- **Capability-partitioned dispatch.** `adapter/execution.rs::dispatch_subqueries()` calls `partition_roles_for_engine()` per job. Supported roles dispatch as `FullySupported` / `PartiallySupported`; unsupported roles dispatch as explicit `Unsupported` jobs that immediately record `SkippedCapabilityUnavailable` (or `NotApplicable`) attempts without network I/O. Unsupported work is therefore visible in the retrieval ledger, never a silent omission.
- **`web_search` exception.** `adapter/web.rs` fans out one query per engine directly with a `JoinSet` (no subquery expansion), but reuses the same timeout, panic-recovery, health, attempt, and `build_retrieval_failures()` conventions.

`EngineSearchRequest` carries `repo_scope` and `excerpt_count` per job; engines answer via `search_batch()` so scope-index metadata (`unindexed_scopes()` → `scope_unindexed` warnings) survives aggregation.

### Dispatch config and error taxonomy

`DispatchConfig { candidate_limit, global_timeout, max_concurrent_jobs, max_concurrent_per_provider }` defaults to 30 / 8 s / 8 / 2; production values come from `AppConfig` (`multiquery_concurrency`, `multiquery_provider_concurrency`) and the per-call `candidate_pool_size()` / `effective_timeout()`. Concurrency floors at 1 to avoid deadlock.

`adapter/error.rs::ErrorClass` is the wire taxonomy: `timeout`, `http_status`, `parse_error`, `network_error`, `rate_limited` (HTTP 429 only), `panic` (dispatch panic string), `unknown` (unsupported). `classify()` maps `EngineError` variants; `provider_diagnostics.rs::FailureClass` mirrors it for health recording. Human warnings always pair the class with the message (`[rate_limited] …`); `ProviderFailure` carries the same pair machine-readably.

---

## 5. Adapter execution paths

- **Web (`adapter/web.rs`).** Candidate pool sizing (`candidate_pool_size`: 3× final, clamped by cap; doubled when domain filters are present), per-engine `EngineSearchRequest::from_web_request()`, JoinSet fan-out, health + attempt recording, excerpt clearing when demand is zero, `aggregate_rrf`, `convert_aggregated`, local domain post-filter, `apply_intent_reranking`, truncate, capability-enforcement telemetry + warnings, `materialize_evidence_roles()` + `postprocess()`.
- **Repo (`adapter/repo.rs`).** Package resolution first (bounded client, deterministic fallback on failure), then either the error planner or `build_repo_search_plan_with_package()`. Multi-query `dispatch_subqueries()` with `RepoScope`, `RetrievalAttemptSet` capture, local-workspace merge (budget `local_result_budget`, inventory match, dirty-state warnings, +50 score boost for matched checkouts), `group_results_with_hints()`, optional exact-error rerank, `generate_suggested_fetches()`, capability/deadline/package warnings, role materialization, workflow-model resolution, `postprocess()`.
- **Research (`adapter/research.rs`).** Plan → prioritized dispatch with precomputed `intended_roles_for_research_source_type()` → `aggregate_source_cards()` → `group_research_results()` → `apply_diversity_caps()` → `generate_research_suggested_fetches()` → workflow context + telemetry + `analyze_research_evidence()` (claims/conflicts/quality/gaps) → `postprocess()`.
- **Security (`adapter/security.rs` + `security_search.rs`).** `security_search_subqueries()` runs `advisory` / `vendor` / `defensive` lanes through the shared dispatcher and returns cards + warnings + failures + attempts. `security_search.rs` adds native advisory lookups under `NativeOperationBudget` (32 identifiers / 64 provider ops), KEV enrichment, applicability assessment, severity filtering, grouping, and suggested fetches.
- **Advisory (`adapter/advisory.rs`).** `NativeAdvisoryOperation::{LookupById, QueryByPackage}` executes once per selected provider with per-provider capability gating (`CapabilityUnavailable`) and deadline interruption (`InterruptedByDeadline`). Aggregators return the first non-empty success and preserve the first error otherwise.
- **Status (`adapter/status.rs`).** `record_provider_health()` runs after dispatch (success + classified failure + never-responded timeout). `provider_status()` reports all `KNOWN_PROVIDER_IDS` with enabled/default/configured/routable flags and stable `ProviderSkipCode`s.

Shared execution helpers (`adapter/execution.rs`): `aggregate_source_cards()`, `provider_failures()` (failed only when zero jobs succeeded; capability-skipped-only providers are not failures), `push_failure_warnings()` (partial vs total), `push_deadline_warning()`, `merge_card_trust_markers()`, `any_engine_supports()`.

### Response and warning shapes

- `response.rs::WebSearchResponse { query, mode: "live_metasearch", results, providers_queried, providers_failed, warnings, trust_markers, evidence_postprocess, capability_enforcement }`. Domain responses (`RepoSearchResponse`, `ResearchSearchResponse`, `SecuritySearchResponse` in `src/core/`) mirror it with groups, suggested fetches, telemetry, and coverage fields.
- Capability warnings derive from `CapabilityEnforcementTelemetry`, never ad hoc: `safe_search_unenforced`, `freshness_unenforced`, `date_range_unenforced`, `language_unenforced`, `region_unenforced`, `domain_filters_local`, plus intent-specific `native_*_unavailable` when no selected engine advertises the needed flag.
- Deadline warnings (`request_deadline_exceeded` with interrupted/skipped counts) and `scope_unindexed` warnings are additive; every response also carries the `generic_context_untrusted` marker.

---

## 6. Grouping, RRF dedup, local domain post-filter

`adapter/normalization.rs` owns the shared math; domain groupers own classification:

- `aggregate_rrf()` (RRF_K = 60): sorts engines by ID, normalizes URLs via `engines/normalizer`, accumulates `Σ 1/(60 + rank)`, merges snippets/excerpts (score-ordered, normalized-text dedup, hard caps), keeps the first valid `published_at`, merges structured metadata without letting `ResultMetadata::None` clobber issue/release/advisory rows. Final sort is score desc, then title, then URL (deterministic ties).
- `convert_aggregated()` drops empty/non-HTTP URLs, classifies `SourceKind`/code metadata, assigns deterministic FNV-1a `source_id`, sanitizes title/snippet/excerpts through `sanitize_field()`, attaches `RankReason`s (`RrfMultiProvider`, native-search markers), and computes card quality.
- `candidate_pool_size()` over-fetches (3×) so reranking can promote just-outside-window hits.
- `apply_domain_filters()` runs after aggregation, before truncation: normalize filter domains, drop non-matching hosts. Provider-native filtering stays separate (telemetry); this step is always local approximation (`domain_filters_local` warning).
- `apply_intent_reranking()` adds bounded boosts (≤ +30% of max base score) for intent `SourceKind` priors plus freshness-timestamp matches, then re-sorts stably.
- `grouping.rs::build_card_groups()` buckets by classifier, applies per-kind rerank, emits non-empty groups in canonical order with `max_per_group` / `max_groups` caps and quality summaries.
- `repo_grouping.rs::classify_group()` maps `SourceKind` + code-evidence roles + URL heuristics to `RepoResultGroupKind` (docs, registry, repository, readme, examples, tests, source files, issues, PRs, releases, changelog, discussion, other).
- `research_grouping.rs` classifies evidence quality tiers and groups by research source class.
- `security_grouping.rs::classify_security_result()` groups by advisory authority (authoritative, vendor, package, KEV, patches, exploits, defensive guidance, general context).

### Canonical group orders

Each domain grouper emits non-empty groups in a fixed canonical order (plus per-group `max_per_group` truncation and quality summaries):

- Repo (`repo_grouping.rs`): OfficialDocs → PackageRegistry → Repository → Readme → Examples → Tests → SourceFiles → Issues → PullRequests → Releases → MigrationNotes → Changelog → CommunityDiscussion → Other.
- Research (`research_grouping.rs`): PrimarySources → OfficialDocs → Specifications → ReferenceImplementations → DesignDiscussions → Benchmarks → SecurityConsiderations → IssueThreads → ReleaseNotes → AcademicOrFormalSources → RecentNews → CommunityDiscussion → Counterpoints → Unknown.
- Security (`security_grouping.rs`): AuthoritativeAdvisories → VendorAdvisories → PackageAdvisories → KevEntries → PatchCommitsOrReleases → ExploitDiscussion → DefensiveGuidance → GeneralContext → Other.

Research adds `apply_diversity_caps()` after grouping; repo adds exact-error reranking inside groups when in exact-error mode.

### Excerpts and timestamps

Excerpts are opt-in: `excerpt_count == 0` clears provider passages before aggregation so default cards stay discovery-only; nonzero demand merges them score-ordered with normalized-text dedup (primary snippet included in the dedup set, `MAX_EXCERPTS_PER_CARD` / `MAX_EXCERPT_CHARS` / `MAX_EXCERPT_TOTAL_CHARS` caps, 500/1200-char sanitize bounds). Timestamps are always preserved: generic `published_at` wins, specialist issue/release dates are fallback, first valid timestamp in sorted engine order survives the merge, and freshness reranking consumes it without touching stable IDs.

---

## 7. Evidence postprocessing

Postprocessing lives in `src/core/` (`evidence_postprocess`, `workflow_coverage`, `retrieval_status`, `evidence_role`) and is invoked by every adapter path after grouping:

1. `materialize_evidence_roles()` assigns per-card semantic roles.
2. `build_retrieval_failures()` converts attempts (preferred) or provider failures into `RetrievalFailure` records.
3. `postprocess()` computes workflow coverage against the resolved model, retrieval summary, conflict metadata, and role summary.
4. Callers add `generate_gap_driven_next_actions()` for gap-driven follow-ups.

The ledger (`RetrievalAttempt { provider_id, subquery_id, operation_id, intended_roles, outcome, result_count, error_class, deadline_interrupted, truncation_evidence, query_fingerprint, duration_ms }`) records success, zero-results, timeout, rate-limit, failure, capability-skip, and deadline-interrupted outcomes. Candidate-limit saturation is `LimitReachedUnknown`, never a truncation claim. Trust flows alongside: per-card `TrustMarkers` merge into response-level aggregates; Tier 1 sanitization is always on, Tiers 2–3 gated by `sanitize_output`.

### Security orchestration stages (`security_search.rs`)

Beyond the generic lanes, `security_search.rs` runs: identifier extraction (CVE/GHSA/package/ecosystem/version, capped by `NativeOperationBudget`), native advisory fan-out via `query_advisories_by_package_scoped()` / `lookup_advisory_scoped()`, KEV enrichment (`KevClient`), applicability assessment (affected/fixed ranges via `advisory_range.rs`, version comparison via `version_compare.rs`), severity filtering, `group_security_results()`, `generate_security_suggested_fetches()` (including synthetic advisory candidates), and the shared `postprocess()` tail. Every native provider call is budget-reserved before execution; exhausted budgets produce explicit skip warnings, not silent drops. Detail lives in [security.md](security.md).

### Research workflow and telemetry (`research_workflow.rs`)

`build_workflow_context()` maps groups and suggested fetches onto workflow dimensions and gaps for the requested `ResearchWorkflow` (API evaluation, architecture decision, security review, performance investigation, migration planning); `build_research_telemetry()` records dimensions, subquery count, diversity warnings, and gaps. Research-domain hints (`research_domain`) resolve to workflow-model keys (`architecture_decision`, `security_review`, `performance_investigation`) for coverage resolution. Detail lives in [research.md](research.md).

`LocalWorkspaceBackend` results join remote cards after RRF, before grouping: local budget is half the effective max (min 1), matches convert via `to_source_cards()`, repo-matched cards get a +50 score boost, and timeout/truncation/dirty-state surface as `local_workspace` warnings. `local_workspace` is appended to `providers_queried` only when actually queried. Backend internals live in [local-workspace.md](local-workspace.md).

### Health routing semantics

Health is advisory, never authoritative: `ProviderHealthRegistry` records successes, latencies, and classified failures per provider, and `ProviderRoutingDecision` may deprioritize flapping providers in profile/default routing — but an explicit `providers` list always routes as requested. Probes update health without overriding selection. Cooldown state is process-local; restarts reset it. Consumers needing a routability snapshot use `provider_status()` descriptors (`enabled`, `configured`, `routable`, `skip_code`) rather than reading the registry directly.

### Adjacent modules (pointers, not owned here)

- `repo_mapper.rs` + `forge_adapter.rs` serve `repo_map` (tree listing, important-file/dir classification, map suggested fetches) — structure discovery, not search dispatch.
- `dependency_parse/` (cargo, npm, go, python, ruby, composer, maven, dotnet, containers, github_actions) normalizes lock/manifest files into `DependencyFinding` records for security-adjacent flows.
- `mock.rs` (feature-gated `mock`) provides the test-only engine harness required for integration/corpus suites; plain `cargo test` misses those suites.
- `evidence_bundle.rs::build_evidence_bundle()` is pure bundle-construction logic shared by `build_evidence_bundle` and CLI — covered in [evidence-workflow.md](evidence-workflow.md).

---

## 8. Forge and package resolvers

- `src/meta/forge_adapter.rs` — native tree retrieval for GitHub, GitLab, Gitea/Forgejo, Codeberg without cloning. Entry/depth/byte/pagination/concurrency/timeout limits, bounded reads, endpoint policy (loopback/private/HTTPS), used by `repo_fetch`/`repo_map` walks.
- `src/meta/package_resolver.rs` — `resolve_package()` bounded registry lookups (crates.io, PyPI, npm, Go, Maven, NuGet, RubyGems, Packagist, OCI, GitHub Actions). Returns URLs, versions, and warnings; failures yield deterministic fallback URLs so repo planning can continue. `repo_search` merges verified resolutions into planner hints and optionally attaches a compact security context via advisory query.
- Forge reads use `read_bounded_body()` semantics (never bare `.text()`/`.bytes()`) with a `ForgeReadBudget` aggregate cap; redirects stay disabled at the transport. Resolver lookups use a 10 s default timeout, overridable per call from the request deadline.

### Suggested-fetch ranking signals

`fetch_ranking.rs::FetchRankReason` scores candidates deterministically (no network, no ML): URL stability (pinned raw permalink > pinned browser permalink > mutable raw > mutable browser > generic web), code-evidence confidence (exact > strong > weak > unknown, sparse-evidence penalty), source role/kind (implementation, docs, readme, example, test, changelog, migration, benchmark, advisory, issue, PR, release), hint matches (symbol, path, language, file, package, error context, version/migration), and domain authority (authoritative advisory, vendor advisory, primary research source, reference implementation, benchmark, security consideration). `rank_and_select()` with `DiversityConfig` keeps the top set diverse across groups instead of letting one repo dominate.

---

## 9. Probe, diagnostics, recipes

- `src/meta/probe.rs` — shared liveness service (`ProviderProbeRequest`/`Outcome`/`Summary`, `PROBE_QUERY = "test"`, 1 result, 5 s per-provider / 20 s aggregate deadlines, max 4 concurrent, bounded sanitized messages). Used by CLI `doctor --probe`, MCP `provider_status(probe=true)`, and live-smoke so all three share outcome semantics. Failures update health; skips use stable codes; credentials never echo.
- `src/meta/provider_diagnostics.rs` — `ProviderHealthRegistry` (process-local success/latency/failure ledger), snapshots, routing decisions (advisory only, never overrides explicit selection), and `CapabilityEnforcementTelemetry` (requested vs natively enforced vs locally approximated capabilities — the source of truth behind `*_unenforced` warnings).
- `src/meta/recipe_catalog.rs` — eight canonical workflow recipes with `Available` / `Partial` / `Unavailable` gating from provider descriptors and `local_enabled`. Capability constants (`CAP_GENERIC_SEARCH`, `CAP_CODE_SEARCH`, `CAP_ISSUE_SEARCH`, `CAP_RELEASE_SEARCH`, `CAP_SECURITY_SEARCH`, `CAP_LOCAL_WORKSPACE`, `CAP_REPO_FILTER`, `CAP_EXPLICIT_FETCH`) map recipes to the same `ProviderCapabilities` flags engines declare, so recipe support and dispatch capability partitions cannot disagree.

### Probe outcome contract

`ProviderProbeOutcome { provider_id, attempted, routable, success, failure_class, latency_ms, http_status, skip_code, message }`: `attempted=false` carries a stable `ProviderSkipCode` (missing key, missing SearXNG config, disabled, unknown); `attempted=true` carries a classified `failure_class` and bounded (256-char) sanitized message. `ProviderProbeSummary { requested, implemented, … }` tells harnesses whether probing ran at all. Health writes from probes use the same `record_failure()` path as dispatch, so `doctor --probe` and live traffic share one health view.

---

## 10. Workflow substrate without flattening policy

`src/meta/workflow.rs` holds mechanics shared by repo, research, and security — not policy:

- `PlannedLane<TPolicy>` (label, query, priority, intended roles, repo scope, excerpt demand, plus a typed domain-policy slot).
- `WorkflowExecution` (per-call scope, attempt ledger, collected cards, coverage summary).
- `RetrievalAttemptSet` (deterministic attempt wrapper feeding existing failure helpers).
- `NormalizedLaneResults`, `EvidenceGroup<K>`, `CoverageSummary`, `FetchCandidateSet` (carry-through containers; dedup and ranking stay in RRF / `fetch_ranking`).

Domain modules keep typed planners, classifiers, builders, and telemetry: repo hints/groups, research source-type taxonomy and diversity caps, security advisory synthesis and severity tiers.

`src/meta/fetch_ranking.rs` is the shared deterministic ranking pipeline: `FetchCandidateBuilder::from_card()` / `new()` (plus `group`, `structured_repo_fetch`, `recommended_extract_mode`), `FetchRankReason` provenance/confidence/role/context signals, `rank_and_select()` with `DiversityConfig`. Domain entry points (`suggested_fetches.rs`, `research_suggested_fetches.rs`, `security_suggested_fetches.rs`, `repo_mapper.rs` via `build_repo_map_suggested_fetches()`) build candidates with domain context and hand ranking to this pipeline.

### Extension rules

- New tools add a module under `src/mcp/tools/` and call adapter methods; they never construct jobs or call engines. New adapter paths go in `adapter/` with shared validation in `execution.rs` / `normalization.rs`.
- New engines implement `SearchEngine::search(&EngineSearchRequest)` and declare `ProviderCapabilities`; unsupported roles stay explicit so dispatch emits skip attempts (see [engines.md](engines.md)).
- Domain workflows reuse `PlannedLane` / `WorkflowExecution` / `RetrievalAttemptSet` / `FetchCandidateBuilder` but keep typed planners, classifiers, and builders — do not flatten repo/research/security policy into one generic workflow.
- `response.rs` and domain response types evolve additively; removal breaks corpus regression. Stable IDs stay content-derived FNV-1a (`src/core/identity.rs`).

---

[← Back to Overview](overview.md) · [Engines →](engines.md) · [Evidence & Workflow →](evidence-workflow.md) · [Research →](research.md) · [Security →](security.md) · [Core Types →](core.md) · [MCP Server →](mcp.md)
