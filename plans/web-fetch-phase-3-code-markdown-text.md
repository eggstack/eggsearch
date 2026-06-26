# web_fetch Phase 3: Code, Markdown, and Plaintext Detection

## Objective

Improve non-HTML resource rendering so codegg agents can fetch raw source, Markdown, configs, diffs, patches, logs, release notes, and plain text without losing the structure that matters. Preserve exact newlines and line numbers for code-like resources. Parse Markdown enough to expose outline and blocks. Keep ordinary text readable.

This is likely the highest-value codegg phase because many agent-selected URLs are raw GitHub files, docs source, gists, manifests, API examples, JSON payloads, TOML/YAML configs, unified diffs, and patch files.

## Dependency on prior phases

Do not start this phase until Phase 1 added the structured document model and Phase 2 added block/chunk rendering plus Markdown-mode plumbing.

## Non-goals

Do not build parsers or compilers for each language. Do not add tree-sitter in this phase. Do not infer semantics beyond lightweight language and kind detection. Do not summarize code. Do not crawl imports, repository links, or references. Do not fetch GitHub API metadata; this remains URL fetch only.

## Detection strategy

Add a deterministic classifier, preferably in `src/fetch/detect.rs`. It should use content type, final URL path extension, host/path patterns, and lightweight content heuristics. It should return document kind, optional detected language, optional source extension, and whether line-preserving rendering should be used.

Recognize common code extensions: Rust, Python, JavaScript, TypeScript, Go, C, C++, Java, Kotlin, Scala, shell, SQL, HTML, CSS, Lua, Ruby, PHP, and Swift.

Recognize structured text/config extensions: Markdown, JSON, JSONL, TOML, YAML, XML, INI, cfg, conf, and env.

Recognize diffs and patches from `.diff`, `.patch`, and unified-diff markers such as `diff --git`, `---`, `+++`, and `@@`.

Recognize logs and plain text from `.log` and `.txt`, while still allowing heuristics to detect code-like content under `text/plain`.

## Content-Type handling

Treat `text/markdown` and `text/x-markdown` as Markdown. Treat `application/json`, `application/ld+json`, and `application/*+json` as JSON. Treat TOML/YAML media types as their corresponding document kinds when present. Treat `text/x-diff` and `text/x-patch` as diff/patch. Treat `text/plain` as a container that still needs URL and content heuristics.

Keep rejecting unsupported binary content. Do not accept arbitrary `application/octet-stream` unless URL extension and byte heuristics strongly indicate safe text and the content is valid or mostly valid UTF-8.

## Code renderer

For code, JSON, TOML, YAML, diffs, and patches, preserve exact line breaks. Do not use `split_whitespace`. Do not trim indentation.

For small files, one code/raw-text block with `line_start = 1`, `line_end = N`, and language metadata is acceptable. For larger files, split into line-bounded blocks by line count or character budget. Chunks should preserve line ranges. The heading path can be empty or include a simple file/language label.

In Markdown mode, legacy `text` may wrap code in a fenced block with language. In text mode, plain source text is preferable for compatibility. Structured blocks should always preserve language and line ranges where known.

## Markdown renderer

For Markdown resources, avoid flattening. Prefer adding a lightweight Markdown parser such as `pulldown-cmark` if dependency weight is acceptable. If avoiding a dependency, implement a conservative line-based parser for headings, fenced code blocks, blockquotes, lists, and paragraphs.

Markdown rendering must extract headings into outline, preserve fenced code blocks with language, preserve list and blockquote boundaries, preserve tables when straightforward, and keep raw Markdown table text as fallback when parsing is too much. Legacy `text` should stay close to original Markdown unless sanitation or bounding requires changes.

## JSON, TOML, YAML, and config handling

Do not deeply parse unless needed. The MVP is line-preserving output with kind and language metadata. Exact source is more useful to agents than pretty output. Optional JSON pretty-printing may be added only if small, valid, and accompanied by a warning that line positions no longer match the original.

## Diff and patch handling

Detect unified diffs and patches. Render them as line-preserving `diff`/`patch` documents with language set to diff when useful. Keep hunk headers visible. Do not parse hunks semantically in this phase unless trivial and well-tested.

## Plain text prose handling

For ordinary prose, preserve paragraph breaks and meaningful line breaks. Avoid aggressive whitespace collapse. Lines with indentation, tabs, or code-like markers should be left intact.

## Truncation behavior

For line-preserving documents, prefer truncating at line boundaries. Set `text_truncated` when max chars are exceeded. Set `blocks_truncated` when not all blocks are emitted. Include line ranges on blocks and chunks. If a single line exceeds the char budget, truncate the line and mark truncation.

## Tests

Add fixture tests for Rust source, Python indentation, TOML, JSON, Markdown headings, Markdown fenced code, unified diff/patch, logs/plain text, `application/json` without a `.json` extension, `text/plain` with a `.rs` path, metadata-only body suppression, and truncation at line boundaries.

Add MCP tests for representative raw source and Markdown responses.

## Documentation updates

Update README with supported document kinds: HTML, plain text, Markdown, common source code files, JSON/TOML/YAML, and diffs/patches. State that language detection is deterministic and best-effort.

## Acceptance criteria

Raw code is no longer whitespace-flattened. Code blocks include line ranges and language where detectable. Markdown files produce outline and structured blocks. Config files and diffs preserve line locality. Existing HTML behavior remains intact. No crawling, summarization, JavaScript execution, or repository API behavior is introduced.

Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`, and `cargo test` before closing the phase.
