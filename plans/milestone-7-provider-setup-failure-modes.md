# Milestone 7 Plan: Provider Setup Matrix and Failure-Mode Documentation

## Objective

Make eggsearch provider setup and failure modes legible enough that an operator can configure common profiles without reading source code, and an agent can use `provider_status` output to make deterministic routing decisions.

The provider inventory is broad. This milestone is not about adding providers; it is about documenting what exists, how to configure it, and how to diagnose it using the current `skip_code` and provider health vocabulary.

## Scope

In scope:

- provider setup matrix;
- credential and env-var examples;
- default provider behavior;
- profile-specific provider setup;
- provider failure-mode documentation;
- provider skip-code documentation;
- provider health documentation cross-linking;
- configuration examples for common codegg use cases;
- docs tests for provider IDs and diagnostic vocabulary.

Out of scope:

- adding new providers;
- changing provider ranking or routing semantics unless a doc audit finds a bug;
- live probe implementation for `provider_status.probe`;
- making live provider checks mandatory in CI;
- storing credentials outside environment variables;
- adding persistent provider health state.

## Current State

The repo now has a stable provider surface and diagnostics:

- built-in provider descriptors;
- provider capabilities;
- machine-readable `ProviderSkipCode` values;
- `provider_status` with `routable`, `skip_reason`, and `skip_code`;
- provider health views;
- CLI `eggsearch providers` output;
- docs inventory references.

The remaining gap is operator ergonomics. A user should be able to answer these questions from docs alone:

- Which providers work by default without credentials?
- Which providers require API keys?
- Which env vars do I need?
- Which providers support code search, issue search, security search, or research search?
- Why is a provider non-routable?
- What should I do when `skip_code` is `credential_env_missing` versus `disabled_by_user` versus `cooldown_active`?
- Which configuration profile should codegg use for repo, security, and research workflows?

## Primary Deliverables

Preferred docs targets:

- `docs/provider-setup.md` — canonical provider setup and matrix;
- `docs/config.md` — config schema and snippets;
- `docs/tool-matrix.md` — compact tool/provider diagnostic link;
- README — short pointer only;
- possibly `docs/agent-workflows.md` — profile examples for codegg.

## Required Provider Matrix

Add or refine a matrix with these columns:

| Column | Meaning |
|--------|---------|
| Provider ID | Stable provider identifier used in config and tool args |
| Kind | `scrape`, `api_key`, `local`, `registry`, `security`, `scholarly`, etc. |
| Default enabled | Whether it is enabled out of the box |
| Default profile use | Generic, coding, security, research, or none |
| Credentials | None, env var, base URL, local root, etc. |
| Config section | Where to configure it in TOML |
| Capabilities | Web, docs, code, issues, releases, security, package, scholarly, freshness, domain, etc. |
| Failure modes | Common `skip_code` and health states |
| Live smoke | Whether opt-in live smoke coverage exists or is planned |

If this table becomes too wide, split it by provider class.

## Provider Classes to Document

### 1. Default no-credential web providers

Document providers that work without API keys under default config, such as default generic web providers.

For each, document:

- whether it is scrape-based;
- whether HTML drift may affect it;
- expected rate-limit behavior;
- whether safe search/freshness/domain filtering is supported or best-effort.

### 2. Optional generic/API web providers

Document providers that require explicit config or credentials, such as API-backed web search.

Include:

- TOML example;
- env var example;
- common skip codes;
- rate-limit health behavior.

### 3. SearXNG-style self-hosted providers

Document:

- required base URL;
- `enabled` semantics;
- common skip codes: `missing_searxng_config`, `missing_base_url`, `invalid_base_url`;
- operator security note that the SearXNG instance receives queries.

### 4. Code/repo providers

Document providers for:

- GitHub code/issues/releases;
- GitLab code/issues/releases;
- Gitea code/issues/releases;
- Sourcegraph;
- local workspace.

For each, document:

- credentials/config requirements;
- supported repo locator fields;
- code search capabilities;
- issue/release metadata support;
- common failures;
- recommended codegg profile use.

### 5. Security providers

Document providers for:

- OSV;
- GitHub Advisory;
- NVD;
- CISA KEV;
- RustSec;
- package registries if used by security search.

For each, document:

- credential requirements;
- advisories/vulnerability metadata coverage;
- package/version applicability limitations;
- rate-limit or update-lag caveats;
- common failure modes.

### 6. Research providers

Document providers for:

- OpenAlex;
- Crossref;
- Semantic Scholar;
- any other scholarly/research provider currently implemented.

For each, document:

- credential requirements;
- DOI/paper metadata support;
- citation/abstract limitations;
- rate-limit caveats;
- common failure modes.

### 7. Package registry providers

Document supported package ecosystems and whether they are direct providers or metadata helpers:

- crates.io;
- PyPI;
- npm;
- Go;
- Maven;
- NuGet;
- RubyGems;
- Packagist;
- OCI;
- GitHub Actions.

Clarify whether each is directly routable as a provider or used inside security/research/package resolution workflows.

## Required Configuration Examples

### Example 1: Minimal default install

Show config with no API keys and defaults only. Explain expected available providers and limitations.

### Example 2: codegg coding profile

Show config enabling repo/code providers for codegg:

```toml
[search]
default_providers = ["duckduckgo", "startpage", "yahoo"]

[search.api.github_code]
enabled = true
api_key_env = "GITHUB_TOKEN"

[search.api.github_issues]
enabled = true
api_key_env = "GITHUB_TOKEN"

[search.api.github_releases]
enabled = true
api_key_env = "GITHUB_TOKEN"
```

Adjust exact TOML shape to match current config model. Do not invent config paths if the source uses different keys.

### Example 3: local workspace for codegg

Show local root config with safe defaults. Include warnings:

- local provenance is not instruction trust;
- local files may contain prompt injection;
- roots should be narrow;
- symlink/binary/hidden behavior should be understood.

### Example 4: security research profile

Show security providers appropriate for CVE/advisory search. Include notes on provider lag and applicability limits.

### Example 5: deep research profile

Show scholarly providers and general web providers for research search. Include notes on source quality and drift.

### Example 6: SearXNG/self-hosted profile

Show base URL config and common failure modes.

## Skip-Code Documentation

Document every current serialized `ProviderSkipCode` value. For each code, include:

- meaning;
- likely cause;
- operator action;
- whether retry helps;
- whether credentials are involved;
- whether provider health is involved.

Suggested table:

| skip_code | Meaning | Common cause | Operator action |
|-----------|---------|--------------|-----------------|
| `unknown_provider` | Config references provider not known to this build | Typo or unsupported provider | Fix provider ID or upgrade eggsearch |
| `disabled_by_user` | Provider known but disabled | Config/defaults exclude it | Enable provider if intended |
| `missing_api_key` | API provider needs key but none configured | Missing API config | Add API config/env var |
| `credential_env_missing` | Env var named but not set | Shell/service env missing | Export env var or service secret |
| `credential_invalid` | Credential exists but appears invalid | Empty/malformed key or provider response | Fix secret; verify live provider |
| `missing_base_url` | Provider requires base URL | SearXNG/self-hosted config incomplete | Add base URL |
| `invalid_base_url` | Base URL is malformed | TOML typo | Fix URL |
| `missing_local_backend` | Local provider unavailable | Local roots/backend disabled | Configure local roots/backend |
| `cooldown_active` | Provider temporarily degraded | Repeated failures/rate-limit/timeout | Retry later or use another provider |
| `not_built` | Provider known but no engine built | Feature/config/build mismatch | Check config/build support |
| `unknown` | Fallback state | Unexpected condition | Inspect logs/doctor output |

Use the exact enum variants from the source. If the current enum has additional variants, document them all. If names differ, docs must match code, not this plan.

## Provider Health Documentation

Add a concise section explaining:

- `health_views` are process-local;
- `Unknown` means no observations, not necessarily failure;
- `Healthy` means a success was recorded and no active cooldown;
- `Degraded` means failures below cooldown threshold;
- `Cooldown` means repeated failures triggered a temporary cooldown;
- explicit provider requests may bypass cooldown if that is current behavior;
- restart resets process-local health.

Cross-link to the threat model and tool matrix.

## Workstream 1: Audit Source Provider Inventory

### Steps

1. Inspect provider constants in `src/core/provider.rs`.
2. Inspect engine modules under `src/meta/engines/`.
3. Inspect config model in `src/core/config.rs`.
4. Build a source-of-truth provider table from code, not README memory.
5. Compare existing `docs/provider-setup.md` against code.
6. Correct stale provider claims.

### Acceptance criteria

- Every built-in provider ID is documented.
- No documented provider ID is absent from source unless explicitly marked planned/deferred.

## Workstream 2: Add Setup Examples

### Steps

1. Create or update TOML snippets.
2. Add docs tests for snippets if `docs_config_snippets` supports them.
3. Ensure snippets avoid leaking real secrets.
4. Include shell examples for env vars using placeholder values:

```bash
export GITHUB_TOKEN="..."
```

5. Include service-manager note if useful:

```text
When running as a daemon, set env vars in the service environment, not just the interactive shell.
```

### Acceptance criteria

- Snippets parse or are covered by docs tests where possible.
- Common codegg configurations are documented.

## Workstream 3: Failure-Mode and Troubleshooting Matrix

### Steps

1. Add skip-code table.
2. Add provider-health table.
3. Add troubleshooting flows:
   - provider missing from status;
   - provider present but non-routable;
   - provider routable but returning no results;
   - provider in cooldown;
   - credential env var not found under service runtime;
   - local workspace roots missing.
4. Add suggested commands:

```bash
eggsearch providers --json
eggsearch doctor
eggsearch doctor --probe
```

Clarify that `provider_status.probe` is reserved/deferred and not a live probe.

### Acceptance criteria

- Operator can resolve most provider issues without reading source.
- `provider_status.probe` limitations are explicit.

## Workstream 4: Docs Contract Tests

### Steps

Update docs tests to verify:

- all provider IDs appear in provider setup docs or documented inventory;
- all skip codes appear in provider setup docs;
- provider-status docs mention `skip_code`, `skip_reason`, `routable`, and `health_views`;
- common config snippet syntax remains valid if parser tests exist.

### Acceptance criteria

- Provider docs cannot silently drift from source inventory.
- Skip-code vocabulary cannot silently disappear from docs.

## Testing Requirements

Run:

```bash
cargo fmt --check
cargo test --all-features --test docs_provider_inventory
cargo test --all-features --test docs_config_snippets
cargo test --all-features --test docs_tool_names
cargo test --features mock --test schema_identity_registry
```

Then include in the Milestone 5 release gate.

## Regression Risks

### Risk: Inventing config syntax

Mitigation: derive examples from `src/core/config.rs` and existing docs tests. If uncertain, use prose instead of invalid TOML.

### Risk: Provider table becomes too wide

Mitigation: split by provider class.

### Risk: Live provider behavior changes

Mitigation: document broad failure classes and skip-code semantics, not exact third-party HTML layouts.

### Risk: Docs overpromise provider capabilities

Mitigation: capabilities must be sourced from `ProviderCapabilities`, engine implementation, or tests.

## Deliverables

- Updated `docs/provider-setup.md` with complete provider matrix.
- Config examples for default, codegg coding, local workspace, security, research, and SearXNG/self-hosted setups.
- Skip-code troubleshooting table.
- Provider health explanation.
- Docs tests updated for provider IDs and skip-code vocabulary.
- README pointer to provider setup docs if needed.

## Definition of Done

This milestone is complete when every built-in provider and every provider skip code is documented, common codegg setup profiles have valid examples, provider health and `provider_status.probe` semantics are clear, docs tests protect the provider inventory, and operators can diagnose non-routable providers without reading source code.
