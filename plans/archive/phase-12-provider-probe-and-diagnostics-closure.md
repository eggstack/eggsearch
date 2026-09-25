# Phase 12 — Provider Probe and Diagnostics Closure

Status: implemented
Depends on: phase 11 preferred; may proceed once shared execution seams are stable
Baseline for planning: `4a713ff82cec701534e285bbe3d330ae121f352c`
Roadmap: `plans/maintenance-codegg-quality-roadmap.md`

## Objective

Unify provider liveness/probe logic behind one core service and expose it consistently through CLI diagnostics, MCP `provider_status(probe=true)`, and explicit live-smoke tests. Close capability-reporting drift so CodeGG and other MCP-only consumers can distinguish configured, routable, healthy, degraded, rate-limited, and actually probed providers without invoking an external CLI.

## Current problem

Provider health tracking already records failure class, consecutive failures, cooldown state, latency, and recent success/failure metadata. `eggsearch doctor --probe` and live-smoke paths can perform active checks, but `provider_status(probe=true)` is documented as accepted-but-unimplemented. This produces two diagnostic planes for the same provider subsystem and prevents MCP-only agents from verifying current provider availability directly.

## Non-goals

- No continuous background monitoring daemon.
- No mandatory live probing during ordinary `provider_status` calls.
- No live-network probes in routine CI.
- No provider-specific benchmark framework.
- No new general-purpose search providers.

## Invariants

1. Active probes occur only when explicitly requested by CLI/MCP/live-smoke callers.
2. Probe requests are bounded by per-provider and aggregate deadlines.
3. Probe failures update advisory health state but must not silently override explicit provider selection beyond existing hard configuration/safety rules.
4. Credentials and sensitive upstream response bodies are never echoed into diagnostics.
5. A provider with missing credentials/config is reported as non-probable/routable with a stable skip code rather than generating a misleading network failure.
6. Keyless providers remain probeable without introducing credential requirements.
7. Routine `make check` stays deterministic and network-free.

## Production changes

### 1. Create a shared provider probe service

Extract a core/internal service, for example:

```text
ProviderProbeRequest
ProviderProbeOutcome
ProviderProbeService
```

A probe outcome should carry stable machine-readable fields such as:

```text
provider_id
attempted
routable
success
failure_class
latency_ms
http_status (when safe/relevant)
skip_code
message (bounded/sanitized)
```

Avoid exposing raw provider payloads.

The probe should exercise the narrowest valid provider request that proves transport/auth/parser viability. Do not consume large result sets or expensive semantic/deep-search modes merely for liveness.

### 2. Reuse the service in `doctor --probe`

Replace any parallel CLI-only provider probing with the shared service. CLI formatting remains CLI-specific, but outcome semantics must come from the same structures used by MCP.

### 3. Implement `provider_status(probe=true)`

When `probe=false` or omitted, preserve current cheap process-local behavior.

When `probe=true`, perform bounded active probes and return a typed probe section. Preserve existing provider descriptors/health fields; this is additive.

Suggested shape:

```json
{
  "probe": {
    "requested": true,
    "implemented": true,
    "started": 5,
    "succeeded": 4,
    "failed": 1,
    "skipped": 3,
    "outcomes": [...]
  }
}
```

Exact field names should follow existing serialization conventions.

### 4. Add concurrency and budget policy

Probes should run concurrently within a small cap. Add explicit configuration/constants for:

- per-provider timeout;
- aggregate timeout;
- maximum concurrent probes;
- maximum diagnostic message length.

Do not let one hung provider delay all diagnostics to the sum of timeouts.

### 5. Add provider conformance tests

Create deterministic mock-backed tests covering:

- configured/routable success;
- missing API key/base URL/config;
- timeout;
- HTTP error;
- parser failure;
- rate limit;
- panic containment if provider dispatch already supports it;
- cooldown interaction;
- explicit provider request semantics after degraded health;
- bounded/sanitized error messages.

Where capabilities differ, add table-driven provider descriptor tests for native versus approximate constraint enforcement.

### 6. Reconcile capability documentation

Audit `docs/tool-matrix.md`, `docs/provider-setup.md`, `architecture/engines.md`, and provider descriptors against code. In particular, remove contradictions around native/local domain filtering and any other recently changed capability flags.

Make provider descriptors/code the source of truth where possible, and generate or contract-test documentation inventories from stable descriptor data instead of duplicating hand-maintained claims.

### 7. CodeGG-facing capability negotiation

Ensure the MCP status response gives CodeGG enough information to decide whether to use:

- ordinary web search;
- credentialed semantic/provider paths;
- local workspace search;
- browser rendering;
- PDF support;
- security databases;
- structured repository backends.

Do not add a CodeGG-only endpoint. Use additive fields in `provider_status`/server capabilities.

## Verification

Routine deterministic verification remains `make check`.

Add explicit live verification commands such as:

```text
eggsearch doctor --probe
# MCP provider_status {"probe": true}
make live-smoke
```

Live checks should be documented as environment-dependent and must tolerate unconfigured optional providers by classifying them as skipped rather than failed.

## Acceptance criteria

- CLI doctor, MCP provider probing, and live-smoke code use one core probe implementation.
- `provider_status(probe=true)` performs real bounded probes and no longer reports `implemented: false`.
- mock-backed deterministic conformance coverage exists for success/failure/skip/cooldown cases.
- provider diagnostics never leak credentials/raw response bodies.
- capability documentation matches provider descriptors and contract tests guard common drift.
- CodeGG can determine actual live/provider capability state over MCP alone.
- routine CI remains network-free and `make check` passes.
