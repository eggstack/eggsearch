# eggsearch Release Skill

Use when preparing or cutting an eggsearch release. Covers pre-release checks, versioning rules, CI pipeline, and publishing steps.

## Pre-release Command Sequence

Run from repository root. Every command must pass. Do not skip steps.

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo test --locked --no-default-features
cargo test --locked --features mock
cargo test --locked --features pdf
cargo test --locked --features mock --test schema_identity_registry
cargo test --locked --features mock --test fetch_safety
cargo test --locked --features mock --test security_applicability_corpus
cargo test --locked --features mock --test research_evidence_corpus
cargo test --locked --features mock --test recipes_next_actions
cargo test --locked --features mock --test evidence_bundle_handoff
cargo test --locked --all-features --test docs_config_snippets --test docs_provider_inventory --test docs_tool_names
cargo build --release
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
cargo publish --dry-run --locked
```

Or use the single-command CI gate:

```bash
make check
```

## CI Pipeline

| Job | What it runs |
|-----|-------------|
| `check` | `cargo check` × 4 feature combos |
| `test` | `cargo test --locked` × 4 feature combos |
| `clippy` | `cargo clippy --all-targets --all-features -- -D warnings` |
| `schema-corpus` | 6 regression test binaries |
| `docs-contract` | 4 documentation contract tests |
| `fmt` | `cargo fmt --check` |
| `release-build` | `cargo build --release` |
| `publish-check` | `cargo publish --dry-run --locked` |
| `hardening` | Property tests, fault injection, adversarial corpus |
| `docs` | `RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps` |

## Required CI Checks for Release

Before tagging, these must be green on the exact commit:

- Formatting (`cargo fmt --check`)
- Clippy (zero warnings)
- Default tests, all-features tests, no-default-features tests, mock feature tests, PDF feature tests
- Schema/corpus tests
- Documentation contract tests
- Release build
- Docs build (no warnings)
- Publish dry-run

## Feature Flags

| Flag | Purpose | Default? |
|------|---------|----------|
| (none) | Minimal build | Yes |
| `pdf` | PDF extraction | No |
| `mock` | Test-only mock engine | No |
| `live-smoke` | Live network tests (opt-in) | No |

## Version Rules

- Version in `Cargo.toml` must be bumped before tagging
- `CHANGELOG.md` must be updated
- `cargo publish --dry-run --locked` must pass
- The `--locked` flag is mandatory (lockfile must match resolved deps)

## Branch Protection

Recommended required checks for `main`:
- All CI jobs listed above
- `check` and `test` matrices must fully succeed (all feature combinations)
- Configure in GitHub UI; pre-release command sequence substitutes if settings cannot be modified

## Live-smoke Policy

Live smoke tests are opt-in and never part of default CI:
```bash
cargo test --features live-smoke --test corpus_runner -- --ignored
```

- A release must not be blocked solely because a live smoke test fails against a third-party provider
- Reproduce locally to distinguish third-party drift from local regression

## Pre-release Checklist

1. All CI checks green
2. Version bumped in Cargo.toml
3. CHANGELOG.md updated
4. `make check` passes locally
5. Release build compiles
6. Publish dry-run passes
7. Documentation is current
8. No unbounded response bodies in forge paths
9. No unbounded Git subprocess output
10. Evidence roles populated on all search result cards
