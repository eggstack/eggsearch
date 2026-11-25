# M001 — Selector Scoping, Attributes, and Cache-Only Policy

Status: closed
Date: 2026-11-24
Subsystem: web acquisition expansion
Milestone class: capability
Baseline: `d932115d1f86af51710747d243455792ff63a6fb` on `docs/user-facing-polish-20261123`
Decision authority: `architecture/fetch.md` (bounded fetch, extraction identity); `architecture/mcp.md` (nine-tool surface). No new ADR: both options are additive fields on the existing `fetch` web item.

References: ADR-0001, ADR-0006, ADR-0011, ADR-0022; `plans/003-planning-process.md` §2.4; `src/core/fetch.rs`; `src/core/batch_fetch.rs`; `src/fetch/cache.rs`; `src/mcp/tools/fetch.rs`.

## Objective

Let a caller say which parts of one page matter and read named CSS attributes,
then optionally prove that a fetch made no network request at all. Both are
request-time controls; neither changes the tool surface.

## Current implementation evidence

- `WebFetchRequest` (`src/core/fetch.rs`) is the canonical web request;
  `BatchFetchItem::Web` (`src/core/batch_fetch.rs`) and `FetchWebItem`
  (`src/mcp/tools/fetch.rs`) mirror it, so one field addition touches three
  shapes plus the handlers and the Python schema.
- `ExtractMode` is `text`, `markdown`, or `metadata_only`.
- `FetchCachePolicy` is `default`, `bypass`, or `refresh`
  (`src/core/fetch.rs:282`).
- `ExtractionCacheKey` (`src/fetch/cache.rs`) carries extract mode, budget
  class, `include_links`, PDF pages and OCR policy, media, `renderer_version`
  (5), and sanitisation; it has no scoping component.
- `focus` already performs deterministic query-driven chunk selection and
  explicitly never traverses links; scoping is structural, not a replacement.
- `scraper = "0.20"` is a dependency, so selector parsing needs nothing new.

## Contract

New optional fields on every web request shape:

```text
include_selectors: [string]        max 16, each <= 256 bytes
exclude_selectors: [string]        max 16, each <= 256 bytes
attributes:        [{ selector, attribute }]   max 16, name <= 64 bytes
cache_policy:      "cache_only"    new variant
```

Semantics:

1. Selectors are parsed with `scraper` before any I/O. An invalid selector is
   an item error `invalid_selector` naming the index; the item never fetches.
2. Exclusion removes matching subtrees from the candidate DOM. Inclusion
   restricts the candidate set to matching subtrees, emitted in document order
   and deduplicated. Exclusion applies to the source DOM, not to the
   post-exclusion result.
3. Text and markdown modes honour scoping. `metadata_only` validates the
   selectors but reports `selectors_ignored_in_metadata_only`.
4. If inclusion matches nothing, the item succeeds with an empty body, a
   `selector_no_match` warning, and `matched_roots: 0`. It is never reported as
   an ordinary successful extraction of the whole page.
5. Attribute extraction returns
   `attributes: [{ selector, attribute, values: [{ value, ordinal }] }]`,
   sanitised as Tier-1, capped at 64 values per selector and 512 characters per
   value, in document order.
6. `cache_only` reads only a stored derived entry. It performs no HTTP, no
   browser launch, no challenge replay, and no egress hop. A miss is an item
   error `cache_only_miss` carrying the cache entry state. A stored entry is
   eligible regardless of caller freshness, and the response reports
   `cache_age_seconds` and `stale: true` when the entry is older than the
   ordinary freshness window.
7. `cache_only` with `render = "browser"` fails with
   `cache_only_render_conflict` instead of starting a browser.

## Bounds

Selector count and length as above; 16 attribute pairs; 64 values per pair;
512 characters per value; the ordinary `max_chars` and `max_total_chars`
budgets still apply. Scoping never increases the returned payload.

## Cache identity

Add `scoping_fingerprint: Option<u64>` to `ExtractionCacheKey`: FNV-1a over a
canonical serialisation of (sorted exclusions, ordered inclusions, ordered
attribute pairs), with the existing content-derived identity rules. Scoped and
unscoped requests never share an entry. `renderer_version` is bumped only if
unscoped extraction output changes; this plan does not change it.

## Ordered work packages

1. Types, validation, and caps in `core/fetch.rs`; mirror in
   `core/batch_fetch.rs`, `mcp/tools/fetch.rs`, and `mcp/tools/batch_fetch_item.rs`.
2. DOM scoping in a new `src/fetch/scope.rs`, wired before text and markdown
   extraction for both the HTTP and browser paths.
3. Attribute extraction in `src/fetch/attributes.rs` with sanitisation and caps.
4. `FetchCachePolicy::CacheOnly` through the client, cache, and browser
   preflight, with the conflict and miss errors.
5. Schema parity, tool-matrix text, and documentation.

## Verification

Focused: `cargo test --locked --features mock --test web_fetch_integration`;
`--test batch_fetch_retrieval`; `--test mcp_tools`; a new scoping unit suite
covering parse failure, ordering, deduplication, empty match, attribute caps,
and cache-key isolation. `cache_only` is asserted with a mock origin that fails
the test if any request arrives, plus a browser-unavailable case.
Broad: `make check`; `make docs-check`.

## Acceptance and stop conditions

- Scoped text and markdown output is a strict subset of the unscoped document
  for the same page, in document order.
- An invalid selector fails before any network call.
- `cache_only` produces zero requests to a mock origin and a typed miss.
- Scoped and unscoped entries do not collide.
- **Stop:** scoping cannot be applied without changing unscoped output, or the
  change requires a tool rename, a new tool, or a model call.

## Non-goals

- Readability-style "main content" heuristics; that is not deterministic.
- XPath, JavaScript, or attribute-value predicates beyond plain CSS selectors.
- Cache-only for repo items (already offline) or for screenshot requests.
- Any change to `focus` semantics.

## Closure evidence

Suite results on the exact candidate, a scoped-versus-unscoped fixture pair, the
zero-request cache-only proof, cache-key isolation evidence, schema parity
result, documentation diff, and limitations.
