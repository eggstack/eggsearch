# M006 Browser-Enabled Python Wheels — Closure Evidence

Status: closed
Recommendation: closed; all M006 acceptance gates passed on the exact candidate.
Implementation candidate: `fedf82c67a6d331fa1fab127b4ad5376cd27a84c`
Source version: `0.4.5` (not published by this work)
Date: 2026-11-24
Plan: `plans/implementation/python-pypi-distribution/006-browser-enabled-wheels.md`
Roadmap: `plans/subsystems/python-pypi-distribution-roadmap.md`
Hard dependency: browser M006 absent-browser degradation, closed on this same candidate; see `plans/closure/browser-artifact-availability/006-status.md`.
PR: [#20](https://github.com/eggstack/eggsearch/pull/20), open and unmerged at the time of closure.

## Candidate and implementation

Qualification is SHA-specific; this record applies to `fedf82c67a6d331fa1fab127b4ad5376cd27a84c` only. M006 implementation and qualification commits are `e71b8f93f77159eca7914f82aa0bbb6e1ff0e3b0`, `2061e2636ec1b66757fac31de99c971764de9a26`, `120a922cbf8a66a21cb812eb256b39dac0b722ff`, `af09fe5bdcb55137a4bcd456a40c6a235580539b`, and `fedf82c67a6d331fa1fab127b4ad5376cd27a84c`. The last commit changes only the bounded timeout settings used by real-Chrome qualification tests; production defaults are unchanged.

## Requirement-to-evidence matrix

| Requirement | Evidence | Outcome |
|---|---|---|
| Six `pdf` + `browser` wheels build with the locked binding graph | Python native qualification run [45363963916](https://github.com/eggstack/eggsearch/actions/runs/45363963916): Linux x86_64/aarch64/ARMv7, macOS x86_64/arm64, Windows x86_64; all build jobs passed | pass |
| Installed wheels load across supported CPython versions | Same run: all 25 native wheel-consumer smoke jobs for CPython 3.11–3.15 passed; installed capability tests passed | pass |
| No-Chrome installation/search/fetch and explicit failure semantics | Same run: five native target-family explicit-absence checks passed; Linux x86_64 automatic discovery without Chrome passed in a browser-free container. Search and ordinary fetch remain usable, while explicit rendering returns `browser_unavailable` with a cause | pass |
| Real Chrome capability and render/lifecycle behavior | Same run: release wheel reports Chrome capability usable; mock-only fixture wheel performs a real Chrome render and asserts idle/session-cap counters. Startup/fetch/outer test timeouts are bounded in test setup | pass |
| ARMv7 binding compile and installed-wheel qualification | Same run: ARMv7 hard-float binding check and wheel build passed; emulated ARMv7 installed-wheel smoke passed | pass |
| Source distribution is complete, offline-rebuildable, and installable | Same run: sdist built locked, extracted, rebuilt offline, installed, and smoke-tested | pass |
| Wheel and sdist size gate | Exact file sizes from the successful hosted build logs are recorded below; each wheel is below 20,000,000 bytes | pass |
| Browser dependency adds no native TLS/shared-library dependency | Same run's binding dependency audit passed; browser `reqwest` edge remains restricted to `chromiumoxide`, with no `openssl` or `native-tls` node | pass |
| Release and project gates | Hosted CI [45363963977](https://github.com/eggstack/eggsearch/actions/runs/45363963977), release drift [45363963952](https://github.com/eggstack/eggsearch/actions/runs/45363963952), and egress qualification [45363963974](https://github.com/eggstack/eggsearch/actions/runs/45363963974) succeeded on the exact head | pass |

## Exact-head artifact sizes

Sizes are the uncompressed wheel/sdist file sizes reported by the build jobs in Python native qualification run [45363963916](https://github.com/eggstack/eggsearch/actions/runs/45363963916), not the larger uploaded-artifact archive sizes.

| Target / artifact | Bytes | Decimal MB |
|---|---:|---:|
| Linux x86_64 wheel | 15,988,476 | 15.99 |
| Linux aarch64 wheel | 15,604,215 | 15.60 |
| Linux ARMv7 hard-float wheel | 15,028,841 | 15.03 |
| macOS x86_64 wheel | 14,896,946 | 14.90 |
| macOS arm64 wheel | 14,391,602 | 14.39 |
| Windows x86_64 wheel | 14,032,259 | 14.03 |
| Source distribution | 1,631,081 | 1.63 |

All six wheels are below the 20 MB review threshold. Artifact SHA-256 manifests are retained in their corresponding hosted workflow artifacts.

## Local checks and documentation

- Focused local Chrome tests passed: `browser_render_with_chrome` and `browser_idle_and_session_cap_counters_are_visible`.
- Local full Python suite passed: 27 passed, 9 skipped.
- `python3 -m py_compile bindings/python/tests/test_native.py` and `git diff --check` passed.
- Local `RUSTUP_TOOLCHAIN=1.89.0 make check` and `make docs-check` passed on prior candidate `af09fe5bdcb55137a4bcd456a40c6a235580539b`; exact-head hosted CI and qualification passed on `fedf82c67a6d331fa1fab127b4ad5376cd27a84c`.
- User-facing and contributor docs updated: `docs/python-library.md`, `bindings/python/PYTHON_README.md`, `docs/features.md`, `docs/config.md`, `architecture/python-library.md`, and `architecture/fetch.md`.

## Security, compatibility, and operational evidence

The real-Chrome test uses the deterministic HTTPS URL `https://93.184.216.34/eggsearch-browser-render-fixture`, but no public fixture host is contacted or needed. With the `mock` feature, request interception in `src/fetch/browser/navigate.rs` fulfills the synthetic HTML locally after the existing request policy check; the mock fixture is absent from normal release artifacts. Private-target protections remain unchanged. No credentials or live provider checks were used.

All wheel targets share one `pdf` + `browser` feature set; Chrome remains a runtime dependency and is not bundled. The source candidate stays at version `0.4.5`; no publication was performed.

## Known limitations and findings

- Docker is not available locally; the hosted browser-free container lane passed.
- ARMv7 physical runtime/benchmark evidence remains blocked under Python M004 by maintainer deferral. M006 proves the hosted cross-build and emulated installed-wheel smoke only; it makes no physical-device claim.
- No unresolved M006 findings. PR #20 remains open and unmerged; this closure does not publish `0.4.5`.
