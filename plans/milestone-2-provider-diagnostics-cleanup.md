# Milestone 2 Plan: Provider Status and Routing Diagnostics Cleanup

## Objective

Make provider diagnostics precise, machine-readable, and consistent across MCP `provider_status`, CLI provider output, runtime routing, warnings, docs, and tests.

A coding agent should be able to inspect provider status and distinguish these states without interpreting vague prose:

- provider is known but disabled;
- provider ID is unknown;
- provider is enabled but missing configuration;
- provider is enabled but missing an API key environment variable name;
- provider has an API key environment variable name but the variable is unset;
- provider is configured but not built into the adapter;
- provider is configured and built but temporarily unhealthy or in cooldown;
- provider requires an optional feature that is not compiled;
- provider is routable now.

## Scope

In scope:

- provider status response model;
- provider descriptor fields;
- skip-reason semantics;
- CLI provider JSON output;
- MCP `provider_status` output;
- provider routing warnings;
- docs provider inventory;
- tests for known/unknown/configured/routable states.

Out of scope:

- adding new providers;
- adding new MCP tools;
- changing provider credentials storage away from environment variables;
- persistent provider health state, except as part of Milestone 4 if later selected;
- live provider smoke tests as release blockers.

## Relevant Code Areas

Primary files to inspect:

- `src/core/provider.rs`
- `src/core/config.rs`
- `src/meta/adapter.rs`
- `src/meta/provider_diagnostics.rs`
- `src/commands/providers.rs`
- `src/commands/doctor.rs`
- `src/mcp/*` provider-status tool implementation files
- `docs/provider-setup.md`
- `docs/tool-matrix.md`
- `docs/config.md`
- `tests/docs_provider_inventory.rs`
- provider-status and schema/corpus tests

## Current Problem Statement

The provider model now has known provider IDs, API provider IDs, configured/routable state, capabilities, and skip reasons. That is the right architecture. The remaining release gap is precision.

A known provider that is disabled or not built should not receive a skip reason that says it is unknown. An API provider with a missing `api_key_env` should not be collapsed with an API provider whose env var is configured but currently unset. SearXNG with no `base_url` should have a distinct state. Local workspace disabled/unavailable should be distinct from remote provider misconfiguration.

The output should be stable enough that codegg can use it for deterministic tool-selection and self-repair hints.

## Design Requirements

### 1. Define a canonical provider state vocabulary

Add a machine-readable skip/status code vocabulary. This can be an enum in Rust serialized as snake_case, or a string field populated from a central helper.

Recommended enum:

```rust
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSkipCode {
    Disabled,
    UnknownProvider,
    NotBuilt,
    NotConfigured,
    MissingApiKeyEnv,
    ApiKeyEnvUnset,
    InvalidBaseUrl,
    LocalBackendUnavailable,
    FeatureNotCompiled,
    Cooldown,
    ProviderError,
}
```

Name can differ, but the vocabulary should be centralized and documented. Avoid many ad hoc string literals scattered across `provider_status`, CLI, and diagnostics code.

### 2. Preserve human-readable messages but do not require parsing them

`skip_reason: Option<String>` can remain for humans. Add a canonical field, such as:

```rust
pub skip_code: Option<ProviderSkipCode>
```

or

```rust
pub status_code: ProviderStatusCode
```

The agent-facing contract should use the code, not the prose.

### 3. Clarify field semantics

Document and enforce the following semantics:

- `known`: provider ID is part of the known provider registry or accepted configured provider namespace.
- `enabled`: operator config allows this provider.
- `configured`: required static config exists, such as base URL, local roots, or API key env-var name and current env-var value depending on selected semantics.
- `built`: adapter has a live engine instance for this provider.
- `healthy`: provider is not currently degraded by health/cooldown state.
- `routable`: provider can be selected for a request now.
- `default`: provider appears in configured default provider list.

If adding all fields is too broad, preserve existing fields but define their exact meaning in docs and tests.

### 4. Separate API provider credential cases

For providers in `API_PROVIDER_IDS`, distinguish:

- not present in `[search].api`;
- present but `enabled = false`;
- `enabled = true` but `api_key_env` missing or empty;
- `api_key_env` set but environment variable is absent;
- environment variable present but empty string, if current code treats this as configured accidentally;
- environment variable present and non-empty.

Do not log or return the secret value. It is acceptable to return the environment variable name.

### 5. Separate SearXNG cases

For `searxng`, distinguish:

- disabled in `[search].providers`;
- enabled but `[search].searxng.enabled = false`;
- enabled but `base_url` missing/empty;
- invalid base URL;
- configured and built;
- built but unhealthy/cooldown.

### 6. Separate local workspace cases

For `local_workspace`, distinguish:

- local backend disabled;
- no roots configured;
- root missing;
- root not directory;
- root available and built;
- local backend excluded by request.

Be careful not to expose sensitive absolute paths beyond what existing local workspace output already exposes. If paths are included, keep them bounded and operator-controlled.

### 7. Keep provider capability reporting stable

Do not regress `ProviderCapabilities`. This milestone should not change whether a provider claims code search, issue search, release search, security search, package metadata, or scholarly search unless the current claims are demonstrably wrong.

If a capability correction is discovered, include a test and note it in the plan execution summary.

## Implementation Steps

### Step 1: Inventory provider-state creation paths

Trace where provider status is constructed:

- provider descriptors in `src/core/provider.rs`;
- config availability in `src/core/config.rs`;
- adapter built provider IDs in `src/meta/adapter.rs`;
- CLI provider output;
- MCP provider-status output;
- doctor command output.

Create a short internal map in comments or tests showing which code path owns each state field.

### Step 2: Add canonical skip/status code type

Add the enum in `src/core/provider.rs` or a closely related diagnostics module. Prefer a core type if it appears in MCP response schema.

Update `ProviderDescriptor` to include the new optional code field. Use `#[serde(default, skip_serializing_if = "Option::is_none")]` only if preserving old clients matters. Since this is pre-release hardening, adding the field without skipping may be acceptable, but defaulting is safer.

### Step 3: Centralize skip-code selection

Add a helper that computes provider state from inputs rather than inlining string branches in `provider_status()`.

Suggested shape:

```rust
struct ProviderStateInputs<'a> {
    id: &'a str,
    known: bool,
    enabled: bool,
    configured: bool,
    built: bool,
    default: bool,
    health: Option<&'a ProviderHealthSnapshot>,
}

fn provider_skip_code(inputs: ProviderStateInputs<'_>) -> Option<ProviderSkipCode> { ... }
```

Keep it simpler if needed, but ensure there is one canonical path.

### Step 4: Fix known-but-disabled semantics

Ensure a provider in `KNOWN_PROVIDER_IDS` that is not enabled reports `disabled`, not `unknown_provider`.

Ensure a provider in config that is not in `KNOWN_PROVIDER_IDS` reports `unknown_provider`, unless the design intentionally allows future/custom providers. If future/custom providers are allowed, document the behavior explicitly and use a different code such as `not_built` or `unsupported_custom_provider`.

### Step 5: Add API credential-state helper

Implement a helper in config/provider code that returns a credential state, not just boolean configured/unconfigured.

Suggested enum:

```rust
pub enum ApiProviderCredentialState {
    Disabled,
    MissingApiKeyEnv,
    ApiKeyEnvUnset,
    Configured,
}
```

This helper should treat empty environment variable values as unset unless the project explicitly wants to permit empty API keys. Prefer treating empty as unset.

### Step 6: Update CLI and MCP output

Update provider-status output structs and CLI JSON output. For non-JSON human output, include concise state code and human reason:

```text
github_code  enabled=true configured=false routable=false skip=api_key_env_unset
```

Do not make prose longer than necessary.

### Step 7: Update warnings and next-action hints if applicable

Where provider resolution fails, use the canonical code in warnings if possible:

- `provider_unavailable: github_code api_key_env_unset`
- `profile_provider_unavailable: github_code disabled`
- `native_code_search_unavailable: no routable code provider`

Do not destabilize existing warning tests unnecessarily; update fixtures intentionally.

### Step 8: Add tests

Add or update tests covering:

- known provider disabled;
- unknown provider configured in providers map;
- default provider disabled;
- default provider unknown;
- SearXNG enabled with missing base URL;
- SearXNG enabled with invalid base URL;
- API provider enabled with missing `api_key_env`;
- API provider enabled with empty `api_key_env`;
- API provider env var name configured but env var unset;
- API provider env var present but empty;
- API provider env var present and non-empty;
- local workspace disabled;
- local workspace enabled but no roots;
- provider in cooldown once Milestone 4 is implemented.

Use scoped environment-variable helpers in tests to avoid cross-test contamination. Restore env vars after test completion.

### Step 9: Update docs

Update:

- `docs/provider-setup.md` with skip/status code vocabulary;
- `docs/tool-matrix.md` if provider-status response shape is documented there;
- `docs/config.md` for API credential examples;
- `docs/agent-workflows.md` if next-action hints rely on provider status;
- README only if a concise mention is warranted.

### Step 10: Update docs contract tests

If docs-provider inventory tests scan for provider IDs or status terms, update expected fixtures. Add a docs test that ensures each skip code is documented if the existing test style supports it.

## Testing Requirements

Run targeted tests first:

```bash
cargo test --all-features provider
cargo test --all-features provider_status
cargo test --features mock --test schema_identity_registry
cargo test --all-features --test docs_provider_inventory
```

Then run the release-style gate:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --no-default-features
cargo test --features mock --test schema_identity_registry
cargo test --features mock --test recipes_next_actions
cargo test --all-features --test docs_config_snippets --test docs_provider_inventory --test docs_tool_names
```

Use `make check` if available in the handoff environment.

## Regression Risks

### Risk: Breaking response compatibility

Adding fields is low risk. Renaming existing fields is higher risk. Prefer additive fields and updated docs unless there is a strong reason to rename.

### Risk: Overcomplicating provider status

Provider status should be diagnostic, not a verbose tracing dump. Keep detailed error text bounded. Use stable codes for agent logic.

### Risk: Environment-variable tests racing

Rust tests run concurrently by default. Tests that mutate environment variables should use unique variable names or serial guards. Prefer unique env names per test.

### Risk: Future custom providers

If the repo intends to support future provider IDs through config, be careful with `unknown_provider`. It may be better to report `unsupported_custom_provider` or `not_built` for configured-but-not-implemented providers.

## Deliverables

- Canonical provider skip/status code type.
- Updated `ProviderDescriptor` or provider-status response schema.
- Centralized provider state computation.
- CLI and MCP provider status output using canonical codes.
- Tests for disabled, unknown, missing config, unset credential, invalid base URL, local backend unavailable, and routable states.
- Updated provider docs and docs contract tests.

## Definition of Done

This milestone is complete when provider-status output can explain every non-routable provider using a stable machine-readable code, known-disabled providers are not mislabeled as unknown, credential/configuration failure modes are distinguishable, CLI and MCP outputs agree, and docs/tests lock down the vocabulary.
