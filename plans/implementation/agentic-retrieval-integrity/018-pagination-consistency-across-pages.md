# M018 — Pagination Consistency Across Pages

Status: ready
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M018
Primary class: invariant (corrective to M009)
Baseline: `dace41c3a75307ea35b745d695a11630ec6495c0` (the worktree also carries uncommitted planning changes — the ADR-0009 amendment, ADR-0010, the roadmap status line, `plans/registry.md` rows, and plans 013/014/016; re-read `git status` before starting and do not revert them)
References: `plans/implementation/agentic-retrieval-integrity/009-long-running-progress-and-pagination.md`; `plans/closure/agentic-retrieval-integrity/009-status.md` § Known limitations; `architecture/mcp.md` § Pagination, progress, and tasks; `src/mcp/cursor.rs`; `src/mcp/tools/web_fetch.rs`; `src/mcp/tools/batch_fetch.rs`; `src/mcp/tools/research_search.rs`; `src/meta/adapter/research.rs`; `src/meta/research_grouping.rs`; `src/core/focus.rs`; `src/mcp/projection.rs`
Dependencies: hard M009 (closed). The closure record's three recorded limitations are this milestone's specification.

## Objective

Close M009's three recorded pagination limitations without changing the cursor contract, the ten-tool surface, any wire string, or any single-page payload. This is a corrective pass under `plans/003-planning-process.md` §7: each defect below states what is wrong and what M009's verification missed, and the regression test that prevents recurrence is named under Verification.

**Defect 1 — page size is decided by chunk boundaries.** `windowed_fetch` (`src/mcp/tools/web_fetch.rs:1057`) slices the already-extracted document by whole `document.chunks` through `page_window` (`src/mcp/cursor.rs:223`), whose `if end > start && next > capacity` rule guarantees at least one unit per page. A page therefore carries whatever whole chunks fit, and a chunk that alone exceeds the budget is emitted whole rather than split. A live trial produced a 14-chunk first page and a 1-chunk second page. M009's exhaustiveness test (`tests/mcp_tools.rs:634`) asserted only that chunk IDs partition the document, which every chunk-aligned slicing satisfies; no test bounded a page by text, so a one-chunk tail page passed. The fix is a page budget with char-exact packing: a page carries at most `page_budget_chars` characters of joined text, a chunk that would overflow is split at a character boundary with its remainder carried to the next page, and the split is reported. Chunks are never dropped, and no chunk is truncated without a marker.

**Defect 2 — `focus` describes the whole document but is reported per page.** `apply_focus_to_document` (`src/mcp/tools/web_fetch.rs:964`) ranks over `resp.document` before slicing, so page 1 and page 5 return the identical `focus` object with no indication that it is not page-local. M009 tested `focus` for determinism and content, never by comparing two pages of the same chain. Decision: `focus` stays document-wide and every paginated page says so — `focus_scope: "document"` is emitted whenever a non-null `focus` accompanies a page chain of more than one page. Per-page recomputation is rejected: it would make `focus` a function of page position, reinterpret `focus_max_chars` as a per-page budget, and let the highest-scoring chunk be absent from every page's selection, which is strictly worse for the navigation purpose `focus` exists to serve.

**Defect 3 — `research_search` aggregates are computed over a widened group set and presented unlabelled.** `run_research_search_with_progress` sets `max_groups: Some(page_groups.max(RESEARCH_PAGE_GROUPS))` (`src/mcp/tools/research_search.rs:220`) and `src/meta/adapter/research.rs:100` groups with `effective_max_groups(14)`, so `claims`, `conflicts`, `conflict_metadata`, `suggested_fetches`, `source_quality`, `evidence_gaps`, `telemetry`, `workflow_context`, `workflow_coverage`, `retrieval_summary`, and `trust_markers` describe up to 14 groups while the page returns as few as one. Two pages can therefore disagree about the same aggregate. M009's pagination tests asserted `groups` only and never compared aggregate fields across pages. Decision: aggregates remain document-wide — they are what makes cursor position resolution sound against the full canonical order — and every paginated page carries `aggregate_scope: "document"`, `total_groups` (the non-empty group count the aggregates were computed over), and `returned_groups` (the groups on this page). Page-scoping the aggregates is rejected because it would change today's response for every caller using `max_groups < 14` and would make the same aggregate legitimately differ between pages with no reconciling total.

**Scope of the fix.** The page budget belongs to the pagination layer, not to extraction. Chunk construction is defined in `src/core/document.rs:226` (`build_document_chunks`, 4,096-char and 8-block limits), called from `src/fetch/client.rs:1124` and `src/fetch/browser/navigate.rs:605`, and built inline at `src/fetch/pdf.rs:937`; extraction caps block text at the caller's `max_chars` (`src/fetch/render/blocks.rs:77`). Raising that to `[fetch].max_chars_cap` would turn one page into several comparable pages, but it would change `truncated`, `text_chars_returned`, chunk IDs, and the payload of a cursor-ignoring caller. It is excluded, so **`src/fetch/**` is out of scope and stays untouched**; the planner is nevertheless specified for multi-page documents because the pagination layer must not assume the extractor's current cap.

## Non-goals

- No new MCP tool and no `tools/list` change. The inventory stays ten tools.
- No new argument, no renamed field, no changed wire string, no new `required` key, no changed error code. Markers and one cursor field are additive.
- No change to `batch_fetch` pagination. Its units are the caller's own `items` and its aggregates are already page-scoped over the page's own `results`, so a page is internally consistent; it gains no marker in this milestone and loses none.
- No change to `repo_fetch` or `repo_search` (already span-addressed), to `build_evidence_bundle`, to progress notifications, or to the Tasks layer.
- No change to `src/fetch/**` extraction, to `build_document_chunks`, or to `[fetch].max_chars*` semantics: `max_chars` stays a per-page text budget.
- No page-scoped `blocks` or `outline`. `document.blocks` and `document.outline` remain the whole document on every page, as today; page-slicing them would subtract from a paginated payload.
- No rebalancing of pages to equalize sizes: pages fill the budget, and the final page is the exact remainder.
- No inference, no embeddings, no model call, no new dependency, no new configuration key.
- No change to the stale/mismatch/oversize contract, to `next_cursor`/`cursor_status` semantics, or to the 512-byte cap.

## Invariants that cannot regress

- Ten MCP tools; stable FNV-1a IDs (`src/core/identity.rs`); `chunk_id` derivation unchanged; sanitization tiers; trust framing; RRF tie ordering; capability-skip semantics; deadline bounds; Mode-Off remote denial.
- **Single-page byte identity.** For any request whose document fits one page, the response is byte-identical to the baseline, including the absence of every marker introduced here. A paginated response is today's payload plus additive keys; no key is removed and no value changes type.
- **Cursor contract.** `e1.` prefix, canonical JSON, base64url without padding, 512-byte cap, `v == 1`, `cursor_mismatch` for a wrong tool or plan key, `budget_invalid` for oversize/malformed/non-canonical, `cursor_status: "stale"` plus the first page and no `next_cursor` when `p.id` is absent.
- **Zero server-side cursor state.** The cursor remains a self-describing replay token; the pipeline is re-run and resumed after `p.id`, so there is nothing to evict, saturate, or poison.
- **No widened bound.** A resumed call uses `min(recorded, requested_or_config)` for both the client timeout and the call deadline, never exceeds `[fetch].max_chars_cap`, and never returns a page wider than the recorded budget.
- **Determinism.** Same request plus same cursor yields the same page; page boundaries depend only on chunk character counts and the budget; no hash-order dependence, no random ID, no clock-derived value in the page shape.
- **Bounded pages.** Every page's `text` is at most `page_budget_chars` characters, and no chunk is dropped, duplicated, or truncated without a marker.
- Ordinary files stay under 1,600 lines and 80 KB, inside existing ratchet ceilings; no code comments; no new dependency.

## Expected production changes

1. **Page budget planner — `src/mcp/cursor.rs`.** Add a text page planner over the flattened document stream (chunk texts in document order joined by a single `\n`): `TextPage { start_unit, start_off, end_unit, end_off }`, a `plan_text_pages(chunk_chars, budget)` pass that returns every boundary in one O(chunks) traversal, and `resolve_text_start(ids, cursor)` returning a normalised `(unit, off)` or `Stale`. Resolution locates `p.id` and advances by `off` rather than taking the chunk's first match, because a split chunk appears as a `tail` part on one page and a `head` part on the next with the same `chunk_id`. Packing is greedy by chunk: add whole chunks while the running joined length (including the separator) stays within the budget; when the next chunk does not fit, split it at the largest character boundary that fits the remaining budget, deliver the prefix, carry the suffix to the next page. A page is never empty, and a page boundary that falls between two chunks consumes the separating newline, so no page text begins or ends with a synthetic separator and a client joining pages with a single `\n` reproduces the document's chunk-join text. Because a page ends only when the budget is exhausted or when even the separator would not fit, **every non-final page carries exactly `budget` or `budget - 1` characters** and the final page carries the remainder. Keep `page_window`/`resolve_page` unchanged for `batch_fetch` and `research_search`, whose units are indivisible.
2. **Cursor position offset.** `CursorPosition` gains `off: u32` with `#[serde(default, skip_serializing_if = ...)]`, so `off == 0` produces today's exact canonical JSON and an in-flight cursor issued before this change still decodes (defaulting to `0`) and resumes correctly. `validate` bounds `off` structurally only; a value beyond the recomputed chunk length or not on a character boundary is classified `stale`, never a panic. `encode_next_text` emits the offset for `web_fetch`; `encode_next` is untouched for the other two tools. Add `resumed_budget_ms(recorded, requested_or_config)` as a named helper so the clamp is unit-testable without a live call.
3. **`web_fetch` wiring — `src/mcp/tools/web_fetch.rs`.** Replace the chunk-window call inside `windowed_fetch` with the planner, using `requested_max_chars` (already clamped at `src/mcp/tools/web_fetch.rs:303`) as `page_budget_chars`. Emit on paginated responses only: `page_index` (1-based), `page_count`, `page_budget_chars`, and `page_cuts` (an array drawn from `"head"`/`"tail"`, omitted when empty) naming that the first and/or last chunk on the page is a part of a source chunk rather than the whole chunk. Emit `focus_scope: "document"` when `focus` is non-null and `page_count > 1`. Set `document.text_chars_returned` to the page's character count and `document.text_truncated` to `page_index < page_count` (today's meaning: a later page exists). A cut chunk keeps its source `chunk_id` and the source chunk's `block_start`/`block_end`; `page_cuts` is the marker that the text no longer equals that block range's join. `focus` selection stays exactly where it is, before slicing.
4. **`research_search` markers — `src/mcp/tools/research_search.rs`.** Keep the internal widening to `RESEARCH_PAGE_GROUPS` and keep the page slice. Add `page_index`/`page_count` computed from the non-empty group count and `page_groups`, plus `aggregate_scope: "document"`, `total_groups`, and `returned_groups` on every paginated or resumed response.
5. **Shared helper — `src/mcp/tools/common.rs`.** Extend `with_cursor_keys` (`:129`) to take the page markers so both tools insert them through one path and the key strings are declared once.
6. **Projection — `src/mcp/projection.rs`.** Add the marker keys to the `brief` allowlist (`:458`) so `compact`/`brief` callers keep resume and scope information, exactly as `next_cursor`/`cursor_status` are already kept (`:599`).
7. **Advertised output envelope — `src/mcp/output_schema.rs`.** Add the new optional output keys to the `web_fetch` (`:110`) and `research_search` (`:86`) schemas. No `required` set changes, and the per-schema 1,200-byte budget is re-asserted, not raised.
8. **Docs.** `architecture/mcp.md` § Pagination (the packing rule, the `page_cuts` and scope vocabularies, the one-character page-size note, and the statement that `page_count` describes the document while `next_cursor` alone reports continuation); `CHANGELOG.md` (append-only); `docs/test-inventory.md` and `architecture/testing.md` for the `property_cursor`/`mcp_tools` descriptions that change.

`src/mcp/tools/batch_fetch.rs`, `src/mcp/tools/batch_fetch_item.rs`, `src/meta/research_grouping.rs`, and `src/meta/research_planner.rs` are read-only for this milestone: the grouping function already returns the full canonical order and the planner already bounds subqueries, so neither needs to change for the aggregate label.

## Storage, protocol, and compatibility effects

No schema-breaking change, no renamed field, no changed wire string, no new tool, no new argument. `cursor` stays an optional input; `next_cursor`/`cursor_status` keep their meanings; every marker is an optional output key that appears only when pagination is engaged.

Cursor arithmetic for the 512-byte cap, worst case (`p.id` at the 128-character ceiling, maximal `ord` and `off`, maximal timeout): `{"v":1,"t":"web_fetch","k":"0123456789abcdef","p":{"ord":4294967295,"id":"<128>","off":4294967295},"d":{"timeout_ms":18446744073709551615}}` is 262 bytes of JSON, 350 bytes as base64url without padding, 353 with the `e1.` prefix. The cap therefore still holds with 159 bytes of headroom and must be re-asserted by test rather than assumed.

Compatibility matrix:

| Situation | Behaviour |
|---|---|
| No `cursor`, document fits one page | Byte-identical to baseline; no marker emitted |
| No `cursor`, document exceeds one page | Page 1 fills the budget (never more than before), plus additive markers; `next_cursor` still offered |
| Cursor issued before this change | Decodes with `off = 0`, resumes at a chunk boundary, unchanged classification |
| Cursor issued after this change, sent to a pre-change server | Rejected as `budget_invalid` by `deny_unknown_fields`; forward-safe, never mis-sliced |
| Stale cursor | First page of the recomputed document plus `cursor_status: "stale"`, markers present, no `next_cursor`; `page_count` describes the document and is not a continuation offer |

There is no new server state: the cursor remains stateless, and the planner runs over content the call already recomputed. Determinism is preserved because the only inputs are chunk character counts and the budget.

## Ordered work packages

1. `src/mcp/cursor.rs`: `TextPage`, `plan_text_pages`, `resolve_text_start`, `encode_next_text`, `resumed_budget_ms`, and the marker-key constants; unit tests for exhaustive partition, budget/split boundaries, single-oversized-chunk splitting, `off == 0` canonical identity, and the offset rejecting an impossible position.
2. `CursorPosition.off` plus serde attributes; prove the pre-change canonical JSON is unchanged when `off == 0`, and that a cursor without `off` decodes.
3. `web_fetch` wiring and markers; confirm the single-page path still returns `text: None`, `document: None`, and no marker.
4. `research_search` markers; confirm the first call, the resumed call, and the stale call all label the same aggregate scope and totals.
5. Shared `with_cursor_keys` extension and projection allowlist.
6. Output-schema keys and the byte-budget assertion.
7. Tests (see Verification for placement): rewrite `tests/mcp_tools.rs:634` from chunk-ID exhaustiveness to character exhaustiveness, since a source chunk may now appear once as a `tail` part and once as a `head` part on consecutive pages.
8. Docs: `architecture/mcp.md`, `CHANGELOG.md`, `docs/test-inventory.md`, `architecture/testing.md`.
9. Full verification and the closure record.

## Verification

```bash
cargo fmt --all
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features --test mcp_tools
cargo test --locked --all-features --test mcp_projection
cargo test --locked --all-features --test property_cursor
cargo test --locked --all-features --test web_fetch_integration
cargo test --locked --all-features --test research_workflow
cargo test --locked --all-features --test static_guards
cargo test --locked --all-features
```

Required tests, placed per `architecture/testing.md`:

- **Page budget produces comparable pages from an uneven chunk distribution** (`tests/mcp_tools.rs`, planner unit test): chunk character counts `[40, 30_000, 60, 5_000, 90]` with an 8,000-character budget. Assert every page's `text` is at most the budget, every non-final page is exactly the budget or one character less, the last page is the remainder, and concatenating page texts with a single newline reproduces the document's chunk-join text exactly once with no gap. This fails on the baseline, where the first page is one 30,000-character chunk over budget or a 100-character page.
- **An oversized single chunk is split under the stated rule with a marker** (`tests/mcp_tools.rs`): a document containing one chunk of three budgets yields three pages; the first carries `page_cuts` containing `"tail"`, the middle both `"head"` and `"tail"`, the last `"head"`; the concatenation equals the source chunk text; no page exceeds the budget.
- **`focus` matches the chosen contract on page 1 and a later page** (`tests/mcp_tools.rs`): with `focus` set and `max_chars` small, page 1 and page 2 both report `focus_scope: "document"` and byte-identical `focus` objects, and that object equals the `focus` of a single-page request for the same document with a large `max_chars`.
- **`research_search` aggregates carry a consistent labelled contract across pages** (`tests/research_workflow.rs`): with `max_groups: 2` over at least three non-empty groups, page 1 and the next page both report `aggregate_scope: "document"` and the same `total_groups`, while `returned_groups` equals each page's group count; `claims`, `conflicts`, `suggested_fetches`, and `telemetry` are identical across pages, so two pages cannot contradict each other. A negative assertion pins that no page reports `aggregate_scope` other than `"document"`.
- **Cursor round-trip, stale, and mismatch unchanged** (`tests/property_cursor.rs`, `tests/mcp_tools.rs`): arbitrary-byte decode, oversize rejection, cross-tool mismatch, non-canonical JSON, `v != 1`, a cursor without `off`, round-trip with `off > 0`, an `off` beyond the chunk length or off a character boundary classified `stale`, and the 512-byte cap with a maximal-length `p.id`.
- **No widened deadline** (`tests/mcp_tools.rs` or a `cursor.rs` unit test): a forged `d.timeout_ms` larger than config resolves to `min(recorded, config)`, and a resume ignores a larger caller `timeout_ms`.
- **Byte identity for single-page responses** (`tests/mcp_tools.rs`): the full payload for a small page equals a recorded baseline, and `page_index`, `page_count`, `page_budget_chars`, `page_cuts`, `focus_scope`, `aggregate_scope`, `total_groups`, and `returned_groups` are absent.
- **Markers survive projection** (`tests/mcp_projection.rs`): every marker is present under `brief` and `compact` for a paginated payload and absent from a single-page one.
- **Vocabulary stays documented** (`tests/static_guards.rs`): every value in the `page_cuts`/scope vocabulary declared in `src/mcp/cursor.rs` appears in `architecture/mcp.md`, so a new marker value cannot ship undocumented.

`make check` is the canonical gate and must pass before closure.

## Acceptance and stop conditions

- A page's `text` never exceeds `page_budget_chars`, and a chunk that would overflow is split with `page_cuts` reporting it; no page carries an oversized whole chunk.
- Iterating a `web_fetch` chain yields every character of the document's chunk-join text exactly once, in order, with each source chunk appearing as at most one `tail` part and one `head` part on consecutive pages, page texts joined by a single newline reproducing that text, and the final page carrying no `next_cursor`.
- Page 1 and a later page of the same `focus` chain report identical `focus` and `focus_scope: "document"`.
- Page 1 and a later page of the same `research_search` chain report identical aggregates, `aggregate_scope: "document"`, and the same `total_groups`.
- A request with no cursor, no progress token, and a document that fits one page returns today's bytes, proven by a fixture comparison.
- Cursor round-trip, stale, mismatch, oversize, non-canonical, `v != 1`, and the 512-byte cap behave exactly as at baseline; a pre-change cursor still resumes.
- A resumed call never exceeds `min(recorded, requested_or_config)` and never exceeds `[fetch].max_chars_cap`.
- Zero server-side state: no cursor table, no page cache, no position stored across calls.
- **Fail closed, return for a decision, and do not proceed if:** the page budget would have to differ from the caller's effective `max_chars` (that would shrink a cursor-ignoring caller's page); the split rule cannot preserve every character exactly; a marker would require renaming an existing field, changing an existing value's meaning, or adding a `required` key; the cursor would exceed 512 bytes at the worst case; or the work would need `src/fetch/**` extraction changes or an eleventh tool. In those cases the correct outcome is a recorded limitation, not a silently weaker page.

## Closure evidence

`plans/closure/agentic-retrieval-integrity/018-status.md` must record: the implementation candidate SHA; the exact test commands and their results including `make check`; the measured page-size distribution for an uneven chunk fixture and for the live 14-chunk trial document (page count, per-page characters, and how much of the remainder the tail carries); the split-marker transcript for one cut chunk; the cross-page `focus` equality evidence; the cross-page aggregate equality evidence with `total_groups`/`returned_groups`; the byte-identity evidence for a single-page response; the cursor compatibility evidence for a pre-change cursor and for a maximal-length cursor against the 512-byte cap; the re-asserted output-schema byte budget; confirmation that no server-side state was added; and residual risk (content drift between pages, an offset that is meaningful only within an unchanged chunk, stale responses reporting a document `page_count` without offering a cursor, and the recorded fact that live extraction still caps total text at `max_chars`, so a live chain remains one full page plus its exact remainder rather than several comparably sized pages).
