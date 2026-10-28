# M004 — Post-Publication SBC Benchmark Status

Status: blocked (operational: native ARMv7 32-bit runtime evidence deferred by maintainer)
Implementation plan: `plans/implementation/python-pypi-distribution/004-post-publication-sbc-benchmarking.md`
Roadmap: `plans/subsystems/python-pypi-distribution-roadmap.md` M004
Package: `eggsearch==0.4.4`
Qualified source: `1135608af5d625cd72db6704adf3bcabd1a58ab1`

The native aarch64 device run is complete. The maintainer directed that the
ARMv7 hardware line stop for now and that ARMv7 support remain experimental.
This is a blocked M004 status record, not closure: ARMv7 32-bit physical-device
measurements remain unrun.

## aarch64 device and artifact

| Property | Value |
|---|---|
| Device | Libre Computer AML-S905X-CC |
| Host | `i2p-20` |
| OS and kernel | Ubuntu 22.04.5 LTS; Linux `6.1.54-10001-gd517e36a604d` |
| Architecture | `aarch64`; native, not emulated |
| Python | CPython 3.11.17, GCC 11.4.0 |
| libc | glibc 2.35 |
| CPU count | 4 |
| RAM / swap | 943 MiB RAM / 1 GiB swap |
| Storage | 30 GiB filesystem; 15.9 GB free at measurement |
| Published wheel | `eggsearch-0.4.4-cp311-abi3-manylinux_2_17_aarch64.manylinux2014_aarch64.whl` |
| Public URL | `https://files.pythonhosted.org/packages/04/9b/30994abf253eb21b308978eeabd1f01e1c21496a0c39636faf5e0f7b4311/eggsearch-0.4.4-cp311-abi3-manylinux_2_17_aarch64.manylinux2014_aarch64.whl` |
| Wheel size / SHA-256 | 9,252,055 bytes / `840e8689a9ec52d26e480f3946e8a389ad0bdab816940681a1431b2faea52ccb` |
| Pip install report SHA-256 | `6e0bef6b5a82ad9965e95f61f83f6def5f73495e5fdc50095792db4636aaf819` |
| Benchmark JSON SHA-256 | `c975db2affa4ba8df789b049c996848333827dd7c2cb4527eb3de6bf062da9d7` |

The device's system Python had no pip or ensurepip, and noninteractive sudo
was unavailable. A temporary venv under `/tmp/eggsearch-m004-aarch64` used
`get-pip.py` with `pip==26.2.1`; no system Python or package files were changed.
The wheel was downloaded and installed with `--only-binary=:all: --no-deps`.
The benchmark verified the wheel digest against both the PyPI pip report and
the installed native extension. The separate `smoke_installed_wheel.py` smoke
passed and reported version `0.4.4`.

## Commands

```bash
python3.11 -m venv --without-pip /tmp/eggsearch-m004-aarch64/venv
/tmp/eggsearch-m004-aarch64/venv/bin/python /tmp/eggsearch-m004-aarch64/get-pip.py --no-cache-dir pip==26.2.1
/tmp/eggsearch-m004-aarch64/venv/bin/python -m pip download --only-binary=:all: --no-deps --dest /tmp/eggsearch-m004-aarch64/wheelhouse eggsearch==0.4.4
/tmp/eggsearch-m004-aarch64/venv/bin/python -m pip install --only-binary=:all: --no-deps --report /tmp/eggsearch-m004-aarch64/pip-report.json eggsearch==0.4.4
/tmp/eggsearch-m004-aarch64/venv/bin/python /tmp/eggsearch-m004-aarch64/smoke_installed_wheel.py
/tmp/eggsearch-m004-aarch64/venv/bin/python /tmp/eggsearch-m004-aarch64/sbc_runtime.py --version 0.4.4 --source-sha 1135608af5d625cd72db6704adf3bcabd1a58ab1 --wheel /tmp/eggsearch-m004-aarch64/wheelhouse/eggsearch-0.4.4-cp311-abi3-manylinux_2_17_aarch64.manylinux2014_aarch64.whl --pip-report /tmp/eggsearch-m004-aarch64/pip-report.json --output /tmp/eggsearch-m004-aarch64/sbc-runtime.json --repeats 20
```

The preserved JSON is `plans/closure/python-pypi-distribution/004-aarch64.json`.
Its full source, wheel, platform, memory, and thermal metadata stays with the
measurements.

## Results

Twenty repetitions were used except at concurrency 32, which ran 32 operations.
Search fixtures used local Mode-Off results; fetch used a loopback-only HTTP
fixture. No provider calls or source builds were run.

| Measurement | Median | p95 | Count / notes |
|---|---:|---:|---|
| Cold import | 293.78 ms | — | Fresh subprocess |
| First operation | 107.99 ms | — | Initializes shared Tokio runtime |
| Mode-Off repo search, concurrency 1 | 25.83 ms | 27.50 ms | 20 |
| Mode-Off repo search, concurrency 2 | 22.26 ms | 27.72 ms | 20 |
| Mode-Off repo search, concurrency 4 | 21.15 ms | 23.53 ms | 20 |
| Mode-Off repo search, concurrency 8 | 31.31 ms | 39.85 ms | 20 |
| Mode-Off repo search, concurrency 16 | 68.83 ms | 83.70 ms | 20 |
| Mode-Off repo search, concurrency 32 | 164.92 ms | 199.19 ms | 32 |
| Loopback fetch, 12,000 returned characters | 14.29 ms | 19.78 ms | 20; 49,489-byte tracemalloc peak |
| Cancel 32 tasks and drain | 8.49 ms | 21.76 ms | 20 |
| Sequential `asyncio.run` search loop | 122.58 ms | 136.50 ms | 20 |

Default and explicit Mode-Off client construction took 1.20 ms and 1.04 ms.
The heavy-output scenario serialized 90,968 bytes for 50 returned cards.
Post-cancellation recovery took 495.66 ms in its single sample; per-client close
took 1.06 ms. Aggregate process CPU was 7.08 user seconds and 2.97 system
seconds.

| Snapshot | RSS | PSS | Threads | Open file descriptors |
|---|---:|---:|---:|---:|
| Before client initialization | 28,940 kB | 25,259 kB | 1 | 7 |
| Runtime initialized | 38,892 kB | 35,034 kB | 6 | 13 |
| After concurrency 32 | 44,596 kB | 40,725 kB | 13 | 13 |
| After fetch | 45,980 kB | 42,034 kB | 13 | 13 |
| After client close | 47,716 kB | 43,770 kB | 13 | 13 |

Peak `ru_maxrss` was 86,812 KiB. Available memory at the system snapshot was
404,748 KiB. The thermal sensor reported 40 C; observed CPU frequencies were
500–667 MHz. The board did not expose thermal-throttle counters. The runtime
is process-scoped, so threads and memory remaining after client close are
expected by the library lifecycle contract.

This single-board, local-fixture run does not show a reason to change the
four-worker runtime or 32-call admission limit. It is not a live-provider,
long-duration, or full-memory stress test and does not establish performance
for other boards.

## ARMv7 residual

The supplied Raspberry Pi 3 Model B endpoint reports `aarch64`, runs 64-bit
Ubuntu 24.04, and has only 64-bit Python installed. It is not a native ARMv7
32-bit Linux test environment. The maintainer directed that no ARMv7 setup or
benchmarking continue now. The published `manylinux_2_28_armv7l` wheel therefore
remains **experimental**, with evidence limited to hosted artifact audit and
emulated installed-wheel smoke recorded in M002/M003. No ARMv7 compatibility,
performance, or resource claim is made from the aarch64 device.

M004 remains blocked only on native ARMv7 32-bit hardware/runtime evidence.
No optimization or patch release is recommended from this aarch64 sample.
