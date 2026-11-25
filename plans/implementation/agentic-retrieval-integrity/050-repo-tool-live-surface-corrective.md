# M050 — Repository Tool Live-Surface Corrective

Status: active
Date: 2026-11-25
Subsystem: agentic retrieval integrity
Milestone class: corrective (corrective to M029 and the repo_map host contract)
Baseline: `a7bdab0` on `docs/user-facing-polish-20261123`
Decision authority: `docs/tool-matrix.md` § Legacy call compatibility (error taxonomy); the existing `repo_fetch` omitted-host default in `src/mcp/tools/repo_fetch.rs`

References: `src/mcp/tools/repo_fetch.rs`; `src/mcp/tools/repo_map.rs`;
`src/mcp/tools/common.rs` § ToolError; `src/core/warning.rs`;
`docs/tool-matrix.md`; `architecture/mcp.md`.

## Objective

A live-condition sweep of the whole MCP surface found two defects in the
repository tools and, while fixing the second, a third latent defect in the
same finalizer. Close all three without adding a tool, an alias, an argument,
or an error code, and without changing the nine-tool advertised inventory.

1. `repo_fetch` maps every fetch-layer failure to `ToolError::Internal`, so a
   missing file or a timeout reaches the caller as JSON-RPC `-32603` instead
   of a repairable tool error. `docs/tool-matrix.md:109` reserves
   `internal_error` for server faults; the canonical `fetch` repo item
   already reports `ok: false` with an `upstream_failed` class, and
   `web_fetch` already returns a repairable `upstream_failed` payload.
2. `repo_map` leaves an omitted `host` as `CodeHost::Unknown`, performs no
   native lookup, and returns an empty `fallback_search` map. `repo_fetch`
   already defaults an omitted host to GitHub (`src/mcp/tools/repo_fetch.rs:204`),
   and native GitHub trees work keyless.
3. `run_repo_map_with_host_scope` ends with
   `response.structured_warnings = convert_warnings(&response.warnings)`, which
   discards the specific forge-failure warning pushed by the error branch. A
   rate limit, an authentication failure, or a missing ref therefore degrades
   to the generic `no_native_tree_provider` notice.

This is corrective work, so it is a new plan rather than an amendment to M029.

## Contract

- `repo_fetch` reports an upstream fetch failure as `upstream_failed` with the
  original message (`http_status: HTTP error: 404 HTTP 404`), never
  `internal_error`. Validation and policy classes are unchanged.
- `repo_map` treats an omitted `host` as GitHub when live dispatch is allowed,
  matching `repo_fetch`. Under `Mode::Off` with an enabled local backend the
  tool infers nothing and performs no remote lookup, preserving the air-gapped
  local-only path.
- When the host was inferred and the forge lookup fails, the warning says that
  GitHub was tried and names the accepted alternatives. When the lookup fails
  for an explicit host, the specific code is retained.
- The advertised input schema changes only by stating the documented default on
  the two `host` fields.
- No tool count, alias, argument, response field, or stable error code changes.

## Ordered work packages

1. **Error class.** `repo_fetch` maps the fetch failure to
   `ToolError::upstream_failed`, keeping the message format and the
   `prefer_local_skipped` suffix.
2. **Host default.** `repo_map` applies GitHub when `host` is omitted and
   `live_allowed` is `Allow`; the off-mode local-only path keeps `None`.
3. **Warning merge.** The finalizer appends the converted search warnings to the
   accumulated structured warnings instead of replacing them, so rate-limit,
   auth, and ref-not-found codes survive.
4. **Regression tests.** One case for the alias error class, one for the
   inferred host success path, and one for the inferred host failure guidance.
5. **Docs and baseline.** `docs/tool-matrix.md`, `architecture/mcp.md`,
   `docs/codegg-integration.md`, `CHANGELOG.md`, and the re-measured
   `tests/tool_surface_live.rs` baseline (+63 definition bytes, +16 estimated
   tokens).

## Verification

Focused: `tests/repo_workflow.rs` (the three new cases), the existing
`repo_map` suite, and `tests/tool_surface_live.rs`.

Broad: `RUSTUP_TOOLCHAIN=1.89.0 make check`, `make docs-check`,
`make packaging-check`, and `python3 packaging/check-planning-consistency.py`.

Live confirmation: rebuild with the `documents` feature, call `repo_fetch`
with a missing path, and call `repo_map` with and without `host` over stdio.

## Acceptance and stop conditions

- A `repo_fetch` 404 returns `isError: true` with `code: upstream_failed` and
  the status in the message.
- `repo_map` with `host` omitted and a GitHub mock returns `host: github` and
  `mode: native` with entries.
- `repo_map` with `host` omitted and a failing GitHub mock returns
  `host: github`, `mode: fallback_search`, and a warning that names the
  inference.
- Off-mode local-only behavior is unchanged.
- **Stop:** if inferring GitHub would require a remote lookup on the off-mode
  local path, keep `None` there; the air-gap invariant outranks the default.

## Non-goals

- Probing GitLab or Codeberg when `host` is omitted. GitHub is the documented
  default; callers pass `host` for other forges.
- Changing advertisement presets, aliases, or the tool inventory.
- Provider-level live fragility (Startpage timeouts, Mojeek challenges, NVD
  rate limits, probe-budget timeouts); those are recorded in the live-sweep
  report, not in this milestone.

## Closure evidence

The three regression tests, the merge behavior in the failure case, the
re-measured schema baseline, and the pinned gates under Rust 1.89.
