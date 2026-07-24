# Release Process

This document is the authoritative release process for eggsearch. It defines the
exact pre-release command sequence, the required CI checks, and the policy for
optional live network smoke tests. The corresponding `Makefile` targets and
the `.github/workflows/ci.yml` workflow intentionally mirror this document; if
one of the three diverges from the others, fix the inconsistency in the same
commit that changes the release gate.

## Pre-release command sequence

Run the following commands from the repository root before tagging a release.
Every command must pass. Do not skip steps. If a step fails, fix the underlying
issue and re-run the full sequence from the top.

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
cargo test --locked --all-features --test docs_config_snippets --test docs_provider_inventory --test docs_tool_names --test docs_safety_vocabulary
cargo build --release
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
cargo publish --dry-run --locked
```

The `make check` target runs the full offline gate — fmt, clippy, all feature
matrix tests, schema-corpus, docs-tests, release build, docs build, and
publish dry-run — in a single command and is the recommended entry point:

```bash
make check
```

The publish dry-run at the end is required: it exercises `cargo publish` in
dry-run mode without `--allow-dirty` and verifies that the on-disk lockfile
matches the resolved dependency graph. The `--locked` flag is mandatory.

## Feature flag behavior

| Flag | Purpose | Default? | In default CI? |
|------|---------|----------|----------------|
| (none) | Minimal build: no optional features enabled. | Yes | Yes (via `--no-default-features`) |
| `pdf` | Enables PDF text extraction via `lopdf`. | No | Yes |
| `mock` | Test-only mock engine harness. Required by integration and corpus tests. | No | Yes |
| `live-smoke` | Live network smoke tests against real providers. Implies `mock`. | No | No (opt-in) |

The default feature set is intentionally minimal. Production builds should
build with the default features unless they need PDF extraction, in which case
they should add `--features pdf`. Tests that exercise provider contracts or
corpus fixtures must be run with `--features mock`; the test harness will not
compile without it. The `live-smoke` feature is opt-in and must never be
required for default CI: live provider behavior is third-party and can drift
without indicating a local regression.

## Required CI checks

Before tagging a release, the following GitHub Actions jobs must be green on
the exact release commit. These are the same jobs the `.github/workflows/ci.yml`
workflow defines; do not weaken or remove them.

| Job | Purpose |
|-----|---------|
| `check` matrix | `cargo check` against all four feature combinations. |
| `test` matrix | `cargo test --locked` against all four feature combinations. |
| `clippy` | `cargo clippy --all-targets --all-features -- -D warnings`. |
| `fmt` | `cargo fmt --check`. |
| `schema-corpus` | The six regression corpus test binaries. |
| `docs-contract` | Documentation snippets, provider/tool/safety contracts, workflow/release contracts, and static guards. |
| `benchmarks` | `cargo bench --locked --all-features --bench perf --no-run`. |
| `hardening` | Property tests, dispatch fault injection, and adversarial corpus validation. |
| `release-build` | `cargo build --release` to confirm the release artifact compiles. |
| `publish-check` | `cargo publish --dry-run --locked` to confirm packaging. |
| `docs` | `RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps`. |

The clippy job in CI is intentionally identical to the `make clippy` target.
If a local developer reports a clippy warning that CI does not, the CI command
is wrong, not the local command.

### Branch protection and required checks

Branch protection rules and required-check settings are managed in the GitHub
repository or organization settings. They are not enforceable from repository
code alone. Recommended required checks for `main` are exactly the jobs listed
above, with the `check` and `test` matrices required to fully succeed (all
feature combinations). The repository owner should configure these in the
GitHub UI; if a release is being cut from a fork or a repository where those
settings cannot be modified, the pre-release command sequence above substitutes
for the required-check enforcement.

## Live-smoke policy

Live smoke tests (`cargo test --features live-smoke --test corpus_runner -- --ignored`)
exercise real upstream search providers and may require network access and
third-party API credentials. The policy is:

- Live smoke tests must remain ignored and opt-in. They are not part of
  default CI and must not be required for any release gate.
- A release must not be blocked solely because a live smoke test fails against
  a third-party provider. Provider UI changes, rate limits, and region
  restrictions can all produce transient failures that do not indicate a
  local regression.
- When a live smoke test fails, reproduce it locally to determine whether
  the failure is a third-party drift (record it in a release note or issue)
  or a local regression (block the release and fix the code).
- Live smoke tests must never write persistent state or require credentials
  beyond those already documented in `docs/provider-setup.md`.

## Release steps

Before tagging or promoting a release, complete the separate native forge
evidence protocol in [`release-verification.md`](release-verification.md). The
manual workflow must run against the exact code-bearing subject SHA and must
pass GitHub, GitLab, Codeberg, and Gitea with structured native evidence. A
scheduled smoke run or fallback repository result is diagnostic only.

The high-level release steps are:

1. Confirm CI is green on the exact release commit (see "Required CI checks"
   above). Do not rely on commit-message claims; check the workflow run
   directly.
2. Bump the version in `Cargo.toml` and add a corresponding entry at the top
   of `CHANGELOG.md` listing every notable change since the last release.
3. Run the full pre-release command sequence (or `make check`) locally to
   catch anything CI missed.
4. Create a git tag: `git tag v{VERSION}` (the `v` prefix is required).
5. Push the tag: `git push origin v{VERSION}`.
6. Wait for the release-build and publish-check jobs to run against the tag.
7. Publish to crates.io: `cargo publish` (the `publish-check` job in CI
   already verified the dry-run; this is the real publish).
8. Create a GitHub release with the changelog excerpt for the new version.
9. Verify the crates.io listing at <https://crates.io/crates/eggsearch>.

## After the release

- Confirm the crates.io listing reflects the new version.
- Confirm the GitHub release is published with the correct tag and notes.
- If any live smoke tests were observed drifting during the release window,
  record them in the release notes or open follow-up issues.
- No persistent release state is created locally; the git tag and crates.io
  record are the only sources of truth.

## Where the release gate is defined

The release gate is defined in three places, intentionally kept in sync:

| Location | Purpose |
|----------|---------|
| `Makefile` | Local offline gate. `make check` runs the full sequence. |
| `.github/workflows/ci.yml` | Remote CI gate. Must mirror the Makefile exactly. |
| `docs/release.md` (this file) | Authoritative documentation. |

If a developer wants to change a flag, a command, or a check, they must update
all three in the same change. Drift between these three is a release blocker.
