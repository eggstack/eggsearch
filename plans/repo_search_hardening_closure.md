# Repo Search Hardening and Closure Plan

## Context

The repo-search roadmap phases have now been attempted:

- Phase 1: typed repo metadata and URL parsing.
- Phase 2: repo query parser and search planner.
- Phase 3: optional `github_code` provider.
- Phase 4: optional `github_issues` and `github_releases` providers.
- Phase 5: code-host raw fetch polish.

The implementation is directionally sound. It keeps the model-facing surface under `web_search` and `web_fetch`, adds internal GitHub providers, propagates issue/release metadata, and preserves the discovery/fetch boundary.

This plan closes the hardening gaps found during review before moving on to GitLab/Codeberg native provider expansion or broader repo-search features.

## Goals

1. Fix UTF-8 unsafe snippet truncation in GitHub issue/release providers.
2. Clarify and correct freshness capability semantics.
3. Verify or disable Codeberg raw-file rewriting.
4. Add mocked HTTP coverage for GitHub providers and code-host raw fetch behavior.
5. Improve metadata preservation when duplicate URLs are aggregated.
6. Tighten provider configuration/status tests for skipped API providers.
7. Ensure docs accurately describe what is implemented versus provisional.
8. Preserve existing architectural boundaries: no new MCP tools, no crawling, no cloning, no search-result body fetching.

## Non-goals

Do not add GitLab or Codeberg native search providers.

Do not add new model-facing tools.

Do not add local workspace search.

Do not fetch issue comments, PR review comments, release assets, or source file bodies from `web_search`.

Do not add recursive fetch behavior to `web_fetch`.

Do not implement branch-name disambiguation for source-host URLs with slash-containing refs in this pass.

Do not add persistent cache or background indexing.

## Current review findings

### Good state

- `github_code`, `github_issues`, and `github_releases` exist as internal `SearchEngine` implementations.
- Provider descriptors include the new provider IDs and repo/code capability fields.
- API providers are config-gated and skipped when required env vars are missing.
- `web_search` still returns compact source cards and does not fetch full content.
- `web_fetch` still takes one explicit URL and now can rewrite recognized code-host source-file URLs to raw content URLs.
- `IssueMetadata` and `ReleaseMetadata` are exposed under `SourceMetadata`.
- `FreshnessMatch` now requires actual timestamp metadata.

### Closure gaps

- GitHub issue/release snippet truncation slices strings by byte index and can panic on non-ASCII text.
- `github_issues` and `github_releases` advertise `supports_result_timestamps = true` but `supports_freshness = false`; this distinction is ambiguous.
- Codeberg raw rewrite is implemented with `/raw/branch/...` for both branch and tag URLs despite the original plan requiring verification first.
- Provider HTTP paths need mocked tests for auth/rate-limit/invalid-query/malformed JSON/timeout/body-size behavior.
- Fetch rewrite path needs tests proving original and transformed URLs go through the same safety checks and redirect validation.
- Aggregation keeps metadata from the first provider for a canonical URL; if richer metadata arrives from a later native provider, it may be lost.

## Workstream 1: UTF-8-safe truncation

### Problem

`github_issues::truncate_body` and `github_releases::truncate_body` currently use byte slicing:

```rust
let truncated = &body[..max_chars];
```

This can panic when `max_chars` falls inside a multi-byte UTF-8 code point.

### Required change

Replace provider-local truncation with a shared UTF-8-safe helper.

Preferred option: use existing sanitizer helper if accessible:

```rust
use crate::core::sanitize::bound_text;

fn truncate_body(body: &str, max_chars: usize) -> String {
    let (bounded, _) = bound_text(body, max_chars);
    bounded
}
```

If `bound_text` has semantics not suitable for snippets, add a dedicated helper:

```rust
fn truncate_chars_lossless(body: &str, max_chars: usize) -> String {
    if body.chars().count() <= max_chars {
        return body.to_string();
    }
    body.chars().take(max_chars).collect()
}
```

Then optionally trim to last whitespace if desired, but do not slice by byte offset. If whitespace trimming is retained, use character indices:

```rust
let truncated: String = body.chars().take(max_chars).collect();
match truncated.rfind(char::is_whitespace) {
    Some(pos) if pos > 0 => truncated[..pos].to_string(),
    _ => truncated,
}
```

`pos` from `rfind` is a valid byte boundary because it comes from string search on the already-valid truncated string.

### Tests

Add provider unit tests for both `github_issues` and `github_releases`:

```rust
#[test]
fn truncate_body_handles_multibyte_utf8() {
    let body = "abc 🦀 rust 🧪 unicode";
    let out = truncate_body(body, 7);
    assert!(out.is_char_boundary(out.len()));
    assert!(out.len() <= body.len());
}
```

Also test CJK text and emoji-only text:

```rust
"修正修正修正修正"
"🦀🦀🦀🦀🦀"
```

Acceptance criteria:

- No string slicing panic on non-ASCII issue/release bodies.
- Snippets remain bounded.
- Existing ASCII snippet tests continue to pass.

## Workstream 2: Freshness capability semantics

### Problem

`github_issues` and `github_releases` set:

```rust
supports_freshness = false
supports_result_timestamps = true
```

But adapter-level freshness reranking can use their returned timestamps. This is internally valid if `supports_freshness` means provider-side query filtering only, but ambiguous for users of `provider_status`.

### Decision required

Pick one of two explicit semantics and document it.

#### Option A: Provider-side semantics

Keep:

```rust
supports_freshness = false
supports_result_timestamps = true
```

Document:

- `supports_freshness` means the provider can filter by freshness before returning results.
- `supports_result_timestamps` means eggsearch can apply local freshness reranking after retrieval.
- GitHub issue/release providers currently support timestamp-backed local freshness ranking, not provider-side freshness filtering.

This is stricter and avoids overclaiming provider filtering support.

#### Option B: User-visible semantics

Set:

```rust
supports_freshness = true
supports_result_timestamps = true
```

Document:

- `supports_freshness` means the provider can participate in freshness-aware results, either provider-side or locally using returned timestamps.

This is simpler for users but less precise.

### Recommendation

Use Option A unless there is already established meaning for `supports_freshness` as user-visible behavior rather than provider-side filtering.

If Option A is chosen, rename or document in code comments:

```rust
/// Provider supports a freshness / time-range request parameter.
pub supports_freshness: bool,
/// Provider returns result-level timestamps usable for local freshness reranking.
pub supports_result_timestamps: bool,
```

Then update README/provider-status docs to explicitly mention the distinction.

### Tests

Add tests:

- GitHub issues descriptor has `supports_result_timestamps = true`.
- GitHub releases descriptor has `supports_result_timestamps = true`.
- If keeping `supports_freshness = false`, test and document that it means no provider-side filter.
- Adapter freshness tests prove `FreshnessMatch` appears for timestamped issue/release results despite provider descriptor semantics.

Acceptance criteria:

- Provider status no longer creates ambiguous expectations.
- Freshness reranking remains timestamp-evidence-only.
- `FreshnessMatch` still never appears for generic providers or missing-date results.

## Workstream 3: Codeberg raw-file rewrite verification or disablement

### Problem

`resolve_code_host_fetch_target` rewrites Codeberg source URLs to:

```text
https://codeberg.org/{owner}/{repo}/raw/branch/{ref}/{path}
```

It also uses `branch` for tag URLs. The plan originally required Codeberg raw URL behavior to be verified before enabling rewriting.

### Required decision

Choose one:

#### Option A: Verify and keep Codeberg raw rewrite

Manually and/or test-fixture verify Codeberg raw URL shapes:

- branch source file:
  ```text
  /src/branch/main/path/file.rs -> /raw/branch/main/path/file.rs
  ```
- tag source file:
  ```text
  /src/tag/v1.2.3/path/file.rs -> /raw/tag/v1.2.3/path/file.rs
  ```

If `/raw/tag/...` is correct, update implementation to preserve ref kind rather than hardcoding `branch`.

This likely requires extending `CodeMetadata` or `CodeHostFetchTarget` to carry `ref_kind` for Codeberg. A smaller alternative is to parse the original URL directly in `code_host_fetch.rs` for Codeberg and build the correct raw path.

#### Option B: Disable Codeberg raw rewrite for now

Return `None` or `raw_url = None` for Codeberg source-file URLs until verified. Normal `web_fetch` will then fetch the browser page through existing extraction behavior.

This is safer and matches the original plan.

### Recommendation

Use Option B unless a quick verification confirms both branch and tag raw URL shapes. If verified, implement Option A with tests for branch and tag separately.

### Tests if keeping Codeberg rewrite

- `codeberg_src_branch_resolves_to_raw_branch`.
- `codeberg_src_tag_resolves_to_raw_tag`.
- `codeberg_directory_returns_none`.
- Fetch integration test with mock server for Codeberg raw URL path.

### Tests if disabling Codeberg rewrite

- `codeberg_source_file_does_not_rewrite_until_verified`.
- Codeberg URL classification remains `SourceFile` for search metadata.
- `web_fetch` uses normal URL path for Codeberg source-file browser URL.

Acceptance criteria:

- README and AGENTS do not overstate Codeberg raw fetch support.
- No unverified tag URL rewrite remains.
- GitHub raw rewrite remains required and tested.
- GitLab raw rewrite remains tested if kept.

## Workstream 4: Mocked HTTP coverage for GitHub providers

### Problem

Provider modules have useful conversion tests, but the actual HTTP paths need mocked coverage.

### Test infrastructure

Use the repo's existing HTTP mock approach if present. If none exists, add one minimal dev dependency such as `httpmock`, `wiremock`, or a tiny local `hyper` test server.

Keep tests deterministic and local. No live GitHub calls.

### GitHub code provider tests

Add mocked HTTP tests for `github_code::search`:

1. success with one result;
2. success with multiple results bounded by `max_results`;
3. empty result set;
4. HTTP 401 unauthorized -> `EngineError::BadStatus { status: 401 }`;
5. HTTP 403 rate limit/forbidden -> `BadStatus { status: 403 }`;
6. HTTP 422 invalid query -> `BadStatus { status: 422 }`;
7. malformed JSON -> `ParseFailed`;
8. response body over `MAX_BODY_BYTES` -> `ParseFailed`;
9. timeout -> `Timeout`;
10. request includes expected headers: Accept, Authorization, X-GitHub-Api-Version.

### GitHub issues provider tests

Add equivalent tests for `github_issues::search`:

- correct endpoint `/search/issues`;
- provider-specific query is sent as `q`;
- snippets are UTF-8 safe and bounded;
- PR item with `pull_request` metadata maps to `is_pull_request = true`;
- timestamp fields preserved;
- 401/403/422/malformed/timeout cases.

### GitHub releases provider tests

Add tests for `github_releases::search`:

- query with `repo:owner/name` calls `/repos/owner/name/releases`;
- no repo query returns empty without network call;
- draft releases are skipped;
- prerelease flag is preserved;
- timestamp fields preserved;
- 404/401/403/malformed/timeout cases;
- snippets are UTF-8 safe and bounded.

### Adapter/provider integration tests

Add tests at adapter level if not already present:

- configured `github_code` provider receives provider-specific planned query.
- configured `github_issues` provider receives issue query and returns `SourceKind::IssueThread` or `PullRequest` as appropriate.
- configured `github_releases` provider returns `SourceKind::ReleaseNotes`.
- result cards from native providers include native provider rank reasons.
- final result count still respects caller `max_results`.

Acceptance criteria:

- Provider network paths are tested without live network.
- Auth/rate-limit/invalid-query/malformed-response paths are classified.
- Headers and endpoints are verified.
- No token is logged or exposed in errors.

## Workstream 5: Mocked HTTP coverage for code-host fetch rewrite

### Problem

The fetch rewrite path validates original and raw URLs and follows redirects with safety checks, but needs regression tests proving the behavior.

### Tests

Add fetch-client tests with a local mock server where possible.

Required tests:

1. GitHub blob URL rewrites to raw URL and returns raw source text.
2. `fetch_transform` is present for rewritten source-file URLs.
3. Non-code URL has no `fetch_transform`.
4. Repo root/tree/issue/PR/release URLs are not rewritten.
5. Raw endpoint 404 returns `FetchError::HttpStatus`.
6. Raw endpoint timeout returns timeout error.
7. Raw endpoint redirect to private/localhost URL is blocked.
8. Original localhost/private URL is rejected before rewrite.
9. Transformed raw URL is validated before fetch.
10. Oversized raw file is bounded by max bytes.
11. Unsupported binary content type is rejected.
12. If `application/octet-stream` support is intentionally absent, document/test rejection.

### Optional content-type improvement

If desired, add UTF-8 sniffing for `application/octet-stream` raw files:

- accept octet-stream only when body bytes are valid UTF-8 and not too large;
- reject binary-looking octet-stream.

This is useful because raw code hosts sometimes serve source as octet-stream. However, conservative rejection is acceptable for closure if documented.

Acceptance criteria:

- Fetch rewrite path has local deterministic tests.
- Safety validation order is protected by tests.
- `web_fetch` remains one URL in, one bounded document out.

## Workstream 6: Metadata merge behavior during aggregation

### Problem

`aggregate_rrf` keeps the first `ResultMetadata` for a normalized URL. If a generic provider result appears first and a native provider result appears later for the same URL, richer native metadata can be lost.

### Required change

Add a deterministic merge function:

```rust
fn merge_result_metadata(existing: &mut ResultMetadata, incoming: ResultMetadata) {
    match (&existing, incoming) {
        (ResultMetadata::None, richer) => *existing = richer,
        (ResultMetadata::Issue(_), ResultMetadata::None) => {}
        (ResultMetadata::Release(_), ResultMetadata::None) => {}
        // If both are same variant, prefer the one with more populated fields.
        ...
        // If conflicting variants, prefer non-None from native provider only if URL kind agrees.
    }
}
```

Simpler acceptable rule:

- `None` never replaces non-None.
- non-None replaces `None`.
- same variant with more fields replaces less-populated variant.
- conflicting non-None variants keep existing and log debug.

Add helper to count populated fields for `IssueMetadata` and `ReleaseMetadata`.

Call it in aggregation when a duplicate URL is merged.

### Tests

- generic first, native issue second -> issue metadata retained.
- native issue first, generic second -> issue metadata retained.
- native release richer second -> richer release metadata retained.
- conflicting issue/release metadata does not panic and keeps deterministic choice.

Acceptance criteria:

- Duplicate URL merge does not drop richer metadata.
- RRF provider merging still works.
- Snippet/title existing behavior remains unchanged unless explicitly improved.

## Workstream 7: Documentation and changelog closure

### README updates

Ensure README clearly states:

- `github_code`, `github_issues`, and `github_releases` are optional internal providers.
- The model-facing tool remains `web_search`.
- `web_search` does not fetch source files, issue comments, release assets, or linked pages.
- `web_fetch` can fetch one explicit source-file URL as raw text for supported hosts.
- Codeberg raw fetch is either verified and supported or explicitly not supported yet.
- `supports_freshness` versus `supports_result_timestamps` semantics are clear.
- Generic fallback works without GitHub tokens.

### AGENTS.md updates

Ensure AGENTS.md says:

- Use `intent = "code"`, `"issues"`, or `"releases"` with repo hints.
- Treat snippets and fetched code as `external_untrusted`.
- Do not use `web_fetch` to crawl adjacent files.
- Use Codegg local tools for local workspace search.

### CHANGELOG

Add concise entries for:

- UTF-8-safe GitHub snippet truncation.
- GitHub provider HTTP hardening tests.
- Codeberg raw fetch support status.
- Metadata merge fix.

## Suggested implementation order

1. Fix UTF-8-safe truncation in `github_issues` and `github_releases`.
2. Decide and document freshness capability semantics.
3. Verify or disable Codeberg raw rewrite; adjust tests/docs accordingly.
4. Add mocked HTTP tests for `github_code`, `github_issues`, `github_releases`.
5. Add mocked fetch rewrite tests.
6. Implement metadata merge helper in RRF aggregation.
7. Update README, AGENTS, and CHANGELOG.
8. Run validation commands.

## Validation commands

Run:

```bash
cargo fmt
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

If all-feature clippy is unsupported, record the exact successful command and why.

## Final acceptance checklist

- [ ] GitHub issue snippet truncation is UTF-8 safe.
- [ ] GitHub release snippet truncation is UTF-8 safe.
- [ ] Non-ASCII body tests cannot panic.
- [ ] Freshness capability semantics are explicitly documented and tested.
- [ ] `FreshnessMatch` remains timestamp-evidence-only.
- [ ] Codeberg raw fetch is verified and correct, or disabled/deferred.
- [ ] GitHub provider HTTP paths have local mocked tests for success and failures.
- [ ] Code-host fetch rewrite path has local mocked safety tests.
- [ ] Duplicate URL aggregation preserves richer native metadata.
- [ ] Provider status accurately reports new provider capabilities/configured states.
- [ ] README/AGENTS accurately describe supported behavior and boundaries.
- [ ] No new MCP tools are added.
- [ ] `web_search` remains discovery-only.
- [ ] `web_fetch` remains one explicit URL, not a crawler.
