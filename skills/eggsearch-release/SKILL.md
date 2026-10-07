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

Once crates.io accepts a version, that version cannot be overwritten. Any correction requires a new version bump and another changelog entry. The exact commit qualified before publication must be the commit tagged for that version.

## Post-publication

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

The Eggpack-generated `Eggpack candidate builds` workflow assembles a **draft**
GitHub Release after all seven targets qualify and validate.

Order matters and is not interchangeable:

1. Review the 19-asset draft.
2. Dispatch `Release provenance` for the same tag to verify and attest the staged
   bytes.
3. **Only then** publish the draft manually.

`Release provenance` **refuses to run against an already-published release** — it
exists to attest unpublished staged bytes, and its guard exits non-zero if
`isDraft` is not `true`. Attesting after publication is therefore impossible, and
skipping it leaves a public release with no provenance. If you already published,
that release cannot be retroactively attested; cut a new version instead.

Before publishing the draft, confirm:

- The release contains exactly 19 assets: 7 executables, 7 adjacent `.sha256`
  files, the reviewed `install.sh` and `install.ps1`, plus Eggpack's additive
  producer evidence `release-manifest.json`, `install-exact.sh`, and
  `install-exact.ps1`.
- `Release provenance` completed and its `gh attestation verify` steps passed.
  Re-verify independently after download:
  `gh attestation verify <asset-path> --repo eggstack/eggsearch`.
- Staging refused to overwrite an already published release, no workflow
  publishes the crate, and no workflow publishes the GitHub Release.

Once crates.io accepts a version it is immutable. A correction requires a new
version bump, a new changelog entry, and a full re-qualification — `v0.4.0`
stayed crate + tag only, and `v0.4.1` is the first binary release carrying the
frozen parity surface. Record the qualification run, release run, asset count,
and provenance evidence in the owning `plans/closure/` record; a green CI badge
is not closure evidence.

## CI Pipeline

| Job | What it runs |
|-----|-------------|
| `ci` | `make ci` — fmt, clippy, no-default-features compile check, all-features tests |
| `Eggpack candidate builds` | Generated release workflow — resolve exact tag, seven-target build/qualify/consumer-validate, required gate, aggregate, draft staging |
| `Release drift guard` | `eggpack ci check` over the generated workflow plus the packaging contract guard |
| `Release provenance` | Read-only staged-draft verification plus artifact attestation over the exact bytes |
| `Egress feature qualification` | Non-publishing `egress` compile lane (`.github/workflows/egress-feature-qualify.yml`) — exact target-set preflight against `packaging/release-targets.txt`, per-target `cargo check --locked --features egress` across the 7 release targets, MSRV 1.89 all-features check |

## Binary release workflow

Release production is owned by Eggpack. The authority set lives in
`release/eggpack/`, the workflow at `.github/workflows/release-eggpack.yml` is
**generated**, and `.github/workflows/release-drift.yml` fails required CI if
the two drift apart.

`.github/workflows/release-eggpack.yml` is never edited by hand. After changing
anything under `release/eggpack/`:

```bash
python3 packaging/gen-release-workflow-shape.py     # re-derive the shape
make producer-drift                                 # render + drift-check locally
```

`make producer-drift` needs the pinned tool; get it with
`cargo install --git https://github.com/eggstack/eggpack --rev <revision from
release/eggpack/github-policy.json> --locked eggpack-cli`.

The release run is `workflow_dispatch`-only and requires an exact **existing**
`release_tag`, so the crate must already be on crates.io before dispatching. It
resolves the tag, verifies the checkout is that exact commit, builds all seven
targets, qualifies them, runs the required consumer validator per target,
enforces the required gate, aggregates into one finalized root plus
`release-manifest.json`, and stages a **draft** release.

ARMv7 is classified `structural` (no hosted ARMv7 builder), so its runtime proof
comes from the required `packaging/validate-armv7-candidate.py` rather than from
core qualification; a failure there fails the release.

Staging is draft-only and fail-closed: absent asset uploads, byte-identical
assets are reused, a differing asset or an unexpected remote asset fails, a
published release fails. There is no `--clobber` and no publish path. To recover
from a refused draft, inspect it, delete the stale draft only when
intentionally restarting the candidate, and rerun the same tag and source.

Run `make packaging-check` locally when changing release target mappings,
installer behavior, or embedded service assets. `make release-check` also runs
the release-candidate tree/version/syntax gate and publish dry-run. The routine
`make check` remains network-free; release workflow jobs are the only place
that require GitHub/crates.io and hosted cross-platform runners.

The installed `eggsearch update` command consumes the same release target and
asset contract. It uses crates.io `crate.max_stable_version`, requests the exact
`vX.Y.Z` asset/checksum, verifies both checksum and candidate identity, and only
uses an isolated exact-version Cargo build for unsupported targets or confirmed
asset HTTP 404. It never invokes elevation. A normal update restarts only a
previously healthy registered persistent service through its existing manager;
stdio-only and stopped services remain untouched.

## Feature Flags

| Flag | Purpose | Default? |
|------|---------|----------|
| (none) | Minimal build | Yes |
| `pdf` | PDF extraction | No |
| `browser` | Headless Chrome/Chromium rendering | No |
| `mock` | Test-only mock engine | No |
| `egress` | Listener-free HTTP/SOCKS proxy-chain route for provider upstreams only (source-build opt-in, never in prebuilt/default binaries) | No |
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
| `check` | `fmt + clippy + feature-check + test + hygiene + dependency-policy + packaging-check` | Local CI gate |
| `ci` | `check` | Alias for `check` |
| `release-check` | `check + release-candidate-check + docs-check + release-build + publish-check` | Pre-release gate |
| `release-candidate-check` | `./packaging/release-validate.sh candidate` | Release tree/version/syntax gate |
| `docs-check` | `RUSTDOCFLAGS="-D warnings" cargo doc --locked --all-features --no-deps` | Docs check |
| `release-build` | `cargo build --locked --release` | Release build |
| `publish-check` | `cargo publish --dry-run --locked` | Pre-publish check |
| `packaging-check` | `./packaging/check-contract.sh` | Release target/input/install contract drift |
| `bench-check` | `cargo bench --locked --all-features --bench perf --no-run` | Compile-check benches without running |
| `fuzz-smoke` | `cargo fuzz run` for `validate_url`, `sanitize_pipeline`, `bounded_response_reader`, `dependency_parse` | Quick fuzz runs for 4 targets |
| `live-smoke` | `cargo test --features live-smoke --test corpus_runner -- --ignored` | Live network tests |
| `eval-tool-surface` | `cargo test --locked --all-features --test tool_surface_evaluation -- --nocapture` | Deterministic 43-case tool-selection corpus |

## Pre-release Checklist

1. All CI checks green
2. Version bumped in Cargo.toml
3. CHANGELOG.md updated
4. `make release-check` passes from a clean tree
5. `cargo publish --locked` succeeds
6. `git tag vX.Y.Z` created and pushed
