---
name: eggsearch-release
description: Use when preparing or cutting an eggsearch release. Covers pre-release checks, versioning rules, CI pipeline, and publishing steps.
---

# eggsearch Release Skill

Use when preparing or cutting an eggsearch release. Covers pre-release checks, versioning rules, CI pipeline, and publishing steps.

## Pre-release Command Sequence

Run from repository root. Every command must pass. Do not skip steps.

```bash
make release-check
```

This runs the full routine gate (fmt, clippy, no-default-features compile check, all-features tests), plus documentation build, release compilation, and `cargo publish --dry-run --locked`.

Or run the individual commands:

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --locked --all-features --no-deps
cargo build --locked --release
cargo publish --dry-run --locked
```

## Publication

```bash
cargo publish --locked
```

Once crates.io accepts a version, that version cannot be overwritten. Any correction requires a new version bump and another changelog entry.

## Post-publication

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

The tagged `Release binaries` workflow assembles a draft GitHub Release after
all target jobs and checksums pass; publish that draft manually after review.

## CI Pipeline

| Job | What it runs |
|-----|-------------|
| `ci` | `make ci` — fmt, clippy, no-default-features compile check, all-features tests |
| `Release binaries` | Tagged/manual workflow — preflight, seven-target binary qualification, checksums, draft assembly |

## Binary release workflow

After `cargo publish --locked` succeeds and the exact version is visible on
crates.io, tag and push `vX.Y.Z`. `.github/workflows/release-binaries.yml`
validates the tag, package version, tagged commit, clean checkout, lockfile,
and registry visibility before building. It produces default-feature assets
using the contract in `packaging/release-targets.txt`, runs native CLI/MCP
smoke where possible, qualifies ARMv7 under QEMU, and assembles a draft
release with checksums and the reviewed installers. It refuses to overwrite a
published release and never publishes the crate.

Run `make packaging-check` locally when changing release target mappings or
installer behavior. The routine `make check` remains network-free; release
workflow jobs are the only place that require GitHub/crates.io and hosted
cross-platform runners.

## Feature Flags

| Flag | Purpose | Default? |
|------|---------|----------|
| (none) | Minimal build | Yes |
| `pdf` | PDF extraction | No |
| `browser` | Headless Chrome/Chromium rendering | No |
| `mock` | Test-only mock engine | No |
| `live-smoke` | Live network tests (opt-in) | No |

## Version Rules

- Version in `Cargo.toml` must be bumped before tagging
- `CHANGELOG.md` must be updated
- `cargo publish --dry-run --locked` must pass
- The `--locked` flag is mandatory (lockfile must match resolved deps)

## Branch Protection

Recommended required check for `main`:
- `CI / ci` — the single required CI job
- Configure in GitHub UI; the pre-release command sequence substitutes if settings cannot be modified

## Live-smoke Policy

Live smoke tests are opt-in and never part of default CI:
```bash
cargo test --features live-smoke --test corpus_runner -- --ignored
```

- A release must not be blocked solely because a live smoke test fails against a third-party provider
- Reproduce locally to distinguish third-party drift from local regression

## Native Forge Smoke Tests

Native forge smoke tests (`tests/native_forge_smoke.rs`) exercise the adapter path directly with configured API tokens. These are maintainer-only diagnostics, not release evidence. Run provider-specific tests:

```bash
# GitHub (requires GITHUB_TOKEN and GITHUB_SLASH_REF)
GITHUB_TOKEN=... \
GITHUB_SLASH_REF=fixture/slash-ref \
make native-forge-smoke-github

# GitLab (requires GITLAB_TOKEN)
GITLAB_TOKEN=... \
make native-forge-smoke-gitlab

# Codeberg (requires CODEBERG_TOKEN)
CODEBERG_TOKEN=... \
make native-forge-smoke-codeberg

# Gitea/Forgejo (requires GITEA_TOKEN and GITEA_INSTANCE_URL)
GITEA_TOKEN=... \
GITEA_INSTANCE_URL=... \
make native-forge-smoke-gitea

# All providers (requires every credential)
make native-forge-smoke-all
```

## Makefile Targets

| Target | Command | Purpose |
|--------|---------|---------|
| `check` | `fmt + clippy + feature-check + test` | Local CI gate |
| `ci` | `check` | Alias for `check` |
| `release-check` | `check + docs-check + release-build + publish-check` | Pre-release gate |
| `docs-check` | `RUSTDOCFLAGS="-D warnings" cargo doc --locked --all-features --no-deps` | Docs check |
| `release-build` | `cargo build --locked --release` | Release build |
| `publish-check` | `cargo publish --dry-run --locked` | Pre-publish check |
| `bench-check` | `cargo bench --locked --all-features --bench perf --no-run` | Compile-check benches without running |
| `live-smoke` | `cargo test --features live-smoke --test corpus_runner -- --ignored` | Live network tests |
| `fuzz-smoke` | Quick fuzz runs for 3 targets | Fuzz smoke test |

## Pre-release Checklist

1. All CI checks green
2. Version bumped in Cargo.toml
3. CHANGELOG.md updated
4. `make release-check` passes from a clean tree
5. `cargo publish --locked` succeeds
6. `git tag vX.Y.Z` created and pushed
