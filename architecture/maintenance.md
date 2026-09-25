# Maintenance, Ownership, and Extension Guidance

**Purpose:** Keep the post-consolidation repository from re-concentrating into new monoliths. Contributor contract for adding tools, providers, workflows, parsers, and tests.

The crate is application-first: the stable contract is MCP tools (10) plus CLI. The Rust module tree is an implementation detail for the binary, integration tests, and fuzz harnesses. Dependency flow: `core <- meta <- mcp <- commands`; `fetch` is independent of `meta`. Factual claims about tools and providers must agree with the code; inventories below are guard-enforced, the rest is kept in sync by discipline.

---

## Ownership boundaries

Guard-enforced by `tests/static_guards.rs` (fail-closed). Transports call tool seams; tools call the `MetadataSearchAdapter`, never engines directly.

| Area | Owner module | Must not own |
|------|--------------|--------------|
| Tool validation + response shape | `src/mcp/tools/<tool>.rs` + shared `common.rs`; goal/workflow resolution in `canonical.rs` | Engine dispatch, RRF, workflow policy |
| Transport lifecycle | `src/mcp/server.rs`, `src/mcp/http.rs` | Domain orchestration (`build_*_plan`, `dispatch_subqueries`, `aggregate_rrf`, `convert_aggregated`, evidence analysis) — call tool seams instead |
| Search orchestration | `src/meta/adapter/` (invocation, advisory, status, web/repo/research/security execution, normalization, builders) | Engine-specific HTTP parsing |
| Upstream parsing | `src/meta/engines/<provider>.rs` via `EngineSearchRequest` | Workflow policy, role assignment, coverage |
| Shared workflow mechanics | `src/meta/workflow.rs` (`PlannedLane`, `WorkflowExecution`, `RetrievalAttemptSet`, `FetchCandidateSet`) + `fetch_ranking.rs` (`FetchCandidateBuilder`) | Typed domain semantics — `repo`/`research`/`security` keep their own planners, grouping, suggested-fetch builders on the shared primitives |
| Bounded dispatch | `src/meta/dispatch/` (`types` owns job/config/output types + capability partition; `execution` owns the bounded concurrent executor) | Engine-specific parsing, workflow policy, role derivation from `rq_` labels |
| Dependency parsing | `src/meta/dependency_parse/` (`mod` owns normalized `parse_dependency_file` dispatch; `cargo`/`npm`/`go`/`python`/`ruby`/`composer`/`maven`/`dotnet`/`containers`/`github_actions` own one ecosystem each) | Path/size/root validation, applicability policy |
| Local workspace | `src/meta/local/` facade + `local_backend.rs` (search orchestration), `local_inventory.rs` (git discovery/identity), `local_inventory_cache.rs` (cache + git runner), `local_symbols.rs` (structured parsing), `local_ignore.rs`, `safe_open.rs` | Cross-owner logic; single cache abstraction only |
| Forge access | `src/meta/forge_adapter.rs` (execution + shared safety owner: base-URL validation, address classification, bounded reads via `read_bounded_body`/`ForgeReadBudget`, redirect rejection) | Duplicated SSRF/credential/redirect policy in per-host code |
| Evidence packaging | `src/meta/evidence_bundle.rs` (`build_evidence_bundle`: dedup, linking, caps, trust/provider summaries, gaps) | Ranking math and candidate ordering (owned by `fetch_ranking.rs`) |
| Fetch ranking | `src/meta/fetch_ranking.rs` (`FetchCandidateBuilder`, `rank_and_select`, scoring) | Domain source semantics and bundle packaging |
| Fetch execution | `src/fetch/` (client, limits, cache, origin, browser) | Search ranking, evidence roles |
| Outbound HTTP transport | `eggfetch-core` (transport, pooling, TLS, decompression, deadlines, bounded reads, pinned routing, redirect mechanics, typed failures); optional `src/fetch/egress.rs` (`EggressDialer` beneath the eggfetch `Dialer` seam, provider-only, fail-closed, credential-indirected); rmcp transitively owns only its Streamable HTTP client transport | eggsearch-local reqwest clients or facades; SSRF/retry/truncation policy stays in eggsearch; dynamic `FetchClient` targets never use egress |
| Release/deploy surface | `src/platform.rs`, `src/update.rs`, `src/startup.rs`, `packaging/` | Search/fetch policy |

Overlap ratchets (each a named guard):

- `workflows_consume_shared_primitives` — domain suggested-fetch builders must use `FetchCandidateBuilder`; domain adapters must record via `RetrievalAttemptSet`.
- `evidence_bundle_owns_packaging_not_ranking` — bundle never owns ranking math (`FetchCandidateBuilder`, `rank_and_select`, `score_candidate`, `FetchRankReason`); ranking never owns bundle semantics (`build_evidence_bundle`, `EvidenceBundle`, `compute_gaps`).
- `canonical_helpers_have_single_owner` — `canonical.rs` owns `parse_repo_goal`, `parse_research_goal`, `parse_security_goal`, `resolve_repo_semantics`, `resolve_research_workflow`, `resolve_security_workflow`; domain tools call through `canonical::`, never duplicate.
- `suggested_fetch_group_helpers_stay_typed` — `recommended_extract_mode_for_group` look-alikes in research/security stay typed per domain (`ResearchResultGroupKind` vs `SecurityResultGroupKind`); over-deduplication across distinct group taxonomies is forbidden.
- `compatibility_dead_code_inventory` — `#[allow(dead_code)]` capped at 45 total, restricted to the documented file inventory (engines protocol fallbacks, startup platform policy, retrieval-state helpers, dispatch internals, local test helpers). New annotations outside the inventory fail; each retained path carries an exit condition (required compatibility, test-only, feature-gated, or removable).
- Dispatch layout: `src/meta/dispatch.rs` must stay decomposed (`mod` + `types` + `execution`); `types` owns `DispatchJob`/`DispatchConfig`/`DispatchOutput` but never `dispatch_parallel`; `execution` owns the executor. Dispatch never derives roles from `rq_` labels — roles come from `PlannedSubquery.intended_roles`; capability dispositions `PartiallySupported`/`Unsupported` map to `SkippedCapabilityUnavailable` with partial roles preserved.
- Dependency layout: `src/meta/dependency_parse.rs` must stay decomposed; `mod` owns `parse_dependency_file`, never `parse_cargo_lock`/`parse_go_mod`/`parse_pom_xml`.
- Tool/adapter layout: `src/mcp/tools.rs` and `src/meta/adapter.rs` must stay decomposed into directories; `src/mcp/tools/mod.rs` and `src/meta/adapter/mod.rs` must exist; `tests/integration.rs` must stay partitioned into behavioral suites.

---

## Extension rules

### New MCP tools

Dedicated module under `src/mcp/tools/` with shared validation in `common.rs` and goal/workflow resolution in `canonical.rs`. Register in `src/mcp/server.rs` via `#[tool]` attrs. Guard `stable_tool_registration_count_and_names` requires exactly these ten:

`web_search`, `web_fetch`, `batch_fetch`, `provider_status`, `repo_search`, `repo_fetch`, `repo_map`, `security_search`, `research_search`, `build_evidence_bundle`

Count changes only when the tool-matrix, docs contract tests (`tests/docs_tool_names.rs`, derived from `server.rs`), and CodeGG integration docs move in the same change. Legacy `local_search` and `search_and_fetch` must never return.

### New engines and providers

- Engines implement `SearchEngine::search(&EngineSearchRequest)` with defaulted advisory methods. Unsupported capabilities stay explicit so dispatch emits capability-skip attempts instead of silent omissions.
- Providers declare the 24-flag `ProviderCapabilities`, add the ID to `KNOWN_PROVIDER_IDS`, document native-vs-local enforcement in `docs/provider-setup.md`, and extend `tests/provider_capability_contract.rs`.
- Native enforcement today: `brave_api` natively enforces safe-search, freshness/date-range, language, region, news; `exa` natively enforces freshness/date-range, domain filters, result timestamps; `tavily` natively enforces safe-search, freshness/date-range, language, region, domain filters, news.
- Domain filters are natively enforced only by providers advertising `supports_domain_filters` (currently `exa`, `tavily`); all other domain filtering is local approximation.
- Advisory outcomes stay provider-scoped and error-visible: no silent `if let Ok` around `lookup_advisory`/`query_advisories_by_package`; aggregate and scoped lookups surface `provider_id`, `CapabilityUnavailable`, `InterruptedByDeadline`, and `SkippedCapabilityUnavailable` rather than `Err(_) => continue`.
- Postprocess calls in `repo_search`, `research_search`, and `security_search` pass the workflow model (`workflow_model.as_ref()`), never `None`.

### Domain workflows

`repo`/`research`/`security` share `WorkflowExecution`, `RetrievalAttemptSet`, and `FetchCandidateBuilder` but keep typed planners, grouping, and suggested-fetch builders. Do not flatten into one generic workflow. Retrieval dimensions use the full `RetrievalDimensionState` vocabulary (`Satisfied`, `CompletedNoMatch`, `Failed`, `SkippedByPolicy`, `CapabilityUnavailable`, `Interrupted`, `Partial`, `NotApplicable`); the attempt ledger is validated by public `validate_attempt_ledger` with public `AttemptLedgerViolation`, summarized by public `summarize_retrieval_with_attempts` with `AttemptSummaryCounts`. Never take only `.first()` of `intended_roles` — expand across all roles. Security records two attempts per provider (`ManifestOrDependencyMetadata` plus `AuthoritativeSecurityAdvisory`), never one combined attempt. Native advisory budgets split into `MAX_NATIVE_ADVISORY_IDENTIFIERS` and `MAX_NATIVE_ADVISORY_PROVIDER_OPERATIONS` behind `NativeOperationBudget::reserve_identifier`/`reserve_providers`.

### New local parsers

Implement the bounded `SymbolBackend` seam (`src/meta/local_backend.rs` + `src/meta/local_symbols.rs`); regex remains the fallback. Budgets breach to partial/regex evidence, never failure. No workspace code execution. Git runs only through `run_bounded_command()` with concurrent stdout/stderr draining (threads, never sequential) and process-group kill on timeout/cap breach; `safe_open.rs` never uses path-based `std::fs::read`/`read_to_string`.

---

## File size ratchets

Ordinary source files must stay under 1,600 lines and 80 KB. Larger modules carry explicit ratchet ceilings enforced by `orchestration_module_size_ratchet` — split further rather than growing the module. The tool/adapter file list in `modular_tool_and_adapter_layout` additionally warns at 1,600 lines. Next-slice notes follow the table.

| Module | Max lines | Max bytes | Next slice / justification |
|--------|-----------|-----------|----------------------------|
| `src/meta/dispatch/mod.rs` | 100 | 81,920 | Facade only |
| `src/meta/dispatch/types.rs` | 400 | 81,920 | Types + capability partition |
| `src/meta/dispatch/execution.rs` | 2,300 | 100,000 | Split deadlines/telemetry/merge out of the executor loop; ~1,400 lines are pre-existing inline fault-injection tests |
| `src/meta/forge_adapter.rs` | 3,050 | 101,000 | Host-independent vs host-specific split; safety invariants locked by forge guards |
| `src/meta/local_backend.rs` | 2,700 | 100,000 | Move backend logic under `local/` |
| `src/meta/evidence_bundle.rs` | 2,150 | 81,920 | Owns packaging + gap analysis, never ranking |
| `src/meta/dependency_parse/mod.rs` | 800 | 81,920 | Dispatch + shared XML helpers + corpus tests |
| `src/meta/dependency_parse/cargo.rs` | 400 | 81,920 | One ecosystem per file |
| `src/meta/dependency_parse/npm.rs` | 400 | 81,920 | One ecosystem per file |
| `src/meta/dependency_parse/go.rs` | 400 | 81,920 | One ecosystem per file |
| `src/meta/dependency_parse/python.rs` | 400 | 81,920 | One ecosystem per file |
| `src/meta/dependency_parse/ruby.rs` | 400 | 81,920 | One ecosystem per file |
| `src/meta/dependency_parse/composer.rs` | 400 | 81,920 | One ecosystem per file |
| `src/meta/dependency_parse/maven.rs` | 400 | 81,920 | One ecosystem per file |
| `src/meta/dependency_parse/dotnet.rs` | 400 | 81,920 | One ecosystem per file |
| `src/meta/dependency_parse/containers.rs` | 400 | 81,920 | One ecosystem per file |
| `src/meta/dependency_parse/github_actions.rs` | 400 | 81,920 | One ecosystem per file |
| `src/meta/local_inventory_cache.rs` | 2,000 | 81,920 | Move cache + git runner under `local/`; bounded-execution invariants locked by git guards |
| `src/meta/fetch_ranking.rs` | 1,600 | 81,920 | Ranking owner; ordinary-file ceiling |
| `src/meta/local_inventory.rs` | 1,600 | 81,920 | Ordinary-file ceiling |
| `src/meta/local_symbols.rs` | 1,600 | 81,920 | Ordinary-file ceiling |
| `src/meta/security_search.rs` | 2,300 | 88,000 | Security orchestration with native advisory + applicability pipeline |
| `src/meta/suggested_fetches.rs` | 1,600 | 81,920 | Ordinary-file ceiling |
| `src/meta/provider_diagnostics.rs` | 2,200 | 81,920 | Provider health + routing; capability-skip semantics locked by dispatch guards |

---

## Hygiene

- **No comments** unless explicitly requested. `cargo fmt` required (CI fails on `cargo fmt --check`).
- **Stable IDs are content-derived FNV-1a** (`src/core/identity.rs`). Never random UUIDs; never change ID semantics (breaks corpus regression + cross-tool dedup).
- **Sanitize all untrusted text** through `src/core/sanitize.rs` / `sanitize_field()`.
- **Bound all untrusted I/O:** forge responses only via `read_bounded_body()` / `read_bounded_response()` / `read_with_budget()` under `ForgeReadBudget` (never bare `.text()`/`.bytes()`/`.json()`; `bytes_stream()` only inside those bounded readers plus `read_error_body_preview`); bounded git execution via `run_bounded_command()`.
- **`commit_sha` comes from `resolved_ref`**, not the entry object SHA (`build_entry_urls` suppresses `object_sha` with `let _ = object_sha;` and never passes it to permalink/browser/raw URL builders).
- **`CacheScope::Profile` uses the opaque profile ID**, never the display name. Profile-scoped browser fetches use the profile manager's opaque-ID-resolved `chrome-data` directory and configured runtime values.
- **Invalid explicit browser path is `ExplicitPathInvalid`** — do not fall back to auto-discovery.
- **`integrate` prints by default; mutate only with `--apply`** (atomic, backed up, `eggsearch` entry only, limited to supported client mutation paths). Inspect rendered output first. Never register `target/debug` binaries — require an installed executable or explicit `--executable`. Zed and existing OpenCode JSONC settings are print-only (no safe native editor).
- **Tool/probe inventories are code-derived** (`tests/docs_tool_names.rs` from `src/mcp/server.rs`, `tests/docs_provider_inventory.rs` from `KNOWN_PROVIDER_IDS`, `provider_capability_contract.rs` for native enforcement): never invent tool-like or provider names in docs; keep prose in agreement with the code. Do not generate narrative documentation from code.
- **Hot paths use shared immutable inventory/cache ownership** and score/select candidates once; preserve deterministic tie ordering when changing selectors. Keep owned-return compatibility wrappers at the boundary instead of reintroducing deep copies.
- **Fetch timeout overrides reuse the shared transport** for equal/shorter values and build one widened client for longer values; batch setup remains one adjustment per call.
- **Direct Tokio features stay explicitly qualified** in `Cargo.toml`; rmcp client, child-process, and Streamable HTTP client features remain required for `integrate --apply` verification.
- **Provider lists resolve through `resolve_providers()`** (validates enabled/known status); never hardcode. Missing credentials are provider-scoped skips, never global failures.
- **Updater and installer fallbacks stay narrow:** Cargo only for unsupported hosts or confirmed exact-asset HTTP 404; checksum, transport, identity, version, and candidate-identity failures are hard stops. Crates.io `max_stable_version` is the authority, never GitHub `latest`. Never supervise client-owned stdio; startup managers apply to persistent `mcp serve` only.

---

## Keep-in-sync lists

Kept in sync by discipline (not guards) plus `make packaging-check`:

- Packaging set: `packaging/release-targets.txt` + `packaging/release-inputs.txt` + release workflow + egress qualification matrix (`packaging/check-egress-qualify-contract.sh`, `tests/egress_qualify_contract.rs`, exact set equality with the provider-route construction seam) + installers + updater + install docs. Edit one, check them all.
- Test-inventory set: `docs/test-inventory.md` + `architecture/testing.md` + `skills/eggsearch-dev/SKILL.md` when adding or renaming suites. The SKILL table is representative (74 suites exist); the full per-suite inventory lives in `docs/test-inventory.md`.
- `CHANGELOG.md` entries are append-only history — never rewrite published entries.

Hygiene script (`packaging/check-repo-hygiene.sh`, part of `make check`) rejects tracked transcripts, ANSI dumps at the repo root, tracked build outputs, oversized unexpected root blobs, and editor temp files. Test/corpus fixtures under `tests/` and `docs/` are legitimate and never matched.

---

## Behavioral test naming

Historical `phase<N>_*` suite names are retired; use behavioral suite names. New tests extend the matching suite; new files only for distinct subsystems or specific bug classes.

| Suite | Covers |
|-------|--------|
| `tests/mcp_tools.rs` | MCP tool input validation and response shape |
| `tests/web_search_integration.rs`, `tests/web_fetch_integration.rs` | Web search/fetch contracts (`mock` required) |
| `tests/provider_routing.rs` | Provider selection and routing |
| `tests/provider_probe_conformance.rs` | Shared probe conformance (success/skip/failure/cooldown, descriptor source-of-truth) |
| `tests/repo_workflow.rs`, `tests/research_workflow.rs`, `tests/security_workflow.rs` | Domain workflow contracts |
| `tests/evidence_contract.rs` | Evidence bundle determinism |
| `tests/corpus_runner.rs` | Multi-step workflow regressions |
| `tests/property_*.rs` | Pure-function `proptest` coverage (sanitize, identity, fetch, render, local FS) |
| `tests/dispatch_fault_injection.rs` | Provider failures, timeouts, concurrency |
| `tests/forge_adapter.rs`, adversarial trocar corpora | Forge safety and malformed-input validation |
| `packaging/test-install.sh`, `packaging/test-install.ps1` | Packaging and installer behavior (not Rust suites) |

Unit tests for private functions live at the bottom of the source file. Preserve fixture provenance in file-level doc comments. Always re-run `cargo clippy --all-targets --all-features -- -D warnings` after adding tests. Criterion results are characterization on exact candidates; `make bench-check` is the deterministic compile gate, never a CI threshold.

---

## Plans registry

`plans/` follows the registry convention (`plans/registry.md`, process in `plans/003-planning-process.md`). Check the registry for the active workstream before planning.

- Canonical `000`–`003` (specification, terminology, roadmap, process) stay stable.
- Roadmaps live in `plans/subsystems/`; handoffs in `plans/implementation/<subsystem>/NNN-*.md`; evidence gates in `plans/closure/<subsystem>/NNN-status.md`; history in `plans/archive/`; control surface in `plans/registry.md`.
- Status vocabulary: `proposed`, `ready`, `active`, `blocked`, `closing`, `closed`, `conditionally closed`, `superseded`, `archived`.
- **Do not create unregistered work.** Closed workstreams (phases now under `plans/archive/phase-*.md`) are historical evidence — do not reopen or extend them; register new milestones under the owning subsystem roadmap.
- **Corrective work is a new plan** referencing the original milestone and closure record, never a silent amendment to an archived plan.
- Registry, roadmap status, and closure record for a milestone update in the same closure commit. Qualification is SHA-specific; re-qualify a different eventual release candidate before publication.
- Edit `skills/` only; never edit the `.opencode/skills/` or `.agents/skills/` mirror symlinks directly.

---

## Guard index

`tests/static_guards.rs` fails closed. Grouped here for navigation; the test bodies are authoritative.

- Tool and transport shape: `stable_tool_registration_count_and_names`, `no_direct_domain_orchestration_in_transport_modules`, `modular_tool_and_adapter_layout`.
- Bounded I/O and execution: `no_unbounded_forge_body_reads`, `all_forge_response_paths_bounded`, `forge_has_aggregate_byte_budget_type`, `no_unbounded_git_output`, `git_runner_drains_stdout_before_stderr_concurrently`, `no_path_based_reads_in_safe_open`, `no_object_sha_in_commit_urls`.
- Dispatch and advisory semantics: `capability_dispatch_preserves_partial_roles`, `no_rq_label_role_derivation_in_dispatch`, `no_first_only_intended_roles_conversion`, `no_silent_if_let_ok_around_native_advisory`, `native_advisory_outcomes_are_provider_scoped_and_error_visible`, `no_single_native_advisory_operations_constant`, `native_operation_budget_has_reserve_methods`, `record_package_outcomes_emits_two_attempts_per_provider`.
- Retrieval state shape: `retrieval_dimension_state_has_all_variants`, `validate_attempt_ledger_is_public`, `summarize_retrieval_with_attempts_is_public`, `dimension_status_has_state_field`, `response_summary_has_dimension_count_fields`.
- Decomposition and overlap: `orchestration_module_size_ratchet`, `decomposed_dispatch_layout`, `decomposed_dependency_layout`, `evidence_bundle_owns_packaging_not_ranking`, `canonical_helpers_have_single_owner`, `suggested_fetch_group_helpers_stay_typed`, `workflows_consume_shared_primitives`, `compatibility_dead_code_inventory`.
- Dependency and transport policy: `no_direct_reqwest_in_production_source`, `eggfetch_feature_budget_stays_bounded`, `tokio_feature_policy_stays_explicit`, `timeout_overrides_retain_the_shared_fetch_client`, `fetch_timeout_paths_use_effective_limits_for_request_and_validation`, `html_scrape_engines_use_automatic_decompression`.
- Egress scope: `egress_route_stays_out_of_dynamic_fetch`, `egress_route_stays_out_of_loopback_paths`, `egress_route_stays_out_of_browser_paths`, `egress_feature_budget_stays_bounded`.
- Smoke discipline: `no_process_exit_in_native_smoke_tests`, `no_fallback_mode_in_native_smoke`, `postprocess_called_with_workflow_model_for_non_web_tools`.

---

## Verification for maintenance changes

```bash
make check
make docs-check
make bench-check
```

`make check` is fmt + clippy + no-default-features compile check + all-features tests + hygiene + packaging contract. Routine work stays network-free; live probes and native-forge smokes run only on explicit opt-in targets.
Start at `src/lib.rs` and `architecture/overview.md`; operator docs live in `docs/`.

---

[← Back to Overview](overview.md)
