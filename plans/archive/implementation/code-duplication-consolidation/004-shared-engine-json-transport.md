# M004 — Shared Engine JSON Transport

Status: closed
Closure: `plans/closure/code-duplication-consolidation/004-status.md`
Date: 2026-11-22
Subsystem: code duplication consolidation
Milestone class: infrastructure
Baseline: `fb9d6538f8ea2e21e08add43fb3f71ebe6ceaa64` on `feat/auto-20261117-044c4885`
Decision authority: no new ADR. `eggfetch-core` transport ownership (ADR-0002) is unchanged;
`reqwest` stays out of production source.

References: `plans/subsystems/code-duplication-consolidation-roadmap.md` §4, §7;
`architecture/engines.md` § Engine contract; `src/meta/engines/mod.rs` (`read_bounded_body`,
`engine_timeout`, `map_request_error`); `tests/engine_api_conformance.rs`.

Dependencies: hard M003 (closed). The conformance suite is the equivalence proof for this
refactor.

## Objective

Give the nine forge engines one bounded JSON GET helper and one home for their duplicated small
helpers, so each engine owns only its request parameters and its `convert` function.

## Current evidence

Measured 2026-11-22 on the baseline:

- The send → status → `EngineError::BadStatus` → bounded body → parse block appears 24 times
  across the engine tree; in the forge family it is identical apart from the URL, the query
  parameters, and the auth headers. Compare `src/meta/engines/github_issues.rs:60`–`:103` with
  `src/meta/engines/gitlab_issues.rs:60`–`:103` and `src/meta/engines/gitea_code.rs:60`–`:100`.
- `truncate_body` is byte-identical in eight engine files:
  `gitlab_issues`, `gitea_issues`, `gitlab_code`, `gitlab_releases`, `osv`, `gitea_releases`,
  `github_issues`, `github_releases`. It is a two-line wrapper over
  `core::sanitize::truncate_at_word`.
- `parse_owner_repo_from_url` is byte-identical in `gitlab_issues.rs` and `gitlab_releases.rs`.
- `param` is byte-identical in five engines (`pubmed`, `arxiv`, `serpapi`,
  `github_repositories`, `hn_algolia`).
- All nine forge engines already share `read_bounded_body`, `engine_timeout`, and
  `map_request_error` from `src/meta/engines/mod.rs`; the missing piece is the sequence around
  them.
- Every engine's body cap is `2 * 1024 * 1024` bytes and every error carries the engine name, so
  the helper must take both as parameters and preserve the existing reason text.

## Invariants that cannot regress

- `EngineError::Http`, `BadStatus`, and `ParseFailed` keep their variants, engine names, and
  reason wording; `tests/engine_api_conformance.rs` asserts variant and text.
- Bounded reads stay bounded: the helper reads through `read_bounded_body` with the caller's cap;
  no bare `.json()`/`.text()`/`.bytes()` is introduced.
- One timeout policy: `engine_timeout(timeout)` on every request.
- The nine `pub async fn search(...)` signatures do not change, so the M003 dispatcher and any
  downstream call keep compiling.
- No new dependency; no `reqwest`; `src/meta` keeps using `eggfetch-core`.

## Work packages

1. **Add `src/meta/engines/api.rs`** with one helper:
   ```rust
   pub(crate) async fn get_json_bounded<T: serde::de::DeserializeOwned>(
       client: &eggfetch_core::Client,
       engine: &'static str,
       url: &str,
       query: &[(&str, &str)],
       headers: &[(&str, &str)],
       max_body: usize,
       timeout: std::time::Duration,
   ) -> Result<T, EngineError>
   ```
   It builds the GET, applies query and headers, applies `engine_timeout`, maps the request
   error with the engine name, rejects non-success with `BadStatus`, reads through
   `read_bounded_body`, and maps a parse failure to `ParseFailed { engine, reason: format!("invalid JSON: {e}") }`.
   Declare it in `src/meta/engines/mod.rs` beside `read_bounded_body`.
2. **Move `truncate_body`** into `api.rs` (or `mod.rs` if a non-forge engine also needs it) and
   delete the eight copies. Keep behaviour identical: `core::sanitize::truncate_at_word`.
3. **Share `parse_owner_repo_from_url`** for the GitLab pair, and `param` for the five engines
   that build query strings, only if the helper is still byte-identical after the move; otherwise
   leave the copies and record why.
4. **Port the forge family in three batches** — gitea, then github, then gitlab. For each engine
   the `search` body becomes: build query parameters, build auth/value headers, call
   `get_json_bounded`, call `convert`. Keep the `max_results == 0` early return in the engine
   (it avoids a request and the conformance suite asserts zero hits).
5. **Run the conformance suite after each batch** and record the per-batch result. Any failure is
   fixed in the helper, not by relaxing the suite.
6. **Record the deferred families.** Registry, advisory, scholarly, and keyless API engines keep
   their current shape in this milestone. Note the remaining identical blocks in the closure
   record so a future milestone can decide with evidence.

## Expected production changes

| File | Change |
|---|---|
| `src/meta/engines/api.rs` | New bounded JSON GET helper and shared small helpers |
| `src/meta/engines/mod.rs` | Module declaration and re-export of the helpers |
| Nine forge engine modules | `search` reduced to parameters, transport call, and `convert` |

No behaviour change; no schema, tool, or configuration change.

## Non-goals

- Porting the HTML scrapers, whose request shape and challenge handling differ.
- Porting the registry, advisory, scholarly, or keyless API engines in this milestone.
- Changing cancellation, retry, or timeout policy.
- Introducing a trait object or a generic engine builder.

## Compatibility and migration

None. The error contract and the public `search` functions are unchanged.

## Verification

```sh
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test engine_api_conformance
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib engines::
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test dispatch_fault_injection
RUSTUP_TOOLCHAIN=1.89.0 make check
```

Equivalence evidence: the conformance suite and the remaining engine unit tests pass unchanged
after each batch; the error-variant-and-text cases are the specific proof that the helper did not
reword an error.

## Acceptance and stop conditions

- The nine forge engines use one transport helper; the duplicated block is gone from them.
- The conformance suite and the engine unit tests pass without an assertion change.
- No new dependency and no unbounded read.
- **Stop:** if an engine needs a request shape the helper cannot express without a closure over
  the builder (for example a GraphQL POST), leave that engine as it is, record the exception, and
  do not widen the helper into a request-builder abstraction in this milestone.

## Required closure evidence

The per-batch conformance output, the before/after line counts for the nine engines, the
grep showing the duplicated block and helper copies are gone, the error-text assertions, and the
list of API engines deferred to a future milestone.
