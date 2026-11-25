# Agentic Retrieval Integrity M050 — Repository Tool Live-Surface Corrective — Status

Status: closed
Date: 2026-11-25
Milestone: M050 — Repository Tool Live-Surface Corrective
Milestone class: corrective
Candidate: `79ea653a124916de696637adda8ed95798153715`
Baseline: `a7bdab0`
Decision authority: `docs/tool-matrix.md` § Legacy call compatibility; the existing `repo_fetch` omitted-host default

## Candidate and changes

A live sweep of the twelve callable MCP names (nine advertised tools plus
`web_fetch`, `batch_fetch`, and `repo_fetch`) over real stdio MCP found two
repository-tool defects and one latent finalizer defect. The candidate is the
baseline plus this change set.

- `src/mcp/tools/repo_fetch.rs:655`: the fetch-failure arm maps to
  `ToolError::upstream_failed`; the message format and the
  `prefer_local_skipped` suffix are unchanged.
- `src/mcp/tools/repo_map.rs:129`: an omitted `host` resolves to
  `CodeHost::Github` when `live_allowed` is `Allow`, the same default as
  `repo_fetch.rs:204`. Mode Off keeps `None`, so the local-only path performs
  no remote lookup.
- `src/mcp/tools/repo_map.rs:339`: the finalizer now appends converted search
  warnings to the accumulated structured warnings instead of replacing them, so
  `forge_rate_limited`, `forge_auth_required`, and `repo_ref_not_found`
  survive.
- `src/mcp/tools/repo_map.rs:212`: the inferred-host failure message names
  GitHub and the accepted alternatives.
- `tests/repo_workflow.rs`: three regression cases.
- `tests/tool_surface_live.rs` and `tests/mcp_tool_contract.rs`: the
  re-measured baseline (+30 definition bytes, +7 estimated tokens; 33,850
  advertised bytes) for the new host-default text.
- `docs/tool-matrix.md`, `docs/codegg-integration.md`, `architecture/mcp.md`,
  and `CHANGELOG.md`: the host default and the alias error-class contract.
- `docs/test-inventory.md`: 6,626 all-features / 6,561 mock tests.

## Why the original verification missed it

- M029 verified that `repo_fetch` remained callable and that a repo item
  behaves like the alias; neither side proved the error class of an upstream
  failure, and the alias is unadvertised, so no advertisement test covers it.
- `repo_map` tests passed an explicit `host` or accepted `fallback_search`
  as a pass condition, so the omitted-host no-op satisfied the assertion.
- No test asserted that a specific forge-failure warning reaches
  `structured_warnings`; the finalizer overwrite was invisible to a
  fallback-mode assertion that only checks `mode`.

## Requirement-to-evidence matrix

| Criterion | Evidence | Outcome |
|---|---|---|
| A `repo_fetch` 404 is a repairable tool error | `repo_fetch_maps_an_upstream_http_failure_to_a_repairable_tool_error` (`code = upstream_failed`, message carries 404) | pass |
| An omitted `host` resolves to GitHub | `repo_map_defaults_an_omitted_host_to_github` (`host = github`, `mode = native`, entries present) | pass |
| An inferred-host failure is explained | `repo_map_names_the_inferred_host_when_the_forge_lookup_fails` (`repo_ref_not_found`, message names the inference) | pass |
| Off-mode local-only path is unchanged | `repo_map_off_mode_with_matching_local_checkout_returns_structure`; inference is gated on `Policy::Allow` | pass |
| Existing `repo_map` behavior is preserved | the `repo_map` suite and the full gate | pass |
| The advertised surface stays within its budgets | `mcp_schema_slimming` (27 pass, `fetch` = 11,312 bytes) and `tool_surface_live` | pass |

## Verification on this candidate

| Command | Outcome |
|---|---|
| `RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test repo_workflow repo_map_` | pass: 13 passed |
| `RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test repo_workflow repo_fetch_maps_an_upstream` | pass: 1 passed |
| `RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test mcp_schema_slimming` | pass: 27 passed |
| `RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test tool_surface_live` | pass: 1 passed, 1 ignored |
| `RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test mcp_tool_contract` | pass: 22 passed |
| `RUSTUP_TOOLCHAIN=1.89.0 make check` | pass (exit 0) |
| `RUSTUP_TOOLCHAIN=1.89.0 make docs-check` | pass (exit 0) |
| `python3 packaging/check-planning-consistency.py` (via `make packaging-check`) | pass |

## Residual risk and limitations

- Only GitHub is inferred. A GitLab or Codeberg repository still needs an
  explicit `host`; the failure message now says so.
- A failed explicit-host lookup still emits the generic fallback notice beside
  the specific code. Removing it would change the `repo_map` fallback contract
  that `repo_map_fallback_mode_warns_no_native_provider` pins, so this
  candidate keeps both and orders the specific warning first.
- Provider-level live fragility observed in the same sweep (Startpage timeouts,
  Mojeek challenges, NVD rate limits, and a 5 s probe budget under 8 s searches)
  is recorded in the sweep report, not fixed here.
