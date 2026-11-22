# M003 — Engine API Conformance Harness

Status: closed
Closure: `plans/closure/code-duplication-consolidation/003-status.md`
Date: 2026-11-22
Subsystem: code duplication consolidation
Milestone class: infrastructure (test-only)
Baseline: `fb9d6538f8ea2e21e08add43fb3f71ebe6ceaa64` on `feat/auto-20261117-044c4885`
Decision authority: no new ADR. No production code changes in this milestone.

References: `plans/subsystems/code-duplication-consolidation-roadmap.md` §4, §7;
`architecture/engines.md` § Scraper Fidelity (the HTML precedent);
`tests/engine_parse_conformance.rs` (the HTML conformance suite);
`architecture/testing.md` § Placement; `docs/test-inventory.md`.

Dependencies: none. M004 hard-depends on this milestone.

## Objective

Replace the per-engine copies of the same API-engine invariants with one table-driven suite over
the nine forge engines, so a new engine inherits the coverage and the duplicated tests can be
deleted.

## Current evidence

Measured 2026-11-22 on the baseline:

- The nine forge engines (`src/meta/engines/{github,gitlab,gitea}_{code,issues,releases}.rs`) are
  7,493 lines, of which 75–82% is the `#[cfg(test)] mod tests` block:
  `github_issues` 854/1043, `github_releases` 721/888, `gitlab_issues` 691/892,
  `github_code` 656/838, `gitlab_code` 637/815, `gitea_code` 674/843,
  `gitlab_releases` 704/933, `gitea_issues` 482/667, `gitea_releases` 404/574.
- The same invariants are re-tested per engine under names that drift:
  `test_convert_respects_max_results` appears in 13 engine files,
  `test_convert_extracts_results` 11, `test_empty_result_set` and
  `test_max_results_zero_returns_empty` 10 each, `test_successful_response_with_multiple_results`
  10, `test_api_key_sent_in_header`/`test_timeout`/`test_unauthorized_401` 9 each,
  `test_malformed_json`/`test_server_error_500` 8 each, `test_forbidden_403` 7.
- All nine engines expose the identical call surface:
  `pub async fn search(&Client, &str api_key, Option<&str> base_url, &str query, usize, Duration) -> Result<Vec<SearchResult>, EngineError>`.
  That makes one dispatcher viable without touching production code.
- `build_http_client` is `pub` (`src/meta/engines/mod.rs:1268`) and each engine module is `pub`,
  so an integration test can call the engines directly. `httpmock` is already a dev-dependency.
- `tests/engine_parse_conformance.rs` is the precedent for the HTML scrapers: a `Case` table plus
  a fixture directory. This milestone does the same for the API engines but keeps the mocks
  in-code, because the payloads are small and the status cases carry no fixture.

## Invariants that cannot regress

- Every assertion the deleted tests make is either represented in the new table or kept in the
  engine module. The closure record lists the mapping.
- Engine-specific conversion behaviour (text matches, issue labels, release metadata, bot
  filtering, URL validation) stays in the engine module; the shared suite does not water it down.
- No test performs network I/O. `make check` remains keyless and offline.
- No production file changes.

## Work packages

1. **Inventory the deleted assertions.** For each of the nine engines, list every test function
   and mark it `shared`, `engine-specific`, or `obsolete`. Produce the mapping in the plan's
   closure record. This is the guard against coverage loss.
2. **Create `tests/engine_api_conformance.rs`.** Structure:
   - `async fn run(engine: &str, client: &Client, base_url: &str, api_key: &str, query: &str, max_results: usize) -> Result<Vec<SearchResult>, EngineError>`
     with one `match` arm per engine (nine arms, no macro).
   - `struct Case { engine, path, auth_header, auth_value, fixture, expected_len, expected_first_title }`
     for the conversion cases.
   - A `const ENGINES: [&str; 9]` so uniform cases iterate every engine.
3. **Uniform cases over all nine:** `401`, `403`, `500` each produce `EngineError::BadStatus`
   with the same status and engine name; malformed JSON produces `ParseFailed` whose reason
   contains `invalid JSON`; `max_results = 0` returns empty **and issues no request**
   (`mock.assert_hits(0)`); `max_results = 2` against a three-row fixture returns two rows; the
   API key reaches the header the engine documents; a non-http or missing URL row is skipped
   without a panic.
4. **Conversion cases per engine:** one fixture each asserting the first title/URL and the row
   count, using the payload shape the engine actually parses.
5. **Delete the duplicated tests** from the nine modules, batched by family (gitea, github,
   gitlab) so a regression is attributable. Keep `truncate_body`/helper unit tests in one place;
   after M004 the helper is shared.
6. **Register the suite.** Add it to `docs/test-inventory.md` and the `architecture/testing.md`
   placement table.

## Expected production changes

| File | Change |
|---|---|
| `tests/engine_api_conformance.rs` | New suite |
| Nine forge engine modules | Delete the shared-invariant tests |
| `docs/test-inventory.md`, `architecture/testing.md` | Register the suite and its coverage |

No production file changes.

## Non-goals

- Rewriting the engines to use a shared transport (that is M004).
- Consolidating HTML scraper tests (already covered by `engine_parse_conformance.rs`).
- Adding a shared mock server layer for other engine families (registry, advisory, scholarly).
- Changing any engine's parsing, ordering, or conversion rules.

## Compatibility and migration

None. Test-only.

## Verification

```sh
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test engine_api_conformance
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib engines::
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test engine_parse_conformance
RUSTUP_TOOLCHAIN=1.89.0 make check
```

Failing-before evidence: change one expected title in the case table and show the suite fails;
delete one uniform case and show the deleted engine test it replaces no longer exists. Record
both.

## Acceptance and stop conditions

- The nine engines are covered by one table for the shared invariants, with per-engine
  conversion cases.
- The deleted-test inventory maps every removed assertion to a table case or a retained test.
- The engine suite count drops by the deleted tests and `make check` passes.
- **Stop:** if an engine's error mapping differs from the other eight (a different status-to-variant
  rule), do not normalise it silently. Record the exception, keep that engine's test local, and
  ask whether the difference is intentional.

## Required closure evidence

The per-engine deleted/retained mapping, the new suite's case count, the injected-failure runs,
the before/after test totals for the nine modules, and the gate output.
