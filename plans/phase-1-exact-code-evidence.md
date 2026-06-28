# Phase 1 Plan: Exact Code Evidence Metadata

## Objective

Upgrade eggsearch's repository and code-search result model so a coding agent receives exact, typed code evidence rather than only a possibly relevant source-file URL. This phase should be metadata-first and backward-compatible. It should not add a new fetch tool yet; that belongs to Phase 2.

The practical target is that `repo_search` and native code-provider results can represent evidence like: repository, ref, path, canonical browser URL, raw URL, permalink URL, matched line range, suggested context line range, matched symbol, enclosing symbol, and confidence/provenance about how those fields were derived.

## Current baseline

The repo already has useful code metadata. `CodeMetadata` captures host, owner, repo, path, ref, inferred language, symbol hint, and line anchors parsed from code-host URLs. `SourceCard` already has deterministic `SourceMetadata`, `SourceKind`, `RankReason`, and optional code/issue/release/vulnerability metadata. `repo_search` groups source cards and generates suggested fetches.

The gap is that a coding agent still mostly receives file-level evidence. It cannot reliably distinguish these cases:

- A file result where the query matched the target symbol directly.
- A file result where only the filename matched.
- A file result that has a browser URL but no raw fetch URL.
- A file result that points at a branch ref versus a stable commit permalink.
- A source result with a known line span versus one that requires a full-file fetch.
- A source file that is likely a test/example/config/build file versus core implementation.

## Non-goals

Do not introduce tree-sitter parsing in this phase. Do not add a local workspace index in this phase. Do not add `repo_fetch` in this phase. Do not require GitHub API access for generic providers to continue working. Do not break existing serialized `SourceCard` consumers.

## Proposed data model additions

Add a new module, likely `src/core/code_evidence.rs`, and re-export it from `src/core/mod.rs` if appropriate.

Define a `CodeEvidence` struct with optional fields so it can be attached opportunistically:

```rust
pub struct CodeEvidence {
    pub host: Option<CodeHost>,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub ref_name: Option<String>,
    pub commit_sha: Option<String>,
    pub path: Option<String>,
    pub language: Option<String>,
    pub source_role: Option<SourceRole>,
    pub browser_url: Option<String>,
    pub raw_url: Option<String>,
    pub permalink_url: Option<String>,
    pub match_line_start: Option<u32>,
    pub match_line_end: Option<u32>,
    pub context_line_start: Option<u32>,
    pub context_line_end: Option<u32>,
    pub matched_symbol: Option<String>,
    pub symbol_kind: Option<SymbolKind>,
    pub enclosing_symbol: Option<String>,
    pub evidence_confidence: Option<EvidenceConfidence>,
    pub evidence_reasons: Vec<CodeEvidenceReason>,
}
```

Keep all fields serde-defaulted and skipped when empty. The exact naming can be adjusted to match repo style, but keep the schema explicit and stable.

Add compact enums:

- `SourceRole`: `implementation`, `test`, `example`, `benchmark`, `configuration`, `build`, `documentation`, `readme`, `changelog`, `migration`, `unknown`.
- `SymbolKind`: `function`, `method`, `struct`, `enum`, `trait`, `class`, `interface`, `module`, `constant`, `type_alias`, `macro`, `unknown`.
- `EvidenceConfidence`: `exact`, `strong`, `weak`, `unknown`.
- `CodeEvidenceReason`: `url_line_anchor`, `provider_text_match`, `provider_path_match`, `provider_symbol_match`, `language_match`, `repo_match`, `path_hint_match`, `file_hint_match`, `raw_url_derived`, `permalink_derived`, `source_role_inferred`.

Add this as a sibling to the existing `metadata.code` rather than replacing `CodeMetadata` immediately. A compatible shape is:

```rust
pub struct SourceMetadata {
    ...
    pub code: Option<CodeMetadata>,
    pub code_evidence: Option<CodeEvidence>,
    ...
}
```

This avoids changing the meaning of existing `metadata.code` while giving newer agents richer fields.

## URL and source-role helpers

Add pure helper functions that can be heavily tested without network:

- Derive GitHub raw URLs from GitHub blob URLs when owner/repo/ref/path are known.
- Derive GitLab raw URLs from GitLab blob URLs when possible.
- Derive stable browser URLs from raw URLs when possible.
- Preserve line fragments and convert `#L10-L20` into match/context fields.
- Infer `SourceRole` from path patterns:
  - README files -> `readme`.
  - `examples/`, `example/`, demo-like paths -> `example`.
  - `tests/`, `test/`, `*_test.*`, `*.test.*`, `spec/` -> `test`.
  - `benches/`, `benchmarks/` -> `benchmark`.
  - `.github/workflows`, `Cargo.toml`, `pyproject.toml`, `package.json`, `Dockerfile`, CI/config files -> `configuration` or `build` depending on file.
  - `CHANGELOG`, `RELEASES`, `UPGRADE`, `MIGRATION` -> changelog/migration.

Do not overfit the taxonomy. Prefer conservative inference and `unknown` over false precision.

## Integration points

Update `convert_aggregated` or the nearest metadata-enrichment boundary so that any code-host URL with parsed `CodeMetadata` also receives `CodeEvidence` when enough information is available. Keep this deterministic and local; do not fetch content.

Update repo grouping and suggested fetch logic to prefer `code_evidence.raw_url` or `code_evidence.permalink_url` when available, but keep current URL behavior as fallback.

Update rank reasons or add evidence reasons so path/file/language/symbol hints are visible per result. If the existing `RankReason::HintMatch` is too coarse, leave it intact and put finer detail in `CodeEvidenceReason` rather than expanding rank reasons too aggressively.

## Provider-specific enrichment

For the GitHub Code Search provider, inspect the provider result model. If it has text matches, path, repository, URL, or fragment data, map them into `CodeEvidence`. If the provider does not return line numbers through the current implementation, still add path/repo/raw URL/source role evidence and mark confidence as `strong` or `weak` depending on available match detail.

For generic HTML providers, only derive evidence from URL shape, title/snippet hints, and existing parsed code metadata. Confidence should usually be `weak` unless line anchors or exact repo/path hints are present.

## Tests

Add unit tests for the new data model serialization. Tests should prove omitted optional fields do not break old response shapes and populated fields serialize using snake_case enum variants.

Add URL helper tests for:

- GitHub blob URL -> raw URL.
- GitHub blob URL with `#L10-L25` -> line range evidence.
- GitLab blob URL -> raw URL if existing transform supports it.
- Repository root and tree URLs do not claim file-level raw URLs.
- Unknown hosts produce no code evidence or weak evidence only.

Add source-role inference tests for Rust and Python common paths:

- `src/lib.rs` -> implementation.
- `tests/integration.rs` -> test.
- `examples/server.rs` -> example.
- `Cargo.toml` -> configuration/build as chosen.
- `.github/workflows/ci.yml` -> build/configuration.
- `README.md` -> readme.
- `CHANGELOG.md` -> changelog.

Add mocked `repo_search` tests verifying that grouped source-file results include `metadata.code_evidence` when a code-host source-file URL is returned.

## Documentation

Update README code metadata docs with a short section explaining the optional `metadata.code_evidence` object. Clarify that evidence fields are deterministic metadata, not fetched content, and that exact line/symbol data is only as strong as the provider/URL allows.

Update any docs or examples that show `SourceCard` metadata to include a compact `code_evidence` example.

## Acceptance criteria

- Existing tests pass without modifying old minimal `SourceCard` consumers.
- `repo_search` source-file results can carry `metadata.code_evidence` with raw URL, source role, and line anchors when derivable.
- GitHub and GitLab URL transformations are tested without network.
- Generic provider results do not overclaim exact line or symbol matches.
- Warnings and trust labels remain unchanged: external source metadata is still untrusted evidence.

## Suggested implementation order

1. Add enums and `CodeEvidence` model.
2. Add pure URL/source-role helper functions and tests.
3. Attach `code_evidence` during source-card metadata enrichment.
4. Update suggested fetch generation to prefer richer evidence URLs.
5. Add mocked repo-search integration tests.
6. Update README and examples.
