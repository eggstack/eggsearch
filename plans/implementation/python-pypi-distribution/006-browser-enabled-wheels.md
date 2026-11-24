# M006 — Browser-Enabled Python Wheels

Status: closed
Closure record: `plans/closure/python-pypi-distribution/006-status.md` (candidate `fedf82c67a6d331fa1fab127b4ad5376cd27a84c`)
Date: 2026-11-24
Baseline: `e012dd7aa9071b1fc14e206e727ac8635f4064e4` on `docs/user-facing-polish-20261123`, after merging `origin/main` `4de20a0f0d748b89147829a51384e385a5bee388`
Subsystem: Python PyPI distribution and prebuilt SBC wheels
Milestone class: capability (distribution alignment)
Decision: `plans/adrs/ADR-0024-browser-enabled-wheels-and-absent-browser-degradation.md`
Dependency: browser M006 absent-browser degradation contract must land first or in the same delivery
References: ADR-0014 §4–§8; `architecture/python-library.md`; `docs/python-library.md`; `bindings/python`; `.github/workflows/python-native-qualification.yml`; `.github/workflows/release-python-pypi.yml`; `packaging/check-http-dependency-shape.py`; `packaging/release-inputs.txt`; `plans/closure/python-pypi-distribution/003-status.md`.

## Objective

Ship one Python wheel set whose capability matches the shipped CLI: PDF extraction and browser
rendering both compiled, with system Chrome discovered at runtime and its absence handled by the
degradation contract rather than by a different artifact.

## Measured baseline (2026-11-23/24, macOS arm64)

| Fact | Measurement |
|---|---|
| Wheel with `features = ["pdf", "browser"]` | builds, installs, imports |
| Size change | 8,498,057 → 13,064,562 bytes (+4,566,505; +53.7%) |
| Warm build time | 1 min 57 s |
| Native libraries added | none; `reqwest` only via `chromiumoxide`; zero `openssl`/`native-tls` |
| `browser_capabilities` | `compiled/configured/discovered/usable = true` on a Chrome host |
| Render from Python | `renders: 1`, `session_launches: 1`, `resident: true` |
| `--locked` build | fails until `bindings/python/Cargo.lock` is regenerated (11 new entries) |
| `close()` | leaves the browser resident; process exit tears it down |

## Work packages

1. **Manifest and lock.** Set the binding dependency to `default-features = false, features = ["pdf", "browser"]`; regenerate the tracked `bindings/python/Cargo.lock`; prove `maturin build --release --locked` succeeds and that an offline sdist build still resolves from the committed lock.
2. **Dependency-shape guard.** `packaging/check-http-dependency-shape.py` inspects only the repository root, and there is no workspace, so the binding graph is currently unchecked. Extend it to evaluate `bindings/python` too: `reqwest` may appear only through `chromiumoxide`, and no `openssl`/`native-tls` node may enter the wheel graph.
3. **Release plumbing.** Add `bindings/python/Cargo.lock` to `packaging/release-inputs.txt` and cover it in the packaged-asset check, so the Python M005 class of defect (a required file missing from the packaged set) cannot recur.
4. **Python process lifetime.** Decide and implement the resident-browser contract at the Python boundary: a client close tears the browser down (preferred) or documents the idle hold; a forked child fails with an explicit error instead of reusing a dead runtime or DevTools handle; idle-teardown and session-cap counters are asserted from Python.
5. **CI lanes.** Extend the wheel matrix with two capability lanes over the existing platform families: one with Chrome that asserts `usable: true` and performs one real render, and one without Chrome that asserts `usable: false` with a reason, an exact `browser_unavailable` code for an explicit request, and unchanged success for search and HTTP fetch. Keep the `armv7-unknown-linux-gnueabihf` compile gate from ADR-0014 §8, now covering the binding crate.
6. **Qualification and documentation.** Record every wheel and sdist size, the installed capability report per platform, and the no-Chrome outcome; update `docs/python-library.md`, `bindings/python/PYTHON_README.md`, `docs/features.md`, and `architecture/python-library.md`; retire the browser roadmap non-goal that forbids rendering in the wheel; bump the version and add a changelog entry for the capability change.

## Acceptance

- Six wheels (linux x86_64/aarch64/armv7l, macOS x86_64/arm64, Windows x86_64) and the sdist build with `--locked` from a clean checkout.
- Every wheel installs and reports `compiled: true`; on a Chrome host `usable: true` and one render succeeds.
- On a host without Chrome every wheel installs, imports, searches, and fetches without error; an explicit browser request returns `browser_unavailable` naming the cause; `provider_status` reports `usable: false` with a reason.
- No wheel gains a native shared library for the browser feature; the dependency-shape guard fails if one does.
- Sizes are recorded per target; no target exceeds 20 MB without an explicit review decision.
- `make check`, the Python suites, `make docs-check`, `packaged-assets`, and `planning-consistency` pass.

## Stop conditions

Stop and report rather than shipping if: `openssl`, `native-tls`, or any new shared library enters
the wheel graph; the armv7 target fails to compile with the browser feature; a platform's wheel
exceeds 20 MB without an accepted review; the resident-browser or fork semantics cannot be made safe
without changing the public Python API; or ADR-0024 is not accepted.
