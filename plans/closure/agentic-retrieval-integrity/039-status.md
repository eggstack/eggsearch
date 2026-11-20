# M039 Fetch Emitted-Payload Budget and Structural Degradation — Closure Evidence

Status: closed

Source plan: `plans/implementation/agentic-retrieval-integrity/039-fetch-emitted-payload-budget.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Decision authority: M036 (unified fetch readable-text budget) and ADR-0016 (canonical
nine-tool inventory). No new ADR.

## Scope

M039 makes `max_total_chars` bound what the caller actually receives while preserving
the requested text:

1. `payload_chars` measures each item's serialized `response` value before
   `response_detail` projection; `total_payload_chars` sums it across emitted items.
2. The aggregate budget is enforced against that measure, not the readable-text
   measure alone.
3. A repo item degrades optional structure before clipping text:
   `document.blocks`/`document.chunks`, then `document`, then `lines`, then
   `text`, then omission.
4. Each drop is recorded in `degraded_fields` and emits one
   `batch_payload_budget_degraded` warning mapped to the existing
   `fetch_content_truncated` code.
5. `chars_returned` keeps the M036 readable meaning and `truncated` still means
   text was clipped. Omitted items keep M036's failed-item contract.

The paper-first contract is in `architecture/fetch.md` § Batch returned-character
and emitted-payload budgets.

## Exact code candidate

Implementation commit `10ed7ed08ac47d80e39ca676ea157ecf0728518c` (parent
`6cd6c9a`). Every command and probe below ran against that tree. This closure
commit carries this record, the plan status, the roadmap status, and the registry
rows together.

## Requirement-to-evidence mapping

| # | Acceptance criterion | Evidence |
|---|---|---|
| 1 | A successful batch never emits item payloads above `max_total_chars` under the documented measure | `tests/batch_fetch_retrieval.rs::batch_total_payload_chars_bounds_three_item_batch` asserts `total_payload_chars` equals the per-item sum and is <= 50,000. The live three-item probe emitted 37,313 measured characters. |
| 2 | A windowed large repo read returns the requested text without silently including the full structure | The live `library/std/src/lib.rs` lines 1-8 probe returned the 443-character text with `ok: true` and a 3,142-character payload after dropping `document.blocks`/`document.chunks`; `document.text_chars_returned` stayed 30,271. |
| 3 | Any structural drop is visible in a marker and one structured warning | `src/mcp/tools/tests.rs::repo_batch_degradation_ladder_drops_structure_before_text` asserts the exact ladder `document.blocks, document.chunks -> document -> lines -> text` and the omission contract. `tests/batch_fetch_retrieval.rs::batch_repo_degradation_drops_structure_before_text` asserts `degraded_fields == ["lines"]`, the `batch_payload_budget_degraded` legacy warning, and `fetch_content_truncated` in `structured_warnings`. |
| 4 | `chars_returned` is unchanged; payload telemetry is present and bounds the projected payload | The live probes kept `chars_returned` at 712 (std) and 10,863 (README). `tests/mcp_projection.rs::batch_payload_budget_fields_survive_compact_and_standard` asserts `payload_chars`, `degraded_fields`, and `total_payload_chars` survive compact and standard projections while the response is projected. |
| 5 | Omitted items keep M036's outcome; `repo_fetch` and shipped defaults are unchanged | `tests/batch_fetch_retrieval.rs::batch_repo_metadata_budget_omission_is_a_failed_item` still passes with `ok: false`, `response: null`, `chars_returned: 0`, one budget warning. No `repo_fetch` source or default changed. |
| 6 | Broad gate passes with zero failures | `make check` exit 0: 85 suites, 6,453 passed, 0 failed, 28 ignored. `make docs-check` exit 0. |

## Live before and after

Probes used the all-features debug binary, MCP `stdio`, and the default 50,000
aggregate budget.

| Probe | Before (M036 evidence) | After (candidate `10ed7ed`) |
|---|---|---|
| `rust-lang/rust` `library/std/src/lib.rs` lines 1-8 | 712 reported characters in a 69,856-byte payload; `document` 64,158 bytes | `ok: true`, text 443, `chars_returned` 712, `payload_chars` 3,142, dropped `document.blocks`/`document.chunks`, document skeleton 290 bytes, `truncated: false` |
| `rust-lang/rust` `src/tools/rustfmt/README.md` | 10,863 reported characters, 59,555 serialized JSON, `document` 28,917 + `lines` 17,875 | `ok: true`, text 10,574, `chars_returned` 10,863, `payload_chars` 31,147, dropped blocks/chunks, `lines` retained, `truncated: false` |
| Three-item repo batch | 12,223 reported characters, **186,812-byte** payload | `total_chars_returned` 12,232, `total_payload_chars` 37,313, 41,765-byte envelope, all three items `ok` with text intact and blocks/chunks dropped |
| `https://www.rust-lang.org/` 100-character window | 197 reported characters, 11,058-byte payload, `document` 5,408 | 303 reported characters, `payload_chars` 6,579, 6,966-byte envelope, `document` 5,408 retained, no degradation |

The structural `document` skeleton (kind, `text_chars_returned`, metadata) survives
the first rung; the full blocks/chunks only return when the budget allows.

## Verification

Using the pinned Rust 1.89 toolchain on the candidate:

- `make check`: exit 0 — **85 suites, 6,453 passed, 0 failed, 28 ignored**, including
  fmt, clippy with `-D warnings`, the no-default-feature check, all-features tests,
  repository hygiene, dependency policy, and the packaging/planning contract.
- `make docs-check`: exit 0.
- Focused: `batch_fetch_retrieval` 27 (22 without PDF/browser), `mcp_projection` 35,
  `extract_fetch_contract` 27, `mcp_tool_contract` 21 (advertised fingerprint
  unchanged), `repo_workflow` 100, `evidence_contract` 12, `static_guards` 67,
  `lib` 3,937.
- `cargo fmt --all -- --check` and `git diff --check`: clean.

## Known limitations and residual risk

1. The payload measure covers item `response` values, not the envelope (wrappers,
   results array, counts, warnings, addressing keys, projection stamps), which are
   documented overhead.
2. Compact and standard projections may remove response fields after measurement, so
   `payload_chars`/`total_payload_chars` conservatively over-count the projected
   payload.
3. A budget too small to hold any serialized response omits the item; a
   one-character budget is not a way to request one character.
4. Web responses now also record `title`/`description`/`links` markers when the
   existing metadata trimming runs, so the warning is truthful about every drop.
5. `truncated` continues to mean text was clipped; structural degradation is
   reported only by `degraded_fields` and its warning.
6. No new dependency, tool, argument, schema property, cache key, or configuration
   default.

## Compatibility and migration

Additive fields only. `chars_returned` does not change meaning. The aggregate
budget now bounds the emitted payload instead of readable text, which is the
documented behaviour change; a caller that needs the full structural copy raises
`max_total_chars`. No migration.

## Recommendation

Accept M039 on `10ed7ed08ac47d80e39ca676ea157ecf0728518c`. Every acceptance
criterion has deterministic test evidence, and the live probes show the measured
before/after payload reduction with the requested text preserved.
