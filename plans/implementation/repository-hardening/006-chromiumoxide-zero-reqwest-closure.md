# Plan 006 — Chromiumoxide Upstream and Full Zero-Reqwest Closure

Status: blocked

Source roadmap:
`plans/subsystems/repository-hardening-roadmap.md`

Milestone: M006 full zero-reqwest dependency-graph closure

Primary class: maintenance + dependency footprint

Planning baseline: `c49c600b76e690bb1bc52f641554cca6bf79f36f`

Hard dependencies:

- M003 must be closed.
- An upstream chromiumoxide release must provide a capability-preserving way to
  compile eggsearch's launch-only browser path without reqwest.

Relevant long-term requirements:

- `plans/000-long-term-specification.md#4`
- `plans/000-long-term-specification.md#7`

Relevant ADR:

- `plans/adrs/ADR-0002-eggfetch-transport-ownership.md`

## Objective

Eliminate the final transitive reqwest package from eggsearch's all-features
dependency graph without removing browser capability, creating a permanent
eggsearch fork, or coupling chromiumoxide to eggfetch unnecessarily.

## Current evidence and upstream research

After M003, rmcp should no longer require reqwest. The remaining blocker is
`chromiumoxide 0.9.1`.

Current upstream chromiumoxide `Cargo.toml` declares:

```toml
reqwest = { version = "0.13", default-features = false }
```

unconditionally.

Its browser source uses reqwest in `Browser::connect(http[s]://...)` to fetch
the DevTools `/json/version` discovery endpoint, inspect the remote address,
and extract `webSocketDebuggerUrl`.

Eggsearch does not use that HTTP connect path. Its browser lifecycle calls
`chromiumoxide::Browser::launch(config)`, obtains the DevTools WebSocket URL
from the launched browser process, and then uses the CDP/WebSocket transport.

Therefore reqwest is dependency-graph residue for eggsearch, not a currently
executed browser HTTP path.

Upstream already has active work around remote CDP connect (for example
chromiumoxide PR #323 adds authenticated connect headers), but planning research
found no existing optional-reqwest patch.

References:

- https://github.com/mattsse/chromiumoxide/blob/main/Cargo.toml
- https://github.com/mattsse/chromiumoxide/blob/main/src/browser/mod.rs
- https://github.com/mattsse/chromiumoxide/pull/323
- https://github.com/eggstack/eggfetch

## Selected direction

The preferred upstream change is to make HTTP DevTools discovery optional or
injectable while preserving chromiumoxide's default behavior for existing
users.

A minimal compatible shape is:

- reqwest becomes optional;
- a feature such as `http-discovery` retains today's HTTP(S)
  `Browser::connect` behavior and is enabled by chromiumoxide's default
  features if upstream wants no default-behavior break;
- direct `ws://` / `wss://` connection and `Browser::launch` remain
  available without that feature;
- callers that need custom HTTP discovery can resolve
  `/json/version` themselves and pass the resulting WebSocket URL.

Do not require chromiumoxide upstream to depend on eggfetch. Eggsearch's
transport-ownership rule applies to eggsearch-owned HTTP; a generic browser
library should keep its own neutral dependency policy.

If eggsearch later adds remote HTTP CDP discovery, resolve that endpoint with
eggfetch under eggsearch policy and pass the resulting WebSocket URL to
chromiumoxide rather than re-enabling reqwest.

## Invariants

- `browser` remains an available source-build feature.
- Browser launch, interception, SSRF checks, navigation, persistent profiles,
  DOM limits, and lifecycle semantics are unchanged.
- The default/release feature composition remains unchanged.
- No git/path patch to a private fork becomes a permanent release dependency
  without a separate accepted ADR.
- Direct remote CDP capability is not advertised if eggsearch does not support it
  today.

## Non-goals

- Replacing chromiumoxide's WebSocket/CDP engine.
- Rewriting chromiumoxide inside eggsearch.
- Forcing eggfetch into chromiumoxide upstream.
- Adding remote browser/CDP support.
- Removing the browser feature.
- Accepting a fork solely to make `cargo tree` cosmetically clean.

## Upstream work package

### 1. Prepare an upstream-compatible change

Against current chromiumoxide main:

- make the HTTP-discovery HTTP client dependency optional;
- keep default upstream behavior backward-compatible;
- ensure `Browser::launch` and direct WebSocket connect compile without
  reqwest;
- add upstream tests/build-matrix coverage for no-default/no-http-discovery
  configuration;
- document feature behavior.

Prefer a small dependency/feature PR over a broad transport abstraction.

### 2. Track upstream disposition

Record upstream issue/PR and maintainer response in the M006 plan/registry
status.

M006 remains blocked until one of these occurs:

1. an upstream crates.io release contains the optional seam; or
2. maintainers explicitly choose a different clean upstream mechanism that
   gives the same no-reqwest result.

If upstream rejects the direction and zero-reqwest remains a project
requirement, stop and write a dedicated ADR comparing: maintained fork,
different CDP crate, direct CDP implementation, or retaining the dependency.
Do not silently switch to a git fork.

## Eggsearch adoption work package

After an acceptable upstream release exists:

1. update chromiumoxide to that release;
2. keep `default-features = false`;
3. enable only features required by current launch/browser behavior;
4. regenerate the lockfile;
5. verify:
   `cargo tree --locked --all-features -i reqwest` reports that package as
   absent;
6. run all browser lifecycle, profile, transport, SSRF, fetch, and release
   qualification tests.

Add a dependency-shape guard requiring reqwest absence from the all-features
graph. At this point the temporary M003 chromiumoxide exception is removed.

## Failure and recovery semantics

If the new upstream feature configuration changes Browser::launch behavior,
stop adoption and report upstream rather than re-enabling reqwest as an
invisible fallback.

If reqwest remains through a different transitive dependency after adoption,
classify that reverse dependency independently; M006 is not closed until the
all-features graph is actually zero-reqwest.

## Focused verification

Once unblocked:

```bash
cargo tree --locked -i reqwest
cargo tree --locked --all-features -i reqwest
cargo test --locked --all-features --test browser_transport
cargo test --locked --all-features --test browser_profiles
cargo test --locked --all-features --test browser_live_smoke
cargo test --locked --all-features --test fetch_safety
cargo test --locked --all-features --test static_guards
make check
make docs-check
make bench-check
```

Run the repository's browser/live qualification where the environment has a
supported Chromium binary.

## Acceptance criteria

- Upstream chromiumoxide release supports eggsearch's launch-only browser build
  without reqwest.
- Eggsearch adopts that release with no browser capability regression.
- `cargo tree --locked --all-features -i reqwest` contains no reqwest package.
- No private fork/git patch is required.
- Browser SSRF/interception/profile/lifecycle tests remain green.
- Documentation and ADR-0002's historical exception notes state the final
  zero-reqwest graph accurately.

## Stop conditions

Remain blocked if:

- no upstream release supports the required feature boundary;
- adoption would remove or materially alter Browser::launch behavior;
- the only available path is a permanent private fork without a separately
  approved ADR;
- a different reqwest reverse dependency appears and has not been analyzed.

## Closure evidence required

Create `plans/closure/repository-hardening/006-status.md` containing:

- upstream issue/PR/release links;
- chromiumoxide feature graph before/after;
- default and all-features reverse dependency trees proving reqwest absence;
- browser qualification results;
- exact candidate/CI references;
- statement that no supported browser capability was removed.
