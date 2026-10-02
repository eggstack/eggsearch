# CodeGG Legacy Search Parity M004 — 0.4.0 Release Publication and Downstream Handoff

Status: blocked — corrective pass required

Recommendation: corrective pass required (M005 registered).

Source implementation plan:

- `plans/implementation/codegg-legacy-search-parity/004-v0.4.0-release-publication-and-downstream-handoff.md`

Source subsystem roadmap:

- `plans/subsystems/codegg-legacy-search-parity-corrective-addendum.md`

Planning baseline: `71e5d4de5d1a278f59d275914c7068da8f7a9b31`

Release-prep candidate: `81a2e4931c97a14010aeda85fccb682f4d4839a6`
(`release: prepare eggsearch 0.4.0 candidate`; metadata/docs only)

Predecessor closures: M001 `e9103b4`, M002 `5233315`, M003 conditionally
closed at `8e5ec75`
(`plans/closure/codegg-legacy-search-parity/003-status.md`), repository
hardening M008 closed at `44f38ab`
(`plans/closure/repository-hardening/008-status.md`).

## 1. Executive finding

M004 is **blocked, not closed**. Every work package the repository controls
succeeded up to and including crates.io publication and tagging, and then the
tag-triggered binary release failed deterministically in the release assembler
at the provenance-attestation step. The failure is a latent defect in the
release workflow introduced by repository-hardening M002 and never exercised
before: release mode had never run with the attestation step present, and
qualification mode skips it by design.

Per M004 §12, this is the "correcting source bytes is required" stop case:
the defect is in the workflow file at the tagged candidate, so no operational
re-run from the same source can succeed, and the tag and published crate are
immutable. `v0.4.0` therefore remains a **partial publication** (crate +
tag, no binary GitHub Release). No tag was moved, no crate was overwritten,
and no gate was weakened.

The corrective path is registered as M005
(`plans/implementation/codegg-legacy-search-parity/005-v0.4.1-corrective-release.md`):
a 0.4.1 patch release carrying only the workflow fix plus version metadata,
repeating the full qualify → publish → tag → release → smoke sequence. M003
remains conditionally closed and the downstream CodeGG adoption milestone
remains blocked until M005 completes.

## 2. Work-package evidence

### WP-A — Release scope and metadata (complete)

Post-baseline audit: the only commits after `71e5d4d` were three plans-only
commits (`e80fba3`, `af0f9f2`, `48ebe50`); no production code moved.

Release-preparation commit `81a2e49` contains exactly the §7 set:

- `Cargo.toml`: `0.3.9` → `0.4.0`.
- `Cargo.lock`: root package `0.3.9` → `0.4.0` via normal Cargo generation
  (one-line diff, no dependency changes).
- `CHANGELOG.md`: Unreleased material moved to `[0.4.0] - 2026-10-02` with a
  fresh empty `[Unreleased]` retained.
- `docs/codegg-integration.md`: migration-inventory wording corrected to
  distinguish the seven newly added parity providers from the full retained
  inventory; frozen disposition matrix untouched.
- No other version reference touched; historical 0.3.9 records unchanged.

Verification on the candidate: `cargo metadata` reports `0.4.0`,
`cargo run --locked -- --version` reports `eggsearch 0.4.0`, provider
inventory still 44 (`docs_provider_inventory` green), ten-tool contract
unchanged (`docs_tool_names` green).

### WP-B — Local exact-candidate verification (complete, all green)

On clean checkout of `81a2e49`:

| Gate | Result |
|------|--------|
| `cargo fmt --all -- --check` | pass |
| `python3 packaging/check-workflow-pins.py` | `ok (4 workflow files)` |
| `python3 packaging/check-planning-consistency.py` | `ok` |
| `make check` | exit 0 |
| `make docs-check` | exit 0 |
| `make release-check` (incl. `cargo publish --dry-run --locked`) | exit 0 |
| `web_search_integration` (`--features mock`) | 56 passed |
| `provider_routing` (`--features mock`) | 83 passed |
| `provider_capability_contract` (`--all-features`) | 22 passed |
| `docs_provider_inventory` (`--all-features`) | pass |
| `dispatch_fault_injection` (`--all-features`) | 33 passed |
| `static_guards` (`--all-features`) | 58 passed |

### WP-C — GitHub exact-candidate binary qualification (complete, green)

Pushed candidate to `main`, dispatched `Release binaries` with
`mode=qualify`, `ref=81a2e4931c97a14010aeda85fccb682f4d4839a6`.

Run `37063695085`: **success** (all 9 jobs green).

- Preflight (qualify): success.
- Seven target jobs: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`,
  armv7, `x86_64-apple-darwin`, `aarch64-apple-darwin`,
  `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc`: all success.
- Assemble qualify release output: success.
- `head_sha` = `81a2e4931c97a14010aeda85fccb682f4d4839a6` = candidate SHA.
- Qualification-only artifact present:
  `qualification-0.4.0-81a2e4931c97a14010aeda85fccb682f4d4839a6-complete`
  plus seven per-target `qualify-0.4.0-81a2e4931c97-*` artifacts.
- No GitHub Release created or mutated (latest remained `v0.3.9`).
- No crates.io publication (max remained `0.3.9` at that point).

### WP-D — Publish crate 0.4.0 (complete)

`cargo publish --locked` from the clean candidate: **published**
(`Uploaded eggsearch v0.4.0`, `Published eggsearch v0.4.0`).

External verification:

- crates.io API: `max_version` = `newest_version` = `0.4.0`; versions list
  shows single unyanked `0.4.0` above `0.3.9`.
- `cargo search eggsearch` resolves `0.4.0`.
- `cargo package --list --locked` enumerates 287 files with no `plans/`,
  `.github/`, or `fuzz/` leakage (package contract holds).

### WP-E — Tag and binary release (tag complete; release workflow failed)

`v0.4.0` created at exactly the qualified candidate and pushed:

- `git rev-list -n 1 v0.4.0` =
  `81a2e4931c97a14010aeda85fccb682f4d4839a6`.
- `tag target == QUALIFIED_SHA == release-prep candidate SHA`: holds.

Tag-triggered `Release binaries` run `37065679022`: **failure**.

- Preflight (release): success (tag/version/commit/Cargo.lock/crates.io
  identity validated).
- All seven target build/smoke/checksum jobs: success.
- `Verify complete asset set`: passed (16 files staged).
- `Attest release binaries and installers`: **failed** with
  `Error: predicate-type must be provided`.
- Downstream steps (attestation verification, draft-release creation,
  summary) skipped. No draft release exists: `gh release view v0.4.0` →
  `release not found`; latest published release remains `v0.3.9`.

### WP-F / WP-G — not executed (stop rule applied)

No 16-asset draft existed to review, so no manual publication, no
attestation sampling, and no external installer/update/MCP smoke were
attempted. Attempting them against unpublished artifacts would have violated
the §12 stop rule.

## 3. Root cause of the release failure

The pinned `actions/attest@daf44fb950173508f38bd2406030372c1d1162b1`
(v3.0.0) declares `predicate-type` as `required: true` in its `action.yml`
and its bundled `dist/index.js` fails closed with
`predicate-type must be provided`. The workflow's attest step supplies only
`subject-path` and was written as if provenance attestation were automatic;
automatic SLSA-provenance mode only exists in `actions/attest` v4, not in the
pinned v3 generic-attestation action.

Why qualification never caught it:

- The attest step is gated `if: mode == 'release'`; every qualification run
  (including M004's `37063695085` and M008's `37036489273`) skips it.
- `v0.3.9` (tag `0cbbeee7`) predates the attest step: `0cbbeee7` is an
  ancestor of `cb40481` (repository-hardening M002, which added attestation).
  Release mode therefore never ran with this step until M004.
- Corroborating sample: `gh attestation verify` against a freshly downloaded
  `v0.3.9` asset returns HTTP 404 (no attestation on record), consistent with
  no prior release ever generating one.

Secondary finding: the `assemble` job grants `contents: write`,
`id-token: write`, `attestations: write` but not `artifact-metadata: write`,
which current `actions/attest` documentation lists as required for the
storage record. M005 must address both the predicate input (or action
upgrade) and the permission set together.

## 4. Partial-publication inventory (immutable facts)

- crates.io `eggsearch 0.4.0`: published, immutable, cannot be overwritten.
- git tag `v0.4.0`: exists, points to `81a2e49`, immutable, never to be
  force-moved.
- GitHub Release `v0.4.0`: does not exist (not even as draft).
- Binary assets for 0.4.0: built and checksummed inside run `37065679022`
  job artifacts only; never assembled into a release.
- Provenance attestations for 0.4.0: none generated.
- `v0.4.0` will permanently remain crate+tag only. The complete binary
  publication of this surface will be `v0.4.1` via M005.

## 5. Invariant and compatibility review

| Invariant | Status |
|-----------|--------|
| `v0.4.0` tag and crates.io `0.4.0` identify the same candidate | holds (`81a2e49`) |
| No tag move, no crate overwrite after publication | holds |
| Seven release targets, glibc 2.17 floor, MSRV 1.89 | unchanged |
| Provider inventory 44; seven parity ids present | holds |
| CodeGG parity/disposition contract (`github_repositories` rename, Kagi v1 equivalent, `google_news` retired) | unchanged |
| Ten MCP tools; no schema/routing/cache/SSRF/updater change | holds (metadata/docs-only delta) |
| Checksums-not-replaced-by-attestations policy | unviolated (no release assembled) |
| 16-asset contract for `v0.4.0` | **unmet — no release exists** (see §6) |

## 6. Unresolved findings by severity

| Severity | Finding | Disposition |
|----------|---------|-------------|
| High, release-blocking | `v0.4.0` has no binary GitHub Release: release run `37065679022` failed at the pinned v3 attest step (`predicate-type must be provided`). | M005 corrective release owns the fix and the 0.4.1 publication. |
| Medium, process | First-release attestation/immutability evidence (repository-hardening M002's condition) was not produced: no attestation generated, no release published. | Carried to M005 acceptance; M002 stays conditionally closed. |
| Low, informational | Downloaded `v0.3.9` asset carries no verifiable attestation (HTTP 404), consistent with the attest step postdating that tag. Historical v0.3.9 records are left unchanged. | Informational only. |
| Low, informational | WP-F/WP-G evidence (draft review, installer/update smoke, installed MCP/provider_status checks) was deliberately not collected per the §12 stop rule. | To be collected against 0.4.1 in M005. |

No provider, tool, or contract defect is open. No medium-or-higher product
finding exists; both open findings are release-operations findings with a
registered corrective owner.

## 7. Registry and roadmap disposition

- M004 plan status `ready` → `blocked` (corrective pass required; this
  record and M005 linked).
- Subsystem roadmap: M004 marked blocked with the attest defect; M005
  registered as ready; workstream remains active (not closed).
- `plans/registry.md`: M004 row, CodeGG roadmap row, blocked-work section,
  and closure control points updated in the same closure commit.
- M003 stays `conditionally closed`: its sole publication condition is not
  cleared (no immutable 0.4.0 release exists). The condition transfers to the
  0.4.1 immutable release delivered by M005; M003 history is not rewritten.
- Repository-hardening M002 stays `conditionally closed` (no first-release
  provenance obtained). M007/M006/M001 dispositions unchanged.
- Downstream CodeGG parity adoption stays blocked; the handoff target moves
  to pinning the M005 release (`>=0.4.1`, which carries the identical parity
  surface).

Recommendation: **corrective pass required**. M004 must not be marked closed
on crate+tag alone; closure of the publication boundary belongs to M005's
completed immutable release.
