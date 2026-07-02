# Phase 1: Provider Truthfulness and Routing Correctness

## Objective

Make provider state a canonical, truthful, agent-consumable contract across eggsearch. `provider_status`, config validation, routing decisions, CLI provider display, and capability enforcement telemetry should all derive from the same provider resolution model. An agent should be able to inspect provider status once and know which providers are enabled, configured, default, healthy, degraded, on cooldown, missing credentials, or incapable of enforcing requested constraints.

This phase is foundational because later agent workflows depend on choosing the right provider set. Incorrect provider status causes poor routing, misleading fallbacks, and unnecessary failed tool calls.

## Current problem statement

The provider model has the right pieces, but the state is distributed. Known provider IDs include scrape providers, API-backed code/issue/release providers, OSV, and local workspace. However, the status path can make API providers appear configured by default or append descriptors from a separate API map, which risks duplicate or misleading status output. Configuration validation is also scrape-provider-oriented and should explicitly support valid API-only deployments.

Agents need stronger guarantees:

- A provider appears exactly once in `provider_status`.
- `configured` means the provider is actually usable, not merely known.
- Missing API secrets are visible before a search call fails.
- Disabled providers are distinguishable from enabled-but-unconfigured providers.
- Default provider sets are validated against the same registry used at runtime.
- Routing telemetry explains why a provider was selected, skipped, degraded, or cooled down.

## Scope

In scope:

- Introduce or refactor a canonical provider registry/resolution layer.
- Make provider descriptors single-source-of-truth for status and routing.
- Correct API provider configured-state handling.
- Correct live-mode validation for API-only and mixed deployments.
- Ensure local workspace provider state is represented consistently.
- Add status/validation/routing tests for all relevant provider classes.
- Preserve existing public provider IDs unless an ID is demonstrably wrong.

Out of scope:

- Adding new external providers.
- Changing the search result ranking model.
- Changing provider HTTP implementation details except where required for configured-state detection.
- Adding live network probes to `provider_status`; this phase should remain non-probing unless the repo already has a local health registry state to report.

## Design requirements

### Canonical provider registry

Create a central provider registry model that can answer these questions for each provider ID:

- Is the provider known?
- Is it built into the binary?
- Is it enabled by config?
- Is it selected by default?
- Does it require an API key?
- If it requires an API key, which environment variable is required?
- Is the environment variable present?
- Is it configured enough to build an engine?
- Is it represented by an engine in the current adapter?
- What capabilities are actually enforceable?
- What health/cooldown state is currently known?

Prefer a small immutable resolved-provider struct generated at server startup and passed or referenced by status/routing code.

Suggested shape:

```rust
pub struct ResolvedProviderStatus {
    pub id: String,
    pub display_name: String,
    pub kind: ProviderKind,
    pub known: bool,
    pub enabled: bool,
    pub default: bool,
    pub built: bool,
    pub configured: bool,
    pub requires_api_key: bool,
    pub api_key_env: Option<String>,
    pub api_key_present: Option<bool>,
    pub capabilities: ProviderCapabilities,
    pub health: Option<ProviderHealthSnapshot>,
    pub unavailable_reason: Option<ProviderUnavailableReason>,
}
```

The exact struct can differ, but it must represent the same facts.

### Single descriptor output

`provider_status` should produce exactly one descriptor per provider ID. If a provider exists in both known IDs and API config maps, merge the state; do not append a second descriptor.

### API provider configured semantics

For API-key-backed providers:

- `configured = enabled && api_key_env is set && std::env::var(api_key_env).is_ok()` unless the provider genuinely supports anonymous mode.
- `enabled = true` with missing env var should not make the provider appear usable.
- Missing env var should emit a clear status reason but should not necessarily abort startup unless the existing config policy intentionally treats that as fatal.

### SearXNG configured semantics

For SearXNG:

- `configured = enabled && base_url is present && base_url is syntactically valid`.
- If SearXNG is listed in defaults but disabled/unconfigured, status and validation should make the reason clear.

### Local workspace semantics

For local workspace:

- `enabled` should reflect `[local].enabled`.
- `configured` should require enabled plus at least one valid root.
- Capabilities should show local code/file search but not remote issue/release/advisory support.
- Trust classification should remain separate from provider capability; do not imply local workspace is globally trusted.

### Routing consistency

Provider routing should consume the same resolved-provider status used by `provider_status`. When routing skips a provider, the skip reason should be stable and machine-actionable:

- unknown_provider
- disabled_provider
- not_configured
- missing_api_key
- not_built
- cooldown
- unsupported_profile
- unsupported_capability

Existing human-readable reasons may remain for compatibility, but stable reason codes should be added if not already present.

## Implementation steps

1. Locate all provider-state paths: config validation, `provider_status`, `resolve_provider_routing`, adapter construction, CLI provider display, provider capability telemetry, and tests.
2. Introduce a resolved provider registry function in a neutral module, likely near `core::provider` or `meta::provider_diagnostics`, depending on dependency direction. Keep core types independent of live engine construction where possible.
3. Refactor `MetadataSearchAdapter::provider_status()` to use the canonical registry and adapter-built engine IDs. Remove any append-style API descriptor logic that can duplicate providers.
4. Refactor config validation so live mode accepts valid API-only configurations. Validation should reject truly empty live configurations but not reject a setup simply because scrape providers are disabled.
5. Ensure configured-state warnings are emitted once and at the correct layer. Missing API env vars should be visible in `doctor`/`providers` output and provider status responses.
6. Update routing to reject/skip providers according to resolved status, not ad-hoc checks.
7. Add compatibility tests to ensure existing default config behavior remains unchanged.
8. Add new tests for edge cases listed below.

## Required tests

Add or update tests for:

- Default config returns scrape providers once each and OSV/local/API states are coherent.
- API provider enabled with env var present appears enabled/configured/built when the engine can be built.
- API provider enabled with env var missing appears enabled/not configured/missing_api_key and is skipped or warned clearly.
- API-only configuration validates when at least one API provider is configured.
- API-only configuration fails when all API providers are missing credentials.
- SearXNG enabled without base URL fails validation or appears unconfigured according to existing policy.
- SearXNG enabled with valid base URL appears configured.
- `local_workspace` appears enabled/configured only when local search is enabled and roots are valid.
- Explicit duplicate provider IDs in request are deduplicated while preserving order.
- `provider_status` never emits duplicate IDs.
- Routing telemetry lists skipped providers with stable reason codes.

## Acceptance criteria

- `provider_status` emits no duplicate provider descriptors.
- API providers do not appear configured unless their required secret is present.
- Valid API-only deployments are accepted.
- Invalid live deployments fail with a precise config error.
- CLI provider output and MCP `provider_status` agree.
- Existing default-provider behavior is preserved for normal scrape-provider deployments.
- Tests cover missing env vars without leaking secret values.

## Risks and mitigations

Risk: Refactoring provider state can perturb existing default behavior.

Mitigation: Add characterization tests for the current default config before changing behavior.

Risk: Core modules may become coupled to adapter/runtime state.

Mitigation: Keep static provider capability descriptors in core and runtime health/build status in meta or state. Merge at the status boundary.

Risk: API providers may have provider-specific anonymous or optional-key behavior later.

Mitigation: Model credential requirements per provider rather than assuming every API provider always requires a key.

## Handoff notes

Prefer small, mechanical commits. First add tests that demonstrate duplicate/misleading states. Then add the registry and route call sites through it. Avoid provider behavior changes unrelated to status/routing correctness.
