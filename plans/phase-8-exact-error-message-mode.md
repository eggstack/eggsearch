# Phase 8: Exact Error Message Mode

## Purpose

Add a dedicated exact-error search mode optimized for coding agents diagnosing compiler errors, runtime exceptions, linker errors, dependency resolution failures, CI failures, and opaque toolchain messages.

Generic web search often mangles quoted errors, drops punctuation, or overweights blog spam. Coding agents need a retrieval mode that preserves the exact error string, searches likely authoritative contexts first, and returns source-quality signals so codegg can decide whether to fetch docs, issues, source, or release notes.

## Non-goals

Do not build a full log parser, run local commands, infer root cause from user code, or modify code. This phase is search and retrieval only. Do not add stack-trace uploading or telemetry collection.

## Tooling approach

Prefer extending `repo_search` rather than adding a new top-level tool unless the implementation becomes too large. Add:

```rust
pub enum RepoSearchMode {
    Normal,
    ExactError,
}
```

or add a request field:

```rust
pub exact_error: Option<ExactErrorRequest>,
```

Recommended MCP arguments:

```json
{
  "query": "error[E0277]: the trait bound ... is not satisfied",
  "profile": "coding",
  "mode": "exact_error",
  "language": "rust",
  "package": "tokio",
  "include_issues": true,
  "include_releases": true
}
```

If adding a separate tool is cleaner, call it `error_search`, but reuse repo-search grouping, provider dispatch, and source-card machinery.

## Error normalization

Create a deterministic error parser:

```rust
pub struct ErrorQueryParts {
    pub original: String,
    pub normalized: String,
    pub quoted_exact: String,
    pub error_codes: Vec<ErrorCode>,
    pub tool_names: Vec<String>,
    pub package_names: Vec<String>,
    pub language_hint: Option<String>,
    pub stack_frames: Vec<StackFrameHint>,
    pub path_fragments: Vec<String>,
}
```

Normalization rules:

- Preserve original text exactly.
- Trim leading/trailing whitespace.
- Collapse internal whitespace only for a normalized secondary query.
- Extract quoted lines and likely primary error lines.
- Extract known compiler/tool error codes, e.g. Rust `E0277`, TypeScript `TS2345`, Python exception names, npm `ERESOLVE`, cargo errors, linker symbols, HTTP status plus named error.
- Strip local absolute paths from search subqueries unless path fragments are useful and non-sensitive.
- Detect and optionally omit ephemeral values: temp paths, memory addresses, UUIDs, timestamps, local usernames, line/column numbers when they reduce recall.

Do not mutate the source query stored in telemetry; agents should see what was searched.

## Subquery generation

Generate a small, bounded set of subqueries:

1. Exact quoted error string.
2. Error code + tool/language.
3. Package/repo + exact error code.
4. Docs query for official documentation.
5. Issues query for maintainer/user reports.
6. Releases/changelog query for regressions if package/version hints exist.

Example for Rust:

```text
"error[E0277]: the trait bound" rust
E0277 trait bound not satisfied rust docs
repo:tokio-rs/tokio E0277 trait bound
```

Example for npm:

```text
"npm ERR! ERESOLVE unable to resolve dependency tree"
npm ERESOLVE dependency tree peer dependency
package-name ERESOLVE release notes
```

Bound subqueries by config, e.g. `exact_error_max_subqueries = 6`.

## Provider behavior

Exact-error mode should favor:

- Official docs for error codes.
- Maintainer issues/PRs for exact strings.
- Release notes/changelogs for regressions.
- Stack Overflow/forum results only after official/maintainer sources unless explicitly requested.
- Source search if the error includes function/type names likely to exist in code.

Provider-specific query formatting should preserve quotes where engines support exact phrases. Engines that do not support exact phrase search should receive normalized fallback queries.

## Ranking additions

Add rank reasons:

- `ExactErrorPhraseMatch`
- `ErrorCodeMatch`
- `ToolchainMatch`
- `OfficialErrorDocs`
- `MaintainerIssueMatch`
- `RegressionReleaseMatch`

Boost results with exact phrase in title/snippet/source text. Boost exact error-code docs. Penalize pages where all terms are present but the error phrase is not.

## Response metadata

Add an error context block:

```rust
pub struct ErrorSearchContext {
    pub original_error: String,
    pub normalized_error: String,
    pub error_codes: Vec<ErrorCode>,
    pub inferred_tools: Vec<String>,
    pub inferred_language: Option<String>,
    pub redactions_applied: Vec<String>,
    pub subqueries: Vec<RepoSearchSubqueryTelemetry>,
    pub warnings: Vec<String>,
}
```

Expose it in `repo_search` responses when exact-error mode is used.

Warnings:

- Query looked like a stack trace and was truncated to primary frames.
- Local paths/usernames were omitted from provider queries.
- No exact phrase matches found; results are fuzzy.
- Only community results found.

## Privacy and safety

Exact-error queries often contain local paths, usernames, tokens, endpoints, or private repo names. Add deterministic redaction before provider dispatch:

- Local absolute paths -> keep basename or relevant crate/module segment.
- Home directory usernames -> redact.
- Obvious API keys/tokens -> redact fully.
- URLs with query tokens -> strip query string unless needed.
- Long hashes/UUIDs -> omit unless they look like commit SHAs and user provided repo context.

Keep `original_error` only in local response if appropriate. Consider exposing `redacted_query` and `redactions_applied` separately.

## Configuration

Add config:

```toml
[search.exact_error]
enabled = true
max_subqueries = 6
max_error_chars = 8000
redact_sensitive_tokens = true
prefer_official_docs = true
```

Reject exact-error queries above cap with clear validation.

## Tests

Add tests for:

- Rust error code extraction.
- TypeScript error code extraction.
- Python exception extraction.
- npm/yarn/pnpm error extraction.
- Local path redaction.
- Token/API-key redaction.
- Exact phrase subquery generation preserves quotes.
- Subquery count cap.
- Exact phrase result is ranked above fuzzy result.
- Official docs result ranked above low-quality blog for same error code.
- Response includes `error_context` and telemetry.
- Invalid oversized error query is rejected.

Use mocked providers. No live search dependency.

## Documentation

Update README and AGENTS.md with examples:

- Rust compiler error.
- Python traceback.
- npm dependency resolution failure.
- CI log fragment.

Document that the tool redacts sensitive-looking local data before provider dispatch and that agents should pass the smallest relevant error block, not entire logs.

## Acceptance criteria

Phase 8 is complete when:

- `repo_search` or `error_search` supports exact-error mode.
- Error parsing extracts codes/tools/languages deterministically.
- Provider subqueries preserve exact phrases where possible.
- Sensitive local data is redacted before dispatch.
- Ranking favors exact phrase, official docs, and maintainer issue matches.
- Responses include structured error context and redaction warnings.
- Tests cover parsing, redaction, ranking, telemetry, and validation.
- `cargo fmt`, clippy, and tests pass.
