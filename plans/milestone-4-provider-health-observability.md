# Milestone 4 Plan: Provider Health Observability and Cooldown Semantics

## Objective

Make provider health state visible, deterministic, test-backed, and documented so operators and coding agents can understand why a provider is or is not currently routable.

This milestone builds on the existing process-local provider health registry. It should not introduce persistence unless the implementation cost is small and the semantics are very clear. The main goal is observability and correctness of current runtime behavior.

## Scope

In scope:

- provider health registry behavior;
- provider success/failure classification;
- inferred timeout recording;
- rate-limit classification;
- panic/error classification during dispatch;
- cooldown semantics if cooldown currently exists or is added;
- provider-status health fields;
- CLI provider/doctor health output;
- tests for health transitions;
- docs for process-local health semantics.

Out of scope:

- distributed provider health;
- persistent health database;
- provider scoring based on historical latency;
- automatic credential rotation;
- live network smoke tests as default CI;
- adding new provider backends.

## Relevant Code Areas

Primary files to inspect:

- `src/meta/provider_diagnostics.rs`
- `src/meta/adapter.rs`
- `src/meta/dispatch.rs`
- `src/core/provider.rs`
- `src/core/result.rs`
- `src/commands/providers.rs`
- `src/commands/doctor.rs`
- MCP provider-status tool handler files
- tests covering provider diagnostics, provider status, dispatch, recipes, and warnings
- `docs/provider-setup.md`
- `docs/tool-matrix.md`
- `docs/safety.md` or a new provider-health docs section

## Current Problem Statement

The adapter records provider successes, explicit failures, and inferred timeouts into a process-local provider health registry after dispatch. This is a strong base, but production readiness requires the semantics to be visible and testable.

An operator or coding agent should be able to answer:

- did this provider fail recently?
- what class of failure occurred?
- is this provider temporarily cooled down?
- is it disabled/misconfigured, or merely unhealthy?
- did it time out versus return a parse error?
- did it rate-limit us?
- will restart reset this state?
- does health affect routing, or only diagnostics?

## Design Requirements

### 1. Define provider health lifecycle

Document and test exactly how health transitions occur.

Required definitions:

- success: provider returned a successful response; define whether empty result sets count as success;
- failure: provider returned a classified `EngineError`;
- timeout: provider did not respond before the request deadline or engine timeout;
- panic: provider task panicked and was converted into a failure;
- rate-limit: HTTP 429 or provider-specific rate-limit response;
- cooldown: temporary state after repeated or severe failures, if supported;
- reset: success after failure should clear or reduce degraded state according to explicit rules.

### 2. Keep failure text bounded and safe

Provider errors may include remote text, URLs, or HTTP details. Any error string returned through MCP or CLI JSON should be bounded and sanitized enough to avoid unbounded logs or prompt injection. Prefer structured `error_class` plus concise bounded `message`.

### 3. Expose health in provider status

Add a compact health block to provider status or provider descriptor.

Suggested fields:

```rust
pub health: Option<ProviderHealthView>

pub struct ProviderHealthView {
    pub status: ProviderHealthStatus,
    pub failure_count: usize,
    pub last_error_class: Option<String>,
    pub last_error_message: Option<String>,
    pub cooldown_until: Option<String>,
    pub last_success_at: Option<String>,
    pub last_failure_at: Option<String>,
}
```

If timestamps complicate tests or require broader chrono use, omit them or use relative/counter semantics. The project already depends on `chrono`, so timestamps may be acceptable, but deterministic tests should not rely on exact wall-clock values.

### 4. Decide whether health affects routability

Two acceptable designs:

#### Diagnostic-only health

Provider health is reported but does not affect routing. This is simpler and avoids surprising users.

#### Routing-affecting health

Providers in cooldown are temporarily skipped, and provider status reports `routable = false` with skip code `cooldown`.

Either is acceptable, but the behavior must be explicit. If cooldown already exists, prefer making it visible rather than changing routing semantics.

### 5. Integrate with Milestone 2 provider skip codes

If Milestone 2 adds canonical skip/status codes, provider health should use those codes. For example, a configured/built provider in cooldown should report:

- `enabled = true`;
- `configured = true`;
- `built = true` if such a field exists;
- `routable = false` if cooldown affects routing;
- `skip_code = "cooldown"`;
- `health.status = "cooldown"`.

If cooldown is diagnostic only, keep `routable = true` but report `health.status = "degraded"` or `"cooldown_observed"` according to the actual design.

### 6. Keep process-local semantics unless explicitly changed

For this release, process-local health is acceptable. Document:

- health state starts empty on process start;
- health state is not written to disk;
- restart clears failures/cooldowns;
- live provider drift can still occur across process restarts;
- `provider_status` reflects only current process observations.

If persistence is added, it must be optional, bounded, and documented. Do not add persistence by accident.

## Implementation Steps

### Step 1: Audit existing provider health registry

Read `src/meta/provider_diagnostics.rs` and identify:

- stored fields;
- status enum variants;
- failure counters;
- cooldown logic, if any;
- record_success behavior;
- record_failure behavior;
- snapshot/export behavior;
- tests already present.

Write down any mismatches between actual code and docs/tests.

### Step 2: Audit adapter health recording

In `src/meta/adapter.rs`, review all paths that call provider health recording:

- `web_search`;
- `repo_search`;
- `security_search`;
- `research_search`;
- any multi-subquery dispatch path.

Ensure every queried provider is accounted for exactly once as success, explicit failure, or timeout. Be careful with multi-subquery dispatch: a provider may have both successes and failures across subqueries. Decide whether any success should prevent provider-level timeout/failure, or whether partial failures should still be visible.

Suggested rule:

- if provider has at least one successful response and one failed subquery, record success plus partial failure telemetry if available;
- do not report provider as wholly failed if it returned usable results;
- still expose partial failures in warnings if useful.

### Step 3: Normalize failure classes

Ensure error classes are stable and reused across provider failures, health state, and warning output.

Recommended classes:

- `timeout`;
- `http_status`;
- `rate_limited`;
- `parse_error`;
- `network_error`;
- `provider_error`;
- `panic` or `dispatch_panic`;
- `unknown`.

If the current code maps panics to `network_error`, consider adding a specific dispatch failure class if it does not destabilize too much. At minimum, tests should cover current behavior.

### Step 4: Add health view type

Add a serialized health view type in core/provider or diagnostics module. Keep internal registry representation separate from public view if needed.

Add conversion from registry snapshot to public view.

Bound any error message field:

- strip control characters;
- truncate to a small cap such as 240 or 512 chars;
- avoid embedding response bodies.

### Step 5: Attach health to provider_status

Update `provider_status()` construction to include health snapshot per provider. If provider has no recorded state, use either `None` or a default healthy/unknown state. Prefer explicit `status = "unknown"` or `"unobserved"` if that is more useful to agents.

Potential statuses:

- `unobserved`;
- `healthy`;
- `degraded`;
- `cooldown`;
- `disabled` should likely remain provider config status, not health status;
- `unavailable` if configured but not routable due to health.

### Step 6: Update CLI provider output

For human output, keep it short:

```text
duckduckgo  routable=true  health=healthy
github_code routable=false skip=api_key_env_unset health=unobserved
brave       routable=false skip=cooldown health=cooldown last_error=rate_limited
```

For JSON output, include full structured health view.

### Step 7: Add tests for health transitions

Unit tests in provider diagnostics should cover:

- initial provider state;
- record success;
- record failure;
- repeated failure increments count;
- success after failure resets or improves state according to documented rules;
- rate-limit failure maps correctly;
- timeout failure maps correctly;
- bounded error message;
- cooldown enter/exit if cooldown exists.

Adapter tests should cover:

- provider returns results -> success recorded;
- provider returns error -> failure recorded;
- provider task panic -> failure recorded;
- provider never responds before deadline -> timeout recorded;
- multi-provider partial response keeps successful provider healthy and failed provider degraded;
- multi-subquery partial failure behavior.

### Step 8: Add provider-status tests

Add tests that call provider status after simulated dispatch and assert health fields appear. If current test harness can use mock engines, use mock engines to force errors/timeouts deterministically.

Avoid real network tests.

### Step 9: Update warnings and recipes if needed

If provider health affects routability, ensure next-action hints and warnings can guide the agent:

- suggest using another provider;
- suggest checking credentials only for credential failures, not cooldown;
- suggest retrying later for rate-limit/cooldown;
- avoid claiming no provider is configured when the actual issue is temporary health.

### Step 10: Update docs

Add provider health docs to provider setup or tool matrix docs:

- state definitions;
- process-local behavior;
- examples of healthy/degraded/cooldown output;
- how health differs from config state;
- live provider drift policy.

Update release docs only if new release-gate commands are introduced, which should not be necessary.

## Testing Requirements

Targeted tests:

```bash
cargo test --all-features provider_diagnostics
cargo test --all-features provider_status
cargo test --features mock --test recipes_next_actions
cargo test --features mock --test schema_identity_registry
cargo test --all-features --test docs_provider_inventory
```

Then run broader checks:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --no-default-features
```

Use `make check` if available.

## Regression Risks

### Risk: Flaky time-based tests

Cooldown/timestamp tests can become flaky. Use injected clocks if already available, or assert broad properties rather than exact times. If injecting a clock would cause excessive refactor, keep tests focused on status transitions and presence of fields.

### Risk: Health suppresses useful providers

If health affects routing, be conservative. A provider with occasional parse errors should not be disabled too aggressively. Consider diagnostic-only health for this release unless cooldown already exists.

### Risk: Confusing config state with health state

Do not use health status to explain missing credentials or disabled config. Config/routing state and health state should be adjacent but distinct.

### Risk: Unbounded error text

Provider error messages must be bounded before exposure in MCP output.

## Deliverables

- Documented provider health lifecycle semantics.
- Provider health view exposed through provider status.
- CLI JSON and human output updated for health state.
- Tests for success/failure/timeout/rate-limit/panic/cooldown transitions.
- Tests showing provider-status health fields after simulated dispatch.
- Docs explaining process-local behavior and routing effect.

## Definition of Done

This milestone is complete when provider health behavior is deterministic, visible in provider status, tested without live network access, bounded for agent-facing output, and documented clearly enough that codegg can distinguish misconfiguration from temporary provider degradation.
