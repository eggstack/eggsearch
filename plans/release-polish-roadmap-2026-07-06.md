# Eggsearch Release Polish Roadmap

Date: 2026-07-06
Status: handoff plan
Scope: release-blocking polish and near-term post-release hardening for `eggsearch`

## Context

The current repository is close to release quality. The MCP surface is now substantially broader than a minimal metasearch server: `web_search`, `web_fetch`, `batch_fetch`, `provider_status`, `repo_search`, `repo_fetch`, `repo_map`, `security_search`, `research_search`, and `build_evidence_bundle` are exposed as the stable tool surface. Provider coverage includes generic HTML search providers, SearXNG, Brave API, GitHub/GitLab/Gitea code-host providers, OSV, and local workspace search. Fetch supports HTML/text/JSON/TOML/YAML/code-ish text, markdown rendering, structured document blocks, and optional PDF extraction.

The remaining release risk is not architectural. The main risks are operator confusion, provider configuration drift, small correctness edge cases, and preventable CPU waste under agent-heavy fetch workloads.

## Release Decision

Do not block release on new large backend additions. Do block release on the P0 items below because they can create confusing or incorrect behavior for first users.

P0 items:

1. Component-aware local workspace path traversal validation.
2. Provider routability/skip-reason diagnostics and documentation for API-backed providers, especially Gitea/Forgejo `base_url` requirements.
3. Documentation snippet validation for TOML examples and provider inventory drift.
4. Release checklist documentation that reflects the actual stable surface and verification commands.

P1 items:

1. Avoid duplicate HTML rendering work in `web_fetch`.
2. Add a conservative/low-power profile for Raspberry Pi and default local installs.
3. Expand fetch conversion coverage for high-value agent formats.
4. Improve examples for codegg/opencode style MCP usage.

P2 items:

1. Add native research backends.
2. Add native registry metadata backends.
3. Add richer security backends beyond OSV.
4. Add Sourcegraph/Zoekt-compatible code search support.

## Handoff File Index

Implement these plans in order:

1. `plans/release-p0-local-path-validation.md`
2. `plans/release-p0-provider-diagnostics-config-docs.md`
3. `plans/release-p0-doc-snippet-validation.md`
4. `plans/release-p1-fetch-performance-conversions.md`
5. `plans/release-p2-backends-expansion.md`

The P0 files should be completed before cutting a crate release. P1 can land immediately after P0 if time permits, but should not expand the stable MCP schema unless the schema tests are updated. P2 is intentionally post-release roadmap work.

## Global Acceptance Criteria

Before release, the repository should satisfy:

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
cargo publish --dry-run --locked
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
```

If new docs validation tests are added, wire them into `make check` and CI before release.

## Non-Goals

Do not rewrite the MCP surface.
Do not replace the current metasearch adapter.
Do not make PDF extraction a default feature.
Do not enable private-network fetch by default.
Do not make live probes mandatory for `provider_status`.
Do not add crawling behavior to `web_fetch`.

## Release Notes Guidance

Release notes should emphasize:

- The server is an agent-facing MCP search/fetch tool, not a browser or crawler.
- Remote web content remains `external_untrusted`.
- Local workspace results are provenance-trusted but not instruction-trusted.
- Provider availability depends on config and environment variables.
- API-backed code-host providers need explicit opt-in.
- PDF extraction requires both the Cargo `pdf` feature and `[fetch].pdf_enabled = true`.
