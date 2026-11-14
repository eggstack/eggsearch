# Browser Artifact Availability and Render Bounds Roadmap

Status: closing (M001/M002 closed; M003 conditionally closed)
Baseline: `f265f08` (branch `feat/auto-20261110-0a674cdc`)
Owner: fetch/extraction and distribution
Decision: `plans/adrs/ADR-0014-browser-artifact-availability-and-render-bounds.md`

References: `plans/000-long-term-specification.md` §6, §7; `plans/002-long-term-roadmap.md`; accepted ADR-0002, ADR-0006, ADR-0007, ADR-0009, ADR-0010, ADR-0014; `architecture/fetch.md` §8, §9; `architecture/build.md` § Feature flags matrix; `architecture/packaging.md` § Target contract; `architecture/mcp.md` § Policy; `docs/features.md`; `docs/config.md`; `docs/release.md`; `docs/installation.md`.

## 1. Purpose and ownership boundary

This workstream makes headless browser rendering a property of the artifact an
operator installs, not of a source build, and bounds its runtime cost so the
capability can be on by default. It owns:

- the `browser` feature's membership in the default feature set;
- `[fetch.browser]` defaults and the render lifecycle in `src/fetch/browser/`;
- the MCP boundary behaviour for explicit and automatic rendering in `web_fetch`;
- `provider_status` browser capability and lifecycle reporting;
- the dependency-shape guard decision that follows from compiling the feature in;
- the wheel's relationship to the binary default.

It does not own PDF extraction or fidelity (`plans/subsystems/pdf-ingestion-fidelity-roadmap.md`
M003–M008), the release producer (`release/eggpack/`), challenge solving
(ADR-0009/ADR-0010), or the ten-tool surface.

## 2. Invariants

- Exactly ten MCP tools, unchanged names, arguments, and response shapes. An
  explicit render request may return a new structured error code; it may not
  gain a new field.
- HTTP is the hot path. Browser rendering is escalation: at most one attempt per
  fetch, only for a JavaScript shell, a non-interactive verification page, or a
  detected challenge.
- No browser download, install, or update. Discovery uses an already-present
  Chrome/Chromium; an explicitly configured invalid path fails deterministically.
- No interactive challenge solving, clicking, or simulated input. ADR-0009's
  engine-level bounded proof-of-work is a separate mechanism and is unaffected.
- One executable per target, version-free asset names, and the 19-name release
  inventory. No feature-suffixed asset and no second SKU.
- `src/` never imports `reqwest`; `Cargo.toml` never declares it directly. In
  default and all-features graphs `reqwest` may appear only through
  `chromiumoxide`, for loopback DevTools discovery.
- All existing bounds stay: DOM size, navigation and startup timeouts, global and
  per-origin concurrency, network interception, and sanitization of rendered DOM
  through the same HTML pipeline as HTTP.
- `--no-default-features` remains a supported build, and the feature-absent
  branches keep compiling and keep their refusal codes.

## 3. Non-goals

- OCR, rasterization, a second PDF parser, or an image pipeline.
- A new CDP client, a chromiumoxide fork, or a vendored patch.
- Per-target feature sets or a browser-enabled variant asset.
- Browser rendering in the Python wheel.
- Persistent profiles enabled by default, or any profile auto-login.
- Making rendering the default extraction path, or automatically rendering pages
  that rendered empty over HTTP.
- Reopening the ten-tool, provider, ranking, or evidence-bundle contracts.

## 4. Current state

- `default = []`; release assets are built with `cargo build --release --locked`
  and no feature flags, so published binaries have neither PDF nor browser
  (`docs/release.md` § Release binaries, `architecture/build.md` § Feature flags
  matrix).
- `[fetch.browser]` defaults to `enabled = false`, `policy = "http_only"`,
  `max_requests = 100`. The lifecycle is constructed only when `enabled` is true
  and is reused across calls.
- `RenderPolicy::Auto` already performs HTTP first and escalates only on a
  JavaScript shell, a non-interactive verification page, or a detected challenge.
  The browser arm of the extraction qualification used exactly this routing:
  `render: "http_only"` for static HTML and both PDF fixtures, `render: "browser"`
  only for the JavaScript-only fixture.
- The shared lifecycle has **no idle teardown**. `close()` runs on explicit
  shutdown and for profile-scoped request lifecycles only. `Drop` aborts the
  handler but cannot await a graceful close.
- `max_requests` is passed into `BrowserConfig` but never read during navigation.
  It is documented as a session bound and enforces nothing today.
- `FetchError::BrowserNotCompiledIn` and `FetchError::BrowserDisabled` have no
  construction site. Without the feature, an explicit `render: "browser"` returns
  ordinary HTTP content; with the feature and `enabled = false`, the call reports
  a missing Chrome executable instead of a disabled capability.
- `provider_status` already reports `browser_capabilities { compiled,
  configured, discovered, usable, reason }` and a `browser_rendering` server bit.
  There are no lifecycle counters.
- `chromiumoxide 0.9.1` requires `reqwest` unconditionally, so the default graph
  cannot carry it without a guard change. `packaging/check-http-dependency-shape.py`
  currently fails any default graph containing `reqwest`. Repository-hardening
  M006 is `blocked` on this, with an explicit instruction not to fork.
- Measured arms: HTTP 19,310,288 B; PDF 20,659,408 B; browser 23,871,336 B;
  combined 25,216,504 B. Peak process-tree RSS during browser use 1,202,784 –
  1,216,844 KiB against 23,768 KiB HTTP-only. JS-only cold latency ~2.5 s against
  ~0.45 s HTTP; warm cached responses are unaffected.

## 5. Dependency graph

```text
M001 render lifecycle bounds
  └──> M002 default artifact availability (hard)
         └──> M003 distribution and live qualification (hard)

PDF roadmap M006 (independent; owns the pdf half)
  └── coordinates with M003 on wheel feature selection
```

- M002 is not dependency-ready until M001 closes: enabling escalation by default
  while Chrome has no idle teardown would pin roughly 1.2 GB for the lifetime of
  every long-running session, which is the defect this workstream exists to close.
- M003 is evidence work for the exact candidate; it cannot start before M002 lands.
- The PDF half has no new plan. M006 of the PDF roadmap is the vehicle, and the
  browser M003 plan owns the wheel's explicit feature selection so the wheel gains
  PDF without gaining browser.

## 6. Milestones

### M001 — Render lifecycle bounds (infrastructure) — closed

Closure: `plans/closure/browser-artifact-availability/001-status.md`.

Introduce `[fetch.browser].idle_timeout_ms` and make the render lifecycle
self-limiting: teardown after an idle interval with no in-flight render, enforced
`max_requests` per session, safe cancellation against concurrent renders, and
lifecycle counters exposed through `provider_status`. No default changes.

Exit condition: after a render and the idle interval, no Chrome process remains
and the process tree returns to the HTTP-only footprint; a concurrent render is
never killed by teardown; reaching `max_requests` closes the session and the next
render relaunches.

### M002 — Default artifact availability (capability) — closed

Closure: `plans/closure/browser-artifact-availability/002-status.md`.

Add `browser` to the default feature set, change `[fetch.browser]` defaults to
`enabled = true` and `policy = "auto"`, construct the two dead error kinds with
actionable messages, make `auto` degradation observable, amend the dependency-shape
guard to allow the chromiumoxide edge in both graphs, and keep the source-level
reqwest guards fail-closed. Includes the armv7 compile gate and the documentation
sweep.

Exit condition: a default build renders a JavaScript-shelled page with no
configuration, a source build without the feature returns `browser_not_compiled_in`
for an explicit request, `enabled = false` returns `browser_disabled`, and the
dependency guard still rejects any other reqwest root in either graph.

### M003 — Distribution and live qualification (infrastructure/evidence) — conditionally closed

Closure: `plans/closure/browser-artifact-availability/003-status.md`. Outstanding: macOS x86_64/aarch64 evidence, Windows arm64 validation, installer smoke beyond x86_64 Linux, and the realistic-mix matrix.

Measure and record the exact candidate: per-target size delta, realistic-mix
qualification with idle teardown, installer smoke, wheel decoupling and matrix,
hosted Linux/macOS/Windows CI, and a live MCP run of all ten tools including one
PDF fetch and one escalated render.

Exit condition: the qualification record is written against the exact candidate,
the wheel carries PDF and not browser, the release inventory is unchanged, and the
live tool-surface verification is recorded in the closure record.

## 7. User-visible exit conditions

- An installed release reads an ordinary PDF with no source build and no extra
  configuration, and disables it explicitly with `pdf_enabled = false`.
- An installed release renders a JavaScript-shelled page when `auto` escalates,
  with no configuration, and returns to baseline memory after the idle interval.
- An agent that explicitly asks for browser rendering gets a stable, actionable
  error when the capability is unavailable, never a silent HTTP substitution.
- An operator can see, from `provider_status` alone, whether browser rendering is
  compiled, configured, discoverable, usable, and currently resident.

## 8. Cross-cutting concerns

**Security.** Rendering executes untrusted page JavaScript in a child process.
The existing interception and network policy, origin scoping, concurrency caps,
DOM and navigation bounds, and sanitization pipeline stay in force. Default-on
means the child process can now appear without a config edit, so teardown and
session caps become security-relevant bounds, not only resource bounds.

**Supply chain.** `reqwest` enters the default graph through chromiumoxide. No new
crate versions appear because the all-features graph already carried them, but the
default install now ships the dependency and `cargo deny` evaluates it in both
graphs. The direct-dependency and source-level guards remain.

**Compatibility.** Three absent-key defaults change. Explicit configuration keeps
its meaning. No tool schema changes; the new outcomes are error codes on an
explicit request and a structured warning on `auto` degradation.

**Observability.** `provider_status` gains lifecycle counters and keeps the
existing capability shape. Release smoke keeps its keyless CLI and MCP checks.

**Migration.** Operators who must not launch Chrome set `[fetch.browser].enabled =
false`. Operators on hosts without Chrome see `discovered = false` with a reason
and continue to receive HTTP extraction.

## 9. Risks and deferred work

- **armv7 build.** `chromiumoxide` must compile for `armv7-unknown-linux-gnueabihf`.
  This is a hard gate in M002; if it fails, the workstream returns to PDF-only
  default rather than introducing per-target feature sets.
- **armv7 runtime.** Chromium is effectively unavailable on that target. The
  feature compiles but cannot escalate, and capability reporting must say so
  rather than leaving agents to guess.
- **Chrome version drift.** Rendering quality and CDP compatibility depend on the
  host browser. Qualification records the exact version and does not claim
  threshold stability.
- **Sticky memory if teardown misbehaves.** M001 is a hard predecessor for this
  reason; M002 cannot land on an unbounded lifecycle.
- **Dependency hygiene.** Repository-hardening M006's zero-reqwest objective is
  superseded, not achieved. The direct and source guards remain, and the
  reverse-dependency set stays restricted to chromiumoxide. Removing the edge
  remains deferred until an upstream release makes HTTP discovery optional; if one
  appears, it is an optimization under this ADR, not a reopening of it.
- **Wheel parity.** The wheel will not render. This is a deliberate asymmetry, not
  an oversight, and must be documented in `docs/python-library.md` and
  `docs/features.md`.
