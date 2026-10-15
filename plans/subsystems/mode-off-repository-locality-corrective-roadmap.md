# Mode-Off Repository Locality — Corrective Roadmap

Status: closed (M001-M003 closed)
Planning baseline: eggsearch `main` at `90e996ef6c83c969e30c30a36bc9d968a85dc00f` (2026-10-13)
Primary class: invariant / security and runtime correctness

Governance:
- `plans/000-long-term-specification.md` — provider ownership, offline policy, local workspace and bounded tool contracts
- `plans/001-terminology-and-domain-model.md`
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md#7-corrective-passes`
- `plans/adrs/ADR-0001-ten-tool-surface-and-adapter-ownership.md`

Historical predecessor: `plans/subsystems/codegg-legacy-search-parity-corrective-addendum.md`, fully closed by `plans/closure/codegg-legacy-search-parity/005-status.md` and immutable `v0.4.1`. Preserve that closure as accurate historical provider-parity/release evidence; this issue was exposed subsequently by the CodeGG real-process compatibility test.

Downstream reports:
- CodeGG `plans/closure/search-eggsearch-pair-compatibility-corrective/001-status.md` (conditionally closed)
- CodeGG `plans/implementation/search-eggsearch-pair-compatibility-corrective/002-v041-local-only-offline-dispatch-requalification.md` (upstream release v0.4.3 now available; downstream CodeGG paired qualification remains outstanding)
- CodeGG `plans/implementation/search-eggsearch-legacy-retirement/002-builtin-search-backend-retirement.md` (do not delete the old provider backend until requalification)

## 1. Purpose

Restore a strict **no-remote-dispatch** invariant for local-only `repo_search` when `search.mode="off"`. A local request must be served only by a configured, authorized local workspace source. If no usable local source exists or the request cannot be answered locally, it must return a truthful absence, incompleteness, or policy/capability error, **never** a network/provider fallback.

This is an **eggsearch** fix. CodeGG's far-future release readiness, TUI/daemon operation, model inference, or installer publication is unrelated. The sole CodeGG dependency is a deterministic offline paired runtime test against an immutable fixed eggsearch artifact.

## 2. Observed defect and independently verified root cause

CodeGG's C001 exploratory real-process test observed `repo_search(include_local=true)` under `search.mode="off"` dispatch to DuckDuckGo and return remote provider telemetry/results/timeouts. No user credentials were supplied. That observation is documented but is not a synthetic packet-capture proof; further tests must use counting/failing local adapters, not external requests.

At the corrective baseline, upstream `src/mcp/tools/repo_search.rs`:
1. identifies `local_only_path` if Mode Off, a local backend is present, and `include_local != Some(false)`;
2. skips the normal remote-provider routing decision and sets `RepoSearchRequest.providers = Vec::new()`;
3. invokes `state.adapter.repo_search(...)`.

However, `src/meta/adapter/mod.rs::selected_engines(&[])` explicitly means **select all configured search engines**, and `src/meta/adapter/repo.rs::repo_search` calls `selected_engines(&req.providers)` and `dispatch_subqueries`. Hence the empty list used to indicate *no providers* becomes *all providers*. This same code is present on upstream `main` reporting 0.4.2; upgrading blindly from 0.4.1 does not solve it.

A second policy concern exists at the same adapter seam: `src/meta/adapter/repo.rs` invokes `package_resolver::resolve_package` (HTTP) for package-coordinates **before** selecting providers. Therefore merely changing the selected engine vector may still permit outbound resolution. Inspect other indirect network paths (forge adapters, enrichers, fetch/suggested-fetch execution) before declaring the route offline.

## 3. Invariants

- `Mode::Off` denies every remote-provider engine and any independent outbound resolver/fetch on all `repo_search` branches, irrespective of input defaults, provider hints, profile, package coordinates, and failure/empty-result paths.
- Local workspace search can run only when configured, enabled, permitted and requested. `include_local=false` under Mode Off must reject/skip locally rather than route elsewhere.
- **No ambiguity:** empty/omitted provider lists preserve their existing *auto-selection* semantics in normal live mode. Do not globally redefine `selected_engines(&[])`. Introduce an explicit, internal remote-dispatch allowance/selection intent at the MCP-to-adapter seam.
- No external source is marked local or source-trusted because it appeared in a local-only response. Preserve source provenance, trust framing, source IDs, warnings and retrieval-state/absence-vs-failure semantics.
- The ten stable MCP tool names, current `repo_search` model-visible arguments and existing CodeGG provider mappings remain unchanged unless a separately justified compatibility change is necessary.
- Do not silently allow external I/O for metadata enrichment merely because a local result exists. Never claim transport network containment from a config flag; prove the required *code paths* have zero outbound adapter/resolver calls with instrumented test doubles.
- No added async background jobs, extra provider retries, new caching, mutable global mode flag, or new provider client.

## 4. Milestones and dependency graph

```text
Historical CodeGG legacy-provider parity release (closed, v0.4.1)
                       |
                       v
M001 explicit no-remote-dispatch ownership + instrumented regressions
                       |
                  strict closure
                       v
M003 non-publishing exact-SHA Eggpack qualifier
                       |
       hosted exact-SHA qualification path
                       |
                       v
M002 immutable corrected eggsearch patch release + downstream handoff
                       |
        immutable tag / source SHA / provenance / zero-call tests
                       v
CodeGG paired requalification C002 (separate repo; not executed here)
                       |
                       v
CodeGG legacy builtin backend retirement M002 (separate repo)
```

| Milestone | Status | Plan | Exit evidence |
|---|---|---|---|
| M001 explicit remote permission and offline local repository correctness | **closed** | `plans/implementation/mode-off-repository-locality-corrective/001-explicit-zero-remote-dispatch-boundary.md` | Closed at `cc0c9c7b27e0706a3bab5f5d7cb137e64e9eaa3d`; closure `plans/closure/mode-off-repository-locality-corrective/001-status.md`. Local match, empty-workspace/exact-error, opt-out, two provider counters, injected resolver counter, advisory skip, live routing, full `make check`, and docs pass. |
| M003 non-publishing exact-SHA Eggpack qualifier | **closed** | `plans/implementation/mode-off-repository-locality-corrective/003-prepublication-exact-sha-qualifier.md` | Closed after hosted exact-SHA run `38476463235` passed all seven target builds, qualifications, consumer validators, required gate, and aggregate for candidate `5e7b853e7eebf29c58f84959805c8774279d7ea4`; closure `plans/closure/mode-off-repository-locality-corrective/003-status.md`. |
| M002 patch publication and CodeGG compatibility handoff | **closed** | `plans/implementation/mode-off-repository-locality-corrective/002-immutable-corrected-release-and-handoff.md` | Candidate `5e7b853` passed local gates and hosted exact-SHA qualification. Immutable crate `0.4.3`, tag `v0.4.3`, public 19-asset release, provenance, and release-backed zero-call evidence are recorded in `plans/closure/mode-off-repository-locality-corrective/002-status.md`. CodeGG C002 remains consumer-owned. |

M001 closed at `cc0c9c7b27e0706a3bab5f5d7cb137e64e9eaa3d`; strict zero-dispatch evidence is recorded in its closure. M003 closed after hosted exact-SHA run `38476463235` qualified candidate `5e7b853e7eebf29c58f84959805c8774279d7ea4` on all seven targets with all required validation jobs passing. M002 closed after crate `0.4.3`, tag `v0.4.3`, the public 19-asset release, provenance, and release-backed zero-call evidence were verified. The remaining CodeGG C002 paired qualification and legacy backend retirement are consumer-owned.

## 5. Verification and authority

Tests must have **counting/failing `SearchEngine` doubles** and a separate **counting/failing `package_resolver`/HTTP seam** or equivalent controlled local transport. Tests for offline success must demonstrate *zero executions* in every external path; an empty results array, a quick timeout, `Mode Off` text, an unconfigured API key or no network connectivity do not prove it.

Include a real local workspace fixture and direct MCP `repo_search` trajectory with `search.mode="off"`, alongside no-local-backend, `include_local=false`, explicit provider list, exact-error, package-coordinate and cancellation/deadline cases. Reconfirm live-mode auto-provider routing unaffected. Observe/fix misleading warnings such as `using generic web search` on a path that is strictly local.

M001 closure: `plans/closure/mode-off-repository-locality-corrective/001-status.md`. M002 closure: `plans/closure/mode-off-repository-locality-corrective/002-status.md`. M003 closure: `plans/closure/mode-off-repository-locality-corrective/003-status.md`. Records include exact commands/run results, fixtures, security and cancellation evidence, source/schema compatibility, unresolved findings, and named blockers. Do not overwrite historical CodeGG/eggsearch parity closure records.

## 6. Deferred and stop conditions

Do not use repeated **live** Mode-Off local-only probes on v0.4.1/0.4.2 as evidence. Do not paper over the issue by changing documentation to claim offline only for web searches or by disabling all repository search under Mode Off: this line explicitly retains supported *local* repository search. Do not assume clearing `req.providers` or `selected_engines` alone prevents other network calls.

If M001 reveals a broader fundamental ownership defect beyond repo_search or cannot isolate package resolution without a genuine internal control, stop and register the smallest additional corrective/ADR rather than adding a global command blocker or claiming success.
