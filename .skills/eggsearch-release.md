# eggsearch Release Skill

## Pre-Release Checklist

1. `make check` passes (fmt + clippy + tests + schema-corpus + docs-tests + release-build + docs + publish-check)
2. Version bumped in `Cargo.toml`
3. `CHANGELOG.md` updated with new version entry
4. `cargo publish --dry-run --locked` succeeds
5. All required GitHub Actions jobs are green on the exact release commit
6. README stays concise; release-facing detail belongs in `docs/config.md`, `docs/safety.md`, `docs/tool-matrix.md`, `docs/agent-workflows.md`, `docs/architecture/codegg-contract.md`, and `docs/release.md`

The full authoritative pre-release command sequence, required CI checks, and
live-smoke policy live in `docs/release.md`. Read it before cutting a release;
this skill is a quick reference and points back to that document.

## Release Steps

```bash
# 1. Run full CI gate
make check

# 2. Dry-run publish check
cargo publish --dry-run --locked

# 3. Tag and push
git tag v{VERSION}
git push origin v{VERSION}

# 4. Publish to crates.io
cargo publish
```

## Versioning

Follows Semantic Versioning. Breaking changes to MCP tool schemas require a major version bump.

### Breaking Changes (major bump)
- Removing or renaming enum variants
- Removing or renaming struct fields
- Changing serialized enum string values
- Changing deterministic ID algorithms
- Removing `WarningCode` or `FetchRankReason` variants

### Non-breaking (minor/patch)
- New enum variants (appended)
- New optional struct fields (`skip_serializing_if`)
- New warning codes, reason codes, tool capabilities
- New `server_capabilities` flags

## CI Pipeline

The CI pipeline in `.github/workflows/ci.yml` intentionally mirrors the
`Makefile` release gate. CI clippy uses the same flags as `make clippy`
(`--all-targets --all-features -- -D warnings`); if you change one, change the
other in the same commit.

| Job | What it runs |
|-----|-------------|
| check | `cargo check` × 4 feature combos |
| test | `cargo test --locked` × 4 feature combos |
| clippy | `cargo clippy --all-targets --all-features -- -D warnings` |
| schema-corpus | 6 regression test binaries |
| docs-contract | 3 documentation contract tests |
| fmt | `cargo fmt --check` |
| release-build | `cargo build --release` |
| publish-check | `cargo publish --dry-run --locked` |
| docs | `RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps` |

Branch protection and required-check settings are managed in the GitHub UI;
they are not enforceable from repository code alone. See `docs/release.md` for
the recommended required-check list and the live-smoke policy.

## Feature Flags

| Flag | Purpose |
|------|---------|
| `mock` | Test-only mock engine harness; required for integration/corpus tests |
| `pdf` | PDF text extraction via `lopdf` |
| `live-smoke` | Live network smoke tests (requires `mock`); ignored by default |

## Publishing Metadata

```toml
[package]
name = "eggsearch"
keywords = ["mcp", "search", "metasearch", "cli", "ai-agent"]
categories = ["command-line-utilities", "web-programming"]
```

Ensure `README.md`, `LICENSE`, `CHANGELOG.md`, and `docs/**/*.md` are in the `include` list.
