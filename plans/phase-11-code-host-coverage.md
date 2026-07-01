# Phase 11 Plan: Code-Host Coverage Completion

## Objective

Complete code-host source fetching and repository evidence coverage for Codeberg, Gitea, and Forgejo so `repo_search`, `repo_map`, `repo_fetch`, `web_fetch` transforms, and `batch_fetch` behave consistently across supported code hosts.

This phase should preserve eggsearch’s provider-neutral tool surface. Do not add separate `github_*`, `gitlab_*`, `gitea_*`, or `codeberg_*` MCP tools. Add host support behind existing abstractions and expose capabilities through provider status and warnings.

## Current baseline

The repo already has strong GitHub/GitLab behavior and some code-host concepts:

- `CodeHost` includes multiple host variants in some paths.
- `repo_search` accepts host hints such as GitHub, GitLab, and Codeberg.
- `repo_fetch` currently supports GitHub/GitLab remote raw URL construction.
- `web_fetch` has explicit fetch and transform metadata.
- `batch_fetch` supports explicit URL and structured repo locators.
- `repo_map` supports repository structure and suggested fetches.

The gap is consistent raw-source transform and structured repo fetch coverage across Codeberg/Gitea/Forgejo.

## Non-goals

Do not implement a generic browser or JavaScript runtime. Do not clone repositories. Do not bypass SSRF validation or redirect safety. Do not add unbounded recursive tree traversal. Do not require every self-hosted Gitea/Forgejo instance to work without explicit host configuration.

## Target hosts

Implement support in this order:

1. Codeberg.
2. Gitea generic host support.
3. Forgejo generic host support.

Codeberg should be treated as a known public Forgejo/Gitea-style host with stable defaults. Generic Gitea/Forgejo should require configured base URL or provider descriptor to avoid arbitrary host SSRF expansion.

## Workstream 1: Host model and configuration

### Required changes

Ensure `CodeHost` and provider descriptors can represent:

- GitHub.
- GitLab.
- Codeberg.
- Gitea.
- Forgejo.
- Local/workspace where applicable.

For generic Gitea/Forgejo, add configuration fields if not already present:

```toml
[providers.gitea]
enabled = true
base_url = "https://git.example.com"
api_url = "https://git.example.com/api/v1"

[providers.forgejo]
enabled = true
base_url = "https://forge.example.com"
api_url = "https://forge.example.com/api/v1"
```

Avoid accepting arbitrary runtime hostnames for raw transforms unless they are explicitly configured or known-safe public hosts.

### Tests

- Parse host aliases: `codeberg`, `gitea`, `forgejo`.
- Configured host base URL normalization.
- Reject unsupported host in `repo_fetch` with clear error.
- Provider status reports configured host capabilities.

## Workstream 2: Raw/browser URL transforms

### URL patterns

Implement browser and raw URL builders for known hosts.

#### Codeberg/Gitea/Forgejo browser pattern

Common browser source path:

```text
https://host/owner/repo/src/branch/<ref>/<path>
https://host/owner/repo/src/commit/<sha>/<path>
```

Raw path often:

```text
https://host/owner/repo/raw/branch/<ref>/<path>
https://host/owner/repo/raw/commit/<sha>/<path>
```

Confirm against provider docs or existing host behavior before implementation. If variants exist, implement conservative host-specific builders and tests.

### Transform metadata

When `web_fetch` transforms a browser URL into raw content URL, include metadata:

```json
"fetch_transform": {
  "kind": "code_host_browser_to_raw",
  "host": "codeberg",
  "original_url": "...",
  "transformed_url": "...",
  "stable": false,
  "reason": "source_browser_url"
}
```

For commit SHA URLs, set stable where appropriate.

### Safety

Run transformed URLs through the same URL validation, SSRF guard, redirect limits, content caps, and timeout policies as normal fetches.

### Tests

- Codeberg branch browser URL transforms to raw URL.
- Codeberg commit browser URL transforms to raw URL and is marked stable.
- Gitea configured host transforms branch and commit URLs.
- Forgejo configured host transforms branch and commit URLs.
- Malformed URLs do not transform.
- Unconfigured arbitrary Gitea-like host does not transform silently.
- Raw transform preserves path encoding safely.

## Workstream 3: `repo_fetch` support

### Required changes

Extend `RepoFetchRequest` host validation and URL building for Codeberg/Gitea/Forgejo.

For Codeberg:

- Owner and repo are required.
- Ref defaults should match existing repo_fetch behavior, but prefer default branch metadata if `repo_map` can resolve it.
- Build browser URL and raw URL using Codeberg patterns.

For generic Gitea/Forgejo:

- Require configured base URL/provider ID or host descriptor.
- Build browser/raw URLs from configured base.
- Include host identity in `RepoLocator`.

### Commit SHA behavior

If `commit_sha` is provided:

- Prefer raw commit permalink.
- Browser permalink should use commit URL form.
- `stable = true` or equivalent metadata should propagate into suggested fetch ranking.

### Tests

- `repo_fetch` Codeberg raw/browser URL construction.
- `repo_fetch` Gitea configured host raw/browser URL construction.
- `repo_fetch` Forgejo configured host raw/browser URL construction.
- Commit SHA raw permalink preferred.
- Unsupported/unconfigured host returns validation error.
- Path traversal protections still apply.

## Workstream 4: `repo_map` support

### Native APIs

Gitea and Forgejo expose repository contents/tree APIs similar in spirit to GitHub/GitLab but with different paths.

Implement bounded native map support where feasible:

- Repository metadata/default branch.
- Root tree/list entries.
- Directory listing up to configured depth/cap.
- File size/type when available.

For Codeberg, use the Forgejo/Gitea API endpoint if public and stable.

### Fallback

If native API is unavailable:

- Return fallback search mode.
- Emit warning: `native_repo_map_unavailable`.
- Still generate deterministic browser URLs and suggested fetches when enough owner/repo/ref/path information is available.

### Tests

Use mocked HTTP fixtures.

- Codeberg native map fixture.
- Gitea native map fixture.
- Forgejo native map fixture.
- Fallback mode warning when API disabled/unavailable.
- Entry classifiers work equally across hosts.
- Suggested fetches include structured repo locators for supported hosts.

## Workstream 5: `repo_search` providers

### Capability model

If native code/issue/release providers exist for Gitea/Forgejo/Codeberg, ensure provider descriptors accurately report:

- code search support.
- issue search support.
- release search support.
- result timestamps.
- path/language enforcement ability.

If native providers do not exist yet, ensure warnings clearly state that host hints are approximated through generic search.

### Tests

- Host hint with no native provider emits `repo_hints_not_enforced_natively` or host-specific equivalent.
- Native configured provider reports enforcement.
- Provider status code-host summary includes Codeberg/Gitea/Forgejo where configured.

## Workstream 6: Batch fetch integration

`batch_fetch` should accept structured `repo_fetch` locators for Codeberg/Gitea/Forgejo once `repo_fetch` supports those hosts.

Tests:

- Batch fetch Codeberg locator.
- Batch fetch configured Gitea locator.
- Per-item trust labels and transform metadata preserved.
- Unsupported host item fails independently without failing the whole batch when `continue_on_error` is true.

## Workstream 7: Documentation

Update README and AGENTS docs:

- Supported code hosts by tool.
- Which hosts have native map/search support.
- Which hosts have raw fetch support.
- Configuration examples for generic Gitea/Forgejo.
- Safety model for configured hosts and SSRF prevention.
- Example `repo_fetch` requests for Codeberg and configured Gitea/Forgejo.

Update `provider_status` examples so agents do not assume unsupported host capabilities.

## Compatibility requirements

- Existing GitHub/GitLab behavior must not regress.
- Existing `repo_fetch` request shapes must keep working.
- Unknown/unconfigured hosts must fail clearly rather than transform arbitrary URLs.
- New host support must be capability-advertised.
- Raw transforms must preserve existing fetch safety checks.

## Acceptance criteria

- Codeberg `repo_fetch` works with browser/raw/permalink URLs.
- Configured Gitea/Forgejo `repo_fetch` works with browser/raw/permalink URLs.
- `web_fetch` can transform supported code-host browser URLs to raw URLs with transform metadata.
- `repo_map` supports native or clearly warned fallback mode for Codeberg/Gitea/Forgejo.
- `batch_fetch` supports structured locators for the newly supported hosts.
- Provider status accurately reports host capabilities.
- Tests cover URL builders, transforms, validation, fallback warnings, and batch integration.
- GitHub/GitLab tests continue passing.
