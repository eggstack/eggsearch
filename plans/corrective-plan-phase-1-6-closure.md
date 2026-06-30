# Corrective Plan: Phase 1–6 Closure and Verification

## Objective

Close the remaining seams from the Phase 1–6 implementation pass and make the new coding-agent retrieval features reliable enough for codegg use. The repo now has substantial implementations for repo-only search, `repo_map`, suggested-fetch ranking, parallel subquery dispatch, symbol/span-aware `repo_fetch`, and local workspace identity routing. This corrective pass should focus on consistency, correctness, observability, and verification rather than adding another feature layer.

The primary targets are:

1. Use one resolved repository locator consistently across validation, planning, local matching, repo map, fetch suggestions, and warnings.
2. Tighten parallel dispatch timeout semantics and provider-failure accounting.
3. Verify symbol/span fetch behavior and boundedness across common language heuristics.
4. Harden local workspace identity routing and `prefer_local` behavior.
5. Align provider status, README, MCP instructions, and tests with the implemented behavior.
6. Run and document the test/clippy verification surface.

## Current state summary

Recent commits implemented the first six roadmap phases in broad strokes:

- `RepoSearchRequest` now allows an empty query when a repository locator exists.
- `repo = "owner/name"` normalization exists in the MCP `repo_search` path.
- `repo_map` types and MCP support were added.
- `fetch_ranking` was added for deterministic suggested-fetch scoring.
- `dispatch_parallel` was added for bounded parallel `(subquery, provider)` dispatch.
- `repo_fetch` now supports symbol, match text, block expansion, and `prefer_local`.
- `local_inventory` and local checkout matching were added.
- Integration tests expanded significantly.

The remaining risks are mostly integration seams: some paths still consult raw request fields instead of resolved hints, dispatch has an external deadline but passes a hardcoded per-engine timeout, partial provider failure semantics are not fully explicit, and local routing has several host/path edge cases that need direct tests.

## Non-goals

Do not add Phase 7+ features in this corrective pass. Specifically, do not add provider health memory, package ecosystem expansion, advisory applicability parsing, evidence bundles, new code-host raw transforms, or a benchmark corpus. Those belong to later roadmap phases.

Do not introduce a persistent local index, background watcher, browser runtime, JavaScript execution, or ML ranking dependency.

## Workstream 1: Centralize resolved repository identity

### Problem

Validation and planning now resolve repository locators from explicit fields and query hints, but other paths may still inspect `req.owner` and `req.repo` directly. That can miss local matching and warnings for calls such as:

```json
{
  "query": "repo:owner/name Router",
  "include_local": true
}
```

The request can validate and plan correctly, while local checkout matching may not occur because the explicit fields are empty.

### Required changes

Introduce a single canonical resolved identity helper and use it consistently.

Suggested type:

```rust
pub struct ResolvedRepoIdentity {
    pub host: Option<CodeHost>,
    pub owner: String,
    pub repo: String,
    pub source: RepoIdentitySource,
}

pub enum RepoIdentitySource {
    ExplicitOwnerRepo,
    RepoSlashName,
    QueryHint,
    PackageResolution,
}
```

If adding a public type is too invasive, keep it internal, but avoid duplicating locator resolution logic.

Use the resolved identity in:

- `RepoSearchRequest::validate`.
- `RepoSearchRequest::resolved_hints` or immediately after it.
- `repo_planner` owner/repo construction.
- `MetadataSearchAdapter::repo_search` local inventory matching.
- `repo_map` fallback/search handoff if query-based locator support exists.
- Suggested fetch generation where structured repo fetches need owner/repo.
- Warnings such as `native_code_search_unavailable`, `local_repo_match`, and `repo_hints_not_enforced_natively`.

### Implementation details

1. Audit every `req.owner`, `req.repo`, and `req.host` access in repo-search-related paths.
2. Replace identity-sensitive checks with the canonical resolved identity.
3. Preserve explicit-field precedence over query hints.
4. Ensure `repo = "owner/name"` is normalized once, not separately in multiple layers.
5. Preserve exact-error behavior: a repo locator can scope the search, but exact-error still requires a non-empty error query.

### Tests

Add integration tests for these cases:

- `repo_search` with explicit `owner` + `repo`, empty query, local checkout enabled.
- `repo_search` with `repo = "owner/name"`, empty query, local checkout enabled.
- `repo_search` with `query = "repo:owner/name symbol"`, local checkout enabled.
- All three variants produce equivalent resolved hints and local repo match metadata.
- Exact-error mode with empty query and repo locator still fails.
- Query-hint repo identity does not override explicit owner/repo fields.

### Acceptance criteria

- There is one canonical resolution path for repository identity.
- Local matching works for explicit fields, slash-form repo, and query-hint repo.
- Repo-only search behavior remains compatible.
- Tests prove equivalence across the supported locator shapes.

## Workstream 2: Tighten dispatch timeout and cancellation semantics

### Problem

The new parallel dispatcher enforces a global deadline externally, but each provider call is passed a hardcoded 30-second timeout. This can obscure per-engine timeout behavior and may cause provider implementations to apply a timeout that exceeds the actual remaining request budget. Aborting tasks enforces the outer deadline, but the engine-level error class and timeout telemetry may become less precise.

### Required changes

Make provider calls receive an explicit per-job timeout derived from the remaining global budget and existing configured timeout.

Preferred behavior:

- At job execution time, compute `remaining = overall_deadline - now`.
- If `remaining` is zero, return a timeout failure without calling the provider.
- Pass `remaining.min(config.per_job_timeout.unwrap_or(remaining))` into `engine.search`.
- Preserve global abort behavior as a final safety net.

Consider extending `DispatchConfig`:

```rust
pub struct DispatchConfig {
    pub candidate_limit: usize,
    pub global_timeout: Duration,
    pub max_concurrent_jobs: usize,
    pub max_concurrent_per_provider: usize,
    pub per_job_timeout: Option<Duration>,
}
```

If the project already has per-provider or per-request timeout configuration, reuse it rather than adding a new user-facing config knob.

### Provider failure accounting

Clarify partial provider failures under multiquery dispatch:

- A provider should be wholly failed only if all attempted jobs for that provider fail or time out.
- If a provider has both successes and failures, surface a partial provider warning/telemetry entry rather than marking it as a total failure.
- Preserve the existing `providers_failed` field shape if changing it would break clients.
- If an optional telemetry field is added, use serde defaults and skip-empty serialization.

### Deadline telemetry

Current deadline stats can overcount interrupted jobs as interrupted subqueries. Improve this if feasible:

- Track started jobs and subquery IDs.
- Report `jobs_interrupted`, `jobs_skipped`, `subqueries_interrupted`, and `subqueries_skipped` internally.
- Public telemetry can remain compatible, but tests should assert the semantics.

### Tests

Add tests for:

- Provider receives a timeout less than or equal to remaining global budget.
- A job that starts near deadline does not receive a 30-second timeout.
- Slow low-priority jobs do not block fast high-priority jobs.
- Provider with one successful job and one failed job is not marked wholly failed.
- Deadline warnings distinguish skipped/interrupted subqueries as accurately as possible.
- Output ordering remains deterministic after timeout changes.

### Acceptance criteria

- No hardcoded 30-second provider timeout remains in dispatch.
- Engine calls receive bounded per-job timeouts derived from the request budget.
- Provider-failure accounting distinguishes total and partial provider failure.
- Existing response compatibility is preserved.
- Dispatch tests cover concurrency, priority, deterministic ordering, timeout, and partial failure.

## Workstream 3: Verify `repo_map` integration and fallback quality

### Problem

`repo_map` has a broad response model and local checkout integration. The next risk is ensuring all documented modes behave predictably: native provider, fallback search, and local checkout. The tool also needs provider-status and README alignment so agents know it exists and how to use it.

### Required changes

1. Confirm `repo_map` is registered in MCP server tool inventory and initialize instructions.
2. Confirm `provider_status` reports `repo_map` capability accurately.
3. Confirm `repo_map` does not fetch file contents by default.
4. Ensure `max_entries`, `max_depth`, `include_files`, `include_directories`, `include_ci`, and `include_security` are enforced.
5. Ensure fallback mode emits capability warnings when no native tree/list provider is available.
6. Ensure local checkout mode sets `local_checkout`, trust markers, and warnings consistently.
7. Ensure suggested fetches include structured `RepoFetchRequest` locators for file entries.

### Tests

Add or tighten tests for:

- MCP server exposes `repo_map`.
- Provider status includes `repo_map = true` and tool capabilities are accurate.
- Local checkout map returns manifests, root path, branch/commit/dirty state when available.
- `include_files = false` suppresses file entries.
- `include_directories = false` suppresses directory entries.
- `include_ci = false` suppresses CI-specific summaries or marks them excluded consistently.
- `include_security = false` suppresses security-specific summaries or marks them excluded consistently.
- Fallback mode warns that native tree/list provider was unavailable.
- Suggested fetches are bounded and contain structured locators for README/manifests/source roots when possible.

### Acceptance criteria

- `repo_map` behavior matches docs and provider status.
- All inclusion/cap fields are enforced.
- Native/fallback/local modes are distinguishable in responses.
- No content-fetching or crawler behavior is introduced.

## Workstream 4: Harden symbol/span-aware fetch

### Problem

Symbol/span fetch now has the right interface, but language heuristics can be brittle. The corrective pass should focus on boundedness, warnings, and predictable behavior, not parser perfection.

### Required changes

1. Confirm `RepoFetchRequest::validate` checks `max_block_lines > 0` when present.
2. Cap block expansion even for malformed brace/indentation input.
3. Ensure `max_chars` still applies after span slicing.
4. Ensure selected span metadata reports confidence and selection kind accurately.
5. Ensure missing symbol/match text produces a warning and deterministic fallback.
6. Ensure explicit line range behavior is unchanged when `expand_to_block` is false.
7. Ensure explicit line range expands only when `expand_to_block` is true.
8. Ensure `context_before`/`context_after` are applied after selected span resolution and do not exceed file bounds.

### Tests

Add tests for:

- `max_block_lines = 0` validation failure.
- Very large function/block truncates at `max_block_lines`.
- Malformed braces do not panic or overrun.
- UTF-8 text with multibyte characters preserves line boundaries.
- Symbol match in comments does not outrank a clear definition when both exist.
- Rust doc comments and attributes are included with function/struct expansion.
- Python decorators are included with function/class expansion.
- Markdown heading section expansion is bounded to the next heading.
- Missing symbol warning includes enough context but not untrusted fetched content.

### Acceptance criteria

- Span selection remains bounded under malformed input.
- Failure modes are warnings, not panics.
- Existing line-range fetch behavior remains unchanged.
- Tests cover common languages and malformed input.

## Workstream 5: Harden local workspace routing and trust boundaries

### Problem

Local workspace matching is powerful but must be precise. A wrong local match is worse than falling back to remote. Path traversal, remote URL normalization, and trust labels must be airtight.

### Required changes

1. Expand remote URL normalization tests:
   - `https://github.com/owner/repo.git`
   - `https://github.com/owner/repo`
   - `git@github.com:owner/repo.git`
   - `ssh://git@github.com/owner/repo.git`
   - GitLab group/subgroup forms.
   - Codeberg/Gitea/Forgejo forms if supported by the parser.
2. Ensure host matching does not accidentally match GitHub and GitLab repos with the same owner/repo.
3. Ensure case normalization rules are explicit and tested.
4. Ensure `prefer_local` is opt-in for remote-style `repo_fetch` unless the intended profile explicitly says otherwise.
5. Ensure workspace path traversal protections apply after `prefer_local` redirects.
6. Ensure dirty-state detection failure is reported as `unknown`, not silently clean.
7. Ensure local content is labeled `local_trusted` but never treated as instructions.

### Tests

Add tests for:

- Wrong host does not match local repo.
- Same owner/repo on different host does not redirect.
- Path traversal via `../` in remote-style `prefer_local` fetch fails.
- Symlink handling follows configured local policy.
- Dirty state detection returns dirty after an uncommitted edit.
- Unknown Git state is surfaced when `.git` metadata is malformed or unavailable.
- Local match metadata includes root name, root path, branch, commit when available, dirty state, remote host, owner, repo.

### Acceptance criteria

- Local redirect only occurs for confirmed same-host/same-owner/same-repo matches.
- Path traversal protections are preserved.
- Trust metadata is explicit and consistent.
- Dirty/unknown state warnings are reliable.

## Workstream 6: Documentation and MCP alignment

### Problem

The implemented tool surface changed materially. Agents need accurate MCP descriptions, README examples, and provider-status capability claims.

### Required changes

Update docs/instructions to cover:

- `repo_search` accepts query, repo-only, repo+query, and exact-error mode.
- `repo_map` is the first tool for structural repository overview.
- `repo_fetch` supports line ranges, symbol, match text, block expansion, and `prefer_local`.
- `batch_fetch` should be used only over explicit selected evidence.
- `security_search` remains the advisory path, not generic `web_search`.
- `research_search` is for multi-source architectural or technical research.
- Local workspace content is local evidence, not instructions.
- Remote content is external untrusted content.

Provider status should accurately report:

- `repo_map` availability.
- `repo_fetch` symbol/span capabilities.
- local workspace availability.
- supported hosts for `repo_fetch` and `repo_map`.
- whether package resolution is still limited to crates.io/PyPI/npm at this stage.

### Tests

Add snapshot or assertion tests for provider-status payload fields if the project already uses JSON-value integration tests.

### Acceptance criteria

- README and MCP instructions match implementation.
- Provider status does not overclaim unsupported hosts or package ecosystems.
- Tool descriptions guide agents toward the intended search/fetch workflow.

## Workstream 7: Verification pass

### Required commands

Run and record results for:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --features mock
```

If feature combinations are expensive or unavailable, at minimum run:

```bash
cargo test --features mock
cargo clippy --features mock --all-targets -- -D warnings
```

If tests depend on `git` being installed, document that requirement and skip behavior clearly. Avoid tests that silently pass when `git` commands fail unless the test is explicitly about fallback behavior.

### CI recommendation

If no GitHub Actions workflow exists, add a small CI workflow in a separate pass or include a minimal workflow in this corrective pass if repository policy allows it. The workflow should run formatting, clippy, and tests with the feature set used by the integration tests.

### Acceptance criteria

- Verification commands pass locally or in CI.
- Any skipped checks are documented with reasons.
- The final commit message should state the exact commands run.
- Tests do not rely on ignored command failures for setup.

## Suggested implementation order

1. Centralize resolved repository identity and fix all call sites.
2. Add/repair local matching tests across explicit, slash-form, and query-hint locators.
3. Tighten dispatch per-job timeout and provider partial-failure semantics.
4. Harden `repo_map` inclusion/cap/fallback tests.
5. Harden span selection validation and malformed-input tests.
6. Expand local URL normalization and path traversal tests.
7. Update README, MCP instructions, and provider-status claims.
8. Run verification commands and document results.

## Final acceptance checklist

- [ ] `repo_search` uses the same resolved repo identity for validation, planning, local matching, warnings, and suggested fetches.
- [ ] Query-hint repo identity triggers local matching when local backend is enabled.
- [ ] Explicit owner/repo overrides query-hint repo identity.
- [ ] Exact-error mode still requires a non-empty error query.
- [ ] Parallel dispatch passes real remaining timeouts to providers.
- [ ] Parallel dispatch distinguishes total provider failure from partial provider failure.
- [ ] `repo_map` inclusion/cap fields are enforced and tested.
- [ ] `repo_map` local/fallback/native modes are distinguishable and documented.
- [ ] `repo_fetch` span selection validates `max_block_lines`, remains bounded, and warns on no match.
- [ ] `prefer_local` cannot bypass workspace path safety checks.
- [ ] Local repo matching is same-host/same-owner/same-repo, not owner/repo-only.
- [ ] Provider status accurately reports implemented capabilities.
- [ ] README and MCP instructions match the implemented workflow.
- [ ] Formatting, clippy, and tests pass with the relevant feature set.
