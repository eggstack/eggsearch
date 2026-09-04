# Release Checklist

Short operational checklist. The authoritative release process lives in
[`docs/release.md`](release.md).

## Pre-release

- [ ] All intended changes are on `main`
- [ ] Version bumped in `Cargo.toml`
- [ ] `CHANGELOG.md` updated
- [ ] `make release-check` passes from a clean tree
- [ ] `make packaging-check` passes

## Publication

- [ ] `cargo publish --locked`
- [ ] Verify crates.io listing at <https://crates.io/crates/eggsearch>
- [ ] Verify the exact version is visible on crates.io before tagging

## Post-publication

- [ ] `git tag vX.Y.Z`
- [ ] `git push origin vX.Y.Z`
- [ ] Confirm `Release binaries` workflow creates a complete draft release
- [ ] Review binary checksums and publish the draft release manually
