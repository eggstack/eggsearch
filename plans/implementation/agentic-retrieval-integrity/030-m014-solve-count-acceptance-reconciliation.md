# M030 — M014 Solve-Count Acceptance Reconciliation

Status: proposed

Class: invariant (corrective to M014's evidence contract)

Source roadmap: plans/subsystems/agentic-retrieval-integrity-roadmap.md

Corrects: the unresolved acceptance condition in M014; it does not reopen M014's implementation or change any closed milestone.

Planning baseline: 3416da6231702dcabc22b2c8e32e94dd88cb20e (M029 implementation candidate)

## Objective

Resolve the mismatch between M014's requested solve-count reduction and its implemented expiry-safe cache policy. Preserve the explicit rule that avoiding a dead-cookie replay is not evidence of fewer solves. Obtain an explicit acceptance-path decision before changing M014's status or adding production behavior.

## Current evidence

- The former fixed-window policy stores a pass cookie for the configured ceiling, `cookie_ttl_seconds`.
- The derived policy stores no more than that ceiling: an upstream lifetime is clamped to it, an unstated lifetime falls back to it, and an expired or malformed lifetime disables reuse. Therefore the derived TTL is never greater than the fixed TTL. With the same request schedule, this policy can preserve or shorten reuse; it cannot establish fewer solves than the fixed-window baseline.
- The captured Startpage pass cookie publishes an approximately 20-minute `Expires`, longer than the shipped 240-second ceiling, so the default clamp binds. The captured Mojeek pass response publishes no lifetime, so its fallback is unchanged.
- The existing 21-minute Startpage observation at a raised ceiling recorded a second solve after expiry and no dead-token replay. It is expiry-safety evidence only; the planned two-arm solve-count comparison was not run.
- `plans/closure/agentic-retrieval-integrity/014-status.md` records the live evidence and keeps M014 conditionally closed. It explicitly rejects treating stale-token avoidance as a solve-count reduction.

## Decision required

Choose one path before this plan becomes active:

1. **Re-scope M014 to expiry safety.** Explicitly supersede only its solve-count-reduction acceptance clause with a measurable validity/reliability criterion: never reuse beyond a published lifetime or the operator ceiling; demonstrate the expiry boundary with a provider that publishes a lifetime; record providers with no published lifetime as unchanged. Solve counts remain unclaimed. The corrective closure record must say the original reduction was not demonstrated and identify the accepted superseding criterion.
2. **Retain solve-count reduction as a required outcome.** Keep M014 conditional and write a separate, user-approved implementation plan for a mechanism that can actually reduce solves while respecting both upstream expiry and the operator ceiling. Any default/configuration change or TTL-policy widening requires its own explicit decision and, where required, an ADR. A two-arm measurement alone cannot satisfy this path if its result does not show fewer solves.
3. **Defer.** Keep M014 conditionally closed and make no claim of solve-count reduction until a feasible acceptance path is selected.

This proposal does not select a path on the user's behalf. Until selected, M014 remains conditionally closed.

## Scope and non-goals

In scope: establish the arithmetic and evidence boundary; obtain the user's acceptance-path choice; register any resulting implementation or evidence plan; update M014's status only with explicit authority and exact evidence.

Out of scope: changing cookie parsing, expiry, cache behavior, the 240-second default, egress behavior, solver budgets, provider behavior, or public interfaces; generating live traffic before a feasible measurement is selected; changing M014's original plan in place; or claiming a solve reduction from avoiding rejected or expired tokens.

## Invariants

- Reuse never exceeds the configured ceiling or an upstream-published lifetime.
- A known-expired, malformed, or rejected cookie is not replayed as a valid cache hit.
- Solve attempts, cache misses, rejected replays, and successful searches are distinct measurements; do not substitute one for another.
- The user selects any changed acceptance contract. A proposal is not approval, and M014 stays conditional until its selected path is completed.
- No closed milestone or the tool-surface roadmap is reopened.

## Ordered work packages

1. Map `reuse_ttl` and its unit/engine tests to the fixed-window comparison. Preserve the proof that the derived TTL is bounded by the configured ceiling.
2. Obtain an explicit choice among the three decision paths above. Do not run a long live comparison while the chosen outcome is unknown.
3. For path 1, define the new validity/reliability acceptance in this corrective plan, map existing unit and live evidence to each criterion, and run only any missing bounded evidence. Keep solve counts explicitly unproven. The M014 status disposition must state that its original solve-reduction claim was not met and that the replacement criterion was accepted here.
4. For path 2, prepare a separate bounded implementation plan and any required ADR. State the mechanism and its safety bound before attempting a live A/B run; if no mechanism can meet the original target safely, leave M014 conditional.
5. For path 3, leave M014 conditionally closed, record the deferral, and stop without changing its acceptance status.
6. When an approved path is completed, update the roadmap, registry, and a new M030 closure record together. Preserve the M014 evidence record and link the superseding decision rather than silently rewriting history.

## Verification

Planning-only changes: `make docs-check`, `make packaging-check` (including planning consistency), and `git diff --check`.

Any production change requires focused challenge-cookie tests, `make check` on the exact candidate with the repository's pinned Rust 1.89 toolchain, and fresh live evidence only when the selected plan authorizes it. Live evidence must record the request schedule, provider, ceiling, published attributes (without cookie values), solve-counter deltas, cache hits/misses, result outcomes, and elapsed window for both arms. Never persist or print cookie values.

## Acceptance criteria

1. The derived-versus-fixed TTL relationship is documented accurately and supported by the existing resolver/unit-test evidence.
2. The user-selected path is recorded explicitly; no proposal is treated as an accepted change.
3. Stale-token avoidance is separated from solve-count reduction in the roadmap, registry, M014 evidence, and any new closure record.
4. M014 remains conditionally closed unless the selected replacement/retained criterion is met on its exact evidence candidate.
5. No change is made to production behavior without a separate approved implementation scope.

## Stop conditions

Stop and leave M014 conditional if no acceptance path is selected; if a purported solve-count gain requires replay beyond upstream expiry or above the configured ceiling; if the only evidence is a dead-cookie rejection avoided; or if the live comparison cannot hold provider, request schedule, and cache state comparable across arms.

## Required closure evidence

Create `plans/closure/agentic-retrieval-integrity/030-status.md` only after the selected path is completed. Record the selected decision and its authority, exact candidate SHA, requirement-to-evidence mapping, commands and outcomes, live-arm details if applicable, the separate solve-count and stale-token results, M014's final status disposition, residual risk, and the roadmap/registry update carried in the same commit.