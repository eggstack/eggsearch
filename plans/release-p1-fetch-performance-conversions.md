# P1 Plan: Fetch Performance, Low-Power Defaults, and Agent-Focused Conversions

Status: handoff plan
Priority: P1, release-adjacent
Area: `web_fetch`, `batch_fetch`, document rendering, low-power operation

## Problem

The fetch pipeline is featureful enough for agent use, but there are two near-term opportunities:

1. HTML fetch currently appears to perform block rendering more than once in the normal extraction path. This is preventable CPU work under concurrent agent fetch workloads.
2. Fetch conversion support should prioritize high-value developer/research formats such as notebooks, OpenAPI specs, RST, CSV/TSV, and structured XML.

The default install should remain conservative and safe, especially for low-power Raspberry Pi deployments. New conversions should be bounded and non-executing.

## Relevant Code

Primary files:

- `src/fetch/client.rs`
- `src/fetch/render/`
- `src/fetch/detect.rs`
- `src/fetch/limits.rs`
- `src/core/document.rs`
- `src/core/fetch.rs`
- `tests/fetch_safety.rs`
- existing fetch/render tests

Config files:

- `src/core/config.rs`
- `docs/config.md`

## Goals

1. Avoid duplicate HTML rendering in `web_fetch`.
2. Preserve existing response schema unless additive metadata is needed.
3. Add tests that prove rendering happens once for the HTML normal path if feasible.
4. Add low-power/conservative operational guidance or config preset.
5. Add bounded conversions for high-value agent formats.
6. Keep all conversions non-executing and byte/character bounded.

## Part A: Avoid Duplicate HTML Rendering

### Current Concern

The normal HTML path renders blocks to get title, description, text, warnings, and block structure. Later, when constructing the structured document, it calls the block renderer again. This wastes CPU and may allocate more than necessary.

### Implementation Plan

Refactor `FetchClient::fetch` to retain a single render result for HTML paths.

Suggested internal enum:

```rust
enum ExtractedBody {
    Html {
        title: Option<String>,
        description: Option<String>,
        rendered: render::blocks::RenderedHtml,
        warnings: Vec<String>,
        non_utf8: bool,
        links: Vec<Link>,
        links_seen: usize,
        links_truncated: bool,
    },
    Text {
        text: Option<String>,
        warnings: Vec<String>,
    },
    MetadataOnly { ... },
}
```

If introducing a larger enum is too invasive, use local `Option<RenderedBlocks>` storage:

```rust
let mut html_rendered_for_document = None;
```

When the first `render_blocks` call happens, store the rendered output and later reuse it for document construction.

### Required Behavior Preservation

- `ExtractMode::Text` returns legacy `text` as plain text.
- `ExtractMode::Markdown` returns markdown text.
- `ExtractMode::MetadataOnly` does not build a full document.
- Sanitization behavior remains unchanged: legacy fields receive Tier 1 plus optional framing/marker scan; document blocks receive Tier 1 only.
- `links_seen` and `links_truncated` behavior remains unchanged.
- `text_truncated`, `block_truncated`, and metadata remain correct.

### Tests

At minimum:

- Existing fetch tests remain green.
- Add a regression test that compares HTML fetch output before/after refactor for title, description, text, document blocks, outline, chunks, and truncation flags.
- If feasible, add a test-only renderer counter behind `#[cfg(test)]` or `#[cfg(feature = "mock")]` to assert one render call for the normal HTML path.

## Part B: Low-Power / Conservative Profile

### Motivation

`eggsearch` is intended to be usable by coding agents and on low-power devices. Default caps are reasonable, but an explicit low-power profile reduces operator guesswork.

### Options

Preferred minimal approach for this pass:

- Add documentation-only low-power config profile.
- Do not add new config schema unless necessary.

Suggested low-power profile:

```toml
[search]
default_max_results = 6
max_results_cap = 20
timeout_ms = 6000
multiquery_concurrency = 4
multiquery_provider_concurrency = 1

[fetch]
timeout_ms = 6000
max_bytes = 1000000
max_chars_default = 8000
max_chars_cap = 24000
batch_max_items = 4
batch_max_items_cap = 8
batch_max_chars_per_item = 8000
batch_max_total_chars = 24000
batch_max_total_chars_cap = 60000
batch_concurrency = 2
```

Optional code approach:

- Add `eggsearch config print-profile low-power` CLI if config CLI already exists or can be added cleanly.
- Otherwise defer CLI generation and keep docs-only.

### Acceptance Criteria

- Low-power profile documented in config docs or quickstart docs.
- No default behavior changes unless explicitly chosen.
- Tests confirm config parses and validates.

## Part C: Additional Fetch Conversions

### Priority 1: Jupyter notebooks (`.ipynb`)

Implement notebook extraction as JSON parsing, not execution.

Behavior:

- Detect by extension `.ipynb` or JSON with notebook markers (`nbformat`, `cells`).
- Extract markdown cells and code cells in order.
- Include cell boundaries in document blocks.
- Skip outputs by default to reduce noise and avoid embedded large/binary data.
- Optionally include text/plain outputs later behind config.
- Never execute code.

Document kind:

- Add `DocumentKind::Notebook` if the enum is stable and schema tests can be updated.
- Otherwise use `DocumentKind::Json` with metadata flag initially.

Tests:

- Minimal notebook with markdown and code cells.
- Notebook with large output is bounded/skipped.
- Invalid JSON falls back to text/JSON behavior or returns clear parse warning.

### Priority 2: OpenAPI / Swagger specs

Detect OpenAPI JSON/YAML by keys:

- `openapi`
- `swagger`
- `paths`
- `components`

Render as structured outline:

- API title/version.
- Paths and methods.
- Operation IDs.
- Schemas/components.

Tests:

- Minimal OpenAPI 3 JSON.
- Minimal Swagger 2 JSON.
- YAML form if YAML parsing is available; otherwise plain text with detection warning.

### Priority 3: CSV / TSV

Render bounded table previews:

- Header row.
- First N rows within char budget.
- Column count.
- Row count if cheaply determined within byte cap.

Do not load unbounded data beyond `max_bytes`.

Tests:

- CSV with header.
- TSV with header.
- Quoted CSV fields.
- Truncation flag when rows exceed budget.

### Priority 4: RST and AsciiDoc

Implement lightweight heading-aware extraction, not full rendering.

RST:

- Detect `.rst`.
- Extract section headings from underline-style headings.

AsciiDoc:

- Detect `.adoc` or `.asciidoc`.
- Extract `=`, `==`, `===` headings.

Tests:

- Basic headings.
- Code blocks remain line-preserved.

### Priority 5: XML

Detect common XML content types and extensions:

- `application/xml`
- `text/xml`
- `application/rss+xml`
- `application/atom+xml`
- `.xml`

Initial behavior:

- Treat as structured text with XML language hint.
- For RSS/Atom, extract title/link/summary entries if inexpensive and bounded.

Tests:

- Plain XML.
- RSS feed preview.
- Atom feed preview.

## Part D: Archive / Package Metadata Inspection

Defer full archive support. For agent use, initial safe metadata inspection may be useful, but arbitrary archive unpacking has risk.

Future-safe approach:

- Support only known registry package formats in a later plan.
- Enforce entry count, total uncompressed bytes, path traversal rejection, and binary skipping.
- Do not implement in this P1 pass unless all bounds are explicit.

## Acceptance Criteria

The P1 implementation is complete when:

- HTML normal fetch path reuses one block-render result.
- Existing fetch behavior and schema tests pass.
- Low-power config profile is documented and parse-tested.
- At least one high-value new conversion lands with tests, preferably `.ipynb` or OpenAPI.
- New conversions are bounded by existing byte/char caps.
- New conversions do not execute code, follow links, or crawl.
- `cargo test --all-features` and `cargo clippy --all-features -- -D warnings` pass.

## Non-Goals

- Do not add JavaScript execution.
- Do not add browser rendering.
- Do not add crawling.
- Do not default-enable PDF extraction.
- Do not unpack arbitrary archives in this pass.
- Do not widen private-network fetch defaults.
