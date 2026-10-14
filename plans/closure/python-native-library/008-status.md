# Python Native Library M008 — Advisory Remediation and Symlink-Containment Closure

Status: closed
Candidate: `067468a26c21716dd1063d67679768c6d794a2b9`
Implementation plan: `plans/implementation/python-native-library/008-rustls-and-symlink-containment-requalification.md`
Roadmap: `plans/subsystems/python-native-library-roadmap.md` M008

## Candidate lineage

All three commits are on `plan/python-native-closure-correctives`, PR #7,
working tree clean at the candidate:

- `9f20d8cb762f12bd8a80eeeefc589764747eda49`: rustls root-lockfile bump,
  `safe_open` non-unix symlink rejection, Rust + Python regression tests,
  `docs/dependency-security.md` and architecture scoping, M008 registration.
- `8af0127c48b38bdb3c020f0b0acda4801b4a665a`: binding-lockfile rustls bump
  (lockfile-only addendum; see §2).
- `067468a26c21716dd1063d67679768c6d794a2b9`: test-only relaxation of the
  intermediate-symlink assertion on unix (see §3). No production byte,
  manifest, version, or packaged-file change after `9f20d8c`.

## Defects resolved

1. Upstream rustls advisories RUSTSEC-2026-0346/0347/0348/0349 against
   locked rustls 0.23.45, reached via `eggfetch-core 0.2.2`,
   `eggress-* 1.0.12`, `hyper-rustls 0.27.9`, and `tokio-rustls 0.26.4`
   plus the direct `rustls = "0.23"` requirement. Remediation is a
   patch bump to 0.23.46 (the advisory-stated fix floor), lockfiles
   only, no waiver, no `deny.toml` change.
2. Windows symlink-following gap: the non-unix walk used
   `std::fs::metadata` (follows links) at every component, so
   `follow_symlinks=false` was silently unenforced on Windows. Every
   component is now `symlink_metadata`-checked at walk time and links
   are rejected with `SymlinkDetected` (message parity with the unix
   branch); the root itself stays followed, matching unix root policy.
   The unix branch is byte-identical.
3. Test-discovery during M008 (no production impact): the new
   intermediate-link test initially pinned `SymlinkDetected` on Linux,
   but Linux CI rejects from the untouched `openat2`/`openat` path with
   a non-`SymlinkDetected` variant (macOS yields `Io`/`ENOTDIR` by the
   same pre-existing code). The guard that matters is fail-closed, so
   unix pins rejection (with a forensic panic if unix ever opens
   through a link) while Windows pins `SymlinkDetected`.

## Verification on the exact candidate

| Check | Result |
|---|---|
| `make check` | Fully green end to end, including `dependency-policy` (`cargo deny check`: advisories/bans/licenses/sources ok) and `packaging-check`. Local run on the `9f20d8c` tree; the only later delta is test-only and the hosted `ci` job below covers the exact candidate. |
| Hosted `CI` run `38641009463` on `067468a` | `ci` success (full `make check` incl. 3482 lib tests with the new symlink tests) and `windows-portability` success. |
| Hosted `Python native qualification` run `38641009246` on `067468a` | **27/27 success**: 5/5 wheel builds, 20/20 installed-wheel smokes, extracted-sdist offline rebuild with clean-venv consumer run, and a green audit job — the first fully-green qualification, audit included. |
| Windows smokes on the candidate | 4/4 `24 passed, 5 skipped` (up from 23 passed: the new symlink test executes green, proving symlinked content never surfaces and real snippets stay intact). |
| `Rebuild and smoke extracted sdist` (`116116191489`) | Offline `CARGO_NET_OFFLINE=true` rebuild, clean-venv consumer run, `SHA256SUMS` recorded. |
| `Audit binding dependency graph` (`116116191915`) | `cargo deny check` on the binding graph plus `pip-audit --local`, both green. |
| Drift guard `38641009439`, egress qualification `38641009536` | Both success on `067468a`. |
| Root package shape | `cargo package --list --locked` 290 entries on the pristine and final trees alike (see §5); `cargo publish --dry-run --locked` clean; binding crate `publish = false`. |
| Python suite | `maturin develop` + `28 passed, 1 skipped` (new test skips off-Windows), `compileall` clean, `pyright` 0 errors. |

Supporting (not qualifying) runs: `38638066765` (27/27 on `8af0127`,
byte-identical wheels — the `067468a` delta is `#[cfg(test)]` only, which
never ships in release wheels); `38637266178` (26/27 on `9f20d8c`, audit
red solely on the binding-lockfile rustls copy, which §2 diagnoses and
`8af0127` fixes — deficiency evidence for the two-lockfile trap).

## Artifact digests (run `38641009246`)

- sdist `eggsearch-0.4.3.tar.gz`: `72cfebbb9e52abfd62a434caf8e702f07aafd8cf7c03cf08077fe98980e641ea`
- extracted-wheel `eggsearch-0.4.3-cp311-abi3-manylinux_2_39_x86_64.whl`: `e800c56e0b0896b326b2e2e7925f4ade9f68c76ea7678dcf6a5a8789b54c7b3e`
- `manylinux_2_17_x86_64`: `afd71f2d82b9595aecba7fb2df09b801d23d508c77d6345b7e9f9eb5c9cd2753`
- `manylinux_2_17_aarch64`: `36d4f50e1fdfc44a0bf31a619494da174945d50c1b6eb19f0f631af750ba8b87`
- `macosx_11_0_arm64`: `47018040d73d538c73c6ba84b698e9d2c2b9ecc5f480bd8b2273a9776deb3422`
- `macosx_10_12_x86_64`: `b99ffae10ed675514bdb0d2cd56c381b0762f2f75ab92063c30c8ad8d4895aed`
- `win_amd64`: `78ff1bf2f28635116cd8b894ab81aab9b00e857f8a31e15b3102d659840fa13f`

## Reconciliation with M004/M006/M007

`001-status.md`–`007-status.md` stand untouched as history. This record
supersedes them as the qualification basis:

| Prior basis | Standing after M008 |
|---|---|
| M004/M006/M007 closed at `faa1da2`, run `38617598636` (26/27, audit red on rustls; Windows symlink-following open) | Historical. Current qualified candidate is `067468a`, run `38641009246` (27/27, audit green, walk-time links rejected, `008-status.md` is the control point). |
| `cargo package --list` "291 entries" in `004-status.md` | Corrected: 290, verified identical on the pristine `8f632b3` tree and the final tree. The 291 was a miscount; no packaged file was added or removed by M008. |
| R2/R3 residuals (rustls advisories, pytest dev-pin note) | R2 resolved by the 0.23.46 bump in both lockfiles. The dev-pin pytest note is unchanged (workflow audit env clean). |
| M007 observation (no Windows Rust-test execution in CI) | Still true and still contained: the Windows proof is the executed smoke test (24 passed), while `cfg(windows)` unit-test bodies compile under `cargo check` and run wherever Windows tests execute. |
| Windows check-to-open swap window | Explicitly contained, not fixed: no atomic no-follow open exists in std, so a swap between the walk-time check and `File::open` remains possible. It requires write access to the user-owned jail root plus precise timing and fails closed otherwise. No new dependency was added to chase it. |

`Published to PyPI: no` — publication remains separately authorized and
would re-qualify whatever bytes it ships. Eggpack seven-target binary
assets unchanged.

## Handoff

M008 is closed. The Python native library workstream stands at
M001–M008 closed with `067468a` / run `38641009246` as the current
fully-green qualification basis, unpublished by design.
