# M004 — Python Wheel, Sdist and Release Qualification

Status: blocked (hard dependency: M003 closed; operational: platform qualification and explicit publishing approval)
Primary class: infrastructure
Source: `plans/subsystems/python-native-library-roadmap.md` M004
Planning baseline: `main` `f386b07d7adaa06bbb4a51a9d7e2eecf971c238b`; re-qualify actual candidate after M003.
ADRs: `ADR-0005`; long-term `plans/000-long-term-specification.md` ``6, 7, 9.

## Objective, scope, gates

Make the Python library distributable through ordinary pip/uv workflows, with verifiably complete source distributions, compatible signed-off native wheels, repeatable provenance and clean-install tests. This milestone qualifies the build and prepares release artifacts; **do not publish to PyPI or modify an immutable GitHub release** without an explicit later publication authorization. M003 must be closed before the qualification gate.

Non-goals: editing Eggpack-generated seven-target binary producer by hand, changing root CLI artifact inventory, new binary SKU or installers, Windows ARM64/PyPy/free-threaded CPython/ARMv7 support without independent evidence, bundling Chrome, C ABI, TypeScript, unattended publication, mandatory source compilation on wheel-supported hosts.

## Baseline and architectural constraints

The root `Cargo.toml` uses Rust 1.89 and specifies one Rust binary/lib; `make release-check`, Eggpack release producer and `packaging/check-contract.sh` control the existing binary release. The Python `bindings/python` Cargo package must remain separately built and published. Maturin documentation requires carefully qualified mixed-project `python-source`, `module-name` and full sdist manifest when a binding crate path-depends on the root parent. Do not assume an sdist built from a nested directory contains a parent Rust package; test extraction and offline isolated rebuild.

## Ordered work packages

**WP1 — Distribution policy and name.** Verify PyPI name `eggsearch` ownership/availability, distribution metadata, SPDX license, README, supported Python versions, classifiers, package import name and version synchronization with the root crate without implying identical release cadence. Choose exact supported PyO3/maturin versions, evaluate `abi3-py311` for CPython 3.11–3.14; if stable ABI or Python 3.14 fails, qualify interpreter-specific wheels and document reasons. Do not claim free-threaded ABI. Decide release signing/attestation policies and publisher credentials separately; source builds are explicit fallback only.

**WP2 — Self-contained sdist and wheel builds.** Ensure `maturin build --sdist` contains the binding crate, root Rust dependency sources/manifests, license, needed lockfile/assets and no secrets/test transcripts, and installs from extracted sdist in fresh environment without repository checkout. Avoid accidental `cargo package` bloating of primary crate or source copying that silently drifts from the root code. Generate platform-tagged wheels and validate ELF/DYLIB/PE dependencies, auditwheel/delocate equivalents, correct module import layout and absence of bundled subprocess executable. Validate manylinux-compatible glibc baseline instead of assuming current binary glibc policy transfers automatically to extension dependencies.

**WP3 — Qualification matrix.** Establish managed CI for Linux x86_64 and aarch64, macOS x86_64 and arm64, Windows x86_64, CPython 3.11–3.14 (reduced ABI3 interpreter wheel inventory permitted if empirically supported). On each candidate: compile with locked or audited resolved dependencies, install `pip --only-binary=:all:` from candidate wheelhouse in a clean venv with no Rust toolchain, import, run a no-network 10-tool fixture smoke, ensure asyncio concurrency/cancellation/close and visible package types; test `uv pip install` if available. Distinguish cross-compiled wheel import from native runtime qualification (QEMU vs native). ARM SBC aarch64 evidence desirable, but do not silently label an untested Raspberry Pi class supported. Test extracted sdist build under network-free cached-source conditions or document required build-network preconditions precisely.

**WP4 — Supply chain and existing pipeline guardrails.** Pin GitHub Actions to immutable SHAs like the repository's other workflows; apply dependency/license/vulnerability scanning to `pyo3`/maturin/build actions; record exact source SHA, dependencies, wheel digests, interpreter/platform tags, attestations and package metadata. Prefer draft/manual publish and TestPyPI/private wheelhouse verification first. Never mutate `release/eggpack` producer authority, `packaging/release-inputs.txt`, `packaging/expected_release_assets.py` or binary installer/update paths for Python wheels. Ensure `cargo publish --dry-run --locked` and root `make check` remain unchanged and pass. No secrets in log/error payloads.

**WP5 — Release docs and closure.** Document Python install/support matrix, Python-native import vs MCP installation, config/credential setup, features unavailable in wheels, sdist fallback, version compatibility with Rust Eggsearch, cancellation/limits/trust behaviors, uninstall/upgrades, and operational support. Cross-reference root README minimally. Ensure docs' claim of PyPI availability remains conditional until an actual separately authorized publication. Update test inventory/architecture and handoff future packaging owner.

## Failure, security and compatibility

Use deterministic fixture tests, never depend on live keyless provider uptime for build gates. Missing/invalid wheels must fail install clearly; runtime import must not trigger auto-download, update, process spawn, browser or API probing. Preserve package ABI and Python version tags honestly; avoid unreviewed relaxed `abi3` flags to suppress build errors. Root default crate and binary compatibility is a mandatory regression gate.

## Exact verification and acceptance

Root: `make check`, `make release-check` (if candidate clean), `cargo package --list` and `cargo publish --dry-run --locked`; `./packaging/check-contract.sh` and `make producer-drift` only if producer inputs intentionally change (not expected). Binding dir: `maturin build --release --sdist`, `maturin build --release` with declared targets/interpreters, `python -m pytest -q`, `python -m build` only if its backend configuration is supported, `python -m pip install --no-index --find-links <wheelhouse> --only-binary=:all: <distribution>` into clean test venv; `python -c "import eggsearch"`; record actual CI command variations. Validate wheel metadata/tags, sdist contents/extracted build and all ten tool behaviors.

Accepted when every advertised target has native import/use evidence on the exact artifact, source distribution recreates the package from clean extraction, supply-chain and existing binary gates pass, and release decision/provenance is documented. Publication is a separate authorization and is **not** an acceptance shortcut. If wheel targets require incompatible root dependency/unsafe policy changes, STOP and register a bounded corrective rather than weakening gates.

Closure: `plans/closure/python-native-library/004-status.md` records exact immutable candidate SHA, full wheel/ABI matrix with checksums and native smoke, extracted sdist evidence, component inventories, package-name decision, security and compatibility verification, unresolved unsupported targets and publish status. Mark roadmap/registry closed only after this record and evidence land together.
