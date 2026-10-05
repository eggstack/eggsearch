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

`v0.4.1` is the current published release and the first binary release carrying
the frozen CodeGG parity surface; `v0.4.0` is crate + tag only, with no
published binaries. It completed the same sequence on fix-prep commit
`32769d6` with a green `mode=qualify` run, a tag-triggered release run that
included artifact attestations and in-workflow `gh attestation verify`, and an
immutable 16-asset release with SLSA provenance. Corrections after a crates.io
publication always require a new version, a new changelog entry, and full
re-qualification; see `plans/closure/codegg-legacy-search-parity/005-status.md`.

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

Before publishing the crate, qualify the exact immutable candidate commit with
Actions → Release binaries → Run workflow, selecting `mode=qualify` and
`ref=<commit SHA>`. Qualification runs the complete seven-target matrix and
uploads a clearly labelled qualification-only artifact; it never queries
crates.io or changes a GitHub Release. Record the `QUALIFIED_SHA` from the
workflow summary and do not tag a different commit.

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
assembles and verifies all 16 assets on a draft first; a maintainer reviews
and manually publishes that draft only after checking the asset set and
attestation. Published immutable releases cannot have their tag or assets
replaced.

The subsequent release sequence is:

```text
make release-check
push the exact release-candidate commit
Release binaries: mode=qualify, ref=<exact SHA>
inspect the qualification-only artifact and QUALIFIED_SHA
cargo publish --locked
confirm the exact version is visible on crates.io
git tag vX.Y.Z at QUALIFIED_SHA && git push origin vX.Y.Z
Release binaries: mode=release, tag=vX.Y.Z
inspect the 16-asset draft release and publish it manually
run external Unix and Windows installer smoke against the published assets
```

For release mode, the workflow checks the exact `vX.Y.Z` tag, package version,
tag commit, clean tree, required release inputs, and crates.io visibility before
starting the matrix. A tag whose crate version is not yet visible on crates.io
fails in preflight. Qualification and release check out the same resolved SHA
and use the same build, smoke, checksum, and exact asset-set validation.

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
| `.github/workflows/release-binaries.yml` | Non-publishing qualification and tagged draft assembly |
