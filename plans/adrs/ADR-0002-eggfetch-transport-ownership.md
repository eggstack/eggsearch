# ADR-0002: Eggfetch Transport Ownership with SSRF Policy in Eggsearch

Status: accepted

Date: 2026-09-25

Decision owners: project maintainers

Related specification sections:

- `plans/000-long-term-specification.md#4`

Affected subsystem roadmaps:

- `plans/subsystems/transport-consolidation-roadmap.md`
- `plans/subsystems/optional-outbound-routing-roadmap.md`

## Context

Eggsearch makes outbound HTTP for provider search, fetch, probes, and
updates. Transport mechanics (pooling, TLS, decompression, deadlines,
redirects) and application policy (SSRF, retry, truncation) were at risk of
blurring across ad-hoc reqwest clients.

## Decision drivers

- One correct total-deadline body lifecycle.
- Pinned resolved-address routing for untrusted targets.
- A single retry/circuit authority.

## Considered options

### Option A — Eggfetch owns transport, eggsearch owns policy

`eggfetch-core` owns HTTP framing, pooling, TLS, decompression, total
deadlines, pinned routing mechanics, and typed failures. Eggsearch owns
SSRF/retry/truncation policy; `OriginController` remains the sole
retry/circuit authority. No eggsearch-local reqwest clients.

### Option B — Per-caller reqwest clients

Each caller builds its own client with local timeout/redirect logic.

## Decision

Option A. Direct reqwest is removed from eggsearch-owned paths; rmcp's
transitive reqwest Streamable HTTP client remains an intentional documented
boundary. The bounded eggfetch feature graph must not accidentally enable
unneeded capabilities.

## Consequences

Positive: shared client and connection reuse; correct body-lifecycle
deadlines; typed failure evidence.

Negative: transport upgrades (for example eggfetch 0.2.0) require
downstream proof through eggsearch's own bounded-body path plus
requalification.

## Compatibility and migration

Transport upgrades preserve the ownership boundary and timeout/client-reuse
semantics; the compression workaround retirement followed this rule.

## Security implications

Untrusted dynamic fetch targets require pinned resolved-address snapshots;
a process-global hostname-to-IP side channel is not acceptable. Redirects
stay bounded with downgrade denial.

## Verification

`provider_request_contract`, dispatch fault-injection, forge-safety, and
`static_guards.rs` transport guards plus seven-target qualification on the
exact candidate.

## Supersession

None.
