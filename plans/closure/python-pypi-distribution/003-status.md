# M003 — First PyPI Publication and Public Wheel Provenance Closure

Status: closed
Candidate source: `1135608af5d625cd72db6704adf3bcabd1a58ab1`
Package: `eggsearch==0.4.4`
Implementation plan: `plans/implementation/python-pypi-distribution/003-first-pypi-release-and-provenance.md`
Roadmap: `plans/subsystems/python-pypi-distribution-roadmap.md` M003
Publication workflow run: `40742653291`
Merged workflow commit: `2ac49f6e945f3d940218587f75c18a9c573abe9d` (PR #10)

## Result

The first Python distribution was published to [PyPI](https://pypi.org/project/eggsearch/0.4.4/) on 2026-10-28. The public project metadata reports name `eggsearch`, version `0.4.4`, and `Requires-Python >=3.11,<3.16`. PyPI serves exactly six ABI3 wheels and one source distribution. The PyPI SHA-256 digests match qualification run `40729011504` and the verified release manifest.

The maintainer confirmed the PyPI Trusted Publisher and GitHub `pypi` environment were configured and directed that no required reviewer be used. The explicitly dispatched workflow verified that the environment existed, checked the exact qualified source and artifact inventory, and published with PyPI Trusted Publishing. The verification job, publication job, and every post-publication consumer job in run `40742653291` succeeded. No PyPI token or secret was used.

## Requirement-to-evidence matrix

| Requirement | Evidence | Result |
|---|---|---|
| Exact qualified source and synchronized version | Candidate `1135608af5d625cd72db6704adf3bcabd1a58ab1`; root, binding, and project manifests all report 0.4.4; workflow run `40742653291` checked candidate CI and Python qualification run `40729011504`. | Passed |
| M001/M002 exact-SHA qualification | Python native run `40729011504` passed all 34 jobs; root CI `40729011529`, release drift `40729011257`, and egress qualification `40729042102` passed. Closure records `001-status.md` and `002-status.md` hold the qualification matrix, audits, sdist rebuild, and ARMv7 ELF evidence. | Passed |
| Immutable release inventory | Workflow run `40742653291`, `Verify qualified source and artifact inventory` job; `verify_release_artifacts.py` accepted the six wheels, sdist, manifest, source SHA, qualification ID, and ARMv7 audit before publication. | Passed |
| Trusted publication and environment | Workflow run `40742653291`, `Manually dispatched PyPI publication` job succeeded using the configured OIDC publisher and pre-created `pypi` environment. No environment reviewer was configured, as explicitly directed by the maintainer; the manual workflow dispatch was the publication action. | Passed |
| Public package metadata and inventory | [PyPI 0.4.4 JSON](https://pypi.org/pypi/eggsearch/0.4.4/json), fetched after upload: expected name/version, Python range, six wheels, and one sdist. Public file digests and sizes are listed below. | Passed |
| Public binary-only consumer installation | Workflow run `40742653291` passed fresh installs with `--only-binary=:all: --no-deps` and `smoke_installed_wheel.py` on Linux x86_64, Linux aarch64, macOS x86_64, macOS arm64, and Windows x86_64 using CPython 3.15, plus emulated ARMv7 manylinux 2.31 using CPython 3.11. The smoke checks ten tools, malformed-input error handling, trusted local Mode-Off evidence, and closed-client behavior. Job logs record each public wheel URL and SHA-256; each matches the public PyPI JSON. | Passed |
| Consumer installation requirements | Consumers installed wheels directly from `files.pythonhosted.org`; no source build step ran, and pip was constrained to binary distributions. PyPI `Requires-Python` excludes unsupported interpreter versions. | Passed |
| Documentation and governance | README, Python library guide, binding README, architecture, release guide/checklist, roadmap, registry, and this closure record describe the published package and separate PyPI release path. | Passed |

The consumer workflow also writes `pip debug --verbose` supported tags to each job summary. Native ARMv7 hardware has not been tested; ARMv7 remains explicitly experimental, with evidence limited to the published hard-float wheel and emulated installed-wheel smoke.

## Published artifact inventory

All sizes are bytes; SHA-256 values are from the public PyPI JSON response and were checked against the qualification manifest.

| PyPI artifact | Size | SHA-256 |
|---|---:|---|
| `eggsearch-0.4.4-cp311-abi3-manylinux_2_17_x86_64.manylinux2014_x86_64.whl` | 9,461,830 | `96247b088c2ae924e49c459c5a29b7b9ac348c776a9c928e1eaafc09f032a4e0` |
| `eggsearch-0.4.4-cp311-abi3-manylinux_2_17_aarch64.manylinux2014_aarch64.whl` | 9,252,055 | `840e8689a9ec52d26e480f3946e8a389ad0bdab816940681a1431b2faea52ccb` |
| `eggsearch-0.4.4-cp311-abi3-macosx_10_12_x86_64.whl` | 8,727,305 | `cb5dd563ee71f1aa8844c4c454effad17c1838ce315c01f14776dbf2733db7ad` |
| `eggsearch-0.4.4-cp311-abi3-macosx_11_0_arm64.whl` | 8,498,057 | `adaadee8bd530f302b642a48886ca723f19c245c5249e107aa885a0cfe78349e` |
| `eggsearch-0.4.4-cp311-abi3-win_amd64.whl` | 8,327,031 | `62bedfea95581d906e5f4749310c2013a277754e05f177d93d4f08a271bc8340` |
| `eggsearch-0.4.4-cp311-abi3-manylinux_2_28_armv7l.whl` | 8,938,271 | `530c31f51018886caaa70659e037ed6ceff5a4d053f762ebef46f92c8ec82eb0` |
| `eggsearch-0.4.4.tar.gz` | 1,299,743 | `42e57e1c9a4ec02b97d715cc28f522e7effde2e1cb9d0ca43ee0fecaac7ae436` |

Public native-consumer jobs reported these wheel identities:

| Consumer | Public wheel SHA-256 | Result |
|---|---|---|
| Linux x86_64, CPython 3.15 | `96247b088c2ae924e49c459c5a29b7b9ac348c776a9c928e1eaafc09f032a4e0` | Passed |
| Linux aarch64, CPython 3.15 | `840e8689a9ec52d26e480f3946e8a389ad0bdab816940681a1431b2faea52ccb` | Passed |
| macOS x86_64, CPython 3.15 | `cb5dd563ee71f1aa8844c4c454effad17c1838ce315c01f14776dbf2733db7ad` | Passed |
| macOS arm64, CPython 3.15 | `adaadee8bd530f302b642a48886ca723f19c245c5249e107aa885a0cfe78349e` | Passed |
| Windows x86_64, CPython 3.15 | `62bedfea95581d906e5f4749310c2013a277754e05f177d93d4f08a271bc8340` | Passed |
| Emulated ARMv7 manylinux 2.31, CPython 3.11 | `530c31f51018886caaa70659e037ed6ceff5a4d053f762ebef46f92c8ec82eb0` | Passed; emulated |

## Residuals and handoff

The sdist is published for developer environments outside the wheel matrix; SBC instructions require `--only-binary=:all:` and do not permit on-device compilation. ARMv7 remains experimental because no physical board has been measured. M004 owns native aarch64/ARMv7 runtime benchmarking and is blocked only on access to physical SBC devices. No benchmark or optimization was performed before publication.

M003 is closed. M004 is recorded separately in `004-status.md` once device evidence or an explicit hardware-access blocker is available.
