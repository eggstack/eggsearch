# P0 Release Plan: Documentation Snippet Validation and Release Checklist

Status: handoff plan
Priority: P0 release blocker
Area: documentation correctness, config examples, release verification

## Problem

The release-facing documentation now describes a large feature surface: provider profiles, API-backed code-host providers, local workspace search, fetch limits, PDF support, security search, research workflows, and evidence bundling. Manual documentation examples are easy to drift from the actual config model and provider registry.

The release should include tests that validate documented TOML snippets and provider inventories so stale docs do not ship.

## Relevant Files

Documentation:

- `README.md`
- `docs/config.md`
- `docs/tool-matrix.md`
- `docs/agent-workflows.md`
- `docs/safety.md`
- `docs/architecture/codegg-contract.md`
- new `docs/quickstart-codegg.md` if added
- new `docs/provider-setup.md` if added
- new `docs/release-checklist.md` if added

Code/test files:

- `src/core/config.rs`
- `src/core/provider.rs`
- `tests/`
- `Makefile`
- `.github/workflows/ci.yml`

## Goals

1. Ensure documented TOML snippets parse.
2. Ensure documented provider IDs exist in `KNOWN_PROVIDER_IDS`.
3. Ensure sample configs validate when environment-sensitive providers are either mocked or deliberately skipped.
4. Add a release checklist document with exact verification commands.
5. Wire docs validation into local and CI quality gates.

## Implementation Plan

### 1. Add fenced-snippet metadata convention

Update documentation TOML snippets that should be tested with a stable marker.

Recommended syntax:

```markdown
```toml eggsearch-config
[search]
mode = "live"
```
```

Use separate markers where needed:

- `toml eggsearch-config` for full config snippets that should parse as `AppConfig`.
- `toml eggsearch-config-fragment` for fragments that need wrapping before parsing.
- `toml eggsearch-provider-fragment` for provider examples.
- `bash no-test` for shell commands that should not be parsed.

Do not attempt to validate arbitrary TOML examples that are intentionally partial unless the test harness knows how to wrap them.

### 2. Add a docs snippet test harness

Create `tests/docs_config_snippets.rs`.

The test should:

- Read markdown files from `README.md` and `docs/**/*.md`.
- Extract fenced code blocks marked `toml eggsearch-config`.
- Parse each block as `AppConfig` using `toml::from_str::<AppConfig>()`.
- Call `cfg.validate()` where appropriate.
- For examples requiring API env vars, either set temporary env vars inside the test or mark those snippets as parse-only.

Because `AppConfig::validate()` warns or errors depending on env/config, create two test modes:

- `eggsearch-config`: parse and validate.
- `eggsearch-config-parse-only`: parse only.

If modifying env vars during tests, guard with a serial test or avoid env mutation by making API-provider examples parse-only.

### 3. Validate provider IDs mentioned in docs

Add a test that scans docs for backtick-delimited provider IDs and checks them against `KNOWN_PROVIDER_IDS` plus known non-provider tokens.

Simpler and less brittle approach:

- Maintain a small table in `tests/docs_provider_inventory.rs` listing documented providers.
- Assert every documented provider exists in `KNOWN_PROVIDER_IDS`.
- Assert every `KNOWN_PROVIDER_IDS` entry is either documented in `docs/config.md` or explicitly exempted.

Required assertions:

- `duckduckgo`, `brave`, `startpage`, `yahoo`, `mojeek`, `searxng` documented.
- `brave_api` documented.
- `github_code`, `github_issues`, `github_releases` documented.
- `gitlab_code`, `gitlab_issues`, `gitlab_releases` documented.
- `gitea_code`, `gitea_issues`, `gitea_releases` documented.
- `osv` documented.
- `local_workspace` documented.

### 4. Validate tool names in docs

Add a test that asserts every stable MCP tool listed in `src/mcp/mod.rs` appears in README and `docs/tool-matrix.md`.

Stable tool list:

- `web_search`
- `web_fetch`
- `batch_fetch`
- `provider_status`
- `repo_search`
- `repo_fetch`
- `repo_map`
- `security_search`
- `research_search`
- `build_evidence_bundle`

If the tool schema source exposes a single registry, prefer deriving the list from the registry rather than duplicating it in the test.

### 5. Add docs/provider-setup.md

Create a dedicated provider setup doc with:

- Provider family table.
- Required config for each family.
- Env vars and token scopes.
- Base URL semantics.
- SearXNG setup.
- Gitea/Forgejo setup.
- GitHub Enterprise/GitLab self-managed notes.
- Troubleshooting with `provider_status`.

Keep `docs/config.md` concise and link to this doc for full provider setup.

### 6. Add docs/quickstart-codegg.md

Create a focused codegg/operator quickstart:

- Install command.
- Minimal `eggsearch mcp stdio` run command.
- Suggested MCP client config shape.
- Default no-token setup.
- GitHub-token coding-agent setup.
- Local workspace setup.
- Security search example.
- Fetch markdown example.
- PDF enablement example.

Do not hardcode secrets. Use environment variable placeholders.

### 7. Add docs/release-checklist.md

Include exact release verification commands:

```bash
cargo fmt --check
cargo clippy --all-features -- -D warnings
cargo test --all-features
cargo test --no-default-features
cargo test --features mock --test schema_identity_registry
cargo test --features mock --test fetch_safety
cargo test --features mock --test security_applicability_corpus
cargo test --features mock --test research_evidence_corpus
cargo test --features mock --test recipes_next_actions
cargo test --features mock --test evidence_bundle_handoff
cargo test --test docs_config_snippets
cargo test --test docs_provider_inventory
cargo publish --dry-run --locked
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
```

Also include manual smoke checks:

- `eggsearch providers`
- `eggsearch mcp stdio`
- `provider_status`
- `web_search` generic query
- `web_fetch` markdown extraction
- `security_search` OSV query
- `repo_search` generic fallback
- local workspace search when configured

### 8. Wire tests into Makefile and CI

Update `Makefile`:

- Add a `docs-tests` target.
- Include `docs-tests` in `check`.

Update `.github/workflows/ci.yml`:

- Add docs snippet tests to `schema-corpus` or a new `docs-contract` job.

Recommended job:

```yaml
  docs-contract:
    name: docs-contract
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: "1.88"
      - run: cargo test --test docs_config_snippets
      - run: cargo test --test docs_provider_inventory
```

## Acceptance Criteria

The implementation is complete when:

- All test-marked TOML config snippets parse.
- Full config snippets validate unless explicitly marked parse-only.
- Every known provider id is documented or explicitly exempted.
- Every documented provider id exists in code.
- Every stable MCP tool is listed in README and tool matrix docs.
- Release checklist exists and includes exact local/CI commands.
- `make check` includes docs validation.
- CI runs docs validation.
- `cargo publish --dry-run --locked` remains green.

## Risk Notes

Do not overfit the markdown parser. A simple fenced-block extractor is enough. Avoid validating every code fence by default; only test explicitly marked snippets.

Do not require external credentials or live network access in docs tests. Provider examples requiring API keys should be parse-only or should use isolated env-var setup inside the test process.
