# Repo Search Phase 3: Optional GitHub Code Search Provider

## Context

Phases 1 and 2 established the foundation for repo-aware search under the existing `web_search` tool:

- `SourceKind` now distinguishes repo roots, source directories, source files, pull requests, tags, and commits.
- `SourceMetadata` can carry optional `CodeMetadata`.
- `RepoQueryHints` parses `repo:`, `org:`, `path:`, `file:`, `lang:`, `symbol:`, and `host:` hints.
- `SearchPlan` accepts selected provider IDs and can produce provider-specific query overrides for future repo providers.
- Adapter fan-out sends planned query strings while preserving candidate-pool limits.

Phase 3 adds the first native repo-host provider: `github_code`. This provider must remain internal to `web_search(intent = "code")`; it must not introduce a new MCP tool.

This phase also absorbs the minor deferred cleanup items from phase 1-2 review:

- verify and adjust GitHub code-search query syntax against the real API;
- improve planner dedupe if necessary;
- ensure provider-specific query scaffolding works with a real provider rather than only planner-unit tests.

## Goals

1. Add an optional `github_code` provider under the existing `web_search` pipeline.
2. Keep `web_search` discovery-only: return source cards, do not fetch source-file bodies.
3. Use the existing `SearchPlan.provider_queries["github_code"]` path.
4. Normalize GitHub API code-search results into compact source cards with deterministic `CodeMetadata`.
5. Make configuration and provider status accurately report whether `github_code` is enabled and configured.
6. Preserve no-token fallback behavior: installs without GitHub token still use generic web providers.
7. Add mocked API tests for success, empty results, rate limit, auth failure, malformed response, timeout, and bounding.
8. Avoid adding `github_search`, `repo_search`, or any new model-facing tool.

## Non-goals

Do not add GitHub issue or release providers in this phase.

Do not add GitLab or Codeberg native providers in this phase.

Do not fetch file contents in `web_search`.

Do not clone repositories.

Do not traverse repository trees.

Do not add local workspace search.

Do not synthesize answers, citations, source sufficiency, or research strategy.

Do not require a GitHub token for generic `web_search` to continue working.

## Provider identity

Add provider ID:

```text
github_code
```

This ID is internal/provider-facing. The model-facing call remains:

```json
{ "query": "repo:tokio-rs/axum Router::layer", "intent": "code" }
```

## Configuration

Reuse the existing env-backed API provider config shape where possible.

Recommended default config behavior:

```toml
[search.providers]
github_code = false

[search.api.github_code]
enabled = false
api_key_env = "GITHUB_TOKEN"
```

If the current config only builds API providers listed in `[search.api]`, then document this explicit opt-in:

```toml
[search.providers]
github_code = true

[search.api.github_code]
enabled = true
api_key_env = "GITHUB_TOKEN"
```

Rules:

- If `github_code` is disabled, it is not built.
- If enabled but the env var is missing, it should be skipped and reported as `configured = false`, not crash startup unless all providers are unavailable.
- If explicitly listed in a request's advanced `providers` array while unconfigured, the caller should get a clear unknown/unavailable provider error or a provider failure depending on existing selection semantics.
- The default provider list should not include `github_code` until operators opt in.

## Provider descriptor and capabilities

Extend provider descriptor support for `github_code`.

Current `ProviderCapabilities` does not include repo/code capabilities. Add capability fields that will also serve later phases:

```rust
pub struct ProviderCapabilities {
    pub supports_safe_search: bool,
    pub supports_freshness: bool,
    pub supports_language: bool,
    pub supports_region: bool,
    pub supports_domain_filters: bool,
    pub supports_news: bool,
    pub supports_code_search: bool,
    pub supports_repo_filter: bool,
    pub supports_org_filter: bool,
    pub supports_path_filter: bool,
    pub supports_language_filter: bool,
    pub supports_symbol_hint: bool,
    pub supports_issue_search: bool,
    pub supports_release_search: bool,
    pub supports_result_timestamps: bool,
}
```

For `github_code`:

```rust
ProviderKind::ApiKey
requires_api_key = true
supports_code_search = true
supports_repo_filter = true
supports_org_filter = true
supports_path_filter = true
supports_language_filter = true
supports_symbol_hint = true // best-effort query term, not AST-aware
supports_issue_search = false
supports_release_search = false
supports_result_timestamps = false // unless API result includes stable indexed/update time
```

Update capability summaries and tests.

## Engine integration

### Files/modules

Add a new provider implementation under the existing engines layout, for example:

```text
src/meta/engines/github_code.rs
```

Update:

```text
src/meta/engines/mod.rs
src/core/provider.rs
src/core/config.rs if needed
src/meta/adapter.rs build_default_engines
```

### Engine type

Add:

```rust
pub struct GithubCodeEngine {
    pub client: Arc<Client>,
    pub api_key: String,
    pub base_url: Option<String>,
}
```

`base_url` should default to `https://api.github.com` but remain configurable for tests and possible GitHub Enterprise support later.

Implement `SearchEngine`:

```rust
impl SearchEngine for GithubCodeEngine {
    fn name(&self) -> &'static str { "github_code" }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> { ... }
}
```

The provider should use the planned provider-specific query string already generated by `SearchPlan`.

## GitHub API behavior

Use GitHub code search API. Implementation should be based on the currently documented API behavior at implementation time.

Expected request shape:

```text
GET /search/code?q=<query>&per_page=<bounded>&page=1
Accept: application/vnd.github+json
Authorization: Bearer <token>
X-GitHub-Api-Version: 2022-11-28
User-Agent: eggsearch/<version or configured UA>
```

Important: verify actual query syntax during implementation. In particular, confirm how `filename:`, `path:`, `language:`, `repo:`, and symbol-ish free-text terms behave. The current planner scaffold maps `file:Cargo.toml` to `path:Cargo.toml`; this may need adjustment.

Suggested phase-3 correction:

- If GitHub accepts `filename:Cargo.toml`, prefer `filename:` for `file:` hints.
- If GitHub accepts `path:` for directory/path scoping, use `path:` only for `path:` hints.
- Keep symbol hints as free-text terms.
- Keep `language:` for language hints.
- Keep `repo:owner/repo` and `org:org` for scope.

If exact GitHub syntax is uncertain, implement conservatively and document what is tested.

## Result normalization

GitHub code search results should be converted into `SearchResult` values without full file content.

Expected fields from GitHub API normally include:

- file name
- path
- html URL
- repository full name
- score
- optional text matches if requested/supported

Normalize to:

```rust
SearchResult {
    title: format!("{} - {}", path, repo_full_name),
    url: html_url,
    snippet: maybe_text_match_or_repository_description_or_none,
    source_engine: "github_code".to_string(),
}
```

The existing `convert_aggregated` path will classify the `html_url` and attach `SourceKind::SourceFile` plus `CodeMetadata` if the URL is a normal GitHub blob URL.

If API returns a URL that is not a browser `html_url`, prefer the browser URL for agent selection and later `web_fetch`.

Do not include raw file contents in snippets. Snippets must be bounded and treated as untrusted, same as other search snippets.

## Metadata correctness

The provider should rely on central URL parsing where possible.

If GitHub API returns richer metadata than URL parsing, do not bypass the central metadata path yet unless needed. This phase should avoid introducing parallel metadata flows.

If richer metadata is required later, introduce an internal provider-result metadata type in a separate phase. For now, URL-derived metadata is sufficient.

## Error handling

Map GitHub API failures into existing `EngineError` classes as much as possible:

- HTTP 401/403 auth or rate limit: `BadStatus` with status and provider name.
- HTTP 422 invalid query: `BadStatus` or `ParseFailed` depending existing conventions; include concise message.
- timeout: `Timeout`.
- malformed JSON: `ParseFailed`.
- network errors: existing HTTP/network error path.

Do not expose tokens in logs or errors.

If GitHub returns a rate-limit response body, do not pass the full body through to the model. Keep provider failures compact.

## Bounding and timeouts

Use the existing per-engine timeout passed into `search`.

Clamp API `per_page` to a safe value:

```rust
let per_page = max_results.clamp(1, 100);
```

If `max_results` is zero, return an empty vector immediately or rely on upstream validation; do not panic.

Do not page beyond page 1 in this phase. Candidate-pool logic already asks for enough results.

## Planner cleanup deferred to phase 3

### Full dedupe

Current planner dedupe only removes consecutive identical terms. Upgrade it to preserve order while removing exact duplicate terms across the whole query.

Suggested helper:

```rust
fn dedupe_terms(parts: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for part in parts {
        let trimmed = part.trim();
        if trimmed.is_empty() { continue; }
        if seen.insert(trimmed.to_string()) {
            out.push(trimmed.to_string());
        }
    }
    out
}
```

Keep this exact-string dedupe only. Do not attempt tokenization or case-folding yet unless tests prove it is necessary.

### GitHub file query syntax

Update `build_github_code_query` after verifying API syntax:

- `file:` hints should likely become `filename:<file>` if supported.
- `path:` hints should remain `path:<path>` if supported.
- `symbol:` remains free text.
- `language:` remains `language:<lang>`.

Add tests matching the chosen syntax.

## Tests

### Unit tests for provider query syntax

Update or add planner tests:

- `github_code_file_hint_uses_verified_file_syntax`
- `github_code_path_hint_uses_path_syntax`
- `github_code_symbol_remains_free_text`
- `dedupe_terms_removes_nonconsecutive_duplicates`

### Mocked API tests

Use a local HTTP mock server if the repo already has a pattern for this. Otherwise add a minimal test helper.

Test cases:

1. success with one result;
2. success with multiple results, bounded by `max_results`;
3. empty result set;
4. 401 unauthorized;
5. 403 forbidden/rate-limited;
6. 422 invalid query;
7. malformed JSON;
8. timeout;
9. missing `html_url` or malformed item ignored/skipped if possible;
10. snippet/text-match bounding if implemented.

### Adapter integration tests

Use a configured `github_code` engine pointing at mock base URL.

Assert:

- `web_search(intent = Code)` with `providers = ["github_code"]` calls the mock API with provider-specific query.
- Result card has provider `github_code`.
- Result card has `metadata.source_kind = SourceFile` for blob URL.
- Result card has `metadata.code.owner`, `repo`, `path`, `ref_name`, and `language` populated.
- `fetched = false`.
- `trust = external_untrusted`.
- final response still respects `max_results`.

### Provider status tests

Assert `provider_status` reports:

- `github_code` known;
- disabled/configured false when not enabled;
- enabled/configured false when token env missing;
- enabled/configured true when token env exists;
- capabilities include code/repo/path/language filters.

## Documentation

Update README repo-search section:

- `github_code` is optional and API-key backed.
- Normal model-facing call remains `web_search` with `intent = "code"`.
- No source files are fetched by `web_search`.
- Use `web_fetch` on one selected result URL to inspect source text.
- Generic fallback remains available without `github_code`.

Add config example:

```toml
[search.providers]
github_code = true

[search.api.github_code]
enabled = true
api_key_env = "GITHUB_TOKEN"
```

Update AGENTS.md:

- Smaller agents should not call provider IDs directly unless host/debug policy exposes them.
- Prefer `intent = "code"` and hints like `repo:owner/name`, `path:...`, `file:...`, `lang:...`, `symbol:...`.

## Validation commands

Run:

```bash
cargo fmt
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

If all-feature clippy is intentionally unsupported, record the exact successful command and why.

## Final acceptance checklist

- [ ] `github_code` provider ID exists and is known to provider descriptors.
- [ ] `github_code` is disabled unless configured.
- [ ] Missing token does not crash startup if generic providers remain available.
- [ ] `provider_status` reports enabled/configured/capabilities correctly.
- [ ] `web_search(intent = "code")` can use `github_code` when selected/configured.
- [ ] `github_code` receives provider-specific planned query.
- [ ] GitHub code search results normalize to compact `SearchResult`s.
- [ ] Result cards classify GitHub blob URLs as `SourceFile` with `CodeMetadata`.
- [ ] `web_search` does not fetch source-file bodies.
- [ ] `web_search` does not clone or crawl repositories.
- [ ] Generic fallback still works without GitHub token.
- [ ] GitHub auth/rate-limit/invalid-query/malformed-response errors are bounded and classified.
- [ ] Planner dedupe and GitHub file/path query syntax are cleaned up.
- [ ] README and AGENTS document optional provider behavior without adding new model-facing tools.
