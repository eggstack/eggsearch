# Repo Search Phase 4: GitHub Issues and Releases Providers

## Context

Phase 3 adds the optional `github_code` provider under the existing `web_search(intent = "code")` flow. Phase 4 extends the same pattern to issue/PR/discussion and release/changelog discovery:

- `web_search(intent = "issues")` should use a native `github_issues` provider when configured.
- `web_search(intent = "releases")` should use a native `github_releases` provider when configured.

These are internal provider IDs, not new MCP tools. Model-facing calls remain:

```json
{ "query": "repo:tokio-rs/axum panic middleware", "intent": "issues" }
```

```json
{ "query": "repo:tokio-rs/axum breaking changes", "intent": "releases" }
```

This phase should continue to keep `eggsearch` as a retrieval substrate. Codegg's research agent decides which results to fetch, whether sources are sufficient, and how to synthesize an answer.

## Goals

1. Add optional `github_issues` provider under `web_search(intent = "issues")`.
2. Add optional `github_releases` provider under `web_search(intent = "releases")`.
3. Preserve the single `web_search` model-facing tool.
4. Normalize GitHub API results into compact `SourceCard`s with deterministic metadata.
5. Add typed issue/release metadata where useful and stable.
6. Use actual timestamps for freshness-aware ranking and `FreshnessMatch` only when evidence exists.
7. Keep generic fallback behavior for no-token installs.
8. Add mocked API tests for success and failure modes.

## Non-goals

Do not add `github_search`, `github_issue_search`, or `github_release_search` tools.

Do not fetch issue bodies beyond bounded snippets required for search-result cards.

Do not fetch issue comments, PR review comments, release assets, or changelog files in this phase.

Do not crawl linked pages.

Do not clone repositories.

Do not synthesize answers or decide source sufficiency.

Do not implement GitLab/Codeberg native providers in this phase.

## Provider IDs

Add provider IDs:

```text
github_issues
github_releases
```

These should be optional API-key-backed providers similar to `github_code`.

## Configuration

Recommended explicit opt-in:

```toml
[search.providers]
github_issues = true
github_releases = true

[search.api.github_issues]
enabled = true
api_key_env = "GITHUB_TOKEN"

[search.api.github_releases]
enabled = true
api_key_env = "GITHUB_TOKEN"
```

If Phase 3 added shared GitHub API config, prefer reusing it to avoid requiring duplicate token config. For example:

```toml
[search.github]
enabled = true
api_key_env = "GITHUB_TOKEN"
code = true
issues = true
releases = true
```

Do not do a large config migration unless necessary. If reusing the existing `[search.api]` map is simpler, keep that for now.

Rules:

- Disabled provider IDs are not built.
- Enabled but missing token means `configured = false` and provider is skipped if other providers exist.
- Generic providers remain available.
- `github_issues` and `github_releases` should not be default providers unless explicitly configured by the operator.

## Provider capabilities

For `github_issues`:

```text
supports_issue_search = true
supports_repo_filter = true
supports_org_filter = true
supports_freshness = true, if API query supports updated/created qualifiers or returned timestamps are used locally
supports_result_timestamps = true
supports_code_search = false
supports_release_search = false
```

For `github_releases`:

```text
supports_release_search = true
supports_repo_filter = true
supports_org_filter = true, if implementation supports org/repo discovery safely
supports_freshness = true, if returned published_at/created_at is used locally
supports_result_timestamps = true
supports_code_search = false
supports_issue_search = false
```

Update provider status and capability summary tests.

## Metadata model

### Option A: extend `CodeMetadata`

This is the simplest path if you want minimal schema churn. Existing `CodeMetadata` can carry host/owner/repo/ref/path/language, but it does not represent issue number, PR number, state, labels, or release tag cleanly.

### Option B: add optional nested metadata under `SourceMetadata`

Preferred for correctness:

```rust
pub struct SourceMetadata {
    pub source_kind: SourceKind,
    pub domain: Option<String>,
    pub rank_reasons: Vec<RankReason>,
    pub code: Option<CodeMetadata>,
    pub issue: Option<IssueMetadata>,
    pub release: Option<ReleaseMetadata>,
}
```

Suggested issue metadata:

```rust
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct IssueMetadata {
    pub host: Option<CodeHost>,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub number: Option<u64>,
    pub state: Option<String>,
    pub is_pull_request: Option<bool>,
    pub labels: Vec<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub closed_at: Option<String>,
}
```

Suggested release metadata:

```rust
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReleaseMetadata {
    pub host: Option<CodeHost>,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub tag: Option<String>,
    pub name: Option<String>,
    pub draft: Option<bool>,
    pub prerelease: Option<bool>,
    pub created_at: Option<String>,
    pub published_at: Option<String>,
}
```

If this feels too large for phase 4, add only `published_at`/`updated_at` via an internal provider-result metadata layer and defer public `issue`/`release` metadata. However, Codegg benefits from structured issue/release metadata, so Option B is recommended.

All timestamp strings should be normalized RFC 3339 strings. Do not expose provider-specific date formats.

## Internal provider result metadata

The current `SearchResult` is generic. For issue/release metadata and timestamps, strongly consider adding an internal result wrapper rather than overloading snippets:

```rust
pub struct ProviderSearchResult {
    pub title: String,
    pub url: String,
    pub snippet: Option<String>,
    pub source_engine: String,
    pub metadata: ProviderResultMetadata,
}

pub enum ProviderResultMetadata {
    None,
    Issue(IssueMetadata),
    Release(ReleaseMetadata),
}
```

Then generic engines can produce `None`, while GitHub providers produce typed metadata. Aggregation should preserve metadata from the first/best provider for a canonical URL.

If this is too invasive, a smaller implementation can normalize URLs and rely on URL parsing for owner/repo/number/tag while deferring labels/state/timestamps. But freshness support requires timestamp evidence, so some metadata path is needed.

## GitHub issues provider

### API options

Possible implementation paths:

1. GitHub Search Issues API:
   ```text
   GET /search/issues?q=<query>&per_page=<bounded>&page=1
   ```
   This supports searching issues and PRs using qualifiers such as `repo:`, `org:`, `is:issue`, `is:pr`, `state:open`, `updated:>=YYYY-MM-DD`.

2. Repository issues API when `repo:owner/name` is known:
   ```text
   GET /repos/{owner}/{repo}/issues?state=all&per_page=...
   ```
   This is less flexible for text search unless combined with local filtering, so prefer search API first.

Preferred: use Search Issues API for Phase 4.

### Query generation

Use the provider-specific query already generated by planner for `github_issues`.

Verify syntax during implementation. Expected shape:

```text
panic middleware repo:tokio-rs/axum is:issue
```

For PRs, decide whether `intent = issues` should include PRs by default. The roadmap treated PRs/discussions as issue-like. GitHub Search Issues API can search issues and PRs. Recommended behavior:

- default `github_issues` query includes `is:issue` for pure issues;
- consider adding a separate `PullRequest` boost later;
- do not include `is:pr` unless the user query or hints mention `pr`, `pull request`, or `merge`.

This keeps results tighter.

### Result normalization

For each API item:

```rust
SearchResult {
    title: format!("#{} {} - {}/{}", number, title, owner, repo),
    url: html_url,
    snippet: bounded body excerpt or None,
    source_engine: "github_issues".to_string(),
}
```

Attach issue metadata if the internal metadata path exists:

- number
- state
- labels
- created_at
- updated_at
- closed_at
- is_pull_request

Do not include full issue body if it is large; snippets must be bounded.

## GitHub releases provider

### API options

If `repo:owner/name` is present, prefer repository releases API:

```text
GET /repos/{owner}/{repo}/releases?per_page=<bounded>
```

If no repo is present, use generic web fallback or GitHub search if a reliable releases search path exists. GitHub does not have a direct global release search equivalent as clean as code/issues search.

Recommended Phase 4 behavior:

- `github_releases` only emits provider-specific results when `owner/repo` is known.
- If no repo scope is known, return empty results from `github_releases` quickly and let generic providers handle fallback, or do not build a provider-specific query for `github_releases` without repo hints.

### Result normalization

For each release:

```rust
SearchResult {
    title: format!("{} {} - {}/{}", tag_name, name_or_empty, owner, repo),
    url: html_url,
    snippet: bounded body excerpt or None,
    source_engine: "github_releases".to_string(),
}
```

Attach release metadata if available:

- tag
- name
- draft
- prerelease
- created_at
- published_at

Skip draft releases unless there is a strong reason to include them. Public API may not return drafts without auth/permission; still handle defensively.

## Freshness semantics

This is the first phase where `FreshnessMatch` can become legitimate.

Rules:

- Only emit `FreshnessMatch` when a result has actual timestamp evidence.
- For issues, use `updated_at` as the primary freshness timestamp.
- For releases, use `published_at` as the primary freshness timestamp, falling back to `created_at` if `published_at` is missing.
- Do not use scrape time as freshness evidence.

Implement helper:

```rust
fn matches_freshness(ts: DateTime<Utc>, freshness: Freshness, now: DateTime<Utc>) -> bool
```

Map:

```text
Day   -> now - 1 day
Week  -> now - 7 days
Month -> now - 30 days
Year  -> now - 365 days
Any   -> always false for FreshnessMatch reason; no freshness boost needed
```

Use current time only inside adapter/reranker or pass a clock for tests.

Reranking:

- Add a small bounded boost for freshness only when timestamp evidence exists and requested freshness is not `Any`.
- Do not let freshness override strong relevance/provider evidence.
- Add tests with fixed clock.

## Rank reasons

Add or use rank reasons:

```rust
ProviderNativeIssueSearch
ProviderNativeReleaseSearch
RepoExactMatch
IssueStateMatch
ReleaseTagMatch
FreshnessMatch
```

If rank-reason expansion is deferred, at minimum emit existing `IntentMatch`, `DomainPriorRelease`, and `FreshnessMatch` correctly.

Do not emit native provider rank reasons unless the result actually came from the native provider.

## Error handling

Map GitHub API failures into existing provider failure classes:

- 401/403 -> `BadStatus`, surfaced as provider failure.
- 404 repository not found for releases -> provider failure or empty results depending semantics; prefer provider failure if explicit repo scope was requested.
- 422 invalid query -> provider failure with concise message.
- rate limit -> `BadStatus` 403 or 429 as returned.
- malformed JSON -> `ParseFailed`.
- timeout -> `Timeout`.

Do not expose tokens or full response bodies.

## Tests

### Unit tests

- `matches_freshness` for day/week/month/year with fixed clock.
- issue metadata serde omits empty fields.
- release metadata serde omits empty fields.
- GitHub issue result normalization preserves number/state/labels/timestamps.
- GitHub release result normalization preserves tag/prerelease/published timestamp.

### Mocked API tests: issues

- successful issue search with one result;
- successful issue search with PR item and `is_pull_request = true`;
- labels extracted and bounded;
- body snippet bounded;
- 401/403/422/malformed JSON/timeout;
- empty result set.

### Mocked API tests: releases

- successful repo release list;
- prerelease metadata retained;
- draft release skipped or handled according to chosen policy;
- body snippet bounded;
- no repo hint returns empty/fallback behavior without malformed API request;
- 404/401/403/malformed JSON/timeout.

### Adapter tests

- `web_search(intent = Issues, providers = ["github_issues"])` returns `IssueThread` cards.
- PR results classify as `PullRequest` when URL is `/pull/<number>`.
- `web_search(intent = Releases, providers = ["github_releases"])` returns `ReleaseNotes` cards.
- `FreshnessMatch` appears only for timestamped results within requested freshness.
- `FreshnessMatch` does not appear for generic providers or missing timestamps.
- `web_search` result cards remain `fetched = false`.

### Provider status tests

- `github_issues` and `github_releases` appear in known provider descriptors.
- configured/enabled/token states are accurate.
- capability flags are accurate.

## Documentation

Update README:

- Add optional GitHub issues/releases config examples.
- Show model-facing examples using `web_search` with `intent = "issues"` and `intent = "releases"`.
- State that issue/release providers are optional and API-key backed.
- State that `web_search` does not fetch full issue threads, comments, release assets, or changelog files.
- State that `web_fetch` can be used on one selected URL afterward.

Update AGENTS.md:

- Research agents should use `intent = "issues"` for bug reports, issue discussions, PR context, and upstream behavior reports.
- Research agents should use `intent = "releases"` for migration notes, breaking changes, version history, and changelogs.
- Treat snippets and metadata as untrusted evidence until fetched/verified.

## Validation commands

Run:

```bash
cargo fmt
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

If all-feature clippy is unsupported, record the exact successful command and reason.

## Final acceptance checklist

- [ ] `github_issues` provider is known, optional, and config-gated.
- [ ] `github_releases` provider is known, optional, and config-gated.
- [ ] Missing GitHub token does not break generic search.
- [ ] Provider status reports issue/release capabilities accurately.
- [ ] `web_search(intent = "issues")` can use `github_issues` when selected/configured.
- [ ] `web_search(intent = "releases")` can use `github_releases` when selected/configured.
- [ ] Issue results normalize to compact source cards with bounded snippets.
- [ ] Release results normalize to compact source cards with bounded snippets.
- [ ] Timestamp-backed `FreshnessMatch` is implemented only when evidence exists.
- [ ] No freshness reason is emitted for generic/missing-date results.
- [ ] No issue comments, release assets, source files, or linked pages are fetched by `web_search`.
- [ ] No new MCP tools are added.
- [ ] README and AGENTS document the intended single-tool usage.
