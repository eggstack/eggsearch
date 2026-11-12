# M020 Offset Addressing and Cursor Retirement — Closure Status

Status: conditionally closed

Source plan: `plans/implementation/agentic-retrieval-integrity/020-offset-addressing-and-cursor-retirement.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

ADR: `plans/adrs/ADR-0011-fetch-document-addressing.md`

Planning baseline: `8b701e220244ddf6422b10fd7fb7ca71a0732587`

Implementation candidate and closure commit: the commit that carries this record. Implementation,
closure evidence, roadmap status, and the registry row all moved in that commit on top of M019's
closure commit `8bb5c8a`.

**Named condition.** `text_total` is the true document-budget total for HTML and browser-rendered
documents only. For PDF and every other non-HTML body it remains bounded by the caller's window,
inherited from M019's recorded deviation 1, so offset windows for those shapes cannot address past
the first window. Measured, not inferred: a 40 KB HTML fixture reports `text_total = 38,055` for a
11,791-scalar first page, while the live `text/plain` read of RFC 9110 reports `text_total = 11,981`
(the 12,000 default window) for a document of several hundred kilobytes. Discharging it requires moving those two extraction paths to the document budget,
which this plan's non-goals forbid (`src/fetch/**` other than the document identity) and which its
fail-closed list names explicitly ("`text_total` cannot be made larger than a single window without
changing extraction"). See "Residual risk" item 1 for the exact call sites.

## Result

The opaque replay cursor is gone from the surface. `web_fetch` addresses characters, `batch_fetch`
addresses items, and `research_search` addresses non-empty groups, each by re-specifying the
original request with `offset` and `limit`. No argument, response key, or error code carries a
token; nothing is stored server-side; a window is a pure function of the request and the cached
document.

`src/mcp/cursor.rs` (499 lines) was deleted and replaced by `src/mcp/plan_key.rs` (~120 lines):
`PlanKey` and `PlanTool` remain, the codec (`RetrievalCursor`, `CursorPosition`, `CursorBudget`,
`CursorError`, `CURSOR_PREFIX`, `MAX_CURSOR_BYTES`, `CURSOR_SCHEMA_VERSION`,
`MAX_CURSOR_ID_CHARS`, `PageWindow`, `page_window`, `locate_start`, `encode_next`, `resolve_page`,
`ResolvedPage`, `cursor_args`) and its entire test module are gone. `base64` stays a dependency
because `src/meta/challenge/altcha.rs` uses it.

## Work package 0 — M019 prerequisite evidence

All four conditions held before any addressing code was written:

1. M019 is `closed` with a closure record: `plans/closure/agentic-retrieval-integrity/019-status.md`
   (Status: closed); its plan is at `Status: closed`.
2. Blocks are truncated at the internal budget, not at the caller's window:
   `src/fetch/render/blocks.rs:75` calls `truncate_blocks_at_budget(&mut blocks, max_chars)` and
   every production caller passes the document budget — `src/fetch/client.rs:806`, `:837`, `:1019`
   pass `document_chars`. The caller window reaches `src/fetch/projection.rs:14` instead.
3. The derived cache key no longer varies by caller budget:
   `src/fetch/cache.rs:600` `build_derived_key(..., budget: ExtractionBudgetClass, ...)` with
   `ExtractionBudgetClass { document_chars, caller_chars: Option<usize> }` (`:26`), and
   `renderer_version: 5` (`:621`).
4. Chunks cover the document-budget block set exhaustively and contiguously with no pre-truncation
   at the caller's budget: `src/core/document.rs:226` `build_document_chunks(doc_id, outline,
   blocks, document_chars)` over the document-budget block set, pinned by
   `tests/mcp_tools.rs::document_budget::document_total_is_caller_independent` and the M019
   coverage tests.

M019 deliberately mints no `FetchDocument.text_total`, so its absence was not a blocker; this
milestone computes the number from the chunk join.

## What changed

- `src/mcp/plan_key.rs` (new): `PlanKey`/`PlanTool` with three unit tests (content-derived 16-hex
  and sensitivity to tool and URL; canonical URL equivalence; optional-field hashing). Entity
  prefix is `request_plan`; plan keys are internal and were re-derived, never transported.
- `src/mcp/mod.rs`: `pub mod cursor` → `pub mod plan_key`.
- `src/mcp/tools/common.rs`: `ToolErrorCode::CursorMismatch` (`cursor_mismatch`) removed;
  `PlanMismatch` (`plan_mismatch`) and `DocumentUnavailable` (`document_unavailable`) added.
  `cursor_rejected`/`with_cursor_keys` replaced by `addressing_rejected(AddressError)`,
  `AddressWindow::resolve/len/next_offset`, the `AddressKeys` constants `TEXT_ADDRESS_KEYS`,
  `ITEM_ADDRESS_KEYS`, `GROUP_ADDRESS_KEYS`, `with_addressing_keys`, and `slice_chars` (scalar
  slicing by `char_indices`, so a window can never split a scalar).
- `src/mcp/tools/web_fetch.rs`: `WebFetchArgs.cursor` → `offset`/`limit`; `limit` defaults to the
  effective `max_chars` window and clamps to `max_chars_cap` with a `max_results_clamped` structured
  warning; `limit == 0` is `invalid_semantic_value` with `repair.field = "limit"`; an addressed
  read with a clamped `focus_max_chars` is `plan_mismatch`; an addressed read with no addressable
  document is `document_unavailable` (`url` when unfetched, `extract_mode` when `metadata_only`);
  a terminal fetch failure is now `upstream_failed` rather than `internal`, matching the matrix and
  the existing rule that `internal_error` is reserved for server faults. `windowed_fetch` is window
  arithmetic: the unaddressed path keeps the chunk-aligned first page, the addressed path slices
  `[from, to)` out of the chunk join, applies `strip_control_chars` then `frame` to the slice,
  reports `text_from`/`text_to`/`text_total`/`doc_id`, `focus_scope: "document"`, and
  `next_offset` only when more text remains. The effective timeout is
  `min(caller timeout_ms, [fetch].timeout_ms)`.
- `src/mcp/tools/batch_fetch.rs`: `cursor` → `offset`/`limit` over the item list; `limit` defaults
  to `max_items`, clamps to `batch_max_items_cap` with a warning; `batch_fetch_plan_key` keeps its
  baseline field list (via `batch_fetch_plan_key_over` for the served bound) and never includes
  `offset`/`limit`; an addressed call whose supplied `max_items`/`max_total_chars` the server clamps
  is `plan_mismatch`; the existing `batch_item_count_truncated` warning is reused verbatim for
  addressed windows (`item_from > 0 || item_to < item_total`); `item_from`/`item_to`/`item_total`/
  `next_offset` are emitted on addressed reads only.
- `src/mcp/tools/research_search.rs`: `cursor` → `offset`/`limit` over non-empty groups; `limit`
  defaults to the `max_groups` page size and clamps to `RESEARCH_PAGE_GROUPS` (14) with a warning;
  `research_search_plan_key` keeps its baseline field list and excludes `offset`/`limit`;
  `plan_mismatch` for a clamped `max_results` or `max_per_group`; `group_from`, `next_offset`,
  `aggregate_scope: "document"`, `total_groups` and `returned_groups` (M018's three labelling
  decisions, emitted here because M018's page-budget mechanism is superseded) on addressed reads
  only.
- `src/core/document.rs`: `FetchDocument.doc_id: String` with `#[serde(skip)]` and
  `#[schemars(skip)]`, so no serialized `document` object gains a key. Populated at
  `src/fetch/client.rs:1141` (HTML, the value already computed at `:1133`), `:589` (non-text PDF
  fallback), `:692` (the PDF document patched after `extract_pdf_text`, because
  `src/fetch/pdf.rs` has no URL and leaves `doc_id` empty), and
  `src/fetch/browser/navigate.rs:609`.
- `src/core/evidence_postprocess.rs`: `found_roles` is now built in card order with a dedupe
  instead of by iterating a `HashSet`, because hash iteration order made two identical
  `research_search` windows report different `workflow_coverage` bytes. Same membership, stable
  order; required by this milestone's determinism invariant.
- `src/mcp/output_schema.rs`, `src/mcp/projection.rs`, `src/mcp/tool_contract.rs`,
  `src/mcp/server.rs`: cursor keys removed from the three output schemas; the addressing keys,
  `focus_scope`, `aggregate_scope`, `total_groups` and `returned_groups` added to the `brief`
  allowlist so a compact caller keeps addressability; `web_fetch` keywords gain `offset`, `limit`,
  `window`, `range` and its `use_when` states the range affordance; the argument name lists move
  from `cursor` to `offset`/`limit`.
- Tests: `tests/property_cursor.rs` deleted. New behavioural coverage in `tests/mcp_tools.rs`
  (scalar safety, framing, boundary, distinguishable codes), `tests/web_fetch_integration.rs`
  (chunk-aligned first window, limit clamp, opt-in live two-window union),
  `tests/research_workflow.rs` (eight group-window tests), `tests/mcp_projection.rs` (addressing
  survives `brief`/`compact`), `tests/mcp_tool_contract.rs` (no cursor token in any advertised
  schema, code registry), `tests/static_guards.rs` (prose guard over `src/mcp/**` and the seven
  doc sites), and the first-slice corpus in `tests/extract_fetch_contract.rs` with
  `tests/fixtures/fetch_windows/`.
- Docs: `architecture/mcp.md` § "Offset windows, progress, and tasks" rewritten;
  `docs/config.md` § "Offset Windows, Progress, and Tasks"; `docs/tool-matrix.md`,
  `docs/codegg-integration.md`, `architecture/codegg-contract.md`,
  `skills/eggsearch-mcp/SKILL.md` stable-code lists; `CHANGELOG.md` additive entry;
  `docs/test-inventory.md`, `architecture/testing.md`, `architecture/hardening.md` property-suite
  counts and the `property_cursor` row; `tests/fixtures/tool_surface/README.md` measurement block.

## Verification

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features --no-fail-fast
python3 packaging/check-planning-consistency.py
./packaging/check-repo-hygiene.sh
```

Results are recorded in "Gate results" below. `make check` is the canonical gate and cannot
complete in this environment because `packaging-check` aborts at
`mavis-trash: unsupported OS: Plan9`; every sub-check before that point passes, and the
corresponding commands above were run individually.

### Compatibility gate

Measured across versions, not only asserted. A throwaway dump test built the same fixed-port
fixture in a `8bb5c8a` worktree (pre-M020) and in the candidate tree, fetched the same URL with
neither `offset` nor `limit`, and compared the parsed payloads:

- single-window fixture (a 400-byte page): the two payloads are deeply **identical**, with no key
  dropped at all — the baseline never emitted a cursor key for a page that fits one window;
- multi-window fixture (a 40 KB page): the two payloads are deeply **identical after removing
  exactly `next_cursor` and `cursor_status`**; chunk count 11 in both, `text` 11,791 scalars in
  both, `document.text_truncated == true` in both, and no other field, type, or value differs.
  The baseline emitted only `next_cursor` (no `cursor_status`, because a first page is neither
  `stale` nor resumed), so the delta is precisely the retired token.

The in-tree counterparts pin the same shape without a second worktree:

- A single-window document read with no `offset`/`limit` carries neither `next_cursor`/
  `cursor_status` nor any addressing key. `tests/web_fetch_integration.rs::
  unaddressed_read_serves_the_chunk_aligned_first_window` asserts the absent key set, asserts
  `text` equals the join of `document.chunks` by `"\n"`, asserts `document.text_chars_returned` is
  that join's scalar count, and asserts `document.text_truncated == true` on a 40 KB page.
- A multi-window document read with no `offset`/`limit` is still the chunk-aligned first page
  (`src/mcp/tools/web_fetch.rs::default_page_chunk_count` reproduces the retired `page_window`
  accounting exactly, including the leading `+1` per chunk and the always-include-first rule); the
  only permitted delta is the absence of the two unreleased keys, and the same test asserts the
  page is strictly smaller than `text_total`.
- `batch_fetch` and `research_search` unaddressed payloads gain nothing:
  `tests/mcp_tools.rs::pagination::legacy_client_payload_has_no_cursor_keys` (extended to the
  addressing key set) and `tests/research_workflow.rs::
  research_search_unaddressed_read_carries_no_addressing_keys`.

### Addressable-text basis, one `text_total`, and gap-free composition

- The basis is stated once in `src/mcp/tools/web_fetch.rs::addressable_text`: every chunk text
  joined by a single `\n`, counted before `strip_control_chars` and before `frame`.
  `text_total` appears only as a top-level `web_fetch` key on addressed reads; it is not
  duplicated inside `document`, whose `text_chars_returned` stays window-scoped.
- Composition is asserted rather than argued: `tests/mcp_tools.rs::
  pagination::web_fetch_offset_windows_compose_to_the_whole_document` walks a 40 KB page in 2,000
  scalar windows, checks `text_from == offset` and `next_offset == text_to` at every step, checks
  `text_total` never moves, and asserts the concatenation equals the single-window whole exactly
  once.
- Scalar safety: `tests/mcp_tools.rs::pagination::windows_are_scalar_safe_on_multibyte_text` walks
  a multi-byte fixture in 1,001-scalar windows and asserts each window's returned text has exactly
  `text_to - text_from` scalars, contains no U+FFFD, and that the concatenation equals the whole.

### Boundary transcript

From `tests/mcp_tools.rs` (`pagination` and `document_budget` modules):

Observed on one real worked example (40 KB HTML body, `[fetch]` defaults, `sanitize_output`
off): `text_total = 38,055`, unaddressed first page = 11,791 scalars over 11 chunks.

| Request | Observed |
|---|---|
| no `offset`, no `limit` | no addressing key; `text` 11,791 scalars; 11 chunks; `document.text_truncated` true |
| `offset=0`, `limit=1_000` | `text_from` 0, `text_to` 1_000, `next_offset` 1_000, 1 chunk, 1,000 scalars |
| `offset=5_000`, `limit=1_000` | `text_from` 5,000, `text_to` 6_000, `next_offset` 6_000, 2 chunks (boundary straddles a chunk), 1,000 scalars |
| `offset=5_000`, no `limit` | 5,000 -> 17,000 (the 12,000 default window), `next_offset` 17,000, 12 chunks |
| no `offset`, `limit=2_000` | 0 -> 2,000, `next_offset` 2,000 |
| `offset=11_791`, `limit=10` | 11,791 -> 11,801; an offset past the *first page* is still addressable |
| `offset=99_999`, `limit=10` | clamped: `text_from == text_to == 38,055`, `text == ""`, no `next_offset`, 0 chunks, `text_truncated` false, no error |
| `offset=0`, `limit=0` | `invalid_semantic_value`, `repair.field == "limit"` |
| `offset=0`, `limit=1_000`, `focus` set | 1,000-scalar window plus `focus_scope: "document"` and a non-null `focus` |
| `offset=0`, `extract_mode: "metadata_only"` | `document_unavailable` |
| `limit` above `max_chars_cap` | clamped to the cap, `next_offset` present, `structured_warnings` carries `max_results_clamped` |

### Framing and sanitization do not move an offset

`tests/mcp_tools.rs::pagination::trust_framing_never_moves_an_offset` reads the same
`offset = 1_000`, `limit = 500` window with `sanitize_output` on and off and asserts
`text_from`, `text_to`, `text_total`, `next_offset` and `doc_id` are identical, that the framed
text differs from the plain text, and that the plain text is contained in the framed text — so
framing wraps the slice instead of shifting it. The same test asserts the empty window is `""` and
unframed. No test asserts `text.chars().count() == text_to - text_from` outside the clean fixtures,
because a slice containing a dropped scalar is legitimately shorter; the extraction paths strip
unsafe scalars per block before chunking, so that shortening is not observable through `web_fetch`
and is pinned structurally (offsets are computed on the pre-strip join) rather than by a fixture.

### Distinguishable codes

| Condition | `code().as_str()` | Where |
|---|---|---|
| addressed read, clamped `focus_max_chars` | `plan_mismatch` | `mcp_tools::pagination::addressed_web_fetch_rejects_a_clamped_plan_field` |
| addressed read, `extract_mode: "metadata_only"` | `document_unavailable` | `mcp_tools::pagination::addressed_reads_report_distinguishable_codes` |
| addressed read, connection refused | `upstream_failed` | same test |
| batch, `max_items` above cap, addressed | `plan_mismatch` | `mcp_tools::pagination::addressed_batch_fetch_rejects_a_clamped_plan_bound` |
| research, `max_results` above cap | `plan_mismatch` | `research_workflow::research_search_addressed_plan_clamp_is_a_plan_mismatch` (`repair.field = "max_results"`, `suggested_value = "50"`) |
| research, `max_per_group: 0` | `plan_mismatch` | same test (`repair.field = "max_per_group"`, `suggested_value = "1"`) |
| any tool, `limit == 0` | `invalid_semantic_value` | `mcp_tools::pagination::zero_limit_is_a_repairable_rejection`, `research_workflow::research_search_zero_limit_is_a_repairable_rejection` |

### Collection exhaustiveness

- `batch_fetch`: `tests/mcp_tools.rs::pagination::batch_fetch_pages_every_item_once` walks 10
  items with `max_items: 4` from `offset: 0` — 3 windows, indices `0..=9` visited in order, no
  duplicate, no gap, last window without `next_offset`.
  `batch_fetch_above_page_capacity_still_terminates` repeats it with 20 items and `max_items: 3`.
- `research_search`: `tests/research_workflow.rs::
  research_search_offset_walk_visits_every_group_once_in_order` walks 5 non-empty groups with
  `limit: 2` — 3 windows (`0..2`, `2..4`, `4..5`), `group_from` equal to the expected absolute
  index each time, no duplicate, no gap, final window without `next_offset`.
  `research_search_aggregates_are_document_scoped_across_windows` asserts the two windows report
  the same `total_groups` and `aggregate_scope: "document"` with byte-identical `claims`,
  `conflicts`, `suggested_fetches`, `telemetry`, `workflow_context`, `workflow_coverage`
  (including `found_roles` order), `trust_markers`, `source_quality` and `evidence_gaps`, while
  `returned_groups` matches each window's count. `retrieval_summary` is compared with each
  dimension's `duration_ms` zeroed, because that field is wall-clock telemetry and differs between
  two identical calls; every structural member of it is asserted identical.
  Mid-walk payload for `offset: 2, limit: 2`:
  `{"aggregate_scope":"document","group_from":2,"groups":["Design Discussions","Benchmarks"],"next_offset":4,"returned_groups":2,"total_groups":5}`.
- A changed set is a fresh read, not a continuation: the same suite's
  `research_search_changed_query_addresses_the_new_group_list` asserts `total_groups` moves from
  5 to 7 and the window addresses the new canonical order.

### Cursor absence

- `tests/mcp_tool_contract.rs::advertised_schemas_carry_no_replay_cursor_tokens` serializes every
  advertised tool's input schema under both schema profiles and every output schema, and asserts
  neither `next_cursor` nor `cursor` appears anywhere; it also asserts `offset`/`limit` are
  advertised for the three tools.
- `tests/mcp_tool_contract.rs::stable_error_codes_include_addressing_failures_and_drop_cursor_mismatch`
  pins both new codes and pins that no variant stringifies to `cursor_mismatch`.
- `tests/static_guards.rs::no_retired_replay_cursor_in_mcp_tree_or_docs` walks all 28 files under
  `src/mcp/**` and asserts the literal `cursor` is absent, then asserts the seven doc sites carry
  neither `next_cursor` nor `cursor_status`. The only remaining `cursor` strings in the tree are
  the Cursor IDE integration under `src/integrations/**` and its docs.

### Re-pinned `tools/list` fingerprints

`tests/mcp_tool_contract.rs::full_preset_advertisement_is_byte_identical_to_the_pinned_surface`
was re-pinned, not waived: full+full `8f2f6953a1fa6eb2` (74,422 bytes, +1,005 over the pre-M020
73,417) and full+compact `32f58c1c4cc33de3` (20,196 bytes, +378). The compact-definition slimming
guards were re-pinned with the delta recorded in their assertion messages: four search schemas
12,818 of 12,850 bytes, all compact definitions 74,411 of 74,450 bytes, `research_search` property
count 16. `src/mcp/output_schema.rs::output_schemas_stay_compact` still asserts the unchanged
1,200-byte per-schema ceiling and passes.

### First-slice precision (recorded, not asserted)

Corpus `tests/fixtures/fetch_windows/corpus.json` (11 synthetic cases, `schema_version` 1, bodies
under 25 KB): four window-edge cases, three multi-byte cases (including one Cyrillic HTML body and
one CJK body), one no-focus case, and one `text/plain` body that exceeds the default window. The
measurement reads the default addressed window (`offset: 0`, no `limit`, `focus` per case) and
records containment of the ground-truth answer span in `[text_from, text_to)`:

```
id                                             contained  answer_start  text_total   text_to
html-answer-inside-first-window                      yes            68         478       478
text-answer-inside-first-window                      yes           182         516       516
html-answer-far-past-window                           no         13050       14033     12000
html-answer-straddles-window-edge                     no         11940       12724     12000
html-answer-starts-at-window-edge                     no         12000       12783     12000
html-answer-ends-at-window-edge                      yes         11919       12701     12000
html-multibyte-answer-inside-window                  yes          4200        4831      4831
html-multibyte-answer-far-past-window                 no         12350       13117     12000
text-multibyte-answer-inside-window                  yes           760        1161      1161
text-no-focus-answer-inside-first-window             yes           240         620       620
text-body-exceeds-window-cap                         yes           900       11997     11997
```

First-window recall 7/11 = 63.6%; median chars-to-answer 4,200. The test
(`tests/extract_fetch_contract.rs::fetch_window_first_slice_precision_is_recorded`) asserts only
determinism and completeness — per case `text_from == 0`, `text_to > 0`, `text_to <= text_total`,
a `doc_` + 16-hex `doc_id`, two identical runs — and no recall or chars threshold, because a
threshold without a candidate-ranking change would be a requirement this milestone cannot
discharge. The corpus and its coordinate derivation are documented in
`tests/fixtures/fetch_windows/README.md`.

### Live two-window union (opt-in, run once)

`cargo test --locked --all-features --test web_fetch_integration -- --ignored offset_windows_live
--nocapture` was run against the real network and passed:

```
url=https://www.rfc-editor.org/rfc/rfc9110.txt doc_id=doc_bdacf44c950f8e6d text_total=11981 first=[0,4000)
first_chars=4000 second=[5000,9000) second_chars=4000
```

Both windows report the same `doc_id` and `text_total`, are disjoint, and stay inside
`text_total`; 4,000 scalars returned per 4,000-scalar window. The same run is the live evidence
for the named condition: RFC 9110 is a plain-text document far larger than 12,000 scalars, yet
`text_total` is 11,981 — the caller-window bound, not the document size. The record is mirrored in
`tests/fixtures/fetch_windows/README.md`.

### Transport-level check (`mcp stdio`)

The built binary was driven over a real client-owned stdio session (`initialize`, `tools/list`,
`tools/call`) by a throwaway Python client, so the contract was observed on the wire rather than
through the in-process test harness:

- `tools/list` advertises all ten tools; `web_fetch`'s input schema carries `offset` and `limit`
  with `required == ["url"]`, and neither its input nor its output schema contains `cursor`,
  `next_cursor`, or `cursor_status`.
- `web_fetch {offset: 0, limit: 25}` on `https://example.com/` returns
  `text_from=0, text_to=25, text_total=156, next_offset=25, doc_id=doc_c45cd26d8d5e9b58` with 95
  returned scalars. The 25-scalar slice occupies 95 scalars on the wire because the 70-scalar trust
  envelope is wrapped around it: framing is an output transform, exactly as the offsets invariant
  requires.
- The same URL without `offset`/`limit` returns none of the addressing keys.
- `limit: 0` returns `isError` with `code=invalid_semantic_value` and `repair.field="limit"`;
  `extract_mode: "metadata_only"` with `offset` returns `code=document_unavailable`.

## Gate results

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: clean.
- `cargo check --locked --no-default-features`: clean.
- `cargo test --locked --all-features --no-fail-fast`: 6194 passed, 0 failed, 28 ignored (exit 0)
- `cargo test --locked --features mock -- --list`: 5839 tests, 4 ignored.
- `python3 packaging/check-planning-consistency.py`: `planning-consistency: ok`.
- `./packaging/check-repo-hygiene.sh`: `repo-hygiene: ok`.
- `make dependency-policy`: `advisories ok, bans ok, licenses ok, sources ok`.
- `make check` does not complete on this workstation: `packaging/check-contract.sh` passes
  `workflow-pin-check`, `planning-consistency`, the eggpack qualifier contract and the egress
  qualification contract, then stops at `mavis-trash: unsupported OS: Plan9` (the helper misdetects
  the host). The missing coverage is the hosted gate, not a code check.
- **Hosted CI: run 43284917524 passed** on `189ee07` (19m15s) — the Ubuntu `ci` job (`make ci`,
  which is `make check`: fmt, clippy, no-default check, the full all-features suite, hygiene,
  dependency policy and packaging-check), the macOS canonical-verification and local-filesystem
  portability job, and the Windows `x86_64-pc-windows-msvc` all-features check. The only commits
  after that SHA are this record and one fixture README, so the code under CI is the code at
  `HEAD`.

## Recorded deviations from the plan

1. **`plan_mismatch` is a clamped-bound guard, not a set-identity guard.** The plan's matrix row
   "set or arguments changed between windows → `plan_mismatch`" is not implementable statelessly:
   with no transported token there is no anchor to compare a changed `items` array or query
   against, and ADR-0011 forbids echoing the plan key. The shipped rule is the other half of the
   same sentence — "the request I am being asked to index is not the request the window was cut
   from" — expressed as: an addressed request whose caller-supplied plan bound the server had to
   clamp is refused, because the plan key hashes the supplied bound while the window is cut over
   the clamped one. Reachable on all three tools (`focus_max_chars`, `max_items`/`max_total_chars`,
   `max_results`/`max_per_group`). A changed set is caller-observable through `total_groups`,
   item indices and group identities, and is listed as residual risk 4.
2. **`web_fetch` derives no plan key.** `max_chars` left the key in M019 and `limit`/`offset` are
   excluded by design, so after the change no request field can diverge from the served plan
   except `focus_max_chars`, which is checked by value. A derivation with no consumer would be
   dead code, so the guard is the value comparison and `PlanKey::WebFetch` exists for the unit-test
   surface only. `batch_fetch` and `research_search` keep their baseline plan-key field lists and
   use them as the comparison anchor.
3. **A terminal fetch failure is now `upstream_failed` instead of `internal`.** The plan's matrix
   requires a failed upstream to be `upstream_failed` and to be distinguishable from the two new
   codes; the previous mapping contradicted `docs/tool-matrix.md`'s rule that `internal_error` is
   reserved for server faults. Circuit-open and browser-profile states still map to `internal`.
4. **`workflow_coverage.found_roles` ordering was fixed in `src/core/evidence_postprocess.rs`.**
   The plan forbids `src/fetch/**` changes but says nothing about `src/core/**`; the determinism
   invariant is unachievable without it, and it is a membership-preserving reorder.
5. **`text_total` is document-scoped only for HTML and browser reads** (the named condition above).
   PDF and other non-HTML bodies inherit M019's caller-window bound.

## Residual risk

1. **`text_total` is window-scoped for PDF and non-HTML bodies.**
   `src/fetch/client.rs:675` calls `extract_pdf_text(&body, window_chars, ...)` (the budget is the
   argument at `:677`), and the non-HTML renderer at `:1074` consumes `pre_framing_text`, which is
   the window-bounded text. An `offset` window on such a document
   cannot address past that bound; `text_total` reports what is addressable, so no invariant is
   violated, but the capability is narrower than for HTML. Discharging it is a new milestone over
   `src/fetch/**`, not a corrective patch inside M020.
2. **A cache miss extracts the whole document even for a small window**, so CPU and memory per
   fetch rise, and `max_chars_cap` is now a real cost knob rather than only a validation ceiling.
3. **`doc_id` is a document identity, not a content hash.** Two different bodies at one URL under
   one title share it; the guarantee that a re-fetch yields the same bytes comes from
   `raw_content_hash` inside the derived cache key. A caller must re-read from `offset = 0` when
   `doc_id` or the extraction inputs change.
4. **Upstream result-set churn between collection windows is caller-observable, not an error.** A
   changed `items` array or a changed query yields a fresh canonical order; the caller detects it
   through `total_groups`, item indices and group identities.
5. **The recorded-deadline clamp is gone with the token.** A resumed read no longer inherits a
   recorded deadline; the surviving rule is the ceiling
   `min(caller timeout_ms, [config].timeout_ms)` on a fresh call.
6. **`research_search` cannot address past 14 groups.** The internal widening to
   `RESEARCH_PAGE_GROUPS` is required for document-scoped aggregates, so at most 13 kinds are ever
   non-empty and `limit`/`offset` are bounded at 14.
7. **`retrieval_summary.dimensions[].duration_ms` is clock-derived**, so two identical
   `research_search` calls do not produce identical `retrieval_summary` bytes. This is pre-existing
   telemetry rather than a window property — every structural member (dimension, role, state,
   counts, subquery ids) is window-invariant and asserted — but it means the determinism invariant
   holds for page composition and aggregates, not for measured durations.
8. **`next_actions` on the no-workflow `research_search` path is built from the sliced groups**, so
   it is window-dependent and deliberately excluded from the cross-window aggregate-equality
   assertion. It is not an aggregate, but it is not window-invariant either.
9. **`docs/codegg-integration.md` is 1,648 lines**, above the 1,600-line ordinary-file guideline
   before this change and still above it after; the change added one line to a stable-code list.
