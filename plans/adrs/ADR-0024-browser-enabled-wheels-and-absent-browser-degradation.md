# ADR-0024 — Browser-Enabled Wheels with Absent-Browser Degradation

Status: accepted
Date: 2026-11-24
Accepted: 2026-11-24 (maintainer instruction: "Accept adr-0024 and complete the 2 new plans")
Supersedes: ADR-0014 §7 (Python wheel exclusion) and the browser roadmap non-goal
"Browser rendering in the Python wheel"
Extends: ADR-0014 §4–§6 (default feature set, browser defaults, dependency boundary) and
ADR-0014 §8 (platform gate)

## Context

ADR-0014 shipped PDF and browser rendering in the binary default feature set, enabled by default
through `cfg!(feature = "browser")`, while deliberately excluding browser rendering from the Python
wheel (§7). Its stated reasons were that a wheel still needs a system Chrome and that a wheel-local
CDP client adds size and a second HTTP client at the Python process boundary.

That split now costs more than it saves. A Python consumer of the published wheel cannot render a
JavaScript shell, cannot pass a challenge wall, and receives `browser_not_compiled_in` for a request
the Rust binary of the same version answers. The two artifacts of one version advertise different
capabilities.

Measured on 2026-11-23/24 (macOS arm64, warm cargo cache) by building the wheel with
`features = ["pdf", "browser"]`:

| Fact | Measurement |
|---|---|
| Wheel size | 8,498,057 → 13,064,562 bytes (+4,566,505; **+53.7%**) |
| Build time | 1 min 57 s warm; cold CI is longer |
| New native libraries | none — `reqwest` appears only through `chromiumoxide`, and the graph has **zero** `openssl`/`native-tls` nodes |
| Lockfile | `bindings/python/Cargo.lock` is tracked and gains 11 entries; a `--locked` build fails until it is regenerated |
| Installed capability | `compiled: true, configured: true, discovered: true, usable: true` |
| Render from Python | `renders: 1`, `session_launches: 1`, `resident: true` |
| Client close | `AsyncEggsearch.close()` leaves the browser resident; process exit tears it down |
| Absent-browser seams | the four codes and the `browser_unavailable` warning already exist (see the contract below) |

The user directed on 2026-11-24 that one version ship with PDF and browser across the regular release
and the wheels, that a system Chrome remain the runtime dependency, and that environments without
Chrome keep working with browser functionality simply unavailable.

## Decision

1. **Single artifact alignment.** The CLI and the Python wheel compile `pdf` and `browser` by
   default. There is no variant wheel, no per-target feature set, and no second SKU. This keeps
   ADR-0014 §4's "one executable per target" invariant and extends it to wheels.
2. **System Chrome is a runtime dependency, never a build dependency.** No browser is bundled. A
   manylinux, macOS, or Windows wheel that compiles must stay self-contained: no new shared library
   may enter the wheel for the browser feature.
3. **Absence is a supported state.** Missing Chrome, an unsupported version, an invalid explicit
   path, or `enabled = false` must never fail installation, process startup, a search, or a fetch
   that does not require rendering.
4. **Degradation contract (normative).**

| Condition | Explicit browser request | Implicit (`auto`) request |
|---|---|---|
| Feature not compiled | `browser_not_compiled_in` | HTTP outcome, `browser_unavailable` warning if rendering was warranted |
| `enabled = false` | `browser_disabled` | same |
| Enabled, no executable found | `browser_unavailable` with an actionable reason | same |
| Enabled, unsupported version | `browser_unavailable` naming the version | same |
| Enabled, explicit path invalid | `browser_unavailable` naming the path; no fallback to auto-discovery | same |
| Enabled and discovered but the launch fails | `browser_unavailable`-class failure naming the launch error, never a silent HTTP fetch | the original HTTP outcome plus the failure reason |

`provider_status.browser_capabilities` always reports `compiled`, `configured`, `discovered`,
`usable`, `reason`, and the lifecycle counters, in every one of those states.
5. **Wheel size is accepted and recorded.** The measured increase is accepted; every published
   target records its size, and an unexplained target exceeding 20 MB pauses the release for review
   rather than shipping silently.
6. **Process lifetime is part of the contract.** A resident browser is torn down by the idle timeout,
   by the session request cap, and by process exit. A client close must either tear it down or
   document the hold. A forked child must fail honestly rather than reuse a dead runtime or DevTools
   handle.
7. **Guards and evidence.** The default feature set, the dependency shape, and the lockfile stay
   guarded; a continuous-integration lane without Chrome proves the degradation contract on every
   change, alongside a lane with Chrome that proves rendering still works.

## Consequences

- Roughly +4.6 MB per wheel and a longer cold build; measured per target before publication.
- `docs/python-library.md` currently documents the opposite and must change in the same delivery, as
  must the browser roadmap non-goal.
- Python consumers gain escalation by default, with its latency, exactly as CLI users already have.
- Two new CI lanes and a lockfile that becomes a release input.
- ADR-0014 §8's `armv7-unknown-linux-gnueabihf` compile gate extends to the binding crate, because a
  published armv7l wheel exists.

## Non-goals

Bundling a browser; publishing a second wheel variant or a second project name; enabling persistent
profiles by default; auto-login; making rendering the default extraction path; reopening the tool,
provider, ranking, or evidence-bundle contracts.
