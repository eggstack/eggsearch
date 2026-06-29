# Final Coding-Agent Search Cleanup Plan

## Purpose

This is the final cleanup pass for the coding-agent search workstream. The major implementation is already in place and the closure pass fixed most remaining semantic issues. This plan should not reopen architecture, add new features, or broaden the roadmap. It should close the remaining repository hygiene issue, finish one small suggested-fetch fallback detail, and verify the code-facing contract for codegg.

## Current status

The repo is now functionally close:

- `repo_fetch` exposes `fetched_url`.
- Remote `repo_fetch` prefers `raw_permalink_url` when `commit_sha` is provided.
- Suggested fetches prefer `raw_permalink_url` over mutable `raw_url`.
- Provider selection telemetry now distinguishes `degraded`, `partial`, and `skipped_providers`.
- Workspace locators no longer masquerade as GitHub.
- Workspace fetch enforces `max_chars` and carries local trust markers.

Two issues remain:

1. A root-level binary file named `test_struct` is still committed.
2. Suggested fetches do not explicitly fall back to `CodeEvidence.browser_url` before `card.url`.

There is also no visible GitHub status/check result from the connector, so final verification should be recorded from local commands or CI output.

## Scope

This pass covers only:

1. Delete `test_struct` from the repository.
2. Add a narrowly scoped `.gitignore` entry if warranted to prevent reintroducing the same artifact.
3. Add `CodeEvidence.browser_url` as an explicit suggested-fetch fallback before `card.url`.
4. Add or confirm tests for the two cleanup behaviors.
5. Run and record final verification.

Do not change provider selection architecture, local indexing, package resolution, fetch sanitization, MCP tool registration, or result grouping unless required by a failing test directly tied to this plan.

## Task 1: Remove root-level binary artifact

### Problem

`test_struct` exists at the repository root and is binary content. It appears to be a compiled artifact, not a source fixture. Root-level binary artifacts make the repo noisy, increase clone size, and create ambiguity for future contributors and agents.

### Required action

Delete the file:

```bash
git rm test_struct
```

### Ignore-rule decision

After deletion, determine whether an ignore rule is warranted.

Preferred approach:

- If `test_struct` is a one-off accidental artifact, do not add a broad ignore rule.
- If a local test or ad hoc command is likely to regenerate `test_struct`, add a specific root-only ignore rule:

```gitignore
/test_struct
```

Do not add broad binary ignores such as `*` extensionless files, because Rust projects may legitimately contain extensionless scripts, fixtures, or generated test inputs.

### Acceptance criteria

- `git ls-files test_struct` returns nothing.
- `git status --ignored --short` does not show an unintentionally tracked replacement.
- The repo root contains no new ad hoc binary fixture.

## Task 2: Add explicit browser URL fallback in suggested fetches

### Problem

Suggested fetch URL selection currently prefers stable raw permalink, then raw URL, then browser permalink, then falls back to `card.url`. The final planned priority included `CodeEvidence.browser_url` before `card.url`, so that code-evidence metadata remains the authoritative URL source when present.

### Required behavior

For code evidence, suggested fetch URL priority should be:

1. `raw_permalink_url`
2. `raw_url`
3. `permalink_url`
4. `browser_url`
5. `card.url`

### Implementation sketch

In `src/meta/suggested_fetches.rs`, update the code-evidence URL chain to include `browser_url`:

```rust
let fetch_url = card
    .metadata
    .code_evidence
    .as_ref()
    .and_then(|ce| {
        ce.raw_permalink_url
            .as_deref()
            .or(ce.raw_url.as_deref())
            .or(ce.permalink_url.as_deref())
            .or(ce.browser_url.as_deref())
    })
    .unwrap_or(&card.url);
```

### Tests

Add one focused unit test in `src/meta/suggested_fetches.rs` or integration test if the helper is easier to exercise there:

- Given a source-card with code evidence containing only `browser_url`, `generate_suggested_fetches` uses that `browser_url` rather than `card.url`.

Also confirm existing tests still cover:

- `raw_permalink_url` wins over `raw_url`.
- `raw_url` wins over `permalink_url`.
- `card.url` remains fallback when no code-evidence URL exists.

## Task 3: Verify commit-SHA fetch behavior remains intact

### Purpose

The previous cleanup pass fixed this behavior, so this task is verification only unless tests fail.

### Required checks

Confirm there are tests covering:

- `repo_fetch` with `commit_sha` sets `raw_permalink_url`.
- `repo_fetch` with `commit_sha` sets `fetched_url` to `raw_permalink_url` when no `test_fetch_url` is supplied.
- `test_fetch_url` still overrides actual network fetch URL.
- `raw_url` remains the mutable ref URL for metadata.

Do not refactor this path unless one of these checks fails.

## Task 4: Verify profile telemetry semantics remain intact

### Purpose

The previous cleanup pass added structured partial telemetry. This task should only ensure tests and docs lock it down.

### Required checks

Confirm there are tests covering:

- All profile providers unavailable: `degraded = true`, `partial = false`, fallback provider set used.
- Some profile providers unavailable: `degraded = false`, `partial = true`, `skipped_providers` contains skipped IDs.
- All profile providers available: `degraded = false`, `partial = false`, `skipped_providers` empty.
- Explicit unavailable provider request remains a validation error and does not degrade.

If tests are missing, add focused tests. Avoid broad test harness rewrites.

## Task 5: Documentation touch-up

Update README and AGENTS.md only if needed:

- Mention `fetched_url` if the response-field docs do not already include it.
- State that suggested fetches prefer commit-stable raw permalinks, then raw URLs, then browser permalinks/browser URLs.
- Confirm `partial` and `skipped_providers` are documented if provider telemetry docs exist.

Keep documentation edits minimal.

## Task 6: Final verification

Run:

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Run targeted tests if available:

```bash
cargo test suggested_fetch
cargo test repo_fetch
cargo test profile
cargo test workspace
cargo test gitlab
```

Record the exact commands and results in the implementation summary or commit message body if CI is not available.

## Acceptance criteria

This workstream is closed when:

- `test_struct` is deleted and no longer tracked.
- `.gitignore` either prevents the same artifact from returning or the implementation summary explains why no ignore rule was added.
- Suggested fetch URL priority includes `CodeEvidence.browser_url` before `card.url`.
- Unit or integration tests cover browser URL fallback for suggested fetches.
- Existing commit-SHA fetch and profile telemetry semantics remain covered by tests.
- README/AGENTS are accurate for `fetched_url`, suggested-fetch priority, and profile telemetry.
- `cargo fmt`, clippy, and tests pass.

## Notes for implementer

Keep this pass intentionally small. The repository is already in good functional shape; the remaining value is hygiene and explicit verification. If unrelated failures appear during full tests, record them separately instead of expanding this cleanup pass into a general repair effort.
