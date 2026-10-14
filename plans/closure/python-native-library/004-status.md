# Python Native Library M004 — Wheel, Sdist and Release Qualification Closure

Status: closed
Candidate: `faa1da2e5d5a8c38045001fec1ae1a230bc36bde`
Implementation plan: `plans/implementation/python-native-library/004-wheel-sdist-and-release-qualification.md`
Qualification plan: `plans/implementation/python-native-library/006-m004-exact-sha-distribution-closure.md`
Corrective: `plans/implementation/python-native-library/007-windows-safe-read-corrective.md`, closed in `plans/closure/python-native-library/007-status.md`
Predecessors: `001-status.md` (M001), `002-status.md` (M002), `003-status.md` (M003), `005-status.md` (M005)
Roadmap: `plans/subsystems/python-native-library-roadmap.md` M004

## Candidate lineage

Baseline `1434c1e55ddd92947042cff9918ffc0fe130ed01` carried a green
`Python native qualification` run `38522548258` (27 jobs) but no closure
record, so M004 stayed active and that run is not used as evidence for
these bytes. M005 changed the candidate (lifecycle gate, default-config
API, 11 new Python tests) and closed at `baf00cf`. M006 qualification of
`c8bd030` (run `38608502185`) and `748ae59` (run `38611932990`, plus
instrumented run `38614592873`) exposed first a Windows-only test-isolation
defect (fixed test-only in `748ae59`) and then a Windows-only product
defect in `safe_open_relative` (fixed in M007). All pre-fix runs are
deficiency evidence only. Every artifact below is recorded against the
M007 repair candidate `faa1da2`.

Root/binding versions on the candidate: `eggsearch 0.4.3`, binding crate
`eggsearch-python 0.4.3`, distribution `eggsearch 0.4.3` — all three
synchronized. Pinned toolchain: Rust `1.89.0`, PyO3 `=0.29.3` with
`abi3-py311`, `pyo3-async-runtimes =0.29.0`, maturin `==1.9.5`,
`requires-python >=3.11,<3.15`. `Cargo.lock`, `bindings/python/Cargo.lock`,
manifests, and workflows are byte-identical to the green baseline
`1434c1e`. No git tag was created; no publication was performed or
authorized.

## Requirement-to-evidence matrix

| M004 requirement (004 plan) | Evidence on `faa1da2` |
|---|---|
| Independently versioned `bindings/python` packaging, `abi3-py311` wheels | pyproject + binding Cargo.toml at 0.4.3; all wheels `cp311-abi3`; workflow asserts name/version/license parity across pyproject, binding, and root crates on every build job |
| Five native build targets (Linux x86_64/aarch64 manylinux2014, macOS x86_64/arm64, Windows x86_64) | Run `38617598636`, jobs `Build {target} CPython 3.11`: 5/5 success |
| Installed-wheel smoke on CPython 3.11–3.14 on each target, no Rust toolchain in consumer venvs | Run `38617598636`, 20 `Smoke` jobs, each in a fresh venv installing `--no-index --no-deps --only-binary=:all:` then running the full `bindings/python/tests` suite: 20/20 success |
| Smoke exercises import, canonical inventory, ten operations, concurrency/cancellation/close, trust/Mode-Off | Suite asserts 10 tool names, ten typed request paths, 32-permit saturation/recovery, cancel/close taxonomy, `local_trusted` Mode-Off flow with content snippets, config selection; 23 pass + 5 debug-gate skips per smoke job on release wheels |
| Self-contained sdist, clean extracted rebuild offline, clean-install consumer run | Run `38617598636` job `Rebuild and smoke extracted sdist`: success; local replication below confirms `CARGO_NET_OFFLINE=true` rebuild plus clean-venv install and suite |
| Wheel/sdist digests durable (not only 14-day artifact links) | SHA-256 table below, recorded from build logs and local builds |
| Dependency/license/vulnerability scanning of the binding graph and build actions | `cargo deny check` executed with findings recorded (see R2); workflow-pinned pip audit env clean; dev-pin pytest advisory recorded (see R3); all actions SHA-pinned |
| Root Cargo package and Eggpack release inputs unchanged | `cargo package --list` (291 entries) and `cargo publish --dry-run --locked` pass; `make packaging-check` passes; no wheel in the seven-target binary assets; release workflow still Eggpack-owned |
| Python install/support matrix documented, PyPI availability conditional | `docs/python-library.md` + `bindings/python/PYTHON_README.md`; availability stays conditional (see Publication) |
| No publication without separate authorization | `Published to PyPI: no` |

## Hosted qualification (exact candidate)

Workflow: `.github/workflows/python-native-qualification.yml` on PR
`plan/python-native-closure-correctives` at `faa1da2`.
Run: `38617598636` (`pull_request`, head SHA `faa1da2`). Overall run
conclusion is `failure` solely from the audit job (upstream rustls
findings, see R2); all 26 artifact jobs succeed.

| Job | Conclusion |
|---|---|
| Build x86_64-unknown-linux-gnu CPython 3.11 (`116036587311`) | success |
| Build aarch64-unknown-linux-gnu CPython 3.11 (`116036587389`) | success |
| Build x86_64-apple-darwin CPython 3.11 (`116036587454`) | success |
| Build aarch64-apple-darwin CPython 3.11 (`116036587284`) | success |
| Build x86_64-pc-windows-msvc CPython 3.11 (`116036587301`) | success |
| Smoke, all five targets × CPython 3.11/3.12/3.13/3.14 (20 jobs) | 20/20 success |
| Rebuild and smoke extracted sdist (`116036587122`) | success |
| Audit binding dependency graph (`116026305625`) | failure on upstream rustls advisories (R2); `cargo deny` bans/licenses/sources pass |

Wheel digests (SHA-256, from build logs):

- `eggsearch-0.4.3-cp311-abi3-manylinux_2_17_x86_64.manylinux2014_x86_64.whl`: `6cb86260d77cbd1e34685638436b00e349cfaedbf0d71aaf56493990addc88f1`
- `eggsearch-0.4.3-cp311-abi3-manylinux_2_17_aarch64.manylinux2014_aarch64.whl`: `8320a24e3d56f2075d77f43d77e6da8be7bc3801dd7ffe28e1661e3e701a03d3`
- `eggsearch-0.4.3-cp311-abi3-macosx_10_12_x86_64.whl`: `09e0a519d3a8b29010bedd2b346b6c06a75b3ea40bd04cad73cfb84716938442`
- `eggsearch-0.4.3-cp311-abi3-macosx_11_0_arm64.whl`: `46e27010bfb4cd75eab508d50b1f96a08776d835101dc902b8cd5484ebfce3dd`
- `eggsearch-0.4.3-cp311-abi3-win_amd64.whl`: `c009724b01bbcb6c113acfc7ab70bf12e9688c85047f3a8dcec11c1c2df3f76a`

Sdist and extracted rebuild (job `116036587122`, ubuntu-24.04):

- `eggsearch-0.4.3.tar.gz`: `c3b32d30d3d8b212918527fa35e3e3916784c4214290e51a90eb8cad67ebad36`
- Offline extracted rebuild (`CARGO_NET_OFFLINE=true`, prefetched locked crates) wheel `eggsearch-0.4.3-cp311-abi3-manylinux_2_39_x86_64.whl`: `c174a37a81813ee759bcb00cdbeea979d51642b97a24651b53b673ba47dca0ba`
- Clean-venv install of the extracted wheel: `23 passed, 5 skipped`, 10 tools, distribution version 0.4.3 import assert green.

Windows proof detail: all four Windows smoke jobs report `23 passed, 5 skipped`, including `test_explicit_path_overrides_default_config` with content-snippet asserts — the exact asserts that failed pre-fix. The 5 skips are the debug-only barrier tests, absent from release wheels by design.

## Local replication on the exact candidate

Working tree clean at `faa1da2`; macOS arm64 host, Rust 1.89.0, maturin
1.9.5, CPython 3.14.2.

| Check | Result |
|---|---|
| `maturin build --manifest-path bindings/python/Cargo.toml --release --sdist --locked` | sdist `6fe2ea6529f2f65d1ceb66f151c7b51dad174f74206362b51482b3a1168ad469` (305 entries: binding crate, root sources, both lockfiles, license, readmes, stubs); local `cp311-abi3-macosx_11_0_arm64` wheel `114f5fbfb49b8ef80997b334694afe2f113a5f4f5a007435d2872ea33e1310b5`; no `__pycache__`/`.pyc`/secrets; 8 files with `_native.abi3.so`, no bundled executable |
| Extract sdist outside checkout, `CARGO_NET_OFFLINE=true` rebuild | Succeeds; extracted wheel `cf5ffcce7134339fae0b6d54f58696c3c541a8ccfd9e90f447abaf3c5f5836da` |
| Clean-venv consumer run of extracted wheel (`arch -arm64`) | 23 passed, 5 skipped; 10 tools; version 0.4.3 |
| `python -m pytest -q bindings/python/tests` (develop, debug) | 28 passed |
| `python -m compileall`, `pyright` | Clean, 0 errors |
| `make check` | fmt, clippy, no-default check, full all-features tests (incl. new `safe_open_root_must_be_directory`), hygiene pass; stops at `dependency-policy` on upstream rustls advisories (R2) |
| `make packaging-check` | Pass (workflow pins, planning consistency, Eggpack qualifier, egress contract) |
| `cargo package --list --locked` / `cargo publish --dry-run --locked` | 291 entries; dry-run upload aborts cleanly, no drift from binding work |
| `./packaging/release-validate.sh candidate`, `cargo doc --locked --all-features` | Pass |
| Workflow-equivalent pip audit env (`pip==26.2.1`, `pip-audit==2.10.1`, `maturin==1.9.5`) | No known vulnerabilities found |

## Root compatibility and release boundary

- `cargo package --list --locked` output unchanged in kind (291 files, root crate only); binding crate `publish = false` so it cannot leak into the root package.
- `cargo publish --dry-run --locked` succeeds; the M007 repair touches one non-unix `cfg` branch plus tests and changes no dependency, version, or packaging input.
- `make packaging-check` green: Python wheels are not in `packaging/release-inputs.txt`, `expected_release_assets.py`, installers, updater, or install docs. The canonical seven-target Eggpack binary asset set is untouched.
- Main `CI` on the PR at `faa1da2` (run `38617598587`): windows-portability check green; `ci` job red only at `dependency-policy` on the same upstream rustls advisories (environmental — manifests/lockfiles identical to green baseline `1434c1e`).
- `Release drift guard` on `faa1da2` (run `38617598774`): success. `Egress feature qualification` on `faa1da2` (run `38617598641`): success.

## Publication

`Published to PyPI: no`. No TestPyPI upload, no tag, no immutable-release
edit. Any future publication needs its own authorization and a fresh
exact-SHA re-qualification of whatever bytes it ships.

## Unsupported targets (explicit)

Not qualified and not claimed: PyPy, CPython free-threaded builds, Windows
ARM64, ARMv7, CPython <3.11 or >=3.15, mandatory source builds on
wheel-supported hosts. Linux aarch64 evidence is native runner execution
(`ubuntu-24.04-arm`), not SBC hardware. Cross-compilation alone was never
counted as native qualification.

## Residuals

- R1 (fixed during this closure): pre-fix runs `38608502185` (3 Windows
  failures: shared-CWD config files plus a Unix-only filename assert) and
  `38611932990`/`38614592873` (1 Windows failure: missing local snippets)
  are deficiency evidence. Fixes: per-test CWD isolation plus fallback
  filename tolerance (test-only, `748ae59`), then the M007 product repair.
  Lesson retained: platform-sensitive tests should see the hosted matrix
  before the owning plan closes.
- R2 (Medium, owned by the repository dependency process): `cargo deny
  check` for the binding graph and root `dependency-policy` fail on
  upstream rustls advisories RUSTSEC-2026-0346/0347/0348/0349 against
  locked rustls 0.23.45 (via `eggfetch-core` 0.2.2). Grown from three to
  four advisories since M005 with no workstream change (lockfiles and
  manifests identical to green baseline `1434c1e`). The suggested
  `cargo update -p rustls` belongs to the dependency process, not to a
  distribution-qualification pass. Carried from M005, re-observed on
  `faa1da2` in the hosted audit job, local `make check`, and main `CI`.
- R3 (Low, test-only): `pip-audit` flags dev-pin `pytest==8.4.2` with
  PYSEC-2026-1845 (fix 9.0.3). pytest ships in no wheel, is absent from
  the workflow audit env (which reports clean), and runs only in ephemeral
  CI/dev. Pin intentionally unchanged to avoid re-qualifying the matrix on
  a new harness version; revisit with the next harness bump.
- Informational: hosted artifacts expire after 14 days; the SHA-256 tables
  above are the durable record. Debug-gate barrier tests (5) are absent
  from release wheels by design; SDK-side barrier tests ship in all
  builds. The non-unix `safe_open` component walk still follows
  per-component symlinks where the unix branch rejects them
  (pre-existing, unchanged by M007, noted for the hardening workstream).

## Handoff

M004 is closed on `faa1da2`. The workstream remains unpublished by design.
M006 reconciliation lives in `plans/closure/python-native-library/006-status.md`;
the M007 repair record lives in `plans/closure/python-native-library/007-status.md`.
Any future wheel rebuild from different bytes must re-qualify at its own
exact SHA; run `38617598636` is evidence for `faa1da2` only.
