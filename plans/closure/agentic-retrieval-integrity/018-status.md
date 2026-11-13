# M018 Pagination Consistency Across Pages — Closure Status

Status: closed (discharged by M020; no separate implementation)

Source plan: `plans/implementation/agentic-retrieval-integrity/018-pagination-consistency-across-pages.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Candidate: `e856468`

## Result

M018 named three M009 pagination defects. ADR-0011 and M020 replaced the mechanism and emitted the
two labelling contracts, so no code from this plan remained to implement at the candidate. M020's
own plan states this explicitly: M018 is superseded in its page-budget mechanism only, and its
three defect statements are discharged by window arithmetic plus the scope labels.

| Defect | Outcome at `e856468` |
|---|---|
| 1 — page size decided by chunk boundaries | Replaced by character-offset windows (`text_from`/`text_to`/`text_total`, `next_offset`). A window may begin or end at any scalar boundary and may split a chunk, so no page carries an oversized whole chunk. |
| 2 — document-wide `focus` reported per page | `focus_scope: "document"` is emitted whenever a non-null `focus` accompanies an addressed read (`src/mcp/tools/web_fetch.rs`). |
| 3 — research aggregates over a widened group set, unlabelled | `aggregate_scope: "document"`, `total_groups`, and `returned_groups` are emitted on every `research_search` window (`src/mcp/tools/research_search.rs`). |

## Verification (candidate `e856468`)

```bash
cargo test --locked --all-features --test mcp_projection addressed_web_fetch_keeps_addressing_keys
# 1 passed, 0 failed, 33 filtered out
cargo test --locked --all-features --test web_fetch_integration unaddressed_read_serves_the_chunk_aligned
# 1 passed, 0 failed, 88 filtered out
cargo test --locked --all-features --test research_workflow research_search_aggregates_are_document_scoped
# 1 passed, 0 failed, 140 filtered out
```

The projection test proves the scope/addressing markers survive `brief` and `compact` and are
absent from a single-window payload; the fetch test proves `focus_scope` and the document-scoped
focus object; the research test proves identical aggregates and totals across windows.

## Why M009 missed it

Recorded in the plan and unchanged: M009's exhaustiveness test asserted that chunk IDs partition
the document, which every chunk-aligned slicing satisfies, and its focus test asserted determinism
and content but never compared two pages of one chain. No test bounded a page by text or compared
the aggregate fields across pages, so a one-chunk tail page and a page-invariant focus object both
passed.

## Residual risk

- The cursor contract the plan quotes (`e1.` prefix, 512-byte cap, `cursor_mismatch`) is retired by
  M020; there is no in-flight cursor from this plan to preserve.
- `batch_fetch` remains unit-scoped over the caller's own items and needs no marker, as the plan
  recorded.
- No separate implementation exists to regress; the surface is covered by M020's offset tests and
  the three tests above.

Hosted CI: run `43472592990` on `0019f48` passed all three jobs — Ubuntu `ci` (12m11s),
Windows all-features check, and macOS canonical verification plus filesystem portability.
