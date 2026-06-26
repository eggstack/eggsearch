# web_fetch Phase 2: HTML Structural Renderer and Markdown Mode

## Objective

Replace flattened HTML extraction with a structure-preserving renderer that is useful to agents. Implement the currently reserved `extract_mode = markdown` behavior. Preserve the existing safety and compatibility contract: one explicit URL, no JavaScript, no crawling, bounded extraction, sanitation, and `external_untrusted` labeling.

The desired result is that agents can see headings, paragraphs, lists, tables, code blocks, blockquotes, definition lists, and link text instead of a single whitespace-normalized blob.

## Dependency on Phase 1

Do not start this phase until the structured document model from Phase 1 exists and tests pass. This phase assumes the response can carry document kind, render format, text format, outline, blocks, chunks, metadata, and truncation flags.

## Non-goals

Do not summarize page contents. Do not follow links. Do not execute JavaScript. Do not add a browser engine. Do not add model-based readability extraction. Do not add PDF support. Do not remove the legacy `text` field.

## Public behavior changes

`extract_mode = markdown` must become implemented instead of rejected.

Recommended behavior:

- `extract_mode = text`: keep returning readable bounded text in `text`; also return structured document blocks.
- `extract_mode = markdown`: return Markdown-rendered bounded text in `text`, set document text format to `markdown`, and include structured blocks.
- `extract_mode = metadata_only`: return metadata only; no body text, blocks, or chunks.

Do not require agents to specify `include_links` to see inline link text. `include_links` should only control the separate extracted links array.

## Module design

Create a renderer path under `src/fetch/render/` or split the existing `src/fetch/extract.rs` into focused renderer modules. Keep the implementation deterministic and small. The renderer should parse HTML with the existing `scraper` dependency, build `RenderedBlock` records first, then render plain text or Markdown from those blocks. Do not maintain separate extraction logic for text and Markdown if block rendering can be the shared source of truth.

## Content root selection

Prefer extracting from the first useful content root in this order: `main`, `article`, element with role main, `body`, then document root fallback. If a preferred root produces almost no text, fallback to `body` rather than returning a blank page.

## Elements to skip

Keep skipping `script`, `style`, `noscript`, `svg`, `nav`, `footer`, `header`, `form`, and `aside`. Also skip `template`, elements with `hidden`, and elements with `aria-hidden=true` when easy to detect.

Do not add fragile cookie-banner class-name heuristics in this phase unless tests cover them. Deterministic structural rules are preferred.

## Element rendering requirements

Headings: Map `h1` through `h6` to heading blocks with levels. Populate document outline from heading order. Use existing element ids as anchors when present; otherwise derive stable slugs from heading text and suffix collisions.

Paragraphs: Preserve paragraph boundaries. Do not collapse unrelated text into one paragraph. Markdown output should separate paragraphs with blank lines.

Lists: Map unordered and ordered lists into list-item blocks. Markdown output should use readable list syntax. Nested lists may be flattened in this phase if nesting metadata would be too invasive, but item boundaries must remain visible.

Code and preformatted blocks: Preserve `pre`, `pre code`, and clear block-level code. Do not normalize whitespace inside code blocks. Preserve newlines and indentation. Detect simple language classes such as `language-rust`, `lang-rs`, and common highlight classes. Inline code inside paragraphs can be represented with backticks in Markdown.

Tables: Map tables to table blocks. Render simple tables as Markdown pipe tables. If a table is irregular, use a readable fallback and add a warning that table rendering used fallback text.

Blockquotes: Map blockquotes to quote blocks. Markdown output should use `>` prefixes.

Definition lists: Preserve `dl`, `dt`, and `dd` as definition blocks or paragraph fallback while keeping term-definition pairing where straightforward.

Links: Inline Markdown links should resolve against the final URL when possible. Separate link extraction remains controlled by `include_links`.

## Bounding and truncation

Apply character bounds after block rendering. Prefer truncating at block boundaries. If a single block exceeds the remaining budget, truncate that block and mark text/block truncation. Preserve code block line boundaries when possible. Keep the existing streamed byte cap unchanged.

## Sanitation

All block text, chunk text, outline titles, and rendered legacy text must pass through the same untrusted-content sanitation policy. Avoid double-framing that makes block text noisy. The response must continue to warn that fetched content is external untrusted data.

## Tests

Add fixture-based tests for headings and outline entries, paragraph boundaries, unordered and ordered lists, pre/code whitespace preservation, language detection for at least Rust and Python, simple table Markdown rendering, blockquote rendering, chrome/script/style stripping, metadata-only body suppression, `max_chars` truncation, relative link resolution, and non-UTF-8 warning behavior.

Add MCP tests proving `extract_mode = markdown` works, default text mode remains compatible, and warnings still include the external-untrusted warning.

## Documentation updates

Update README and MCP docs to mark Markdown extraction as implemented. Explain that Markdown is a rendering mode, not summarization. Explain best-effort preservation of headings, code blocks, tables, lists, and links. Clarify that JavaScript-rendered pages still cannot be dynamically rendered.

## Acceptance criteria

Markdown mode works over MCP and CLI where CLI exposes fetch modes. Existing text mode tests still pass. HTML docs pages become substantially more readable for agents. Code blocks are not whitespace-flattened. Tables retain useful row/cell boundaries. No crawling, JavaScript execution, or summarization is introduced.

Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`, and `cargo test` before closing the phase.
