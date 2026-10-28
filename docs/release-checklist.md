# Release Checklist

Short operational checklist. The authoritative release process lives in
[`docs/release.md`](release.md).

## Pre-release

- [ ] All intended changes are on `main`
- [ ] Version bumped in `Cargo.toml`
- [ ] `CHANGELOG.md` updated
- [ ] `make release-check` passes from a clean tree
- [ ] `make packaging-check` passes
- [ ] Push the exact candidate commit and run `Release binaries` with `mode=qualify` and `ref=<exact SHA>`
- [ ] Inspect the qualification-only artifact and record `QUALIFIED_SHA`

## Publication

- [ ] `cargo publish --locked`
- [ ] Verify crates.io listing at <https://crates.io/crates/eggsearch>
- [ ] Verify the exact version is visible on crates.io before tagging
- [ ] Confirm `git rev-parse vX.Y.Z^{commit}` equals `QUALIFIED_SHA`

## Post-publication

- [ ] `git tag vX.Y.Z`
- [ ] `git push origin vX.Y.Z`
- [ ] Confirm the Eggpack-generated `Eggpack candidate builds` workflow creates a complete draft release
- [ ] Review binary checksums and publish the draft release manually
- [ ] Verify the published release has exactly 19 assets (7 executables, 7 `.sha256` sidecars, `install.sh`, `install.ps1`, `release-manifest.json`, `install-exact.sh`, `install-exact.ps1`) and run external Unix/Windows installer smoke

## Separate Python package publication

- [ ] Confirm M001 and M002 exact-SHA qualification and root CI are green
- [ ] Configure PyPI Trusted Publishing for `eggstack/eggsearch`, workflow `release-python-pypi.yml`, environment `pypi`
- [ ] Configure GitHub environment `pypi` with a required human reviewer
- [ ] Dispatch the Python publication workflow with the qualified SHA, Python qualification run ID, and synchronized version
- [ ] Approve the `pypi` environment only after the verified inventory is ready
- [ ] Confirm public binary-only installs and the M003 closure record before announcing availability
- [ ] Schedule M004 benchmarks on native SBC devices after publication
