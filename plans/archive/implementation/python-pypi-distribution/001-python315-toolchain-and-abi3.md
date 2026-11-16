# M001 — Python 3.15, PyO3 Toolchain and Stable-ABI Qualification

Status: closed
Primary class: infrastructure
Source: `plans/subsystems/python-pypi-distribution-roadmap.md` M001.
Execution candidate: branch `plan/python-pypi-prebuilt-arm-release-20261028`, SHA `1135608af5d625cd72db6704adf3bcabd1a58ab1`, Eggsearch 0.4.4.
Hard dependency: closed Python native M008 (`plans/closure/python-native-library/008-status.md`); interface guarantee: `abi3-py311` and ten-tool SDK contract.
Authority: accepted ADR-0005; long-term specification §§6–9 and `plans/003-planning-process.md`.

## Objective and current evidence

Qualify normal CPython 3.11–3.15 on the current five platform families and remove version-bound packaging defects **before** a controlled first PyPI release. Retain one stable-ABI wheel per platform rather than building a wheel for each interpreter. Python 3.15 is final as of 2026-10-09; not experimental. Current `bindings/python/pyproject.toml` explicitly excludes it (`>=3.11,<3.15`); `bindings/python/Cargo.toml` pins PyO3 0.29.3 and `pyo3-async-runtimes` 0.29.0; Maturin is 1.9.5. Workflow tests only 3.11–3.14 and hardcodes `'0.4.4'` in sdist smoke. M008's green 0.4.3 matrix is historical evidence only.

Upstream candidate versions verified as available on 2026-10-28: PyO3 0.30.1, `pyo3-async-runtimes` 0.30.0, Maturin 1.15.0. These are **starting candidates, not forced upgrades**: check current lockfile resolution, Rust 1.89 compatibility and `future_into_py` cancellation behavior before pinning, and select the smallest coherent set proven by tests.

Execution selected that coherent version set. The binding compiles with Rust 1.89.0; the release wheel suite passes 23 tests with six expected debug-gate skips; local `maturin develop` passes 28 tests with one skip, including in-flight cancellation and close. Extracted sdist rebuild from the sdist root succeeds with Cargo offline. Both Cargo dependency graphs pass `cargo deny` after updating `webpki-roots` to 1.0.10, `rustls-webpki` to 0.103.16, and Tokio to 1.53.3; the pinned Python build/audit environment reports no known vulnerabilities. Hosted qualification run `40729011504` passed all 25 CPython 3.11–3.15 smoke cells on the five native platform families at the exact candidate SHA. The complete M001 evidence is recorded in `plans/closure/python-pypi-distribution/001-status.md`.

## Non-goals / preserved contracts

No Python 3.10, PyPy, free-threaded/abi3t claim, Rust API general stabilization, new tool semantics, subprocess wrapper, on-SBC builds, Eggpack edits or PyPI upload in M001. Root dependency graph, ten-tool request/response semantics, no-implicit-config API, side-effect-free import, bounded concurrency, cancellation and trust policy must not regress. Treat C-extension-only changes as isolated to the binding package.

## Ordered work packages

1. Inspect current manifests, both `Cargo.lock` files, CI candidate and PyO3 3.15 compatibility. Upgrade matching PyO3 and async-runtime pair if required; update Maturin/PEP 517 build requirement and pinned action version where compatible. Run `cargo tree` for both locks; preserve Rust 1.89 in the binding. Do not use a Python-version bypass variable to conceal unsupported ABI behavior.
2. Set `requires-python = ">=3.11,<3.16"`, add Python 3.15 classifier, update `docs/python-library.md` and `bindings/python/PYTHON_README.md` to describe normal `abi3-py311` support and explicitly exclude free-threaded Python. Ensure the wheel metadata and generated tags reflect only tested interpreters. Keep the distribution and import name `eggsearch`.
3. Extend `.github/workflows/python-native-qualification.yml` with CPython 3.15 clean installed-wheel smoke for Linux glibc x86_64/aarch64, macOS x86_64/arm64 and Windows x86_64; preserve the five-wheel ABI3 build approach. If hosted `setup-python` lacks a 3.15 interpreter, install a trusted final CPython build in the consumer job and document provenance. Do not substitute build-only checks for interpreter use.
4. Remove the hardcoded version `'0.4.4'` from sdist smoke and use synchronized `Cargo.toml`/`pyproject.toml` metadata; add guard for all three versions, supported Python metadata, and wheel name/abi3 tags. Preserve offline rebuilt sdist from extraction and complete clean-venv consumer testing. Update pinned pytest/tooling where security scanning requires it, with explicit repeat qualification after any toolchain change.
5. Re-run deterministic binding tests, schema/TypedDict coverage, 10-operation fixture calls, Mode-Off zero remote dispatch, import-no-side-effects, cancellation including in-flight release/close, sequential event loops, and Python 3.15 (including import-related changes). Include debug-only bridge tests via local `maturin develop`, and separate release-wheel smoke (debug gates absent).
6. Run dependency/license/vulnerability audits separately for both Cargo graphs and the Python audit/build environment. Record any outstanding finding, severity, owner, and whether it affects published bytes. Keep the workflow actions SHA-pinned and protect root package boundaries.

## Concrete verification

From repository root where applicable: `make check`, `make packaging-check`, `cargo publish --dry-run --locked` on a clean tree, `cargo tree --locked --manifest-path bindings/python/Cargo.toml`, `cargo deny check` inside `bindings/python`, `maturin build --manifest-path bindings/python/Cargo.toml --release --sdist --locked`, `python -m pytest -q bindings/python/tests`, `pyright bindings/python/python/eggsearch bindings/python/examples`. Drive `Python native qualification` on an exact pushed commit; download each platform wheel into separate clean consumer environments with no Rust/CC/Cargo and install using `python -m pip install --no-index --no-deps --only-binary=:all: <wheel>`. Test **each** CPython 3.11–3.15 on every existing wheel platform, not simply 3.15 on x86_64. Validate metadata via `importlib.metadata` and `packaging.tags`.

## Acceptance, stop and closure

Accepted only if all 25 interpreter/platform smoke cells pass on the **same candidate**, sdist extraction/rebuild is green, Python 3.15 is explicitly supported in wheel metadata and actually imports, no code/tool behavior regressions, full security audits are green or explicitly blocked by actionable findings, and root packaging remains unchanged. If upstream ABI dependencies cannot support 3.15 with Rust 1.89, block and record the dependency and the smallest correction instead of falsely advertising compatibility. The closure file `plans/closure/python-pypi-distribution/001-status.md` must include exact SHA, versions, wheel digests, 25-job matrix, commands/outcomes, security and compatibility notes, docs, and residuals. Register closure with roadmap/registry in the same commit. No PyPI upload in this milestone.
