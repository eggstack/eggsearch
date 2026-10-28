# Python PyPI Distribution and Prebuilt SBC Wheels Roadmap

Status: active (M001–M003 closed; M004 blocked on native ARMv7 32-bit evidence)
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

Required first-release matrix: CPython 3.11, 3.12, 3.13, 3.14, 3.15 with `abi3-py311`, plus existing five native wheel families (Linux glibc x86_64/aarch64 at the demonstrated manylinux2014/2.17 floor, macOS x86_64/arm64, Windows x86_64) and **Linux ARMv7 hard-float** (`armv7-unknown-linux-gnueabihf` / PyPI `armv7l` tag). Execution targets `manylinux_2_28_armv7l` using the pinned cross image, then installs and tests in the official manylinux 2.31 ARMv7 image. Audit must confirm the actual tag and glibc floor; support is limited to ARMv7 hard-float and is not a generic all-Pi/all-ARM promise. Keep the ABI3 baseline; there is no need for per-CPython wheels for a given platform.

Additional desirable hosted-built wheels: `musllinux_1_2` for Linux x86_64/aarch64, optionally ARMv7 and i686. M002 determines which can be qualified without destabilizing the mandatory first-release matrix. Do not silently advertise a target lacking a wheel. If any required target cannot be built and installed under controlled emulation/native CI, record a **release blocker**, not an unqualified substitute or a misleading tag; seek an explicit scoped decision rather than silently dropping ARMv7.

The term *experimental SBC support* covers hardware-model/runtime/footprint uncertainty, not unvalidated ABI or broken wheel loading. CI must prove a correct, importable wheel under a matching target environment before it reaches PyPI.

## Dependency graph, milestones, and exits

| Milestone | Class | Status | Dependencies | Release-facing exit |
|---|---|---|---|---|
| M001 Python 3.15 and binding toolchain qualification | closed | hard: historical Python M008 closed | Closed at `1135608af5d625cd72db6704adf3bcabd1a58ab1`; exact-SHA qualification run `40729011504` passed all 25 CPython 3.11–3.15 cells across the five existing platform families. Closure: `plans/closure/python-pypi-distribution/001-status.md`. |
| M002 hosted ARMv7 and Linux wheel expansion | closed | interface: M001 keeps `abi3-py311` stable; operational integration before M003 | Closed at `1135608af5d625cd72db6704adf3bcabd1a58ab1`; hosted ARMv7 hard-float build, ELF audit and CPython 3.11/3.15 QEMU installs passed in run `40729011504`. No SBC compilation; optional musllinux remains deferred. Closure: `plans/closure/python-pypi-distribution/002-status.md`. |
| M003 controlled PyPI first publication | closed | interface: established M001/M002 artifact contracts; operational: M001/M002 qualification and exact candidate | Published `eggsearch==0.4.4` from candidate `1135608af5d625cd72db6704adf3bcabd1a58ab1`; workflow `40742653291` passed source/artifact verification, OIDC upload, and public binary-only installs on five native families plus emulated ARMv7. Closure: `plans/closure/python-pypi-distribution/003-status.md`. |
| M004 post-publication SBC benchmarking and tuning decision | blocked | operational: native ARMv7 32-bit runtime evidence | The published 0.4.4 aarch64 wheel passed native installed-wheel smoke and benchmark on a Libre Computer AML-S905X-CC. Full measurements are in `plans/closure/python-pypi-distribution/004-status.md` and `004-aarch64.json`. The maintainer deferred ARMv7 physical testing; keep that wheel experimental. |

M001–M003 are closed. The first public release was manually dispatched through the configured PyPI Trusted Publisher and pre-created GitHub `pypi` environment; the maintainer directed that no required reviewer be used. M004 is post-publication. Its native aarch64 sample is complete; ARMv7 32-bit physical benchmarking is deferred by the maintainer and remains the only M004 blocker. Keep ARMv7 support experimental until that evidence exists.

Implementation handoffs: `plans/implementation/python-pypi-distribution/001-python315-toolchain-and-abi3.md` through `004-post-publication-sbc-benchmarking.md`. Closure records go to `plans/closure/python-pypi-distribution/NNN-status.md`. Keep the registry aligned.

## Global verification and risks

Record exact full commit SHA, frozen version, wheel names/tags, hashes, host/emulator architecture, build/audit tools and CI run IDs. Run `make check`, `make packaging-check`, `cargo publish --dry-run --locked` when tree cleanliness permits, binding `pytest`/type checks, sdist extraction/rebuild, and actual installation from the *selected final* wheelhouse with `--only-binary=:all: --no-deps --no-index`. Runtime tests must cover all ten operations, configuration policy, no-network Mode-Off, lifecycle/cancellation and import without side effects. An experimental ARMv7 wheel must at least pass a release-installed smoke under emulation; full physical-SBC performance and long-duration soak are M004.

Risks: cross-ABI interpreter discovery, glibc floor/correct `armv7l` tagging, host runner/emulator availability, PyO3 ABI compatibility with 3.15, and upstream audit findings. Any incompatibility must be visible as a blocked requirement with the exact failing evidence. Python publication uses explicit manual workflow dispatch, OIDC Trusted Publishing, and immutable PyPI version handling; a failed upload cannot be overwritten.
