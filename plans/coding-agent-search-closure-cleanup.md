# Coding-Agent Search Closure Cleanup Plan

## Purpose

This plan closes the remaining small issues after the corrective hardening pass for eggsearch's coding-agent search surface. The substantive architecture is now in place: exact code evidence, `repo_fetch`, workspace locators, package-aware repo search, local workspace search, local trust-marker scanning, tool capability reporting, and profile fallback behavior. This pass should be a narrow cleanup pass, not a new feature phase.

The goal is to leave the repo clean enough for codegg to depend on this surface without compensating for avoidable edge cases.

## Scope

This closure pass covers five concrete cleanup items:

1. Remove the accidental root-level binary file `test_struct`.
2. Make `repo_fetch` use commit-SHA raw URLs for the actual fetch when `commit_sha` is provided.
3. Prefer stable raw permalinks in suggested fetches when available.
4. Distinguish partial profile availability from full profile degradation in telemetry.
5. Add final GitLab URL, locator, and provider-profile regression tests.

Do not add new provider types, package ecosystems, local indexing strategies, tree-sitter integration, recursive fetch, or broader API changes in this pass.

## Current state summary

The corrective pass successfully addressed the major design gaps:

- `RepoLocator` now has `RepoLocatorKind::Remote` and `RepoLocatorKind::Workspace`.
- Workspace fetch no longer serializes fake GitHub locators.
- Workspace fetch enforces `max_chars` through `clamp_lines_to_max_chars`.
- Workspace fetch scans local content for trust markers without mutating source-line semantics.
- `RepoFetchResponse` and `CodeEvidence` split `permalink_url` and `raw_permalink_url`.
- Local search scores content matches directly and reuses read content for scoring/snippets/symbols.
- `provider_status` now exposes `tool_capabilities`.

The remaining issues are comparatively small but worth fixing before closure.

## Cleanup item 1: Remove stray root-level binary `test_struct`

### Problem

A root-level file named `test_struct` was added during the corrective pass. It is binary, not UTF-8 text, and appears to be a compiled Mach-O-like artifact. It does not belong at the repository root.

### Required action

Delete `test_struct` from the repo.

### Follow-up hygiene

Add or tighten ignore rules so similar artifacts do not return:

- Check `.gitignore` for common Rust/build artifacts and ad hoc compiled binaries.
- Ensure `target/` is ignored.
- Consider adding a project-specific ignore entry for `test_struct` only if it is likely to be recreated by a local test command.
- Do not add broad ignores that could hide legitimate source fixtures.

### Tests / verification

- `git status` should not show `test_struct` after removal.
- `git ls-files test_struct` should return nothing.
- Full test suite should not depend on this file.

## Cleanup item 2: Fetch commit-SHA raw URL when `commit_sha` is provided

### Problem

`repo_fetch` currently builds both `raw_url` for the requested ref and `raw_permalink_url` for the commit SHA, but the actual fetch still uses the mutable `raw_url` unless a test override is provided. For exact evidence retrieval, a request that includes `commit_sha` should fetch from the commit-stable raw URL.

### Required behavior

When `RepoFetchRequest.commit_sha` is present:

- `raw_url` should continue to represent the requested ref URL.
- `raw_permalink_url` should represent the commit-SHA raw URL.
- The actual fetch URL should be `raw_permalink_url` unless `test_fetch_url` is set.
- `ref_resolved` should remain the requested ref name when known, but the response should make clear that content was fetched at the commit SHA.

Add an optional response field if needed:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub fetched_url: Option<String>
```

This is useful for agents and tests because `raw_url` and `raw_permalink_url` are both valid metadata fields, but neither necessarily tells which one was actually requested. If avoiding schema expansion, at least add a warning or test-only assertion; however, a `fetched_url` field is cleaner and will help codegg debugging.

### Implementation sketch

In `run_repo_fetch`:

```rust
let canonical_fetch_url = raw_permalink_url.as_deref().unwrap_or(&raw_url);
let fetch_url = args.test_fetch_url.as_deref().unwrap_or(canonical_fetch_url);
```

If adding `fetched_url`, populate it with the final selected URL before the network fetch.

### Tests

Add integration or unit tests that verify:

- GitHub `repo_fetch` with `commit_sha` uses `raw_permalink_url` as the actual fetch URL.
- GitHub `repo_fetch` without `commit_sha` uses `raw_url`.
- `test_fetch_url` still overrides the actual fetch URL for tests.
- The response includes both browser permalink and raw permalink when `commit_sha` is provided.

## Cleanup item 3: Prefer stable raw permalink in suggested fetches

### Problem

Suggested fetch generation currently chooses `raw_url` before `raw_permalink_url`. If both are present, codegg should prefer stable raw permalink evidence over mutable branch/tag evidence.

### Required behavior

Update suggested fetch URL priority for code evidence:

1. `raw_permalink_url`
2. `raw_url`
3. `permalink_url`
4. `browser_url`
5. card URL fallback

This ordering gives coding agents the most stable machine-fetchable source first, while retaining useful fallbacks.

### Implementation sketch

In `src/meta/suggested_fetches.rs`, change the selection from roughly:

```rust
ce.raw_url.or(ce.raw_permalink_url).or(ce.permalink_url)
```

to:

```rust
ce.raw_permalink_url
    .as_deref()
    .or(ce.raw_url.as_deref())
    .or(ce.permalink_url.as_deref())
    .or(ce.browser_url.as_deref())
```

### Tests

Add tests for URL priority:

- When both raw URL and raw permalink are present, suggested fetch uses raw permalink.
- When only raw URL is present, suggested fetch uses raw URL.
- When only browser permalink is present, suggested fetch uses permalink/browser fallback.
- Existing non-code cards still use `card.url`.

## Cleanup item 4: Split profile partial availability from full degradation

### Problem

The current telemetry treats `profile_partial` as `degraded = true`. That makes partial provider availability look the same as full fallback to defaults. For codegg, this distinction matters. A coding profile with `github_code` missing but `github_issues` and `github_releases` available is weaker, but not equivalent to fully degraded generic search.

### Required behavior

Telemetry should distinguish three states:

- Full profile applied: profile providers available; no fallback or skipped critical providers.
- Partial profile applied: some profile providers skipped, but at least one profile provider remains.
- Degraded profile: profile provider set was unusable and execution fell back to default providers.

Recommended model change:

```rust
pub struct ProviderSelectionTelemetry {
    pub profile_requested: Option<SearchProfile>,
    pub profile_applied: Option<SearchProfile>,
    pub degraded: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub partial: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped_providers: Vec<String>,
    pub reason: Option<String>,
}
```

If keeping the schema minimal, at least change `degraded` so it is false for partial profile availability and rely on warnings for the partial details. Prefer the explicit `partial` field because it is easier for codegg to consume.

### Behavior details

- `degraded = true` only when fallback to default providers occurred.
- `partial = true` when one or more profile providers were skipped but profile-specific providers remain.
- `reason` should be stable and concise:
  - `"using coding profile providers"`
  - `"coding profile skipped unavailable providers"`
  - `"coding profile fell back to default providers"`
- Warnings should continue to include provider-specific skipped reasons.

### Tests

Add tests covering:

- Coding profile with all profile providers unavailable: `degraded = true`, `partial = false`.
- Coding profile with some profile providers available: `degraded = false`, `partial = true`.
- Coding profile with all profile providers available: `degraded = false`, `partial = false`.
- Explicit provider request failure remains a validation error, not profile fallback.

## Cleanup item 5: Final GitLab URL and locator regression tests

### Problem

GitLab browser/raw URL generation uses the same general pattern as branch refs but with SHA substituted for permalink behavior. This is plausible, but GitLab namespaces and URL forms can be tricky. The closure pass should lock the intended semantics down with tests.

### Required tests

Add tests for:

- `gitlab_browser_url("group", "repo", "main", "src/lib.rs")` produces `https://gitlab.com/group/repo/-/blob/main/src/lib.rs`.
- `gitlab_raw_url("group", "repo", "main", "src/lib.rs")` produces `https://gitlab.com/group/repo/-/raw/main/src/lib.rs`.
- GitLab `repo_fetch` with commit SHA populates browser permalink and raw permalink using SHA as ref.
- Remote GitLab locator serializes as `kind = remote` with `host = gitlab`.
- Workspace locator serializes as `kind = workspace` with no remote host.

If nested GitLab groups are supported elsewhere as an owner string containing slashes, test that too:

- owner `group/subgroup`, repo `repo`, path `src/lib.rs`.

Do not broaden GitLab implementation in this pass unless tests reveal a clear bug.

## Documentation updates

Update README and AGENTS.md only if implementation behavior changes:

- If adding `fetched_url`, document it under `repo_fetch` response fields.
- Clarify that commit-SHA `repo_fetch` fetches from `raw_permalink_url` when available.
- Clarify profile telemetry fields: `degraded` versus `partial`.
- Confirm suggested fetches prefer stable raw permalinks when available.

Do not rewrite large sections. Keep doc changes surgical.

## Verification checklist

Run or ensure the implementer runs:

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Also run targeted tests if available:

```bash
cargo test repo_fetch
cargo test suggested_fetch
cargo test profile
cargo test local_workspace
cargo test gitlab
```

If CI is configured, verify the latest commit has passing checks. If no status checks are exposed, record local command output in the implementation summary.

## Acceptance criteria

The closure pass is complete when:

- `test_struct` is removed from the repo and does not reappear.
- `repo_fetch` with `commit_sha` fetches commit-stable raw content by default.
- Suggested fetches prefer `raw_permalink_url` over mutable `raw_url`.
- Provider selection telemetry distinguishes partial profile availability from full degradation.
- GitHub and GitLab permalink/raw URL semantics are covered by tests.
- Workspace locator tests prove no fake remote host is serialized.
- README/AGENTS accurately describe any changed fields.
- Formatting, clippy, and tests pass.

## Suggested implementation order

1. Delete `test_struct` and update `.gitignore` only if warranted.
2. Add or adjust `RepoFetchResponse.fetched_url` if chosen.
3. Change actual remote `repo_fetch` URL selection to prefer `raw_permalink_url` when commit SHA is present.
4. Update suggested-fetch URL priority.
5. Add partial profile telemetry field or adjust `degraded` semantics.
6. Add targeted tests for GitHub/GitLab URL semantics, workspace locator serialization, suggested fetch priority, and profile telemetry.
7. Apply minimal README/AGENTS updates.
8. Run full verification.

## Notes for implementer

Keep this pass small. Avoid opportunistic refactors. The current code is close; the highest-value outcome is stable semantics and clean repository hygiene.

For profile telemetry, do not make codegg infer partial state from English warning strings if a small structured field can be added. A `partial` boolean and `skipped_providers` list are preferable.

For commit-SHA fetches, stable evidence should win over branch freshness. The branch/tag raw URL remains useful metadata, but when the caller provides a commit SHA, they are asking for reproducibility.
