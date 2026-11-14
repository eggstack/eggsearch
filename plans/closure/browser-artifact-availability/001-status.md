# B001 — Render Lifecycle Bounds — Closure

Status: closed
Recommendation: closed
Implementation candidate: `5605cc240349c65b6442fa73569516cad8e2ad95`
Plan: `plans/implementation/browser-artifact-availability/001-render-lifecycle-bounds.md`
Roadmap: `plans/subsystems/browser-artifact-availability-roadmap.md` M001

## Implementation

- `[fetch.browser].idle_timeout_ms` (default 90 000, minimum 5 000) with
  configuration validation.
- `BrowserLifecycle` gained a render gate (`RwLock`), last-activity tracking,
  per-session counters, an idle watchdog, and enforced `max_requests`.
- `browser_fetch_with_policy` holds the render gate for the whole render and
  completes the session accounting afterwards; reaching `max_requests` closes
  the session and the next render relaunches.
- `provider_status.browser_capabilities.lifecycle` reports `resident`,
  `session_launches`, `renders`, `idle_teardowns`, and
  `session_cap_teardowns`.
- No default, tool, or schema change in this milestone.

## Requirement-to-evidence

| Requirement | Evidence | Outcome |
|---|---|---|
| Idle teardown after the configured interval | `idle_watchdog_closes_a_resident_session`; live probe with `idle_timeout_ms = 5000` reported `idle_teardowns = 1` and `resident = false` | pass |
| Teardown waits for an in-flight render | `teardown_gate_waits_for_an_in_flight_render` (write side stays blocked while a read guard is held) | pass |
| Recent activity defers teardown | `idle_watchdog_leaves_recent_activity_alone` | pass |
| `max_requests` closes the session after the render | `session_cap_closes_the_session_after_the_render` | pass |
| Relaunch after teardown | Live probe: second `render = "auto"` call escalated again after the idle close | pass |
| Counters and idempotent close | `status_starts_zeroed_and_explicit_close_does_not_count_as_teardown`, `teardown_is_idempotent` | pass |
| Feature-absent builds still compile | `cargo check --locked --no-default-features` | pass |
| Process tree returns to baseline | Live probe: 833 264 KiB of Chrome descendants during the render, 0 after the idle interval | pass |

## Gates and commands

| Command | Outcome |
|---|---|
| `cargo test --locked --all-features --lib fetch::browser::lifecycle` | 10 passed |
| `cargo test --locked --all-features --test browser_transport` | passed (feature-gated browser tests) |
| `make check` | fmt, clippy, no-default check, and 6 320 tests passed; packaging stops at the workstation-only `mavis-trash: unsupported OS: Plan9` |
| `cargo zigbuild --locked --release --target armv7-unknown-linux-gnueabihf` | passed on the final code (4m03s) |
| Hosted CI run `43567644057` | `ci` (Linux) and Windows portability jobs succeeded; macOS canonical verification is recorded with the M002 closure |

## Migration and compatibility

- One new configuration key with a documented default and a hard floor.
- No tool, argument, response-shape, cache, or identity change.
- The lifecycle is process-local; no persisted state.

## Security

- Teardown and the session cap bound a child browser that now ships enabled by
  default. Closing cancels the CDP handler and removes ephemeral user-data
  directories; profile-scoped lifecycles keep their request-scoped close.
- The render gate prevents teardown from racing a navigation.

## Documentation

`docs/config.md`, `docs/features.md`, `architecture/fetch.md` §9,
`architecture/config.md`, and `CHANGELOG.md`.

## Known limitations

- The live RSS observation is one macOS arm64 host with Chrome 157; it is an
  observation, not a threshold.
- The watchdog interval is bounded by a poll tick (maximum 5 s), so teardown can
  occur up to one tick after the configured interval.

## Recommendation

Closed. The lifecycle is bounded, observable, and covered by deterministic unit
tests plus a live teardown observation.
