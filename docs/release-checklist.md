# Release Checklist

This document is a short operational checklist for cutting a release. The
authoritative release process — including the exact pre-release command
sequence, the required CI checks, and the live-smoke policy — lives in
[`docs/release.md`](release.md). Read that document before tagging.

## Pre-release

- [ ] All required CI jobs are green on the exact release commit
      (see [`docs/release.md`](release.md#required-ci-checks))
- [ ] Manual native forge smoke workflow passes every required provider on the
      exact release subject and emits the combined evidence manifest
- [ ] `docs/release-verification.md` records the exact R/E SHAs and artifact
      hashes, or explicitly remains provisional while evidence is pending
- [ ] `make check` passes locally
- [ ] `cargo publish --dry-run --locked` passes
- [ ] Version bumped in `Cargo.toml`
- [ ] `CHANGELOG.md` updated with all notable changes since last release

## Release

- [ ] Create git tag: `git tag v{VERSION}`
- [ ] Push tag: `git push origin v{VERSION}`
- [ ] Wait for the `release-build` and `publish-check` jobs to run against the tag
- [ ] Publish to crates.io: `cargo publish`
- [ ] Create GitHub release with changelog excerpt

## Post-release

- [ ] Verify crates.io listing at <https://crates.io/crates/eggsearch>
- [ ] Update any external documentation or references to the old version
- [ ] Announce if applicable

## Verification

```bash
# Full CI gate before release
make check

# Publish dry-run (catches packaging issues; --locked is mandatory)
cargo publish --dry-run --locked

# After tagging, confirm the tag is correct
git tag -l 'v*'
git log --oneline -5
```

If `docs/release.md`, the `Makefile`, and `.github/workflows/ci.yml` ever
disagree, `docs/release.md` is the source of truth and the others must be
updated to match.
