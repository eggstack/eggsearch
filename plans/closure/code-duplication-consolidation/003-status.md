# M003 Engine API Conformance Harness — Closure Evidence

Status: closed

Candidate: the working tree on `feat/auto-20261117-044c4885` above baseline
`fb9d6538f8ea2e21e08add43fb3f71ebe6ceaa64`, uncommitted because the request did not ask for a
commit. The canonical gate ran on exactly this tree; its output and the diff fingerprint are in
`plans/closure/code-duplication-consolidation/gate.md`.

Source plan: `plans/archive/implementation/code-duplication-consolidation/003-engine-api-conformance-harness.md`

Roadmap: `plans/subsystems/code-duplication-consolidation-roadmap.md` M003

Decision authority: no ADR. Test-only; no production file changed.

## Change

- `tests/engine_api_conformance.rs` (474 lines) is the table-driven suite: one `run`
  dispatcher with nine arms, a nine-row `Case` table, and `const ENGINES: [&str; 9]`.
- Uniform cases over all nine engines: 401/403/500 -> `BadStatus` with the same engine name
  and status; malformed JSON -> `ParseFailed` whose reason contains `invalid JSON`;
  `max_results = 0` -> empty **and** `mock.assert_hits(0)`; a two-result limit over a
  three-row payload; the documented auth header; a non-http/missing URL row skipped without a
  panic; and an empty payload -> empty rows.
- One conversion case per engine asserts the first title/URL and (where present) the first
  snippet, plus the second title under the limit case.
- The duplicated shared-invariant tests were deleted from the nine modules in three batches
  (gitea, github, gitlab). Engine-specific conversion, URL, label, release-metadata, and
  provider-descriptor tests were retained.

## Deleted/retained mapping

| Engine | Tests before -> after | Deleted |
|---|---|---|
| gitea_code | 27 -> 15 | 12 |
| gitea_issues | 17 -> 10 | 7 |
| gitea_releases | 16 -> 11 | 5 |
| github_code | 30 -> 18 | 12 |
| github_issues | 38 -> 26 | 12 |
| github_releases | 33 -> 21 | 12 |
| gitlab_code | 32 -> 20 | 12 |
| gitlab_issues | 30 -> 18 | 12 |
| gitlab_releases | 33 -> 21 | 12 |
| **Total** | **256 -> 160** | **96** |

The twelve shared invariants deleted where present were `test_convert_respects_max_results`,
`test_max_results_zero_returns_empty`, `test_parse_json_response_*`, `test_empty_result_set`,
`test_unauthorized_401`, `test_forbidden_403`, `test_server_error_500`, `test_malformed_json`,
`test_respects_max_results`, `test_successful_response_with_multiple_results`, and
`test_api_key_sent_in_header`. Every engine-specific assertion the modules carried was either
kept locally or represented by the table; no engine differed in its status-to-error mapping.

## Failing-before evidence

Changing the `github_code` `expected_first_title` to `"src/lib.rs - mutated"` produced:

```
test conversion_cases_parse_expected_rows ... FAILED
assertion `left == right` failed: github_code first title
  left: "src/lib.rs - test/repo"
 right: "src/lib.rs - mutated"
test result: FAILED. 0 passed; 1 failed
```

The mutation was restored and the suite passed again.

## Verification

- `cargo test --locked --all-features --test engine_api_conformance`: 7 passed.
- `cargo test --locked --all-features --lib engines::`: 627 passed at this milestone (before
  M004 consolidated the duplicated truncation tests; the final shared-helper count is 602 in the
  M004 record).
- `cargo test --locked --features mock --test engine_parse_conformance`: 6 passed.
- The suite is registered in `docs/test-inventory.md` and the `architecture/testing.md`
  placement table. No production source changed.
