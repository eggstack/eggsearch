# Phase 9: Expanded Host-Native Providers

## Purpose

Expand eggsearch's code-host-native retrieval beyond the current GitHub-focused path so codegg can search and fetch code across GitLab, Forgejo/Gitea, Sourcegraph-like indexes, and self-hosted enterprise instances without falling back to generic web search for everything.

The goal is better code search quality and source metadata, not broad crawling. Native providers should return structured source cards with host/owner/repo/ref/path/line evidence and suggested `repo_fetch` locators where possible.

## Non-goals

Do not implement repository cloning, local mirrors, authenticated enterprise admin APIs, write operations, or provider-specific behavior that cannot be tested with mocks. Do not add every possible forge in one pass. Keep this phase modular.

## Provider targets

Prioritize in this order:

1. GitLab code/project/issues/releases provider.
2. Gitea/Forgejo provider for code and issues where APIs are compatible.
3. Sourcegraph provider for code search if a configured endpoint exists.
4. Generic self-hosted code-host adapter model.

GitHub should remain supported and should not regress.

## Configuration model

Add a host-native provider section that can represent multiple instances:

```toml
[search.code_hosts.gitlab_com]
kind = "gitlab"
base_url = "https://gitlab.com"
enabled = true
token_env = "GITLAB_TOKEN"
capabilities = ["code_search", "issue_search", "release_search"]

[search.code_hosts.company_gitlab]
kind = "gitlab"
base_url = "https://gitlab.example.com"
enabled = false
token_env = "COMPANY_GITLAB_TOKEN"
capabilities = ["code_search", "issue_search"]

[search.code_hosts.forgejo_local]
kind = "forgejo"
base_url = "https://git.example.com"
enabled = false
token_env = "FORGEJO_TOKEN"
capabilities = ["code_search", "issue_search"]
```

Keep legacy provider IDs working. Expose derived provider IDs like:

- `gitlab_code`
- `gitlab_issues`
- `gitlab_releases`
- `forgejo_code`
- `sourcegraph_code`

For multi-instance setups, use stable IDs such as `gitlab_com_code` or configured instance names.

## Core traits

Introduce or refine a host-native abstraction:

```rust
pub trait CodeHostProvider {
    fn id(&self) -> &str;
    fn host_kind(&self) -> CodeHostKind;
    fn capabilities(&self) -> CodeHostCapabilities;
    async fn search_code(&self, request: CodeSearchRequest) -> Result<Vec<SourceCard>, SearchError>;
    async fn search_issues(&self, request: IssueSearchRequest) -> Result<Vec<SourceCard>, SearchError>;
    async fn search_releases(&self, request: ReleaseSearchRequest) -> Result<Vec<SourceCard>, SearchError>;
}
```

If the current provider/engine trait is already sufficient, do not introduce a parallel hierarchy. Instead, add a small host-native helper module that generates structured source cards consistently.

## GitLab implementation

Implement GitLab APIs behind a bounded HTTP client:

- Project search when owner/repo not supplied.
- Repository file search where GitLab API supports search scope `blobs`.
- Issues search.
- Merge request search if useful and low-cost.
- Releases/tags where API supports it.

Important details:

- Support namespaces with subgroups, e.g. `group/subgroup/repo`.
- URL-encode project paths correctly for API calls.
- Preserve browser and raw URL construction.
- Attach `CodeEvidence` with `host = Gitlab`, owner namespace, repo, path, ref when known, language, source role, match line if available.
- Generate `structured_repo_fetch` for fetchable file results.

## Gitea/Forgejo implementation

Add a shared adapter for Gitea-compatible APIs:

- Repository search.
- Code search if enabled by the server and API supports it.
- Issues search.
- Releases/tags.

Because self-hosted Gitea/Forgejo deployments vary, provider descriptors must expose precise capabilities and warnings when code search is unavailable.

## Sourcegraph implementation

Add a configurable Sourcegraph endpoint provider:

```toml
[search.code_hosts.sourcegraph]
kind = "sourcegraph"
base_url = "https://sourcegraph.example.com"
enabled = false
token_env = "SOURCEGRAPH_TOKEN"
```

Capabilities:

- Code search with repo/path/language/symbol hints.
- Result metadata for repo, file, line ranges.
- Suggested `repo_fetch` only if the underlying host can be mapped to GitHub/GitLab/Gitea or if Sourcegraph raw URLs are fetchable and stable.

Do not hardcode public Sourcegraph assumptions if the API is private/self-hosted.

## Provider status

Extend `provider_status` with per-instance capability details:

```json
"code_hosts": [
  {
    "id": "gitlab_com",
    "kind": "gitlab",
    "base_url": "https://gitlab.com",
    "enabled": true,
    "configured": true,
    "capabilities": {
      "code_search": true,
      "issue_search": true,
      "release_search": true,
      "repo_fetch": true
    }
  }
]
```

Do not expose token values. If base URLs are sensitive in some deployments, allow config to hide them from status or return host kind plus ID only.

## Query planning integration

Update repo planner/provider selection so:

- `profile = coding` prefers host-native code providers when repo/org/host hints are present.
- `host:gitlab` routes to GitLab providers first.
- `host:gitea` or configured instance name routes to matching provider.
- Generic providers remain fallback when native providers are unavailable.
- Warnings clearly state when native provider is unavailable and generic fallback is used.

## Fetch integration

Ensure `repo_fetch` supports the host types whose URL construction is deterministic. GitLab is already partly supported; verify nested namespaces and commit-SHA raw permalink behavior.

For Gitea/Forgejo, add support only if URL patterns are reliable and testable:

- browser URL: `/src/branch/{ref}/{path}` or equivalent depending on implementation
- raw URL: `/raw/branch/{ref}/{path}` or API endpoint

If not reliable, return source cards with browser/raw URLs but omit structured `repo_fetch` and add warning.

## Tests

Use mocked HTTP endpoints for every provider. Add tests for:

- GitLab project path URL encoding.
- GitLab code result -> `SourceCard` with `CodeEvidence`.
- GitLab issue result -> issue metadata.
- GitLab release result -> release metadata.
- Nested GitLab namespace handling.
- GitLab commit-SHA raw permalink construction.
- Gitea/Forgejo provider descriptor when code search unavailable.
- Sourcegraph code result parsing.
- Provider status exposes capability flags.
- Planner prefers GitLab for `host:gitlab`.
- Profile fallback warning when requested host-native provider is unavailable.

No tests should require live tokens or public network.

## Documentation

Update README and AGENTS.md:

- Configuration examples for GitLab, Forgejo/Gitea, and Sourcegraph.
- Capability matrix by provider kind.
- Explanation of self-hosted instance IDs.
- Note that native providers improve code evidence and suggested fetches.
- Note fallback behavior to generic search.

## Acceptance criteria

Phase 9 is complete when:

- GitLab native search is implemented and tested for code/issues/releases.
- Gitea/Forgejo or Sourcegraph support is added with honest capability flags; if one target is deferred, document it explicitly in the plan handoff summary.
- Source cards from native providers carry structured code evidence.
- `repo_search` planner routes host hints to native providers.
- `provider_status` exposes code-host instance capabilities.
- `repo_fetch` behavior for GitLab nested namespaces is tested.
- Docs include config examples and fallback behavior.
- `cargo fmt`, clippy, and tests pass.
