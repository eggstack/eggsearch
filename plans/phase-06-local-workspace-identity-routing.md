# Phase 6 Plan: Local Workspace Identity and Trust-Aware Routing

## Objective

Make local workspace search repository-aware. When a user asks eggsearch about a remote repository that is already checked out under a configured local root, codegg should prefer the local checkout and understand its state: remote identity, branch, commit, dirty status, package/workspace layout, and trust boundary.

This phase builds on the existing local workspace search and `repo_fetch host = "workspace"` support. It should not introduce a heavyweight persistent index or background daemon.

## Rationale

For coding agents, local files are usually more relevant than remote default-branch files. The local checkout may be on a feature branch, contain uncommitted changes, or differ from upstream. If eggsearch treats local workspace evidence as just another search source without matching it to requested repo identity, agents may fetch stale remote source when they should read local trusted files.

The correct behavior is:

- If the requested repo maps to a local checkout, local evidence should be preferred.
- The response should state that local evidence corresponds to a specific remote URL/owner/repo.
- The response should include worktree state so the agent knows whether it is reading dirty local code.
- Remote evidence should remain available as fallback/comparison when useful.

## Scope

In scope:

- Discover Git repositories under configured local roots.
- Normalize remote URLs to host/owner/repo identity.
- Match incoming `repo_search`, `repo_map`, and `repo_fetch` locators to local checkouts.
- Prefer local trusted results when a match exists.
- Add local repository state metadata: root, branch, commit, dirty/clean/unknown.
- Add package/workspace manifest detection for local checkouts.
- Improve `workspace://` URLs and structured workspace fetch locators.
- Add tests using temporary Git repositories.

Out of scope:

- Full persistent local index.
- Background watching or incremental indexing.
- Deep semantic symbol index.
- Mutating local files.
- Running build/test commands.

## Local repository inventory

Add a lightweight inventory component, likely under `src/meta/local_backend.rs` or a new module `src/meta/local_inventory.rs`.

For each configured root:

- Walk bounded directories respecting existing local config:
  - `include_hidden`
  - `respect_gitignore`
  - `follow_symlinks`
  - file/entry caps
- Detect Git worktrees by `.git` directory or gitfile.
- Read repository metadata with either direct file parsing or `git` command invocation if the project already accepts shelling out. Prefer direct parsing if feasible.

Suggested metadata:

```rust
pub struct LocalRepoIdentity {
    pub root_name: String,
    pub root_path: PathBuf,
    pub worktree_path: PathBuf,
    pub remotes: Vec<LocalGitRemote>,
    pub matched_host: Option<CodeHost>,
    pub matched_owner: Option<String>,
    pub matched_repo: Option<String>,
    pub current_branch: Option<String>,
    pub current_commit: Option<String>,
    pub dirty_state: LocalDirtyState,
    pub manifests: Vec<LocalManifestSummary>,
}
```

`LocalDirtyState` can be `Clean`, `Dirty`, `Unknown`, and maybe `NotGit`.

## Remote URL normalization

Support common forms:

- `https://github.com/owner/repo.git`
- `https://github.com/owner/repo`
- `git@github.com:owner/repo.git`
- `ssh://git@github.com/owner/repo.git`
- GitLab equivalents.
- Codeberg/Gitea/Forgejo where host mapping is known.

Normalize to:

```rust
pub struct NormalizedRepoId {
    pub host: CodeHost,
    pub host_domain: Option<String>,
    pub owner: String,
    pub repo: String,
}
```

Strip `.git`, normalize case where appropriate, and preserve original remote URL for diagnostics.

## Search routing behavior

### `repo_search`

When local backend is enabled and `include_local` is true:

1. Resolve incoming repo locator from explicit fields/query hints.
2. Look for matching local repo identity.
3. If found, run local search against that worktree first.
4. Add local results with `trust = local_trusted` and metadata indicating `local_repo_match`.
5. Keep remote search unless the request or config explicitly disables it.
6. Boost local results when the local repo matches the target repo.

Add warnings/telemetry:

- `local_repo_match: using local checkout for owner/repo`
- `local_repo_dirty: local checkout has uncommitted changes`
- `local_repo_state_unknown` when state cannot be determined

### `repo_fetch`

If caller passes `host = "workspace"`, preserve current behavior.

Additionally, allow a normal remote-style `repo_fetch` request to resolve to local when config says local preference is enabled and a matching checkout exists. This should be controlled by a request/config flag to avoid surprising callers who explicitly want remote default branch content.

Potential request field:

```rust
pub prefer_local: Option<bool>
```

Default can be true for codegg-facing profile behavior, but be careful with compatibility. If in doubt, default false for `repo_fetch` and true only when the caller uses a local-aware profile.

### `repo_map`

If Phase 2 is already implemented, `repo_map` should map local checkout structure first when a match exists. It should include local branch/commit/dirty state in the response.

## Trust metadata

Local workspace content should remain distinct from remote content.

- Remote web/code-host results: `external_untrusted`.
- Local workspace source files: `local_trusted`, but still not instructions.
- Dirty local files: `local_trusted` with `dirty_state = Dirty`.

Do not collapse local and remote evidence into one indistinguishable result. Agents should be able to tell where each result came from.

## Package/workspace detection

Within each matched local repo, detect manifests:

- Rust: `Cargo.toml`, workspaces, crates.
- Node: `package.json`, workspaces, lockfiles.
- Python: `pyproject.toml`, `setup.py`, requirements/poetry.
- Go: `go.mod`.
- Maven/Gradle: `pom.xml`, `build.gradle`, `settings.gradle`.
- .NET: `.sln`, `.csproj`.

This should be metadata only. Do not solve dependencies in this phase.

## Affected modules

Likely files:

- `src/core/local.rs`
- `src/meta/local_backend.rs`
- new `src/meta/local_inventory.rs`
- `src/core/repo_search.rs`
- `src/core/repo_fetch.rs`
- `src/meta/adapter.rs`
- `src/mcp/tools.rs`
- `README.md`
- tests using `tempfile`

## Implementation steps

1. Add normalized remote URL parser with tests.
2. Add local Git repo discovery within configured roots.
3. Add worktree state detection.
4. Add manifest detection.
5. Add local repo identity matching helper.
6. Integrate matching into `repo_search` local backend path.
7. Add local metadata to local `SourceCard` metadata, using serde-compatible optional fields.
8. Optionally add `prefer_local` to `repo_fetch` and implement local resolution.
9. Update README local workspace section.

## Tests

Add tests for:

- Remote URL normalization for HTTPS, SSH scp-like, and SSH URL forms.
- `.git` and gitfile worktree detection.
- Branch and commit detection.
- Dirty state detection using a temporary Git repo, if invoking git is acceptable in tests.
- Manifest detection across a small synthetic workspace.
- Matching `repo:owner/name` to local checkout.
- Local results receive boost and `local_trusted` trust label.
- Dirty checkout warning appears.
- No local match leaves remote search behavior unchanged.
- Workspace fetch preserves path traversal protections.

## Acceptance criteria

- eggsearch can identify configured local Git checkouts and normalize their remote identities.
- `repo_search` prefers matching local workspace evidence when enabled/requested.
- Responses expose local repo match and worktree state metadata.
- Remote and local evidence retain distinct trust labels.
- No background indexing or file mutation is introduced.
- Existing local search behavior remains compatible.
- `cargo test` passes.

## Handoff notes

This phase should be careful with trust semantics. `local_trusted` means the file came from configured local storage, not that its contents are safe to follow as instructions. Keep warnings and metadata precise. Do not execute arbitrary project code or shell commands beyond tightly bounded Git metadata inspection, and prefer direct file parsing where practical.
