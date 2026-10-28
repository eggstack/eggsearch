# Python PyPI Distribution and Prebuilt SBC Wheels Roadmap

Status: active (M001/M002/M003 ready; M004 blocked on first PyPI publication)
Primary outcome: publish the existing in-process `eggsearch` Python package promptly, with CPython 3.11–3.15 and broad precompiled platform wheels, explicitly experimental SBC support, then perform hardware benchmarking **after publication**.
Planning baseline: `main` `13fcac0b7ac0125d38e4e9775ebe1d62028fda72` (2026-10-28; root/binding/pyproject 0.4.4). Refresh the baseline at execution.
Authority: user confirmed the `eggsearch` PyPI project name is available by manual check and requested a release-first sequence without compiling on SBCs.
Owners: `bindings/python`, `.github/workflows/python-native-qualification.yml`, a separate Python publication workflow, Python documentation, and Python-only benchmarking. Not Eggpack.
References: `plans/000-long-term-specification.md` §§1, 6–9; `plans/001-terminology-and-domain-model.md`; `plans/002-long-term-roadmap.md`; `plans/003-planning-process.md`; accepted `plans/adrs/ADR-0005-python-native-library-boundary.md`; `architecture/python-library.md`; historical `plans/subsystems/python-native-library-roadmap.md` and `plans/closure/python-native-library/008-status.md`.

## Boundary and decision

The original Python native library M001–M008 is **closed**, not reopened. Its M008 historical qualifying evidence (`067468a`, 27/27 jobs on 0.4.3) does not qualify the current 0.4.4 or a future PyPI release candidate. This is a separate distribution/publication workstream. No Python subprocess/MCP-server fallback is introduced. The library continues to delegate the ten canonical operations through `src/sdk.rs` and the Rust MCP tool handlers.

**Release first, hardware benchmarks later.** Build and publish cross-platform wheels on CI/hosted runners or cross-compilation containers. Never require a native SBC Rust toolchain, Cargo, C compiler, or wheel compilation for installation or pre-publication qualification. Where no native ARMv7 runner exists, use hosted QEMU/compatible emulation and explicitly mark the installed result as emulation-qualified. Mark ARM/SBC compatibility *experimental* until post-release native-device evidence is gathered. Do not describe unverified SBCs as tested.

The initial PyPI upload is part of M003, not M004. M004 performance characterization, tuning and native hardware sampling **must not block first publication**. Only correctness, distribution integrity, security, and at-least-emulated target installation gates are release prerequisites.

## Invariants and exclusions

- Preserve the canonical ten-tool API, schema fingerprint, tool payloads, typed calls, trust markers, sanitization, SSRF/config/Mode-Off constraints, 32-call bounded admission semantics, Python cancellation/close guarantees, and import-side-effect-free behavior.
- Keep PyO3, `pyo3-async-runtimes`, and Maturin dependencies isolated to `bindings/python`; preserve root Rust 1.89 minimum and binary/CLI packaging, the Eggpack 19-asset contract, and `#![deny(unsafe_code)]` in the root.
- No false interpreter/platform tags, `--manylinux off` passing as publishable, ABI mismatches, undocumented glibc/musl requirements, or claims of PyPy/free-threaded Python/Windows ARM64/ARMv6 without qualification.
- Do not add a mandatory on-SBC build, native SBC benchmarking, or physical SBC hardware to the release gate. No Rust/cargo invokes on SBC during first-release preparation. Source distribution is an explicit developer/unsupported-target fallback, not the SBC installation method.
- PyPI name is user-verified as available, not a reason to delay writing plans; check ownership and exact version once immediately before first publication. Do not pre-announce PyPI availability until the public index serves the bytes.
- Do not create a second release artifact inventory inside Eggpack, automatically publish on root tags, or modify previously published immutable releases.
- Upstream provider services are not a dependency of deterministic CI smoke.

## Compatibility and wheel policy

Required first-release matrix: CPython 3.11, 3.12, 3.13, 3.14, 3.15 with `abi3-py311`, plus existing five native wheel families (Linux glibc x86_64/aarch64 at the demonstrated manylinux2014/2.17 floor, macOS x86_64/arm64, Windows x86_64) and **Linux ARMv7 hard-float** (`armv7-unknown-linux-gnueabihf` / PyPI `armv7l` tag), targeting `manylinux_2_31_armv7l` if actually qualified. The ARMv7 glibc 2.31 floor means Debian 11-class or newer systems only; it is not a generic all-Pi/all-ARM promise. Keep the ABI3 baseline; there is no need for per-CPython wheels for a given platform.

Additional desirable hosted-built wheels: `musllinux_1_2` for Linux x86_64/aarch64, optionally ARMv7 and i686. M002 determines which can be qualified without destabilizing the mandatory first-release matrix. Do not silently advertise a target lacking a wheel. If any required target cannot be built and installed under controlled emulation/native CI, record a **release blocker**, not an unqualified substitute or a misleading tag; seek an explicit scoped decision rather than silently dropping ARMv7.

The term *experimental SBC support* covers hardware-model/runtime/footprint uncertainty, not unvalidated ABI or broken wheel loading. CI must prove a correct, importable wheel under a matching target environment before it reaches PyPI.

## Dependency graph, milestones, and exits

| Milestone | Class | Status | Dependencies | Release-facing exit |
|---|---|---|---|---|
| M001 Python 3.15 and binding toolchain qualification | infrastructure | ready | hard: historical Python M008 closed | `abi3-py311` wheel installs on CPython 3.11–3.15 across the five established platforms; matching PyO3/async and Maturin versions, metadata, tests, sdist and audits qualified on an exact SHA. |
| M002 hosted ARMv7 and Linux wheel expansion | infrastructure | ready | interface: M001 keeps `abi3-py311` stable; operational integration before M003 | A publishable ARMv7 hard-float manylinux wheel, hosted/emulated install/import/representative operations, tags and ELF audit; supplemental musl wheels where qualified; no SBC compilation. |
| M003 controlled PyPI first publication | capability | ready (publication gated) | interface: established M001/M002 artifact contracts; operational: M001/M002 qualification and exact candidate | Trusted Publisher configured; exact-SHA wheel/sdist set passes audits and clean consumers; operator publishes `eggsearch` to PyPI and independently verifies public binary-only installs. |
| M004 post-publication SBC benchmarking and tuning decision | polish | blocked | operational: successful M003 publication | Native SBC installed-wheel benchmarks for memory, latency and concurrency; no source compilation; evidence-based recommendations/patch release only if needed. |

M001, M002 and the M003 *workflow implementation* may proceed in parallel against the fixed interface and artifact contract. The **M003 publish action** cannot run until M001/M002 exact-release-candidate acceptance gates pass. M004 benchmarking is deliberately scheduled **after** publication; a device not immediately available is not a retroactive blocker for M003.

Implementation handoffs: `plans/implementation/python-pypi-distribution/001-python315-toolchain-and-abi3.md` through `004-post-publication-sbc-benchmarking.md`. Closure records go to `plans/closure/python-pypi-distribution/NNN-status.md`. Keep the registry aligned.

## Global verification and risks

Record exact full commit SHA, frozen version, wheel names/tags, hashes, host/emulator architecture, build/audit tools and CI run IDs. Run `make check`, `make packaging-check`, `cargo publish --dry-run --locked` when tree cleanliness permits, binding `pytest`/type checks, sdist extraction/rebuild, and actual installation from the *selected final* wheelhouse with `--only-binary=:all: --no-deps --no-index`. Runtime tests must cover all ten operations, configuration policy, no-network Mode-Off, lifecycle/cancellation and import without side effects. An experimental ARMv7 wheel must at least pass a release-installed smoke under emulation; full physical-SBC performance and long-duration soak are M004.

Risks: cross-ABI interpreter discovery, glibc floor/correct `armv7l` tagging, host runner/emulator availability, PyO3 ABI compatibility with 3.15, and upstream audit findings. Any incompatibility must be visible as a blocked requirement with the exact failing evidence. Publication requires human-controlled approval (GitHub environment) and immutable PyPI version handling; a failed upload cannot be overwritten.
