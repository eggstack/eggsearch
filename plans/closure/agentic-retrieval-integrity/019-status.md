# M019 Fetch Document Budget and Truncation Semantics — Closure Status

Status: closed

Source plan: `plans/implementation/agentic-retrieval-integrity/019-fetch-document-budget-and-truncation-semantics.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Planning baseline: `8b701e220244ddf6422b10fd7fb7ca71a0732587`

Implementation candidate and closure commit: the commit that carries this record. Implementation,
closure evidence, roadmap status, and the registry row all moved in that commit on top of the
planning baseline above.

## Result

One budget no longer does two jobs. `src/fetch/budget.rs` holds

```rust
pub struct FetchBudgets {
    document_chars: usize,
    window_chars: usize,
}
```

with `FetchBudgets::resolve(limits, requested)` deriving `document_chars = limits.max_chars_cap`
and `window_chars = requested.unwrap_or(max_chars_default).min(max_chars_cap)`. Extraction runs at
`document_chars`; the legacy `text` field is projected at `window_chars`.

`src/fetch/render/blocks.rs` now exposes `truncate_blocks_at_budget(&mut Vec<RenderedBlock>, usize)
-> bool`, the previously inline block-boundary loop including the code-block newline snap. Both
`render_blocks` and the new `src/fetch/projection.rs::project_document_text` call it, so the cold
path and the cache-hit path cannot drift.

`FetchDocument` gained exactly one field, `document.document_truncated` (omitted when false). The
document's own completeness and the caller's window clipping are now separate facts:

- `document.document_truncated` — extraction stopped at the document budget;
- `document.text_truncated` — the caller received less than the document holds.

A page larger than one window is therefore addressable: `document.blocks` and `document.chunks`
cover the whole extracted document while `text` stays inside the caller's window.

## What changed

- `src/fetch/budget.rs` (new): the budget pair, resolver, and six unit tests covering defaults,
  clamping, `window == document`, a one-character document, a zero budget, and a narrow window.
- `src/fetch/render/blocks.rs`: `truncate_blocks_at_budget` extracted; `render_blocks` output is
  unchanged because the helper is the same loop with the same inputs.
- `src/fetch/projection.rs` (new): `project_document_text(blocks, window, markdown) -> (String,
  bool)`, the single projection used by both paths, with four unit tests including
  input immutability.
- `src/core/document.rs`: `document_truncated` field; `build_document_chunks` now names its
  parameter `document_chars` and keeps the `min(budget, 4096)` clamp.
- `src/fetch/client.rs`: `derive_from_raw` takes `FetchBudgets`; the extraction branch returns
  the document flag and the window flag separately (the previously merged `tt || txt_truncated`
  is gone); blocks, per-block Tier-1 bound, `raw_text`, and chunks run at the document budget;
  `text` runs at the window; the HTML document's `text_chars_returned` and `text_truncated` come
  from the projection.
- `src/fetch/cache.rs`: `ExtractionCacheKey.max_chars_class: usize` replaced by
  `ExtractionBudgetClass { document_chars, caller_chars: Option<usize> }`; `renderer_version`
  bumped 4 → 5; `build_derived_key` takes the class instead of a bare caller budget.
- `src/mcp/tools/common.rs`: `project_cached_response` (for `WebFetchResponse`) and
  `project_cached_document` (for `CachedExtractedDocument`), so a cache hit is projected at the
  current caller's window instead of leaking the first caller's.
- `src/mcp/tools/web_fetch.rs`, `batch_fetch_item.rs`: budgets resolved once per call; the derived
  key built from `ExtractionBudgetClass::for_fetch`; both batch cache-hit paths project.
- `src/fetch/browser/navigate.rs`: `browser_result_to_response` takes `FetchBudgets`, renders at
  the document budget, projects text at the window, and separates the two flags.
- `src/fetch/pdf.rs`: PDF documents report `document_truncated` from PDF text truncation and keep
  extracting at the caller window.

## Invariants and their evidence

1. **Legacy field byte identity for fitting pages.** `web_fetch_integration` (85 tests) is
   unchanged except for one test that now sets a small `max_chars_cap`; every fitting-page payload
   is byte-identical because the projection reproduces the old loop exactly, and the new field is
   skipped when false.
2. **Budget-derived members are the only delta on long pages.** `mcp_tools::document_budget`
   asserts the document spans more than one window, every chunk stays inside 4,096 characters, and
   walking the chain delivers each chunk exactly once.
3. **Truncation truthfulness.** `truncation_flags_never_claim_completeness_below_the_document`
   covers four cases: small page with and without an explicit window, a long page at the default
   window, and a long page with a cap-sized budget. It fails the build if both flags are false while
   the returned text is shorter than the addressable text.
4. **Completeness does not vary with the caller.** `document_total_is_caller_independent` walks the
   full page chain at 2,000 / 8,000 / 12,000 and asserts one identical addressable total, contiguous
   `block_start`/`block_end` coverage, and no chunk delivered twice.
5. **Cache sharing and hit/miss parity.** `html_derived_keys_share_one_entry_across_caller_windows`
   and `pdf_derived_keys_still_vary_by_caller_window` (in `src/fetch/cache/tests.rs`) pin the key
   shape; `cache_hit_matches_miss_and_reprojects_per_window` asserts that a hit at 4,000 returns the
   bytes a cold 4,000 fetch returns (apart from `cache_status`) while a wide read of the same entry
   still renders more text.
6. **Sanitization, framing, and bounds unchanged.** No new untrusted text path; the projection
   operates on already Tier-1-stripped blocks and the response passes through the existing
   `frame`/`sanitize_field` sites.
7. **Ten MCP tools, no wire-string change.** `static_guards` (61), `docs_tool_names` (2), and
   `docs_config_snippets` (2) pass; no tool, provider, or capability flag was invented.

## Verification

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo check --locked --all-features
cargo check --locked --all-features --all-targets
cargo test --locked --all-features --no-fail-fast        # 6,192 tests, 27 ignored
cargo test --locked --features mock -- --list            # 5,837 tests
cargo test --locked --all-features --lib                 # 3,761 passed
cargo test --locked --all-features --test web_fetch_integration    # 85 passed
cargo test --locked --all-features --test mcp_tools                # 23 passed
cargo test --locked --all-features --test extract_fetch_contract   # 26 passed
cargo test --locked --all-features --test property_fetch_response
cargo test --locked --all-features --test browser_transport        # 75 passed
cargo test --locked --all-features --test config_validation        # 91 passed
cargo test --locked --all-features --test schema_identity_registry # 88 passed
cargo test --locked --all-features --test mcp_projection           # 31 passed
cargo test --locked --all-features --test property_cursor          # 5 passed
cargo test --locked --all-features --test static_guards             # 61 passed
./packaging/check-repo-hygiene.sh
python3 packaging/check-planning-consistency.py
```

`make check` cannot complete in this sandbox: `packaging-check` stops at
`mavis-trash: unsupported OS: Plan9`. Every sub-check that runs before that point passes, and the
`--all-features` and `--features mock` suites were run directly.

## Measurements

Recorded, from the tests above:

- addressable document total for a ~40,000-character page: identical at windows 2,000 / 8,000 /
  12,000 (asserted equal);
- chunk count for that page: greater than ten, each chunk at most 4,096 characters;
- page chain length at a 2,000-character window: more than two and fewer than forty pages;
- derived keys for one HTML body: one entry across caller windows; PDF and non-HTML bodies keep one
  entry per caller window.

**Not run, and why.** `cargo bench --bench perf -- fetch_document_budget` does not exist: the plan
nominated a bench function that was never added, so there is no same-build A/B wall clock or peak
RSS record, and no in-crate `derived_entry_bytes` before/after number. Per-entry size certainly
grew (blocks, chunks, and text now hold up to `max_chars_cap` rather than the caller window) and the
entry count per URL collapsed for HTML; the magnitude is unmeasured. This is recorded as a gap, not
as a satisfied requirement.

## Deviations from the plan

1. **Cache sharing is HTML-only, not "HTML/text".** Non-HTML documents (plain text, CSV, code,
   notebooks, PDF) cannot be re-projected from `document.blocks`, because their `text` is the raw
   decoded prefix while their blocks are renderer output. Their derived key therefore keeps
   `caller_chars: Some(window)`. `for_fetch` decides by `Content-Type` containing `html`; a page
   with no usable content type stays caller-scoped, which costs a shared entry rather than
   correctness.
2. **The browser path is now window-bounded.** It previously bounded each block at `max` and then
   returned the whole block join, so its `text` could exceed the window by an unbounded amount. It
   now projects at `window_chars`. This is visible only for browser-rendered pages whose joined text
   exceeds one window, and it is the parity fix the plan anticipated, extended to large pages.
3. **`document.text_chars_returned` on the HTML path counts the projected text.** For clean text
   this is unchanged; when `strip_control_chars` drops a control scalar the count is now the count
   of what is returned rather than of the pre-strip source. This is what makes cache-hit parity
   exact, since a cached entry stores stripped blocks.
4. **No `document.text_total`.** The plan forbade minting it; M020 owns the number.
5. **`renderer_version` 5** rather than the planned 4 → 5 phrasing being ambiguous: entries written
   by 0.4.4 simply miss and are evicted by the LRU.

## Residual risks

- `max_chars_cap` is now an unbounded-above cost knob, bounded only by `max_bytes` (default
  2,000,000 bytes of body). An operator who raises it raises per-fetch CPU and per-entry memory.
- Chunk ids changed for pages whose text exceeded the previous caller window: earlier chunks keep
  their ids, later ones are new. Outstanding cursors degrade to `cursor_status: "stale"` with the
  complete first page rather than erroring.
- Focus ranking now runs over a document up to 4.2× larger at default settings; cost is unmeasured.
- The browser path bounds its text but still reports `truncated` from the bounded body read, not
  from the text window; M020 revisits the window semantics.

## M020 handoff

M020 may now start: `text_total` can be computed over chunk texts joined by `\n` and it will be the
true addressable total rather than an echo of the caller's truncation, because chunks cover the
document-budget block set exactly once and chunk text is no longer pre-truncated at the caller's
window. The basis is pre-sanitization, matching the existing `text_chars_returned` count.
