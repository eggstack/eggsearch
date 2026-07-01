# Phase 7 Plan: Provider Routing, Diagnostics, and Adaptive Degradation

## Objective

Promote provider/profile routing into a shared diagnostic layer across eggsearch tools. Agents and UIs should be able to understand which providers were selected, which capability each provider enforced, which providers degraded or failed, and why a fallback occurred.

This phase should add process-local provider health snapshots and capability-aware selection telemetry without changing eggsearch into a persistent scheduler or provider broker. The focus is transparency, deterministic routing, bounded adaptive degradation, and better operator/agent feedback.

## Current baseline

The repo already has:

- `provider_status` with provider descriptors, code-host summaries, server capabilities, and tool capabilities.
- Strict validation for explicitly requested unknown providers.
- Profile routing for `repo_search` with visible profile warnings.
- Provider failures surfaced in search responses.
- Parallel subquery dispatch with total/partial provider failure semantics.
- Capability-aware warnings for repo filters, issue search, release search, freshness, and native-provider availability.

The gap is that this behavior is spread across tools and helpers. Provider health is not tracked coherently, routing explanations are inconsistent by tool, and agents cannot ask “why did this provider not run?” in a uniform way.

## Non-goals

Do not add persistent provider health storage. Do not add cross-process provider cooldown state. Do not add retry storms, exponential backoff loops, or automatic credential discovery. Do not silently remove explicitly requested providers except for hard validation failures.

Do not make routing nondeterministic. Health snapshots can influence default/profile selection, but responses must expose the decision.

## User-facing behavior

Provider routing should follow these principles:

1. Explicit provider lists remain strict.
   - Unknown explicit provider ID: validation error.
   - Disabled/unavailable explicit provider: validation error or explicit failure, not silent fallback.
2. Profile/default provider selection may degrade.
   - If a profile provider is unavailable, skip it with a warning.
   - If all profile providers are unavailable, fall back to default providers with a clear degraded flag.
3. Temporary cooldown is advisory for profile/default selection.
   - If a provider recently rate-limited or timed out, deprioritize or skip it for profile/default calls.
   - Expose cooldown decisions in telemetry.
4. Capability enforcement must be visible.
   - A provider may be selected but unable to enforce repo/path/language/freshness/security constraints.
   - Responses should state which constraints were enforced, approximated, or not enforced.

## Proposed core types

Add a shared provider diagnostics module, likely `src/meta/provider_diagnostics.rs` or `src/core/provider_diagnostics.rs` depending on existing layering.

Suggested types:

```rust
pub struct ProviderHealthSnapshot {
    pub provider_id: String,
    pub enabled: bool,
    pub configured: bool,
    pub last_success_at: Option<SystemTime>,
    pub last_failure_at: Option<SystemTime>,
    pub recent_failure_class: Option<String>,
    pub recent_failure_message: Option<String>,
    pub recent_latency_ms: Option<u64>,
    pub consecutive_failures: u32,
    pub cooldown_until: Option<SystemTime>,
    pub cooldown_reason: Option<String>,
}

pub struct ProviderRoutingDecision {
    pub requested_profile: Option<SearchProfile>,
    pub requested_providers: Vec<String>,
    pub selected_providers: Vec<String>,
    pub skipped_providers: Vec<ProviderSkipReason>,
    pub degraded: bool,
    pub partial: bool,
    pub reason: Option<String>,
}

pub struct ProviderSkipReason {
    pub provider_id: String,
    pub reason: String,
    pub failure_class: Option<String>,
    pub cooldown_until: Option<String>,
}

pub struct CapabilityEnforcementTelemetry {
    pub requested: Vec<String>,
    pub enforced: Vec<String>,
    pub approximated: Vec<String>,
    pub not_enforced: Vec<String>,
}
```

Keep public response additions compatible: `#[serde(default, skip_serializing_if = ...)]` for new telemetry fields.

## Workstream 1: Provider health tracker

### Implementation

Add a process-local health tracker owned by `MetadataSearchAdapter` or `ServerState`.

Requirements:

- Track health per provider ID.
- Update health after each provider job result.
- Record failure class, message, consecutive failure count, and recent latency.
- Record last success and reset consecutive failures on success.
- Derive cooldown for rate-limit and repeated timeout/transport failures.
- Cooldown should be short and conservative by default, for example:
  - Rate limit: 60 seconds unless provider exposes a retry-after signal.
  - Timeout: 15 seconds after 3 consecutive timeouts.
  - Transport: 30 seconds after 3 consecutive failures.
- Disable cooldown entirely in tests unless explicitly enabled or make it deterministic with injectable clock.

### Data ownership

Use `Arc<Mutex<...>>` or `tokio::sync::RwLock` depending on async needs. Keep the critical section small. Avoid holding locks across provider calls.

### Tests

- Success updates last-success and clears consecutive failures.
- Consecutive failures increment and set recent failure class.
- Rate-limit failure enters cooldown.
- Cooldown expires with fake/injected clock or a deterministic helper.
- Explicit provider requests do not silently skip cooled-down provider.

## Workstream 2: Shared routing decision builder

### Implementation

Extract provider selection/profile logic into a helper that all search tools can use.

Suggested function:

```rust
fn resolve_provider_routing(
    requested_providers: &[String],
    profile: Option<SearchProfile>,
    available_provider_ids: &[String],
    config: &AppConfig,
    health: &ProviderHealthRegistry,
    strict_explicit: bool,
) -> Result<ProviderRoutingDecision, ProviderRoutingError>
```

Use it for:

- `web_search` where applicable.
- `repo_search`.
- `security_search`.
- `research_search`.
- `repo_map` if native providers/fallback providers are selectable.

Do not force every tool into profile support if that would bloat schemas. The shared layer can still return standard telemetry for default/explicit provider selection.

### Tests

- Explicit unknown provider fails.
- Explicit disabled provider fails or is visible as explicit failure, matching existing semantics.
- Profile with one unavailable provider yields partial routing.
- Profile with all unavailable providers degrades to default providers.
- Cooled-down provider is skipped only for profile/default selection.
- Routing decisions are deterministic in provider order.

## Workstream 3: Capability enforcement telemetry

### Implementation

Add capability enforcement metadata to search responses where constraints matter.

For `repo_search`, track:

- repo identity enforcement.
- path enforcement.
- language enforcement.
- symbol enforcement.
- issue search enforcement.
- release search enforcement.
- freshness enforcement.
- package coordinate enforcement.

For `security_search`, track:

- advisory ID exact lookup.
- package/ecosystem filtering.
- version filtering.
- KEV support.
- severity support.
- exploit-context support.

For `research_search`, track:

- source-type diversity enforcement.
- primary-source preference.
- recency/freshness support.

Prefer a compact structure that agents can inspect without parsing prose warnings.

### Tests

- GitHub code provider enforces repo/path better than generic HTML provider.
- Generic provider emits not-enforced/approximated for repo/path/language.
- Freshness requested with no timestamp support yields not-enforced.
- Security exact CVE/GHSA ID lookup reports enforced when native advisory provider is used.

## Workstream 4: Provider status expansion

### Implementation

Extend `provider_status` to include health snapshot summaries.

Add fields such as:

```json
{
  "providers": [
    {
      "id": "github_code",
      "enabled": true,
      "configured": true,
      "health": {
        "status": "healthy|degraded|cooldown|unknown",
        "recent_failure_class": null,
        "recent_latency_ms": 123,
        "consecutive_failures": 0,
        "cooldown_until": null
      }
    }
  ]
}
```

Avoid exposing secrets, API keys, raw credential errors, or request URLs containing private tokens.

### Tests

- Provider status includes health field with defaults.
- Failed provider updates status to degraded/cooldown.
- Healthy provider updates status after success.
- Local workspace provider status remains accurate.

## Workstream 5: Documentation and agent instructions

Update README and AGENTS docs to describe:

- Strict explicit provider behavior.
- Profile/default provider degradation.
- Cooldown semantics.
- Capability enforcement telemetry.
- How agents should choose tools when provider capability is degraded.

Add one or two compact JSON examples for degraded provider routing.

## Compatibility requirements

- Do not remove existing fields.
- New telemetry fields must be optional/defaulted.
- Existing provider IDs must remain stable.
- Existing explicit provider validation behavior must not become silent fallback.
- Health state must be process-local and non-authoritative.

## Acceptance criteria

- Shared routing decision helper exists and is used by the major search paths where practical.
- Provider health snapshots are tracked process-locally.
- Provider cooldown affects profile/default routing but not silent explicit-provider behavior.
- Responses expose routing decision telemetry and capability enforcement telemetry.
- `provider_status` exposes health without secrets.
- Partial and degraded provider selection are deterministic and test-covered.
- `cargo fmt --check`, `cargo clippy --all-features --all-targets -- -D warnings`, and relevant tests pass.
