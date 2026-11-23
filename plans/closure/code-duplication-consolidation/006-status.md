# M006 Core Request Validation Helpers — Closure Evidence

Status: closed

Candidate: the working tree on `feat/auto-20261117-044c4885` above baseline
`fb9d6538f8ea2e21e08add43fb3f71ebe6ceaa64`, uncommitted because the request did not ask for a
commit. The canonical gate ran on exactly this tree; its output and the diff fingerprint are in
`plans/closure/code-duplication-consolidation/gate.md`.

Source plan: `plans/archive/implementation/code-duplication-consolidation/006-core-request-validation-helpers.md`

Roadmap: `plans/subsystems/code-duplication-consolidation-roadmap.md` M006

Decision authority: no ADR. Error types, ordering, and messages are the tool contract and did
not change.

## Change

- `src/core/validation.rs` (56 lines) adds `require_non_empty`, `require_char_limit`
  (counting `chars()`, not bytes), and `require_positive`, each returning
  `Result<(), String>` with the existing message text.
- `WebSearchRequest::validate` delegates all four bounds checks and maps the helper error into
  `CoreError::InvalidQuery`.
- `ResearchSearchRequest::validate`, `RepoSearchRequest::validate`, and
  `SecuritySearchRequest::validate` delegate at the exact points the checks occurred. The
  security strong-identifier empty-query branch, the repo locator/exact-error branches, and the
  repo `max_error_chars` selection before the character limit are unchanged.
- Timeout fields are `Option<u64>` while the helper takes `Option<usize>`; call sites pass
  only the zero/non-zero state, preserving the prior `> 0` decision without narrowing.

## Line counts

| Validator | Before | After |
|---|---:|---:|
| `WebSearchRequest::validate` | 86 | 81 |
| `ResearchSearchRequest::validate` | 24 | 15 |
| `RepoSearchRequest::validate` | 53 | 48 |
| `SecuritySearchRequest::validate` | 31 | 26 |

## Message pins and failing-before evidence

Each validator now asserts the exact empty and oversize text. Changing
`require_char_limit`'s message from `"must be <= {max} characters"` to
`"must be <= {max} characterz"` made
`core::query::tests::validate_rejects_oversized_query` fail; the helper was restored and the
test passed again. The multi-byte boundary case counts `é🦀` as two characters (limit 2
passes, limit 1 rejects).

## Verification

- `cargo test --locked --all-features --lib core::query`: 40 passed.
- `... core::repo_search`: 55 passed.
- `... core::research`: 28 passed.
- `... core::security`: 89 passed.
- `... core::validation`: 3 passed.
- No accepted input, error code, message, ordering, limit, or default changed.
