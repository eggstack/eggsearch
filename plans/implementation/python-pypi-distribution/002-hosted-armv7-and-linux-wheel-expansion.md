# M002 — Hosted ARMv7 and Broad Linux Prebuilt Wheels, Experimental SBC Support

Status: active
Primary class: infrastructure
Source: `plans/subsystems/python-pypi-distribution-roadmap.md` M002.
Execution baseline refreshed: branch `plan/python-pypi-prebuilt-arm-release-20261028` at `62db213179100109e532d9be43a81b1df054dcde`, root/binding 0.4.4.
Interface dependency: M001 preserves CPython `abi3-py311` and the Rust 1.89 SDK; implement build support in parallel, then integrate exact candidate for M003. Operational dependency: first PyPI release needs passing required wheel qualifications.
Authority: ADR-0005, `plans/003-planning-process.md`, Python M004/M008 historical closure, and `architecture/python-library.md`.

## Objective and baseline

Produce an **installable precompiled ARMv7 hard-float wheel** for SBC-class 32-bit Linux without ever compiling the package on a Raspberry Pi, Le Potato, or other resource-constrained board. Expand Linux platform coverage with additional prebuilt wheels when qualified. Document SBC support as **experimental** until post-publication physical-device benchmarking. Existing Python workflow builds `manylinux2014` x86_64/aarch64, macOS Intel/ARM64 and Windows x86_64, all stable ABI; no ARMv7/musllinux wheels today.

Execution selects `ghcr.io/rust-cross/manylinux_2_28-cross@sha256:b1f069c62f7744faa6db5f1ab28b11d59f72e161d8f03f6b0a388fe1cecc065d` and requires the generated wheel to retain its `manylinux_2_28_armv7l` tag after Maturin's audit. This is a broader glibc compatibility floor than the original 2.31 candidate. The independent install smoke runs in `quay.io/pypa/manylinux_2_31_armv7l@sha256:ff4a72812594311c071eebce43efcaa9061bd1205649d864c1054d4e56e6bdae` under QEMU. The pinned runtime image config declares Linux ARM. If either image or tag behavior differs, stop and update the decision with observed evidence before publication.

The workflow now has a separate ARMv7 cross-build artifact, a `readelf` audit of ELF class, EABI, hard-float attributes, dynamic dependencies and maximum GLIBC symbol version, and QEMU installed-wheel jobs under CPython 3.11 and 3.15. The release verifier requires the six mandatory platform families and checks that the audit record accompanies the exact qualification artifacts. The image digests were checked against their registries on 2026-10-28. Hosted build/audit/emulated install evidence remains pending; no physical SBC build or benchmark has been performed.

Upstream PyPA `manylinux_2_31_armv7l` image (`quay.io/pypa/manylinux_2_31_armv7l`) supports a Linux glibc 2.31 baseline and the `armv7l` platform tag. PyPA `musllinux_1_2` images exist for x86_64/aarch64/armv7l. These are *build infrastructure candidates*, not proof that the project or its Rust dependencies link on ARMv7. Choose an audited and reproducible CI recipe (hosted QEMU/binfmt running the proper image, or true cross-compile plus QEMU consumer smoke), with all command details and image digests recorded.

## Mandatory and optional targets

Mandatory for first publication: keep the five existing wheel families and add `armv7-unknown-linux-gnueabihf` producing a Python `cp311-abi3-manylinux_2_28_armv7l`-compatible wheel, with exact tag determined by audit. Its floor is glibc 2.28, **not** manylinux2014 and not all 32-bit Linux; does not include ARMv6, soft-float, Android, or Alpine/musl. The independent install test runs under the official manylinux 2.31 ARMv7 userland. Existing Linux aarch64 covers 64-bit SBC systems only if distribution/libc baseline matches.

Preferred additional builds: `musllinux_1_2_x86_64`, `musllinux_1_2_aarch64`. Optional/conditionally deferred: `musllinux_1_2_armv7l`, `manylinux2014_i686`. Record explicit decision/evidence for any supplemental platform deferred so the release documentation names only actual wheels; inability to add optional targets must not delay a green mandatory release.

Execution decision: defer both preferred musllinux wheels from the first inventory. The release verifier and consumer workflow are scoped to the six mandatory platform families; no musllinux compatibility is claimed. This keeps the first publication focused on the required CPython ABI matrix and ARMv7 hard-float gate. Revisit musllinux in a separately qualified expansion after M003 if hosted builds and matching consumer environments are available.

## Non-goals / invariants

**No compilation, Cargo, C linker, Rust installation, wheel rebuild, or `pip install` source fallback on any physical SBC before release.** Physical SBC operation and memory/performance experiments are M004 **after publication**. Do not add new runtime dependencies or change the root Eggpack ARMv7 executable pipeline. Preserve `abi3-py311`, bounded tool semantics, credentials/config and security; never turn off manylinux audit, retag a `linux_*` wheel as manylinux, or label a 64-bit wheel as 32-bit ARM.

## Ordered work packages

1. Derive an explicit platform grid from PyPI compatibility tags: `cp311-abi3`, `armv7l`, manylinux glibc floor, musllinux versus glibc, Python 3.11–3.15. Inspect `eggfetch-core`/TLS/crypto dependency target support and CPU instructions, Rust 1.89 ARMv7 hard-float toolchain/linker availability, and the upstream images' architecture. Document minimum supported OS/toolchain, avoid assumptions about Pi model.
2. Implement the ARMv7 job in `.github/workflows/python-native-qualification.yml` with hosted CI (e.g., x86_64 host + QEMU static/binfmt + pinned manylinux ARMv7 image, native ARM64 if it can reliably execute AArch32, or auditable cross compiler). Pin actions and container digests/versions. Build *once* with `--release --locked`; inspect Maturin audit/tag result, ELF class, ARM EABI hard-float, dynamic deps, required GLIBC symbols, correct `_native.abi3.so`, importable Python package and no CLI executable in wheel. A cross-compile alone is insufficient.
3. Download the **exact generated wheel** into an ARMv7 userland under QEMU and perform a binary-only install without Cargo or compiler. Verify `platform.machine()`, `sys.version_info`, `importlib.metadata`, `eggsearch.tool_names()`, local Mode-Off `repo_search`/source evidence, representative ten-operation semantics/errors, bounded cancellation/close and no import network/process side effects. Test at minimum CPython 3.11 and 3.15; when practical use 3.12–3.14 also, because the stable ABI is otherwise shared. Log emulator and distro details so this evidence is not mistaken for physical Pi qualification.
4. Add `musllinux_1_2` x86_64/aarch64 jobs with comparable hosted build/audit/import tests, and ARMv7 musl/i686 only if time/toolchain permits. Use a musl userland for musl smokes, not a glibc container. Verify absence of forbidden glibc deps and ensure compatibility tags match published platform. Do not silently substitute one libc family for another.
5. Add an artifact verifier that rejects wrong arch/libc/ABI tags, missing mandatory wheel, duplicate wheel platform tags, sdist contamination, root executable bundling, and wheel whose contents are not installable; verify `--only-binary=:all:` from exact candidate wheelhouse. Add deterministic Python/Rust guard tests if practical.
6. Update `docs/python-library.md`, `bindings/python/PYTHON_README.md`, root README's short Python link and support matrix only as appropriate. Clearly label ARM/SBC support experimental. State example SBC install `python -m pip install --only-binary=:all: eggsearch`; explain pip platform checks and that an incompatible host should **fail without attempting a local source build**. State explicitly that `pip install eggsearch` without `--only-binary` may attempt an sdist build on unsupported platforms, which is **not recommended on SBCs**. Disclose glibc/musl/ARMv7 vs ARM64 distinctions and post-release test plan.

## Verification and release boundary

Run `make check`, `make packaging-check`, binding sdist and audit checks, plus workflow ARMv7 compile-and-ELF audit, **emulated installed-wheel smoke** and architecture/libc tag assertions. Reuse existing 5-platform × 3.11–3.15 qualification from M001 and pin the exact tested candidate in logs. Record SHA-256 for every wheel, source SHA, CPU/emulation method, interpreter versions, wheel platform tag, libc floor, and audit output. The post-publication M004 tests are *not* part of this matrix.

Stop rather than ship if ARMv7 wheel is not importable under matching emulation, contains incorrect hard-float ABI/GLIBC requirements, is falsely tagged, or the package's security policy breaks. Do not replace mandated ARMv7 with an unproven cross-compiled artifact. If unavoidable, register a corrective and obtain an explicit scoped decision before moving publication forward. Optional musllinux/i686 failures can be documented and deferred.

## Closure evidence

`plans/closure/python-pypi-distribution/002-status.md` records mandatory ARMv7 build/install/audit success and exact SHA/digest; each supplemental target as qualified or explicitly deferred; native vs emulator evidence; interpreter and libc floors; runtime fixture outcomes; source builds on SBC explicitly **not performed**; remaining hardware-model uncertainty classified *experimental*. Update roadmap and registry together when closing.
