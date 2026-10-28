# M002 — Hosted ARMv7 and Linux Wheel Expansion Closure

Status: closed
Candidate: `1135608af5d625cd72db6704adf3bcabd1a58ab1`
Implementation plan: `plans/implementation/python-pypi-distribution/002-hosted-armv7-and-linux-wheel-expansion.md`
Roadmap: `plans/subsystems/python-pypi-distribution-roadmap.md` M002
Qualification run: `40729011504`

## Mandatory ARMv7 artifact

The hosted cross-build produced
`eggsearch-0.4.4-cp311-abi3-manylinux_2_28_armv7l.whl` with SHA-256
`530c31f51018886caaa70659e037ed6ceff5a4d053f762ebef46f92c8ec82eb0`.
The pinned `readelf` audit reported:

- ELF32, ARM EABI hard-float, `manylinux_2_28_armv7l` tag.
- Maximum GLIBC symbol version 2.28.
- Dynamic dependencies: `ld-linux-armhf.so.3`, `libc.so.6`, `libdl.so.2`,
  `libgcc_s.so.1`, `libm.so.6`, `libpthread.so.0`, and `librt.so.1`.
- No bundled Eggsearch executable; the wheel contains the ABI3 extension,
  Python package and stubs, metadata, license and SBOM.

The exact hosted wheel was installed binary-only under QEMU in the pinned
PyPA manylinux 2.31 ARMv7 image on CPython 3.11.18 and 3.15.2. Each run passed
`23` tests with `6` expected skips, plus the installed-wheel ten-tool smoke.
Both reported `platform.machine() == "armv7l"` and Eggsearch 0.4.4. The pytest
cache emitted one warning because the checked-out source was mounted read-only;
the test results were successful.

## Linux coverage and artifact inventory

The existing manylinux 2.17 x86_64 and aarch64 wheels, macOS x86_64 and arm64
wheels, and Windows x86_64 wheel all built and passed the M001 25-job
CPython/interpreter matrix. The release artifact verifier passed on the exact
six-wheel-plus-sdist bundle. All artifact hashes are recorded in
`001-status.md`.

Optional `musllinux_1_2` x86_64/aarch64 and conditional ARMv7/i686 wheels are
deferred; this release makes no musl or i686 compatibility claim.

## Limits and handoff

No physical SBC was compiled on or benchmarked. ARMv7 support is ABI- and
emulation-qualified, while hardware-model, memory and performance uncertainty
remains experimental. M004 begins only after first PyPI publication and access
to native ARMv7/aarch64 devices. M002 is closed with the hosted evidence above.
