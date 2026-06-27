# Phase 2: Repo/API/Project Retrieval Bundles

## Objective

Add a repo-oriented retrieval layer optimized for codegg's most common coding-agent workflow: understanding an API, library, project, repository, source file, symbol, release, issue, or migration path. This phase should preserve generic `web_search` as a simple flat source-card tool while adding an explicit structured path for repository context.

The preferred implementation is a new MCP tool named `repo_search`. It should return grouped evidence bundles rather than a flat ranked list. The tool should use existing primitives where possible: `RepoQueryHints`, code-host URL metadata, source-kind classification, GitHub code/issues/releases providers, RRF aggregation, provider warnings, and `web_fetch` for later explicit fetches.

## Non-goals

Do not remove or degrade `web_search intent=code`. It remains the generic fallback.

Do not clone repositories locally.

Do not create a persistent local code index.

Do not crawl repository links recursively.

Do not summarize source code or documentation.

Do not require GitHub API credentials for basic fallback behavior. Native API providers should improve results when configured, but generic providers should still produce useful bundles where possible.

Do not attempt full semantic code intelligence. That remains codegg/LSP/semantic-index territory. Eggsearch should find and classify external evidence.

## User-facing behavior

A codegg agent should be able to ask eggsearch for a structured repository context bundle with a request shaped roughly like this:

```json
{
  "query": "middleware Layer extract request extensions",
  "host": "github",
  "owner": "tokio-rs",
  "repo": "axum",
  "language": "rust",
  "symbol": "Layer",
  "include_docs": true,
  "include_registry": true,
  "include_issues": true,
  "include_releases": true,
  "include_examples": true,
  "max_results": 24,
  "max_per_group": 5,
  "freshness": "year"
}
```

The response should group source cards into sections such as official docs, package registry, repository, README, examples, source files, issues, pull requests, releases, migration notes, community discussion, and other. Each card should preserve normal `SourceCard` metadata and add repo-bundle-specific group/ranking context where needed.

## Proposed public types

Add new core request/response types, likely in new files:

- `src/core/repo_search.rs`
- or `src/core/repo_bundle.rs`

Export them through `src/core/mod.rs`.

### `RepoSearchRequest`

Recommended fields:

```rust
pub struct RepoSearchRequest {
    pub query: String,
    pub host: Option<CodeHost>,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub org: Option<String>,
    pub path: Option<String>,
    pub file: Option<String>,
    pub language: Option<String>,
    pub symbol: Option<String>,
    pub include_docs: Option<bool>,
    pub include_registry: Option<bool>,
    pub include_issues: Option<bool>,
    pub include_releases: Option<bool>,
    pub include_examples: Option<bool>,
    pub include_pull_requests: Option<bool>,
    pub max_results: Option<usize>,
    pub max_per_group: Option<usize>,
    pub freshness: Freshness,
    pub timeout_ms: Option<u64>,
    pub providers: Vec<String>
}
```

Notes:

- `query` remains required and must validate like `WebSearchRequest.query`.
- Explicit fields should override or supplement hint tokens parsed from `query`.
- If `owner` and `repo` are absent but `query` contains `repo:owner/name` or bare `owner/repo`, reuse `RepoQueryHints::parse`.
- Keep defaults conservative: include docs, registry, repository, source files, issues, releases, and examples unless explicitly disabled.
- `providers` should optionally constrain provider selection just like `WebSearchRequest.providers`.

### `RepoResultGroup`

Recommended enum:

```rust
pub enum RepoResultGroupKind {
    OfficialDocs,
    PackageRegistry,
    Repository,
    Readme,
    Examples,
    SourceFiles,
    Tests,
    Issues,
    PullRequests,
    Releases,
    MigrationNotes,
    Changelog,
    CommunityDiscussion,
    Other,
}
```

Recommended struct:

```rust
pub struct RepoResultGroup {
    pub kind: RepoResultGroupKind,
    pub label: String,
    pub results: Vec<SourceCard>,
    pub truncated: bool,
}
```

### `RepoSearchResponse`

Recommended fields:

```rust
pub struct RepoSearchResponse {
    pub query: String,
    pub mode: String,
    pub resolved: RepoQueryHints,
    pub groups: Vec<RepoResultGroup>,
    pub suggested_fetches: Vec<RepoSuggestedFetch>,
    pub providers_queried: Vec<String>,
    pub providers_failed: Vec<ProviderFailure>,
    pub warnings: Vec<SearchWarning>,
    pub trust_markers: TrustMarkers,
}
```

### `RepoSuggestedFetch`

This should help codegg decide what to fetch next without eggsearch fetching automatically.

Recommended fields:

```rust
pub struct RepoSuggestedFetch {
    pub url: String,
    pub reason: String,
    pub group: RepoResultGroupKind,
    pub expected_kind: SourceKind,
    pub recommended_extract_mode: Option<ExtractMode>,
    pub priority: u8,
}
```

Keep `reason` deterministic enum-like text, not generated prose. Example reasons: `official_docs`, `readme`, `source_file_symbol_match`, `example_file`, `recent_release`, `issue_thread`, `migration_note`.

## MCP tool surface

Add a new `repo_search` MCP tool in `src/mcp/*` using the same style as existing `web_search`.

Tool contract:

- Validate query and limits.
- Resolve hints from explicit fields and query tokens.
- Select providers using the adapter provider-selection path.
- Return structured groups and warnings.
- Do not fetch page bodies.
- Do not call `web_fetch` internally.

The tool description should make clear that `repo_search` is for structured repository evidence discovery, while `web_search` remains the generic search tool.

## Planner design

Add a repo bundle planner, likely:

- `src/meta/repo_planner.rs`
- or `src/meta/repo_bundle.rs`

The planner should generate a bounded set of subqueries from the resolved repo hints. Keep the set small and transparent.

Recommended subqueries:

1. Official docs query:
   - residual query + package/project terms + `docs documentation api reference`.
   - Prefer known docs domains when possible, but do not require them.

2. Package registry query:
   - package/repo name + ecosystem/language + registry terms.
   - For Rust, include `docs.rs` and `crates.io` terms.
   - For Python, include `PyPI` and documentation terms.
   - For JS/TS, include `npm` and documentation terms.

3. Repository/source query:
   - use provider-specific query if `github_code` is available.
   - otherwise generic query with owner/repo, path/file/language/symbol hints, and code-host terms.

4. README/examples query:
   - repo scope + `README examples sample usage`.
   - If path/file hints exist, include them.

5. Issues/PR query:
   - use `github_issues` if available.
   - otherwise generic query with issue/discussion/pull request terms.

6. Releases/changelog/migration query:
   - use `github_releases` if available.
   - otherwise generic query with release/changelog/migration terms.

The planner should return the subqueries used in debug logs and optionally in response metadata if the response type includes this field later.

## Provider execution strategy

Avoid uncontrolled fan-out. Recommended bounds:

- Maximum subqueries: 6 by default.
- Maximum providers per subquery: selected providers or all enabled providers, but keep per-provider result count small.
- Candidate count per subquery: derive from `max_results`, `max_per_group`, and configured cap.
- Apply the existing per-request timeout across the full operation, not independently per subquery in a way that can multiply latency unexpectedly.

Implementation options:

1. Reuse `MetadataSearchAdapter::web_search` by constructing internal `WebSearchRequest`s for each subquery, then merging/grouping results.
2. Add a lower-level adapter method that accepts a `SearchPlan` or explicit provider query set and returns raw `SourceCard`s plus failures.

Prefer option 1 initially if it avoids invasive refactoring. If repeated `web_search` calls duplicate too much work or warning behavior, refactor later.

## Grouping and classification

Implement deterministic grouping based on `SourceKind`, domain, URL path, code metadata, issue metadata, release metadata, title, and provider ID.

Suggested rules:

- `OfficialDocs`: `SourceKind::OfficialDocs` or known docs domains such as docs.rs, developer.mozilla.org, official project docs if recognized.
- `PackageRegistry`: `SourceKind::PackageRegistry` or known registry domains.
- `Repository`: repository root/source repository pages.
- `Readme`: code-host source files with path basename matching README variants.
- `Examples`: path contains `example`, `examples`, `sample`, `samples`, `demo`, or `demos`.
- `Tests`: path contains `test`, `tests`, or common test file naming patterns.
- `SourceFiles`: `SourceKind::SourceFile` not otherwise classified.
- `Issues`: `SourceKind::IssueThread` and not pull request.
- `PullRequests`: `SourceKind::PullRequest` or issue metadata indicates PR.
- `Releases`: `SourceKind::ReleaseNotes` or release metadata present.
- `MigrationNotes`: title/path/snippet includes migration, upgrade, breaking change, changelog, release note, deprecation.
- `Changelog`: path/title contains CHANGELOG variants or release/changelog source kind.
- `CommunityDiscussion`: forums/discussions/Q&A domains and issue discussion fallback where not authoritative.
- `Other`: everything else.

A card may qualify for several groups. Choose a primary group deterministically. Do not duplicate the same card in multiple groups unless the response explicitly supports cross-links. Simpler initial behavior: one primary group only.

## Ranking within groups

Within each group, order by a combined deterministic score:

- Existing card score.
- Source-kind match to group.
- Exact owner/repo match.
- Path/file/language/symbol hint match.
- Native provider evidence, e.g. `github_code`, `github_issues`, `github_releases`.
- Official/registry/docs domain prior.
- Freshness only when timestamp evidence exists.

Do not let heuristic boosts overwhelm multi-provider RRF evidence. Follow the existing bounded boost style from `apply_intent_reranking`.

## Suggested fetch generation

Generate a small `suggested_fetches` list. Suggested default: 5 to 8 entries.

Prioritize:

1. Best official docs or API reference page.
2. README if present.
3. Most relevant source file for path/file/symbol/language query.
4. Best examples file/page.
5. Most relevant release/migration note.
6. Most relevant open or recent issue if the query looks bug/regression-related.

Each suggestion must correspond to a result card in the grouped response.

Recommended extract modes:

- Docs/README/release/advisory pages: `markdown` when available.
- Source files, diffs, patches, JSON/TOML/YAML: default/text/code-specific rendering through `web_fetch` detection.
- Metadata-only should not be suggested unless the user explicitly requested metadata.

## Warning behavior

Add repo-search-specific warnings for approximate behavior:

- No native code provider configured; falling back to generic web providers.
- Repo hints were parsed but no selected provider supports repo filtering.
- Symbol hint was parsed but no selected provider supports symbol-aware search; using text query fallback.
- Issues requested but no native issue provider configured.
- Releases requested but no native release provider configured.
- Group is empty because provider support is missing or no results were found.

Warnings should be concise and stable enough for codegg TUI display.

## Tests

Add deterministic tests using mock engines and/or lower-level grouping functions.

Core tests:

- `RepoSearchRequest` validation rejects empty query and zero limits.
- Explicit fields override parsed query hints where intended.
- Bare `owner/repo` and `repo:owner/repo` are resolved consistently.
- Include flags suppress corresponding subqueries/groups.
- `max_results` and `max_per_group` are enforced.

Planner tests:

- Docs, registry, source, examples, issues, and releases subqueries are generated for a full request.
- Provider-specific GitHub code/issues/releases queries are used when providers are selected/configured.
- Generic fallback subqueries are generated when native providers are absent.
- Query expansion remains bounded.

Grouping tests:

- GitHub blob README goes to `Readme`.
- `examples/` source file goes to `Examples`.
- `tests/` source file goes to `Tests`.
- Docs.rs result goes to `OfficialDocs` or `PackageRegistry` depending existing source-kind classification.
- Crates.io/PyPI/npm result goes to `PackageRegistry`.
- GitHub issue result goes to `Issues`.
- GitHub PR result goes to `PullRequests`.
- GitHub release result goes to `Releases`.
- Changelog/migration result goes to `MigrationNotes` or `Changelog`.

MCP tests:

- `repo_search` returns grouped response shape.
- Provider failures are preserved.
- Trust markers aggregate across returned cards.
- No fetch is performed by `repo_search`.

Integration-style mocked workflow tests:

- `repo:tokio-rs/axum middleware symbol:Layer language:rust` returns source/docs/issues/release groups with suggested fetches.
- `rust crate axum migration 0.7` returns docs/package/release/migration groups.

## Documentation

Update README with a new `repo_search` section.

Include:

- Purpose and non-goals.
- Minimal request.
- Full request with repo hints.
- Example grouped response excerpt.
- Fallback guidance: if `repo_search` unavailable, use `web_search intent=code`.
- Provider configuration notes for GitHub API-backed code/issues/releases.

Update CHANGELOG under Unreleased.

## Implementation order

1. Add core types and exports.
2. Add planner and grouping functions with unit tests.
3. Add adapter method or orchestration function for repo bundle search.
4. Add MCP tool wrapper.
5. Add warning handling.
6. Add suggested fetch generation.
7. Add docs and changelog.
8. Run full tests and fix clippy/doc warnings.

## Validation commands

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test
```

Live-provider tests, if added, must remain opt-in and must not be required for CI.

## Acceptance criteria

This phase is complete when:

- `repo_search` exists as an explicit MCP tool or an equivalent explicit repo-bundle interface is implemented and documented.
- Generic `web_search` remains available and simple.
- Repo search returns grouped source-card bundles rather than a single undifferentiated result list.
- Native GitHub code/issues/releases providers improve results when configured.
- Generic provider fallback works when native providers are absent, with visible warnings.
- Suggested fetches point codegg at high-value URLs without fetching them automatically.
- Tests cover request validation, planning, grouping, ranking, warning behavior, and MCP response shape.
- Full local test suite passes.

## Handoff notes

Keep this phase focused on evidence discovery. Do not let it turn into code intelligence, repo cloning, or summarization. Codegg can combine this with its local workspace, LSP integration, and agent reasoning. Eggsearch's job is to find and classify external project evidence with strong provenance and bounded behavior.
