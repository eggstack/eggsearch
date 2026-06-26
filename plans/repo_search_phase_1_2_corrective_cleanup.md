# Repo Search Phase 1-2 Corrective Cleanup Plan

## Context

Phases 1 and 2 of the repo-search roadmap have been attempted. The implementation landed in the expected places:

- `src/core/code_metadata.rs` for deterministic code-host URL parsing.
- `src/core/repo_query.rs` for repo-oriented query hints.
- `src/meta/planner.rs` for intent-aware query planning.
- `src/core/source_card.rs` for expanded `SourceKind` and optional `SourceMetadata.code`.
- `src/meta/adapter.rs` for planner integration and centralized metadata extraction.

The overall direction is correct: repo/code/issue/release search remains under the single `web_search` tool, and `eggsearch` still does not fetch, crawl, clone, or act as a research agent.

This cleanup plan closes the remaining phase 1-2 issues before native repo providers such as `github_code`, `github_issues`, and `github_releases` are added.

## Goals

1. Correct Codeberg source-file classification.
2. Include parsed `path`, `file`, `language`, `symbol`, `host`, and `org` hints in planned generic queries.
3. Make `build_search_plan` accept selected provider IDs so future provider-specific query overrides can be generated without reshaping the planner API again.
4. Add provider-specific query generation for future provider IDs, even before those providers exist.
5. Add adapter tests proving planned query strings and candidate limits are both passed to engines.
6. Preserve the single `web_search` model-facing surface.
7. Preserve search-only behavior: no fetching, cloning, crawling, summarization, or research-agent logic.

## Non-goals

Do not add native GitHub/GitLab/Codeberg API providers in this cleanup.

Do not add a `repo_search`, `github_search`, or `gitlab_search` tool.

Do not modify the MCP `web_search` schema.

Do not change `web_fetch` behavior.

Do not add source-file body retrieval in `web_search`.

Do not introduce persistent caching or background indexing.

Do not add model-generated ranking explanations.

## Issue 1: Codeberg source-file classification is too broad

### Current problem

`parse_codeberg_url` handles `/src/branch/...` and `/src/tag/...`, but currently classifies every `ref_kind == "branch"` as `SourceDirectory`, even when there is a concrete file path such as:

```text
https://codeberg.org/owner/repo/src/branch/main/src/lib.rs
```

That URL should classify as `SourceFile`, with:

```text
host = codeberg
owner = owner
repo = repo
ref_name = main
path = src/lib.rs
language = rust
```

The current behavior undermines Codegg's ability to distinguish source files from directory views.

### Required behavior

Classify Codeberg `/src/branch/<ref>/<path>` and `/src/tag/<ref>/<path>` based on whether the trailing path is file-like.

Suggested rule:

- If there is no trailing path after the ref, classify as `SourceDirectory`.
- If the trailing path has a known file extension, classify as `SourceFile`.
- If the trailing path has an unknown extension but appears file-like, classify as `SourceFile` only when a conservative helper says so.
- Otherwise classify as `SourceDirectory`.

### Add helper

Add a helper near `language_from_extension`:

```rust
fn looks_like_file_path(path: &str) -> bool {
    path.rsplit('/').next().is_some_and(|last| {
        last.contains('.') && !last.ends_with('.') && !last.starts_with('.')
    })
}
```

Then:

```rust
let kind = match file_path.as_deref() {
    None => SourceKind::SourceDirectory,
    Some(path) if language_from_extension(path).is_some() => SourceKind::SourceFile,
    Some(path) if looks_like_file_path(path) => SourceKind::SourceFile,
    Some(_) => SourceKind::SourceDirectory,
};
```

Use the same logic for Codeberg branch and tag paths.

### Tests

Add tests:

```rust
#[test]
fn codeberg_branch_source_file() {
    let (kind, code, _) = classify_and_extract(
        "https://codeberg.org/owner/repo/src/branch/main/src/lib.rs",
    );
    assert_eq!(kind, SourceKind::SourceFile);
    let code = code.unwrap();
    assert_eq!(code.ref_name.as_deref(), Some("main"));
    assert_eq!(code.path.as_deref(), Some("src/lib.rs"));
    assert_eq!(code.language.as_deref(), Some("rust"));
}

#[test]
fn codeberg_branch_directory() {
    let (kind, code, _) = classify_and_extract(
        "https://codeberg.org/owner/repo/src/branch/main/src",
    );
    assert_eq!(kind, SourceKind::SourceDirectory);
    assert_eq!(code.unwrap().path.as_deref(), Some("src"));
}

#[test]
fn codeberg_tag_source_file() {
    let (kind, code, _) = classify_and_extract(
        "https://codeberg.org/owner/repo/src/tag/v1.0.0/src/lib.rs",
    );
    assert_eq!(kind, SourceKind::SourceFile);
    assert_eq!(code.unwrap().ref_name.as_deref(), Some("v1.0.0"));
}
```

## Issue 2: Generic planned queries drop useful parsed hints

### Current problem

`RepoQueryHints` correctly parses `path`, `file`, `language`, `symbol`, `host`, and `org`, but `build_repo_query` mostly uses only:

- residual query
- owner/repo
- intent suffix

This means a query such as:

```text
repo:tokio-rs/axum file:Cargo.toml
```

may produce a generic query containing `tokio-rs/axum` and the generic code suffix, but not `Cargo.toml`. That is particularly harmful before native repo-host providers exist, because generic providers are the only fallback.

### Required behavior

Generic planned queries for repo intents should include parsed hints as searchable terms.

For `intent = Code`, include:

- residual query
- owner/repo
- org if present and no owner/repo
- path
- file
- language
- symbol
- host keyword if a concrete host is parsed
- conservative suffix

For `intent = Issues`, include:

- residual query
- owner/repo or org
- symbol if present
- issue/discussion/PR suffix
- host keyword if parsed

For `intent = Releases`, include:

- residual query
- owner/repo or org
- file/path if it looks like changelog/release metadata
- release/changelog/migration suffix
- host keyword if parsed

### Suggested implementation

Replace `build_repo_query(query, hints, suffix)` with intent-specific helpers:

```rust
fn build_code_generic_query(query: &str, hints: &RepoQueryHints) -> String;
fn build_issues_generic_query(query: &str, hints: &RepoQueryHints) -> String;
fn build_releases_generic_query(query: &str, hints: &RepoQueryHints) -> String;
```

Each helper should call shared utilities:

```rust
fn push_residual_and_repo_terms(parts: &mut Vec<String>, query: &str, hints: &RepoQueryHints);
fn push_repo_scope(parts: &mut Vec<String>, hints: &RepoQueryHints);
fn push_host_term(parts: &mut Vec<String>, hints: &RepoQueryHints);
fn dedupe_terms(parts: Vec<String>) -> Vec<String>;
```

### Query construction rules

Do not use advanced `OR` syntax for generic providers yet.

Do not use provider-specific native syntax in generic queries except ordinary visible text like `github` or `gitlab`.

Do not emit empty generic queries.

Avoid duplicating terms. For example, if `residual_query` already contains `Cargo.toml`, do not add it again because `file:Cargo.toml` was parsed.

Prefer simple whitespace-separated query terms.

### Examples

Input:

```text
repo:tokio-rs/axum file:Cargo.toml
intent = code
```

Expected generic query should contain at least:

```text
tokio-rs/axum Cargo.toml github gitlab codeberg source repository
```

Input:

```text
repo:tokio-rs/axum path:axum/src/routing/mod.rs symbol:Router::layer lang:rust
intent = code
```

Expected generic query should contain:

```text
tokio-rs/axum axum/src/routing/mod.rs Router::layer rust github gitlab codeberg source repository
```

Input:

```text
org:rust-lang borrow checker
intent = issues
```

Expected generic query should contain:

```text
borrow checker rust-lang issues discussions pull request github gitlab
```

Input:

```text
repo:tokio-rs/axum file:CHANGELOG.md
intent = releases
```

Expected generic query should contain:

```text
tokio-rs/axum CHANGELOG.md releases changelog migration tag github gitlab
```

### Tests

Update/add planner tests:

- `code_intent_file_hint_included_in_generic_query`
- `code_intent_path_symbol_language_included_in_generic_query`
- `code_intent_host_hint_included_in_generic_query`
- `issues_intent_org_hint_included_without_raw_org_prefix`
- `releases_intent_changelog_file_included`
- `generic_query_dedupes_terms`
- `generic_query_never_empty_when_all_hints`

## Issue 3: Planner API cannot generate provider-specific queries yet

### Current problem

`build_search_plan(req)` does not accept selected provider IDs. It always returns an empty `provider_queries` map. This was acceptable as a minimal phase-2 first pass, but it forces phase 3 to reshape the planner API before adding `github_code`.

### Required behavior

Change the public planner entry point to:

```rust
pub fn build_search_plan(
    req: &WebSearchRequest,
    selected_provider_ids: &[String],
) -> SearchPlan
```

Update adapter call site:

```rust
let plan = build_search_plan(req, &queried_ids);
```

For tests that do not care about providers, pass `&[]`.

### Provider-specific query map

Generate query overrides for future provider IDs even before the providers exist:

- `github_code`
- `github_issues`
- `github_releases`
- `gitlab_code`
- `gitlab_issues`
- `gitlab_releases`
- `codeberg_code`

Only add an entry if the provider ID appears in `selected_provider_ids`.

### Native-style query examples

For `github_code` and `intent = Code`:

```text
<residual/symbol/file/path terms> repo:owner/repo path:<path> language:<language>
```

For `github_issues` and `intent = Issues`:

```text
<residual/symbol terms> repo:owner/repo is:issue
```

For `github_releases` and `intent = Releases`:

```text
<residual/release terms> repo:owner/repo release changelog migration
```

For GitLab, use conservative future syntax for now. If GitLab native syntax is uncertain, generate a provider-specific simple query that includes owner/repo, path/file/language/symbol terms, but avoid unsupported `repo:` operators. Document this as provisional.

For Codeberg, generate simple query terms.

### Empty or missing repo hints

If owner/repo is missing, still generate a provider-specific query only if useful. For example:

```text
Router::layer language:rust
```

is acceptable for future `github_code`, but do not emit malformed `repo:` filters.

### Tests

Add tests:

```rust
#[test]
fn github_code_provider_query_generated_when_selected() { ... }

#[test]
fn github_code_provider_query_omitted_when_not_selected() { ... }

#[test]
fn github_issues_provider_query_generated_for_issues_intent() { ... }

#[test]
fn github_releases_provider_query_generated_for_releases_intent() { ... }

#[test]
fn unknown_provider_uses_generic_query() { ... }
```

Also update any existing planner tests to call the new signature.

## Issue 4: Adapter tests should prove planned query and candidate limit together

### Current problem

The previous candidate-limit work added tests proving providers receive `candidate_limit`. Phase 2 should also test that provider fan-out receives the planned query, not the raw query.

### Required behavior

Extend mock infrastructure or add a new test mock engine that records both:

- `query`
- `max_results`

Suggested type:

```rust
pub struct RecordingQueryLimitMockEngine {
    name: &'static str,
    results: Vec<SearchResult>,
    seen_query: Arc<Mutex<Option<String>>>,
    seen_limit: Arc<Mutex<Option<usize>>>,
}
```

Or extend existing `RecordingMockEngine` to optionally record query as well as limit.

### Tests

Add adapter/integration tests:

```rust
#[tokio::test]
async fn code_intent_provider_receives_planned_generic_query_and_candidate_limit() { ... }
```

Scenario:

- provider name: `mock_a`
- query: `repo:tokio-rs/axum file:Cargo.toml`
- intent: `Code`
- max_results: 2
- max_results_cap: 50

Assert:

- recorded query contains `tokio-rs/axum`
- recorded query contains `Cargo.toml`
- recorded query contains `github gitlab codeberg source repository` or whatever final suffix policy is
- recorded limit is 6
- response query field remains original query, not rewritten query

Add neutral test:

```rust
#[tokio::test]
async fn web_intent_provider_receives_raw_trimmed_query() { ... }
```

Assert:

- recorded query is the trimmed original query, or exactly whatever the planner specifies for `Web` intent;
- no repo suffix is added.

Add future provider-specific test if feasible with a mock named `github_code`:

```rust
#[tokio::test]
async fn github_code_provider_receives_provider_specific_query() { ... }
```

This may require registering `github_code` in test config or bypassing config with direct adapter calls. If config does not yet know that provider, put this at planner-unit level for now and defer adapter-level test until phase 3.

## Issue 5: Documentation should describe hint inclusion but not overpromise native providers

### README updates

Ensure the repo-search section says:

- repo hints influence search terms and future provider-specific queries;
- current generic providers use best-effort query planning;
- no native GitHub/GitLab API provider exists until later phases unless already implemented;
- `web_search` still returns source cards only;
- `web_fetch` is still required to inspect one selected URL.

Avoid wording such as:

```text
eggsearch searches GitHub code directly
```

until the native provider lands.

### AGENTS.md updates

Add guidance:

```text
When using repo search, prefer `intent = "code"`, `"issues"`, or `"releases"` and include hints such as `repo:owner/name`, `path:...`, `file:...`, `lang:...`, and `symbol:...`. These are search hints only. They do not fetch or inspect repository contents.
```

## Suggested implementation order

1. Fix Codeberg source-file classification and tests.
2. Refactor planner query construction to include parsed hints.
3. Change `build_search_plan` signature to accept selected provider IDs.
4. Add future provider-specific query generation for selected repo provider IDs.
5. Update adapter to pass `queried_ids` to planner.
6. Add query+limit recording mock and adapter tests.
7. Update README/AGENTS wording.
8. Run validation commands.

## Validation commands

Run:

```bash
cargo fmt
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

If `clippy --all-features` is intentionally unsupported, record the exact command used and why.

## Final acceptance checklist

- [ ] Codeberg `/src/branch/<ref>/<file>` with known extension classifies as `SourceFile`.
- [ ] Codeberg `/src/branch/<ref>/<directory>` classifies as `SourceDirectory`.
- [ ] Codeberg `/src/tag/<ref>/<file>` classifies as `SourceFile`.
- [ ] Generic code queries include file/path/language/symbol hints.
- [ ] Generic issue queries include org or repo scope when present.
- [ ] Generic release queries include changelog file/path hints when present.
- [ ] Generic planned queries avoid duplicate terms and never become empty.
- [ ] `build_search_plan` accepts selected provider IDs.
- [ ] `provider_queries` is populated for selected future repo-provider IDs.
- [ ] Adapter passes selected/queried provider IDs into the planner.
- [ ] Adapter fan-out sends planned query strings to providers.
- [ ] Adapter fan-out still sends candidate limits, not final limits.
- [ ] `WebSearchResponse.query` remains the user's original query.
- [ ] README and AGENTS describe repo hints as search hints only.
- [ ] No new MCP tools are added.
- [ ] No fetching, cloning, crawling, caching, or research-agent behavior is added.
