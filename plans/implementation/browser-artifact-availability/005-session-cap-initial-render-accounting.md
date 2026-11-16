# B005 — Session-Cap Initial Render Accounting

Status: active
Source: `plans/subsystems/browser-artifact-availability-roadmap.md` M005
Primary class: infrastructure correctness
Baseline: `8faa0f15a40f336f361d523c9b45b3fe3b0106ad` (branch `feat/auto-20261110-0a674cdc`)
References: `plans/implementation/browser-artifact-availability/001-render-lifecycle-bounds.md` (archived); `plans/closure/browser-artifact-availability/001-status.md`; `plans/implementation/browser-artifact-availability/003-distribution-and-live-qualification.md`; `plans/implementation/browser-artifact-availability/004-m003-qualification-corrective-pass.md`; ADR-0014 § Decision 7 and § Verification.
Dependencies: Interface: M001's `max_requests` session-cap contract. Operational: M003 exact-candidate requalification after this correction; M003 remains blocked until all its acceptance criteria pass.

## Objective

Correct the browser lifecycle so the render that launches a cold browser counts
against that browser session's configured `max_requests` cap. Add regression
coverage for the launch/accounting sequence, then requalify the cap behavior on a
new exact candidate without treating any evidence from the baseline SHA as
transferable.

## Current evidence

- The adjusted public-fixture qualification on baseline candidate
  `8faa0f15a40f336f361d523c9b45b3fe3b0106ad` produced 294 matrix samples and
  recorded 39/39 expected cold JavaScript/verification browser escalations.
- The same report observed idle teardown, a later browser relaunch, and final
  teardown with zero browser descendants. The realistic matrix therefore passes
  those observed routing and idle-lifecycle checks on that candidate.
- Its separate `max_requests = 13` probe rendered 13 pages while reporting the
  browser still resident and zero session-cap teardowns. The next render raised
  the render count to 14 and only then recorded one session-cap teardown.
- `src/fetch/browser/lifecycle.rs` increments `requests_this_session` in
  `begin_render`, before `ensure_browser` launches the initial browser. Successful
  launch resets that counter to zero, discarding the cold-starting render from the
  session count. `finish_render` then tears down one render later than configured.
- The existing `session_cap_closes_the_session_after_the_render` unit test does
  not launch a browser between `begin_render` and `finish_render`, so it does not
  exercise this reset ordering. The main realistic-mix run uses
  `max_requests = 100`, so its repeated matrix did not reach the cap.
- The adjusted report is non-qualifying only because of this cap-probe
  expectation; it remains diagnostic evidence, not M003 closure evidence.

## Non-goals

- No MCP tool, argument, schema, response shape, cache, identity, or CLI change.
- No relaxation or bypass of SSRF, private-address, DNS, browser interception,
  or challenge-handling protections.
- No change to `MAX_RESTARTS`, the restart budget, or the meaning of idle teardown.
- No claim that M003 is closed; this correction does not replace its exact-SHA
  wheel, hosted, installer, live-MCP, or other acceptance gates.
- No edits to generated release workflows or Eggpack inputs.

## Invariants

- The first render performed by a newly launched browser counts as request one
  for that browser session; failed browser launches do not consume that session's
  request budget.
- A session is closed after exactly `max_requests` renders; the following render
  may use the existing single-restart budget.
- The lifecycle render gate continues to serialize teardown against in-flight
  renders, and counters remain process-local.
- The ten-tool MCP contract, browser SSRF protections, and HTTP-first routing
  remain unchanged.

## Expected changes

- Keep total render telemetry in `begin_render`, but increment the per-session
  request count only after `ensure_browser` returns a browser. Retain the session
  reset at launch/teardown so a failed launch does not consume the next session's
  cap.
- Complete launch/reset bookkeeping while holding the browser slot, before
  publishing the browser to concurrent render callers, so no request races with
  the session reset.
- Add deterministic unit coverage that exercises a `begin_render` followed by
  browser-launch bookkeeping and verifies the first render triggers the cap when
  `max_requests = 1`.
- Update `architecture/fetch.md` with the precise first-render accounting rule.

## Ordered work packages

1. Add regression tests at the lifecycle state boundary for cold-start ordering
   and a failed launch that must not consume session capacity.
2. Move per-session request accounting after successful browser acquisition while
   keeping overall render telemetry at entry; preserve launch/teardown resets and
   do not alter restart policy.
3. Run focused lifecycle and browser-transport tests, then the broader checks
   below. Preserve the known workstation `mavis-trash: unsupported OS: Plan9`
   stop without bypassing it.
4. Rebuild and rerun the public synthetic realistic-mix qualification on the new
   exact candidate. Require the cap teardown after request 13 and a successful
   single relaunch on request 14; also recheck idle teardown and the 39 expected
   cold escalations.
5. Record the new candidate SHA and outcomes. Keep M003 blocked until every
   original M003 acceptance criterion is requalified on that same SHA.

## Verification

```bash
cargo fmt --check
cargo test --locked --all-features --lib fetch::browser::lifecycle
cargo test --locked --all-features --test browser_transport
cargo test --locked --all-features
make check
make packaging-check
```

The realistic-mix qualification is an additional exact-candidate runtime check;
unit tests and local builds do not substitute for it. The existing public
synthetic fixture origin is retained, and the runtime's private-target
protections must remain unchanged.

## Acceptance and stop conditions

Accept this corrective pass when deterministic regressions fail against the
baseline ordering and pass with the correction; a failed browser launch does not
consume session capacity; `max_requests = 1` closes the session after the
cold-starting first browser render; the 13-request diagnostic records the
session-cap teardown after render 13; render 14 relaunches once; and focused plus
broad Rust checks pass apart from any explicitly recorded workstation-only
wrapper stop.

Stop if the change needs an MCP contract change, weakens a security boundary,
changes restart-budget semantics, or fails the lifecycle/browser transport
regressions. M003 remains blocked until its complete original acceptance suite
passes on one exact candidate.

## Required closure evidence

Corrective source diff and candidate SHA; regression test and focused/broad check
outcomes; exact-candidate realistic-mix evidence covering the first-render cap,
request-13 teardown, request-14 relaunch, idle teardown, browser escalation,
and process cleanup; unresolved limitations; and a recommendation that does not
claim M003 closure unless its full gate is complete.
