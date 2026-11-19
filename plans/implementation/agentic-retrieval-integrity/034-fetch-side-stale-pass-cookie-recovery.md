# M034 — Fetch-Side Stale Pass-Cookie Recovery

Status: closed

Class: capability

Source roadmap: plans/subsystems/agentic-retrieval-integrity-roadmap.md (M034)

Requires: ADR-0017 acceptance (hard dependency — the solve contract this milestone refines);
ADR-0018 acceptance (hard dependency — the in-call recovery and the refined replay bound this
milestone implements); M031 closed (hard dependency — this milestone reuses M031's shared
orchestration module and its single-implementation guard); M033 closed (hard dependency — this
milestone corrects M033's known limitation #1 and reuses M033's origin-keyed cache).

Planning baseline: 2c1a7225ac22caa95fc21a78d9016f78ce3cb807 (current committed code candidate; M033 closure)

References: ADR-0009; ADR-0010; ADR-0017 (accepted); ADR-0018 (accepted); M033 and its closure record;
`architecture/fetch.md` § Fetch-side pass-cookie reuse; `src/challenge_replay/mod.rs`;
`src/challenge_replay/cache.rs`; `src/meta/challenge/cookie.rs`;
`src/meta/challenge/ledger.rs`.

## Objective

A fetch that discovers the stored pass pair set the upstream refuses recovers inside the same
call: the refused entry is evicted and the call solves the challenge it was already holding,
replays once with the freshly issued pair set, and returns the document. The caller never has to
await a second fetch to get a document.

## Current implementation evidence

- `src/challenge_replay/mod.rs` owns the shared detect → solve → replay → classify sequence.
  `replay_challenge` consults the origin-keyed cache before it solves; on a hit it calls
  `replay_cached`, which replays the request once with the stored pair set.
- `replay_cached` detects the refusal: when the replayed response is itself a challenge page, it
  calls `fetch_pass_cookie_cache().remove(origin)` and returns `None`. The caller therefore keeps
  the original marked challenge page.
- The consequence is recorded as M033's known limitation #1: "A stale pair set costs one degraded
  fetch. When the upstream's challenge changes, the stored pair set is rejected; that fetch returns
  the original marked response, the entry is evicted, and the next fetch solves."
- The refusal is detected from the response the replay already produced, so the information needed
  to recover is in hand at the eviction site. No extra request, no extra detection, and no second
  challenge page is needed: the call already holds the challenge page it was about to solve.
- The search side does not self-heal. `src/meta/engines/startpage.rs` removes the provider entry
  and returns `Err(blocked(BOUND_COOKIE_REJECTED))` on a rejected replay. This milestone is
  scoped to the fetch path; the search-side behavior is unchanged.

## Scope

In scope:

1. The fetch-path self-heal: a refused cached pair set evicts the entry and the same call performs
   one bounded solve of the challenge page it already holds, followed by one replay.
2. The behavioral tests that prove the recovery, the eviction, the unchanged budget, and the
   unchanged original response when the recovery itself fails.
3. The documentation of the rule.

Out of scope: any change to the solve contract's ceilings; any change to M014's lifetime rules; any
change to the search-side `PassCookieCache` or the search-side rejection behavior; any change to
the origin key, the same-origin gate, or the first-hop-only cookie attachment; a change to the
shipped `cookie_ttl_seconds` default; a new configuration key; a change to the Anubis or ALTCHA
solve logic; a change to the tool surface or any advertised schema; a change to the refusal
vocabulary; any change to ADR-0009's prohibition list.

## Invariants that cannot regress

- At most one solve and one replay of a solved pair set per fetch call, as ADR-0018 records. The
  refused pair set is presented at most once per call as a reuse probe, and the call's single solve
  produces the pair set its single replay uses.
- Every solve stays inside `max_difficulty`, `max_attempts`, `timeout_ms`, and the remaining
  call deadline.
- The replayed cookie is attached on the first hop only, so a redirect cannot forward it to another
  origin.
- A cached pair set is never replayed to an origin other than the one that issued it, and the
  same-origin gate is unchanged: a fetch that crossed an origin stores nothing and reuses nothing.
- The reuse window is M014's rule, unchanged: never past a published expiry, never on a malformed
  lifetime, clamped by `cookie_ttl_seconds`. The recovery does not extend the window.
- A refused pair set is evicted before the solve, so it is never presented to the upstream again.
- The cache stays process-local, is never written to disk, and is never serialized into a response,
  a diagnostic, or a fixture.
- No cookie, nonce, salt, challenge id, or pass token appears in a log, an error, a warning, a
  `Debug` output, a fixture, or a `provider_status` block.
- A recovery that refuses, errors, exhausts the deadline, or replays into another challenge page
  leaves the original response exactly as it was.
- With the opt-in off, every surface behaves exactly as it does today: the cache is never consulted
  and no solve is attempted.
- ADR-0009's prohibition list is unchanged and unexercised.

## Expected production changes

1. **The self-heal in the shared orchestration module.** `src/challenge_replay/mod.rs` changes the
   reuse path so that a refused cached pair set does not end the call. The refusal site evicts the
   entry and the call continues into the ordinary solve path with the challenge page it already
   holds, then the ordinary replay, then the ordinary storage of the freshly issued pair set.
2. **No new module, no new configuration key, and no change to the cache.** The cache keeps its
   owner, its bounds, its eviction semantics, and its `Debug` redaction. The recovery only adds
   the continuation after the eviction.
3. **No change to the solver, the derivation, or the transport.** The Anubis and ALTCHA branches
   are entered exactly as they are on the cold path.

## Migration and compatibility effects

- No MCP tool, argument, or advertised-schema change. The advertised inventory is unchanged and the
  byte-pinned advertisement still matches.
- No configuration key is added, removed, or renamed, and no shipped default changes.
- `Cargo.toml` is unchanged, so no dependency moved.
- The refusal vocabulary keeps its ten keys and the `provider_status` `challenge_solver` block is
  unchanged.
- Observable behavior change: a fetch that meets a refused cached pair set now returns the
  document instead of the marked challenge page. Before the change the same call returned the
  marked page and the next fetch solved; after the change the same call solves and returns the
  document. The number of upstream requests in that call rises by one, because the refused replay
  is still made.

## Ordered work packages

1. **Write the rule into `architecture/fetch.md` before the code.** The stale-pair-set paragraph
   states the eviction, the continuation, the budget, and the unchanged original response when the
   recovery fails. The rule is reviewable independently of the implementation.
2. **Record the scope decision as ADR-0018.** The in-call recovery refines ADR-0017 clause 5, whose
   literal wording forbids a call from both replaying a stored pair set and then solving. ADR-0018
   authorizes the recovery and states the refined bound: one solve and one replay of a solved pair
   set per call, with the refused pair set presented at most once as a reuse probe.
3. **Implement the continuation** in `src/challenge_replay/mod.rs`, reusing the existing solve and
   replay paths rather than duplicating them.
4. **Extend the behavioral suite.** Extend `tests/web_fetch_integration.rs`; do not create a new
   suite file. Prove the recovery returns the document, that the refused pair set is presented once
   and evicted, that the call performs exactly one solve, that a failed recovery leaves the
   original response unchanged, and that the disabled opt-in is unaffected.
5. **Update the documentation set:** `architecture/fetch.md` § Fetch-side pass-cookie reuse (the
   rule), `architecture/engines.md` § Challenge solving, `docs/features.md`, and
   `docs/test-inventory.md`.
6. **Run the verification below** and record the outcomes.
7. **Record the M033 live qualification** in the closure record, including what the real protected
   origin did and did not demonstrate.

## Verification

Focused tests, using the repository-pinned Rust 1.89 toolchain:

```bash
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH cargo test --locked --all-features --test web_fetch_integration
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH cargo test --locked --all-features --lib challenge_replay
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH cargo test --locked --all-features --lib meta::challenge
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH cargo test --locked --all-features --test static_guards
```

Broad gate:

```bash
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH make check
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH make docs-check
```

All new tests use loopback fixtures and never a public upstream. The live qualification against a
real protected origin is recorded as evidence and is never a substitute for the deterministic tests.

## Static guards and documentation to update

- The guard that keeps `src/fetch/**` free of solver imports must still pass.
- The guard that keeps the fetch-side pass-cookie cache single-owner and disk-free must still
  pass.
- The guard that keeps one shared challenge-solve orchestration must still pass.
- Documentation: `architecture/fetch.md`, `architecture/engines.md`, `docs/features.md`,
  `docs/test-inventory.md`, and `CHANGELOG.md`.

## Acceptance criteria

1. A fetch that meets a cached pair set the upstream refuses returns the document in the same
   call, proven by a loopback test that counts the solve attempts and the challenge-page hits.
2. The refused pair set is presented to the upstream at most once and is evicted, proven by the
   same test.
3. The call performs exactly one solve, proven by a loopback test that counts the verification
   endpoint hits.
4. A recovery that is refused, that errors, or that exhausts the deadline leaves the original
   response exactly as it was, proven by a loopback test.
5. A fetch with the opt-in disabled is unchanged: no solve, no replay, the marked page.
6. The origin key, the same-origin gate, the first-hop-only attachment, and the reuse window are
   unchanged, proven by the existing M033 tests still passing.
7. No cookie material appears in any log, error, warning, `Debug` output, fixture, or diagnostic
   payload.
8. The broad gate passes on the exact candidate, and every test that was run is recorded with its
   outcome, including anything deliberately not run.
9. The M033 live qualification against a real protected origin is recorded, with the observed
   results stated plainly.

## Stop conditions

Stop and leave M034 open if ADR-0017 or M031 or M033 is not closed; if the recovery cannot be
performed inside the call's existing budget; if the recovery would require a second solve or a
second replay in one call; if the recovery would present a pair set to an origin other than the
one that issued it; if the recovery would extend the reuse window; or if the implementation would
change M014's lifetime rules, the search-side cache, the refusal vocabulary, the tool surface, or
ADR-0009's prohibition list.

## Required closure evidence

`plans/closure/agentic-retrieval-integrity/034-status.md` records the implementation commits, a
requirement-to-evidence matrix for each acceptance criterion, the focused and broad command
outcomes including anything unrun, the unchanged-safety evidence, the secret-handling evidence, the
M033 live qualification results against a real protected origin, the documentation set that was
updated, known limitations, residual risk, and the recommendation. The closure commit carries this
plan, the closure record, the roadmap status, and the registry row together.
