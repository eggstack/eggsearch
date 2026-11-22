# M030 — M014 Expiry-Safety Acceptance Reconciliation

Status: closed

Class: invariant (corrective to M014's evidence contract)

Source roadmap: plans/subsystems/agentic-retrieval-integrity-roadmap.md

Corrects: supersedes only M014's unmet solve-count-reduction acceptance clause; it does not rewrite M014's implementation plan, reopen its implementation, or change any closed milestone.

Planning baseline: 46fda7764387fc35e9babca54036e80ed250ae78 (current committed code candidate; M029 closure)

## Objective

Record the user's selected path 1 for M030: re-scope M014 success to safe pass-cookie expiry handling. Preserve the explicit evidence boundary that avoiding a dead-cookie replay does not demonstrate fewer solves. Make no production changes.

## Accepted decision

The user selected path 1 with: “lets re-scope m014 to expire safely. go ahead with m030.” This authorizes the M030 acceptance reconciliation, not a change to production behavior or a commit.

Only M014's solve-count-reduction acceptance clause is superseded. The M014 implementation plan remains unchanged. Its replacement exit criterion is:

1. Never reuse a cookie past its published Max-Age/Expires lifetime or the configured cookie_ttl_seconds ceiling; a known-expired or malformed lifetime disables reuse, and a rejected replay evicts the cookie.
2. Demonstrate the expiry boundary with a provider that publishes a lifetime. Existing Startpage live evidence observes a cache miss and fresh successful solve after its published expiry.
3. Record providers that publish no lifetime as unchanged: Mojeek continues to fall back to the configured ceiling.
4. Make no solve-count-reduction claim. M014's planned two-arm solve-count comparison was not run and is no longer the selected criterion.

## Evidence map

- src/meta/challenge/cookie.rs implements reuse_ttl as the minimum of the usable published remaining lifetime and the configured ceiling; Unstated falls back to that ceiling, while malformed, zero, and expired lifetimes yield no reuse. Its deterministic unit tests cover these boundaries without sleeping.
- Startpage engine-flow tests cover zero Max-Age, ceiling-clamped lifetime, zero-TTL repeat solves, and rejected-cookie eviction.
- The M014 live record captures Startpage Expires (~20 minutes) and a run with a 1,800-second ceiling. At 1,261.4 seconds, the second search recorded cookie_hits=0, cookie_misses=2, two total successful solves, and three result rows; it took a miss and solved afresh after expiry. This is expiry-safety/reliability evidence, not a solve-count comparison.
- The captured Mojeek pass response published no lifetime; fallback to the configured ceiling is unchanged.

## Scope and non-goals

In scope: record the selected acceptance path, map existing deterministic and live evidence to it, and reconcile M030/M014 closure evidence, roadmap, and registry.

Out of scope: changing cookie parsing, expiry, cache behavior, the 240-second default, egress behavior, solver budgets, providers, public interfaces, or any production file; changing M014's original implementation plan; claiming fewer solves based on fewer stale-token requests; reopening M029 or the tool-surface roadmap.

## Invariants

- Reuse is bounded by both an applicable published lifetime and the operator ceiling; an unstated lifetime retains the existing configured-ceiling fallback.
- A known-expired, malformed, or rejected cookie is not reused as a valid cache hit.
- Solve attempts, cache misses, rejected replays, successful searches, and solve-count deltas are distinct measurements.
- No solve-count reduction is claimed; the two-arm comparison remains unrun.
- No production behavior changes without a separately approved implementation scope.
- No closed milestone or the tool-surface roadmap is reopened.

## Ordered work packages

1. Map the resolver and engine-flow tests to expiry, ceiling, and no-lifetime behavior — complete.
2. Record the user's explicit path-1 decision and its narrow effect on M014 acceptance — complete.
3. Reuse the existing Startpage post-expiry live observation and Mojeek no-lifetime capture; make no new live requests because the selected criteria already have evidence — complete.
4. Update M014 closure evidence, this M030 record, the subsystem roadmap, and registry; run focused challenge tests and planning checks — complete.
5. Do not change production code; commit the closure set only after explicit user approval.

## Verification

Focused Rust tests use the repository-pinned Rust 1.89 toolchain:

```bash
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH cargo test --locked --all-features --lib meta::challenge
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH cargo test --locked --all-features --lib meta::engines::startpage
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH cargo test --locked --all-features --lib meta::engines::mojeek
```

Planning-only validation: run make docs-check, make packaging-check, and git diff --check with the same pinned toolchain path. No production change is in scope, so a fresh live comparison and full make check are not acceptance requirements for M030.

## Acceptance criteria

1. The user's path-1 decision and the exact clause it supersedes are explicit.
2. The expiry-safety criterion is mapped to deterministic resolver/engine tests and the existing Startpage post-expiry observation.
3. Mojeek's no-lifetime fallback is recorded as unchanged.
4. The records state that the solve-count comparison was not run and no reduction is claimed.
5. M014's original implementation plan and production behavior are unchanged.
6. M014 and M030 close together in the exact closure commit; M029 stays untouched.

## Stop conditions

Stop and leave M014 conditional if the published-lifetime bound is contradicted by code or tests, if a live expiry observation cannot be tied to the published expiry, or if closure would require production changes or treating stale-token avoidance as a solve reduction.

## Required closure evidence

plans/closure/agentic-retrieval-integrity/030-status.md records the selected decision and its authority, exact code candidate SHA, requirement-to-evidence mapping, commands and outcomes, separate solve-count and stale-token conclusions, M014's disposition, residual risk, and the roadmap/registry update. The authorized closure commit must carry this plan, both closure records, the roadmap, and registry together.
