# Web Acquisition M001 — Selector Scoping, Attributes, and Cache-Only — Status

Status: closed
Date: 2026-11-24
Milestone: M001 — Selector Scoping, Attributes, and Cache-Only Policy
Decision authority: `architecture/fetch.md`; `architecture/mcp.md`
Baseline: `d932115d1f86af51710747d243455792ff63a6fb`

## Candidate and changes

Local candidate: the baseline plus this change set. The work is additive on the
existing `fetch` web item: three optional request fields, one new
`FetchCachePolicy` variant, a scoping fingerprint in the derived extraction
key, and additive response fields. No tool was added or renamed.

- `src/core/fetch.rs`, `src/core/batch_fetch.rs`: `include_selectors`,
  `exclude_selectors`, `attributes`, `FetchCachePolicy::CacheOnly`, and the
  `attributes` / `matched_roots` / `cache_age_seconds` / `stale` response
  fields.
- `src/fetch/scope.rs`: pre-I/O selector validation, caps, inclusion-root
  deduplication, source-tree exclusion, and the FNV-1a scoping fingerprint.
- `src/fetch/attributes.rs`: bounded, Tier-1 sanitised attribute extraction.
- `src/fetch/render/blocks.rs`: a separate scoped render entry point; the
  unscoped `render_blocks` path is unchanged and still the default.
- `src/fetch/cache.rs`: `scoping_fingerprint` joins the extraction key;
  `renderer_version` remains 5.
- `src/mcp/tools/web_fetch.rs`, `src/mcp/tools/fetch.rs`,
  `src/mcp/tools/batch_fetch_item.rs`, `src/mcp/tools/common.rs`: wiring,
  typed item errors (`invalid_selector`, `cache_only_miss`,
  `cache_only_render_conflict`), and cached-response projection.
- `bindings/python/python/eggsearch/types.py`,
  `bindings/python/tests/test_native.py`: additive TypedDict parity.

## Requirement-to-evidence matrix

| Criterion | Evidence | Outcome |
|---|---|---|
| Selectors parsed before any I/O | `web_fetch_invalid_selector_fails_before_io` (`tests/web_fetch_integration.rs:3052`) and `invalid_selector` assertion in `tests/batch_fetch_retrieval.rs:548` | pass |
| Scoped output is a document-order subset | `selector_inclusion_exclusion_and_attribute_values_are_bounded`; `inclusion_roots_are_ordered_deduplicated_and_exclusions_use_source_tree` | pass |
| Empty inclusion match | `matched_roots == 0` with `selector_no_match`; body carries only the standard Tier-2 empty frame | pass |
| Attribute bounds and sanitisation | 16 pairs, 64 values/pair, 512 chars/value asserted in the scoping regression | pass |
| `cache_only` performs no network | Mock origin with a request-count guard; typed `cache_only_miss` on miss | pass |
| `cache_only` + browser | `cache_only_render_conflict` before browser I/O; batch preflight covered | pass |
| Cache isolation | `scoping_fingerprint` in `ExtractionCacheKey`; scoped and unscoped keys differ; `renderer_version` still 5 (asserted in `src/fetch/cache/tests.rs`) | pass |
| Schema parity | Three Rust request shapes plus the Python TypedDict accept the same optional keys | pass |

## Verification on this candidate

| Command | Outcome |
|---|---|
| `cargo test --locked --features mock --test web_fetch_integration` | pass: 130 passed, 1 ignored |
| `cargo test --locked --features mock --test mcp_tools` | pass: 29 passed |
| `cargo test --locked --features mock --test batch_fetch_retrieval` | pass: 30 passed |
| `cargo test --locked --all-features --lib fetch::` | pass: 722 passed |
| `cargo fmt --check` | pass |
| `RUSTUP_TOOLCHAIN=1.89 make check` | pass: fmt, clippy, no-default check, all-features tests, hygiene, dependency policy, packaging |
| `make docs-check` | pass |

The MCP content-equivalence test now normalises `cache_age_seconds` alongside
`cache_status`: a cold response reports `null` and a cache hit reports `0`.
The compared content fields are unchanged.

## Limitations

- Unscoped output is preserved by construction (a separate scoped entry point)
  and by the existing unit and integration suites; no separate before/after
  byte-for-byte baseline capture was recorded.
- Local Clippy (rustc 1.100.0) reports pre-existing warnings in untouched
  modules only; the repository MSRV is 1.89 and no warnings originate in the
  files this milestone changed.
- `cache_only` is scoped to the `fetch` web item. Repo items are already
  offline and screenshots are not cache-eligible.
