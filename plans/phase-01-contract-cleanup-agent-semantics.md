# Phase 1 Plan: Contract Cleanup and Agent-Facing Semantics

## Objective

Bring eggsearch’s documented behavior, MCP tool schemas, and implementation behavior into alignment for coding-agent use. The most important correction is `repo_search`: documentation describes repo-only discovery, but the current implementation rejects an empty query. This phase should allow repository-only discovery when a valid repo locator is provided, then update MCP instructions so agents choose the right tool for each retrieval workflow.

This phase is intentionally small and compatibility-focused. It should not add a repository tree API, symbol extraction, ranking overhaul, or new provider integrations. Those belong to later phases.

## Current problem

`repo_search` documentation and examples imply that a caller may provide only a repository locator, such as `repo = "tokio-rs/axum"`, and omit a free-text query. The implementation currently models `query` as a required `String` and validation rejects `query.trim().is_empty()` before considering whether owner/repo information is present. This creates a failure mode for exactly the first call a coding agent wants to make: “inspect this repo.”

The MCP tool descriptions are also too brief to reliably teach agents the intended routing policy. Agents need explicit guidance:

- Use `repo_search` for repository/API/codebase discovery.
- Use `repo_search` with `mode = "exact_error"` for compiler/runtime/toolchain errors.
- Use `repo_fetch` for known repository file paths or line ranges.
- Use `batch_fetch` only for explicit selected URLs/locators.
- Use `security_search` for CVE/GHSA/OSV/RustSec/package advisory questions.
- Use `research_search` for architectural or multi-source technical questions.
- Use `web_search` only as generic fallback or non-repository discovery.

## Scope

In scope:

- Make repo-only `repo_search` calls valid when a repository locator is provided.
- Parse `repo = "owner/name"` into owner and repo when needed.
- Generate default structural discovery subqueries for repo-only calls.
- Preserve current query-based `repo_search` behavior.
- Tighten request validation and error messages.
- Update README examples and MCP initialize instructions.
- Add tests for repo-only calls, invalid empty calls, and compatibility with existing calls.

Out of scope:

- New `repo_map` tool.
- Provider scheduler changes.
- Symbol/span fetch changes.
- Security/package ecosystem expansion.
- Persistent index or background tasks.

## Proposed behavior

A `repo_search` request is valid when either condition is true:

1. `query.trim()` is non-empty.
2. A repository locator can be resolved from explicit fields or parsed hints.

A repository locator can be resolved from:

- explicit `owner` + `repo` fields;
- explicit `repo` containing `owner/name` if the current MCP args shape preserves that pattern;
- `repo:owner/name`, `repository:owner/name`, or equivalent hints in query text;
- package resolution in later code paths only when package metadata is already available, but this should not be required for Phase 1 validation.

If neither a query nor a repo locator is present, return a validation error such as:

`repo_search requires a non-empty query or a repository locator such as owner+repo or repo:owner/name`

For repo-only discovery, the planner should generate bounded default subqueries. A reasonable initial set:

- `owner/repo README`
- `owner/repo docs documentation api reference`
- `owner/repo examples usage sample`
- `owner/repo source src lib main`
- `owner/repo tests test suite`
- `owner/repo releases changelog migration`
- `owner/repo security policy advisory`

These should still pass through the existing grouping and suggested-fetch machinery. Avoid making this a clone/tree walk in Phase 1.

## Implementation notes

### Request parsing and validation

Likely files:

- `src/core/repo_search.rs`
- `src/core/repo_query.rs`
- `src/meta/repo_planner.rs`
- `src/mcp/tools.rs`
- `src/mcp/server.rs`
- `README.md`

Add a helper on `RepoSearchRequest` such as:

```rust
pub fn has_resolvable_repo_locator(&self) -> bool
```

or:

```rust
pub fn resolved_repo_locator(&self) -> Option<(String, String)>
```

The helper should use `resolved_hints()` so explicit fields override parsed query hints. If `self.repo` contains `owner/name` and `self.owner` is absent, normalize that into owner/repo. If this normalization belongs in `RepoQueryHints`, add tests there.

Update `validate()` so empty query is only rejected when no locator exists. Keep exact-error mode stricter: exact-error mode should still require a non-empty error query because repo-only exact-error search has no error phrase to preserve.

### Planner changes

In `build_repo_search_plan_with_package`, handle empty residual query with owner/repo present. Current source/docs builders often return `Some` if owner_repo is present, but verify all intended default subqueries are generated. Add a small repo-only path if necessary so tests do not rely on accidental behavior.

If a repo-only call would currently generate too few groups, add explicit helper functions for repo overview subqueries. Keep the cap at the existing limit unless tests show an important structural query is truncated.

### MCP args normalization

`RepoSearchArgs` currently has both `owner` and `repo`. If `repo` is `owner/name`, normalize before constructing `RepoSearchRequest`. Do not break users who pass `owner = "tokio-rs"`, `repo = "axum"`.

If an invalid repo string is supplied, provide a targeted validation error rather than letting the planner silently treat it as a bare repo name.

### Instructions and docs

Update `EGGSEARCH_INSTRUCTIONS` to include a compact routing policy. Keep it short enough for MCP initialize but operational enough for agents.

Update README sections for `repo_search` so the minimal call, rules, and examples all match implementation. Ensure the docs distinguish:

- query-only repo search;
- repo-only discovery;
- repo + query search;
- exact-error search.

## Tests

Add or update unit/integration tests covering:

- `RepoSearchRequest { query: "", owner: Some("tokio-rs"), repo: Some("axum") }` validates.
- `RepoSearchRequest { query: "", repo: Some("tokio-rs/axum") }` validates if this request shape is supported.
- `RepoSearchRequest { query: "repo:tokio-rs/axum" }` validates and resolves hints.
- `RepoSearchRequest { query: "" }` fails with the new specific validation error.
- `mode = ExactError` with empty query fails even with repo locator.
- Repo-only planner emits docs/source/examples/releases-oriented subqueries.
- Existing non-empty query tests still pass.
- MCP `run_repo_search` accepts repo-only args and serializes a normal response with grouped results when using mock engines.

## Acceptance criteria

- README and MCP instructions no longer contradict implementation.
- Repo-only `repo_search` calls are accepted when a valid repo locator is present.
- Empty query with no locator still fails deterministically.
- Exact-error mode still requires a non-empty error query.
- Existing query-based `repo_search` behavior and response fields remain backward-compatible.
- Tests cover validation, hint resolution, planner behavior, and MCP integration.
- `cargo test` passes.

## Handoff notes

Keep this phase focused. Do not start implementing `repo_map` here. The goal is to unblock agents from the first repo-inspection call and make the documented contract reliable. Any deeper structural repository map should be built in Phase 2 using the corrected semantics from this phase.
