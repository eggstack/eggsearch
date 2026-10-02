# CodeGG Legacy Search Parity M005 — 0.4.1 Corrective Release (Release-Workflow Attest Fix)

Status: closed

Recommendation: closed. All M005 acceptance criteria are met; M004 and M003
are reconciled to closed on this evidence; downstream CodeGG parity adoption
is unblocked at `>=0.4.1`.

Source implementation plan:

- `plans/implementation/codegg-legacy-search-parity/005-v0.4.1-corrective-release.md`

Source subsystem roadmap:

- `plans/subsystems/codegg-legacy-search-parity-corrective-addendum.md`

Planning baseline: `81a2e4931c97a14010aeda85fccb682f4d4839a6` (M004
release-prep candidate; the M004 closure commit is plans/docs only)

Fix-prep candidate: `32769d684417567dfb30fc5adea1475cee7f0ed0`
(`release: prepare eggsearch 0.4.1 corrective candidate`; workflow +
metadata only, no production Rust change)

Predecessor closures: M004 blocked at
`plans/closure/codegg-legacy-search-parity/004-status.md`, M003
conditionally closed at `8e5ec75`
(`plans/closure/codegg-legacy-search-parity/003-status.md`),
repository-hardening M002 conditionally closed
(`plans/closure/repository-hardening/002-status.md`), M008 closed at
`44f38ab`.

## 1. Executive finding

M005 is **closed**. The latent release-workflow attestation defect that
blocked M004 is repaired (one workflow change, verified against upstream
`actions/attest` v4.2.2 documentation and `action.yml`), and the identical
CodeGG parity surface is now completely published as immutable `eggsearch
0.4.1`: crate on crates.io, tag `v0.4.1` at exactly the qualified candidate,
and a manually reviewed 16-asset GitHub Release with SLSA build-provenance
attestations that verify back to this repository, the release workflow, and
the exact candidate commit. No tag was moved, no crate overwritten, no gate
weakened, and `v0.4.0` is untouched (crate+tag only, permanently).

## 2. Work-package evidence

### WP-A — Fix scope and release-prep commit (complete)

Pre-change state matched the plan §3 premise: tag `v0.4.0` resolved to
`81a2e49`, crates.io max was `0.4.0`, no `v0.4.1` tag or crate existed
anywhere, and the workflow carried the v3 attest pin with only
`subject-path`.

Fix-prep commit `32769d6` contains exactly the §7 set (4 files, +14/-3):

- `.github/workflows/release-binaries.yml`: `actions/attest`
  `daf44fb950173508f38bd2406030372c1d1162b1` (v3.0.0) →
  `1e69f48acb82d1966a394da916b4c1698aa569d6` (v4.2.2), keeping the existing
  `subject-path` shape; `assemble` job gains `artifact-metadata: write`.
  No other workflow change.
- `Cargo.toml`: `0.4.0` → `0.4.1`.
- `Cargo.lock`: root package `0.4.0` → `0.4.1` via normal Cargo generation
  (one-line diff; `rand_xorshift`/`winapi-*` 0.4.0 entries are third-party
  and untouched).
- `CHANGELOG.md`: new `[0.4.1] - 2026-10-02` entry describing the
  attestation repair and the 0.4.0→0.4.1 publication facts; `[0.4.0]`
  preserved verbatim; fresh empty `[Unreleased]` retained.

Pin provenance (not guessed): `git ls-remote` on `actions/attest` shows
`refs/tags/v4.2.2` → `1e69f48acb82d1966a394da916b4c1698aa569d6` (lightweight
tag, direct commit); the `action.yml` at that SHA declares
`predicate-type` `required: false` with provenance auto-generation as the
default mode for subject-only invocations; the v4.2.2 README lists exactly
`id-token: write`, `attestations: write`, `artifact-metadata: write` as the
required permission set.

Verification on the candidate: `cargo metadata` reports `0.4.1`,
`cargo run --locked -- --version` reports `eggsearch 0.4.1`,
`providers --json` carries 44 ids, ten-tool contract unchanged
(`docs_tool_names` green).

### WP-B — Local exact-candidate verification (complete, all green)

On the clean candidate `32769d6`:

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
| `docs_tool_names` (`--all-features`) | 2 passed |
| `dispatch_fault_injection` (`--all-features`) | 33 passed |
| `static_guards` (`--all-features`) | 58 passed |

### WP-C — GitHub exact-candidate binary qualification (complete, green)

Pushed the candidate to `main`, dispatched `Release binaries` with
`mode=qualify`, `ref=32769d684417567dfb30fc5adea1475cee7f0ed0`.

Run `37070932877`: **success** (all 9 jobs green: preflight + seven
targets + assemble).

- `head_sha` = `32769d684417567dfb30fc5adea1475cee7f0ed0` = candidate SHA;
  run page binds the run to `@32769d6`.
- Qualification-only artifact present:
  `qualification-0.4.1-32769d684417567dfb30fc5adea1475cee7f0ed0-complete`
  plus seven per-target `qualify-0.4.1-32769d684417-*` artifacts.
- No GitHub Release created or mutated (latest remained `v0.3.9`); no
  crates.io publication (max remained `0.4.0` at that point); no
  attestation generated (attest step is release-gated by design).

### WP-D — Publish crate 0.4.1 (complete)

`cargo publish --locked` from the clean candidate: **published**
(`Uploaded eggsearch v0.4.1`, `Published eggsearch v0.4.1`).

External verification:

- crates.io API: `max_version` = `newest_version` = `0.4.1`, unyanked,
  above `0.4.0`.
- `cargo search eggsearch` resolves `0.4.1`.
- `cargo package --list --locked` contains zero `plans/`, `.github/`, or
  `fuzz/` entries (package contract holds).

### WP-E — Tag and binary release (complete, green including attest)

`v0.4.1` created at exactly the qualified candidate and pushed:

- `git rev-list -n 1 v0.4.1` =
  `32769d684417567dfb30fc5adea1475cee7f0ed0`.
- `tag target == QUALIFIED_SHA == release-prep candidate SHA`: holds.

Tag-triggered `Release binaries` run `37072825644`: **success** (all 9
jobs green, including `Assemble release release output` with every step
green: asset-set validation, attest, in-workflow attestation verification,
draft creation, summary).

Attestation generation (the M004 regression point), from the run log:

- `Attestation type: Build Provenance` (SLSA, not generic).
- `Attestation created for 16 subjects`, signed via the Public Good
  Sigstore instance.
- Certificate SAN binds
  `https://github.com/eggstack/eggsearch/.github/workflows/release-binaries.yml@refs/tags/v0.4.1`,
  workflow `Release binaries`, repository `eggstack/eggsearch`.
- Draft release `v0.4.1` created (draft) with exactly 16 assets; `v0.4.0`
  untouched (`gh release view v0.4.0` → `release not found`).

### WP-F — Review and publish the immutable GitHub Release (complete)

Draft review before manual publication:

- 16 assets exactly: 7 executables + 7 adjacent `.sha256` + `install.sh`
  + `install.ps1`; filenames match the release-target convention.
- All 7 checksums verify (`sha256sum -c`: 7 × OK).
- Notes read `Default-feature release binaries for eggsearch 0.4.1.`
  (no parity overclaims, no Google News replacement, no Kagi v0 claim).
- Independent attestation sampling with `gh attestation verify`
  (exit 0 on all three): Linux executable, Windows executable,
  `install.sh`. The verified SLSA bundle (`predicateType:
  https://slsa.dev/provenance/v1`, 16 subjects) binds
  subjectAlternativeName `.../release-binaries.yml@refs/tags/v0.4.1`,
  `githubWorkflowSHA`/`sourceRepositoryDigest`
  `32769d684417567dfb30fc5adea1475cee7f0ed0`, run
  `37072825644`; attested digests match the downloaded bytes
  (Linux `156e7d0d…`, Windows exe `5269ee7d…`, `install.sh`
  `d860e743…`).

Published manually: `gh release edit v0.4.1 --draft=false` —
`draft=false`, `published=2026-10-02T22:49:33Z`, 16 assets. Immutable
thereafter; no further asset or tag operation performed.

### WP-G — External install/update and CodeGG handoff smoke (complete)

Against the public 0.4.1 artifacts with a sandboxed `HOME`
(no network-backed live searches as gates):

- Unix installer: `install.sh --version 0.4.1` fetched the published
  asset, checksumPassed (`x86_64-apple-darwin: OK`), identity-checked the
  candidate, and installed; installed binary reports `eggsearch 0.4.1`.
- `update --check` on the installed binary: `eggsearch 0.4.1 is already
  current` (exit 0).
- `packaging/release-smoke.sh <installed> 0.4.1`: exit 0 — covers
  `--version`, `--help`, stdio initialize + `tools/list` (exactly the ten
  expected tools), and loopback Streamable HTTP health + initialize +
  `tools/list` with graceful shutdown.
- `provider_status` over installed stdio: 44 ids, containing all seven
  parity ids (`brave_api`, `exa`, `tavily`, `serpapi`, `kagi`,
  `hn_algolia`, `wikipedia`); `google_news` absent (retired, no silent
  replacement).
- Typed errors, no fallback: unknown provider id →
  `provider_unavailable` / `unknown provider id`; unconfigured
  credentialed provider (`serpapi`) → `provider_unavailable` /
  `provider is disabled`.
- Windows: no local PowerShell on this host, so the `.ps1` was not
  executed locally. Covered instead by (a) native `Build, smoke, and
  checksum` success on both Windows targets inside release run
  `37072825644` (runs `release-smoke.ps1`: version + MCP stdio + health),
  (b) the downloaded `.exe` checksum OK, and (c) the `.exe` attestation
  verify exit 0 with the §2 binding. The published `install.ps1` carries
  the pinned-URL, `Get-FileHash -Algorithm SHA256`, and candidate
  version-mismatch checks asserted by `packaging/test-install.ps1`.

## 3. Invariant and compatibility review

| Invariant | Status |
|-----------|--------|
| `v0.4.0` tag/crate bytes and identity (`81a2e49`) unaltered | holds |
| `v0.4.1` tag points exactly to `QUALIFIED_SHA` | holds (`32769d6`) |
| Published `v0.4.1` release immutable, exact 16-asset contract | holds |
| Attestations cover 7 executables + 2 installers, verify to repo/workflow/commit; checksums retained | holds |
| Seven release targets, glibc 2.17 floor, MSRV 1.89 | unchanged |
| Provider inventory 44; seven parity ids; frozen disposition matrix | holds |
| Ten MCP tools; no schema/routing/cache/SSRF/updater/default-provider change | holds (workflow + metadata-only delta) |
| No post-publish force-replacement | holds |

## 4. Unresolved findings by severity

| Severity | Finding | Disposition |
|----------|---------|-------------|
| Low, process | The workflow's `Write release summary` echo lines contain unquoted backticks around the `` `gh attestation verify …` `` usage hint, so release mode executes that fragment and embeds `gh` usage text in the step summary. Cosmetic only: the assemble job is green and the summary's factual lines are correct. | Follow-up hygiene for the hardening workstream; no new release needed. |
| Low, environmental | No local `pwsh` on the release host, so `install.ps1` was not executed outside CI. Native Windows `.exe` smoke ran green on hosted runners; the `.exe` checksum and attestation were verified locally. | Recorded as a coverage note, not a gate failure. |
| Low, informational | `gh attestation verify` (gh 2.102.0) prints no human-readable success line; exit code 0 plus `--format json` is authoritative. This also explains the empty in-workflow verify output in M004-era logs. | Informational only. |
| Informational | `v0.4.0` permanently remains crate+tag only. | Permanent historical fact; downstream pins `>=0.4.1`. |

No provider, tool, or contract defect is open. No medium-or-higher finding
of any kind remains.

## 5. Registry and roadmap disposition

- M005 plan status `ready` → `closed` (this record).
- M004 plan status `blocked` → `closed`: its publication boundary is
  satisfied by the immutable 0.4.1 release carrying the identical parity
  surface. The M004 closure record stands unchanged as the account of the
  0.4.0 attest stop; a reconciliation note is appended to it pointing
  here, not rewriting it.
- M003 `conditionally closed` → `closed`: its sole publication condition
  is cleared by the immutable 0.4.1 release. Its closure record stands;
  a reconciliation note is appended pointing here.
- CodeGG legacy-search-parity corrective roadmap: all milestones closed;
  workstream closed. Downstream CodeGG parity adoption is unblocked and
  may pin `>=0.4.1`.
- Repository-hardening M002: its single named operational condition
  (first tagged release records generated/verified attestation with
  repository/workflow/commit binding plus immutable-release behavior) is
  **satisfied** by the §2 WP-E/WP-F evidence above. The evidence is
  captured and linked here without rewriting M002's historical record.
  The formal status flip stays with the hardening workstream: the
  CI-enforced planning gates (`packaging/check-planning-consistency.py`
  expectations plus `tests/static_guards.rs`
  `planning_status_consistency_no_closed_range_shorthand`) pin M002's
  roadmap/overview rows to `conditionally`, so only a hardening-owned
  reconciliation commit can flip them. M002 therefore remains
  `conditionally closed` with its condition met and evidence cited.

Recommendation: **closed**. The publication boundary for the CodeGG
parity surface is complete at `v0.4.1`; CodeGG may proceed with parity
adoption.
