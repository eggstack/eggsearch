# Release Process

Release cadence is manual and maintainer-controlled. GitHub Actions does not
publish eggsearch. The crate is published directly to crates.io with
`cargo publish`. Optional provider smoke tests do not block a core release.

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

This runs the full routine gate (formatting, clippy, feature compilation,
deterministic tests), plus documentation build, release compilation, and
`cargo publish --dry-run --locked`. The publish dry-run requires a clean
working tree; commit or stash changes before running this step.

## Publication

```bash
cargo publish --locked
```

Once crates.io accepts a version, that version cannot be overwritten. Any
correction requires a new version bump and another changelog entry.

## Post-publication

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

Creating a GitHub release from the changelog is optional and manual.

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
