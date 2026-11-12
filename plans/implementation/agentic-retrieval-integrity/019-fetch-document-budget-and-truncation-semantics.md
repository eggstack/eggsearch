# M019 — Fetch Document Budget and Truncation Semantics

Status: ready
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M019
Primary class: invariant (corrective)
Baseline: `8b701e220244ddf6422b10fd7fb7ca71a0732587`
References: ADR-0011 §Decision; `architecture/fetch.md` §Per-fetch loop; `architecture/config.md` §`[fetch]`; `docs/config.md` §Fetch Defaults; `docs/safety.md` (`raw_text`); `src/fetch/client.rs`; `src/fetch/render/blocks.rs`; `src/core/document.rs`; `src/fetch/cache.rs`; `src/fetch/limits.rs`; `src/core/config.rs`; `src/mcp/tools/common.rs`; `src/mcp/tools/web_fetch.rs`; `src/fetch/browser/navigate.rs`.
Dependencies: none (ADR-0011 is accepted). M020 is a hard dependent of this milestone: the document-scoped text, the split truncation facts, and the derived-key shape established here are its interface contract.

## Objective

Make the fetched document larger than any single caller window, and make truncation state truthful, by separating the **document budget** (how much of the page is extracted and addressable) from the **output budget** (how much text this call renders), without changing the bytes a caller receives in any field that already existed.

Today one budget does both jobs. `max_chars` is resolved and clamped in `src/fetch/client.rs:182-184`, then used to truncate the block list inside `render_blocks` (`src/fetch/render/blocks.rs:77-113`), to bound each block and the rendered text (`src/fetch/client.rs:846`, `:1025`, `:1095`), and to build chunks (`src/fetch/client.rs:1124` → `src/core/document.rs:226-354`). Page capacity in `windowed_fetch` is that same `max_chars` (`src/mcp/tools/web_fetch.rs:993`), so the document never contains more than one page, and pagination is structurally vacuous. `derive_from_raw` already takes a second argument, `max_chars_raw` (`src/fetch/client.rs:483`), but applies it only to `raw_text`.

The consequence for ADR-0011 is direct: a total computed over today's document would merely echo the caller's own truncation, so offsets would address nothing useful. This milestone is the prerequisite, not an optimisation.

**Interface boundary with M020.** `020-offset-addressing-and-cursor-retirement.md` owns `text_total` as a payload key and fixes its construction: the scalar count of chunk texts joined by a single `\n`, taken before sanitization so that offsets and lengths share one coordinate system. This milestone therefore does **not** mint a second field of that name. It supplies the two conditions that make that number truthful — chunks cover the whole document-budget block set exactly once, and chunk text is no longer pre-truncated at the caller's budget — and it emits one new boolean, `document.document_truncated`, so the document's own completeness is observable before M020 adds the number.

## Non-goals

- No `offset`/`limit` arguments, no `text_total` field, no `next_offset`, no cursor retirement, no response-surface removal, and no change to the cursor codec or plan-key shape. That is M020.
- No PDF change. `pdf_max_chars_per_page` and `pdf_max_total_chars` remain the PDF document budget, PDF extraction keeps operating at the caller's budget, and PDF keeps the caller budget in its derived-cache key through the tool layer's `build_derived_key(…, max_chars, …)` (`src/fetch/cache.rs:565-591`; the class is set at `:576`, and the extraction call itself is `src/fetch/client.rs:679-684`).
- No new MCP tool, no new cache tier, no persistence, no background work.
- No new dependency, and no change to `Cargo.toml` feature shape.
- No change to sanitization tiers, trust framing, `sanitize_field` behaviour, bounded body reads (`body_cap`), redirect handling, SSRF/origin policy, or cache scope isolation.
- No change to `select_content_root`, block classification/minimum probes, renderer output shape, chunk grouping rules, or focus scoring. Focus will rank a larger document; that is a cost to measure, not a scoring change.
- No change to `max_chars_default` (12,000) or `max_chars_cap` (50,000) defaults, and no new validation bound on `max_chars_cap` (rationale in § Storage, protocol, and compatibility effects).
- No re-commenting of production code; the "no comments" rule stands.

## Invariants that cannot regress

1. **Legacy payload byte identity.** For the same response body and the same request, these are byte-identical to `8b701e22` output: `title`, `description`, `text`, `raw_text`, `raw_text_chars_returned`, `raw_text_truncated`, `raw_text_cap`, `links`, `links_seen`, `links_truncated`, `truncated`, `trust_markers`, `stable_id`, and `document.kind`, `document.render_format`, `document.text_format`, `document.metadata`, `document.link_truncated`.
2. **Whole-payload byte identity where the document fits.** For any page whose extracted document fits inside the caller's `max_chars`, the entire serialized `web_fetch` payload is byte-identical, including `document.blocks`, `document.chunks`, and chunk ids. This covers every existing captured fixture: none of the 15 HTML files under `tests/fixtures/` exceeds 12 KB.
3. **Scoped exception, stated rather than assumed.** `document.blocks`, `document.chunks`, `document.outline` (pruned to the retained blocks, so entries and `block_index` values move), `document.block_truncated`, `document.text_chars_returned`, and the new field are budget-derived and **must** change for pages whose text exceeds `max_chars`. A blanket "the whole payload is byte-identical" claim is incompatible with the objective and is therefore not asserted; the split above is the contract. This is the reading ADR-0011 requires ("the document is extracted at the internal raw budget … the caller's `max_chars` bounds the returned window").
4. **`text` and `raw_text` keep meaning and budget.** `text` remains the rendered output bounded by the caller's `max_chars`; `raw_text` remains the Tier-1 body text bounded by `max_chars_cap`. No caller-visible text field grows.
5. **Truncation truthfulness.** `document.text_truncated == false` must mean the returned text is the document's complete text: the window covers the whole document-budget text, so nothing was clipped at the window. Its character count equals the count of that text after the same sanitization, which equals the pre-sanitization count only when no unsafe scalar was dropped — the basis rule M020 records. `document.document_truncated == true` must mean extraction itself stopped at the document budget, so the document is complete only up to that budget. The forbidden pairing — both flags false while the returned text is shorter than the extracted text — must be impossible by construction and covered by a test. A caller is never told "not truncated" while holding less than the document contains.
6. **Completeness does not vary with the caller.** The document-scoped text that M020 will count as `text_total` is independent of the caller's `max_chars` and survives a page window unchanged. A test computes it exactly as M020 defines it — the scalar count of chunk texts joined by `\n`, before sanitization — at three different caller budgets and asserts one identical value.
7. **Chunk ids stay content-derived FNV-1a.** `chunk_id(doc_id, index, heading_path)` (`src/core/identity.rs:864`) is unchanged in scheme, salt, and prefix. Ids move only because boundaries move; no new id algorithm, no random component, no cross-tool change to `doc_id`.
8. **Sanitization, framing, and bounds unchanged.** Every new field is produced inside the existing pipeline; no new untrusted text reaches output unframed or unnetted; nothing bypasses `bound_text`/`sanitize_field`; body reads stay under `body_cap`.
9. **Ten MCP tools, additive schema only.** No tool added or removed, no wire string changed, no `error_class` value changed.
10. **No new dependency**, and dependency-policy plus hygiene checks stay green.
11. **File ceilings respected.** `src/fetch/client.rs` is at 2,585 lines / 95,335 bytes against its ratified 2,600 / 100,000 ratchet (`architecture/maintenance.md`), leaving 15 lines and ~4.6 KB. New logic goes into new or non-ratcheted modules; the `client.rs` delta must be net-small, and no ratchet is raised in this milestone.
12. **Determinism.** Chunk order, focus tie-ordering, and warning order remain deterministic; the same body plus the same request produces the same payload across runs.

## Expected production changes

1. **Budget type.** New `src/fetch/budget.rs`: `FetchBudgets { document_chars, window_chars }` with one resolver (`document_chars = limits.max_chars_cap`, `window_chars = caller.unwrap_or(max_chars_default).min(max_chars_cap)`) and unit tests for default, clamp, `window == document`, and `document == 1` edges. No behaviour change on its own. This is where the two budgets are named, so no call site has to guess which `usize` it holds.

2. **One block-truncation rule.** In `src/fetch/render/blocks.rs`, extract the block-boundary truncation loop (currently inline at `:77-113`, including the code-block newline snap) into `pub(crate) fn truncate_blocks_at_budget(blocks: &mut Vec<RenderedBlock>, budget: usize) -> bool`. `render_blocks` calls it, and so does output projection. Byte identity is then guaranteed by construction rather than by coincidental agreement between two loops. `RenderedBlocks.block_truncated` and `RenderedBlocks.text_truncated` keep their current derivation inside the renderer (`:123` already makes them equal), but from this milestone neither feeds `FetchDocument.text_truncated`: at the document budget their only meaning is "extraction stopped at the document budget", and that value populates `document.document_truncated`.

3. **Document field.** In `src/core/document.rs`, `FetchDocument` gains exactly one field, `document_truncated: bool`, serialized only when true (matching `block_truncated`'s `skip_serializing_if = "std::ops::Not::not"`). It does not gain `text_total`: that name belongs to M020's payload key, and minting a differently-computed field under it would report one number under two meanings. `build_document_chunks` takes an explicit document budget; its internal clamp stays `min(budget, DEFAULT_CHUNK_CHAR_LIMIT = 4096)`, which means chunk text can only exceed a caller window when that window is below 4096 — the milestone's headline test must use e.g. `max_chars = 2000`, not 12,000. A new unit test asserts chunk coverage over the document-budget block list is exhaustive and contiguous (`block_start == 0` first, `block_end == blocks.len() - 1` last, no gap, no overlap), because M020's total is only truthful if that holds.

4. **Cold extraction path.** In `src/fetch/client.rs`, `derive_from_raw` takes `FetchBudgets` and uses `document_chars` for `render_blocks` (all three call sites at `:804`, `:825`, `:1011`), for the per-block Tier-1 bound (`:1025`, `:1095`), for the non-HTML text path (`:863-869`), and for `build_document_chunks` (`:1124`). The document-level artifacts become document-scoped; `window_chars` produces the legacy `text` by projecting the document-budget block list through `truncate_blocks_at_budget` and then `bound_text`, which reproduces today's block-boundary output exactly. Metadata-only mode keeps `window_chars` (no document is built).

5. **Projection module.** New `src/fetch/projection.rs` holding `project_document_text(blocks, window) -> String` and a response-level helper, so the cold path and the cache-hit path share one implementation. This is also what keeps `client.rs` inside its ratchet.

6. **Truncation facts, split.** The first fact is new: `document.document_truncated`, set only when extraction stopped at `document_chars`, derived from the same signal as `raw_capped` (`:900-903`, `:916-918`) for HTTP and from body/document budget exhaustion for browser. The second fact is not new and does not get a second field: `document.text_truncated` already names "you were given less than the document holds", keeps that meaning, and is now set by output projection alone — computed at `:1004`, `:1057`, `:1106`, defaulted at `:1116` for the empty non-HTML case, written to the field at `:1135`, and set directly by `windowed_fetch` (`src/mcp/tools/web_fetch.rs:1131`). Splitting the fact is not the same as re-pointing the variable: the extraction branch returns one merged flag today (`tt || txt_truncated` at `:856` for HTML, `tt` at `:889` for text), so the two sources must be returned separately rather than merged, or the extraction fact is dropped and invariant 5 fails in the opposite direction. Adding a parallel `output_clipped` boolean would report one fact under two names, the same mistake rejected for `text_total` above, so it is deliberately not introduced. No numeric total is emitted here; M020 counts it. The existing `document.text_chars_returned` keeps its basis (pre-Tier-1, pre-framing) and its meaning (characters in the returned text), so the truthfulness test compares returned characters against the extracted document's characters rather than against a new field.

7. **Cache key.** In `src/fetch/cache.rs`, `ExtractionCacheKey.max_chars_class: usize` (`:28`, set at `:576`) is replaced by `ExtractionBudgetClass { document_chars: usize, caller_chars: Option<usize> }`. HTML/text extraction passes `caller_chars: None`; PDF passes `Some(max_chars)`. `renderer_version` goes 4 → 5 (`:587`). `build_derived_key` takes that class in place of its `max_chars: usize` parameter, so no bare caller budget remains in the signature — M020's work-package-0 check ("`build_derived_key` no longer takes `max_chars` as a class") is then literally true rather than approximately true. All call sites (`src/mcp/tools/web_fetch.rs:350`, `:934`; `src/mcp/tools/batch_fetch_item.rs:137`, `:260`, `:572`) construct it from the resolved budgets.

8. **Cache-hit projection.** `cached_document_response` (`src/mcp/tools/common.rs:741`) gains the window budget and projects on read; without it, a shared entry would leak the first caller's window to every later caller. Its three call sites in `web_fetch.rs` (`:403`, `:462`, `:489`) pass the resolved window. `batch_fetch_item.rs` does not use that helper: it builds payloads inline from the shared entry at `:148` and `:272`, so both of those sites must go through the same projection helper rather than reading `derived.response.text` and `derived.response.document` directly.

9. **Cache-hit and cold-path parity for `text`.** A stored entry holds the document at `document_chars` and the unprojected text; both paths project to the same window, so a hit and a miss on identical input produce identical payloads. This is asserted, not assumed.

10. **Browser parity.** `browser_result_to_response` (`src/fetch/browser/navigate.rs:487-690`) takes `FetchBudgets` instead of `max_chars: Option<usize>`, drops the hardcoded `unwrap_or(12000)` at `:502`, renders at `document_chars`, projects output at `window_chars`, and builds chunks at `document_chars` (`:605`). `document.text_truncated` stops being an alias of `trust_markers.text_truncated` (`:616`) and `document.document_truncated` carries the document-budget fact. `WebFetchResponse.truncated` keeps its browser-path derivation from the bounded read and is not repurposed. Because the browser path currently ignores `max_chars_default` entirely, it is called with explicitly resolved budgets: under default configuration the payload is unchanged; under a non-default `max_chars_default` the browser path becomes consistent with the HTTP path, which is a parity fix recorded in closure evidence as a deliberate, browser-feature-only change.

11. **Tool layer.** `src/mcp/tools/web_fetch.rs` and `batch_fetch_item.rs` resolve `FetchBudgets` once per call from config plus arguments, use `window_chars` for page capacity and `max_total_chars` comparisons, and keep the cursor plan key exactly as shipped (it still binds `requested_max_chars`) so cursor behaviour is unchanged until M020 replaces it.

12. **Documentation.** `docs/config.md` (Fetch Defaults row and prose for `max_chars_cap`), `architecture/config.md` (the row currently reads "Hard extraction upper bound"), `architecture/fetch.md` step 2, `docs/safety.md`'s `raw_text` paragraph, and `docs/codegg-integration.md` (the two `max_chars_cap` examples and the `web_fetch` budget table) must all state the new meaning; `docs/test-inventory.md` per-suite counts are updated for the added tests; `CHANGELOG.md` gets an `Unreleased` entry. No new suite names.

## Storage, protocol, and compatibility effects

- **Protocol:** one additive field inside the existing `document` object. No new tool, no new argument, no removed field, no wire-string change, no schema `required` change. A client that ignores `document` sees no difference at all.
- **`max_chars_cap` becomes the document budget and the cost knob.** Documentation must say so; the current "hard extraction upper bound" wording becomes actively misleading, because it will now describe what is extracted rather than what is returned. Its validation rule (`cap >= default`, `src/core/config.rs:1244`) is unchanged. No new upper bound is added: effective characters are already bounded by the body cap (`max_bytes`, default 2,000,000), so a large `max_chars_cap` cannot exceed the bytes actually read. That bound is stated in docs and recorded as residual risk instead of silently introducing a compatibility-breaking validation error for configurations that parse today.
- **Cache:** `max_chars_class` becomes a constant for HTML/text extraction — the configured document budget, fixed for the life of a process, since a config change builds new state — and stops being a per-caller dimension entirely; only the PDF path keeps a caller-varying class. The derived entry becomes document-scoped, so per-entry bytes grow (blocks, chunks, and text at up to `max_chars_cap` instead of `max_chars`) while the number of entries per URL collapses from one-per-caller-budget to one. `derived_max_bytes` (64 MB) is unchanged, so entry capacity falls; both effects must be measured (see § Verification) rather than asserted either way. The cache is process memory only, so there is no on-disk migration: `renderer_version` 4 → 5 makes every existing entry unreachable, and the LRU evicts them naturally.
- **Cursors:** chunk ids change for pages whose text exceeds `max_chars`, so outstanding cursors become stale. This is expected and already handled: `resolve_page` returns page 0 with `stale: true` (`src/mcp/cursor.rs:296-299`), which the tool maps to `cursor_status: "stale"` rather than an error, and `tests/mcp_tools.rs` already exercises that path.
- **Fixtures that must be re-baselined:** none of the captured HTML fixtures; none exceeds 12 KB. The affected test fixture is `long_page_server(40_000)` in `tests/mcp_tools.rs`, used at six call sites; its document grows from ~12,000 to ~40,000 characters, so `web_fetch_pages_cover_every_chunk_once` loses its "chain must terminate on page two" assumption and its exact page-size assertion, and the three cursor tests that need a next cursor still have one but with different chunk membership. Every one of the six must be re-derived, not merely made to pass.
- **Configuration without change:** a site that sets `max_chars_cap == max_chars_default` keeps today's vacuous document, deliberately. Documentation says so; the milestone does not override an operator who has chosen a single budget.

## Ordered work packages

1. `src/fetch/budget.rs`: budget type, resolver, unit tests. No behaviour change; prove by running the fetch suites unchanged.
2. `src/fetch/render/blocks.rs`: extract `truncate_blocks_at_budget`, keep `render_blocks` output identical; add a test that the helper over the full list with budget `N` equals the old loop's output.
3. `src/core/document.rs`: add `document_truncated`, parameterise the chunk budget, add unit tests for the `min(budget, 4096)` clamp, for chunk text exceeding a 2,000-character window, and for exhaustive contiguous chunk coverage.
4. `src/fetch/client.rs` cold path: extract at the document budget, project at the window, populate the new field, keep `raw_text` at `max_chars_cap`. Prove `text` byte identity per document kind before touching the cache.
5. `src/fetch/cache.rs`: key change, `renderer_version` bump, `derived_entry_bytes` accounting re-check; convert `build_derived_key_distinguishes_max_chars` (`src/fetch/cache/tests.rs:1052`) into a sharing test and add a PDF-still-varies test.
6. `src/fetch/projection.rs` plus `cached_document_response` projection and its call sites; add the hit-versus-miss parity test.
7. `src/fetch/browser/navigate.rs` parity, with a test comparing browser and HTTP payload shapes for the same body under the `browser` feature.
8. Tool layer: resolve budgets once, keep the cursor plan key, keep batch per-item and total caps applied to windows.
9. Tests: byte-identity matrix (html, markdown, plain text, code, csv, notebook, diff, json/toml/yaml, xml/rst/asciidoc), the document-larger-than-window chunk test, the truncation truth table, cache sharing, saturation under a cap-sized document, PDF unchanged.
10. Measurement and docs (config, architecture, safety, codegg, test inventory, changelog).

## Verification

Focused, all suites confirmed present at this baseline:

```bash
cargo fmt --all
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features --test web_fetch_integration
cargo test --locked --all-features --test property_fetch_response
cargo test --locked --all-features --test mcp_tools
cargo test --locked --all-features --test static_guards
cargo test --locked --all-features
```

Supporting gates for the surfaces this touches:

```bash
cargo test --locked --all-features --test extract_fetch_contract
cargo test --locked --all-features --test property_cursor
cargo test --locked --all-features --test browser_transport
cargo test --locked --all-features --test config_validation
cargo test --locked --all-features --test docs_config_snippets
cargo test --locked --all-features --test schema_identity_registry
make bench-check
./packaging/check-repo-hygiene.sh
./packaging/check-dependency-policy.sh
python3 packaging/check-planning-consistency.py
make check
```

Required tests (behavioral placement per `architecture/testing.md`, no new suites):

- **Byte identity:** for each document kind, a no-argument call on a page that fits `max_chars` produces a payload byte-identical to a recorded `8b701e22` payload; and a page that does not fit produces an identical legacy surface with only the budget-derived document members (`blocks`, `chunks`, `outline`, `block_truncated`, `text_chars_returned`) and the new field differing.
- **Document exceeds window:** with `max_chars = 2000` and default cap, the returned `document.chunks` contain at least one chunk whose text exceeds 2,000 characters while the legacy `text` stays within its window — `document.text_chars_returned <= 2000`, and with `sanitize_output` on, `text` exceeds it only by the trust envelope — and `document.text_truncated == true`.
- **Truncation truth table:** four cases — fits with no window; fits, page window applied; exceeds window without page window; exceeds the cap. Assert that no case yields `!document.document_truncated && !document.text_truncated` while the returned text is shorter than the extracted document text, and that a cap-sized document reports `document.document_truncated == true`.
- **Complete-document total is caller-independent:** for one body, compute the M020 quantity (scalar count of `\n`-joined chunk text, before sanitization) at `max_chars` = 2,000 / 8,000 / 12,000 and assert one identical value, plus exhaustive contiguous chunk coverage.
- **Browser parity:** same body, same request, HTTP and browser paths agree on `text`, on the document character count, and on both flags for the fitting case.
- **PDF unchanged:** a PDF at `max_chars = 3000` returns the same text, document, and cache key behaviour as `8b701e22`, and two callers at different budgets still get distinct derived keys.
- **Cache sharing:** three callers at 2,000 / 8,000 / 12,000 on the same HTML body produce one distinct `build_derived_key` and three successful reads of the same entry, each projected to its own window.
- **Saturation:** a document at `max_chars_cap` completes inside the fetch deadline, with chunk count and total chunk characters reported, so the per-fetch cap is visible rather than inferred.

**Recorded measurements, not assertions.** Benchmarks are characterization only (`make bench-check` compiles them; no threshold decides pass or fail). The `fetch_document_budget` bench function does not exist yet: work package 10 adds it to `benches/perf.rs` and its `criterion_group!` list, so until then the filter below selects nothing.

```bash
cargo bench --locked --all-features --bench perf -- fetch_document_budget
```

Cost is measured as a same-build A/B, which isolates the budget change from unrelated drift: one HTML body of 50,000 characters, `max_chars = 12000`, with `max_chars_cap = 12000` (today's document budget) versus `max_chars_cap = 50000` (new). Record wall clock for the fetch, peak RSS for the run, the extracted document character count, and chunk count for both arms. Cross-check the whole-request wall clock against a run of the same measurement at `8b701e22` so an accidental second parse or render is caught. Memory per entry is recorded deterministically from `derived_entry_bytes` in an in-crate test over a fixed 50,000-character fixture, before and after. Cache magnitude is recorded as entry count and total derived bytes for one URL at one, two, and three caller budgets.

## Acceptance and stop conditions

- Default configuration, no new arguments: legacy fields byte-identical; whole payload byte-identical for every page that fits `max_chars`; all 15 captured HTML fixtures unchanged.
- A page larger than `max_chars` yields a document whose text and chunks exceed the window (chunk text when the window is below 4,096), a caller-independent document character count, and `text` still bounded by `max_chars`.
- The four-case truth table passes, including the forbidden pairing being unconstructible.
- Two callers with different `max_chars` share exactly one derived entry for HTML/text; PDF callers still do not.
- Browser and HTTP paths agree for the fitting case; PDF behaviour is byte-identical to baseline.
- Cost and cache measurements exist as recorded numbers with the exact commands, in both arms, on one machine.
- Stop and return for a decision if: output projection cannot reproduce baseline bytes for some document kind (name the kind, do not ship a silent change); per-entry growth makes the 64 MB derived budget hold materially fewer entries than the operator can accept; the cap-sized document exceeds the fetch deadline at default limits; or browser parity would change default-configuration output.

## Closure evidence

`plans/closure/agentic-retrieval-integrity/019-status.md` must record: the implementation candidate SHA and its baseline; every command above with its result; the byte-identity matrix per document kind including the fixture list that was compared; the four-case truncation truth table; the caller-independent document character count at three budgets with the chunk-coverage proof; the chunk-id churn inventory (which fixtures and tests moved, and the confirmation that stale cursors degrade to `cursor_status: "stale"`); the cache record (distinct keys, entry count, total derived bytes at one, two, and three caller budgets; `derived_entry_bytes` before and after; confirmation that pre-change entries simply miss after the `renderer_version` bump); the cost record (wall clock, peak RSS, chunk count for both A/B arms, with the harness commands and machine description); the browser parity record; the PDF unchanged record; the documentation diff summary proving `max_chars_cap`'s new meaning is stated everywhere it is described; and residual risk — `max_chars_cap` as an unbounded-above cost knob bounded only by `max_bytes`, per-entry growth against `derived_max_bytes`, chunk-id churn across releases for agents holding citations, focus ranking over a document up to 4.2× larger at defaults, and the M020 handoff: `text_total` counts `\n`-joined chunk text before sanitization, so it is only truthful while chunk coverage is exhaustive and chunk text is never pre-truncated at the caller's window — both of which this milestone establishes and M020 must continue to rely on rather than re-derive.
