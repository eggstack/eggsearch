# M030 M014 Expiry-Safety Acceptance Reconciliation — Closure Evidence

Status: closed

Source plan: `plans/implementation/agentic-retrieval-integrity/030-m014-solve-count-acceptance-reconciliation.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

M014 record: `plans/closure/agentic-retrieval-integrity/014-status.md`

## Decision and scope

The user explicitly selected M030 path 1: “lets re-scope m014 to expire safely. go ahead with m030.” This supersedes only M014's unmet solve-count-reduction acceptance clause. M014's original implementation plan is preserved. No production behavior or source file was changed; no commit was made.

## Exact code candidate

`46fda7764387fc35e9babca54036e80ed250ae78` — committed code candidate at the M029 closure. The M030 changes are planning/closure documentation only; the focused code tests below ran against this exact candidate.

## Accepted M014 criterion and evidence mapping

1. Expiry bound: the cache lifetime is limited by a usable published Max-Age/Expires and the configured cookie_ttl_seconds ceiling. Expired, zero, or malformed published lifetimes yield no reuse; a rejected replay evicts the cookie. The deterministic resolver tests cover future expiry (honored/clamped), past/tie expiry, malformed attributes, zero ceiling, and unstated fallback.
2. Published-lifetime provider: the recorded Startpage pass response published an Expires about 20 minutes after issuance. The live run used cookie_ttl_seconds=1800. At elapsed 1,261.4 seconds, the second search had cookie_hits=0 and cookie_misses=2 for the run, attempted and succeeded in two solves overall, and returned three rows. It missed the expired cache entry, received a fresh challenge, and solved successfully; it did not replay the dead cookie. This satisfies expiry-safety evidence only.
3. No-lifetime provider: the captured Mojeek pass response published no lifetime. Its reuse continues to fall back to the configured ceiling; no changed behavior is claimed.
4. Solve-count result: the planned two-arm comparison was not run. No solve-count reduction is demonstrated or claimed. Cache misses, avoided rejected-token requests, and successful solves are not substituted for a solve-count delta.

## Verification

Using the pinned Rust 1.89 toolchain on candidate 46fda7764387fc35e9babca54036e80ed250ae78:

- `cargo test --locked --all-features --lib meta::challenge`: 118 passed, 0 failed.
- `cargo test --locked --all-features --lib meta::engines::startpage`: 51 passed, 0 failed.
- `cargo test --locked --all-features --lib meta::engines::mojeek`: 20 passed, 0 failed.
- Pinned-toolchain `make docs-check`: passed (`cargo doc --locked --all-features --no-deps`).
- Pinned-toolchain `make packaging-check`: passed, including `planning-consistency: ok`, workflow-pin, Eggpack qualifier, and egress qualification contracts.
- `git diff --check`: passed (exit 0).

## Status disposition and residual risk

M014's solve-count clause is not met; only that clause is superseded by the user-selected expiry-safety criterion. The replacement criterion is supported by resolver and engine tests plus the recorded Startpage post-expiry observation. M014 and M030 close together in the commit carrying this closure record, the plan, roadmap, and registry. M029 is unchanged.

The evidence is provider-observation-specific: upstream attributes can drift, and Mojeek currently publishes no lifetime. The existing `Expires` wall-clock conversion has the documented bounded clock-step risk (no more than the configured ceiling). No fresh live traffic was generated for M030.
