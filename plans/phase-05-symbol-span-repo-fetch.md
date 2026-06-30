# Phase 5 Plan: Symbol/Span-Aware Repository Fetch

## Objective

Extend `repo_fetch` so coding agents can fetch the relevant source block around a symbol, match text, or provider text-match span without manually guessing line numbers. The current `repo_fetch` file/range interface is correct and should remain, but codegg often knows a symbol or search match rather than a stable line range.

This phase should add deterministic span expansion and symbol-aware fetch options while preserving bounded output and trust labeling.

## Rationale

Coding-agent workflows often look like:

- `repo_search` finds `src/routing/mod.rs` and indicates a matched symbol.
- The agent needs the enclosing function/impl block, not the whole file.
- The result has line anchors or text-match fragments, but not always enough context.
- The agent should call `repo_fetch` with structured fields and receive a bounded, line-numbered source span.

Without symbol/span-aware fetch, agents either over-fetch whole files or under-fetch narrow line ranges that omit the enclosing context.

## Scope

In scope:

- Add optional symbol/match fields to `RepoFetchRequest`.
- Add deterministic block expansion for common languages and config formats.
- Use existing `line_start`, `line_end`, `context_before`, and `context_after` behavior when supplied.
- Convert code evidence match/context line metadata into better structured fetch locators.
- Return metadata describing how the final span was selected.
- Keep all output bounded by `max_chars` and existing fetch caps.

Out of scope:

- Full parser correctness for every language.
- Tree-sitter dependency unless it is already acceptable and small enough; start with heuristics.
- Repository-wide symbol index; local symbol search is Phase 6 or later.
- Fetching multiple files in one `repo_fetch` call.

## Request extensions

Add optional fields to `RepoFetchRequest`:

```rust
pub symbol: Option<String>,
pub symbol_kind: Option<SymbolKind>,
pub match_text: Option<String>,
pub expand_to_block: Option<bool>,
pub max_block_lines: Option<usize>,
```

Use serde defaults and skip-empty serialization. Existing callers that provide only file/range fields should see identical behavior.

Suggested behavior precedence:

1. If explicit `line_start`/`line_end` are provided and `expand_to_block != true`, preserve current range behavior.
2. If explicit line range plus `expand_to_block = true`, expand from that range to the enclosing block when possible.
3. If `symbol` is provided, scan the fetched file for a likely definition or declaration, then expand to block.
4. If `match_text` is provided, find the first or best match and expand around it.
5. If no range, symbol, or match is supplied, preserve current whole-file bounded fetch behavior.

## Response extensions

Add optional span-selection metadata to `RepoFetchResponse`:

```rust
pub selected_span: Option<RepoSelectedSpan>
```

Suggested type:

```rust
pub struct RepoSelectedSpan {
    pub line_start: usize,
    pub line_end: usize,
    pub selection_kind: String,
    pub symbol: Option<String>,
    pub symbol_kind: Option<SymbolKind>,
    pub confidence: String,
    pub reasons: Vec<String>,
    pub expanded: bool,
    pub truncated_by_max_block_lines: bool,
}
```

`selection_kind` examples:

- `explicit_range`
- `expanded_explicit_range`
- `symbol_definition`
- `symbol_reference`
- `match_text`
- `whole_file_bounded`

Confidence examples:

- `exact`
- `strong`
- `weak`
- `unknown`

## Block expansion heuristics

Start with deterministic heuristics. Keep them isolated in a module such as `src/fetch/span.rs` or `src/core/repo_span.rs`.

### Rust

Detect definitions:

- `fn name`
- `pub fn name`
- `async fn name`
- `struct name`
- `enum name`
- `trait name`
- `impl ...`
- `mod name`
- `macro_rules! name`

Expansion:

- Use brace matching for `{}` blocks.
- Include doc comments and attributes immediately above the definition.
- Cap with `max_block_lines`.

### Python

Detect:

- `def name(`
- `async def name(`
- `class name(` or `class name:`

Expansion:

- Use indentation block rules.
- Include decorators and comments immediately above.

### JavaScript/TypeScript

Detect:

- `function name(`
- `export function name(`
- `const name = (` / `const name = async (` / arrow functions
- `class name`
- methods inside classes/objects when feasible

Expansion:

- Prefer brace matching.
- Fall back to semicolon/blank-line bounded spans.

### Go

Detect:

- `func name(`
- `func (receiver) name(`
- `type name struct`
- `type name interface`

Expansion:

- Brace matching for functions/types.

### Java/C/C++/C#/Kotlin/Scala

Keep first pass conservative:

- Match class/interface/struct/enum/function-like declarations by symbol name.
- Expand with brace matching.
- Use weak confidence when declaration classification is ambiguous.

### Config and markdown

For TOML/YAML/JSON/Markdown:

- If `match_text` is provided, return a bounded context window.
- For Markdown heading matches, expand to the heading section.
- For TOML/YAML, expand to the nearest table/key block when feasible.
- For JSON, avoid attempting full object extraction unless simple brace matching is reliable.

## Integration with suggested fetches

When `CodeEvidence` includes match lines or context lines, `generate_suggested_fetches` should set:

- `line_start`
- `line_end`
- `context_before`
- `context_after`
- `expand_to_block = true` when the source role is implementation/test/example and language is supported.

When `CodeEvidence` includes `matched_symbol`, set `symbol` and `symbol_kind` where possible.

Do not require this phase to overhaul suggested-fetch ranking; that is Phase 3. This phase should simply make available structured fetch locators richer.

## Affected modules

Likely files:

- `src/core/repo_fetch.rs`
- `src/core/code_evidence.rs`
- `src/meta/suggested_fetches.rs`
- `src/fetch/*` or new `src/core/repo_span.rs`
- `src/mcp/tools.rs`
- `README.md`
- tests for repo fetch and span expansion

## Implementation steps

1. Add request/response fields with serde-compatible defaults.
2. Add language detection reuse from existing document/source rendering if available.
3. Implement span selection helpers:
   - explicit range selection;
   - match text selection;
   - symbol definition search;
   - block expansion.
4. Integrate span selection into `repo_fetch` after file text is retrieved and before output truncation.
5. Preserve existing line-range clamping semantics.
6. Add selected-span metadata.
7. Teach suggested fetch generation to populate symbol/range expansion fields when code evidence supports it.
8. Update README examples.

## Tests

Add unit tests for:

- Rust function block expansion, including attributes/doc comments.
- Rust impl method expansion.
- Python class/function indentation expansion.
- JS/TS function and arrow function expansion.
- Go function expansion.
- Markdown heading-section expansion.
- Match-text context fallback.
- `max_block_lines` truncation.
- Explicit range unchanged when `expand_to_block` is false.
- Explicit range expanded when `expand_to_block` is true.
- Missing symbol returns a useful warning or falls back deterministically.
- Existing `repo_fetch` tests still pass.

Add integration tests for MCP `repo_fetch` with symbol and line-range requests using mocked/raw test content.

## Acceptance criteria

- Existing `repo_fetch` calls remain backward-compatible.
- Callers can request a symbol or match text and receive a bounded line-numbered span.
- Supported languages expand to plausible enclosing blocks.
- Selected-span metadata explains what happened.
- Failure to find a symbol is non-catastrophic and clearly reported.
- Suggested fetch locators carry richer range/symbol context when code evidence is available.
- No unbounded file reads or multi-file fetch behavior is introduced.
- `cargo test` passes.

## Handoff notes

Do not chase parser perfection. The goal is deterministic, useful source spans for common coding-agent cases. Keep heuristics small, tested, and easy to refine. If a language is ambiguous, return weak confidence and a bounded context window rather than overclaiming exact block selection.
