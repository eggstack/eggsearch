# B001 — Render Lifecycle Bounds

Status: ready
Source: `plans/subsystems/browser-artifact-availability-roadmap.md` M001
Primary class: infrastructure
Baseline: `f265f08` (branch `feat/auto-20261110-0a674cdc`)
References: `plans/adrs/ADR-0014-browser-artifact-availability-and-render-bounds.md` § Decision 4; `plans/000-long-term-specification.md` §6, §7; `architecture/fetch.md` §8, §9; `architecture/mcp.md` § Policy; `docs/features.md` § Browser Rendering.
Dependencies: none. Hard predecessor for B002.

## Objective

Make the headless browser self-limiting so it can later be enabled by default.
Concretely: add a configurable idle teardown that closes the shared
`BrowserLifecycle` after a period with no in-flight render, enforce the documented
`max_requests` session cap, make teardown safe against concurrent renders, and
expose lifecycle counters through `provider_status`. No default changes and no MCP
surface change in this milestone.

## Non-goals

- No change to `default` features, `[fetch.browser].enabled`, or `policy`.
- No new tool, argument, config section, or response field outside
  `provider_status.browser_capabilities`.
- No new error kinds; the two existing dead kinds are handled in B002.
- No change to render policy semantics, escalation triggers, discovery, or
  profile handling.
- No chromiumoxide fork, patch, or dependency change.
- No performance claim beyond the measured teardown behaviour.

## Current implementation evidence

- `BrowserLifecycle` holds `browser: Mutex<Option<Arc<Browser>>>` and a
  `handler_handle`; `close()` takes both and cleans the ephemeral user-data
  directory. `Drop` aborts the handler and cannot await a graceful close.
- The shared lifecycle is created in `ServerState` only when
  `config.fetch.browser.enabled`; `run_web_fetch` and `run_browser_fetch` reuse it
  across calls. Only profile-scoped request lifecycles are closed per request.
- There is no idle timer, no `last_activity`, and no active-render count anywhere
  in `src/fetch/browser/`.
- `[fetch.browser].max_requests` exists in `FetchBrowserSection`, is validated
  positive, is copied into `BrowserConfig`, and is never read during navigation.
- `provider_status` emits `browser_capabilities { compiled, configured,
  discovered, usable, reason }` and a `browser_rendering` server bit; it has no
  lifecycle counters.
- `docs/features.md` states request count and navigation time are bounded by
  configuration; only navigation time is true today.
- Browser tests live in `tests/browser_transport.rs` (feature-gated) and
  `tests/browser_live_smoke.rs` (`live-smoke`, `#[ignore]`). Both skip when no
  Chrome is discoverable, using the established discovery pattern.

## Invariants that cannot regress

- A render in flight is never closed by teardown; teardown waits for it.
- Teardown is idempotent and safe under repeated or concurrent invocation.
- The first render after teardown relaunches cleanly, including the ephemeral
  user-data directory lifecycle and the singleton-lock cleanup.
- `global_concurrency` and `per_origin_concurrency` still cap renders; teardown
  does not bypass either semaphore.
- Profile-scoped lifecycles keep their existing request-scoped close.
- Ten tools, existing argument names, and every existing response shape are
  unchanged. `provider_status` gains keys only inside `browser_capabilities`.
- Feature-absent builds compile and behave exactly as today.

## Expected production changes

1. **Configuration.** Add `idle_timeout_ms: u64` to `FetchBrowserSection` with
   `default_browser_idle_timeout_ms() -> 90_000`, set it in `Default for
   FetchBrowserSection`, and validate `>= 5_000` in the existing fetch validation
   path. A value below the floor is a configuration error, not a clamp, so an
   operator cannot silently disable the bound.
2. **Lifecycle state.** `BrowserLifecycle` gains:
   - `idle_timeout: Duration`;
   - `render_gate: tokio::sync::RwLock<()>` where each render holds a read guard
     for its full duration and teardown takes the write guard;
   - `last_activity: Mutex<Instant>`, updated when a render completes;
   - per-session counters (`session_launches`, `renders`, `idle_teardowns`,
     `session_cap_teardowns`) and a `resident` flag derived from the browser slot;
   - a teardown task handle stored alongside the lifecycle and cancelled by
     `close()`.
3. **Teardown task.** Spawned with the lifecycle. It wakes at a bounded interval
   (the smaller of one quarter of the idle timeout and five seconds), takes the
   write guard, and closes only when no render is in flight and
   `last_activity.elapsed() >= idle_timeout`. Closing resets per-session counters
   and increments `idle_teardowns`. The task must not hold the guard across a
   browser launch.
4. **Session cap.** Count completed renders per session. When the count reaches
   `max_requests`, close the session after the render returns and increment
   `session_cap_teardowns` (a separate counter from `idle_teardowns`); the next
   render relaunches. Do not reject the render that reaches the cap.
5. **Observability.** Extend `provider_status.browser_capabilities` with
   `resident`, `session_launches`, `renders`, `idle_teardowns`, and
   `session_cap_teardowns`. The values are process-local observations, not
   thresholds; document them as such.
6. **Shutdown.** Keep the existing explicit close on server shutdown, and ensure
   the teardown task is aborted before cleanup so it cannot race with shutdown.

## Storage, protocol, and compatibility effects

- One new config key with a documented default and a hard floor. No existing key
  changes meaning.
- `provider_status` gains four keys inside an existing object; no tool schema or
  wire string changes.
- No cache, identity, or evidence-bundle effect.
- Feature-absent builds compile with the new config field and report zeroed
  counters.

## Ordered work packages

1. Config: field, default helper, `Default` impl, validation, and
   `tests/config_validation.rs` cases for the floor.
2. Lifecycle: render gate, activity tracking, counters, teardown task, and
   shutdown ordering. Keep `close()` idempotent.
3. Enforcement: wire the render guard into `browser_fetch_with_policy` and the
   `max_requests` counter into the navigation path.
4. Reporting: extend `provider_status` and its JSON-shape test.
5. Tests: teardown after timeout, teardown deferred by an in-flight render,
   relaunch after teardown, session-cap relaunch, repeated close, and zeroed
   counters without a lifecycle.
6. Docs: `docs/config.md`, `docs/features.md`, `architecture/fetch.md` §9, and a
   `CHANGELOG.md` entry.

## Verification

Focused:

```bash
cargo test --locked --all-features --test browser_transport
cargo test --locked --all-features --lib fetch::browser
cargo test --locked --all-features --test config_validation
cargo test --locked --features mock --test mcp_tools provider_status
```

Live teardown observation (requires Chrome; record the version and the idle
interval used):

```bash
cargo test --locked --all-features --test browser_live_smoke -- --ignored --nocapture
```

The record must show: peak process-tree RSS during a render, RSS after
`idle_timeout_ms` plus one poll interval, and no remaining Chrome child process.

Broad:

```bash
make check
```

`make check` stops at the workstation-only `mavis-trash: unsupported OS: Plan9`
in `packaging/check-contract.sh`; every check before that point must pass and the
known stop must be recorded as such.

## Static guards and documentation

- `tests/static_guards.rs`: no new guard required; confirm the no-direct-reqwest
  and default-feature guards still pass unchanged.
- `packaging/check-http-dependency-shape.py`: unchanged in this milestone.
- Docs to update: `docs/config.md` (browser table), `docs/features.md` (bounded
  extraction claims), `architecture/fetch.md` §9, `CHANGELOG.md`.

## Acceptance and stop conditions

Accept when:

- A render followed by the idle interval leaves no browser process, and the
  process tree returns to the HTTP-only baseline on the recorded host.
- An in-flight render is never cut off, and a render that starts during the idle
  window resets the deadline.
- Reaching `max_requests` closes the session and the next render succeeds.
- Counters move as specified and are zero without a lifecycle.
- No MCP schema, tool, or default changes; `make check` passes apart from the
  recorded packaging stop.

Stop and report, without proceeding to B002, if:

- Teardown cannot be made race-free against a render start with the existing
  lifecycle ownership, or
- Relaunch after teardown leaves a stale singleton lock or ephemeral directory, or
- The measured post-teardown footprint does not return to roughly the HTTP-only
  baseline on the recorded host.

## Required closure evidence

Exact candidate SHA; diff summary; the focused and broad command outcomes; a
recorded teardown observation with peak and post-teardown RSS, Chrome version,
and idle interval; the `provider_status` counters sample; documentation updated;
known limitations and any residual race or relaunch finding classified by
severity; recommendation.
