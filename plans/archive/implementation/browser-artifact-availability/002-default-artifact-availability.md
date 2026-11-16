# B002 — Default Artifact Availability

Status: ready
Source: `plans/subsystems/browser-artifact-availability-roadmap.md` M002
Primary class: capability
Baseline: `f265f08` (branch `feat/auto-20261110-0a674cdc`)
References: `plans/adrs/ADR-0014-browser-artifact-availability-and-render-bounds.md` § Decision 1–8; `plans/adrs/ADR-0007-pdf-artifact-availability.md`; `plans/000-long-term-specification.md` §6; `architecture/build.md` § Feature flags matrix; `architecture/fetch.md` §9; `architecture/mcp.md` § Policy; `docs/features.md`; `docs/config.md`; `docs/release.md`.
Dependencies: hard B001 (render lifecycle bounds). Interface: PDF roadmap M006 has a stable written contract for the `pdf` default; this plan does not change PDF extraction, only ships it. Operational: a system Chrome/Chromium on the qualification host.

## Objective

Make browser rendering part of the artifact an operator installs: add `browser`
to the default feature set, resolve `[fetch.browser]` to `enabled = true` and
`policy = "auto"` when compiled, make explicit render requests fail honestly when
the capability is unavailable, make automatic degradation observable, and amend
the dependency-shape guard to permit the chromiumoxide edge. The ten-tool surface
and every existing response shape stay unchanged.

## Non-goals

- No render, extraction, DOM, sanitization, or escalation-trigger change. B001 owns
  the lifecycle; this plan only ships it.
- No PDF extraction change; PDF roadmap M006 owns that half and its evidence.
- No new MCP tool, argument, or response field. The new outcomes are error codes
  on an explicit request and one structured warning on `auto` degradation.
- No springboard, no browser download, no challenge solving, no profile auto-login.
- No chromiumoxide fork, patch, or version change.
- No per-target feature sets and no second SKU.
- No wheel change in this milestone; B003 owns the wheel.

## Current implementation evidence

- `Cargo.toml` declares `default = []`, `pdf = ["dep:lopdf", "dep:flate2"]`,
  `browser = ["dep:chromiumoxide"]`. Release builds run
  `cargo build --release --locked` with no feature flags, so published binaries
  carry neither.
- `[fetch.browser]` defaults to `enabled = false`, `policy = "http_only"`,
  `max_requests = 100`; B001 adds `idle_timeout_ms`.
- `run_web_fetch` parses the render policy only under `#[cfg(feature = "browser")]`;
  without the feature the argument is read into an underscore binding and ignored
  (`src/mcp/tools/web_fetch.rs`, render-policy block). An explicit
  `render = "browser"` therefore performs an HTTP fetch with no warning.
- With the feature and `enabled = false`, `state.browser_lifecycle()` is `None`,
  so an explicit `browser` request returns `browser_unavailable` with the message
  "browser rendering is enabled but no Chrome/Chromium executable was found".
  The message is false: rendering is disabled, not missing a browser.
- `FetchError::BrowserNotCompiledIn` and `FetchError::BrowserDisabled` exist with
  `as_str` values `browser_not_compiled_in` and `browser_disabled`, and no
  construction site anywhere.
- `provider_status` already emits `browser_capabilities { compiled, configured,
  discovered, usable, reason }` with per-discovery-state reasons, plus a
  `browser_rendering` bit. Only the disabled/default reasons need to agree with
  the new defaults.
- `packaging/check-http-dependency-shape.py` requires the default graph to contain
  no `reqwest`, and in all-features requires the direct reverse-dependency set to
  be exactly `["chromiumoxide"]`. `chromiumoxide 0.9.1` declares `reqwest`
  non-optionally, so `default = ["browser"]` fails the first assertion today.
- `tests/static_guards.rs` already forbids direct `reqwest` in production source
  and in `Cargo.toml`. Those guards must stay unchanged.
- `tests/config_validation.rs::default_config_has_expected_values` asserts the
  current PDF and browser defaults and becomes feature-aware in this milestone.
- `bindings/python/Cargo.toml` inherits the default; B003 decouples it.

## Invariants that cannot regress

- Exactly ten tools and every existing argument name and response shape.
- HTTP is the hot path; `auto` escalates at most once and only for a JavaScript
  shell, a non-interactive verification page, or a detected challenge.
- No interactive challenge solving, clicking, or simulated input.
- Explicit browser requests never silently become HTTP fetches.
- `src/` remains reqwest-free; `Cargo.toml` declares no direct `reqwest`.
- `default` never enables `egress`.
- One executable per target, version-free asset names, 19-name inventory.
- `--no-default-features` keeps compiling the feature-absent branches, and the
  feature-absent refusal codes remain reachable and tested.
- All existing bounds and policy gates stay: Mode-Off denial, SSRF and redirect
  policy, DOM and navigation caps, concurrency caps, sanitization.

## Expected production changes

1. **Feature default.** `Cargo.toml` gains `default = ["pdf", "browser"]`.
   `--no-default-features` remains supported. No producer edit: `release/eggpack/`
   and the generated workflow inherit the default.
2. **Browser config defaults.** Add `default_browser_enabled()` returning
   `cfg!(feature = "browser")` and use it in the `#[serde(default = "…")]`
   attribute and `Default for FetchBrowserSection`; change the `policy` default
   from `"http_only"` to `"auto"`. A no-feature build therefore reports
   `configured = false` as well as `compiled = false`, so the two layers agree.
   An explicit `enabled = true` in a no-feature build still resolves to
   `browser_not_compiled_in`.
3. **Explicit-request taxonomy.** In `run_web_fetch`, before any fetch:
   - feature absent and the request is explicit (a render policy other than
     `http_only`, or `browser_profile`) → `browser_not_compiled_in` with a message
     naming the feature and the build command;
   - feature present, `enabled = false`, and the request is `render = "browser"`
     or supplies `browser_profile` → `browser_disabled` with a message naming
     `[fetch.browser].enabled`;
   - `render = "auto"` never errors; it degrades to HTTP with the warning below.
4. **Observable `auto` degradation.** Add `WarningCode::BrowserUnavailable`
   (wire string `browser_unavailable`) with a recommended action naming
   `provider_status.browser_capabilities.reason`. Emit it when the HTTP
   classification or challenge detection says escalation is warranted but no
   usable browser exists. Keep `browser_escalated = false` as today. Register the
   code in `src/core/warning.rs`; no prefix-table entry is needed because the
   warning is constructed as a structured warning rather than parsed from a
   fetch-warning string.
5. **Dependency-shape guard.** Rewrite
   `packaging/check-http-dependency-shape.py` to evaluate the default and
   all-features graphs independently: `reqwest` may be absent, or present with
   exactly `["chromiumoxide"]` as its direct reverse-dependency set; anything else
   fails. Keep the direct-dependency and source guards in `tests/static_guards.rs`
   unchanged. Update the guard's failure messages and the build documentation.
6. **Configured policy is the default.** When the caller omits `render`, use
   `[fetch.browser].policy` if rendering is enabled, otherwise `http_only`. An
   explicit per-call `render` overrides it. Validate the configured policy as one
   of `http_only`, `auto`, or `browser`; an invalid configured value is a
   configuration error rather than a silent fallback.
7. **`provider_status` consistency.** Keep the existing keys; ensure `reason`
   distinguishes `not compiled`, `disabled in config`, and discovery failures, and
   that `browser_rendering` reflects `usable` not merely compiled.
8. **armv7 gate.** Run
   `cargo check --locked --features browser --target armv7-unknown-linux-gnueabihf`
   before changing the default. If it fails, stop and return to ADR-0014
   alternative 2 (PDF default, browser opt-in); do not introduce per-target
   feature sets.

## Storage, protocol, and compatibility effects

- Three absent-key defaults change for default-feature builds: `pdf_enabled`,
  `browser.enabled`, and `browser.policy`. Explicit values keep their meaning.
- Two previously dead error codes become reachable on explicit requests; one new
  structured warning code is added for `auto` degradation. No tool schema changes.
- The default dependency graph gains `reqwest` transitively through
  `chromiumoxide`; no new crate versions enter the lockfile.
- Binary size moves to the measured combined arm (~25.2 MB on the recorded macOS
  host); per-target evidence is B003.
- Python wheels are unaffected in this milestone because B003 has not yet
  decoupled them; do not publish a wheel between B002 and B003.

## Ordered work packages

1. Record the armv7 `cargo check` outcome; stop if it fails.
2. `Cargo.toml` default set and config defaults with feature-aware helpers.
3. Explicit-request taxonomy in `run_web_fetch`, including message text and the
   precedence order (not compiled → disabled → discovery).
4. `browser_unavailable` warning: enum, prefix, recommended action, emission.
5. Guard rewrite and its own regression check:
   `python3 packaging/check-http-dependency-shape.py` must pass with browser in
   `default`, and must still fail a synthetic direct `reqwest` dependency (cover
   the failure path in the guard's own test if one exists, otherwise assert by
   inspection and record it).
6. Tests:
   - default config resolves `pdf_enabled = true`, `browser.enabled = true`,
     `policy = "auto"`;
   - `#[cfg(not(feature = "browser"))]` explicit request returns
     `browser_not_compiled_in`;
   - `enabled = false` explicit `browser` returns `browser_disabled`;
   - `enabled = false` `auto` returns HTTP with `browser_unavailable`;
   - a challenge-detected page with no Chrome returns HTTP plus
     `manual_interaction_required` and `browser_unavailable`;
   - `provider_status` reason text for not-compiled, disabled, and no-Chrome.
7. Documentation: `docs/features.md`, `docs/config.md`, `architecture/build.md`,
   `architecture/fetch.md` §9, `architecture/packaging.md`, `docs/release.md`,
   `docs/installation.md`, `README.md`, `docs/python-library.md` (pointer to B003),
   `CHANGELOG.md`.

## Verification

Focused:

```bash
cargo check --locked --features browser --target armv7-unknown-linux-gnueabihf
python3 packaging/check-http-dependency-shape.py
cargo test --locked --all-features --test web_fetch_integration
cargo test --locked --no-default-features --features mock --test web_fetch_integration
cargo test --locked --all-features --test config_validation
cargo test --locked --features mock --test mcp_tools provider_status
cargo test --locked --all-features --test browser_transport
```

Broad:

```bash
make check
make packaging-check
make producer-drift
```

`make check` stops at the workstation-only `mavis-trash: unsupported OS: Plan9`
in `packaging/check-contract.sh`; every check before that point must pass and the
known stop must be recorded. `make producer-drift` must be clean because no
producer input changed.

## Static guards and documentation

- `tests/static_guards.rs`: keep the direct-reqwest and default-feature guards;
  add an assertion that `default` contains `pdf` and `browser` and never `egress`.
- `packaging/check-http-dependency-shape.py`: amended as described, and its output
  string updated so the recorded evidence states the allowed root set.
- `packaging/check-dependency-policy.sh` and `deny.toml`: unchanged; re-run and
  record the all-features graph result.
- Docs listed in work package 7; `docs/test-inventory.md`,
  `architecture/testing.md`, and `skills/eggsearch-dev/SKILL.md` only if a suite
  is added or renamed.

## Acceptance and stop conditions

Accept when:

- A default-feature build with no configuration escalates `auto` to a render on a
  JavaScript-shelled fixture and returns rendered text.
- A `--no-default-features` build returns `browser_not_compiled_in` for an
  explicit browser request and never silently fetches over HTTP.
- `enabled = false` returns `browser_disabled`, and `auto` returns HTTP with a
  `browser_unavailable` warning when escalation was warranted.
- The dependency-shape guard passes with browser in `default` and still enforces
  the chromiumoxide-only edge; the direct and source guards are unchanged.
- armv7 `cargo check` passed before the default changed; the outcome is recorded.
- `make check`, `make packaging-check`, and `make producer-drift` pass apart from
  the recorded workstation packaging stop.

Stop and report, without proceeding to B003, if:

- armv7 does not compile with the browser feature, or
- the guard cannot be made to fail closed for any reqwest root other than
  chromiumoxide, or
- removing the silent fallback exposes a caller that depended on it inside the
  ten-tool surface beyond `web_fetch`, or
- enabling the default changes any existing render, extraction, or policy test
  outcome beyond the documented defaults.

## Required closure evidence

Exact candidate SHA; armv7 check transcript; guard output for both graphs; focused
and broad command outcomes; a live default-build render of a JavaScript fixture
with the escalation marker; the refusal transcripts for no-feature and
`enabled = false`; a `provider_status` sample showing the three reasons; docs
updated; known limitations; recommendation.
