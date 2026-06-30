# Phase 2 Plan: Repository Map and Structural Discovery

## Objective

Add a bounded repository-structure discovery capability for coding agents. The goal is to let codegg quickly answer “what is this repository shaped like, and where should I look first?” without cloning the repository, crawling links, or relying entirely on generic web search snippets.

This phase should introduce a `repo_map` or `repo_overview` MCP tool and corresponding core types. It should prefer native code-host APIs where configured and degrade honestly to search-based discovery when native tree/list access is unavailable.

## Rationale

`repo_search` is useful for query-driven evidence discovery, but coding agents often need an initial structural map before they know the right query. A manager agent inspecting a new repo needs entrypoints, manifests, source roots, examples, tests, docs, CI, releases, and security policy. Today, the agent has to infer this through repeated `repo_search` and `web_fetch` calls.

A bounded repository map gives the agent a deterministic starting context and improves later search/fetch ranking.

## Scope

In scope:

- Add a new request/response model for repository map retrieval.
- Expose a new MCP tool, likely `repo_map`.
- Implement GitHub native repository metadata/tree support first.
- Add provider-neutral abstractions so GitLab/Gitea/Forgejo can be added later.
- Add fallback behavior when no native tree provider is available.
- Detect important files and directories using deterministic classifiers.
- Return suggested next actions/fetches based on the map.
- Document the tool and its relationship to `repo_search` and `repo_fetch`.

Out of scope:

- Full clone or recursive repository indexing.
- Persistent background index.
- Semantic code analysis or symbol extraction beyond simple file classification.
- Security advisory applicability reasoning.
- Local workspace identity matching; that is Phase 6.

## Proposed tool shape

### Request

A starting shape:

```rust
pub struct RepoMapRequest {
    pub host: Option<CodeHost>,
    pub owner: String,
    pub repo: String,
    pub ref_name: Option<String>,
    pub commit_sha: Option<String>,
    pub max_entries: Option<usize>,
    pub max_depth: Option<usize>,
    pub include_files: Option<bool>,
    pub include_directories: Option<bool>,
    pub include_ci: Option<bool>,
    pub include_security: Option<bool>,
    pub timeout_ms: Option<u64>,
    pub providers: Vec<String>,
}
```

Keep `owner`, `repo`, and `host` explicit. Do not accept arbitrary URLs as the primary interface. If URL parsing is desired, add it as a convenience helper but normalize to structured fields.

### Response

Suggested response fields:

```rust
pub struct RepoMapResponse {
    pub host: CodeHost,
    pub owner: String,
    pub repo: String,
    pub ref_name: Option<String>,
    pub commit_sha: Option<String>,
    pub default_branch: Option<String>,
    pub mode: String,
    pub root_entries: Vec<RepoMapEntry>,
    pub important_files: Vec<RepoImportantFile>,
    pub important_directories: Vec<RepoImportantDirectory>,
    pub manifests: Vec<RepoManifestSummary>,
    pub source_roots: Vec<RepoPathSummary>,
    pub docs: Vec<RepoPathSummary>,
    pub examples: Vec<RepoPathSummary>,
    pub tests: Vec<RepoPathSummary>,
    pub ci: Vec<RepoPathSummary>,
    pub security: Vec<RepoPathSummary>,
    pub suggested_fetches: Vec<RepoSuggestedFetch>,
    pub providers_queried: Vec<String>,
    pub providers_failed: Vec<ProviderFailure>,
    pub warnings: Vec<SearchWarning>,
    pub trust_markers: TrustMarkers,
}
```

The response should not include file contents by default. It should return metadata and explicit fetch suggestions. Any content reading should remain `repo_fetch`, `web_fetch`, or `batch_fetch`.

## Important-file classifier

Add deterministic classification for common files and directories.

Root files:

- `README`, `README.md`, `README.rst`, `README.adoc`
- `Cargo.toml`, `Cargo.lock`
- `package.json`, lockfiles, workspace files
- `pyproject.toml`, `setup.py`, `requirements.txt`, `poetry.lock`
- `go.mod`, `go.sum`
- `pom.xml`, `build.gradle`, `settings.gradle`
- `.csproj`, `.sln`, `Directory.Build.props`
- `Gemfile`, `Gemfile.lock`
- `composer.json`, `composer.lock`
- `Dockerfile`, `docker-compose.yml`
- `CHANGELOG`, `RELEASES`, `MIGRATION`, `UPGRADE`
- `SECURITY.md`, `CODEOWNERS`, `CONTRIBUTING`, `LICENSE`

Directories:

- `src`, `lib`, `crates`, `packages`, `apps`, `cmd`, `internal`, `pkg`
- `examples`, `example`, `samples`, `demo`
- `tests`, `test`, `spec`, `benches`, `benchmarks`
- `docs`, `doc`, `website`, `book`
- `.github/workflows`, `.gitlab-ci.yml`, `.forgejo/workflows`, `.gitea/workflows`

Each classification should include reasons. Example: `source_root` because path is `src/`; `rust_manifest` because filename is `Cargo.toml`.

## Native provider implementation

### GitHub first

Add a native GitHub map provider if `github_code` or a dedicated GitHub API provider is configured. Prefer GitHub REST endpoints that return repository metadata and tree/list data. Keep this inside the existing provider abstraction; do not expose a GitHub-specific tool.

Implementation should:

- Retrieve repository metadata to find default branch when `ref_name`/`commit_sha` is absent.
- Retrieve bounded tree entries for the selected ref.
- Respect `max_entries` and `max_depth`.
- Avoid recursively walking the entire repo by default.
- Return a warning when truncation occurs.
- Preserve provider failure accounting.

### Fallback mode

If no native tree provider is available, use `repo_search`-style fallback queries to discover README, docs, manifests, examples, tests, releases, and security policy. The response mode should indicate fallback, for example `repo_map_fallback_search`, and warnings should state that no native tree/list provider enforced structure.

Fallback results should be useful but clearly lower confidence than native tree output.

## Suggested fetches

`repo_map` should return prioritized fetch suggestions:

1. README or primary docs.
2. Primary manifest(s).
3. Main source entrypoint(s), if detected.
4. Examples or quickstart files.
5. Changelog/migration files.
6. Security policy, if relevant.
7. Test entrypoints, if relevant.

For repository files, include structured `RepoFetchRequest` objects so agents can call `repo_fetch` directly. Avoid fetching contents automatically.

## Affected modules

Likely additions/changes:

- `src/core/repo_map.rs` for request/response types.
- `src/core/mod.rs` re-exports.
- `src/meta/repo_map.rs` or `src/meta/repo_mapper.rs` for planning/classification.
- `src/meta/engines/*` for native provider support.
- `src/mcp/tools.rs` for args and runner.
- `src/mcp/server.rs` for MCP tool exposure and instructions.
- `README.md` for docs.
- `tests/integration.rs` or focused integration files for mock-provider tests.

## Tests

Add tests for:

- Important-file classification.
- Directory classification.
- `max_entries` and `max_depth` truncation.
- Default-branch/ref resolution behavior with mock provider.
- Fallback mode when no native tree provider is available.
- Suggested fetch priority order.
- Serialization stability of `RepoMapResponse`.
- MCP tool registration and minimal call.
- Validation errors for missing owner/repo, invalid host, zero caps, and conflicting ref/commit semantics if applicable.

## Acceptance criteria

- A `repo_map` or `repo_overview` MCP tool is available.
- The tool returns structure metadata only; it does not fetch file contents by default.
- GitHub native map support works when configured.
- Fallback mode works and emits honest capability warnings.
- Important files/directories are classified with deterministic reasons.
- Suggested fetches include structured `repo_fetch` locators when possible.
- Existing tools remain backward-compatible.
- README documents when to use `repo_map` versus `repo_search`.
- `cargo test` passes.

## Handoff notes

Keep the first version bounded and deterministic. Do not attempt to fully index all files in large repositories. The first useful version can inspect shallow tree entries plus known important paths. Later phases can use `repo_map` output to improve suggested-fetch ranking and local workspace matching.
