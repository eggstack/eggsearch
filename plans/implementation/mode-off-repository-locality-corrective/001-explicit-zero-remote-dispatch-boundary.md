# Mode-Off Repository Locality M001 — Explicit Zero-Remote-Dispatch Boundary

Status: ready
Primary class: invariant / security and correctness
Repository baseline: `eggstack/eggsearch@90e996ef6c83c969e30c30a36bc9d968a85dc00f` (main, 2026-10-13)
Roadmap: `plans/subsystems/mode-off-repository-locality-corrective-roadmap.md`
Normative: `plans/000-long-term-specification.md`, `plans/001-terminology-and-domain-model.md`, `plans/002-long-term-roadmap.md`, `plans/003-planning-process.md#7-corrective-passes`.
Existing ADR: `plans/adrs/ADR-0001-ten-tool-surface-and-adapter-ownership.md`; no new ADR expected if the new execution scope remains private/internal.
Historical closure: `plans/closure/codegg-legacy-search-parity/005-status.md` (do not edit).
Downstream regression evidence: `dbowm91/codegg: plans/closure/search-eggsearch-pair-compatibility-corrective/001-status.md`.
Downstream recovery plan: `dbowm91/codegg: plans/implementation/search-eggsearch-pair-compatibility-corrective/002-v041-local-only-offline-dispatch-requalification.md`.

## 1. Objective

Prove and enforce that **`repo_search` under `search.mode="off"` executes no remote provider engines, package-registry resolvers, remote forge adapters, metadata fetches or other outbound work**, even if `include_local=true`. When the private local workspace backend is explicitly enabled and allowed, it may return scoped, correctly attributed local source cards; otherwise the request must fail or report unfulfilled coverage truthfully. This correctness fix is the upstream precondition for CodeGG's paired-process requalification, not a CodeGG release milestone.

## 2. Why the predecessor suite missed it

At the source baseline:

- `src/mcp/tools/repo_search.rs` permits a local path under `Mode::Off` when `state.local_backend.is_some()` and `include_local != Some(false)`. It creates a provider routing decision with zero selected providers, then writes `req.providers = Vec::new()`.
- `src/meta/adapter/mod.rs::selected_engines(&[])` interprets an empty list as **all configured engines**. `src/meta/adapter/repo.rs::repo_search` calls that method and `dispatch_subqueries`, so the MCP decision *inverts* when it crosses the adapter boundary.
- `src/meta/adapter/repo.rs` independently calls `package_resolver::resolve_package` using an HTTP client when a package coordinate is present, before engine selection. A provider-selector-only patch therefore cannot satisfy the offline contract.
- The CodeGG v0.4.1 exploratory test observed remote DuckDuckGo dispatch/telemetry despite an explicit Mode-Off/private-local request. Existing M001/M005 parity qualification did not include a true remote-engine call counter on this path; earlier paired tests exercised provider-status and policy-denied nonrepo tools.

This is a **failure of dispatch authorization**, not a request to change normal `providers=[]` semantics or to hide the issue by suppressing warnings.

## 3. Invariants and explicit boundaries

1. `search.mode="off"` is a hard deny on external repo-search work. It holds for empty/nonempty explicit providers, `profile`, `goal`, `workflow`, `mode=exact_error`, issue/release/doc sources, package ecosystem/version coordinates, no matches, local error, timeout, cancel and retry.
2. `include_local=false` or no configured/usable local backend under Mode Off must never route remotely; return actionable policy/unavailable status (if the local-only route is unsupported). Do not invent successful absence for unattempted local evidence.
3. Preserve **live mode** provider routing exactly: `providers=[]` still means automatic/default providers, explicit lists still select the correct subset, and all established provider diagnostics remain valid.
4. Deny must flow through to the *actual adapter and every HTTP-producing subpath*. Use an explicit private call-scoped execution policy, such as `RepoRemoteExecution::Denied` vs `Allowed` passed to `MetadataSearchAdapter::repo_search`, or an equivalent typed enum/struct. Do not use empty vectors, magic provider ID sentinels, a mutable global mode flag, unvalidated user JSON fields, or a new MCP tool.
5. Preserve the stable ten-tool names, existing `repo_search` JSON argument schema, response envelope, source IDs/provenance, grouped evidence, structured warnings and trust framing. Local workspace text remains external data/instruction-untrusted.
6. No new provider client, additional network dispatch, cache, async jobs or process-per-call. The fix belongs to eggsearch; CodeGG must not mask the bug by inserting its own provider implementation.

## 4. Production changes and ordered work packages

### WP1 — Census all potential external work before implementation

Trace the full request path:

`src/mcp/tools/repo_search.rs::run_repo_search` → `src/meta/adapter/repo.rs::repo_search` → `src/meta/adapter/mod.rs::selected_engines` → `src/meta/adapter/execution.rs::dispatch_subqueries`, including `package_resolver` HTTP requests, forge adapters, any enrichment, suggestion/fetch **execution** (distinct from inert suggested URLs), and local backend discovery/indexing.

Make a short ownership table for each outbound seam (eg. `SearchEngine::search`, package-resolution HTTP, forge-source lookups): whether called under remote-allowed vs remote-denied, control point, and test observer. Prove local inventory/discovery does not itself initiate network requests.

### WP2 — Implement an explicit internal dispatch authority

The MCP endpoint derives authority from **server configuration policy**, not untrusted request input:
- Mode Off + enabled, permitted local source → `remote_denied` and `local_enabled`.
- Mode Off + local source unavailable/disabled or `include_local=false` → fail closed with policy/unavailable response before adapter/external work.
- Live mode → `remote_allowed`; retain configured routing and optional local results.

Pass this typed, call-scoped selection through the repository adapter seam. Ensure `remote_denied` provides **zero engines** to dispatch and zero remote attempted-provider telemetry without calling `selected_engines(&[])`. Protect the package-coordinate metadata resolver separately: either skip it in remote-denied mode with explicit incomplete-metadata status or refactor to a *non-network* local-only resolver, but never initiate HTTP. Audit source-group assembly, advice, fallback, deadline and package enrichment for other outbound calls. No silent failure substitution.

Place a local helper assertion/test guard by the adapter boundary so subsequent refactors cannot reintroduce empty-means-all leakage. If the generic `MetadataSearchAdapter::repo_search` is also called from Rust outside the MCP endpoint, require its caller to choose or justify authority rather than defaulting to a permissive scope in a Mode-Off context. Any argument-signature change is internal; preserve public MCP schemas.

### WP3 — Honest output/status when no remote work is possible

Keep locally matched cards, stable IDs, source kind, evidence roles, and trust markers. Under remote denial:
- `providers_queried`, `providers_failed`, attempt/dispatch counters and remote provider health updates must indicate **no remote attempts**, never manufactured provider absence.
- Retrieval status and workflow coverage must distinguish **local search completed with zero matches** from **remote sources skipped by policy / unavailable**. Do not claim all evidence roles satisfied from the local-only route. If current summary types are insufficient, use a small additive typed warning/coverage field without changing existing schema or inventing status.
- Remote-specific warnings (for example `using generic web search`) must not claim the fallback occurred; retain appropriate warnings about local scope/limited coverage and untrusted source text.
- The no-remote policy still holds if local backend returns errors, is empty, becomes unavailable, or hits its deadline.

### WP4 — Deterministic injected regression tests

Add test doubles that **count every remote provider/search attempt**, not merely return no results. Reuse the existing `MockEngine` / `SearchEngine` infrastructure, and add a narrow injected HTTP/package-resolution transport spy for the independent resolver. The acceptance suite needs at least:

| Case | Required result and counters |
|---|---|
| Mode Off, `include_local=true`, enabled private workspace with known matching file | Actual local source content/identity only; all outbound engine/HTTP/forge counters **0** |
| Mode Off, enabled local source, no matches | Local-scoped zero match with proper partial/skip coverage; outbound counters **0** |
| Mode Off, `include_local=false` | Policy rejection, **0** calls |
| Mode Off, local backend missing/disabled | Policy/unavailable rejection, **0** calls |
| Mode Off, provider list explicitly specified (including valid default provider) | Remote work still denied; no silently selected provider; **0** calls |
| Mode Off, package ecosystem/name/version coordinate supplied | No package resolver HTTP, no provider engine call; local output or incomplete status |
| Mode Off, exact-error / specialized goals and sources | Inert remote routing; local-only or policy result, **0** calls |
| Mode Off, local failure / timeout / cancellation | No fallback to remote, **0** calls |
| Live mode, omitted/empty providers | Existing automatic default selection still executes mock engine; expected positive count |
| Live mode, explicitly selected subset | Only requested mock engine is called; unspecified mock engine count **0** |
| Regression mutation: replace remote-denied with empty provider vector in adapter | Instrumented test must fail by observing positive outbound engine call count |

Tests must exercise both an adapter-level path and actual MCP `repo_search` invocation through a fake/counting adapter or local in-process server; a string-only response assertion or a socket-blocked process is not proof. For direct MCP integration, configure Mode Off and a private fixture, with **no live network path reachable** (counting/failing doubles). Keep fixture size/budgets small enough for CI and preserve deterministic ordering.

### WP5 — Defensive static and documentation updates

Clarify `src/mcp/tools/repo_search.rs`, adapter API docs, `architecture/codegg-contract.md`, `docs/tool-matrix.md` and the local backend docs: "local-only" is an **execution authority**, not a synonym for empty provider list. Add the narrowest static guard/contract test needed to catch a future `local_only -> req.providers=[] -> selected_engines([])` reintroduction. Do not rewrite earlier parity/release closure records.

## 5. Verification

Run from the exact implementation SHA; correct command or test names as implemented and record departures:

```bash
cargo fmt --all -- --check
cargo test --locked --lib meta::adapter::tests -- --test-threads=1
cargo test --locked --lib mcp::tools::repo_search -- --test-threads=1
cargo test --locked --test provider_routing -- --test-threads=1
cargo test --locked --test dispatch_fault_injection -- --test-threads=1
cargo test --locked --test static_guards -- --test-threads=1
python3 packaging/check-planning-consistency.py
make check
make docs-check
git diff --check
```

A standalone targeted test for the new Mode-Off path is mandatory regardless of test module placement. Avoid real external network in **all** offline fixtures. The broader `make check` includes all-feature tests/Clippy; note platform/feature skips or environmental blockers rather than marking unrun gates passed.

## 6. Security, lifecycle and migration

This is a transport/policy correction, not an OS network sandbox. Preserve SSRF/egress, secret handling, shared MCP lifecycle and all existing outbound policy enforcement for other tool families. If an optional search feature triggers outbound work in local-only mode, apply the same explicit denial or exclude that route with truthful diagnostic. Cancellation/deadline must never convert policy denial into remote fallback. No durable storage, configuration-format, tool-schema or wire migration is expected; if one becomes unavoidable, document it and stop for review. No publication/tag change in M001.

## 7. Acceptance and stop conditions

**Strict close only when:** outbound provider, package-resolver, forge/metadata and background-call counters are **zero** for the full Mode-Off matrix; valid local matches are returned with local provenance; no-match/failure/policy outcomes are truthful; normal live mode routing unchanged; tests fail under the original empty-means-all bug; no new remote seam remains unmeasured; focused/quick/docs checks pass.

**Stop** if a required outbound seam cannot be instrumented, if any local backend turns on network indirectly, if test-only flags weaken production Mode-Off behavior, or if remediation requires changing public JSON schema/ten-tool contracts without a separate decision. Record a bounded follow-up rather than claiming offline support.

Closure artifact: `plans/closure/mode-off-repository-locality-corrective/001-status.md` with implementation SHA, owned seam census, per-scenario engine/resolver/forge counters, local-fixture evidence, security/status matrix, exact commands and outcomes, no-run disclosure, remaining findings by severity, and M002 readiness disposition. Keep upstream historical closure immutable.
