# M005 Packaged-Artifact Asset Completeness — Closure Evidence

Status: closed

Source plan: `plans/implementation/python-pypi-distribution/005-packaged-artifact-asset-completeness.md`

Roadmap: `plans/subsystems/python-pypi-distribution-roadmap.md` M005

Corrective to: Python M001–M003 packaging verification (closures
`plans/closure/python-pypi-distribution/001-status.md`, `002-status.md`, `003-status.md`) and to
agentic-retrieval-integrity M005 (`plans/closure/agentic-retrieval-integrity/005-status.md`).

Decision authority: accepted ADR-0005 (Python native library boundary), the "Global verification
and risks" section of the Python roadmap, and the packaging conventions in `AGENTS.md`. No new
ADR, tool, argument, schema property, configuration key, provider, or dependency.

## Exact code candidate

Implementation candidate `ca24ee1ca7ddd355bd8d2170617238b02ddbaafa` (parent `56f0bcb`), on
`feat/auto-20261117-044c4885`. Every command, probe, and hosted job below ran against that tree.
This closure commit carries this record, the plan status, the roadmap status, and the registry
rows together.

## Symptom and root cause

`Python native qualification` failed in `Rebuild and smoke extracted sdist` on every tip from
`bc52970` through `56f0bcb`:

```text
error: couldn't read …/eggsearch-0.4.4/src/meta/knowledge_delta/catalog.json: No such file or directory
 --> …/src/meta/knowledge_delta/catalog.rs:55:34
55 |             serde_json::from_str(include_str!("catalog.json"))
💥 maturin failed — Failed to build a native library through cargo
```

The workflow was green on `57f5fed` and the six runs before it. The catalog arrived in
`e817dfd`, which did not touch `Cargo.toml`. `[package] include` lists `src/**/*.rs` plus
specific `packaging/` assets, so a non-Rust asset added under `src/` is excluded from both the
crate tarball and the maturin sdist by default.

The new guard found a second, older gap: `src/platform.rs:127` reads
`../packaging/release-targets.txt` from a unit test, and that file was not packaged either, so
`cargo test` inside an extracted crate would have failed.

## Change

1. `Cargo.toml` `[package] include` now carries `src/meta/knowledge_delta/catalog.json` and
   `packaging/release-targets.txt`. Both entries are additive; `[package] exclude` is unchanged.
2. New `packaging/check-packaged-assets.py` runs `cargo package --list --allow-dirty --locked`,
   scans `src/**/*.rs` for `include_str!`/`include_bytes!`, and fails when a literal target does
   not exist or is absent from the packaged set. A non-literal invocation is reported as an error
   rather than skipped, so the guard cannot be bypassed silently.
3. `packaging/check-contract.sh` calls the guard next to the existing workflow-pin and
   planning-consistency checks, so `make check` and `make ci` run it.

## Requirement-to-evidence mapping

| # | Acceptance criterion | Evidence |
|---|---|---|
| 1 | `cargo package` verifies from its own tarball | Before: `cargo package --allow-dirty --locked` exited 101 with the missing catalog and `failed to verify package tarball`. After: exit 0, `Packaged 359 files`, `Verifying eggsearch v0.4.4` compiled cleanly. |
| 2 | The tarball contains both assets | `tar -tzf target/package/eggsearch-0.4.4.crate` lists `eggsearch-0.4.4/src/meta/knowledge_delta/catalog.json` and `eggsearch-0.4.4/packaging/release-targets.txt`; 359 entries. |
| 3 | The guard fails when an asset is missing and passes on the candidate | Negative control (include entry removed): `packaged-assets: src/meta/knowledge_delta/catalog.rs: catalog.json is not matched by [package] include in Cargo.toml`, exit 1. Restored: `packaged-assets: ok (5 compile-time asset(s) across 359 packaged files)`, exit 0. |
| 4 | The sdist carries the assets | Local `maturin 1.9.5 sdist` (the lane pins 1.15.0) produced `eggsearch-0.4.4.tar.gz`, and extraction contains both assets. The hosted sdist job is the authoritative 1.15.0 evidence. |
| 5 | `make check` and `make docs-check` pass on the candidate | `make check` exit 0 with 6 553 passed, 0 failed, 28 ignored across 92 binaries, and `packaged-assets: ok (5 compile-time asset(s) across 359 packaged files)` inside the gate; `make docs-check` exit 0; clippy `-D warnings` and `cargo fmt -- --check` clean. |
| 6 | The hosted sdist lane is green on the exact candidate | `Python native qualification` run `44839984115`: success, including `Rebuild and smoke extracted sdist`, on attempt 1. |
| 7 | No tool, argument, schema, ranking, release-matrix, or dependency change | `static_guards` passes; `Cargo.toml` changes are confined to three `include` lines; no dependency, workflow, or tool surface changed. |
| 8 | The M043 record is reconciled rather than silently rewritten | `plans/closure/agentic-retrieval-integrity/043-status.md` gains a dated reconciliation line stating that its "pre-existing" attribution was wrong and naming this milestone. |

## Before and after

| Check | Before | After |
|---|---|---|
| `cargo package --list` | 357 files, no catalog | 359 files, both assets |
| `cargo package` verify | exit 101, missing catalog | exit 0, packaged crate compiles |
| Extracted sdist build (hosted) | failed, missing catalog | passed |
| Guard | absent | exit 0 on the candidate, exit 1 without the include entry |

## Hosted qualification

On candidate `ca24ee1`, all on attempt 1:

- `Python native qualification` run `44839984115`: **success**, including
  `Rebuild and smoke extracted sdist`.
- `CI` run `44839984052`: `ci`, `windows portability`, and `macOS canonical verification and
  local filesystem portability` all success.
- `Egress feature qualification` run `44839984130`: success.
- `Release drift guard` run `44839984046`: success.

## Residual risk

1. **The guard covers `src/` only.** Assets read by tests or benches are not packaged at all, so
   they are out of scope by design; a future `include_str!` there is not checked.
2. **Literal paths only.** A non-literal macro argument fails the guard instead of being resolved,
   which keeps it honest but requires the literal form.
3. **Local sdist used maturin 1.9.5.** The lane pins 1.15.0; the local build is corroboration and
   the hosted job is the qualifier.
4. **Cost.** The guard adds one `cargo package --list` (about 0.3 s) to `packaging-check`.
   It needs `cargo`, which `make check` already requires.

## Not done

No dependency, workflow, release-matrix, wheel-family, PyPI publication, or tool-surface change.
Python M001–M003 and agentic M005 closure records are unchanged; the M043 reconciliation is
additive.
