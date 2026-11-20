# M005 — Packaged-Artifact Asset Completeness

Status: ready
Date: 2026-11-20
Subsystem: Python PyPI distribution and prebuilt SBC wheels
Milestone class: invariant (corrective to Python M001–M003 packaging verification and to
agentic-retrieval-integrity M005)
Baseline: `56f0bcbec122af5c6d5d2697046f7dd70fc9ec9b` on `feat/auto-20261117-044c4885`
Decision authority: existing contracts — ADR-0005 (Python native library boundary), the
"Global verification and risks" section of `plans/subsystems/python-pypi-distribution-roadmap.md`
(which already requires `cargo publish --dry-run` and sdist extraction/rebuild), and the
packaging conventions in `AGENTS.md`. No new ADR.

References: `plans/000-long-term-specification.md` §2; `plans/003-planning-process.md` §7;
`plans/subsystems/python-pypi-distribution-roadmap.md` M001–M004;
`plans/implementation/agentic-retrieval-integrity/005-verified-model-cutoff-coverage.md`;
`plans/closure/agentic-retrieval-integrity/005-status.md`;
`architecture/python-library.md`; `docs/test-inventory.md`.

## Objective

Make every asset a published crate reads at compile time part of the published artifact, so
`cargo package` verification and the maturin sdist rebuild compile from the tarball they
actually ship, and add the missing gate that would have caught it.

## Evidence

Observed on 2026-11-20, candidate `56f0bcb`, branch `feat/auto-20261117-044c4885`.

1. `Python native qualification` run `44832586001` (and every run from `bc52970` onward)
   failed in `Rebuild and smoke extracted sdist`:
   `error: couldn't read …/eggsearch-0.4.4/src/meta/knowledge_delta/catalog.json: No such file
   or directory`, at `src/meta/knowledge_delta/catalog.rs:55` (`include_str!("catalog.json")`),
   then `maturin failed — Failed to build a native library through cargo`.
2. The workflow was green on `57f5fed` and the six runs before it. `bc52970` is the first
   commit that contains the catalog, which arrived in `e817dfd`;
   `git log -1 -- src/meta/knowledge_delta/catalog.json` names `e817dfd`. The catalog commit
   did not touch `Cargo.toml`.
3. `cargo package --list --allow-dirty --locked` shipped 357 files and no
   `src/meta/knowledge_delta/catalog.json`; the `src/meta/knowledge_delta/` subtree shipped
   only `.rs` files. `[package] include` matches `src/**/*.rs` and specific `packaging/`
   assets, so a non-Rust asset under `src/` is excluded by default.
4. `cargo package --allow-dirty --locked` on the same tree exited 101 with the identical
   missing-file error and `failed to verify package tarball`. The defect therefore blocks
   `cargo publish` for 0.4.4, not only the Python lane.
5. The same guard input found a second gap: `src/platform.rs:127` reads
   `../packaging/release-targets.txt` from a unit test, and that file was not packaged either,
   so `cargo test` inside an extracted crate would fail.
6. `make packaging-check` runs `packaging/check-contract.sh`, which had no packaged-file-list
   check, so the routine gate could not see either gap.

## Non-goals and compatibility

- No new tool, argument, schema property, configuration key, provider, or dependency.
- No change to the release matrix, wheel families, PyPI publication workflow, Eggpack contract,
  or the ten-tool surface.
- No change to `[package] exclude`; the corrective is additive to `include`.
- No reopening of Python M001–M003 or agentic M005. Their closure records stay as written; the
  agentic M043 record gains an explicit reconciliation line because it had described this
  failure as pre-existing.

## Code pointers

- `Cargo.toml:14` — `[package] include`; both added entries are here.
- `src/meta/knowledge_delta/catalog.rs:55` — `include_str!("catalog.json")`.
- `src/platform.rs:127` — `include_str!("../packaging/release-targets.txt")` (unit test).
- `src/startup.rs:29` — two `include_str!` targets already covered by
  `packaging/systemd/*` and `packaging/launchd/*`, the pattern the correction follows.
- `src/meta/challenge/ledger.rs:475` — `include_str!("mod.rs")`, covered by `src/**/*.rs`.
- `packaging/check-contract.sh:17` — the `packaging-check` sequence that now calls the guard.
- `.github/workflows/python-native-qualification.yml:205` — the sdist job that failed.

## Ordered work packages

1. **Restore the published file list.** Add `src/meta/knowledge_delta/catalog.json` and
   `packaging/release-targets.txt` to `[package] include`.
2. **Gate it.** Add `packaging/check-packaged-assets.py`: run `cargo package --list`, scan
   `src/**/*.rs` for `include_str!`/`include_bytes!` literals, and fail when a target does not
   exist or is absent from the packaged set. Non-literal invocations fail rather than being
   skipped, so the guard cannot be bypassed silently. Call it from
   `packaging/check-contract.sh` so `make check` and `make ci` run it.
3. **Prove before and after.** `cargo package --list` must contain both assets;
   `cargo package --allow-dirty --locked` must verify cleanly; removing one include entry must
   make the guard exit non-zero with a message naming the asset and `Cargo.toml`.
4. **Sdist lane.** With the pinned `maturin==1.15.0` available, build the sdist, extract it,
   and rebuild offline, mirroring the workflow step; otherwise record that the hosted job is
   the first sdist evidence.
5. **Docs.** Record the invariant in `architecture/python-library.md` and the guard in
   `docs/test-inventory.md`, with a `CHANGELOG.md` entry.
6. **Reconcile.** Add a dated reconciliation line to
   `plans/closure/agentic-retrieval-integrity/043-status.md` correcting its "pre-existing"
   attribution to this milestone, without rewriting the rest of that record.

## Verification

```sh
python3 packaging/check-packaged-assets.py
cargo package --allow-dirty --locked
RUSTUP_TOOLCHAIN=1.89.0 make check
RUSTUP_TOOLCHAIN=1.89.0 make docs-check
```

Plus a negative control (remove one include entry, expect the guard to fail, restore it) and a
hosted `Python native qualification` run that is green on the exact candidate.

## Acceptance and stop conditions

- `cargo package` verifies from its own tarball, and the tarball contains both assets.
- The guard fails when an `include_str!` target is missing from `[package] include` and passes
  on the candidate.
- `make check` and `make docs-check` pass on the exact candidate.
- `Python native qualification` passes `Rebuild and smoke extracted sdist` on the exact
  candidate.
- No tool, argument, schema, ranking, release-matrix, or dependency change.

**Stop:** the sdist lane needs a build change beyond the include list (for example a maturin
configuration change), or the guard cannot be made authoritative without adding a dependency.
In either case, record the blocker rather than widening this milestone.

## Required closure evidence

`plans/closure/python-pypi-distribution/005-status.md` with the before/after packaged-file
counts, the failing and passing `cargo package` output, the guard's positive and negative
controls, the hosted run id, the broad-gate results, and residual risk. Update the roadmap and
registry together.
