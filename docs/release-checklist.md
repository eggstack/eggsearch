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

## Initial Python package publication — completed 2026-10-28

- [x] Confirm M001 and M002 exact-SHA qualification and root CI are green for `1135608af5d625cd72db6704adf3bcabd1a58ab1`
- [x] Configure PyPI Trusted Publishing for `eggstack/eggsearch`, workflow `release-python-pypi.yml`, environment `pypi`
- [x] Create GitHub environment `pypi` before dispatching; no required reviewer was configured per maintainer direction
- [x] Dispatch run `40742653291` with the qualified SHA, Python qualification run `40729011504`, and version `0.4.4`
- [x] Confirm public binary-only installs on five native platforms and emulated ARMv7; see `plans/closure/python-pypi-distribution/003-status.md`
- [ ] Run M004 benchmarks on native aarch64 and ARMv7 SBC devices after publication

## Future Python package publications

- [ ] Qualify the exact candidate source and verify a synchronized, unused Python version
- [ ] Confirm PyPI Trusted Publishing and the pre-created GitHub `pypi` environment remain configured
- [ ] Dispatch `.github/workflows/release-python-pypi.yml` with the exact SHA, successful qualification run ID, and version
- [ ] Confirm public binary-only installs and record the release evidence before announcing availability
