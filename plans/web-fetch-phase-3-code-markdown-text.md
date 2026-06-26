# web_fetch Phase 3: Code, Markdown, and Plaintext Detection

## Objective

Improve non-HTML fetch rendering so codegg agents can fetch raw source, Markdown, configs, diffs, patches, logs, release notes, and plain text without losing the semantics that matter. The phase should preserve exact newlines and line numbers for code-like resources, parse Markdown enough to expose outline/blocks, and keep ordinary text readable.

This phase is the highest-value codegg-specific improvement because many agent-selected URLs are raw GitHub files, docs source, gists, manifests, API examples, JSON payloads, TOML/YAML configs, unified diffs, and patch files.

## Dependency on prior phases

This phase assumes:

- Phase 1 added the structured document model.
- Phase 2 added block/chunk rendering and Markdown-mode plumbing.

Do not start this phase if `document.blocks`, `document.chunks`, and text/document truncation metadata are not present.

## Non-goals

- Do not build a parser or compiler for each language.
- Do not add tree-sitter in this phase.
- Do not infer semantics beyond lightweight language/kind detection.
- Do not summarize code.
- Do not crawl import links or repository references.
- Do not fetch GitHub API metadata; this remains URL fetch only.

## Detection strategy

Add a deterministic classifier that uses, in order:

1. Content-Type header.
2. Final URL path extension.
3. Final URL host/path patterns.
4. First-pass content heuristics.

The classifier should return:

- `DocumentKind`
- optional `detected_language`
- optional `source_extension`
- text handling policy: preserve lines vs prose normalization

Recommended module:

```text
src/fetch/detect.rs
```

Recommended function shape:

```rust
pub fn detect_document_kind(
    final_url: &str,
    content_type: Option<&str>,
    body_prefix: &[u8],
) -> DocumentDetection
```

Where `DocumentDetection` contains `kind`, `language`, `extension`, and maybe `confidence` if useful.

## Extension/language mapping

Start with a practical deterministic table:

### Code

- Rust: `.rs`
- Python: `.py`, `.pyi`
- JavaScript/TypeScript: `.js`, `.jsx`, `.ts`, `.tsx`, `.mjs`, `.cjs`
- Go: `.go`
- C/C++: `.c`, `.h`, `.cc`, `.cpp`, `.cxx`, `.hpp`
- Java/Kotlin/Scala: `.java`, `.kt`, `.kts`, `.scala`
- Shell: `.sh`, `.bash`, `.zsh`, `.fish`
- SQL: `.sql`
- HTML/CSS: `.html`, `.htm`, `.css`, `.scss`
- Lua: `.lua`
- Ruby: `.rb`
- PHP: `.php`
- Swift: `.swift`

### Structured text/config

- Markdown: `.md`, `.markdown`, `.mdown`, `.mkd`
- JSON: `.json`, `.jsonl`
- TOML: `.toml`
- YAML: `.yaml`, `.yml`
- XML: `.xml`
- INI/config/env: `.ini`, `.cfg`, `.conf`, `.env`

### Diffs and patches

- `.diff`
- `.patch`

### Logs/plain text

- `.log`
- `.txt`

Do not overfit. A short table is better than a brittle mega-table.

## Content-Type handling

Recognize:

- `text/markdown` and `text/x-markdown` as Markdown.
- `application/json`, `application/ld+json`, and `application/*+json` as JSON.
- `application/toml` and `text/toml` as TOML if present.
- YAML media types where present.
- `text/x-diff`, `text/x-patch`, or patch-looking plain text as Diff/Patch.
- `text/plain` as a container that still requires URL/heuristic classification.

Keep existing behavior that rejects unsupported binary content. Do not accept arbitrary `application/octet-stream` unless URL extension and byte heuristics strongly indicate safe text and UTF-8/mostly-UTF-8 content.

## Line-preserving code renderer

For `DocumentKind::Code`, `Json`, `Toml`, `Yaml`, `Diff`, and `Patch`, preserve exact line breaks. Do not use `split_whitespace`. Do not trim indentation.

Recommended block model:

- For small files, one `Code` or `RawText` block with `line_start = 1`, `line_end = N`, and `language` if known.
- For larger files, split into line-bounded blocks, e.g. 120-250 lines per block or by char budget.
- Chunks should preserve line ranges and heading path can be empty or include file/language labels.

Recommended legacy `text` rendering for code:

```text
```rust
// original lines preserved
fn main() {}
```
```

Only use fenced Markdown in legacy `text` when `extract_mode = "markdown"`. In `text` mode, plain source text may be preferable for compatibility. The structured blocks should always preserve language and line ranges.

## Markdown renderer

For Markdown resources, avoid flattening. Minimal parser options:

1. Add a Markdown parser dependency such as `pulldown-cmark`, if acceptable.
2. Implement a conservative line-based parser for headings, fenced code blocks, blockquotes, lists, and paragraphs.

Prefer `pulldown-cmark` if dependency weight is acceptable. It is common and lightweight enough for this purpose. If used, add it without default-heavy extras and document why.

Markdown rendering requirements:

- Extract headings into outline.
- Preserve fenced code blocks with language info.
- Preserve lists and blockquotes.
- Preserve tables only if supported; otherwise keep raw Markdown table text as a table/raw block.
- Keep legacy `text` close to original Markdown unless sanitation/bounding requires changes.

## JSON/TOML/YAML handling

Do not parse deeply unless needed. The MVP is line-preserving output with kind/language metadata.

Optional nice-to-have if low risk:

- Pretty-print minified JSON only when it is valid JSON and small enough.
- Add a warning if JSON was pretty-printed because line positions no longer match original bytes.

Default should preserve original text and line positions. Agent correctness often benefits from exact source more than pretty output.

## Diff and patch handling

Detect unified diffs by lines starting with:

- `diff --git `
- `--- `
- `+++ `
- `@@ `

Render as `DocumentKind::Diff` or `Patch`, `language = "diff"`, line-preserving. Do not parse hunks semantically in this phase unless trivial. Keep hunk headers visible.

## Plain text prose handling

For ordinary prose/plain text, keep line breaks where they appear meaningful. Avoid aggressive whitespace collapse. A safe approach:

- Preserve paragraph breaks.
- Collapse runs of spaces inside prose lines only if not code-like.
- Avoid changing lines that have indentation, tabs, or monospace/code markers.

## Truncation behavior

For line-preserving documents:

- Prefer truncating at line boundaries.
- Set `text_truncated = true` when max chars are exceeded.
- Set `blocks_truncated = true` if not all blocks are emitted.
- Include `line_start` and `line_end` on blocks and chunks.
- If a single line exceeds max_chars, truncate that line and mark truncation.

## Tests

Add fixtures and tests for:

- Rust raw source URL/path detects `DocumentKind::Code`, language `rust`, and preserves line numbers.
- Python source preserves indentation.
- TOML detects as TOML and preserves line structure.
- JSON detects as JSON.
- Markdown detects as Markdown and populates outline from headings.
- Markdown fenced code preserves language.
- Unified diff detects as Diff/Patch and preserves hunk headers.
- Logs/plain text remain readable and bounded.
- `application/json` content type works even without `.json` extension.
- `text/plain` with `.rs` path is code, not prose.
- `metadata_only` does not leak body lines.
- Truncation prefers line boundaries.

Add MCP tests for representative raw source and Markdown responses.

## Documentation updates

Update README `web_fetch` section with a short table of supported document kinds:

- HTML
- plain text
- Markdown
- common source code files
- JSON/TOML/YAML
- diffs/patches

State clearly that language detection is deterministic and best-effort.

## Acceptance criteria

- Raw code is no longer whitespace-flattened.
- Code blocks include line ranges and language where detectable.
- Markdown files produce outline and structured blocks.
- Config files and diffs preserve line locality.
- Existing HTML behavior from Phase 2 remains intact.
- No crawling, summarization, JS execution, or repository API behavior is introduced.

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test
```
