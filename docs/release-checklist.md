# Release Checklist

## Pre-release

- [ ] Version bumped in `Cargo.toml`
- [ ] `CHANGELOG.md` updated with all notable changes since last release
- [ ] `make check` passes (fmt, clippy, test-all, test-no-default, schema-corpus, docs-tests)
- [ ] CI green on `main` branch

## Release

- [ ] Create git tag: `git tag v{VERSION}`
- [ ] Push tag: `git push origin v{VERSION}`
- [ ] Wait for CI to build release artifacts
- [ ] Publish to crates.io: `cargo publish`
- [ ] Create GitHub release with changelog excerpt

## Post-release

- [ ] Verify crates.io listing at https://crates.io/crates/eggsearch
- [ ] Update any external documentation or references to the old version
- [ ] Announce if applicable

## Verification

```bash
# Full CI gate before release
make check

# Publish dry-run (catches packaging issues)
cargo publish --dry-run

# After tagging, confirm the tag is correct
git tag -l 'v*'
git log --oneline -5
```
