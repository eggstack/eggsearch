# Repo Search Phase 2: Repo Query Parser and Search Planner

## Context

Phase 1 adds typed repo/code metadata and deterministic URL parsing. Phase 2 makes `web_search(intent = "code" | "issues" | "releases")` operationally smarter without changing the MCP tool schema and without adding separate `github_search`, `gitlab_search`, or `repo_search` tools.

The key idea is to parse repo-oriented hints from the existing free-text `query`, then build an internal `SearchPlan` that can provide provider-specific query strings while preserving generic fallback search.

This phase should still not add native GitHub/GitLab API providers. It prepares the planner surface that Phase 3+ providers will consume.

## Goals

1. Parse repo-oriented hints from a single `web_search.query` string.
2. Keep the model-facing `web_search` schema unchanged.
3. Add an internal `SearchPlan` that distinguishes generic web-provider queries from future repo-provider queries.
4. Make `intent = code`, `issues`, and `releases` produce better generic queries immediately.
5. Preserve explicit `providers` as an advanced/host override.
6. Add tests that prove query hints are parsed, residual query is preserved, and planner output is deterministic.

## Non-goals

Do not add new MCP tools.

Do not add new model-facing fields.

Do not add GitHub/GitLab API calls in this phase.

Do not fetch source files inside `web_search`.

Do not clone repositories or crawl directories.

Do not make `eggsearch` decide source sufficiency or synthesize answers.

Do not make generic providers depend on native GitHub syntax that would degrade normal search quality.

## Current baseline

`WebSearchRequest` already includes:

```rust
pub struct WebSearchRequest {
    pub query: String,
    pub max_results: Option<usize>,
    pub providers: Vec<String>,
    pub safe_search: Option<SafeSearch>,
    pub timeout_ms: Option<u64>,
    pub intent: SearchIntent,
    pub freshness: Freshness,
}
```

`SearchIntent` already includes `Code`, `Issues`, and `Releases`, and alias deserialization maps `github`, `gitlab`, `repo`, and `repository` to `Code`.

The current adapter sends one query string to all selected engines. Phase 2 introduces a planning layer before provider fan-out.

## Deliverable 1: `RepoQueryHints`

Add a parser for structured hints embedded in the `query` string.

Suggested type:

```rust
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RepoQueryHints {
    pub host: Option<CodeHost>,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub org: Option<String>,
    pub path: Option<String>,
    pub file: Option<String>,
    pub language: Option<String>,
    pub symbol: Option<String>,
    pub residual_query: String,
}
```

Put this in a new module such as:

```text
src/core/repo_query.rs
```

or, if planner-specific, under:

```text
src/meta/planner.rs
```

Prefer `core` if the metadata is useful for tests and future providers; prefer `meta` if you want to keep it internal to search planning.

### Supported hint syntax

Parse the following canonical hints:

```text
repo:owner/name
org:owner
path:src/foo.rs
file:Cargo.toml
lang:rust
language:rust
symbol:Router::layer
host:github
```

Aliases:

```text
repository:owner/name -> repo
project:owner/name    -> repo
owner:owner           -> org, only when unambiguous
language:rust         -> lang
repo=owner/name       -> repo, optional if simple to support
```

Host aliases:

```text
github, gh -> github
gitlab, gl -> gitlab
codeberg, cb -> codeberg
```

### Owner/repo extraction

If no explicit `repo:` exists, tolerate a bare `owner/repo` token when it is unambiguous:

```text
tokio-rs/axum Router::layer
```

Parse:

```text
owner = tokio-rs
repo = axum
residual_query = Router::layer
```

Do not parse URLs in this helper; Phase 1 URL parsing handles result URLs. For query strings containing a code-host URL, either leave it in residual query or add a later helper if needed.

### Residual query behavior

The parser must remove recognized hint tokens from `residual_query` while preserving meaningful free text.

Examples:

```text
repo:tokio-rs/axum Router::layer
```

becomes:

```text
owner = tokio-rs
repo = axum
residual_query = Router::layer
```

```text
repo:rust-lang/rust path:compiler/rustc_hir lang:rust lower_impl_trait
```

becomes:

```text
owner = rust-lang
repo = rust
path = compiler/rustc_hir
language = rust
residual_query = lower_impl_trait
```

If all tokens are hints and residual query would be empty, use a safe fallback such as repo/path/symbol terms rather than producing an empty provider query.

### Validation

Do not over-validate. The parser should be tolerant but deterministic:

- reject or ignore malformed `repo:` values without `/`;
- do not panic on empty values;
- trim quotes around values when simple, e.g. `path:"src/lib.rs"`;
- preserve unknown `key:value` tokens in residual query unless the key is recognized and malformed;
- normalize language to lowercase;
- normalize host to `CodeHost`.

## Deliverable 2: `SearchPlan`

Add an internal search plan produced from a request and selected/effective providers.

Suggested type:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPlan {
    pub original_query: String,
    pub intent: SearchIntent,
    pub freshness: Freshness,
    pub hints: RepoQueryHints,
    pub generic_query: String,
    pub provider_queries: BTreeMap<String, String>,
}
```

The plan should be generated before provider fan-out. In Phase 2, most providers will still receive `generic_query`, but the `provider_queries` map lets future `github_code` providers receive native query syntax.

Suggested function:

```rust
pub fn build_search_plan(
    req: &WebSearchRequest,
    selected_provider_ids: &[String],
) -> SearchPlan
```

or:

```rust
pub fn build_search_plan(req: &WebSearchRequest) -> SearchPlan
```

If provider IDs are not needed yet, keep the function simple.

## Deliverable 3: Generic query planning by intent

### `intent = Web`

Keep generic query equal to original query, except perhaps normalized whitespace.

Do not apply repo-host filters for neutral web search.

### `intent = Docs`

No major changes in this phase. Keep docs behavior primarily in current reranking and generic providers.

### `intent = Code`

For generic providers, append safe host filters when useful:

If repo hints exist:

```text
<residual> <owner>/<repo> <path/file/lang terms> site:github.com OR site:gitlab.com OR site:codeberg.org
```

If no repo hints exist:

```text
<original query> site:github.com OR site:gitlab.com OR site:codeberg.org
```

Be careful: some providers may not handle `OR` well. Consider a conservative string such as:

```text
<query> github gitlab codeberg source repository
```

or provider-specific generic query variants if useful.

The planner should not make generic web search worse by adding too much syntax. Prefer conservative terms until real provider-specific support is added.

### `intent = Issues`

Generic query should bias toward issue URLs:

```text
<query> issues discussions pull request github gitlab
```

If repo hints exist:

```text
<residual> <owner>/<repo> issues discussions pull request
```

### `intent = Releases`

Generic query should bias toward releases/changelogs:

```text
<query> releases changelog migration guide tag github gitlab
```

If repo hints exist:

```text
<residual> <owner>/<repo> releases changelog migration tag
```

### `intent = Security`

No major repo behavior in this phase. Preserve existing behavior.

### `intent = News`

No repo behavior.

## Deliverable 4: Future provider query map

Even before native providers exist, implement the provider-query map shape for future compatibility.

Examples:

For future `github_code`:

```text
repo:tokio-rs/axum Router::layer path:axum/src lang:rust
```

For future `github_issues`:

```text
repo:tokio-rs/axum middleware ordering is:issue
```

For future `github_releases`:

```text
repo:tokio-rs/axum release changelog migration
```

Do not route to nonexistent providers yet. This phase only prepares the map and tests it.

## Deliverable 5: Adapter integration

Update `MetadataSearchAdapter::web_search` to build a `SearchPlan` before fan-out.

Current flow:

```rust
for engine in &engines {
    let query = req.query.clone();
    join_set.spawn(async move {
        let result = engine.search(&query, per_provider_limit, engine_timeout).await;
        (engine.name().to_string(), result)
    });
}
```

Target flow:

```rust
let plan = build_search_plan(req, &queried_ids);

for engine in &engines {
    let provider_id = engine.name().to_string();
    let query = plan
        .provider_queries
        .get(&provider_id)
        .cloned()
        .unwrap_or_else(|| plan.generic_query.clone());
    join_set.spawn(async move {
        let result = engine.search(&query, per_provider_limit, engine_timeout).await;
        (provider_id, result)
    });
}
```

Keep all candidate-pool behavior from the recent corrective pass. Provider calls should still receive `candidate_limit`, not final result count.

### Logging

Update debug logging to include:

```rust
intent = %req.intent.as_str(),
generic_query = %plan.generic_query,
has_repo_hints = plan.hints.has_any(),
```

Do not log API tokens or credentials. This phase should not introduce any.

## Deliverable 6: Tests

### Parser tests

Add tests for:

```text
repo:tokio-rs/axum Router::layer
repository:tokio-rs/axum Router::layer
project:tokio-rs/axum Router::layer
org:rust-lang MIR borrow checker
host:github repo:rust-lang/rust path:compiler/rustc_borrowck lang:rust
language:python repo:psf/requests Session.send
symbol:Router::layer repo:tokio-rs/axum
bare tokio-rs/axum Router::layer
malformed repo:tokio-rs does not panic
empty path: does not panic
unknown foo:bar remains in residual query
```

Assert extracted hints and residual query.

### Planner tests

Add tests for:

- `Web` intent leaves query essentially unchanged.
- `Code` intent with repo hint includes owner/repo and residual query.
- `Code` intent without repo hint adds conservative code-host/source terms.
- `Issues` intent adds issue/discussion/PR terms.
- `Releases` intent adds release/changelog/migration terms.
- Future `github_code` provider query string is generated when provider ID is present in selected providers.
- Unknown providers fall back to generic query.

### Adapter tests

Add a mock engine that records the query string it was called with.

For `intent = Code`, assert it receives the planned generic query rather than raw query.

For `intent = Web`, assert it receives the raw/normalized query.

For future provider-specific behavior, if a mock engine is named `github_code`, assert it receives the native GitHub-style query from `provider_queries`.

Also assert candidate-pool behavior remains intact: the mock should receive the candidate limit and the planned query.

## Documentation updates

Update README with a brief repo-search usage section:

```md
### Repo/code search

Use the existing `web_search` tool with `intent`:

```json
{ "query": "repo:tokio-rs/axum Router::layer", "intent": "code" }
```

`repo:`, `org:`, `path:`, `file:`, `lang:`, `language:`, `symbol:`, and `host:` hints are best-effort query hints. They do not trigger cloning, crawling, or fetching page bodies. Use `web_fetch` on one selected result URL to inspect content.
```

Update AGENTS.md with the same boundary:

- repo hints are search hints only;
- `web_search` returns source cards only;
- `web_fetch` must still be an explicit URL decision.

## Implementation notes

### Avoid brittle query syntax

Different generic providers handle advanced operators differently. Do not rely heavily on `OR`, parentheses, or provider-specific search syntax for all generic engines. Keep generic query rewriting conservative.

### Preserve user query

The plan should store `original_query` and preserve it in the final response's `query` field. Do not replace `WebSearchResponse.query` with rewritten query; that would surprise callers.

### Keep warnings minimal

Do not warn merely because repo hints were parsed. Warnings should be reserved for actionable degradation, e.g. later phases may warn that no native repo provider is configured.

### Privacy/security

Repo query planning is string manipulation only. It should not introduce network access beyond existing providers.

## Validation commands

Run:

```bash
cargo fmt
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

If all-feature clippy is intentionally unsupported, record the exact command used and why.

## Final acceptance checklist

- [ ] `RepoQueryHints` parses canonical hints.
- [ ] Alias hints are handled deterministically.
- [ ] Bare `owner/repo` can be extracted when unambiguous.
- [ ] Malformed hints do not panic.
- [ ] Unknown `key:value` tokens are preserved or handled conservatively.
- [ ] `SearchPlan` stores original query, intent, freshness, hints, generic query, and provider-specific queries.
- [ ] `Web` intent preserves raw/normalized query behavior.
- [ ] `Code` intent generates better generic code-host-oriented queries.
- [ ] `Issues` intent generates issue/discussion/PR-oriented queries.
- [ ] `Releases` intent generates release/changelog-oriented queries.
- [ ] Adapter fan-out uses planned query per provider.
- [ ] Adapter still passes candidate limit to providers.
- [ ] Final response `query` remains the user's original query.
- [ ] No new MCP tools are added.
- [ ] No provider API work is added in this phase.
- [ ] `web_search` remains discovery-only.
- [ ] README and AGENTS document repo hints as search hints only.
