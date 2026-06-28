# Coding-Agent Search Corrective Hardening Plan

## Purpose

This plan closes the correctness and hardening gaps found after the coding-agent search roadmap implementation. The repo now has the intended major surfaces: exact code evidence metadata, `repo_fetch`, search profiles and telemetry, package/version-aware repo search, local workspace search, and workspace fetch. The follow-up work should not add another large feature tranche. It should make the new surfaces semantically correct, bounded, and reliable enough for codegg to consume without special-case compensations.

The central theme is boundary integrity. `repo_fetch` must distinguish remote repository files from local workspace files without lying in its locator model. Local content must obey context-budget limits and trust-marker policy. Search profiles must degrade based on actually available engines rather than nominal config. Package and permalink metadata should avoid ambiguous names. Tests should lock down these behaviors.

## Scope

This corrective pass covers six areas:

1. Workspace locator semantics and host typing.
2. Workspace fetch budget enforcement and local trust/sanitization handling.
3. Profile provider degradation against actual adapter availability.
4. Permalink/raw URL semantics for `repo_fetch` and `CodeEvidence`.
5. Local search scoring and symbol-enrichment honesty.
6. Documentation and regression tests for codegg-facing behavior.

Do not introduce tree-sitter, persistent indexes, background file watchers, recursive fetch/crawling, or additional package ecosystems in this pass. Those should remain future enhancements.

## Current issues to correct

### 1. Workspace fetch uses a fake GitHub host

`repo_fetch` currently special-cases `host = "workspace"`, but the `RepoLocator` type still requires `CodeHost`. The workspace path therefore fills `RepoLocator.host` with `CodeHost::Github` as a placeholder. This is semantically wrong and will confuse agents, logs, telemetry, and any future codegg branch that uses the locator to decide whether content is remote or local.

Required fix: represent workspace as a real locator kind, not as GitHub.

### 2. Workspace fetch does not enforce `max_chars`

The workspace fetch path computes `truncated` by comparing sliced text length to `max_chars`, but it does not actually clamp returned text or `lines`. A caller can receive over-budget content despite the response saying it was truncated. This violates the main safety and context-budget contract of fetch tools.

Required fix: enforce `max_chars` before serialization and ensure `text`, `lines`, `returned_line_end`, and `truncated` are internally consistent.

### 3. Workspace fetch does not apply trust-marker scanning/framing

Local workspace content is operator-configured provenance, but not instruction-trusted. Comments, README text, generated files, vendored files, and test fixtures can contain prompt-injection-like strings. The current workspace fetch path returns default `TrustMarkers` without marker scanning. This creates inconsistent semantics compared with `web_fetch` and external `repo_fetch`.

Required fix: run the same relevant sanitization and marker-scan policy for local fetch output, with local-specific warnings and trust labels.

### 4. Profile provider resolution can select engines that were not built

The profile resolver decides availability from config maps and API config keys. The adapter is the true source of which engines were actually built. A profile can select `github_code` because it appears in `[search].api`, even if the env var was missing and the adapter skipped it. The tool then sees the provider as unknown and returns a validation error instead of degrading.

Required fix: profile resolution should degrade against actual adapter availability for ordinary profile requests. Explicit `providers` should remain strict.

### 5. `permalink_url` semantics are ambiguous

For GitHub, `github_permalink_url` currently returns a raw.githubusercontent.com URL at a commit SHA. That is stable, but the name `permalink_url` usually implies a browser-viewable `github.com/{owner}/{repo}/blob/{sha}/{path}` URL. The raw stable URL is useful but should be named distinctly.

Required fix: either make `permalink_url` browser-viewable and add `raw_permalink_url`, or rename existing raw SHA URL fields consistently. Prefer explicit separate fields.

### 6. Local search is too path-biased for content search

The local search backend scores path/name/language and then finds snippets. A file containing the query text but not matching path tokens may be missed. For a coding agent, local text search needs to be a primary signal, especially for exact error messages, symbols, config keys, and function names that appear inside files but not in the path.

Required fix: include bounded content match scoring as part of file scoring without blowing up scan cost.

## Detailed implementation plan

### Step 1: Introduce a proper repo fetch locator kind

Replace the assumption that every `RepoLocator` has a `CodeHost` with a host enum that can represent remote code hosts and local workspace locators.

Recommended model:

```rust
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RepoFetchLocator {
    Remote {
        host: CodeHost,
        owner: String,
        repo: String,
        ref_name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        commit_sha: Option<String>,
        path: String,
    },
    Workspace {
        root: String,
        path: String,
    },
}
```

If tagged enums are too disruptive for existing callers, use a flatter compatible model:

```rust
pub struct RepoLocator {
    pub kind: RepoLocatorKind, // remote | workspace
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<CodeHost>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ref_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_sha: Option<String>,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<String>,
}
```

Prefer the second form if minimizing response-shape churn is important. The critical requirement is that workspace locators must not serialize as GitHub.

Update `run_repo_fetch` and `run_workspace_fetch` so workspace fetch returns a workspace locator with no fake `CodeHost`. Update suggested fetches from local workspace search to emit the same locator shape.

Tests:

- Workspace `repo_fetch` response serializes `locator.kind = "workspace"` or equivalent.
- Workspace locator does not contain `host = "github"`.
- Remote GitHub/GitLab locators remain backward-compatible enough for existing tests.
- `provider_status` or docs describe the workspace locator shape.

### Step 2: Enforce `max_chars` in workspace fetch

Implement a shared helper for line-range output clamping. The helper should take line-numbered slices and a max character budget, then return internally consistent `text`, `lines`, `returned_line_start`, `returned_line_end`, and `truncated`.

Expected semantics:

- Apply requested line range and context first.
- Build line-numbered candidate output.
- Enforce `max_chars` against the actual returned textual content.
- If clamping cuts through a line, either omit that partial line or return the partial line with a clear warning. Prefer omitting partial lines to keep line semantics clean, unless the existing fetch behavior normally truncates mid-string.
- If no full line fits, return a bounded prefix of the first line with a warning like `max_chars_cut_first_line`.
- `text` must never exceed `max_chars` when `max_chars` is supplied.
- `lines` must correspond to `text`; do not return lines that are not represented in `text`.
- Add warning `workspace_fetch_truncated_by_max_chars` when clamping occurs.

Consider moving this helper into `src/core/repo_fetch.rs` so both remote and workspace `repo_fetch` can use it. Remote `repo_fetch` currently relies on `web_fetch` for content-level truncation, but line slicing after fetch can still produce inconsistent behavior if improved later.

Tests:

- Workspace fetch with `max_chars` smaller than selected line output returns text length <= max.
- `truncated = true` when clamped.
- Returned lines correspond to returned text.
- Line-range clamping and char-budget clamping warnings can coexist.
- Remote repo fetch still respects existing fetch caps.

### Step 3: Apply local fetch trust-marker scanning and warnings

Local workspace content should be `local_trusted` for provenance but still scanned as data. Add a small local sanitization path that reuses existing primitives from `sanitize`:

- Always strip or reject control characters consistently with the external fetch path.
- When configured sanitization is enabled, scan for prompt-injection markers in local returned text.
- Add `TrustMarkers` counts to workspace fetch responses.
- Add warning such as `local_content_marker_warning: possible prompt injection markers detected in local workspace content` when marker hits occur.
- Do not wrap local source code in `<<<EXTERNAL_UNTRUSTED>>>` if that would corrupt source-line semantics. Instead, preserve source text and report markers out-of-band. If framing is desired, only frame metadata fields or add a separate `framed_text` field later.

Apply the same marker-scan logic to local search snippets if those snippets are included in `SourceCard` text. Local result cards should retain `TrustLevel::LocalTrusted`, but their `trust_markers` should reflect detected marker-like text in snippets.

Tests:

- Workspace fetch of a file containing common injection phrases increments `trust_markers.injection_hits`.
- Workspace fetch warning includes the local marker warning.
- Source line text is not corrupted by framing.
- Local search cards with marker-like snippets carry trust markers.

### Step 4: Resolve profile providers against actual adapter engines

Change ordinary profile resolution so it considers the adapter's actual `provider_ids()` or an equivalent available-provider set. Explicit `providers` should remain strict and error on unknown/disabled providers because that is operator-directed behavior. Profile requests should degrade because they are intent-level hints.

Recommended flow in `run_repo_search`:

1. If `args.providers` is non-empty, use existing strict `resolve_providers` and `adapter.select_engines` validation.
2. If `args.providers` is empty and `profile` is supplied, ask config for candidate profile providers, but filter them through `adapter.provider_ids()`.
3. Add warnings for profile providers skipped because not built, not configured, disabled, or unsupported.
4. If no profile providers remain, fall back to default providers filtered through actual adapter availability.
5. If defaults also fail, return a clear internal/config error.

This can be implemented as a new helper on `ServerState` or a free function in `mcp/tools.rs`, because it needs both config and adapter knowledge. Keep the current config-level resolver for CLI/config validation if useful, but do not rely on it alone for profile execution.

Expected warnings:

- `profile_provider_not_built: github_code is in coding profile but no engine was constructed`.
- `profile_provider_unavailable: brave_api is configured but unavailable`.
- `profile_degraded: coding profile fell back to default providers`.
- `profile_partial: coding profile skipped unavailable providers`.

Telemetry should report:

- `profile_requested`.
- `profile_applied`.
- `degraded` true only when fallback occurred, not when merely skipping one optional provider while retaining native coding providers.
- `reason` with a stable concise explanation.

Tests:

- Coding profile with no GitHub API key degrades to generic providers with warnings, not validation failure.
- Coding profile with only `github_code` unavailable and `github_issues` available remains partial, not fully degraded.
- Explicit `providers = ["github_code"]` still errors if no such engine exists.
- Telemetry reflects partial versus degraded behavior.

### Step 5: Clarify raw URL and permalink fields

Update `CodeEvidence` and `RepoFetchResponse` URL fields so names match semantics.

Recommended schema:

- `browser_url`: human-viewable URL for the requested ref.
- `raw_url`: raw content URL for the requested ref.
- `permalink_url`: human-viewable commit permalink when commit SHA is known.
- `raw_permalink_url`: raw content URL at the commit SHA when commit SHA is known.

Update helpers:

- `github_browser_url(owner, repo, ref, path)` -> `https://github.com/.../blob/{ref}/{path}`.
- `github_raw_url(owner, repo, ref, path)` -> `https://raw.githubusercontent.com/.../{ref}/{path}`.
- `github_permalink_url(owner, repo, sha, path)` -> `https://github.com/.../blob/{sha}/{path}`.
- `github_raw_permalink_url(owner, repo, sha, path)` -> `https://raw.githubusercontent.com/.../{sha}/{path}`.

Do the same for GitLab where deterministic. If GitLab raw permalink construction is uncertain, only populate `permalink_url` and leave `raw_permalink_url` absent, or keep raw URL construction with tests.

Backward compatibility:

- Keep `permalink_url` field but change it to the browser URL meaning.
- Add `raw_permalink_url` as optional.
- Update README examples and tests.

Tests:

- GitHub permalink helper returns `github.com/.../blob/{sha}/...`.
- GitHub raw permalink helper returns `raw.githubusercontent.com/.../{sha}/...`.
- `repo_fetch` with commit SHA populates both fields.
- `CodeEvidence` from GitHub code-host URL uses the new semantics.

### Step 6: Make local search score content matches directly

The local backend should not require a path/name hit before content search can matter. Add bounded content scoring during file scanning:

- For files within size cap and not binary, search content for the full query string and query tokens.
- Exact full-query content match should add a substantial score.
- Token content matches should add smaller score, capped to avoid huge files dominating.
- Exact symbol match should remain high priority.
- Preserve path/name boosts, but do not make them prerequisites.

Suggested scoring:

- Exact filename match: +100.
- Path segment/token match: +20 per token, capped.
- Exact full-query content match: +50.
- Content token match: +5 per token, capped at +30.
- Symbol definition match: +80 or existing score +30, whichever is clearer.
- Source role boost as currently implemented.
- Generated/minified/lock penalties remain.

To avoid rereading files repeatedly, refactor local scanning so content is read at most once per file when content scoring or snippet extraction is needed. This can be a modest internal helper:

```rust
struct LocalFileScan {
    text: Option<String>,
    content_score: f64,
    snippet: Option<String>,
    line_start: Option<u32>,
    line_end: Option<u32>,
    matched_symbol: Option<String>,
    symbol_kind: Option<SymbolKind>,
}
```

Tests:

- A file containing the query text but with an unrelated filename is returned.
- A path-only match still works.
- Symbol match outranks generic content match.
- Large files over cap are not read for content scoring.
- Binary files are not read.

### Step 7: Improve provider/tool capability reporting for codegg

Extend `provider_status` with a compact `tool_capabilities` object so codegg does not need to infer support from README text.

Suggested payload additions:

```json
"tool_capabilities": {
  "repo_fetch": {
    "remote_hosts": ["github", "gitlab"],
    "workspace": true,
    "line_ranges": true,
    "context_lines": true,
    "max_chars_enforced": true
  },
  "repo_search": {
    "profiles": ["generic", "coding", "security", "research"],
    "package_resolution": ["crates_io", "pypi", "npm"],
    "local_workspace": true,
    "subquery_telemetry": true
  },
  "local_workspace": {
    "enabled": true,
    "symbol_enrichment": "regex_heuristic"
  }
}
```

Do not expose absolute local root paths by default. If root visibility is useful, expose only count or sanitized aliases.

Tests:

- `provider_status` includes `tool_capabilities`.
- Local disabled reports `local_workspace.enabled = false`.
- Local enabled reports `local_workspace.enabled = true` without leaking full paths.

### Step 8: Documentation updates

Update README and AGENTS.md with corrected semantics:

- `repo_fetch` locators distinguish `remote` and `workspace`.
- Workspace fetch is local-only, root-bounded, line-range capable, and max-char bounded.
- `local_trusted` means operator-configured provenance, not instruction trust.
- Local marker scanning is out-of-band and does not mutate source lines.
- `permalink_url` is browser-viewable; `raw_permalink_url` is raw content.
- Profiles are best-effort and degrade based on actual available providers.
- Symbol enrichment is regex-heuristic, not full semantic/LSP precision.

Update examples for:

- Remote GitHub `repo_fetch` with commit SHA.
- Workspace `repo_fetch` using the corrected locator shape.
- `repo_search` with `profile = "coding"` when native providers are unavailable.

## Required test matrix

Add or update tests in `tests/integration.rs` and unit tests near the relevant modules.

### Locator tests

- Remote GitHub locator serializes as remote.
- Remote GitLab locator serializes as remote.
- Workspace locator serializes as workspace.
- Workspace locator does not serialize fake GitHub host.

### Budget tests

- Workspace fetch enforces `max_chars` on `text`.
- Workspace fetch enforces `max_chars` on `lines`/text consistency.
- Workspace fetch still honors line range and context.
- Remote fetch behavior is unchanged.

### Trust tests

- Workspace fetch scans marker-like content.
- Workspace fetch returns `trust = local_trusted` and nonzero trust markers when appropriate.
- Local search snippets carry trust markers when marker-like snippets are returned.

### Profile tests

- Coding profile degrades when native code providers are not built.
- Partial profile availability does not count as full degradation.
- Explicit unavailable provider remains an error.
- Telemetry and warnings are stable.

### URL semantics tests

- Browser permalink and raw permalink are distinct.
- `CodeEvidence` uses correct URL fields.
- `RepoSuggestedFetch.structured_repo_fetch` remains valid after locator changes.

### Local scoring tests

- Content-only matches are returned.
- Symbol matches outrank content-only matches.
- Large/binary files are skipped.

## Acceptance criteria

This pass is complete when:

- Workspace fetch no longer uses `CodeHost::Github` or any other fake remote host in locators.
- Workspace fetch cannot return `text` over requested `max_chars`.
- Workspace fetch and local snippets report marker hits consistently without corrupting source text.
- `profile = "coding"` degrades gracefully when native providers are absent from the actual adapter engine list.
- Explicit provider requests remain strict.
- `permalink_url` and `raw_permalink_url` have unambiguous meanings and tests.
- Local search finds content-only matches, not only path/name matches.
- `provider_status` exposes enough tool capability metadata for codegg to decide how to use eggsearch.
- README and AGENTS.md match the implemented behavior.
- The full test suite and clippy pass.

## Suggested implementation order

1. Fix locator model and update workspace fetch serialization.
2. Add shared line/text budget enforcement helper and apply it to workspace fetch.
3. Add local trust-marker scanning and warnings.
4. Fix profile provider resolution using actual adapter availability.
5. Split browser permalink and raw permalink fields.
6. Refactor local content scoring and symbol/content scan reuse.
7. Extend provider/tool capability reporting.
8. Update docs.
9. Run full tests and clippy, then add any missing regression tests from the matrix.

## Notes for handoff implementer

Be conservative with response-shape churn. If a breaking locator enum would be too disruptive, use optional fields and a `kind` discriminator to preserve most of the current shape. The important invariant is that local workspace locators must be machine-identifiable as local and must not masquerade as a remote host.

Keep local source text line-preserving. Do not inject framing delimiters into code lines unless a separate field is introduced; source mutation makes line numbers and copied code unreliable. Prefer trust markers and warnings out-of-band.

When in doubt, make profile behavior forgiving and explicit provider behavior strict. Profiles are agent intent hints; explicit provider lists are operator/debug directives.
