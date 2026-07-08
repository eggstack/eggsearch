# Milestone 5 Plan: Release Gate Verification and CI Trustworthiness

## Objective

Establish a trustworthy release gate for eggsearch by proving that the exact release-candidate commit passes the documented local and GitHub CI checks. This milestone is about verification discipline, not new functionality.

The repository now has substantial release-hardening code in place. The remaining blocker is evidence: release readiness must be based on recorded checks for the exact commit, not commit messages or prior local runs.

## Scope

In scope:

- exact-commit verification;
- GitHub Actions workflow status confirmation;
- local release-gate command confirmation;
- release docs alignment;
- Makefile/CI command drift audit;
- feature matrix verification;
- docs-contract verification;
- rustdoc warnings-as-errors verification;
- `cargo publish --dry-run --locked` verification;
- release checklist update if needed.

Out of scope:

- adding new runtime features;
- adding new providers;
- changing the MCP tool surface;
- making live smoke tests mandatory in normal CI;
- changing branch protection through source code, except documenting required manual settings.

## Current State

The codebase has a strong release structure:

- `Makefile` contains local release-style targets;
- `.github/workflows/ci.yml` mirrors the core release gate;
- `docs/release.md` is the authoritative release procedure;
- `docs/release-checklist.md` points at the release process;
- release-hardening tests exist across fetch safety, schema identity, evidence bundle handoff, recipes, and docs contracts.

The current gap is that automated CI status was not visible for the latest head during review. This may be a connector limitation, but a production release still needs a human-verifiable green check record.

## Required Verification Matrix

Verify the exact release-candidate commit SHA. Record the SHA in the execution notes or changelog preparation notes.

### Local commands

Run the following from a clean checkout:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --no-default-features
cargo test --features mock --test schema_identity_registry
cargo test --features mock --test recipes_next_actions
cargo test --features mock --test evidence_bundle_handoff
cargo test --all-features --test fetch_safety
cargo test --all-features --test docs_config_snippets --test docs_provider_inventory --test docs_tool_names
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
cargo publish --dry-run --locked
```

If `make check` covers a subset, run it as a convenience but do not use it as the only release gate unless it explicitly covers docs build and publish dry-run.

Recommended sequence:

```bash
make check
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
cargo publish --dry-run --locked
```

Then run any missing commands not covered by `make check`.

### GitHub Actions checks

Confirm green status on the exact release-candidate SHA for:

- formatting;
- clippy all-targets/all-features;
- all-features tests;
- no-default-features tests;
- mock feature tests;
- pdf feature tests, if the workflow has a pdf job;
- docs-contract tests;
- release build;
- publish dry-run;
- rustdoc warnings-as-errors.

If any workflow is absent or not triggered, document whether that is intentional. Do not infer CI health from local success.

## Workstream 1: Audit Makefile, CI, and Release Docs Alignment

### Goal

Ensure local developer commands, GitHub Actions, and release docs describe the same release gate.

### Steps

1. Inspect `Makefile` targets:
   - `fmt`;
   - `clippy`;
   - `test-all`;
   - `test-no-default`;
   - `schema-corpus`;
   - `docs-tests`;
   - `check`.
2. Inspect `.github/workflows/ci.yml`:
   - verify clippy uses `--all-targets --all-features -- -D warnings`;
   - verify test matrix covers `--all-features`, `--no-default-features`, `--features mock`, and `--features pdf` if intended;
   - verify docs-contract tests are present;
   - verify rustdoc uses `RUSTDOCFLAGS=-D warnings`;
   - verify publish dry-run uses `--locked`.
3. Inspect `docs/release.md` and `docs/release-checklist.md`:
   - ensure the documented command list matches actual CI and Makefile behavior;
   - ensure live smoke tests are described as opt-in;
   - ensure branch-protection requirements are explicit as maintainer/manual steps.
4. Fix drift. Prefer changing docs if commands are already correct. Change CI/Makefile only if the release gate is incomplete.

### Acceptance criteria

- No drift between Makefile, CI, and release docs.
- The exact release-gate command set is documented in one authoritative location.
- Any manual-only settings, such as branch protection, are clearly called out.

## Workstream 2: Verify Feature Matrix and Optional PDF Behavior

### Goal

Make sure the release remains valid under every supported feature configuration.

### Steps

1. Run `cargo test --all-features`.
2. Run `cargo test --no-default-features`.
3. Run `cargo test --features mock`.
4. Run `cargo test --features pdf` if not already covered by all-features.
5. Verify `pdf` remains optional and disabled by default in configuration.
6. Confirm docs.rs build behavior is acceptable with all features.

### Acceptance criteria

- No-default build remains green.
- All-features build remains green.
- Mock feature tests remain deterministic and offline.
- PDF feature does not become accidentally required by default.

## Workstream 3: Verify Release-Hardening Test Coverage

### Goal

Confirm the newly added safety/diagnostic tests are actually part of the release gate.

### Must-cover test areas

- SSRF address policy and boundary tests;
- redirect-target revalidation;
- code-host URL rewrite validation;
- raw-text MCP omission;
- outline-title sanitization;
- provider skip-code serialization and no-duplicate tests;
- provider status skip-code cases;
- provider health view/error-bounding/panic classification tests;
- evidence bundle handoff tests;
- docs-provider inventory tests;
- docs tool-name tests.

### Steps

1. Confirm test names are discoverable with `cargo test --all-features -- --list` if needed.
2. Confirm relevant integration tests are run in CI, not only unit tests.
3. Confirm schema/corpus tests include the new serialized fields and do not permit accidental enum-name drift.
4. Confirm docs-contract tests check the provider inventory and tool names.

### Acceptance criteria

- The release gate exercises the newly added hardening coverage.
- No major new safety behavior exists only in unrun tests.

## Workstream 4: CI Trigger and Status Verification

### Goal

Ensure GitHub Actions status is available on normal development and release commits.

### Steps

1. Inspect `.github/workflows/ci.yml` triggers.
2. Confirm pushes to `main` trigger the CI workflow, unless the project intentionally requires PR-only CI.
3. If CI is PR-only, document the release process requiring a PR or manual dispatch before tagging.
4. If statuses are missing because of connector limitations only, record the direct GitHub UI result in release notes or the release checklist.
5. If statuses are genuinely absent, fix the workflow trigger or document local-only release verification as a temporary limitation.

### Acceptance criteria

- There is a verifiable green CI result for the exact release-candidate commit, or an explicit documented reason why CI is unavailable.
- Release readiness is not inferred from commit messages.

## Workstream 5: Release Dry-Run and Package Audit

### Goal

Ensure the crate package is ready to publish.

### Steps

1. Run:

```bash
cargo package --locked --list
cargo publish --dry-run --locked
```

2. Inspect packaged file list for:
   - README;
   - license;
   - docs referenced by README;
   - source files;
   - examples if intended;
   - no local-only artifacts;
   - no secrets/config files;
   - no generated build output.
3. Confirm `Cargo.toml` metadata:
   - version;
   - repository;
   - docs.rs metadata;
   - license;
   - keywords/categories if present;
   - rust-version.
4. Confirm `CHANGELOG.md` or release notes reflect major release-hardening changes.

### Acceptance criteria

- Publish dry-run succeeds with `--locked`.
- Package list is clean.
- Version and release notes are coherent.

## Workstream 6: Branch Protection and Release Tag Checklist

### Goal

Make release tagging deliberate and reproducible.

### Steps

1. Confirm branch protection manually in GitHub UI, if available:
   - required status checks;
   - no direct pushes if desired;
   - signed commits/tags if desired;
   - required review rules if desired.
2. Confirm tag naming convention.
3. Confirm release checklist includes:
   - exact commit SHA;
   - local gate result;
   - GitHub CI result;
   - publish dry-run result;
   - live smoke result if run;
   - known caveats.

### Acceptance criteria

- Tagging cannot accidentally bypass the documented release gate.
- Maintainers know which manual settings are outside repository code.

## Risks

### Risk: CI is not visible through API but is green in UI

Mitigation: record direct UI evidence in release checklist. Do not block indefinitely on connector visibility if a human can verify the run.

### Risk: CI triggers only on PRs

Mitigation: either add push/main trigger or make the release process require a PR before tagging.

### Risk: local test counts drift from commit-message claims

Mitigation: ignore commit-message test counts. Use current command output only.

### Risk: publish dry-run exposes package include/exclude problems late

Mitigation: run dry-run before final release polish, not after tagging.

## Deliverables

- Any required Makefile/CI/docs alignment patches.
- Recorded local release-gate command output for exact SHA.
- Recorded GitHub CI status for exact SHA.
- Publish dry-run result.
- Release checklist updated if needed.
- Any package metadata corrections discovered during dry-run.

## Definition of Done

This milestone is complete when the exact release-candidate commit has a clean local release gate, a verifiable GitHub CI result or documented CI unavailability, successful docs build with warnings denied, successful publish dry-run with lockfile, and no drift between Makefile, CI, and release docs.
