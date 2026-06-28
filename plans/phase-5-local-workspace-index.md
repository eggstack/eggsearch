# Phase 5 Plan: Lightweight Local Workspace Index and Symbol Enrichment

## Objective

Add an optional local workspace search backend so codegg can retrieve local repository evidence through eggsearch with the same structured result model used for remote repo search. Start with deterministic file/path/text search and local trust/provenance metadata. Then add lightweight symbol enrichment for supported languages if dependency cost and build impact remain acceptable.

The goal is not to replace codegg's direct file tools. The goal is to let codegg ask one retrieval layer for a coding-oriented evidence set that can include local workspace files, remote docs, upstream source, issues, releases, and advisories with clear trust boundaries.

## Current baseline

The trust model has a `LocalTrusted` variant reserved for future local-index results, but no current path produces it. Repo/code metadata and source-card grouping already provide a shape that local results can reuse. `web_fetch` and the planned `repo_fetch` handle explicit retrieval; this phase adds local discovery and optional local exact fetch integration.

## Non-goals

Do not add a heavyweight persistent index database in the first pass. Do not watch files continuously. Do not parse every language. Do not run build tools or LSP servers. Do not treat local file contents as trusted instructions; local source is more provenance-trusted than web content, but comments and docs can still contain adversarial text. Do not expose arbitrary filesystem reads outside configured roots.

## Configuration

Add optional local workspace configuration:

```toml
[local]
enabled = false
roots = ["/path/to/workspace"]
max_file_bytes = 1048576
max_indexed_files = 50000
include_hidden = false
respect_gitignore = true
follow_symlinks = false
```

If the project prefers keeping config under `[search]`, use `[search.local]`. Defaults must keep local search disabled.

All roots should be canonicalized at startup. Reject roots that do not exist or are not directories unless the repo's config style prefers warning and skipping. Never allow local search outside configured roots.

## Tool and response integration

Do not add a separate `local_search` tool initially unless necessary. Prefer integrating local backend into `repo_search` through profile/config:

- `profile = "coding"` can include local backend when enabled.
- Add `include_local: Option<bool>` to `RepoSearchRequest` if explicit control is needed.
- Add a provider id such as `local_workspace` so telemetry and provider status can show local participation.

Local results should serialize as normal `SourceCard` values with:

- `trust = local_trusted` or a new clearer trust/provenance field if the current trust vocabulary is insufficient.
- `metadata.source_kind = source_file` or appropriate source kind.
- `metadata.code` and `metadata.code_evidence` populated with path, language, source role, line ranges when available.
- URL field using a safe pseudo-URL or file URI policy. Prefer a pseudo-URL like `workspace://root-id/path/to/file.rs` rather than raw `file://` if the rest of the system is not designed for arbitrary file URLs.

If pseudo-URLs are introduced, document that only eggsearch/codegg should dereference them through local-aware tools. Generic `web_fetch` must not fetch `workspace://` unless explicitly designed to do so.

## Local backend architecture

Add a local backend abstraction separate from web providers. Do not force it into the same HTTP search-engine trait if that creates awkward semantics. A clean boundary is:

```rust
pub trait LocalSearchBackend {
    async fn search(&self, req: &LocalSearchRequest) -> LocalSearchResult;
}
```

Initial implementation can be synchronous internally and wrapped safely, but avoid blocking the async runtime on large scans. Use `spawn_blocking` for filesystem traversal and text scanning if needed.

Initial search strategy:

- Walk configured roots within limits.
- Apply ignore rules and extension filters.
- Score path matches, filename matches, language matches, and text matches.
- Return bounded matches with line ranges and snippets.
- Cache nothing or only cache a short-lived file list in memory if simple and safe.

A later pass can add persistent indexing. The first pass should be robust and bounded.

## Ignore and safety rules

Respect `.gitignore` by default if adding the `ignore` crate is acceptable. If avoiding the dependency, implement conservative skips and leave full gitignore support for a follow-up. Skip common heavy/generated directories:

- `.git`, `target`, `node_modules`, `.venv`, `venv`, `dist`, `build`, `.mypy_cache`, `.pytest_cache`, `__pycache__`, `.next`, `.turbo`, `coverage`.

Skip binary files using extension and content sniffing. Enforce `max_file_bytes`. Enforce `max_indexed_files`. Enforce a per-search timeout or scan budget so local search cannot hang codegg.

Do not follow symlinks by default. If following symlinks is enabled later, ensure canonicalized target paths remain within configured roots unless explicitly allowed.

## Local result scoring

Keep scoring deterministic and simple:

- Exact filename match.
- Path segment match.
- Language match.
- Exact symbol/text match.
- Query token text match.
- Source role boost for implementation/test/example depending on requested groups.
- Penalty for generated/lock/minified files unless explicitly requested.

Attach `CodeEvidenceReason` values from Phase 1 where possible: `provider_path_match` can be generalized or add local-specific reasons such as `local_path_match`, `local_text_match`, `local_symbol_match` if the enum is expanded.

## Symbol enrichment

Add symbol enrichment after basic local search lands. Evaluate dependency footprint before choosing tree-sitter crates. If acceptable, start with Rust and Python.

Symbol extraction should be optional and bounded:

- Parse only files under size cap.
- Extract definitions and rough enclosing ranges.
- Store ephemeral symbol info during a search or in a short-lived in-memory cache.
- Populate `matched_symbol`, `symbol_kind`, `enclosing_symbol`, and match/context line ranges when a symbol query matches.

If tree-sitter is deferred, implement a conservative regex fallback for Rust/Python definitions with clear `weak` confidence. Do not pretend regex symbol matching is precise.

## Local fetch integration

If Phase 2 `repo_fetch` supports local workspace locators, integrate local results with it. Otherwise add a narrow `workspace_fetch` path or extend `repo_fetch` later.

The local fetch request should only accept workspace result locators emitted by local search or validated root-relative paths under configured roots. It should return line-numbered content using the same response model as `repo_fetch` where possible.

## Provider status

Expose local backend status in `provider_status` or a tool-capabilities response:

- `id = local_workspace`.
- `kind = local` or an added provider kind.
- `enabled` and `configured` flags.
- Capabilities: code search, path filter, language filter, symbol hint if enabled, result timestamps if file modified timestamps are exposed.

Do not expose absolute root paths by default unless the operator wants that. For codegg UI, a root alias or count may be enough.

## Tests

Add unit tests for config validation:

- Local disabled by default.
- Canonical root accepted.
- Missing root skipped or rejected according to chosen config policy.
- Symlink behavior follows config.
- Path traversal rejected.

Add local search tests using `tempfile`:

- Filename search returns matching file.
- Path hint narrows results.
- Language hint narrows results.
- Text query returns matching line range.
- Hidden/generated directories are skipped by default.
- Binary files are skipped.
- File size cap is enforced.
- Results are bounded by `max_results` and timeout/scan limits.

Add trust/provenance tests:

- Local results use local trust/provenance metadata.
- Local results still carry trust markers if source text contains injection-like markers.
- Workspace pseudo-URLs cannot escape configured roots.

Add symbol tests if symbol enrichment is implemented in this phase:

- Rust function/struct/trait detection.
- Python function/class detection.
- Symbol query returns enclosing range and symbol kind.
- Large files are skipped or downgraded gracefully.

## Documentation

Update README and config docs with local workspace search:

- Disabled by default.
- Requires configured roots.
- Respects bounds and ignores generated directories.
- Local trust means operator-configured provenance, not instruction trust.
- Explain how codegg should combine local and remote evidence.

Add an example `repo_search` call with `profile = "coding"` and local enabled, showing a local source-file result next to remote docs/issues.

## Acceptance criteria

- Local workspace search is disabled by default and safe when enabled.
- Configured local roots are canonicalized and enforced.
- `repo_search` can include local source results through a clear provider/provenance id.
- Local results carry structured code metadata and line evidence.
- Local search is bounded by file count, file size, result count, and timeout/scan budget.
- Trust/provenance semantics are documented and tested.
- Optional symbol enrichment either lands with tests or is explicitly deferred behind a feature flag/follow-up plan.

## Suggested implementation order

1. Add local config model and validation.
2. Add local provider/status descriptor.
3. Implement bounded local file walking and filtering.
4. Implement deterministic text/path/language scoring.
5. Convert local matches into `SourceCard` + `CodeEvidence`.
6. Wire local backend into `repo_search` behind config/profile controls.
7. Add local fetch integration if Phase 2 abstractions make it straightforward.
8. Add optional symbol enrichment or document deferral.
9. Update README and tests.
