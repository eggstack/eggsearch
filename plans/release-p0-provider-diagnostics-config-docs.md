# P0 Release Plan: Provider Diagnostics, Routability, and Config Documentation

Status: handoff plan
Priority: P0 release blocker
Area: provider configuration, `provider_status`, operator docs

## Problem

The provider surface is now broad enough that `enabled`, `configured`, `default`, and `actually queried` are no longer the same thing. Generic HTML providers can be built directly. SearXNG needs a base URL. API-backed providers need both config and a resolvable environment variable. Gitea/Forgejo providers additionally require a `base_url`. Local workspace availability depends on `[local]` config and usable roots.

The current model has good provider descriptors, but first-time users still need a direct answer to: "will this provider actually be queried, and if not, why?"

This plan adds explicit routability diagnostics and improves docs so that release users can configure providers without reading source code.

## Relevant Code

Primary files:

- `src/core/provider.rs`
- `src/core/config.rs`
- `src/meta/adapter.rs`
- `src/mcp/tools.rs`
- `docs/config.md`
- `docs/tool-matrix.md`
- `README.md`

Provider construction path:

- `AppConfig::effective_provider_ids`
- `AppConfig::provider_is_available`
- `AppConfig::resolve_providers`
- `MetadataSearchAdapter::new`
- `build_default_engines`
- `provider_status` tool implementation

## Goals

1. Add a clear routability signal to provider diagnostics.
2. Preserve the stable MCP surface unless a schema change is intentionally accepted and covered by tests.
3. Document exact config requirements for each provider family.
4. Make Gitea/Forgejo `base_url` requirements explicit.
5. Make SearXNG enablement requirements explicit.
6. Improve operator error messages for skipped providers.

## Design

### Provider state vocabulary

Use the following terms consistently:

- `known`: provider id exists in `KNOWN_PROVIDER_IDS`.
- `enabled`: provider is enabled by config.
- `configured`: required non-secret config is present and any referenced environment variable resolves.
- `default`: provider appears in `default_providers`.
- `routable`: an engine can actually be built and selected for a live request.
- `skip_reason`: stable human-readable reason for not being routable.

Recommended skip reasons:

- `disabled`
- `missing_searxng_base_url`
- `missing_api_key_env`
- `api_key_env_not_set`
- `missing_base_url`
- `local_workspace_disabled`
- `local_workspace_unavailable`
- `unknown_provider`
- `unsupported_provider_kind`

If schema compatibility is a concern, put `routable` and `skip_reason` into an additive diagnostics object rather than replacing existing fields.

## Implementation Plan

### 1. Add provider routability helper

Create a small internal helper that can be used by both config validation/status and tests.

Suggested type:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderRoutability {
    pub id: String,
    pub routable: bool,
    pub skip_reason: Option<String>,
}
```

Suggested function location:

- `src/core/provider.rs` if it only depends on public provider/config state.
- `src/core/config.rs` if it needs `AppConfig` internals.

Suggested behavior:

- HTML providers: routable when enabled.
- `osv`: routable when enabled.
- `searxng`: routable when enabled and `[search.searxng].enabled = true` and non-empty valid `base_url`.
- API providers: routable when enabled in `[search.api.<id>]`, `api_key_env` is non-empty, and `std::env::var(api_key_env)` succeeds with non-empty value.
- Gitea/Forgejo API providers: additionally require non-empty `base_url`.
- `local_workspace`: routable when `[local].enabled = true` and roots are valid enough for backend construction.

### 2. Feed routability into `provider_status`

Update the provider-status response to include explicit routability.

Preferred additive shape:

```json
{
  "id": "gitea_code",
  "enabled": true,
  "configured": false,
  "routable": false,
  "skip_reason": "missing_base_url"
}
```

If `ProviderDescriptor` is considered stable and should not grow, add a parallel `provider_runtime` array keyed by provider id. If schema tests already snapshot descriptors, update snapshots intentionally.

### 3. Improve startup warnings

When `build_default_engines` skips a provider, log the reason, not just the id.

Current behavior returns `Vec<String>` for skipped providers. Replace or augment it with a structured type:

```rust
pub struct SkippedProvider {
    pub id: String,
    pub reason: String,
}
```

Then log with fields:

```rust
warn!(provider_id = %skipped.id, reason = %skipped.reason, "skipped provider in config");
```

If changing return type creates too much churn, keep `Vec<String>` internally but add a reason map in `provider_status` first. Prefer structured skip reasons before release.

### 4. Update docs/config.md provider setup

Add a provider requirements table:

| Provider family | Provider ids | Config requirement | Env requirement | Notes |
|---|---|---|---|---|
| HTML scrape | `duckduckgo`, `brave`, `startpage`, `yahoo`, `mojeek` | `[search.providers.<id>] = true` | none | HTML scraping can be brittle/rate-limited. |
| SearXNG | `searxng` | provider enabled plus `[search.searxng].enabled = true`, `base_url` | none | Self-hosted instance. |
| Brave API | `brave_api` | `[search.api.brave_api]` enabled | API key env var | Optional paid/API backend. |
| GitHub | `github_code`, `github_issues`, `github_releases` | `[search.api.<id>]` enabled | `GITHUB_TOKEN` or equivalent | Base URL optional for Enterprise. |
| GitLab | `gitlab_code`, `gitlab_issues`, `gitlab_releases` | `[search.api.<id>]` enabled | token env var | Base URL optional for self-managed instances. |
| Gitea/Forgejo | `gitea_code`, `gitea_issues`, `gitea_releases` | `[search.api.<id>]` enabled plus `base_url` | token env var | `base_url` is required. |
| OSV | `osv` | `[search.providers.osv] = true` | none | Native vulnerability lookup. |
| Local workspace | `local_workspace` | `[local].enabled = true`, roots | none | Not instruction-trusted. |

### 5. Add complete config examples

Add examples for:

- Minimal default web search.
- Coding-agent GitHub provider setup.
- Gitea/Forgejo provider setup with `base_url`.
- SearXNG setup.
- Security setup with OSV.
- Local workspace setup.

Gitea example:

```toml
[search.providers]
duckduckgo = true
startpage = true

[search.api.gitea_code]
enabled = true
api_key_env = "GITEA_TOKEN"
base_url = "https://git.example.org"

[search.api.gitea_issues]
enabled = true
api_key_env = "GITEA_TOKEN"
base_url = "https://git.example.org"

[search.api.gitea_releases]
enabled = true
api_key_env = "GITEA_TOKEN"
base_url = "https://git.example.org"
```

### 6. Update provider_status docs

Document that `provider_status` is configuration-derived and does not necessarily perform live network probes. Its job is to report static routability, capabilities, cached health, and workflow recipes.

Mention that `probe` is reserved unless live probing is implemented.

### 7. Add tests

Required unit tests:

- SearXNG enabled with missing base URL is not routable and has `missing_searxng_base_url`.
- Gitea code provider enabled with token but no base URL is not routable and has `missing_base_url`.
- GitHub code provider enabled with token env var is routable.
- API provider enabled with missing env var is not routable and has `api_key_env_not_set`.
- Disabled provider is not routable and has `disabled`.
- OSV enabled is routable without env var.

Required integration/schema tests:

- `provider_status` includes the new routability signal.
- `provider_status` remains valid under `--features mock`.

## Acceptance Criteria

The implementation is complete when:

- `provider_status` lets a user determine whether each known provider can actually be queried.
- Skipped providers have stable reason strings.
- Gitea/Forgejo `base_url` requirements are documented and tested.
- SearXNG requirements are documented and tested.
- API provider env-var behavior is documented and tested.
- README or docs point users to the provider setup matrix.
- Schema/corpus tests are updated if response shape changes.
- `make check` passes.

## Risk Notes

Adding fields to provider diagnostics is acceptable if schema tests are updated and the fields are additive. Avoid renaming existing fields for release unless downstream consumers are also updated.

Do not turn `provider_status.probe` into live network behavior in this pass. Live probes introduce latency, failure modes, and potential rate-limit problems. This pass is about static routability and clear skip reasons.
