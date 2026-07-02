# Phase 6: Code-Aware Fetch and Repository Evidence Enrichment

## Objective

Improve `web_fetch`, `repo_fetch`, `repo_search`, `repo_map`, and suggested-fetch outputs so coding agents receive code-shaped evidence instead of generic text blobs. This phase should make source files, examples, tests, manifests, changelogs, migration notes, security policies, and local workspace files easier to identify, fetch, cite, and hand off.

This phase also includes the small docs polish left from the phase 1–5 corrective pass: fix stale comments/docs around warning fallback behavior and update the corrective checklist to reflect completed items. Treat that docs polish as a starter task, not as the main theme.

## Current context

Phases 1–5 established the reliability substrate:

- Provider status is more truthful.
- MCP docs and tool matrix exist.
- Structured warnings are available.
- Dispatch is bounded and priority-preserving.
- Stable identity spans source cards, fetches, suggested fetches, repo locators, documents, chunks, and evidence bundles.

The next problem is the evidence itself. Agents still need richer metadata for code-specific reasoning:

- Is this file implementation, test, example, benchmark, docs, config, manifest, lockfile, changelog, migration guide, or security policy?
- Which symbol or enclosing item does the span belong to?
- Which imports/use declarations matter for this span?
- Which tests/examples/manifests should be fetched next?
- Is this local content dirty, generated, vendored, or first-party source?
- Can the fetched lines be linked back to stable source/fetch IDs and code-host permalinks?

## Non-goals

- Do not add an unbounded local or remote code index.
- Do not add LSP as a hard dependency.
- Do not execute code.
- Do not run package manager commands.
- Do not parse every language with a full AST requirement in the first pass.
- Do not summarize code in the retrieval layer.

## Workstream 0: Carry forward docs polish from corrective pass

### Tasks

1. Fix stale comments in `src/core/warning.rs`, especially the `convert_fetch_warnings()` comment that still says unknown fetch warnings fall back to `ProviderFailed`. It should say unknown fetch warnings map to `FetchWarning`.
2. Update `plans/phase_01_05_corrective_closure.md` completion checklist to mark all implemented corrective items as complete, or add a short note that the plan has been implemented by the corrective closure commit.
3. Confirm `AGENTS.md`, `CHANGELOG.md`, `docs/agent-workflows.md`, and `docs/tool-matrix.md` use the same stable warning-code count and identity-hash wording.

### Acceptance criteria

- No docs or comments claim unknown fetch warnings map to provider failure.
- Corrective plan checklist does not misleadingly show implemented items as open.
- This work lands in the same phase 6 implementation series before functional code-aware fetch changes.

## Workstream 1: Define a code evidence span model

### Problem

The repo already has code evidence metadata and selected spans, but agents need a more explicit cross-tool span object that can represent a fetched region of code with stable identity, file role, language, line numbers, byte ranges, and derived context.

### Proposed model

Introduce or extend a `CodeSpanEvidence` model with fields similar to:

```rust
pub struct CodeSpanEvidence {
    pub stable_id: String,
    pub source_id: Option<String>,
    pub fetch_id: Option<String>,
    pub locator_id: Option<String>,
    pub host: Option<CodeHost>,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub ref_name: Option<String>,
    pub commit_sha: Option<String>,
    pub path: String,
    pub language: Option<String>,
    pub source_role: SourceRole,
    pub line_start: Option<u32>,
    pub line_end: Option<u32>,
    pub byte_start: Option<u64>,
    pub byte_end: Option<u64>,
    pub symbol: Option<String>,
    pub symbol_kind: Option<String>,
    pub enclosing_symbol: Option<String>,
    pub imports: Vec<String>,
    pub trust: FetchTrust,
    pub permalink_url: Option<String>,
    pub raw_permalink_url: Option<String>,
}
```

The exact shape can differ, but the response should clearly identify the fetched span as code evidence.

### Implementation guidance

- Reuse existing `CodeEvidence`, `SourceRole`, `RepoLocator`, `SelectedSpan`, and identity helpers where possible.
- Do not duplicate large content; the span object should describe content already present in the response.
- Use `stable_id` / `source_id` / `fetch_id` links from phase 5.
- Preserve backward compatibility by adding optional fields rather than renaming existing response fields.

### Tests

- `repo_fetch` of a Rust file with explicit line range returns code span metadata.
- `repo_fetch` with `symbol` and `expand_to_block` returns symbol/enclosing metadata if available.
- `web_fetch` of a code-host raw/blob URL returns code evidence metadata after URL transform.
- Non-code documents do not incorrectly claim code span metadata.

## Workstream 2: Source role classification hardening

### Problem

Agents need reliable source roles to choose next fetches. A source file and a test file should not be treated the same. A changelog or migration doc should be obvious. Generated/vendor files should be lower confidence.

### Required source roles

At minimum, classify:

- implementation
- test
- example
- benchmark
- configuration
- build
- manifest
- lockfile
- documentation
- readme
- changelog
- migration
- security_policy
- ci
- generated
- vendor
- unknown

### Heuristics

Use path/name/extension heuristics first:

- `tests/`, `test/`, `_test.*`, `.test.*`, `.spec.*` -> test
- `examples/`, `example/` -> example
- `benches/`, `benchmarks/`, `*_bench.*` -> benchmark
- `Cargo.toml`, `package.json`, `pyproject.toml`, `go.mod`, `pom.xml`, etc. -> manifest
- `Cargo.lock`, `package-lock.json`, `poetry.lock`, `go.sum`, etc. -> lockfile
- `README*` -> readme
- `CHANGELOG*`, `RELEASES*`, `NEWS*` -> changelog
- `MIGRATION*`, `UPGRADE*` -> migration
- `SECURITY.md`, `.github/SECURITY.md` -> security_policy
- `.github/workflows/`, `.gitlab-ci.yml`, `azure-pipelines.yml` -> ci
- `target/`, `dist/`, `build/`, `node_modules/`, `vendor/`, generated markers -> generated/vendor

### Tests

- Table-driven tests for each role.
- Case-insensitive README/CHANGELOG/SECURITY matching.
- Generated/vendor classification has lower confidence than first-party source.
- Existing role classification behavior is not regressed for source files.

## Workstream 3: Lightweight language and symbol context extraction

### Problem

Agents often need imports and enclosing item context. Full AST parsing can wait, but lightweight extraction dramatically improves usefulness.

### First-pass language support

Implement lightweight line-oriented extraction for:

- Rust
- Python
- TypeScript/JavaScript
- Go
- TOML/YAML/JSON/Markdown as structured-document roles

### Required extraction

For code files:

- language from extension and/or code-host metadata.
- top-of-file imports/use statements within a bounded prefix.
- enclosing item around fetched line range using simple regex/line heuristics:
  - Rust: `fn`, `impl`, `struct`, `enum`, `trait`, `mod`
  - Python: `def`, `async def`, `class`
  - TS/JS: `function`, `class`, `export function`, `const name =`, `interface`, `type`
  - Go: `func`, `type`, `var`, `const`

### Constraints

- Keep extraction bounded by max lines and max chars.
- Extraction failure should not fail the fetch.
- Emit structured warnings only if extraction failure is surprising; otherwise silently omit fields.

### Tests

- Rust impl method span identifies enclosing `impl` and method.
- Python async function span identifies function and class where applicable.
- TypeScript exported function identifies symbol.
- Go method identifies receiver function.
- Large file extraction stays bounded.

## Workstream 4: Suggested fetch enrichment

### Problem

`repo_search` and `research_search` already produce suggested fetches, but agents need more intentional code-task suggestions: implementation plus tests, examples, manifest, changelog, migration guide, security policy, and docs.

### Required behavior

When a source card or repo search result points to code:

- Suggest the exact source file/span.
- Suggest nearby test files if path heuristics can infer them.
- Suggest examples for public APIs.
- Suggest manifest/lockfile when package/version or dependency context is involved.
- Suggest changelog/migration docs for version/upgrade queries.
- Suggest security policy/advisories for security profile queries.

Each suggested fetch should carry:

- stable ID
- source ID
- reason code
- source role
- priority
- expected information gain
- explicit fetch locator or URL

### Reason codes

Suggested reason codes should include:

- `exact_source_match`
- `nearby_test_candidate`
- `example_candidate`
- `manifest_context`
- `lockfile_context`
- `changelog_context`
- `migration_context`
- `security_policy_context`
- `official_docs_context`
- `repo_root_context`

### Tests

- Source file result suggests itself plus plausible tests.
- Package query suggests manifest/lockfile context.
- Migration query suggests changelog/migration docs.
- Security profile query suggests security policy/advisory context.
- Suggested fetch IDs remain stable.

## Workstream 5: Local workspace code evidence metadata

### Problem

Local results are more actionable for coding agents, but also need trust and state markers. Agents should know if local code is dirty, untracked, generated, vendored, first-party, or mismatched from the remote repo.

### Required behavior

For local workspace results and fetches, include:

- workspace root identity
- git remote match/mismatch
- branch/ref
- commit SHA where available
- dirty/untracked state
- generated/vendor classification
- source role
- line span
- local trust marker

### Tests

- Clean git checkout result reports clean state.
- Dirty checkout result reports dirty state.
- Untracked file result reports untracked state if supported.
- Vendor/generated paths are classified correctly.
- Local result matching remote repo carries remote identity metadata.

## Workstream 6: Evidence bundle compatibility

### Problem

Evidence bundles should preserve the richer code evidence model without bloating the bundle or breaking existing consumers.

### Required behavior

- Evidence bundle source/fetched items retain code evidence span metadata if provided.
- Bundle gap analysis can identify missing tests/examples/manifests when suggested fetches were not fetched.
- Bundle item IDs remain stable and linked to source/fetch IDs.

### Tests

- Bundle built from repo search + repo fetch preserves code span metadata.
- Bundle gap analysis reports missing nearby tests when only implementation was fetched.
- Bundle remains backward-compatible when code evidence metadata is absent.

## Acceptance criteria

- Code fetch responses expose optional code evidence metadata with stable identity links.
- Source roles are consistently classified and tested.
- Lightweight symbol/import extraction works for Rust, Python, TS/JS, and Go without requiring full AST parsers.
- Suggested fetches become more useful for implementation/test/example/manifest/changelog/security workflows.
- Local workspace metadata is explicit enough for codegg agents to prefer local evidence safely.
- Evidence bundles preserve code evidence metadata and gap analysis.
- Docs polish from corrective pass is completed.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` pass.
