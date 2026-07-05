# Release Verification Pass Plan

## Purpose

This is a verification-only handoff plan for eggsearch after the targeted release-polish pass. The previous pass materially improved provider diagnostics, fetch safety, document chunking, metadata-only behavior, and release-facing docs. This plan should prove that those changes compile, test, package, and document correctly.

The intent is not to add another feature phase. Treat this as a release gate. Fix only defects discovered by verification, stale docs/comments, broken examples, packaging omissions, or test/CI failures.

## Release posture

Current repo shape is close to release-ready, but not release-proven. The recent polish commit changed core runtime behavior in several sensitive paths:

- Provider configured/enabled semantics.
- `eggsearch doctor` output.
- `provider_status` and health snapshot configured-state.
- Fetch target validation and DNS-address pinning.
- Redirect validation.
- `web_fetch` document chunk construction.
- `metadata_only` behavior.
- Local workspace result allocation.
- README and documentation structure.

Because these areas affect public behavior, this pass should focus on hard evidence: passing commands, schema stability, docs link validity, and release packaging checks.

## Non-goals

Do not introduce new providers, new MCP tools, new search modes, new fetch formats, or broad refactors.

Do not change public tool schemas unless a verification failure proves the current schema is broken. If a schema change is unavoidable, update schema identity/corpus tests and document the compatibility impact.

Do not expand network behavior. `web_fetch` remains explicit, bounded, no-crawl, and no-JavaScript.

## Phase 1 — Baseline repository sanity

### Tasks

1. Confirm repository status is clean before starting.
2. Confirm the latest commit is the intended release-polish commit or a direct descendant.
3. Inspect `Cargo.toml` for:
   - Version number.
   - `rust-version`.
   - package include/exclude list.
   - README path.
   - license, repository, homepage, docs.rs metadata.
4. Inspect `Makefile` or equivalent task runner and verify README's `make check` claim is true.
5. Verify `plans/release_polish_pass.md` remains historical handoff material and is not referenced as active release documentation.

### Acceptance criteria

- Working tree is clean before and after the pass except intentional verification fixes.
- README's development command exists and runs the stated gate.
- Package metadata is coherent for crates.io.

## Phase 2 — Required local command matrix

### Tasks

Run the exact release gate locally:

```bash
cargo fmt --check
cargo clippy --all-features -- -D warnings
cargo test --all-features
cargo test --no-default-features
cargo test --features mock
cargo test --features pdf
cargo build --release
cargo doc --all-features --no-deps
cargo publish --dry-run
```

If the project has a maintained `make check`, also run:

```bash
make check
```

If `make check` does not cover the whole gate, either update README wording or update the Makefile so the claim is accurate.

### Failure handling

For each failure:

1. Capture the command and error summary.
2. Fix the smallest root cause.
3. Re-run the failing command.
4. Re-run any broader command that might be affected.
5. Do not suppress warnings globally unless the warning is genuinely false-positive and documented.

### Acceptance criteria

- All required commands pass.
- `cargo publish --dry-run` passes.
- `cargo doc --all-features --no-deps` passes without rustdoc warnings if `RUSTDOCFLAGS=-D warnings` is used.

## Phase 3 — CI parity and workflow proof

### Tasks

1. Inspect `.github/workflows/ci.yml`.
2. Confirm CI covers the same meaningful matrix as local release verification:
   - format
   - clippy all features
   - tests all features
   - tests no default features
   - mock-feature tests
   - pdf-feature tests
   - release build
   - schema/corpus tests
3. Add missing CI jobs only if they are cheap and directly release-relevant.
4. Prefer adding `cargo publish --dry-run` to CI if it is not already covered.
5. Consider adding docs build:

```bash
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
```

6. If MSRV is claimed via `rust-version`, verify the CI or release checklist proves it. Use Rust 1.85 because `Cargo.toml` currently declares that as the minimum supported compiler.

### Acceptance criteria

- CI and local verification gates no longer diverge materially.
- If a command remains manual-only, release docs explicitly say it is manual-only.
- There is a visible workflow run or local evidence trail for the final verification commit.

## Phase 4 — Provider diagnostics verification

### Tasks

Verify the recent provider-state changes through tests and manual CLI output.

Test scenarios:

1. Default config:
   - DuckDuckGo, Startpage, and Yahoo are enabled/default as expected.
   - No-key HTML providers do not appear unconfigured merely because they lack API credentials.
   - API providers are not reported configured unless their configured env var exists.

2. SearXNG disabled:
   - `enabled = false` and `configured = false` or unavailable semantics are coherent.

3. SearXNG enabled without `base_url`:
   - Validation or diagnostics report the missing URL clearly.

4. SearXNG enabled with valid `base_url`:
   - `configured = true` in doctor/provider status.

5. API provider enabled without env var:
   - `enabled = true`, `configured = false`, warning emitted.

6. API provider enabled with env var:
   - `enabled = true`, `configured = true`.

7. Local workspace disabled/enabled:
   - `local_workspace` state reflects actual local backend availability.

Manual commands to run with temporary configs:

```bash
eggsearch doctor --config /tmp/eggsearch-default.toml
eggsearch doctor --config /tmp/eggsearch-searxng-missing-url.toml
eggsearch doctor --config /tmp/eggsearch-api-missing-env.toml
eggsearch providers --config /tmp/eggsearch-api-missing-env.toml
```

If the CLI does not support `--config` for a command, use the supported config path mechanism and document that limitation.

### Acceptance criteria

- `doctor`, `providers`, `provider_status`, and health snapshots use compatible configured/enabled vocabulary.
- API-key providers no longer appear configured unless credentials exist.
- No-key providers no longer appear unconfigured merely because they lack credentials.
- Tests cover the above scenarios.

## Phase 5 — Fetch safety verification

### Tasks

Verify the fetch safety changes are both tested and accurately documented.

1. Validate URL rejection:
   - `file://`
   - `ftp://`
   - embedded credentials
   - localhost literals
   - private IPv4 literals
   - IPv6 loopback
   - IPv6 unique-local/link-local/unspecified
   - IPv4-mapped blocked addresses if supported by parser

2. Validate DNS resolution behavior:
   - Resolved private addresses are blocked by default.
   - Resolved localhost addresses are blocked by default.
   - When both private-network and localhost are allowed, behavior is explicitly operator-enabled.

3. Validate pinned-resolution behavior:
   - `validate_fetch_target_with_resolved_addrs` returns validated socket addresses.
   - `FetchClient` uses `resolve_to_addrs` for the actual request attempt.
   - Redirect targets are revalidated before fetch.

4. Validate redirects:
   - Redirect to unsupported scheme is blocked.
   - Redirect to embedded credentials is blocked.
   - Redirect to private/localhost target is blocked by default.
   - Redirect limit is enforced.

5. Validate docs:
   - `docs/safety.md` does not overclaim perfect SSRF protection.
   - It accurately describes DNS validation and connection pinning.
   - It clearly states `allow_private_network` and `allow_localhost` are operator-only escape hatches.

### Acceptance criteria

- Fetch safety tests pass offline where possible.
- Any online-only edge cases are documented as manual checks or skipped tests with justification.
- Safety docs match implementation.

## Phase 6 — `web_fetch` document model and chunk verification

### Tasks

Verify document output remains stable and useful for coding agents.

1. HTML fetch:
   - `document.kind = html`.
   - `render_format = agent_blocks_v1`.
   - Outline includes headings when present.
   - Blocks are typed correctly.
   - Chunks are non-empty for non-empty documents.
   - Chunk block ranges are valid and non-overlapping.

2. Plain text fetch:
   - `document.kind = plain_text`.
   - Paragraph/block splitting is deterministic.
   - Chunks are bounded.

3. Code/source fetch:
   - Code kind and language detection remain correct where previously supported.
   - Large code blocks are bounded.
   - Line metadata remains correct.

4. Markdown fetch:
   - Headings and fenced code are represented correctly.
   - Chunks split on heading boundaries when useful.

5. Diff/patch fetch:
   - Diff language/kind remains stable.
   - Line-oriented rendering remains intact.

6. Truncation:
   - Byte truncation and character truncation are distinct.
   - `text_truncated` and response-level `truncated` semantics remain correct.

7. `metadata_only`:
   - Legacy `text` is null or omitted as intended.
   - `document` is null or minimal exactly as documented.
   - Title and description are bounded.
   - Docs state whether a bounded body read can still occur.

### Acceptance criteria

- Existing agents reading `text` are not broken in normal text/markdown modes.
- New agents can rely on `document.blocks`, `document.outline`, and `document.chunks` for structured navigation.
- Chunk IDs are stable for the same document content and URL inputs.

## Phase 7 — Local workspace allocation verification

### Tasks

The prior bug was that local search could receive zero budget from `effective_max_results / 2` when the caller asked for one result. Verify this is fixed end to end.

1. Add or confirm a test with local workspace enabled and `max_results = 1`.
2. Ensure local search receives at least one result budget for any positive caller budget.
3. Ensure final response still respects caller caps.
4. Test `max_results = 1`, `2`, and larger values.
5. Verify `include_local` behavior remains opt-in/default-coherent.

### Acceptance criteria

- Low-budget local search can return a local result.
- Final results never exceed requested or configured caps.
- Local trust labels and dirty/unknown working-tree warnings still appear when applicable.

## Phase 8 — Documentation link, example, and packaging verification

### Tasks

1. Check README links:
   - `docs/config.md`
   - `docs/safety.md`
   - `docs/tool-matrix.md`
   - `docs/agent-workflows.md`
   - `docs/architecture/codegg-contract.md`

2. Check docs for stale names and stale counts:
   - Tests or docs saying “three tools,” “nine tools,” or stale phase names should be updated if they describe the current public surface.
   - The test function currently named like `all_nine_tools` should be renamed to reflect ten tools.

3. Validate examples:
   - JSON examples should be schema-valid or clearly marked abbreviated/truncated.
   - Config examples should use actual provider IDs and actual field names.
   - `metadata_only` spelling should match the schema.
   - `extract_mode` options should match the enum.

4. Verify crate package contents:

```bash
cargo package --list
cargo publish --dry-run
```

5. If README links to docs, confirm those docs are included in the published crate package or adjust README links/packaging.

### Acceptance criteria

- No broken README/doc links.
- No stale public surface counts.
- Crate package includes required docs or README does not depend on missing package files.
- `cargo publish --dry-run` proves packaging is valid.

## Phase 9 — Changelog and release note verification

### Tasks

1. Inspect `CHANGELOG.md`.
2. Ensure the current version entry reflects actual shipped behavior:
   - Ten stable MCP tools.
   - Search profiles.
   - Provider diagnostics.
   - Repo/code search.
   - Security/advisory search.
   - Research search.
   - Explicit bounded fetch.
   - Document blocks/chunks.
   - Metadata-only mode.
   - Evidence bundles.
   - Safety defaults.
3. Remove claims about unshipped providers, tools, or guarantees.
4. Add a short release checklist if absent.

### Acceptance criteria

- Changelog is accurate and not aspirational.
- Release notes do not overstate safety or provider support.

## Phase 10 — Final release decision

### Tasks

Produce a final verification note in the repo or release issue/PR summary with:

- Commit SHA verified.
- Exact commands run.
- Pass/fail result for each command.
- Any tests intentionally skipped and why.
- Any residual risks.
- Recommendation: release / hold / release with caveats.

Suggested format:

```markdown
## Release Verification Result

Verified commit: <sha>

| Gate | Result | Notes |
|------|--------|-------|
| cargo fmt --check | pass/fail | ... |
| cargo clippy --all-features -- -D warnings | pass/fail | ... |
| cargo test --all-features | pass/fail | ... |
| cargo test --no-default-features | pass/fail | ... |
| cargo test --features mock | pass/fail | ... |
| cargo test --features pdf | pass/fail | ... |
| cargo build --release | pass/fail | ... |
| cargo doc --all-features --no-deps | pass/fail | ... |
| cargo publish --dry-run | pass/fail | ... |

Residual risks:
- ...

Decision:
- release / hold
```

### Acceptance criteria

- The final verifier leaves enough evidence for a maintainer to decide whether to cut the release.
- Any release-blocking issue has a concrete fix or follow-up plan.

## Release blockers

Treat the following as blockers:

- `cargo test --all-features` fails.
- `cargo clippy --all-features -- -D warnings` fails.
- `cargo publish --dry-run` fails.
- Public README links are broken in the packaged crate.
- `provider_status` or `doctor` reports clearly incorrect provider configured-state.
- `web_fetch` can reach localhost/private-network targets with default config.
- `metadata_only` behavior contradicts docs.
- Stable MCP tool list is inconsistent across README, docs, tests, and server output.

## Non-blocking cleanup

These are desirable but should not block if the release gates pass:

- Add dedicated `docs/providers.md` if provider docs still feel too dense inside `docs/config.md`.
- Add more manual live smoke tests for provider-specific upstream quirks.
- Add optional `cargo audit` or `cargo deny` if dependency policy is desired.
- Expand examples for codegg/opencode host configuration.

## Final handoff summary

This pass should end with a verified release decision, not another roadmap. If all gates pass and docs/package checks are clean, eggsearch should be ready to tag and publish. If any gate fails, fix the smallest concrete defect and rerun the affected gate before making the release decision.
