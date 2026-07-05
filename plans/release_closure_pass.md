# Release Closure Pass Plan

## Purpose

This is a narrow closure plan for the final release blockers identified after the verification pass. The repository is already in good shape: provider diagnostics, fetch safety, CI structure, docs build, publish dry-run, and stable tool-surface verification have all been improved. The remaining work should be treated as a short release-closing patch, not a new development phase.

The goal is to remove the last release caveats:

1. Ensure README documentation links work in the packaged crate/crates.io context.
2. Fix stale test comments or public-surface wording that still refers to older tool counts.
3. Confirm the full CI/release gate runs and leaves verifiable evidence.
4. Avoid new feature work.

## Non-goals

Do not add providers, tools, fetch modes, search modes, agent workflows, or new runtime behavior.

Do not expand the release scope beyond packaging, documentation correctness, stale wording cleanup, and verification evidence.

Do not change public MCP schemas unless an existing release gate proves they are broken.

## Current known caveats

### 1. README links to docs that are not packaged

`README.md` links to:

- `docs/config.md`
- `docs/safety.md`
- `docs/tool-matrix.md`
- `docs/agent-workflows.md`
- `docs/architecture/codegg-contract.md`

`Cargo.toml` currently includes only:

- `src/**/*.rs`
- `tests/**/*.rs`
- `README.md`
- `LICENSE`
- `CHANGELOG.md`

If the crate package excludes `docs/**/*.md`, README links may be broken on crates.io or in the unpacked crate package. This should be treated as a small release blocker.

### 2. One stale test comment remains

The test function has been renamed to `mcp_tool_surface_all_ten_tools_with_mock_state`, and the assertion expects 10 tools, but the nearby comment still says it verifies exactly the three expected tools. This is not a functional bug, but it is release-polish residue and should be fixed.

### 3. CI structure exists, but run evidence must be confirmed

The workflow now includes check/test matrices, clippy, schema-corpus tests, fmt, release build, publish dry-run, and docs with `RUSTDOCFLAGS=-D warnings`. The closure pass should confirm those checks run successfully for the final commit.

## Phase 1 — Fix package documentation links

### Objective

Ensure README links are valid both on GitHub and in the crate package/crates.io rendering.

### Preferred fix

Add documentation files to `Cargo.toml` package include list:

```toml
include = [
    "src/**/*.rs",
    "tests/**/*.rs",
    "docs/**/*.md",
    "README.md",
    "LICENSE",
    "CHANGELOG.md",
]
```

This preserves relative README links and keeps release documentation available to crate users.

### Alternative fix

If package size or policy argues against bundling docs, convert README doc links to canonical repository URLs. Example:

```markdown
[Configuration](https://github.com/eggstack/eggsearch/blob/main/docs/config.md)
```

The preferred fix is better for crate consumers because the README and linked docs remain self-contained in the published package.

### Tasks

1. Update `Cargo.toml` package `include` list to include `docs/**/*.md`.
2. Run:

```bash
cargo package --list | grep '^docs/'
cargo publish --dry-run --locked
```

3. Confirm the listed package contents include:
   - `docs/config.md`
   - `docs/safety.md`
   - `docs/tool-matrix.md`
   - `docs/agent-workflows.md`
   - `docs/architecture/codegg-contract.md`

4. If `cargo package --list` does not include docs after the change, investigate package include pattern behavior and correct it.

### Acceptance criteria

- README doc links resolve in the packaged crate.
- `cargo publish --dry-run --locked` passes.
- The fix does not alter runtime behavior.

## Phase 2 — Clean stale public-surface wording

### Objective

Remove stale wording that contradicts the ten-tool MCP surface.

### Tasks

1. Update the comment near `mcp_tool_surface_all_ten_tools_with_mock_state` in `tests/integration.rs`.
2. Replace wording like:

```text
exactly the three expected tools: web_search, web_fetch, provider_status
```

with wording like:

```text
exactly the ten stable MCP tools exposed by the current public surface
```

3. Search the repo for stale count references:

```bash
rg "three expected tools|nine tools|all_nine|exactly three|exactly 3|exactly nine|exactly 9" .
```

4. Fix any stale release-facing references. Historical plan files can remain historical unless they are likely to confuse current handoff work.

### Acceptance criteria

- No active source/test/doc comments describe the current MCP surface as three or nine tools.
- Tests still assert exactly 10 stable MCP tools.
- Historical plan documents are left alone unless actively linked as current docs.

## Phase 3 — Verify CI workflow correctness

### Objective

Confirm the new workflow is syntactically and semantically correct before relying on it as a release gate.

### Tasks

1. Inspect `.github/workflows/ci.yml` for YAML validity.
2. Confirm job names and commands match release expectations:
   - `check` matrix over `--all-features`, `--no-default-features`, `--features mock`, `--features pdf`
   - `test` matrix over the same feature set, using `cargo test --locked`
   - `clippy` with `cargo clippy --all-features -- -D warnings`
   - `schema-corpus` focused test binaries
   - `fmt` with `cargo fmt --check`
   - `release-build` with `cargo build --release`
   - `publish-check` with `cargo publish --dry-run --locked`
   - `docs` with `RUSTDOCFLAGS=-D warnings cargo doc --all-features --no-deps`

3. Confirm Rust toolchain pinning is coherent with `Cargo.toml`:
   - `rust-version = "1.85"`
   - CI uses Rust 1.85 for release verification.

4. Decide whether `cargo check` should also use `--locked`. The current `test` and publish gates use locked resolution, which is the most important part. Adding `--locked` to `cargo check` is optional but reasonable for strict reproducibility.

5. Decide whether to reintroduce `Swatinem/rust-cache`. This is not required for release correctness; omit unless CI runtime is excessive.

### Acceptance criteria

- Workflow is valid YAML.
- Workflow commands match the release verification plan.
- MSRV verification is intentional and documented.

## Phase 4 — Run and record the local release gate

### Objective

Create concrete local evidence that the final closure commit is release-ready.

Run:

```bash
cargo fmt --check
cargo clippy --all-features -- -D warnings
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
cargo build --release
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
cargo package --list
cargo publish --dry-run --locked
```

Also run:

```bash
make check
```

Note that `make check` currently covers formatting, clippy, all-features tests, no-default tests, and schema-corpus tests. It does not cover `cargo test --features pdf`, release build, docs build, or publish dry-run. That is acceptable if README describes it only as the local CI gate rather than the complete release gate.

### Acceptance criteria

- All commands pass.
- Any failing command is fixed with a minimal patch and re-run.
- Final verification notes record the command matrix and result.

## Phase 5 — Confirm GitHub Actions evidence

### Objective

Ensure the release gate is not only locally valid but also visible in GitHub Actions for the final commit.

### Tasks

1. Push the closure commit.
2. Confirm a CI workflow run exists for the final commit.
3. Confirm each job passes:
   - check matrix
   - test matrix
   - clippy
   - schema-corpus
   - fmt
   - release-build
   - publish-check
   - docs
4. If a workflow does not run, check repository Actions settings and workflow trigger configuration.
5. If the connector/API cannot see workflow runs, manually inspect the GitHub Actions UI and record the result in the final handoff note.

### Acceptance criteria

- Final commit has passing CI evidence.
- No release tag is cut before passing CI is visible or manually confirmed.

## Phase 6 — Final release handoff note

### Objective

Leave maintainers with a concise release decision record.

Create a final note in the closure commit message, release issue, or a short `plans/release_closure_verification_result.md` if desired. Do not add a new plan unless there is another blocker.

Suggested format:

```markdown
# Release Closure Verification Result

Verified commit: <sha>

| Gate | Result | Notes |
|------|--------|-------|
| cargo fmt --check | pass | |
| cargo clippy --all-features -- -D warnings | pass | |
| cargo test --all-features | pass | |
| cargo test --no-default-features | pass | |
| cargo test --features mock | pass | |
| cargo test --features pdf | pass | |
| schema-corpus focused tests | pass | |
| cargo build --release | pass | |
| cargo doc --all-features --no-deps | pass | RUSTDOCFLAGS=-D warnings |
| cargo package --list | pass | docs included |
| cargo publish --dry-run --locked | pass | |
| GitHub Actions CI | pass | run URL or note |

Residual risks:
- None known / list any caveats.

Decision:
- Release-ready / hold.
```

### Acceptance criteria

- Release decision is explicit.
- Any residual caveat is documented.
- If all gates pass, the repo is ready to tag and publish.

## Release blockers for this closure pass

Treat these as blockers:

- `cargo publish --dry-run --locked` fails.
- README doc links are broken in the package context.
- `docs/**/*.md` are not included and README keeps relative docs links.
- `cargo doc --all-features --no-deps` fails with warnings denied.
- GitHub Actions workflow does not run or fails for the final commit.
- Stable MCP tool count is inconsistent across README, docs, tests, or server output.

## Non-blocking cleanup

The following can be deferred if all release gates pass:

- Add dedicated `docs/providers.md` later.
- Add dependency audit/deny policy later.
- Add more live provider smoke tests later.
- Add more codegg/opencode host config examples later.

## Final expected outcome

After this closure pass, eggsearch should be either:

1. **Release-ready**, with docs packaged, stale wording fixed, local release gate passed, and GitHub Actions passing; or
2. **Held for one concrete blocker**, with that blocker identified precisely and no new roadmap needed.
