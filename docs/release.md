# Release Process

Release cadence is manual and maintainer-controlled. GitHub Actions does not
publish eggsearch or publish a GitHub Release automatically. The crate is
published directly to crates.io with `cargo publish`; the binary workflow then
assembles a draft GitHub Release after all target artifacts qualify.

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

For an existing tag, use Actions → Release binaries → Run workflow and enter
the exact `vX.Y.Z` tag. A tag whose crate version is not yet visible on crates.io
fails in preflight with instructions to publish and wait for the registry
index. Publish the draft release manually after reviewing its assets and
checksums.

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

This runs formatting, clippy, feature compilation, and the deterministic
test suite. It does not require a clean tree, does not build release
artifacts, and does not run packaging checks.

## Where the release gate is defined

| Location | Purpose |
|----------|---------|
| `Makefile` / `make check` | Routine deterministic local gate |
| GitHub Actions / `make ci` | Remote repetition of routine gate |
| `Makefile` / `make release-check` | Local packaging gate |
| `cargo publish --locked` | Explicit maintainer publication |
| `.github/workflows/release-binaries.yml` | Tagged binary qualification and draft assembly |
