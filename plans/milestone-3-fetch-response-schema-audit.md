# Milestone 3 Plan: Fetch Response Schema and Raw Text Exposure Audit

## Objective

Audit and tighten fetch-related response schemas so agent-facing output is bounded, predictable, and explicit about the distinction between rendered text, structured document blocks, chunks, and internal raw text.

This milestone should preserve current useful behavior, especially repo line/span selection, while preventing raw extraction fields from becoming an accidental token-budget or trust-boundary footgun.

## Scope

In scope:

- `web_fetch` response shape;
- `batch_fetch` response shape;
- `repo_fetch` response shape;
- `build_evidence_bundle` use of fetched items;
- `WebFetchResponse` fields;
- `FetchDocument` fields;
- `raw_text`, `text`, `document.blocks`, and `document.chunks` truncation behavior;
- prompt-injection framing and trust marker behavior for all fetch text paths;
- PDF response/error/truncation behavior;
- docs and tests describing fetch text budgets.

Out of scope:

- changing fetch from explicit single URL to crawler;
- adding JS rendering;
- replacing the current renderer architecture;
- adding a full content-addressed cache;
- adding binary extraction beyond current optional PDF support.

## Relevant Code Areas

Primary files to inspect:

- `src/core/fetch.rs`
- `src/core/document.rs`
- `src/core/batch_fetch.rs`
- `src/core/repo_fetch.rs`
- `src/core/evidence_bundle.rs`
- `src/fetch/client.rs`
- `src/fetch/render/*`
- `src/fetch/pdf.rs`
- `src/mcp/*` tool handlers for fetch/repo/evidence tools
- `src/meta/evidence_bundle.rs`
- `tests/fetch_safety.rs`
- `tests/evidence_bundle_handoff.rs`
- repo-fetch integration tests
- docs for fetch, safety, tool matrix, and agent workflows

## Current Problem Statement

The fetch client intentionally keeps a larger internal `raw_text` budget bounded by `max_chars_cap`. This supports internal consumers like `repo_fetch`, which may need to select a target line or span even when a caller requested a smaller display budget.

That design is useful. The release risk is ambiguity. If MCP responses expose `raw_text` by default without precise metadata, agent outputs can become larger than expected and trust/framing semantics can become unclear. A caller should always know whether a field is rendered for display, raw internal extraction text, structured block text, or chunk text, and each should have an explicit cap/truncation signal.

## Design Requirements

### 1. Preserve line/span selection capability

Do not remove the internal ability for `repo_fetch` to select lines from a larger raw source body. This behavior exists for a reason. The goal is to control and document exposure, not regress functionality.

### 2. Distinguish text classes

The schema and docs should distinguish:

- `text`: bounded rendered tool output for normal agent consumption;
- `raw_text`: raw decoded/extracted text, if public at all;
- `document.blocks[].text`: structured block text for navigation and citation-like use;
- `document.chunks[].text` or chunk references: bounded chunk text for downstream selection;
- PDF extracted text: document text from binary extraction, not HTML/text decode.

### 3. Add metadata if raw text remains public

If `raw_text` is retained in agent-facing MCP output, add or expose metadata that makes its budget explicit:

- `raw_text_chars_returned`;
- `raw_text_truncated`;
- `raw_text_cap`;
- optionally `raw_text_source = "decoded_body" | "rendered_text" | "pdf_extraction"`;
- docs stating raw text is intended for internal line/span selection and should not be treated as instructions.

If a metadata field already exists under a different name, reuse it. Do not duplicate state unnecessarily.

### 4. Consider omitting raw text from default MCP output

Preferred production posture: keep raw text available internally but avoid returning it in default `web_fetch` responses unless requested via an explicit mode or config. If changing this is too invasive for the current release, preserve output but add strict metadata and tests.

Possible designs:

- keep `raw_text` in Rust internal type but skip it in serialized MCP output by default;
- add request field `include_raw_text: bool`, default false;
- add `extract_mode: "raw"` or a separate advanced mode, if compatible with existing schema;
- leave `raw_text` public but document and bound it explicitly.

For a release-tightening pass, prefer the least disruptive change that eliminates ambiguity.

### 5. Ensure trust markers cover all public text fields

Prompt-injection scanning and framing already apply to key fields. Audit whether any public raw/document/chunk text bypasses trust markers or warnings. It is acceptable for structured document blocks to be Tier-1 sanitized but unframed if the docs explicitly say that the document object is external untrusted and the response-level trust markers apply. It is not acceptable for public text to be completely unsanitized and unbounded.

### 6. Make truncation machine-readable

Every public text-bearing path should have machine-readable truncation state. Existing fields such as `truncated`, `text_truncated`, `block_truncated`, and `link_truncated` should be audited for consistency.

A caller should be able to answer:

- was the HTTP body truncated by byte cap?
- was rendered text truncated by char cap?
- were document blocks truncated?
- were links truncated?
- was raw text truncated by raw cap?
- was PDF extraction truncated by page, per-page, or total cap?

### 7. Keep metadata-only cheap and predictable

`metadata_only` should not perform expensive extraction beyond what is necessary for title/description/metadata. For PDFs, metadata-only should avoid text extraction. Tests should lock this behavior where feasible.

## Implementation Steps

### Step 1: Inventory fetch response types and serialization

Trace response structs from core type to MCP output:

- `WebFetchResponse`;
- `BatchFetchResponse` and `BatchFetchResult`;
- `RepoFetchResponse`;
- evidence bundle fetched item types;
- document/chunk structs.

Record which fields are serialized and which are internal-only. If there is no explicit internal-only boundary, identify where to add one.

### Step 2: Add an internal schema audit note

Create a short comment or test fixture documenting intended field semantics. This does not need to be public docs yet, but the implementation should have an authoritative expectation.

Example table:

| Field | Intended consumer | Cap | Framed? | Trust level | Truncation field |
|-------|-------------------|-----|---------|-------------|------------------|
| `text` | agent display | request max | yes when sanitize enabled | external_untrusted | `text_truncated`/`truncated` |
| `raw_text` | internal selection | config cap | no or yes, decide | external_untrusted | `raw_text_truncated` |
| `document.blocks` | navigation | request max/block cap | no, Tier 1 only | external_untrusted | `block_truncated` |

### Step 3: Decide raw text public behavior

Choose one of these paths:

#### Option A: Hide raw text by default

- Keep `raw_text` internal in Rust.
- Add serialization skip or construct MCP response with `raw_text: None` unless explicitly requested.
- Ensure `repo_fetch` still receives internal raw text before serialization.

#### Option B: Gate raw text explicitly

- Add request field `include_raw_text: Option<bool>` with default false.
- Return raw text only when requested.
- Add docs warning about token budget and trust.

#### Option C: Keep raw text public but fully annotate

- Add metadata fields for cap and truncation.
- Ensure docs describe it clearly.
- Add tests that assert raw text never exceeds cap.

For release tightening, Option C is simplest if compatibility matters. Option A or B is safer if output compatibility is not yet public.

### Step 4: Add raw text metadata

If raw text remains visible or request-gated, add fields to the relevant response type or document metadata. Prefer response-level fields if raw text is response-level.

Possible fields:

```rust
pub raw_text_chars_returned: Option<usize>,
pub raw_text_truncated: bool,
pub raw_text_cap: Option<usize>,
```

Use `Option` if not all modes produce raw text. If raw text is absent in `metadata_only`, metadata should be absent or zero consistently.

### Step 5: Audit `truncated` naming

The current top-level `truncated` field may refer to byte/body truncation or extracted text truncation depending on path. Verify actual semantics. If ambiguous, add more precise fields while preserving existing `truncated` for compatibility.

Possible additions:

- `body_truncated`;
- `text_truncated`;
- `raw_text_truncated`.

Do not remove existing fields in this pass unless tests and docs are updated thoroughly.

### Step 6: Audit structured document block sanitation

Review block construction in `src/fetch/client.rs` and renderers. Confirm:

- control characters are stripped;
- block text is bounded;
- block count/chunk count is bounded;
- document outline strings are bounded;
- metadata strings such as charset/source extension are bounded or derived from safe parsers;
- chunk text does not exceed intended budget.

Add tests for malicious/injection-like content in:

- HTML title;
- HTML body;
- plain text;
- markdown source;
- JSON/source code;
- document blocks;
- chunks if public.

### Step 7: Audit `batch_fetch`

`batch_fetch` has per-item and total budgets. Verify:

- total response text cannot exceed configured cap through `raw_text` fields;
- failed items do not include unbounded error strings;
- item-level truncation is visible;
- total-budget exhaustion is visible;
- concurrency does not bypass total cap.

Add tests where multiple items each have large bodies and confirm aggregate output stays bounded.

### Step 8: Audit `repo_fetch`

Verify repo fetch can still return a target line/span beyond default display cap. Existing tests may already cover this. Add/update tests for:

- target line beyond normal `max_chars_default`;
- line selection uses internal raw text but output is bounded;
- selected span includes stable line numbers;
- raw backing text is not accidentally returned in addition to selected span unless explicitly requested.

### Step 9: Audit evidence bundles

Evidence bundles should be deterministic and bounded. Verify fetched item payloads do not duplicate large raw text plus rendered text plus document blocks unless intended.

Add tests that build an evidence bundle from large fetch inputs and assert:

- total chars <= configured bundle cap;
- truncation/gap entry is present when content is omitted or truncated;
- source/fetch IDs remain stable.

### Step 10: Audit PDF behavior

For `pdf` feature builds, verify response states are distinct:

- PDF feature not compiled;
- PDF compiled but `[fetch].pdf_enabled = false`;
- unsupported content type;
- blank/no extractable PDF text;
- page limit reached;
- per-page char limit reached;
- total char limit reached;
- metadata-only PDF skips expensive extraction.

Where the current extractor cannot distinguish all truncation types, add at least clear warning strings or structured warning codes if available.

### Step 11: Update docs

Update docs to include:

- fetch mode table: `metadata_only`, `text`, `markdown`;
- `text` versus `document` versus `raw_text` semantics;
- budget fields and caps;
- truncation fields;
- PDF feature and config requirements;
- trust/sanitization warning that all remote content remains external untrusted.

Update README only if needed for a concise summary. Put deep material in `docs/safety.md` or a fetch-specific doc.

## Testing Requirements

Targeted tests:

```bash
cargo test --all-features fetch
cargo test --features mock --test fetch_safety
cargo test --features mock --test evidence_bundle_handoff
cargo test --all-features --test docs_config_snippets --test docs_tool_names
```

Then run broader release checks:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --no-default-features
```

Run `make check` if available.

## Regression Risks

### Risk: Breaking clients expecting `raw_text`

If hiding or gating `raw_text`, consider whether external users already depend on it. Since this is a pre-release hardening line, breaking unstable behavior may be acceptable, but document it in the changelog.

### Risk: Regressing repo_fetch span selection

This is the main behavior to protect. Add tests before changing raw text exposure.

### Risk: Duplicating text and increasing output size

Be careful that adding metadata does not cause `web_fetch`, `batch_fetch`, and evidence bundle outputs to include multiple copies of the same large text.

### Risk: Confusing truncation flags

Avoid reusing one `truncated` flag for multiple meanings. If preserving old fields, add precise new fields and document legacy behavior.

## Deliverables

- Fetch response schema audit completed in code/tests/docs.
- Clear decision on `raw_text` public/default behavior.
- Raw text cap/truncation metadata if raw text remains public or request-gated.
- Tests for bounded output across `web_fetch`, `batch_fetch`, `repo_fetch`, and evidence bundle paths.
- Tests preserving repo line/span selection beyond default display cap.
- PDF behavior tests and docs updates.
- Updated docs explaining fetch text classes and budgets.

## Definition of Done

This milestone is complete when every public fetch text field is bounded and documented, raw text exposure is intentional rather than accidental, truncation is machine-readable, repo_fetch keeps its large-source span-selection capability, and tests prove default tool outputs cannot unexpectedly exceed their configured budgets.
