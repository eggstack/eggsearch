# M033 — Fetch-Side Pass-Cookie Reuse

Status: ready

Class: capability

Source roadmap: plans/subsystems/agentic-retrieval-integrity-roadmap.md (M033)

Requires: ADR-0017 acceptance (hard dependency); M031 closed (hard dependency — this milestone
reuses M031's shared orchestration module and its single-implementation guard). Interface
dependency: M014's published-lifetime and expiry-safety rules, which are closed and unchanged.

Planning baseline: 556462bb0b8e7c15150f20548e02fe00a1e03923 (current committed code candidate; M030 closure)

References: ADR-0009; ADR-0017 (proposed); M014 and its closure record; `architecture/engines.md` §
Challenge solving; `architecture/fetch.md` § Challenge pages on the HTTP path;
`src/meta/challenge/cookie.rs`; `src/meta/challenge/mod.rs`; `src/commands/doctor.rs`.

## Objective

A pass cookie issued on the fetch path is reused for later fetches to the same origin instead of
being solved again every time, and that reuse is bounded by the upstream-published lifetime and the
operator's ceiling exactly as the search-side reuse is.

## Current implementation evidence

- `src/meta/challenge/cookie.rs` owns published-lifetime parsing for every scheme. It resolves
  `Max-Age` over `Expires`, rejects a malformed attribute, rejects an expiry at or before now,
  and reports an unstated lifetime distinctly. M014 closed on the rule that reuse never exceeds a
  usable published lifetime or the configured ceiling.
- `src/meta/challenge/mod.rs` owns `PassCookieCache`: a process-local, bounded, TTL-scoped cache
  keyed by provider, with exact expiry eviction and a `remove` entry for a rejected replay.
- The search side uses both: an engine caches the issued pair set under the provider key and evicts
  it when the upstream rejects it. `doctor` warns when routed egress meets cookie reuse, because a
  token bound to the client's exit address is re-challenged when the address changes.
- The fetch path does neither. `architecture/engines.md` records that a fetch-side pass pair set is
  never cached for reuse and that `cookie_ttl_seconds` governs search upstreams only, so every
  challenged fetch pays a full solve — measured at roughly 3 seconds for a difficulty-6 Anubis wall
  on one core in a release build.

## Scope

In scope:

1. An origin-keyed, process-local cache for fetch-issued pass cookies, owned by the shared
   orchestration module introduced in M031.
2. Reuse bounded by the upstream-published lifetime and by `cookie_ttl_seconds`, using M014's
   unchanged expiry-safety rules.
3. Eviction when the upstream rejects the replayed cookie.
4. The operator warning that covers the fetch-side cache under routed egress.

Out of scope: any change to M014's lifetime rules or to the search-side cache; persistence of any
cookie to disk; a cross-origin or later-hop cookie; a rule-hash-keyed cache; a change to the
shipped `cookie_ttl_seconds` default; a new configuration key; a change to the Anubis or ALTCHA
solve logic.

## Invariants that cannot regress

- The cache is process-local. A pass cookie is never written to disk, never persisted to a cache
  directory, and never serialized into a response, a diagnostic, or a fixture.
- A cookie is reused only for the origin that issued it. It is never replayed to another origin.
- Reuse never exceeds an applicable published lifetime and never exceeds `cookie_ttl_seconds`. An
  unstated lifetime retains the configured-ceiling fallback; a malformed lifetime, a zero lifetime,
  or an expiry at or before now stores nothing and reuses nothing.
- A pair set the upstream rejects is evicted, and the next request starts cold.
- The cache is bounded in entry count, and its capacity and byte accounting are explicit.
- No cookie value, nonce, salt, challenge id, or pass token appears in a log, an error, a warning, a
  `Debug` output, a fixture, or a `provider_status` block.
- The single-replay rule is unchanged: one solve and one replay per fetch call, and a fetch never
  reuses a cookie to avoid a solve inside the same call.
- `src/fetch/**` imports no solver and no cache.
- The `doctor` warning for routed egress plus cookie reuse still fires exactly once, and now
  covers the fetch-side cache as well.

## Expected production changes

1. **An origin-keyed cache owned by the shared orchestration module.** It reuses M014's published
   lifetime resolution rather than re-implementing it, applies the same configured ceiling, evicts
   on rejection, and stays inside an explicit entry-capacity bound.
2. **Reuse on the fetch path.** When a fetch meets a supported published challenge, the shared
   module first checks the origin-keyed cache for an unexpired, usable pair set. A hit replays the
   request once with that pair set and skips the solve. A miss solves as today and stores the issued
   pair set under its published lifetime.
3. **The operator warning.** The `doctor` check that warns when routed egress meets cookie reuse
   is extended to the fetch-side cache, keeping the one-warning behavior and the existing
   remediation text.

## Migration and compatibility effects

No MCP tool, argument, or advertised-schema change. No configuration key is added, removed, or
renamed, and the shipped `cookie_ttl_seconds` default stays at its current value. Behavior change:
a second challenged fetch to the same origin may now reuse a still-valid pass cookie instead of
solving again. That is a cost and latency reduction, not a correctness change, and it is bounded by
the same lifetime rules the search side already uses.

## Ordered work packages

1. **Extend the lifetime unit tests** to cover the fetch-side key and the ceiling interaction
   before any cache exists, so the expiry rules are proven independently of the cache.
2. **Implement the origin-keyed cache** in the shared orchestration module, reusing M014's lifetime
   resolution, with explicit capacity bounds and eviction on rejection.
3. **Wire reuse into the fetch path** ahead of the solve, keeping the one-solve-one-replay rule and
   the unchanged-response-on-refusal behavior.
4. **Extend the `doctor` warning** to the fetch-side cache.
5. **Update the documentation set:** `architecture/engines.md` § Challenge solving,
   `architecture/fetch.md`, `docs/features.md`, and the operator configuration documentation.
6. **Run the verification below** and record the outcomes.

## Verification

Focused tests, using the repository-pinned Rust 1.89 toolchain:

```bash
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH cargo test --locked --all-features --lib meta::challenge
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH cargo test --locked --all-features --lib meta::engines::startpage
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH cargo test --locked --all-features --test web_fetch_integration
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH cargo test --locked --all-features --test static_guards
```

Broad gate:

```bash
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH make check
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH make docs-check
```

All new tests use loopback fixtures and never a public upstream. A live two-call observation on a
real protected origin is optional evidence and is recorded as such when it is not run; it is never
a substitute for the deterministic expiry and eviction tests.

## Static guards and documentation to update

- The guard from M031 that keeps `src/fetch/**` free of solver imports must still pass.
- Add a guard that no pass cookie is persisted to disk and that no fetch-side cache is written
  outside the process-local owner.
- Documentation: `architecture/engines.md`, `architecture/fetch.md`, `docs/features.md`, and
  the operator configuration documentation.

## Acceptance criteria

1. A second challenged fetch to the same origin replays the cached pair set and performs no solve,
   proven by a loopback test that counts solve attempts.
2. A fetch after the published lifetime expires solves again rather than replaying an expired
   cookie.
3. A fetch with a malformed published lifetime solves again and stores nothing.
4. A replay rejected by the upstream evicts the entry, and the next fetch solves again.
5. A cached pair set is never replayed to a different origin, and a test asserts the request
   carrying it targets the issuing origin.
6. The cache is bounded, and a saturation test proves the bound holds.
7. The `doctor` warning fires exactly once when routed egress meets cookie reuse, and its text
   covers the fetch-side cache.
8. No cookie material appears in any log, error, warning, `Debug` output, fixture, or diagnostic
   payload.
9. The broad gate passes on the exact candidate, and every test that was run is recorded with its
   outcome, including anything deliberately not run.

## Stop conditions

Stop and leave M033 open if ADR-0017 or M031 is not closed; if reuse cannot be bounded by the
published lifetime and the configured ceiling together; if a pass cookie would have to be persisted
to disk to be useful; if the origin key cannot be derived without following a redirect; if the
rejection-eviction path cannot be proven; or if the implementation would change M014's lifetime
rules or the search-side cache.

## Required closure evidence

`plans/closure/agentic-retrieval-integrity/033-status.md` records the implementation commits, a
requirement-to-evidence matrix for each acceptance criterion, the focused and broad command
outcomes including anything unrun, the unchanged-safety evidence, the secret-handling evidence, the
documentation set that was updated, known limitations, residual risk, and the recommendation. The
closure commit carries this plan, the closure record, the roadmap status, and the registry row
together.
