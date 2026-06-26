# Repo Search Roadmap

## Context

`eggsearch` is now a bounded MCP metasearch and single-URL fetch substrate intended for Codegg. The current tool surface is deliberately simple:

- `web_search` discovers candidate public sources and returns compact `SourceCard`s.
- `web_fetch` fetches one explicit HTTP(S) URL and returns bounded extracted text.
- `provider_status` is diagnostic and host/UI-facing.

Recent work added optional `intent` and `freshness` hints, deterministic `SourceCard.metadata`, candidate-pool reranking, and strict search/fetch separation. This creates the right base for repo-aware search without introducing separate model-facing tools such as `github_search` or `gitlab_search`.

The goal of this roadmap is to make public remote repository search first-class under the existing `web_search` tool, especially for Codegg research-agent use.

## Design position

Do not add separate model-facing repo/code tools.

Preferred model-facing calls remain:

```json
{ "query": "repo:tokio-rs/axum Router::layer", "intent": "code", "max_results": 10 }
```

```json
{ "query": "axum middleware ordering", "intent": "issues", "freshness": "year" }
```

```json
{ "query": "tokio 1.48 breaking changes", "intent": "releases" }
```

Internally, `eggsearch` may add provider IDs such as `github_code`, `github_issues`, `github_releases`, `gitlab_code`, `gitlab_issues`, `gitlab_releases`, and `codeberg_code`. These are provider/adaptor choices, not new MCP tools.

## Non-goals

Do not turn `eggsearch` into a research agent.

Do not add `include_content` to `web_search`.

Do not fetch page bodies or source files inside `web_search`.

Do not clone repositories.

Do not crawl repositories, branches, directories, issues, or links.

Do not add local workspace search. Codegg owns local repo search via ripgrep, tree-sitter, LSP, and project indexes.

Do not add answer synthesis, source sufficiency decisions, citation synthesis, or model-generated ranking explanations.

## Current architecture fit

The current repo already has the necessary seams:

- `SearchEngine` is a small provider trait: provider name plus `search(query, max_results, timeout)`.
- Provider descriptors already expose kind, configured/enabled/default state, and feature capabilities.
- `SearchIntent` already includes `Code`, `Issues`, and `Releases`.
- The adapter already performs provider fan-out, RRF aggregation, candidate-pool reranking, truncation, and failure accounting.
- `SourceCard` already has deterministic metadata and trust markers.
- `web_fetch` already handles one explicit URL with SSRF and content-safety boundaries.

The main gaps are:

- Generic `SearchResult` contains only title, URL, snippet, and source engine.
- `SourceKind` cannot distinguish source files from repository roots.
- There is no typed code/repo metadata in `SourceCard`.
- There is no repo-query parser for `repo:`, `org:`, `path:`, `lang:`, or symbol hints.
- There is no search planner that creates provider-specific queries from a single model-facing `web_search` call.
- There are no native repo-host providers yet.

## Target architecture

### Model-facing surface

Keep the MCP schema essentially unchanged:

```json
{
  "query": "string",
  "intent": "web|docs|code|issues|releases|security|news",
  "freshness": "any|day|week|month|year",
  "max_results": 10
}
```

Only `query` remains required. `intent`, `freshness`, and `max_results` remain optional.

### Internal pipeline

The target search pipeline should become:

1. validate `WebSearchRequest`;
2. parse repo/code hints from `query`;
3. build a `SearchPlan` from intent, freshness, hints, configured providers, and explicit providers if supplied;
4. compute candidate pool size;
5. fan out with provider-specific planned queries;
6. normalize provider results into a richer internal result type;
7. aggregate and deduplicate;
8. attach deterministic source/repo metadata;
9. apply bounded intent-aware reranking;
10. truncate to final `max_results`;
11. return compact `SourceCard`s.

### Result shape

Code-aware results should still be `SourceCard`s, but metadata should optionally contain structured code/repo details:

```json
{
  "id": "src_...",
  "title": "axum/src/routing/mod.rs - tokio-rs/axum",
  "url": "https://github.com/tokio-rs/axum/blob/main/axum/src/routing/mod.rs",
  "snippet": "...bounded snippet...",
  "providers": ["github_code"],
  "trust": "external_untrusted",
  "fetched": false,
  "metadata": {
    "source_kind": "source_file",
    "domain": "github.com",
    "rank_reasons": ["provider_native_code_search", "repo_exact_match", "intent_match"],
    "code": {
      "host": "github",
      "owner": "tokio-rs",
      "repo": "axum",
      "path": "axum/src/routing/mod.rs",
      "ref_name": "main",
      "language": "rust",
      "symbol_hint": "Router::layer"
    }
  }
}
```

All repo/code metadata must be deterministic. Do not include generated prose explanations.

## Phase 1: Typed repo metadata and URL parsing

### Goal

Teach `eggsearch` to classify and expose repository-aware metadata from URLs and provider results without changing the model-facing tool surface.

### Key deliverables

- Extend `SourceKind` with code-host-aware variants.
- Add `CodeHost`, `CodeMetadata`, and optional `SourceMetadata.code`.
- Add deterministic URL parsers for GitHub, GitLab, and Codeberg URL shapes.
- Preserve backward-compatible JSON for non-code results.
- Add tests for source files, directories, repo roots, issues, PRs, releases, tags, commits, and unknown URLs.

### Expected value

This phase gives smaller agents enough metadata to distinguish source files from repo roots and issue threads. It also prevents later provider work from cramming repo semantics into snippets or titles.

## Phase 2: Repo-query parser and search planner

### Goal

Parse repo-oriented hints from a single `web_search.query` string and build provider-specific planned queries without exposing new model-facing fields or tools.

### Key deliverables

- Add `RepoQueryHints` parser for `repo:`, `org:`, `path:`, `file:`, `lang:`, `symbol:`, and `host:`.
- Add weak-model aliases such as `repository:`, `language:`, and obvious `owner/repo` extraction.
- Add `SearchPlan` generation based on `SearchIntent`, `Freshness`, parsed hints, and selected providers.
- Keep generic providers useful by rewriting generic queries with safe site/path terms where appropriate.
- Preserve explicit `providers` behavior as an advanced/host override.

### Expected value

This phase makes `intent="code"`, `intent="issues"`, and `intent="releases"` operationally meaningful before native repo-host APIs exist. It also creates the routing point for later `github_code` and `github_issues` providers.

## Phase 3: Optional GitHub code search provider

### Goal

Add an optional native GitHub code-search provider under the existing `web_search(intent="code")` path.

### Key deliverables

- Add provider ID `github_code`.
- Add config support using env-backed API key, preferably `GITHUB_TOKEN` by default or configurable `api_key_env`.
- Add provider descriptor/capabilities for native code search.
- Implement GitHub code search API adapter with bounded result counts and clear error classification.
- Map GitHub API results into internal provider results with `CodeMetadata`.
- Keep provider disabled unless configured.
- Add mocked API tests and provider-status tests.

### Expected value

This is the first major quality jump for Codegg. It lets `web_search` return source-file candidates rather than generic web pages when the research agent needs upstream code evidence.

## Phase 4: GitHub issues and releases providers

### Goal

Add issue/PR/discussion and release/tag/changelog search under `web_search(intent="issues")` and `web_search(intent="releases")`.

### Key deliverables

- Add provider IDs `github_issues` and `github_releases`.
- Add typed issue and release metadata.
- Add timestamp-backed freshness semantics for issue/release results.
- Emit `FreshnessMatch` only when actual timestamp evidence exists.
- Add `RankReason` variants for native issue/release search and timestamp evidence.
- Add mocked API tests.

### Expected value

Coding agents often need bug discussions, migration notes, and release history more than raw source files. This phase makes that workflow first-class without adding more tools.

## Phase 5: Intent-aware provider selection and fallback policy

### Goal

Make provider choice depend on intent while preserving generic fallback behavior.

### Key deliverables

- If `providers` is omitted, choose repo-capable providers first for `code`, `issues`, and `releases` when configured.
- Always keep generic fallback providers available unless config disables them.
- Add warnings when a specialized intent was requested but no native provider is configured and only generic fallback was used.
- Add provider capability flags for code/repo search.
- Update `provider_status` and CLI provider views.

### Expected value

No-token installs still work. Token-configured installs get better repo search. Smaller models do not need to know provider IDs.

## Phase 6: Repo-aware reranking and rank reasons

### Goal

Make code/issues/releases reranking use structured metadata rather than broad URL/domain classes.

### Key deliverables

- Add rank reasons such as `repo_exact_match`, `path_match`, `language_match`, `symbol_hint_match`, `provider_native_code_search`, `provider_native_issue_search`, and `provider_native_release_search`.
- Update reranking priorities for `Code`, `Issues`, and `Releases`.
- Keep all boosts bounded so RRF/provider evidence remains dominant.
- Add tests proving neutral `Web` ordering remains stable.

### Expected value

Agents see better top results without hidden multi-step behavior or prose scoring explanations.

## Phase 7: Code-host fetch polish

### Goal

Improve `web_fetch` for explicit code-host source URLs while preserving the one-URL boundary.

### Key deliverables

- Detect GitHub/GitLab/Codeberg source-file URLs.
- For GitHub blob URLs, optionally fetch the corresponding raw file URL after normal URL/redirect/SSRF validation.
- Return bounded text only; reject or bound binary/large files.
- Include source-kind/language metadata in fetch responses where deterministic.
- Do not clone repos, list directories, or follow links.

### Expected value

A Codegg research agent can search for a source file, select the best `SourceCard`, then fetch exactly that file through the existing `web_fetch` contract.

## Phase 8: GitLab and Codeberg expansion

### Goal

Extend the provider abstraction to additional code hosts after GitHub semantics have stabilized.

### Key deliverables

- Add `gitlab_code`, `gitlab_issues`, `gitlab_releases` for configured GitLab instances.
- Add `codeberg_code` or generic Forgejo/Gitea support where feasible.
- Keep each provider optional and config-gated.
- Reuse `CodeMetadata` and result-normalization pipeline.

### Expected value

This broadens remote repository coverage without changing Codegg's tool usage.

## Phase 9: Codegg integration guidance

### Goal

Document how Codegg should use repo search without exposing more tools to weaker agents.

### Key deliverables

- Add docs showing `web_search(intent="code")`, `web_search(intent="issues")`, `web_search(intent="releases")`, and `web_fetch(url)` patterns.
- Document which fields Codegg should hide or treat as advanced: `providers`, `timeout_ms`, `safe_search`.
- Clarify that Codegg owns local search, budgets, duplicate suppression, research strategy, sufficiency decisions, and citations.
- Clarify that `eggsearch` owns remote public search, source cards, single-URL fetch, SSRF safety, and untrusted labeling.

## Implementation sequencing

Recommended order:

1. Phase 1: metadata and URL parsing.
2. Phase 2: query parser and search planner.
3. Phase 3: `github_code` provider.
4. Phase 4: `github_issues` and `github_releases` providers.
5. Phase 5: intent-aware provider selection/fallback policy.
6. Phase 6: repo-aware reranking.
7. Phase 7: code-host fetch polish.
8. Phase 8: GitLab/Codeberg expansion.
9. Phase 9: Codegg integration guidance.

Phases 1 and 2 should land before provider API work. They define the stable metadata and planning surfaces that later provider implementations will use.

## Validation baseline

Every phase should run:

```bash
cargo fmt
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

If `clippy --all-features` is intentionally unsupported, record the exact command used and why.

## Global acceptance criteria

The full repo-search line of work is complete when:

- `web_search(intent="code")` can return structured source-file/repo results when providers support it.
- `web_search(intent="issues")` can return structured issue/PR/discussion results.
- `web_search(intent="releases")` can return structured release/tag/changelog results.
- Generic fallback remains functional without API tokens.
- No new model-facing search tools are required.
- `SourceCard` remains compact and deterministic.
- `web_search` remains discovery-only.
- `web_fetch` remains one explicit URL per call.
- No cloning, crawling, summarization, or research-agent behavior is added to `eggsearch`.
