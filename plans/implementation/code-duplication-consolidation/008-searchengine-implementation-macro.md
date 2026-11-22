# M008 — SearchEngine Implementation Macro

Status: closed
Closure: `plans/closure/code-duplication-consolidation/008-status.md`
Date: 2026-11-22
Subsystem: code duplication consolidation
Milestone class: polish
Baseline: `fb9d6538f8ea2e21e08add43fb3f71ebe6ceaa64` on `feat/auto-20261117-044c4885`
Decision authority: no new ADR. The `SearchEngine` trait and dispatch semantics do not change.

References: `plans/subsystems/code-duplication-consolidation-roadmap.md` §4, §7;
`architecture/engines.md` § Engine contract; `src/meta/engines/mod.rs`.

Dependencies: soft M004 (same directory and import churn). M008 can land without M004, but not
concurrently on the same files.

## Objective

Replace the 39 mechanical `impl SearchEngine for …` blocks in `src/meta/engines/mod.rs` with one
macro and a per-engine call expression, keeping every engine's name and behaviour identical.

## Current evidence

Measured 2026-11-22 on the baseline:

- 39 `impl SearchEngine for X` blocks occupy `src/meta/engines/mod.rs:358`–`1210`, about 853
  lines.
- Each block repeats the same three items: `fn name(&self) -> &'static str`, a
  `fn search<'a>(&'a self, request: &'a EngineSearchRequest) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>>`,
  and `Box::pin(engine::search(&self.client, &request.query, request.max_results, request.timeout, …))`.
- The per-engine call arguments genuinely differ: `renderer`, `challenge`, `api_key`,
  `base_url`, or a fixed endpoint. A macro cannot infer them, so each entry supplies its own
  expression.
- Several engines override trait methods beyond the forwarding shape: advisory lookup
  (`osv`, `github_advisory`, `nvd`, `cisa_kev`, `rustsec`), `search_batch` (specialist
  engines), and `supports_role`. Those impls stay hand-written; the macro covers only the
  forwarding shape.

## Invariants that cannot regress

- Every engine's `name()` string is unchanged; provider routing and diagnostics key on it.
- The dispatch set is unchanged: the same 39 engines are reachable, with the same
  `search_batch`, `supports_role`, `advisory_capabilities`, and advisory overrides.
- `BoxFuture` allocation and `Box::pin` remain (the trait is object-safe and `Send`).
- No engine gains or loses a capability flag, feature gate, or construction requirement.
- No test changes its assertions.

## Work packages

1. **Add `impl_engine!`** in `src/meta/engines/mod.rs`:
   `impl_engine!(DuckDuckGoEngine, "duckduckgo", |engine, request| duckduckgo::search(&engine.client, &request.query, request.max_results, request.timeout));`
   The macro expands to the `name` and `search` methods and a `Box::pin` over the supplied
   expression. Document the macro with one rustdoc example and no inline commentary beyond what
   the repository allows.
2. **Port in five batches**: HTML scrapers (`duckduckgo`, `brave`, `startpage`, `yahoo`,
   `mojeek`, `searxng`), API-key search (`brave_api`, `exa`, `tavily`, `serpapi`, `kagi`,
   `firecrawl_developer`, `sourcegraph`), forges (`github_*`, `gitlab_*`, `gitea_*`), registries
   (`crates_io`, `pypi`, `npm_registry`, `go_pkg`, `maven_central`, `nuget`, `rubygems`,
   `packagist`), and the remainder (`osv`, advisories, scholarly, `wikipedia`, `arxiv`, `pubmed`,
   `hn_algolia`, `github_repositories`). Engines with an override keep a hand-written impl; the
   macro may still cover their forwarding `search` only if a partial impl is not possible — if it
   is not, leave the whole impl manual and record it.
3. **Verify each batch** with the engine unit tests and `provider_routing`; do not batch-verify
   only at the end.
4. **Add a name-set test** if none exists: assert the engine name set produced by the
   constructors equals the baseline list. If an equivalent test exists in
   `src/meta/adapter` or `tests/provider_routing.rs`, cite it instead of adding a duplicate.
5. **Record the line delta** and the count of impls that stayed manual and why.

## Expected production changes

| File | Change |
|---|---|
| `src/meta/engines/mod.rs` | `impl_engine!` plus 39 expanded entries; overrides kept manual |

No other production file should change.

## Non-goals

- Changing the `SearchEngine` trait, its default methods, or `BoxFuture`.
- Replacing dynamic dispatch with an enum.
- Moving engine construction out of `MetaSearchAdapter`.
- Changing any engine's request parameters, parsing, or error mapping.

## Compatibility and migration

None. This is a source-shape change with identical behaviour.

## Verification

```sh
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib engines::
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test provider_routing
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test engine_api_conformance
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test engine_parse_conformance
RUSTUP_TOOLCHAIN=1.89.0 make check
```

Equivalence evidence: the engine test count is unchanged, the name set is unchanged, and the diff
contains only the impl scaffolding, not a call expression's arguments.

## Acceptance and stop conditions

- The forwarding impls exist once; overrides are documented as manual.
- The engine name set and the engine test count are unchanged.
- `make check` passes without an assertion edit.
- **Stop:** if the macro cannot express an engine's call without changing its arguments, keep
  that impl manual. Do not normalise engine call signatures to fit the macro.

## Required closure evidence

The before/after line counts for `mod.rs`, the manual-impl list with reasons, the name-set test
output, and the unchanged engine suite totals.
