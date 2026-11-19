# M036 — Unified Fetch Repo-Item Budget and Outcome

Status: closing
Date: 2026-11-19
Subsystem: agentic retrieval integrity
Milestone class: invariant (corrective)
Decision authority: ADR-0016 (canonical nine-tool inventory), M029 (unified fetch and legacy
aliases), and adaptive-fetch-routing M002 (batch per-target capability parity). No new ADR: this
restores the batch contract those records already state.

References: `plans/000-long-term-specification.md` §2 (stable MCP tool contract) and §3
(retrieval architecture); `architecture/fetch.md` (batch budget and envelope);
`architecture/codegg-contract.md` (fetch result contract).

## Objective

A `fetch` repo item must obey the same budget and success contract as a web item. The caller
either receives bounded content with truthful truncation, or an item explicitly marked failed.
An item must never report `ok: true` with no content and an error string, and a repository file
that `repo_fetch` can return at default settings must not become unreturnable through `fetch`
merely because its diagnostic JSON is large.

## Evidence

Measured on 2026-11-19 against candidate `55180790c6dc5dbb2ce0d94c186ef88cd7584f48`, using an
all-features debug binary, MCP `stdio`, default `batch_max_total_chars` (50,000; cap 120,000),
and a GitHub raw locator:

1. `fetch` repo item `rust-lang/rust` `src/tools/rustfmt/README.md` with default settings:
   10,574 characters of text, 59,555 characters of serialized JSON, reported
   `chars_returned: 57,274`; the item was **omitted** with
   `error: batch_total_budget_exhausted: item omitted because its response metadata exceeds the
   remaining budget of 50000 of max_total_chars=50000`, `chars_returned: 0`, yet
   `ok: true`. The envelope reported `fetched: 1`, `failed: 0`,
   `telemetry.items_completed: 1`.
2. `fetch` repo item `library/std/src/lib.rs` lines 1-8 with `max_total_chars: 200000`
   (clamped to 120,000): a 66,928-character response, lines 1-8 of 818, text correct, because the
   full-file `document` is included in the item. With the default budget the same item is omitted.
3. Direct `repo_fetch` with the same locator: status 200, lines 1-8 of 818, 114 ms, text correct.
4. Web items are charged `text + title + description + link text`; repo items are charged
   `batch_payload_chars(payload)`, the whole serialized response
   (`src/mcp/tools/batch_fetch.rs`).
5. `truncate_batch_result_to_budget` sets `response: None`, `error: Some(...)`, and
   `truncated: true`, but leaves `ok` untouched. The envelope's `fetched` is
   `results.iter().filter(|r| r.ok).count()`, so an omitted item counts as fetched.
6. No test asserts the item outcome on a budget-omitted repo item.
   `tests/batch_fetch_retrieval.rs::batch_aggregate_truncation_is_explicit` covers web items and
   the aggregate warning string only.
7. A repo item missing the required `repo` field fails at the untagged `FetchItems` boundary with
   `failed to deserialize parameters: data did not match any variant of untagged enum FetchItems`,
   which names no field and offers no repair.

## Scope

In scope:

- the character measure and truncation policy for repo items in the batch envelope;
- the item outcome, aggregate `fetched`/`failed`, telemetry, and warning for an omitted item;
- the item-level validation error for an incomplete repo locator;
- tests and documentation for those contracts.

Out of scope:

- `repo_fetch`'s own response shape, budget, or line/span semantics;
- changing `batch_max_total_chars` or its cap;
- a new tool, argument, or `fetch` schema property;
- browser, PDF, challenge, cache, and workspace authorization behavior;
- re-opening M029's advertised inventory or ADR-0016's tool set.

## Invariants

- The aggregate returned text never exceeds `max_total_chars`, and the cap is never bypassed.
- An item with `response: null` never reports `ok: true`.
- `fetched` and `failed` agree with the per-item `ok` values and with `telemetry`.
- No silent omission: an omitted item carries an error and a structured warning naming the item
  and the budget.
- Result order, deterministic stable IDs, per-item labels, trust labels, and sanitization are
  unchanged.
- `repo_fetch` behavior is unchanged; the change is confined to the unified batch envelope.
- No new dependency, no new comment beyond required doc comments, `cargo fmt` clean, ordinary
  files under 1,600 lines / 80 KB.

## Expected production changes

- `src/mcp/tools/batch_fetch.rs`: the per-item measure, truncate-before-omit policy, and outcome
  accounting.
- `src/mcp/tools/fetch.rs` or `src/mcp/tools/batch_fetch_item.rs` only if the measure needs the
  caller's `response_detail` projection before the budget check.
- `tests/batch_fetch_retrieval.rs`, `tests/research_workflow.rs`, `tests/mcp_projection.rs`:
  regression coverage.

## Compatibility and migration

- No MCP tool, argument, or schema change; the `fetch` envelope keeps its shape.
- `chars_returned` changes meaning for repo items, from serialized JSON to readable text. This is
  a telemetry correction, documented in the changelog, and it can lower reported sizes.
- No cache, configuration, storage, or dependency change. No migration.

## Dependencies

- Hard: M029 (closed), which established the unified `fetch` envelope.
- Interface: adaptive-fetch-routing M002 (closed), which established per-target batch routing.
- Soft: none.

## Work packages

1. **Paper first.** Update the batch-budget section of `architecture/fetch.md` to state one
   returned-character measure for both item types (readable text plus bounded user-visible
   metadata; structural `document` and `lines` must not be charged a second time), the
   truncate-before-omit order, and the rule that an omitted item is failed. This lands before
   code.
2. **One measure.** For repo items, measure the payload with the same text-plus-metadata rule the
   web path uses. Do not charge the structural `document`/`lines` duplication. Document whether
   the measurement happens before or after the caller's `response_detail` projection, and keep
   it deterministic.
3. **Truncate before omit.** Clip a repo item's returned `text` to the remaining budget, keep its
   locator and bounded metadata, and set `truncated: true`. Omit only when even the locator and
   metadata cannot fit; that case is a failure, not a success.
4. **Truthful outcome.** Set `ok: false` on omission, propagate to `failed` and
   `telemetry.items_failed`/`items_completed`, and emit one structured
   `batch_total_budget_exhausted` warning naming the item and the budget. Keep `response: null`.
5. **Field-level repair.** For an incomplete repo item (missing `repo`, `owner`, `path`, or
   `host` where required), return a validation error naming the field and its accepted shape
   instead of the untagged-enum message. Prefer pre-validation over a bespoke deserializer; if only
   a deserializer can preserve the inner error, keep the schema unchanged.
6. **Tests.** Extend `tests/batch_fetch_retrieval.rs` (and `tests/research_workflow.rs` where the
   batch budget is exercised) with:
   - a repo file at the default budget returns its text with `ok: true`;
   - a large repo file at the default budget is either truncated with `ok: true` and
     `truncated: true`, or failed with `ok: false` and an attributable error — never an empty
     success;
   - an omitted item sets `ok: false`, increments `failed`, and carries the warning;
   - `chars_returned` follows the one measure for mixed web and repo items;
   - a missing locator field returns a repair-shaped error;
   - a workspace repo item still returns `local_trusted` content.
7. **Documentation.** `architecture/fetch.md`, `docs/features.md`, `docs/tool-matrix.md` (the
   `fetch` envelope wording), `docs/test-inventory.md`, and a `CHANGELOG.md` entry under the
   existing unreleased section.

## Verification

```bash
cargo test --locked --all-features --test batch_fetch_retrieval
cargo test --locked --all-features --test research_workflow
cargo test --locked --all-features --test mcp_projection
cargo test --locked --all-features --test repo_workflow
cargo test --locked --all-features --test static_guards
make check
make docs-check
./packaging/check-repo-hygiene.sh
```

## Acceptance criteria

- A repository file that `repo_fetch` returns at default settings is returnable through `fetch`
  at the default budget, truncated if necessary, with `ok: true`; if it is omitted, `ok` is
  `false` and the error names the budget.
- `chars_returned` uses one measure for web and repo items and counts only content the caller can
  read.
- `fetched + failed` agrees with the item states, and `telemetry.items_completed`/
  `items_failed` agree with them.
- An omitted or truncated item is visible in a structured warning, never only in a loss of
  content.
- `max_total_chars` is still respected across mixed requests.
- A repo item missing a locator field produces a field-level validation error.
- `repo_fetch`, the advertised tool surface, and configuration defaults are unchanged.
- The broad gate passes with 0 failures.

## Stop conditions

Stop and record rather than working around any of these:

- the only viable fix raises the default `batch_max_total_chars` (a shipped default change needs
  its own measured acceptance);
- the fix requires changing `repo_fetch`'s response shape or the `fetch` schema;
- a repo item cannot be measured without consuming the structural document, so truncation would
  silently drop content the caller asked for at `response_detail: diagnostic` — record the
  tradeoff and choose the smaller contract;
- the untagged-enum error cannot be improved without a custom deserializer that risks the item
  schema; defer that sub-item and record it as residual risk.

## Required closure evidence

`plans/closure/agentic-retrieval-integrity/036-status.md` with the exact candidate SHA, a
requirement-to-evidence matrix, the before/after measurements for the three probes above, the
suite results, the broad-gate suite and test counts, the chosen measure and its documentation, and
residual risk (including any deferred field-level error work).
