# Browser Artifact Availability and Render Bounds Roadmap

Status: closing (M001–M005 acceptance complete; PR #16 merged as `aea34ee`); ready (M006 absent-browser degradation)
Baseline: `f265f08` (branch `feat/auto-20261110-0a674cdc`)
Owner: fetch/extraction and distribution
Decision: `plans/adrs/ADR-0014-browser-artifact-availability-and-render-bounds.md`;
`plans/adrs/ADR-0024-browser-enabled-wheels-and-absent-browser-degradation.md` (accepted 2026-11-24)

References: `plans/000-long-term-specification.md` §6, §7; `plans/002-long-term-roadmap.md`; accepted ADR-0002, ADR-0006, ADR-0007, ADR-0009, ADR-0010, ADR-0014, ADR-0024; `architecture/fetch.md` §8, §9; `architecture/build.md` § Feature flags matrix; `architecture/packaging.md` § Target contract; `architecture/mcp.md` § Policy; `docs/features.md`; `docs/config.md`; `docs/release.md`; `docs/installation.md`.

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
- Browser rendering in the Python wheel — superseded 2026-11-24 by ADR-0024: the wheel compiles
  the feature and relies on the M006 degradation contract instead of a different artifact.
- Persistent profiles enabled by default, or any profile auto-login.
- Making rendering the default extraction path, or automatically rendering pages
  that rendered empty over HTTP.
- Reopening the ten-tool, provider, ranking, or evidence-bundle contracts.

## 4. Current state

- Release binaries compile `pdf` and `browser` by default. Browser defaults to
  `enabled = true` and `policy = "auto"`; the Python wheel remains PDF-only.
- HTTP is the hot path. `auto` escalates only for JavaScript shells,
  non-interactive verification pages, or detected challenges.
- The shared lifecycle enforces `max_requests`, tears down after 90 seconds idle,
  and reports residency and teardown counters through `provider_status`. The
  M003 realistic-mix requalification exposed a first-render accounting defect;
  the corrective work is tracked separately in M005.
- Explicit render requests return actionable errors when browser support is
  unavailable or disabled. Unserved automatic escalation is reported as a
  structured warning.
- M001 and M002 are closed. Their closure records capture implementation and
  live-tool evidence.
- M003 acceptance is complete on exact candidate
  `fb99a43734bd4f2329bdfa2ba4315b403c2cd985`; PR-associated run `43897293446`,
  attempt 7, passed all 34 jobs and all displayed PR checks are green. PR #16
  merged as `aea34eeb89749e3196712d9e9b9741f75c58e760` on 2026-11-16T21:05:25Z.
  The approved 294-sample public fixture requalification passed locally: 39 expected cold browser escalations,
  HTTP routing for static/PDF cases, idle teardown, request-13 cap teardown,
  request-14 relaunch, and final process cleanup. Evidence is in
  `target/m003-public-fixture/evidence-candidate-fb99a43/evidence.json`; sampled
  peak descendant RSS was 1,180,352 KiB.
- On the same candidate, the macOS arm64 installer/release smoke passed with
  binary SHA-256 `f6dcee6b3bce92c96d7dc0d89c56e3bc7bd5e8f5abe26beb20cbbfb926a43ac3`,
  and the live ten-tool MCP smoke passed with PDF fetch, JavaScript browser
  escalation, aliases, duplicate-alias rejection, four behavior hints, and the
  canonical-only advertised schema. Evidence is under
  `target/m003-installer-smoke/evidence-candidate-fb99a43/` and
  `target/m003-live-mcp/evidence-candidate-fb99a43.json`.
- Hosted CI (`43897293997`), egress (`43897294001`), and Eggpack drift
  (`43897293439`) passed on `fb99a43`. Exact-SHA qualifier `43901942424`, attempt 2,
  passed all seven builds, qualification stages, and consumer validations,
  including aarch64 macOS; Windows wrappers passed and their evidence was uploaded.
  `required_gate` (`133470803916`) and `aggregate` (`133471719996`) passed, with
  aggregate outcome `complete`. The exact seven-target size inventory is recorded
  in the M003 closure record. Python full-matrix run `43916322850`, attempt 4,
  passed all 34 jobs on this SHA, including all 25 installed-wheel smoke cells,
  ARMv7 build/smoke, sdist rebuild/smoke, and dependency audit. Exact wheel sizes
  and the dependency delta are in the closure record. Earlier attempts had two
  aarch64 macOS consumer jobs cancelled before runner assignment; targeted retries
  on the same SHA completed them. The PR-associated run `43897293446`, attempt 7,
  passed all 34 jobs and all displayed PR checks are green. PR #16 merged as
  `aea34eeb89749e3196712d9e9b9741f75c58e760` on 2026-11-16T21:05:25Z.
  Earlier green results on `cdc9445` do not transfer. That candidate's Windows
  wrapper step failed because its mapping assertion was sensitive to CRLF checkout;
  `fb99a43` normalizes line endings.
- The approved public fixture origin is retained and browser private-target
  protections remain unchanged. `packaging/test-install.sh` is blocked on this
  workstation by `mavis-trash: unsupported OS: Plan9`; neither `pwsh` nor
  `powershell` is installed locally. Do not bypass the guard.

## 5. Dependency graph

```text
M001 render lifecycle bounds
  └──> M002 default artifact availability (hard)
         └──> M003 distribution and live qualification (hard)

PDF roadmap M006 (independent; owns the pdf half)
  └── coordinates with M003 on wheel feature selection

M003 ──> M006 absent-browser degradation contract (correctness; independent of the wheel)
ADR-0024 + M006 ──> Python M006 browser-enabled wheels (delivery)
```

- M002 is not dependency-ready until M001 closes: enabling escalation by default
  while Chrome has no idle teardown would pin roughly 1.2 GB for the lifetime of
  every long-running session, which is the defect this workstream exists to close.
- M003 is evidence work for the exact candidate; it cannot start before M002 lands.
- The PDF half had no new plan. The wheel's feature selection was owned by browser M003 and kept
  PDF without browser; ADR-0024 supersedes that on 2026-11-24, Python M006 now compiles both, and
  browser M006 owns the degradation contract that makes a browser-less host safe.

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

### M003 — Distribution and live qualification (infrastructure/evidence) — closed

Closure: `plans/closure/browser-artifact-availability/003-status.md`. Exact candidate `fb99a43734bd4f2329bdfa2ba4315b403c2cd985` passes the local realistic-mix, macOS arm64 installer/release-smoke, live ten-tool MCP checks, hosted CI (`43897293997`), egress (`43897294001`), and Eggpack drift (`43897293439`). Exact-SHA Eggpack qualification (`43901942424`, attempt 2) passed all seven builds, qualification stages, and consumer validations, including aarch64 macOS; `required_gate` and aggregate passed, and the seven-target size inventory is in the closure record. Python full-matrix run `43916322850`, attempt 4, passed all 34 jobs on this SHA; the wheel inventory and dependency delta are recorded in the closure record. PR-linked Python run `43897293446`, attempt 7, passed all 34 jobs and all displayed PR checks are green. PR #16 merged as `aea34eeb89749e3196712d9e9b9741f75c58e760` on 2026-11-16T21:05:25Z. See the closure record.

Measure and record the exact candidate: per-target size delta, realistic-mix
qualification with idle teardown, installer smoke, wheel decoupling and matrix,
hosted Linux/macOS/Windows CI, and a live MCP run of all ten tools including one
PDF fetch and one escalated render.

Exit condition: the qualification record is written against the exact candidate,
the wheel carries PDF and not browser, the release inventory is unchanged, and the
live tool-surface verification is recorded in the closure record.

### M004 — M003 qualification corrective pass (infrastructure/evidence) — closed

Implementation plan: `plans/archive/implementation/browser-artifact-availability/004-m003-qualification-corrective-pass.md`.

Correct the Python request-type schema coverage and unsupported macOS arm64 runner
mapping found by M003 qualification. Preserve M003's blocked status and all
existing tool and SSRF contracts. A public HTTPS synthetic fixture origin was
approved and used during requalification; do not weaken the browser's
private-target policy. The newly exposed lifecycle accounting defect is tracked
separately in M005.

Exit condition: corrective contract and producer checks pass, and all original
M003 acceptance criteria are then requalified on one exact candidate before M003
can be considered for closure.

### M005 — Session-cap initial render accounting (infrastructure correction) — closed

Implementation plan: `plans/archive/implementation/browser-artifact-availability/005-session-cap-initial-render-accounting.md`.

The M003 realistic-mix cap probe found that the baseline increments
`requests_this_session` in `begin_render` before a cold browser launch, then the
successful launch resets that counter. As a result, `max_requests` tears down one
render later than configured on a cold session. Move per-session counting after
successful browser acquisition while retaining overall render telemetry at entry;
failed launches must not consume session capacity. Add deterministic regression
coverage and requalify on a new exact candidate. Do not change the restart budget
or M003's tool/security contracts.

Exit condition: the cold-starting render counts as request one; a failed launch
consumes no session capacity; the cap closes after exactly `max_requests` renders;
the following render relaunches within the existing budget; lifecycle and browser
transport regressions pass. The full M003 acceptance criteria have now also passed
on exact candidate `fb99a43734bd4f2329bdfa2ba4315b403c2cd985`; see its closure record.

The local exact-candidate fixture on `fb99a43734bd4f2329bdfa2ba4315b403c2cd985`
confirmed cap teardown after render 13 and relaunch on render 14, along with idle
teardown and final process cleanup. The hosted Python matrix, Eggpack qualifier,
CI, egress, and drift gates all pass on this SHA; see the M003 closure record.

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
- **Wheel parity.** Superseded 2026-11-24 by ADR-0024. The wheel now compiles rendering and the
  asymmetry moves from capability to environment: a wheel host without Chrome still runs, and the
  M006 degradation contract governs what it reports.

### M006 — Absent-browser degradation contract (correctness) — ready

**Objective.** Prove and, where needed, complete the promise this workstream already makes: a host
without Chrome still installs, starts, searches, and fetches, and every browser-unavailable outcome
names its exact cause.

**Why now.** ADR-0024 compiles the feature into the Python wheel as well, so the absence path stops
being an edge case and becomes the default on every machine without a browser.

**Scope.** Seam audit across the CLI, MCP tools, `provider_status`, `doctor`, `browser_login`,
`browser_profiles`, and the Python client; exact-code assertions for `browser_not_compiled_in`,
`browser_disabled`, and `browser_unavailable`; `auto` warning completeness; the
discovered-but-launch-fails path (container sandbox, missing shared library); a continuous-integration
lane with no browser executable; documentation of the degradation table and discovery order.

**Exit.** Every seam degrades without panic, hang, or silent explicit fallback; capability reporting
is accurate for all five discovery states; the no-Chrome lane is the regression barrier.

**Plan.** `plans/implementation/browser-artifact-availability/006-absent-browser-degradation-contract.md`.
