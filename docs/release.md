# Release Process

Release cadence is manual and maintainer-controlled. GitHub Actions does not
publish eggsearch or publish a GitHub Release automatically. The crate is
published directly to crates.io with `cargo publish`; the binary workflow then
assembles a draft GitHub Release after all target artifacts qualify.

The first binary-enabled release, `v0.3.9`, completed this sequence on the
qualified commit `0cbbeee79a34b7f6d2d226cef535096adc58b4c3`. Qualification run
`34653366561` and tagged release run `34655458760` passed all seven targets;
the published release contains the exact 16-asset contract. Unix installer and
updater checks passed locally, and native Windows PowerShell latest/pinned
installer plus `update --check` smoke passed in run `34660182644`.

`v0.4.3` is the latest published binary release. It was qualified and published
from the exact source recorded in `plans/closure/mode-off-repository-locality-corrective/002-status.md`.
The separate Python distribution `eggsearch==0.4.4` was published to PyPI on
2026-10-28; it does not represent a new binary release. `v0.4.2` was staged by
the Eggpack producer with the 19-asset contract, attested while draft, and
published on 2026-10-07. `v0.4.1` remains the first binary release carrying the
frozen CodeGG parity surface; `v0.4.0` is crate + tag only, with no published
binaries. Corrections after a crates.io publication always require a new
version, a new changelog entry, and full re-qualification; see
`plans/closure/codegg-legacy-search-parity/005-status.md` and the current
Mode-Off corrective plan.

## Preparation

1. Ensure intended changes are on `main`.
2. Choose the next SemVer version.
3. Update `Cargo.toml`.
4. Update `CHANGELOG.md`.
5. Commit the release preparation.
6. Ensure the working tree is clean.

## Verification

```bash
make release-check
```

This runs the full routine gate (formatting, clippy, no-default-features compile
check, all-features deterministic tests), plus documentation build, release
compilation, and `cargo publish --dry-run --locked`. The publish dry-run
requires a clean working tree; commit or stash changes before running this step.

`.github/workflows/qualify-eggpack-candidate.yml` provides a separate,
non-publishing hosted qualification for an exact 40-character commit SHA. It
reuses the current Eggpack producer's seven target builds, qualification,
consumer validators, required gate, and aggregate. It creates only
run-scoped qualification artifacts and does not stage a draft, create a tag,
or publish a crate. The workflow must be available on the default branch
before it can be dispatched; inspect its run and all seven target results
before publication.

The generated `.github/workflows/release-eggpack.yml` remains the release
producer. It accepts an exact existing tag and stages the exact 19-asset draft.
The candidate must be a clean, pushed commit, and the tag must resolve to that
same commit. Do not dispatch the release producer until the matching crate
version is visible on crates.io. See `architecture/packaging.md` for the live
producer contract.

## Python package publication

The native Python distribution has its own release path and does not use root
Rust tags, crates.io publication, Eggpack, or the 19-asset binary inventory.
The controlling workstream is `plans/subsystems/python-pypi-distribution-roadmap.md`.
The first publication completed on 2026-10-28 as `eggsearch==0.4.4` from
qualified source `1135608af5d625cd72db6704adf3bcabd1a58ab1`. Publication run
`40742653291` verified the exact source and wheelhouse, published through
Trusted Publishing, and passed public binary-only installs on five native
platform families plus emulated ARMv7. See
`plans/closure/python-pypi-distribution/003-status.md` for the inventory and
evidence.

For future Python releases, configure the PyPI Trusted Publisher for owner
`eggstack`, repository `eggsearch`, workflow `release-python-pypi.yml`, and
pre-create the GitHub environment `pypi`. The workflow takes the exact candidate
SHA, successful Python qualification run ID, and synchronized package version.
It verifies the environment exists, checks the qualified source and artifact
inventory, then publishes with OIDC. Explicit workflow dispatch is the release
action; a required environment reviewer is not configured.

PyPI's long description is embedded from `bindings/python/PYTHON_README.md`,
which mirrors the root `README.md`. Updating that source changes the description
on the next version upload; an existing PyPI release cannot be overwritten.

Public install instructions are in `README.md` and `docs/python-library.md`.
The published aarch64 wheel has passed native smoke and benchmarking; results
are recorded in `plans/closure/python-pypi-distribution/004-status.md`. ARMv7
support remains experimental until physical 32-bit board evidence is gathered.
Physical SBC measurements happen after publication and do not block a future
Python release unless they reveal a compatibility defect.

## Publication and binary release

```bash
cargo publish --locked
```

Once crates.io accepts a version, that version cannot be overwritten. Any
correction requires a new version bump and another changelog entry.

After the exact crate version is visible on crates.io:

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

The `Release binaries` workflow validates the tag, checked-out commit,
`Cargo.lock`, and exact crates.io version before starting the matrix. It builds
default-feature assets for the seven targets in [installation.md](installation.md),
uses a glibc 2.17 floor for Linux GNU artifacts, runs CLI plus keyless MCP
stdio and Streamable HTTP/health/tool-list smoke tests, and writes checksums
only after smoke completes.
The ARMv7 job runs CLI smoke under QEMU; native jobs run the full protocol
smoke. macOS artifacts are unsigned.

The final job verifies every artifact from that workflow run, attaches the
reviewed `packaging/install.sh` and `packaging/install.ps1` bytes, and creates
or updates a draft release for the exact tag. It refuses to overwrite a
published release. Rerunning the workflow for the same tag is safe because all
jobs check out the tag and the assembler uploads only the newly verified
artifact set with matching names.

Every external GitHub Action in `.github/workflows/` is pinned to its full
40-character commit SHA. Keep the upstream version in the adjacent YAML
comment; when upgrading, review the intended upstream release and source at
that commit, update the SHA and version comment together, then run
`python3 packaging/check-workflow-pins.py`. `make check` enforces this policy.

Release mode creates GitHub artifact attestations for all seven executable
assets and both installers. Attestations bind artifact digests to the GitHub
repository, workflow, and candidate commit; they do not assert that software
is vulnerability-free. SHA-256 checksum files continue to support download
integrity checks and the existing updater contract. To verify a downloaded
asset with GitHub CLI, run
`gh attestation verify <asset-path> --repo eggstack/eggsearch` and inspect the
reported source repository, workflow, and commit. Release mode requires the
workflow SHA and candidate commit to match, so run it from the exact candidate
tag.

Repository release immutability is enabled for future releases. The workflow
assembles and verifies all 19 assets on a draft first; a maintainer reviews
and manually publishes that draft only after checking the asset set and
attestation. Published immutable releases cannot have their tag or assets
replaced.

The current Eggpack release sequence is:

```text
make release-check on a clean, pushed candidate commit
Eggpack exact-SHA candidate qualification: dispatch candidate_sha=<full commit SHA>
inspect all seven target, consumer, and aggregate results; confirm no release state changed
cargo publish --locked
confirm the exact version is visible on crates.io
git tag vX.Y.Z at the candidate commit && git push origin vX.Y.Z
Eggpack candidate builds: dispatch release_tag=vX.Y.Z
Release provenance: dispatch release_tag=vX.Y.Z (after the draft is staged)
inspect the 19-asset draft and publish it manually
run external Unix and Windows installer smoke against the published assets
```

`Release provenance` additionally proves, without mutating anything, that the
staged draft holds exactly the 19 expected files with digests and sizes
matching GitHub's own report, that all seven binary/checksum pairs verify, and
that `release-manifest.json` declares the same tag, source revision, and target
inventory.

Publishing stays manual. The candidate qualifier, release producer, and
provenance workflow do not publish the crate or GitHub Release: crates.io is
first, the draft is second, and a human performs the final publish. A tag whose
crate version is not yet visible on crates.io must not be dispatched — publish
the crate first.

`Release provenance` must run **while the release is still a draft**: its guard
rejects an already-published release, so the ordering above is not advisory.
That guard also means the job needs a push-scoped token. GitHub answers
`GET /releases/tags/{tag}` with 404 for a draft unless the token carries push
access, so a read-scoped token fails closed with `release not found` even when
the draft exists. The job therefore requests `contents: write` while issuing
only GET requests; `packaging/check-contract.sh` enforces that it stays a
non-writer by rejecting any mutating release command and by requiring the write
scope to remain job-scoped rather than workflow-wide.

Installers never elevate and only use Cargo for unsupported targets or a
confirmed HTTP 404 for the exact binary. They fail closed on all other download,
integrity, identity, or version errors. The release smoke still starts `mcp
serve` in the foreground and requests its bounded graceful shutdown; service
registration is exercised separately through the CLI manager paths.

The installed `eggsearch update` command consumes this same seven-target asset
contract. crates.io `crate.max_stable_version` is its version authority; it then
requests only the matching `vX.Y.Z` release asset and checksum, verifies the
candidate, and replaces the currently running executable. Exact asset 404 or an
unsupported host may use an isolated exact-version Cargo build. Other network or
integrity failures never fall back to compilation. A normal update restarts
only a previously healthy registered persistent service; stdio-only and
stopped services remain untouched. See [Managed service](service.md).

The deployment closure also runs `eggsearch integrate list` and validates
rendered stdio/HTTP forms for CodeGG, Zed, Codex, Claude Code, VS Code, Cursor,
and OpenCode. Applied paths verify MCP initialize and `tools/list`; strict JSON
edits are atomic and backup-preserving. Official MCP Registry metadata is
deferred until its package schema can represent the complete multi-architecture
release contract without implying unsupported remote authentication.

## Routine verification

The daily developer command is:

```bash
make check
```

This runs formatting, clippy, feature compilation, the deterministic
test suite, repository hygiene, and packaging contract checks. It does
not require a clean tree and does not build release artifacts.

## Where the release gate is defined

| Location | Purpose |
|----------|---------|
| `Makefile` / `make check` | Routine deterministic local gate |
| GitHub Actions / `make ci` | Remote repetition of routine gate |
| `Makefile` / `make release-check` | Local packaging gate |
| `cargo publish --locked` | Explicit maintainer publication |
| `.github/workflows/qualify-eggpack-candidate.yml` | Hosted exact-SHA candidate qualification; no tag or release writes |
| `release/eggpack/` + `.github/workflows/release-eggpack.yml` | Eggpack producer authority and the generated release workflow (build, qualify, validate, aggregate, draft staging) |
| `.github/workflows/release-drift.yml` | `eggpack ci check` drift guard on every push and pull request |
| `.github/workflows/release-provenance.yml` | Read-only verification of the staged draft plus artifact attestation |
