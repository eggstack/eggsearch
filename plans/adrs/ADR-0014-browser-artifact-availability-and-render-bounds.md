# ADR-0014: Default Artifact Capabilities and Bounded Browser Render Lifecycle

Status: accepted
Date: 2026-11-14
Decision owners: project maintainers
Affected subsystems: fetch and extraction; browser rendering; binary distribution and installation; Python native wheel; provider diagnostics; dependency policy; release qualification.
Related: `plans/000-long-term-specification.md` §6, §7; `plans/002-long-term-roadmap.md`; `plans/adrs/ADR-0002-eggfetch-transport-ownership.md`; `plans/adrs/ADR-0006-keyless-deterministic-retrieval-and-optional-extraction.md`; `plans/adrs/ADR-0007-pdf-artifact-availability.md`; `plans/adrs/ADR-0009-bounded-challenge-solving.md`; `plans/adrs/ADR-0010-challenge-solver-coverage-and-cost-policy.md`; `plans/subsystems/pdf-ingestion-fidelity-roadmap.md` M006; `plans/subsystems/repository-hardening-roadmap.md` M006.

## Context and forces

ADR-0006 rejected baking browser and PDF into the default release binary, citing a feature/release policy that forbids multiple SKUs and values the lean default. ADR-0007 revisited that for PDF on measured grounds and accepted alternative 2: `pdf` joins the default feature set, `[fetch].pdf_enabled` resolves to `true` when the feature is compiled, and one binary per target remains. M006 of the PDF roadmap carries that acceptance gate and is still `ready`; no release has shipped the capability.

The shipped default therefore answers `pdf_not_compiled_in` to every PDF fetch and `browser not compiled` to every render request, while `docs/features.md` advertises both as product capabilities. For a general search MCP this is a large capability loss: agents encounter PDFs and JavaScript-shelled documentation routinely, and neither can be reached without a Rust toolchain.

The cost picture is no longer the barrier it appeared to be. The extraction qualification records four arms against one binary family:

| Arm | Binary size | Peak process-tree RSS | JS-only latency (cold / warm) |
|---|---:|---:|---:|
| HTTP only | 19,310,288 B | 23,768 KiB | 447 ms / 26 ms (not rendered) |
| PDF | 20,659,408 B | 24,096 KiB | 540 ms / 33 ms |
| Browser | 23,871,336 B | 1,216,844 KiB | 2,542 ms / 28 ms |
| PDF + browser | 25,216,504 B | 1,202,784 KiB | 2,594 ms / 28 ms |

The browser arm is not a browser-hot-path measurement. Its evidence records `render: "http_only"` for static HTML, text PDF, and scanned PDF, and `render: "browser"` only for the JavaScript-only fixture. The product behaves the same way: `RenderPolicy::Auto` performs HTTP first and escalates only on a JavaScript shell, a non-interactive verification page, or a detected challenge. The +4.56 MB browser and +1.35 MB PDF binary deltas are the only always-on costs; the 1.2 GB figure is the cost of one launched browser, and the 2.5 s figure applies only to the first uncached render of a page that needs it.

Three defects make the current browser path unfit to ship before it is bounded:

1. The shared `BrowserLifecycle` is reused across calls and has no idle teardown. Once one JavaScript page launches Chrome, roughly 1.2 GB stays resident for the process lifetime. For a long-lived stdio MCP server that is the dominant cost, not latency.
2. `[fetch.browser].max_requests` (default 100) is documented as a session bound but is never read outside configuration and type definitions. Nothing ages a browser session out.
3. `BrowserNotCompiledIn` and `BrowserDisabled` exist as fetch error kinds but have no construction site. Without the feature, an explicit `render: "browser"` silently performs an HTTP fetch; with the feature but `enabled = false`, the same call reports a missing Chrome executable. `architecture/fetch.md` §9 states the first outcome should be `BrowserNotCompiledIn`.

Two further facts constrain the decision. `chromiumoxide 0.9.1` declares `reqwest` as a non-optional dependency, used for loopback DevTools HTTP discovery. `packaging/check-http-dependency-shape.py` currently requires the default graph to be entirely reqwest-free and allows reqwest only through chromiumoxide in the all-features graph. That is why repository-hardening M006 (full zero-reqwest closure) is `blocked`, with the explicit instruction not to fork. And `bindings/python/Cargo.toml` depends on `eggsearch` without `default-features = false`, so anything added to `default` also enters every Python wheel.

## Considered alternatives

1. **Keep PDF and browser as source-build opt-ins.** Lowest risk and no dependency change, but the shipped artifact keeps answering refusals to ordinary sources, docs keep advertising capabilities the artifact lacks, and agents have no way to opt in without a Rust toolchain. Rejected; ADR-0007 already rejected this shape for PDF.
2. **Ship PDF by default and keep browser opt-in.** Preserves the reqwest-free default graph and fixes the larger documentation defect for text documents. Rejected as the terminal state because JavaScript-shelled documentation and non-interactive challenge walls remain unreachable on a default install. This remains the fallback if the browser gates fail.
3. **Ship both by default but leave `[fetch.browser].enabled = false`.** Compiles the capability in and refuses to use it until the operator edits config. Rejected for the same reason ADR-0007 rejected the analogous PDF state: it is the worst of both worlds, because the operator has no error to act on.
4. **Ship both by default with `enabled = true`, `policy = "auto"`, a bounded render lifecycle, and a decoupled wheel.** **Selected.** HTTP stays the hot path; rendering is escalation; idle teardown returns the process to baseline; explicit browser requests fail honestly; the wheel keeps only the self-contained PDF capability.
5. **Publish a second browser-enabled binary per target.** Forbidden by `plans/000-long-term-specification.md` §6 and already rejected by ADR-0006. Rejected.

## Decision

1. **Default feature set.** `default = ["pdf", "browser"]`. One executable per target, version-free asset names, and the existing 19-name release inventory are unchanged. No feature-suffixed asset and no second SKU. `--no-default-features` remains a supported source-build path and keeps compiling the feature-absent branches.
2. **PDF defaults.** `[fetch].pdf_enabled` resolves from the compiled-in state: `true` when the `pdf` feature is compiled, `false` otherwise. Explicit values keep their present meaning. M006 of the PDF roadmap owns the measured acceptance and the per-target size evidence for this half.
3. **Browser defaults.** `[fetch.browser].enabled` resolves from the compiled-in state (`true` when the `browser` feature is compiled, `false` otherwise, mirroring the PDF rule) and `[fetch.browser].policy` defaults to `"auto"`. HTTP remains the hot path. Escalation happens at most once per fetch, only for a JavaScript shell, a non-interactive verification page, or a detected challenge, matching the existing `RenderPolicy::Auto` contract.
4. **Bounded render lifecycle.** A new `[fetch.browser].idle_timeout_ms` (default `90000`, minimum `5000`) closes the shared browser after the last completed render and cancels the handler. Teardown must be safe against an in-flight render and must not disable itself; unbounded residency is the defect being closed. `max_requests` is enforced per browser session: when the cap is reached the session closes and the next render relaunches. Both bounds are observable in `provider_status`.
5. **Honest failure taxonomy.** An explicit `render = "browser"` or `browser_profile` request returns `browser_not_compiled_in` when the feature is absent and `browser_disabled` when `enabled = false`, with an actionable message. It never silently performs an HTTP fetch. `auto` remains non-erroring, but a page that needed escalation and could not get it exposes `browser_escalated = false` plus a structured warning carrying the capability reason.
6. **Dependency boundary.** The default and all-features graphs may contain `reqwest` only as a reverse dependency of `chromiumoxide`, for loopback DevTools discovery. `src/` remains reqwest-free and `Cargo.toml` declares no direct `reqwest`; both existing guards stay. `packaging/check-http-dependency-shape.py` is amended to assert the allowed reverse-dependency root set in both graphs instead of requiring absence from `default`. ADR-0002's transport ownership continues to govern eggsearch-owned outbound HTTP; the browser's loopback DevTools client is not provider or fetch traffic, consistent with `architecture/fetch.md` §8 keeping browser traffic direct. This supersedes repository-hardening M006's zero-reqwest objective; the plan becomes `superseded`, its closure record remains historical evidence, and the direct-dependency and source guards remain fail-closed.
7. **Python wheel.** `bindings/python/Cargo.toml` decouples from the binary default with `default-features = false, features = ["pdf"]`. PDF, being pure Rust and self-contained, ships in the wheel; browser rendering does not, because a wheel still needs a system Chrome and a wheel-local CDP client adds size and a second HTTP client to a Python process boundary. This is not a binary SKU: asset names and the release inventory are unchanged, and Python users who need rendering keep a source build, documented as such.
8. **Platform gate.** Before the browser half lands, `cargo check --features browser --target armv7-unknown-linux-gnueabihf` must pass. If it does not, stop and return to alternative 2 rather than introducing per-target feature sets. Where the feature builds but no Chrome exists at runtime, `provider_status.browser_capabilities` must report `compiled = true`, `discovered = false`, `usable = false`, and a reason.

## Consequences and tradeoffs

- **Size.** Combined arm measured 25,216,504 B against 19,310,288 B HTTP-only: +5,906,216 B (+30.6%). ADR-0007 requires this to be measured per target rather than asserted; the browser milestone reuses that evidence obligation.
- **Memory.** Rendering is the only large allocation: ~1.2 GB peak process-tree RSS while a browser is alive, returning to roughly the HTTP-only baseline after idle teardown. Without the teardown this decision would not be acceptable.
- **Latency.** A cache miss on a page that needs rendering costs roughly 2.5 s against 0.45 s over HTTP; cached warm responses are unaffected.
- **Supply chain.** `reqwest` enters the default graph transitively. `cargo deny` already evaluates the all-features graph, so no new crate versions or advisories appear, but the default install now carries the dependency and the dependency-policy gate applies to it. This is the cost of avoiding a fork, which repository-hardening M006 forbade.
- **Trust.** The default process can launch a browser that executes untrusted page JavaScript. Existing controls stay: request interception and network policy, origin scoping, global and per-origin concurrency of 1, bounded DOM and navigation timeouts, manual-login-only profiles, and no challenge solving. Opt-in remains for persistent profiles.
- **Compatibility.** Three absent-key defaults change for default-feature builds: `pdf_enabled`, `browser.enabled`, and `browser.policy`. Explicit values retain their meaning, `--no-default-features` builds keep refusing with the existing codes, and no MCP tool, argument, or response shape changes. The ten-tool contract is untouched.
- **Documentation debt retired.** `docs/features.md` stops describing capabilities the artifact cannot perform.

## Affected long-term sections and subsystems

- `plans/000-long-term-specification.md` §6: the "one canonical default/release binary includes a feature, or the feature remains a documented source-build opt-in" rule selects alternative 4 rather than multiple SKUs. §7's ban on changing browser/PDF availability "merely to improve benchmark or binary-size numbers" does not apply: this is a capability decision with its own qualification, not a performance optimization.
- `plans/002-long-term-roadmap.md`: repository-hardening M006's dependency on an upstream chromiumoxide release is retired by decision.
- ADR-0002: transport ownership unchanged for eggsearch-owned HTTP; the chromiumoxide edge is recorded as a bounded exception.
- ADR-0006: the browser-in-default clause is superseded in part. ADR-0006 is not rewritten; the earlier PDF clause is already superseded by ADR-0007.
- ADR-0007: unchanged; its PDF decision is executed by PDF M006.
- ADR-0009 and ADR-0010: unchanged. Browser rendering still does not solve interactive challenges; engine-level bounded proof-of-work remains the separate opt-in mechanism.

## Migration and compatibility

- Operators with an explicit configuration see no change beyond the defaults they already set.
- Operators relying on the absence of a key gain PDF extraction and browser escalation. Both can be disabled with `pdf_enabled = false` and `browser.enabled = false`.
- `provider_status` already reports `browser_capabilities { compiled, configured, discovered, usable, reason }` and `pdf_capabilities { compiled, configured, usable, layout, ocr }`; the milestone adds lifecycle counters without changing existing fields.
- Source builds with `--no-default-features` keep the existing refusal codes, which become reachable for the first time.

## Verification and status

Status is **accepted**. Acceptance is a decision, not evidence. The milestone plans own the gates, and each gate is SHA-specific:

1. `cargo check --features browser --target armv7-unknown-linux-gnueabihf` passes; otherwise stop and return to alternative 2.
2. Per-target size delta for the seven release targets on the exact candidate, against the current default binary.
3. Realistic-mix qualification: static pages, JavaScript shells, a non-interactive challenge page, and both PDF classes, reporting escalation rate, cold p95 latency, peak process-tree RSS, and RSS after idle teardown.
4. Hosted Linux, macOS, and Windows CI on the merged candidate, plus installer smoke on both platforms and the wheel matrix.
5. `make check`, `make packaging-check`, and `make producer-drift` clean; release assets and the 19-name inventory unchanged.
6. A live MCP run of all ten tools on the exact candidate, including one PDF fetch and one escalated render, recorded as closure evidence.
