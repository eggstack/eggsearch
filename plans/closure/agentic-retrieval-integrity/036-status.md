# M036 Unified Fetch Repo-Item Budget and Outcome — Closure Evidence

Status: closed

Source plan: `plans/archive/implementation/agentic-retrieval-integrity/036-unified-fetch-repo-item-budget-and-outcome.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Decision authority: ADR-0016 (canonical nine-tool inventory), M029 (unified fetch and legacy
aliases), adaptive-fetch-routing M002 (batch per-target capability parity). No new ADR.

## Scope

M036 restores the budget and outcome contract the unified `fetch` envelope already claimed:

1. A repo item was charged its whole serialized response (`batch_payload_chars`), so a
   10,574-character README serialized to 59,555 characters and was omitted at the default
   50,000 budget while `repo_fetch` returned it.
2. `truncate_batch_result_to_budget` replaced the payload with `null` but left `ok: true`,
   so an omitted item counted as fetched.
3. The envelope emitted no item-specific warning for a budget omission.
4. A repo item missing `repo`/`owner`/`path` failed at the untagged `FetchItems` boundary
   with a message that named no field.
5. No test asserted the item outcome for a budget-omitted repo item.

The paper-first contract is in `architecture/fetch.md` § Batch returned-character budget.

## Exact code candidate

Implementation commit `76c1156a2c5418d4b9dab4493eddd9232bdd7b8a` (parent `614634198bfb6f145f7c223d5a330cc8924f677d`). The
pre-commit implementation fingerprint was
`103dbdf22a28f3bed06ff0209fc8fbabacf691be13d76e5bce072bd90fc89c7d`, the SHA-256 of
`git diff -- src tests docs architecture CHANGELOG.md` taken at 2026-11-19T21:21:54Z. Every
command and probe below ran against that tree. This closure commit carries this record, the plan
status, the roadmap status, and the registry rows together.

## Requirement-to-evidence mapping

| # | Acceptance criterion | Evidence |
|---|---|---|
| 1 | One readable-text measure for web and repo items | `batch_returned_chars` and `batch_payload_metadata_chars` in `src/mcp/tools/batch_fetch.rs` count top-level `text`, `title`, `description`, visible link URL/text, and (repo only) string-valued locator and returned-URL fields. `src/mcp/tools/batch_fetch_item.rs` uses that one helper for web and repo items. `architecture/fetch.md` states the rule and that measurement precedes `response_detail` projection. |
| 2 | Truncate before omit | The repo branch of `truncate_batch_result_to_budget` subtracts measured metadata, clips only the top-level `text` UTF-8-safely, and sets `truncated: true`. `tests/batch_fetch_retrieval.rs::batch_repo_large_workspace_file_is_clipped_to_default_total_budget` proves `ok: true`, `truncated: true`, non-empty text, and a bounded total at the default budget. |
| 3 | An omitted item is failed | `omit_batch_result_for_budget` sets `ok: false`, `response: null`, `chars_returned: 0`, `truncated: true`, and an item-specific budget error. `::batch_repo_metadata_budget_omission_is_a_failed_item` asserts exactly those fields plus `fetched: 0`, `failed: 1`, `telemetry.items_completed: 0`, `telemetry.items_failed: 1`. |
| 4 | One structured budget warning, never silent | `budget_warning` is accumulated once per batch and pushed to `warnings`; the envelope converts it to `structured_warnings`. The large-file and omission tests find one `fetch_content_truncated` entry carrying the item label and budget. The raw prefix `batch_total_budget_exhausted` is preserved in `warnings`. |
| 5 | Field-level locator repair | `impl Deserialize for FetchItems` in `src/mcp/tools/fetch.rs` pre-validates each item: a repo item names the missing `owner`/`repo`/`path` and the accepted shape; a web item names the missing `url`. The schema is unchanged. `src/mcp/tools/fetch.rs::fetch_locator_errors_name_the_missing_field` asserts the field name, the accepted shape, and the array index. |
| 6 | Mixed web/repo `chars_returned` uses the measure | `::batch_web_and_workspace_repo_in_one_batch` recomputes the documented measure from each payload and asserts exact equality for the web and repo items. |
| 7 | Workspace trust and content unchanged | The same test asserts `trust: local_trusted` and the workspace text, and `repo_fetch` is untouched. |
| 8 | `repo_fetch`, advertised surface, defaults unchanged | No file under `src/mcp/tools/repo_fetch*` or `src/core/repo_fetch.rs` changed; `tests/repo_workflow.rs` passes 100 tests and `tests/mcp_tool_contract.rs` keeps the nine-tool surface (fingerprint updated only for the additive advisory summary, not for `fetch`). |
| 9 | Broad gate | `make check` exit 0: 85 suites, 6,445 passed, 0 failed, 28 ignored. `make docs-check` exit 0. |

## Live before and after

All probes used `target/debug/eggsearch` built `--locked --all-features` from the candidate, MCP
`stdio`, and the default 50,000-character aggregate budget.

| Probe | Before (review evidence) | After |
|---|---|---|
| `rust-lang/rust` `src/tools/rustfmt/README.md` | 10,574 text / 59,555 serialized JSON; item **omitted** with `ok: true`, `chars_returned: 0`, budget error; envelope `fetched: 1`, `failed: 0` | `ok: true`, `chars_returned: 10,863` (10,574 text + 289 metadata), `truncated: false`, `fetched: 1`, `failed: 0`, no budget warning; 1,772 ms |
| `rust-lang/rust` `library/std/src/lib.rs` lines 1-8 | 66,928-character response; item **omitted** at the default budget | `ok: true`, `chars_returned: 712` (443 text + metadata), `truncated: false`, no error; 169 ms |
| Small workspace repo item | Returned `local_trusted` | Unchanged: `local_trusted`, same content, measured chars |

The second probe makes the chosen tradeoff visible: the structural `document` still travels in the
payload (69,856 bytes of JSON on the wire) but is not charged, because the contract measures
readable text and bounded metadata, not duplicated structural copies. `chars_returned` therefore
changed meaning for repo items, which is recorded as a telemetry correction.

## Verification

Using the pinned Rust 1.89 toolchain on the candidate:

- `make check`: exit 0 — **85 suites, 6,445 passed, 0 failed, 28 ignored**, including fmt, clippy
  with `-D warnings`, the no-default-feature check, all-features tests, repository hygiene, the
  dependency policy, and the packaging contract (`planning-consistency: ok`).
- `make docs-check`: exit 0.
- `cargo test --locked --all-features --test batch_fetch_retrieval`: 25 passed, 0 failed.
- `cargo test --locked --all-features --test research_workflow`: 141 passed, 0 failed.
- `cargo test --locked --all-features --test mcp_projection`: 34 passed, 0 failed.
- `cargo test --locked --all-features --test static_guards`: 67 passed, 0 failed.
- `cargo test --locked --all-features --test repo_workflow`: 100 passed, 0 failed.
- `cargo test --locked --all-features --lib mcp::tools::fetch::tests`: 4 passed, including the
  locator-repair test.
- `cargo fmt --all -- --check` and `git diff --check`: clean.

The suite total moved from 6,430 to 6,445 (+15): three batch-retrieval regressions, one locator
unit test, the local-search regressions, and the advisory tests counted in M037/M038.

## Known limitations and residual risk

1. `chars_returned` changes meaning for repo items, from serialized JSON to readable text. This is
   the documented telemetry correction; consumers that compared the old number will see smaller
   values.
2. The aggregate budget bounds the readable measure, not the serialized JSON. A repo item can
   return a full structural `document` (the std slice returned 712 measured characters in a
   69,856-byte response). This is the deliberate contract: structural `document`/`lines` are not
   charged twice, and clipping them would drop content a diagnostic caller asked for.
3. An omitted item is a failure, so the existing failure semantics abort later items unless
   `continue_on_error` is set. That is consistent with every other item failure but changes the
   mixed-budget sequence from "skip silently" to "fail and stop".
4. The structured entry uses the existing `fetch_content_truncated` code; the
   `batch_total_budget_exhausted` prefix survives in the raw `warnings` list. No `WarningCode`
   taxonomy change was made.
5. Field-level pre-validation covers the four required repo locator fields and the web URL. Other
   malformed item shapes still fall through to the untagged-enum message.
6. No new dependency, tool, argument, schema property, cache key, or configuration default.

## Compatibility and migration

No MCP tool, argument, advertised schema, or configuration change. `repo_fetch` keeps its response
shape and budget. The only compatibility effect is the size reported by `chars_returned` for repo
items, and the failure state of a budget-omitted item, both documented in the changelog.

## Recommendation

Accept M036 on the closure commit. Every acceptance criterion has deterministic test evidence and
the three probes above show the before/after behaviour at a live origin and a live workspace; the
one documented tradeoff (structural document not charged) is stated in the architecture page and
recorded here.
