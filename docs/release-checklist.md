# Release Checklist

Short operational checklist. The authoritative release process lives in
[`docs/release.md`](release.md).

## Pre-release

- [ ] All intended changes are on `main`
- [ ] Version bumped in `Cargo.toml`
- [ ] `CHANGELOG.md` updated
- [ ] `make release-check` passes from a clean tree

## Publication

- [ ] `cargo publish --locked`
- [ ] Verify crates.io listing at <https://crates.io/crates/eggsearch>

## Post-publication

- [ ] `git tag vX.Y.Z`
- [ ] `git push origin vX.Y.Z`
- [ ] Optionally create a GitHub release with changelog excerpt
