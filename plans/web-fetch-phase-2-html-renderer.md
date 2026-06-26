# web_fetch Phase 2: HTML Structural Renderer and Markdown Mode

## Objective

Replace flattened HTML extraction with a structure-preserving renderer that is useful to agents. Implement the currently reserved `extract_mode = "markdown"` behavior. Preserve the existing safety and compatibility contract: one explicit URL, no JavaScript, no crawling, bounded extraction, sanitation, and `external_untrusted` labeling.

The result should let agents see document structure instead of a single whitespace-normalized blob. Headings, paragraphs, lists, tables, code blocks, and blockquotes should survive extraction in both `document.blocks` and Markdown-rendered `text` when requested.

## Dependency on Phase 1

This phase assumes `WebFetchResponse` has an optional structured `document` field with at least:

- `kind`
- `render_format`
- `text_format`
- `outline`
- `blocks`
- `chunks`
- `metadata`
- `text_truncated`
- `blocks_truncated`
- `links_truncated`

Do not start this phase until Phase 1 has landed and tests pass.

## Non-goals

- Do not summarize page contents.
- Do not follow links.
- Do not execute JavaScript.
- Do not add a browser engine.
- Do not attempt readability ML or LLM-based boilerplate removal.
- Do not add PDF support in this phase.
- Do not remove the legacy `text` field.

## Public behavior changes

`extract_mode = "markdown"` must become implemented instead of rejected.

Recommended behavior:

- `extract_mode = "text"`: preserve current compatibility by returning readable bounded text in `text`; also return structured `document.blocks` if not metadata-only.
- `extract_mode = "markdown"`: return Markdown-rendered bounded text in `text`; set `document.text_format = "markdown"`; include structured blocks.
- `extract_mode = "metadata_only"`: return metadata only; no body text, no body chunks.

Do not require agents to specify `include_links` to see inline link text. `include_links` should only control the separate `links` array.

## Renderer design

Create a renderer module, preferably under `src/fetch/render/` or `src/fetch/extract/html.rs`, depending on how much refactor is tolerable. Keep modules small.

Suggested module split:

```text
src/fetch/render/mod.rs
src/fetch/render/html.rs
src/fetch/render/markdown.rs
src/fetch/render/chunk.rs
```

If a smaller change is preferred, place the renderer beside the existing `extract.rs` first, then split later.

The HTML renderer should parse the document with the existing `scraper` dependency. It should build `RenderedBlock` records first, then render either plain text or Markdown from those blocks. Avoid maintaining two independent extraction paths.

## Element handling requirements

### Remove or skip non-content elements

Keep skipping:

- `script`
- `style`
- `noscript`
- `svg`
- `nav`
- `footer`
- `header`
- `form`
- `aside`

Also consider skipping common hidden/template elements when straightforward:

- `template`
- elements with `hidden`
- elements with `aria-hidden="true"`

Do not implement fragile class-name heuristics for cookie banners in this phase unless covered by tests. Deterministic structural rules are preferred.

### Main content root

Prefer extracting from the first matching content root:

1. `main`
2. `article`
3. `[role="main"]`
4. `body`
5. document root fallback

If `main` or `article` exists but produces very little text, fallback to `body`. This avoids blank docs pages caused by overly narrow selection.

### Headings

Map `h1` through `h6` to `BlockKind::Heading` with `level`. Populate `outline` from headings. Preserve heading order. Use an `anchor` when the heading or parent has an id, or derive a stable slug from heading text. If slug collision occurs, suffix with `-2`, `-3`, etc.

### Paragraphs

Map `p` and coherent text runs to `Paragraph`. Do not collapse unrelated text into one paragraph. Preserve paragraph boundaries in Markdown with blank lines.

### Lists

Map `ul`, `ol`, and `li`. It is acceptable to render each item as `BlockKind::ListItem` with text. Markdown output should use `- ` for unordered and `1. ` / incrementing numbers for ordered lists where feasible. Nested lists can be flattened in Phase 2 if nesting metadata is too much, but preserve readable item boundaries.

### Code and preformatted blocks

This is a high-priority requirement for codegg. Preserve:

- `pre`
- `pre code`
- standalone `code` blocks that are clearly block-level

Do not whitespace-normalize code block content. Preserve newlines and indentation. Detect language from classes like `language-rust`, `lang-rs`, `highlight-source-python`, or URL/page hints when simple. Put language in `RenderedBlock.language` when known.

Inline `code` inside paragraphs can remain backticked in Markdown text. In plain text mode it can be included as normal text.

### Tables

Map `table` to `BlockKind::Table`. Markdown output should produce a simple pipe table when rows/cells are parseable. If table structure is irregular, return a tab-separated or newline-separated fallback inside the table block and add a warning such as `html table rendered with fallback text format`.

### Blockquotes

Map `blockquote` to `BlockKind::BlockQuote`. Markdown output should prefix lines with `> `.

### Definition lists

Map `dl`, `dt`, and `dd` to `BlockKind::Definition` or paragraph fallback. Preserve term-definition pairing where straightforward.

### Links

Inline links in Markdown should render as `[text](url)` only when URL resolution succeeds and the text is non-empty. For empty link text, omit or render the URL as text. Separate extracted links remain controlled by `include_links`.

## Bounding and truncation

Apply bounds after rendering blocks, not by chopping raw HTML. Rules:

- Byte cap remains enforced while streaming.
- `max_chars` bounds the returned `text` and the aggregate block/chunk text.
- Prefer truncating at block boundaries when possible.
- If a single block exceeds the remaining budget, truncate that block text and mark `text_truncated = true` and/or `blocks_truncated = true`.
- Preserve code block line boundaries where possible.

## Sanitation

All untrusted text fields in blocks, chunks, outline titles, and rendered `text` must pass through the same sanitation policy as current title/description/text handling.

Avoid double-framing. If legacy `text` is framed, block text should not be independently framed unless the response contract explicitly expects framed block text. The safer approach is:

- Strip controls and bound all block/chunk fields.
- Scan for injection markers across rendered document text.
- Frame the top-level `text` as today when `sanitize_output = true`.
- Record trust markers at response/document level.

If implementation chooses to frame each block, tests must prove the result is still readable and not over-noisy for agents.

## Tests

Add fixture-driven unit tests for:

- Headings produce outline entries and heading blocks.
- Paragraph boundaries are preserved.
- Lists produce separate list item blocks and Markdown list syntax.
- `pre code` preserves exact newlines/indentation.
- Language class detection for at least Rust and Python.
- Tables render to Markdown pipe tables when simple.
- Blockquotes render with quote markers in Markdown mode.
- Nav/header/footer/sidebar/script/style/svg are stripped.
- `extract_mode = "markdown"` no longer returns validation error.
- Metadata-only does not produce body blocks.
- `max_chars` truncates without violating bounds.
- Links resolve against the final URL.
- Non-UTF-8 HTML continues to warn and lossy-decode as before.

Add MCP-level tests for:

- `web_fetch` with `extract_mode = "markdown"` returns `text` containing Markdown heading/code/table syntax.
- `web_fetch` with default mode remains backward-compatible enough for existing tests.
- `warnings` still include the external-untrusted warning.

## Documentation updates

Update README `web_fetch` section:

- Mark `markdown` as implemented.
- Explain that Markdown is a rendering mode, not summarization.
- Explain that code blocks and tables are preserved on a best-effort basis.
- Clarify that JavaScript-rendered pages still cannot be rendered dynamically.

Update any AGENTS or module docs that say Markdown is reserved.

## Acceptance criteria

- `extract_mode = "markdown"` works over MCP and CLI if CLI exposes fetch modes.
- Existing `text` mode tests still pass.
- HTML docs pages become substantially more readable for agents.
- Code blocks are not whitespace-flattened.
- Tables retain useful row/cell boundaries.
- No crawling, JS execution, or summarization is introduced.

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test
```
