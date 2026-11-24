# M006 — Absent-Browser Degradation Contract

Status: ready
Date: 2026-11-24
Subsystem: browser artifact availability and render bounds
Milestone class: correctness (capability degradation)
Decision: `plans/adrs/ADR-0024-browser-enabled-wheels-and-absent-browser-degradation.md`
References: ADR-0014 §4–§8; `architecture/fetch.md` §8–§9; `docs/features.md`; `docs/config.md`; `src/fetch/browser/discover.rs`; `src/mcp/tools/web_fetch.rs`; `src/mcp/tools/provider_status.rs`; `plans/closure/browser-artifact-availability/003-status.md`.

## Objective

Make "no Chrome installed" a proven, harmless state across every entry point, and make every
browser-unavailable outcome name its exact cause. The feature is now compiled in everywhere, so the
absence path is the common case on a fresh Linux server, a slim container, and any machine without a
browser.

## What already exists (verified 2026-11-24)

- `discover_browser()` yields `Available`, `NotFound`, `NotConfigured`, `VersionUnsupported`, or
  `ExplicitPathInvalid`; `browser_lifecycle` is constructed whenever `enabled` is true, so an
  undiscovered browser is "enabled but unavailable", not "disabled".
- Tool seams return `browser_not_compiled_in` (feature absent), `browser_disabled`
  (`enabled = false`), and `browser_unavailable` (enabled, not usable), while an implicit `auto`
  request returns the HTTP outcome plus a `browser_unavailable` warning carrying the capability
  reason.
- `provider_status.browser_capabilities` reports `compiled`, `configured`, `discovered`,
  `usable`, `reason`, and lifecycle counters, with a reason string per discovery state.
- Unit coverage exists for the capability report; there is **no** end-to-end lane that runs with no
  browser installed.

## Work packages

1. **Seam audit.** Build a table of every entry point — CLI `search`/`fetch`, MCP `web_search`,
   `web_fetch`, `batch_fetch`, unified `fetch`, `provider_status`, `doctor`, `browser_login`,
   `browser_profiles`, and the Python client — and record the exact outcome with no Chrome present.
   Any hard failure, panic, hang, or silent HTTP retry at an explicit browser request is a defect.
2. **Taxonomy tightening.** Assert the exact code at each seam for each discovery state, including
   the branch where the lifecycle is absent because rendering is disabled versus the branch where it
   exists but discovery failed. No call may report `browser_disabled` for a missing executable.
3. **Warning completeness.** Prove the `auto` path emits `browser_unavailable` with the reason
   whenever rendering was warranted, and emits nothing when it was not.
4. **Launch-failure path.** A discovered browser can still fail to start (container sandbox, missing
   shared library, profile lock). Ensure the outcome names the launch error and never silently
   repeats the HTTP fetch; cover the container case with the documented `--no-sandbox` guidance.
5. **No-Chrome CI lane.** Add a lane that removes or hides every browser executable and runs the
   seam audit as an executable test: search still returns provider results, fetch still returns
   content, explicit browser requests return the exact structured code, and `provider_status`
   reports `usable: false` with a reason. This lane is the regression barrier the product currently
   lacks.
6. **Documentation.** Publish the degradation table in `docs/features.md` and mirror it in
   `docs/config.md` and the browser section of the Python guide, including the discovery order and
   the `[fetch.browser].executable` escape hatch.

## Acceptance

- The seam audit table is complete and every row passes in the no-Chrome lane.
- Explicit browser requests return exactly `browser_not_compiled_in`, `browser_disabled`, or
  `browser_unavailable`; no silent HTTP fallback exists.
- `auto` requests never fail because rendering is unavailable, and warn exactly when warranted.
- `provider_status.browser_capabilities` is accurate for all five discovery states.
- A launch-failure test yields a named reason rather than a bare status.
- `make check`, `make docs-check`, `packaged-assets`, and `planning-consistency` pass.

## Stop conditions

Stop and report if any entry point cannot degrade without changing a tool schema, an error code, or
the advertised tool inventory; if the no-Chrome lane cannot be made deterministic on a hosted runner;
or if a launch-failure path would require bundling a browser.
