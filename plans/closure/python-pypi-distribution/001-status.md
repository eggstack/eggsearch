# M001 — Python 3.15, PyO3 Toolchain and Stable-ABI Qualification Closure

Status: closed
Candidate: `1135608af5d625cd72db6704adf3bcabd1a58ab1`
Implementation plan: `plans/implementation/python-pypi-distribution/001-python315-toolchain-and-abi3.md`
Roadmap: `plans/subsystems/python-pypi-distribution-roadmap.md` M001
Pull request: #10

## Candidate lineage

The candidate is the exact source SHA used for every qualifying check and every
artifact below. The work was implemented in `3c531920`, corrected for clean
hosted metadata inspection in `08769df`, and finalized in `1135608` with a
packaged Python license file and verifier assertion. No distributable source
changed after the qualifying SHA.

## Result

CPython 3.11–3.15 is supported through `cp311-abi3`. PyO3 0.30.1 and
`pyo3-async-runtimes` 0.30.0 build on Rust 1.89; Maturin is pinned to 1.15.0.
The package metadata advertises `>=3.11,<3.16`, includes the Python 3.15
classifier, and does not claim PyPy, free-threaded Python, or Python 3.10.
The five existing native platform families each have one ABI3 wheel. All five
interpreters installed and exercised the matching wheels in clean consumer
environments.

The binding package now declares `LICENSE-PYTHON`; its bytes match the root
MIT license. Maturin places it under `.dist-info/licenses/` and records it in
wheel metadata. The unique filename avoids an sdist path collision with the
root `LICENSE`.

## Exact-SHA verification

| Check | Result |
|---|---|
| Python native qualification, run `40729011504` | **34/34 jobs passed**: six platform wheel builds including ARMv7, dependency audit, extracted-sdist offline rebuild, ARMv7 emulated install, and all 25 CPython 3.11–3.15 clean installed-wheel smokes across the five native platform families. |
| Root CI, run `40729011529` | `ci` and Windows portability passed; canonical `make check` gate green. |
| Release drift guard, run `40729011257` | Eggpack workflow drift, release contract and pinned action checks passed. |
| Egress qualification, run `40729042102` | All target checks passed, including macOS ARM64 and Rust 1.89 all-features. |
| Artifact verifier | Passed against the complete hosted artifact bundle with exact source SHA and qualification run ID. It accepted exactly six wheels, one sdist, and one ARMv7 audit record. |
| `cargo +1.89.0 publish --dry-run --locked` | Passed on the clean candidate. The existing crates.io 0.4.4 warning is expected; no upload occurred. Cargo packaged 290 files. |
| Binding dependency audit | Both Cargo graphs passed `cargo deny`; pinned Python `pip-audit --local` reported no known vulnerabilities. |
| Local package checks | Rust 1.89 binding check, local Maturin wheel build and sdist generation passed; the generated wheel contained `LICENSE-PYTHON` and the sdist preserved both root and Python license paths. |

The root Cargo dependency graph and Eggpack asset contract are unchanged by the
Python binding work. No new Python runtime dependency was added.

## Hosted release artifact digests

These SHA-256 values are from qualification run `40729011504`. The verifier
manifest was generated from the downloaded artifacts at
`/tmp/eggsearch-qualification-1135608/release-manifest.json`.

| Artifact | SHA-256 |
|---|---|
| `eggsearch-0.4.4-cp311-abi3-manylinux_2_17_x86_64.manylinux2014_x86_64.whl` | `96247b088c2ae924e49c459c5a29b7b9ac348c776a9c928e1eaafc09f032a4e0` |
| `eggsearch-0.4.4-cp311-abi3-manylinux_2_17_aarch64.manylinux2014_aarch64.whl` | `840e8689a9ec52d26e480f3946e8a389ad0bdab816940681a1431b2faea52ccb` |
| `eggsearch-0.4.4-cp311-abi3-macosx_10_12_x86_64.whl` | `cb5dd563ee71f1aa8844c4c454effad17c1838ce315c01f14776dbf2733db7ad` |
| `eggsearch-0.4.4-cp311-abi3-macosx_11_0_arm64.whl` | `adaadee8bd530f302b642a48886ca723f19c245c5249e107aa885a0cfe78349e` |
| `eggsearch-0.4.4-cp311-abi3-win_amd64.whl` | `62bedfea95581d906e5f4749310c2013a277754e05f177d93d4f08a271bc8340` |
| `eggsearch-0.4.4-cp311-abi3-manylinux_2_28_armv7l.whl` | `530c31f51018886caaa70659e037ed6ceff5a4d053f762ebef46f92c8ec82eb0` |
| `eggsearch-0.4.4.tar.gz` | `42e57e1c9a4ec02b97d715cc28f522e7effde2e1cb9d0ca43ee0fecaac7ae436` |

ARMv7 audit record SHA-256:
`7d574ce6c34f7874666b8525cca70ab145bd5d9a4bc0db2533f7f58ea34f5f76`.

## Scope and handoff

M001 is closed. No publication was performed. M003 remains the controlled
publication gate and consumes candidate `1135608`; it requires a PyPI Trusted
Publisher plus the GitHub `pypi` environment approval configuration. M002's
ARMv7 evidence is separately recorded in `002-status.md`.
