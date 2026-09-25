# Phase 16 — First Binary Release Hardening and Cutover

Status: implemented
Depends on: implemented phases 6-10
Baseline for planning: `34b36d1004121ba9891bac17b1033b150e8f1a3d` (`eggsearch` 0.3.8 on `main`)
Governing deployment contract: `plans/deployment-roadmap.md`
Historical implementation plan: `plans/phase-6-release-binaries-and-bootstrap-installers.md`

## Why this corrective phase exists

The binary-distribution implementation landed after the currently published `v0.3.8` tag and GitHub Release. `v0.3.8` therefore has no attached executable/checksum/installer assets, while the current README already advertises `releases/latest/download/install.sh`.

The release workflow has never been exercised by a compatible release tag. Its current release path is intentionally strict:

1. the exact crate version must already be visible on crates.io;
2. the workflow checks out the exact `vX.Y.Z` tag;
3. packaging/release-smoke files must therefore already exist in that tag;
4. every required target must qualify before assembly;
5. assembly creates or updates a draft GitHub Release and refuses to overwrite an already-published release.

This is safe for normal steady-state releases but creates a first-release hazard: the first end-to-end execution of the seven-target matrix would occur only after the new crate version is immutable on crates.io. This phase closes that operational gap before the next version is published.

Do not attempt to retrofit `v0.3.8`. The tag predates the packaging tree and the published GitHub Release is intentionally immutable to the assembler. The first binary-enabled release must be a new version/tag.

## Objective

Qualify and harden the complete binary/installer pipeline before publishing the next crate version, then make the next release the first supported binary-enabled release with a deterministic and observable cutover.

The implementation should make it possible to answer, before `cargo publish`, all of the following:

- Can every required target compile from the exact candidate commit?
- Do architecture/linkage checks and runtime smoke pass?
- Does the assembled artifact set exactly match the public asset contract?
- Do Unix and PowerShell installers select the same assets as the workflow/updater?
- Does a simulated release download/install path work without silently compiling from source?
- Is the candidate tag guaranteed to contain every file the tagged release workflow requires?
- If the eventual tagged release fails, is the failure actionable without mutating or weakening release invariants?

## Non-goals

- Do not add new package managers or container distribution.
- Do not change the seven-target public asset contract unless qualification proves a target impossible; such a change requires a separate compatibility decision and atomic docs/updater/installer changes.
- Do not auto-publish crates.io or auto-publish the final GitHub Release.
- Do not weaken checksum, version, glibc-floor, architecture, MCP smoke, or fail-closed installer behavior to obtain a green matrix.
- Do not add best-effort target rows that are allowed to fail silently.
- Do not backfill binaries into `v0.3.8`.

## Invariants

1. Release mode always builds the exact tagged commit.
2. Qualification mode and release mode execute the same build/stage/smoke/checksum logic; only release-state validation and publication differ.
3. Qualification must not create, edit, or publish a GitHub Release.
4. Qualification must not require the candidate version to exist on crates.io.
5. Release mode must continue to require exact crates.io publication before assembling the draft release unless a separate release-order plan deliberately changes that policy.
6. The public target/asset mapping has one canonical source of truth and all consumers are checked against it.
7. Missing release assets are never confused with a successful binary install; Cargo fallback remains explicit and observable.
8. Routine PR CI remains materially smaller than the release matrix.
9. The next published version must not be tagged until full qualification succeeds on the exact intended release commit.
10. The release workflow must fail before expensive matrix work when the tagged tree lacks required packaging inputs or version metadata is inconsistent.

## Work item 1 — Add a non-publishing full-matrix qualification path

Extend `.github/workflows/release-binaries.yml` or factor its reusable build logic into a reusable workflow so maintainers can exercise the exact release matrix before publishing a version.

Preferred interface:

```text
workflow_dispatch
  mode: qualify | release
  ref: commit SHA / branch for qualify
  tag: vX.Y.Z for release
```

Equivalent input naming is acceptable, but the modes must be unambiguous.

### Qualification mode requirements

Qualification mode must:

- resolve the supplied ref to a commit SHA and print it prominently;
- checkout that exact SHA in every job rather than allowing a moving branch checkout after preflight;
- read the package version from that checkout;
- require a clean tracked checkout and `Cargo.lock`;
- verify required packaging/release files exist before launching the matrix;
- run the same Linux x86-64, Linux AArch64, Linux ARMv7, macOS Intel, macOS Apple Silicon, Windows x86-64, and Windows ARM64 build jobs used by release mode;
- run the same architecture/linkage, CLI, MCP, checksum, and artifact-set checks used by release mode;
- merge the complete output into a workflow artifact for maintainer inspection;
- not query crates.io as a gating condition;
- not create or upload to a GitHub Release;
- clearly label artifacts as qualification-only so they cannot be mistaken for released binaries.

Qualification should be safe to run repeatedly on `main` or an exact release-candidate SHA.

### Release mode requirements

Release mode must preserve the current production invariants:

- require a `v<semver>` tag;
- require Cargo package version == tag version;
- require checkout commit == tag commit;
- require the exact crate version to be visible on crates.io;
- require all build jobs to pass;
- assemble only artifacts from the current run;
- create/update only a draft release for that exact tag;
- refuse to mutate a published release.

Avoid duplicating shell logic between qualify and release. A reusable workflow, composite action, or checked-in release helper is preferable to two divergent matrices.

## Work item 2 — Add a release-tree completeness preflight

Before any matrix job, verify that the exact candidate tree contains every input needed by the workflow.

At minimum check:

```text
Cargo.toml
Cargo.lock
packaging/release-targets.txt
packaging/release-smoke.sh
packaging/release-smoke.ps1
packaging/install.sh
packaging/install.ps1
```

Also check any service/integration fixtures invoked by the smoke scripts.

If a required path is absent, fail in preflight with the missing path and checked-out commit SHA. This specifically prevents recurrence of the `v0.3.8` situation where a historical tag cannot satisfy a later workflow implementation.

The file list should be maintained near the release helper rather than buried independently in workflow YAML if practical.

## Work item 3 — Make the target/asset contract machine-checked

`packaging/release-targets.txt` should remain the canonical public mapping unless implementation finds a better single-source representation.

Add a deterministic contract check that verifies every consumer agrees with it:

- `.github/workflows/release-binaries.yml` matrix;
- `packaging/install.sh` host mapping;
- `packaging/install.ps1` host mapping;
- Rust self-update target mapping;
- `docs/installation.md` target table;
- release assembly expected file count.

Do not merely compare the number of targets. Validate exact Rust target, exact asset filename, operating system, architecture, executable suffix, and checksum companion filename.

Prefer parsing a checked-in canonical table from tests/helpers over maintaining multiple hand-written expectation lists. If workflow YAML cannot directly consume the canonical file without undue complexity, add a test that detects drift.

Acceptance should prove that changing an asset name in one consumer alone causes a deterministic test failure.

## Work item 4 — Harden assembly semantics and diagnostics

The assembler currently verifies the complete set and expects 16 files: seven binaries, seven checksums, and two installer scripts. Keep the completeness guarantee, but make diagnostics explicit.

Assembly must report:

- expected asset set;
- observed asset set;
- missing assets;
- unexpected/duplicate assets;
- commit SHA associated with the run;
- version/tag in release mode;
- package version in qualification mode.

Do not rely only on `find ... | wc -l`. A count can be correct while names are wrong. Exact set equality is the release invariant.

Ensure per-target artifact names are unique and that `download-artifact` cannot overwrite two same-named files unnoticed during `merge-multiple`.

## Work item 5 — Add installer contract tests that do not require a real published release

The current bootstrap script's internal Cargo fallback cannot help if the README bootstrap URL itself returns 404. Add deterministic installer tests around a local/mock HTTP endpoint or injected download base so the installer behavior can be qualified before release.

Cover at minimum:

- supported host + binary 200 + checksum 200 -> verified binary install;
- binary 404 -> explicit Cargo fallback path;
- binary 500/network failure -> hard failure, no Cargo fallback;
- checksum 404 -> hard failure;
- checksum mismatch -> hard failure;
- candidate version mismatch -> hard failure;
- unsupported host -> explicit Cargo fallback;
- pinned version constructs the exact `vX.Y.Z` URL;
- latest mode constructs the latest-release URL;
- install destination and PATH messaging remain correct;
- no installer path invokes `sudo` automatically.

Mirror applicable cases for PowerShell.

Where practical, expose a test-only/environment override for the release base URL rather than rewriting scripts during tests. Ensure production behavior ignores unsafe overrides unless explicitly intended for testing.

## Work item 6 — Repair the pre-first-release bootstrap documentation state

Until the first binary-enabled GitHub Release is actually published, `https://github.com/eggstack/eggsearch/releases/latest/download/install.sh` is a dead bootstrap URL because `v0.3.8` has no assets.

Do not leave documentation claiming the one-line release bootstrap currently works when it does not.

Choose one temporary policy before the next release:

### Preferred policy

Temporarily document Cargo installation as the guaranteed path and mark the one-line GitHub Release installer as becoming active with the first binary-enabled release. Keep the script in-repo and test it through qualification.

This avoids teaching users to execute a mutable `main` branch script remotely.

### Acceptable alternative

If maintainers explicitly prefer immediate installer availability, provide a versioned/raw bootstrap URL pinned to a reviewed commit. Do not recommend an unpinned mutable `raw/main` script as the security-equivalent of an attached release artifact.

After the first binary-enabled release is published and assets are verified externally, switch the primary documentation to `releases/latest/download/install.sh` and remove the transitional warning in the same closure pass.

## Work item 7 — Add a release-candidate gate to `make release-check`

`make release-check` should remain locally useful and deterministic where possible, but it must catch release packaging drift before `cargo publish`.

Add checks for:

- required packaging files present;
- release target contract consistency;
- installer shell syntax (`bash -n`) and PowerShell syntax/static validation where available;
- release smoke helpers are executable/parseable as appropriate;
- workflow references only existing packaging paths;
- Cargo package version is a valid release candidate version;
- `cargo publish --dry-run --locked` still passes.

Do not try to reproduce seven OS/architectures locally. The full matrix qualification workflow is the authoritative portability gate.

Document the intended pre-publication sequence as:

```text
make release-check
-> push exact release-candidate commit
-> run Release binaries / qualify on that exact SHA
-> inspect complete qualification artifact set
-> only then cargo publish --locked
-> create/push vX.Y.Z tag at the same qualified SHA
-> Release binaries / release runs
-> inspect draft release assets/checksums
-> publish draft release
-> run external installer smoke against the published release
```

## Work item 8 — Preserve exact qualification SHA through publication

The commit qualified before publication must be the commit eventually tagged.

Add a simple maintainer-visible mechanism to prevent accidental drift:

- qualification summary prints `QUALIFIED_SHA=<sha>` and package version;
- documentation requires `git rev-parse vX.Y.Z^{commit}` to equal that SHA before the release is considered valid;
- tagged release preflight continues to verify tag/check-out identity;
- if any source, lockfile, packaging, workflow, installer, or release-smoke file changes after qualification, rerun qualification on the new SHA.

Do not accept "same version, nearby commit" as equivalent.

## Work item 9 — Add workflow summaries and failure observability

Every qualification/release run should produce a concise GitHub Actions job summary containing:

- mode (`qualify` or `release`);
- commit SHA;
- package version;
- tag when applicable;
- seven target rows with build/smoke/checksum result;
- artifact-set completeness result;
- whether a draft release was created/updated;
- clear next action.

Failures should identify the stage and target without requiring maintainers to infer which matrix row blocked assembly.

Do not add external telemetry or credentials for this.

## Work item 10 — First binary-enabled release cutover

Use a new SemVer version after all preceding hardening is implemented. At planning time the natural next version is `0.3.9`, but use the actual next version selected by maintainers; do not reuse or move `v0.3.8`.

The first release procedure is deliberately stricter than steady-state:

1. Ensure all phase-16 changes are committed.
2. Update version/changelog for the next release candidate.
3. Run `make release-check`.
4. Push the exact candidate commit.
5. Run full `qualify` mode against its immutable SHA.
6. Require all seven targets and complete merged artifact set to pass.
7. Record the qualified SHA in the release notes/work log.
8. Publish the exact crate version to crates.io.
9. Confirm the registry exposes that exact version.
10. Create `vX.Y.Z` on the same qualified SHA and push it.
11. Allow release mode to rebuild from the tag; do not reuse qualification artifacts as release assets.
12. Confirm the workflow produces a draft GitHub Release containing exactly seven binaries, seven checksums, `install.sh`, and `install.ps1`.
13. Download at least one representative asset/checksum from the draft or workflow output and independently verify it.
14. Publish the draft release manually.
15. From outside the repository checkout, exercise the documented Unix bootstrap against `releases/latest/download/install.sh` on a supported host and confirm the log indicates binary download rather than Cargo fallback.
16. Exercise the pinned installer path for the released version.
17. Exercise PowerShell bootstrap on a Windows host or the same Windows CI environment against the published assets.
18. Verify `eggsearch update --check` recognizes the released version contract.
19. Verify the GitHub Release API reports all 16 assets.
20. Only then mark phase 16 implemented and restore/remove any transitional installation documentation.

If the tagged release matrix exposes a failure that qualification did not catch, do not mutate the tag or weaken the gate. Diagnose the difference, fix it in a new commit/version as required by crates.io immutability, and update this plan with the discovered gap.

## Tests and verification

Required deterministic/local checks after implementation:

```text
make check
make release-check
```

Required remote pre-publication check:

```text
Release binaries workflow: mode=qualify, ref=<exact candidate SHA>
```

Qualification acceptance requires all seven public targets to succeed and the merged artifact set to contain exactly the expected binaries/checksums/installers.

Required first-release post-publication checks:

```text
GitHub Release asset-set equality
Unix latest bootstrap installs a prebuilt binary
Unix pinned bootstrap installs the exact version
Windows bootstrap installs a prebuilt binary
checksum/version verification succeeds
update --check resolves the same release
```

## Acceptance criteria

Phase 16 is complete only when all of the following are true:

1. A full seven-target release qualification can run before crates.io publication and cannot publish/mutate a GitHub Release.
2. Qualification and tagged release share the same build/smoke/checksum implementation.
3. Release preflight fails immediately if required packaging inputs are absent from the exact checkout.
4. Target/asset mappings are contract-tested across workflow, installers, updater, docs, and assembly.
5. Assembly proves exact asset-set equality rather than relying only on a file count.
6. Installer success/fallback/fail-closed behavior is deterministically tested without depending on a live GitHub Release.
7. Documentation no longer claims a currently nonexistent release bootstrap is usable before the first binary-enabled release.
8. `make release-check` includes packaging/contract checks sufficient to detect obvious release drift before publication.
9. The exact SHA that passes qualification is the SHA tagged for the first binary-enabled release.
10. The first binary-enabled GitHub Release contains exactly seven executables, seven checksum files, `install.sh`, and `install.ps1`.
11. External Unix and Windows bootstrap verification demonstrates prebuilt-binary installation from the published release rather than Cargo fallback.
12. `eggsearch update --check` and the installers resolve the same version/asset contract.
13. `plans/registry.md` is updated with implementation evidence and this phase is marked `implemented` only after the published-release smoke completes.

## Handoff notes

Treat the existing phase-6 implementation as the baseline, not as a specification to rewrite wholesale. The goal is to add pre-publication qualification and contract observability around the existing release logic with minimal duplication.

The most important architectural property is that qualification and release cannot drift. A green preflight-only surrogate that uses different commands from production does not solve the first-release risk.

The next release should be the first normal consumer of this hardened path. Do not create a special one-off `v0.3.8` recovery mechanism that becomes dead maintenance code immediately afterward.
