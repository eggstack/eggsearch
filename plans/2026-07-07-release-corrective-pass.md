# eggsearch Release Corrective Pass

Date: 2026-07-07

## Purpose

This is a narrow corrective handoff plan for the remaining production-release blockers after the production hardening test pass. The previous implementation substantially improved test coverage, but several release-readiness items remain open:

1. GitHub CI still does not match the stricter local clippy gate.
2. The latest head commit still has no visible GitHub workflow/status evidence from the inspected API results.
3. Release documentation/checklist is missing.
4. `provider_status.probe` remains ambiguous: it is accepted by the schema but no structured live probe implementation is visible.

This pass should be small, mechanical, and verification-oriented. Avoid broad refactors. Do not add unrelated features.

## Current state

The hardening implementation commit added a large amount of coverage, including config validation tests, fetch safety tests, integration response-shape tests, and local workspace hardening tests. It also updated developer instructions to use:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

However, the actual GitHub Actions workflow still uses:

```bash
cargo clippy --all-features -- -D warnings
```

while the Makefile already uses the stricter all-targets command. This leaves CI weaker than the local release gate and should be corrected before release.

## Non-goals

Do not change the stable MCP tool list. Do not remove existing test coverage. Do not add new search backends. Do not weaken fetch/private-network safety defaults. Do not make live network tests required in default CI. Do not introduce persistent state or a database.

## Phase 1: Align GitHub CI with local release gate

### Tasks

1. Update `.github/workflows/ci.yml` clippy command.
   - Change:
     ```bash
     cargo clippy --all-features -- -D warnings
     ```
     to:
     ```bash
     cargo clippy --all-targets --all-features -- -D warnings
     ```
   - Add a short YAML comment near the clippy job explaining that CI intentionally mirrors the Makefile release gate.

2. Review workflow parity against `Makefile`.
   - Confirm CI covers:
     - `cargo fmt --check`
     - `cargo clippy --all-targets --all-features -- -D warnings`
     - `cargo test --all-features`
     - `cargo test --no-default-features`
     - `cargo test --features mock` matrix or explicit mock corpus tests
     - `cargo test --features pdf`
     - docs contract tests
     - schema/fixture corpus tests
     - `cargo publish --dry-run --locked`
     - `cargo doc --all-features --no-deps` with `RUSTDOCFLAGS=-D warnings`
   - Do not make live-smoke tests part of default CI.

3. If workflow duplication is too high, keep the existing matrix but ensure the required commands are explicitly present and readable. The goal is clarity and release confidence, not maximal YAML abstraction.

### Acceptance criteria

- CI clippy matches Makefile clippy exactly.
- CI remains offline/deterministic by default.
- A reviewer can compare Makefile and CI and see no meaningful release-gate mismatch.

## Phase 2: Add release documentation

### Tasks

1. Create `docs/release.md`.

2. Include a release checklist with exact commands:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --no-default-features
cargo test --features mock
cargo test --features pdf
cargo test --features mock --test schema_identity_registry
cargo test --features mock --test fetch_safety
cargo test --features mock --test security_applicability_corpus
cargo test --features mock --test research_evidence_corpus
cargo test --features mock --test recipes_next_actions
cargo test --features mock --test evidence_bundle_handoff
cargo test --all-features --test docs_config_snippets --test docs_provider_inventory --test docs_tool_names
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
cargo publish --dry-run --locked
```

3. Document expected feature behavior.
   - Default feature set is intentionally minimal.
   - `pdf` enables PDF extraction support.
   - `mock` is for tests and offline harnesses.
   - `live-smoke` is opt-in and may require network/API credentials.

4. Document recommended GitHub release requirements.
   - Before tagging a release, require visible green CI on the exact release commit.
   - Recommended required checks:
     - check matrix
     - test matrix
     - clippy
     - fmt
     - schema-corpus
     - docs-contract
     - release-build
     - publish-check
     - docs
   - State that branch protection/settings are managed in GitHub and are not enforceable from repository code alone.

5. Document live-smoke policy.
   - Keep live-smoke tests ignored/opt-in.
   - No release should be blocked solely by third-party provider drift unless the drift indicates a local regression.
   - Record provider drift in release notes or issues when discovered.

6. Link release docs from `README.md` or an existing docs index if appropriate.
   - Prefer adding a single line under the README docs list: `Release checklist`.

### Acceptance criteria

- `docs/release.md` exists and contains exact pre-release commands.
- README or docs index links to it.
- Release docs clearly distinguish required offline gates from optional live smoke tests.

## Phase 3: Resolve `provider_status.probe` ambiguity

### Problem

The schema exposes `provider_status` with a `probe` field, but the current implementation appears to reserve it for future use rather than actually probing providers. This is confusing for production operators.

### Decision point

Choose one of two acceptable outcomes for this corrective pass.

### Option A: Explicitly defer probe implementation

This is the preferred option if the pass should stay small.

Tasks:

1. Keep `probe` backward-compatible in the schema.
2. Make `provider_status` response include an explicit field such as:
   ```json
   "probe": {
     "requested": true,
     "implemented": false,
     "message": "provider_status.probe is reserved for a future bounded live probe; use eggsearch doctor --probe or live-smoke tests for now"
   }
   ```
   when `probe = true`.
3. If `probe = false`, either omit this field or include:
   ```json
   "probe": { "requested": false, "implemented": false }
   ```
   whichever is most consistent with current response style.
4. Update `ProviderStatusArgs` docs to say it is accepted for forward compatibility but not a live probe yet.
5. Add a regression test proving `provider_status { probe: true }` is not silently ignored and returns an explicit deferred/unsupported signal.
6. Update docs/tool-matrix/provider docs to mention the current probe status.

Acceptance criteria:

- Calling `provider_status` with `probe = true` no longer silently implies probing occurred.
- The response is explicit and machine-readable.
- Existing clients remain compatible.

### Option B: Implement minimal bounded probes

Choose this only if the implementing agent has time and wants to close the feature fully.

Tasks:

1. Add a bounded provider probe path.
   - One low-cost query per routable provider.
   - Per-provider timeout bounded by config search timeout.
   - Probe failures are diagnostic, not fatal.
2. Add structured fields:
   - `probed: bool`
   - `probe_status: "ok" | "failed" | "skipped" | "unsupported"`
   - `probe_error_class: Option<String>`
   - `probe_message: Option<String>`
   - `probe_latency_ms: Option<u64>`
   - `probe_result_count: Option<usize>`
3. Keep `provider_status { probe: false }` fast and network-light.
4. Add mock/provider-contract tests for probe response shape.
5. Keep live network behavior opt-in.

Acceptance criteria:

- `provider_status { probe: true }` performs bounded, nonfatal structured probes.
- `provider_status { probe: false }` does not probe.
- Probe failures update diagnostics without crashing the tool.

## Phase 4: Verify visible CI and release gate

### Tasks

1. Push the corrective commit.
2. Confirm GitHub Actions runs on the exact new head commit.
3. Confirm the combined status/check UI is visible for the head commit.
4. If Actions do not run, investigate repository settings/workflow trigger issues.
   - Confirm workflow file is on default branch.
   - Confirm Actions are enabled for the repository/org.
   - Confirm branch name is `main` and workflow triggers include push to `main`.
   - Confirm no path filters are suppressing execution.
5. If GitHub API still reports no status after a push, document this in the release notes/plan and do not claim CI is green.

### Acceptance criteria

- The new head commit has visible workflow/check evidence, or a clear documented reason why GitHub Actions did not run.
- Do not rely only on commit-message claims for release status.

## Phase 5: Final validation commands

Run at minimum:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --no-default-features
cargo test --features mock
cargo test --features pdf
cargo test --all-features --test docs_config_snippets --test docs_provider_inventory --test docs_tool_names
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
cargo publish --dry-run --locked
```

If time permits, also run:

```bash
make check
cargo test --features live-smoke --test corpus_runner -- --ignored
```

The live-smoke command is optional and must not be required for default CI.

## Review checklist

Before closing this pass, verify:

- `.github/workflows/ci.yml` clippy uses `--all-targets`.
- `docs/release.md` exists and is linked.
- `provider_status.probe` is either explicitly deferred in structured output or implemented with bounded structured probes.
- GitHub Actions/check evidence is visible on the final commit, or a clear issue is documented.
- No stable MCP tool has been removed or renamed.
- No fetch/local safety default was weakened.
- Existing hardening tests remain intact.
- `cargo publish --dry-run --locked` passes locally and/or in CI.

## Expected final repo state

After this corrective pass, eggsearch should be in release-candidate shape for codegg integration and public release. Any remaining work should be non-blocking polish, provider-specific drift handling, or new capability development rather than release infrastructure gaps.
