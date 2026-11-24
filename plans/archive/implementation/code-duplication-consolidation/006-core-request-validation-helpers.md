# M006 — Core Request Validation Helpers

Status: closed
Closure: `plans/closure/code-duplication-consolidation/006-status.md`
Date: 2026-11-22
Subsystem: code duplication consolidation
Milestone class: polish
Baseline: `fb9d6538f8ea2e21e08add43fb3f71ebe6ceaa64` on `feat/auto-20261117-044c4885`
Decision authority: no new ADR. Error types and messages are part of the tool contract and do
not change.

References: `plans/subsystems/code-duplication-consolidation-roadmap.md` §4, §7;
`architecture/mcp.md` (validation and repairable errors); `docs/tool-matrix.md`.

Dependencies: none.

## Objective

Put the four repeated bounds checks behind one set of helpers, keeping each request type's error
type, message text, and check ordering.

## Current evidence

Measured 2026-11-22 on the baseline. Four validators repeat the same checks with two error
conventions:

| Validator | Location | Error type | Repeated checks |
|---|---|---|---|
| `WebSearchRequest::validate` | `src/core/query.rs:443` | `CoreError::InvalidQuery` | empty, char cap, `max_results > 0`, `timeout_ms > 0` |
| `ResearchSearchRequest::validate` | `src/core/research.rs:443` | `String` | empty, char cap, `max_results`, `max_groups`, `max_per_group`, `timeout_ms` |
| `RepoSearchRequest::validate` | `src/core/repo_search.rs:362` | `String` | locator/exact-error special cases, char cap, `max_results`, `max_per_group`, `timeout_ms` |
| `SecuritySearchRequest::validate` | `src/core/security.rs:1536` | `String` | strong-identifier special case, char cap, `max_results`, `max_per_group`, `timeout_ms` |

Every MCP handler wraps the result again (`ToolError::Validation(format!("invalid query: {e}"))`),
so the two error types are already flattened at the tool boundary. The check text is the same:
`query must not be empty`, `query must be <= {n} characters`, `{field} must be > 0`.

## Invariants that cannot regress

- Error text is byte-identical to the baseline for every rejected input; the tool boundary keeps
  its `invalid query:` prefix.
- Check ordering is unchanged per validator, so the first reported failure is the same one.
- The special cases stay explicit: `security_search` accepts an empty query with a strong
  identifier; `repo_search` accepts it with a locator, rejects it in exact-error mode, and reads
  the exact-error char cap; `WebSearchRequest` keeps `CoreError::InvalidQuery`.
- No new accepted or rejected input.
- The date-range/domain/language checks stay where they are; they are not bounds helpers.

## Work packages

1. **Add `src/core/validation.rs`** with three helpers returning `Result<(), String>`:
   `require_non_empty(field, value)`, `require_char_limit(field, value, max)`, and
   `require_positive(field, value: Option<usize>)`. Unit-test multi-byte input (the limit is
   `chars().count()`, not bytes), the boundary value, zero, and empty.
2. **Delegate the three `String` validators** at the exact points the checks occur today. Do not
   hoist them above the special cases. Keep the existing `format!` call sites' wording by using
   the helpers' fixed messages.
3. **Delegate `WebSearchRequest`** and map the helper's `String` into
   `CoreError::InvalidQuery` at the call site, preserving its message.
4. **Pin the messages.** Add or extend a unit test asserting the exact text for one empty and one
   oversize rejection per validator, so a future helper edit cannot reword the contract.
5. **Leave `RepoSearchRequest`'s exact-error branch** intact: it selects `max_error_chars` before
   applying the char limit, and that ordering is observable.

## Expected production changes

| File | Change |
|---|---|
| `src/core/validation.rs` | New helpers and unit tests |
| `src/core/query.rs`, `research.rs`, `repo_search.rs`, `security.rs` | Delegate the shared checks |
| `src/core/mod.rs` | Module declaration |

No tool, schema, or configuration change.

## Non-goals

- Unifying the four request types or the two error types.
- Moving validation into the MCP tool layer.
- Sharing the domain/date/language checks.
- Changing any limit, default, or cap.

## Compatibility and migration

None. Rejected inputs and their messages are unchanged.

## Verification

```sh
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib core::query
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib core::research
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib core::repo_search
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib core::security
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test mcp_tools
RUSTUP_TOOLCHAIN=1.89.0 make check
```

Failing-before evidence: change one helper message by a character and show the message-pin test
fails; restore it.

## Acceptance and stop conditions

- Four validators call the shared helpers at their current positions.
- Every validator test passes unchanged; the message-pin test passes.
- `make check` passes and no error string moved.
- **Stop:** if sharing a check would require changing one validator's text, ordering, or error
  type, keep that check local and record it. A polish milestone never rewords a tool error.

## Required closure evidence

The before/after validator line counts, the message-pin test, the multi-byte boundary case, the
unchanged validator test counts, and a statement that no accepted input, error code, or message
changed.
