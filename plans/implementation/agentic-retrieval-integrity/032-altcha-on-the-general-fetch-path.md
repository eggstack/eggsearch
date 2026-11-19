# M032 — ALTCHA on the General Fetch Path

Status: closed

Class: capability

Source roadmap: plans/subsystems/agentic-retrieval-integrity-roadmap.md (M032)

Requires: ADR-0017 acceptance (hard dependency); M031 closed (hard dependency — this milestone
reuses M031's shared orchestration module and its single-implementation guard).

Planning baseline: 556462bb0b8e7c15150f20548e02fe00a1e03923 (current committed code candidate; M030 closure)

References: ADR-0009; ADR-0010; ADR-0017 (proposed); `architecture/engines.md` § Challenge solving;
`architecture/fetch.md` § Challenge pages on the HTTP path; `src/fetch/challenge.rs`;
`src/meta/challenge/altcha.rs`; `src/meta/engines/mojeek.rs`.

## Objective

A general fetch that meets a published ALTCHA PBKDF2 challenge attempts a bounded solve and one
replay, so that a site behind an ALTCHA gate is reachable through the same fetch surfaces that
already solve Anubis.

## Current implementation evidence

- `src/fetch/challenge.rs` classifies an ALTCHA wall from the markers `captcha-wrap`, "javascript
  is required to complete this challenge", and `captcha/challenge`. The classification marks the
  page for every caller; no caller solves it.
- `src/meta/challenge/altcha.rs` parses a published challenge document — a JSON payload with
  `parameters.algorithm`, `cost`, `keyLength`, `nonce`, `salt`, and `keyPrefix` — and solves
  it by searching a PBKDF2 counter until the derived key starts with the published prefix. It
  supports `PBKDF2/SHA-256` only, and it bounds the counter search with the shared attempt
  ceiling.
- `src/meta/challenge/altcha.rs::solve_and_verify` takes a challenge URL and a verify URL. Its
  only caller, `src/meta/engines/mojeek.rs`, supplies Mojeek's two fixed same-origin paths. The
  solver therefore has no reviewed way to reach a challenge that a general page publishes in its
  own markup.
- The ALTCHA scheme reports the replayable pair under the published `chllg` name, and a
  `Max-Age=0` verification still performs one replay and stores nothing.
- The ALTCHA ceiling today is `max_attempts` plus `timeout_ms`. `max_difficulty` is carried into
  the budget but never consulted for ALTCHA, and no operator documentation says so.

## Scope

In scope:

1. A bounded, same-origin rule for deriving the ALTCHA challenge and verify endpoints a general
   page publishes.
2. A bounded parse rule that extracts the published challenge payload from a general page.
3. The solve and one replay, wired through M031's shared orchestration module so that the ALTCHA
   path has exactly one implementation.
4. The operator documentation that states which knobs bound an ALTCHA solve.

Out of scope: any Anubis method change; the interactive and verification families; `argon2id` and
`hashx`; an ALTCHA extension class; a new configuration key; a change to the shipped defaults; a
new dependency (`pbkdf2` and `base64` are already direct dependencies); any change to ADR-0009's
prohibition list; a change to the Anubis solve path.

## Invariants that cannot regress

- No file under `src/fetch/**` imports the solver or the shared orchestration module.
- The endpoint derivation accepts only same-origin targets. A cross-origin, scheme-changing, or
  unparseable endpoint fails closed as `unsupported scheme` before any request.
- The endpoint path and shape are derived from a fixed allowed form, never copied verbatim from
  arbitrary page text into a request URL.
- The published challenge payload is length-bounded before it is parsed, and the parse rejects
  anything outside the documented parameter set as `malformed parameters`.
- The solve stays inside `max_attempts` and `timeout_ms`, and inside the call deadline. At most
  one solve and one replay per fetch call.
- A refused solve, a malformed payload, a cross-origin endpoint, an exhausted deadline, or a replay
  that is itself a challenge leaves the original response exactly as it is.
- The replayed cookie is attached on the first hop only.
- No cookie, salt, nonce, derived key, or challenge payload appears in a log, an error, a warning, a
  `Debug` output, or a fixture.
- An unsupported ALTCHA algorithm fails closed as `unsupported scheme`.
- The refusal vocabulary stays fixed at its ten keys and gains no new member.

## Expected production changes

1. **Endpoint derivation.** One bounded rule that derives the ALTCHA challenge and verify endpoints
   from what the page publishes, restricted to same-origin HTTP(S) targets on the challenge page's
   own origin, with a fixed path shape and a bounded length. Anything else fails closed.
2. **Payload extraction.** One bounded rule that extracts the published challenge payload from the
   challenge page, with a documented accepted shape and an explicit `malformed parameters` refusal
   for anything else. The rule is fixture-tested against captured pages, never against a live site.
3. **Solve and replay through the shared module.** The shared orchestration module gains the ALTCHA
   branch alongside the Anubis branch, keeping the same one-solve-one-replay contract and the same
   unchanged-response-on-refusal behavior. No second orchestration copy is created.
4. **A documented ceiling statement.** The operator documentation states that an ALTCHA solve is
   bounded by `max_attempts` (counter iterations) and `timeout_ms`, that `max_difficulty` does
   not gate ALTCHA, and that the published PBKDF2 `cost` multiplies the work each iteration
   performs.

## Migration and compatibility effects

No MCP tool, argument, or advertised-schema change. No configuration key is added, removed, or
renamed, and no shipped default changes. A page that was previously marked `fetch_challenge_page`
and returned as a stub may now be solved and replayed when the operator opted in and the challenge
fits the ceilings; with the opt-in off, the marked response is unchanged. No storage or protocol
change.

## Ordered work packages

1. **Establish the endpoint-derivation rule and its refusals** on paper first: the accepted shapes,
   the same-origin check, and the exact `unsupported scheme` sentence for each rejection. Write it
   into `architecture/fetch.md` before implementing so the rule is reviewable independently of the
   code.
2. **Implement payload extraction** with captured fixtures for the accepted shape and for each
   malformed variant.
3. **Implement endpoint derivation** with the same-origin and shape rejections.
4. **Wire the ALTCHA branch into the shared orchestration module** used by M031, reusing its
   attempt, deadline, replay, and unchanged-response behavior.
5. **Update the documentation set:** `architecture/fetch.md`, `architecture/engines.md` § Challenge
   solving, `docs/features.md`, and the operator configuration documentation for the ceiling
   statement.
6. **Run the verification below** and record the outcomes.

## Verification

Focused tests, using the repository-pinned Rust 1.89 toolchain:

```bash
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH cargo test --locked --all-features --lib meta::challenge
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH cargo test --locked --all-features --test web_fetch_integration
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH cargo test --locked --all-features --test static_guards
```

Broad gate:

```bash
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH make check
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH make docs-check
```

All new tests use loopback fixtures, never a public upstream, and need no network, credentials, or
browser. Live qualification against a real ALTCHA deployment is optional evidence and is recorded
as such when it is not run; it is never a substitute for the deterministic suite.

## Static guards and documentation to update

- The guard from M031 that keeps `src/fetch/**` free of solver imports must still pass.
- Add a guard that the ALTCHA branch exists only in the shared orchestration module and that no
  second ALTCHA solve path appears.
- Add a guard that an endpoint derivation cannot produce a cross-origin target.
- Documentation: `architecture/fetch.md`, `architecture/engines.md`, `docs/features.md`, and
  the operator configuration documentation.

## Acceptance criteria

1. A general fetch against a loopback ALTCHA wall, with the opt-in enabled, returns the replayed
   document with `manual_interaction_required: false` and no `fetch_challenge_page` warning.
2. A cross-origin published endpoint fails closed as `unsupported scheme` with zero verification
   requests.
3. A malformed published payload fails closed as `malformed parameters` before any solve.
4. An unsupported published algorithm fails closed as `unsupported scheme`.
5. An ALTCHA solve that exceeds `max_attempts` or `timeout_ms` is refused with the operative
   ceiling named, and the original response is returned unchanged.
6. At most one solve and one replay occur per fetch call on every surface.
7. The operator documentation states that `max_attempts` and `timeout_ms` bound an ALTCHA solve
   and that `max_difficulty` does not gate it.
8. The broad gate passes on the exact candidate, and every test that was run is recorded with its
   outcome, including anything deliberately not run.

## Stop conditions

Stop and leave M032 open if ADR-0017 or M031 is not closed; if safe same-origin endpoint derivation
turns out to require executing page script; if the published payload cannot be extracted with a
bounded, fixture-testable rule; if the work would need a new dependency; if the refusal vocabulary
would need to grow; or if implementing the ALTCHA branch would produce a second orchestration copy.

## Required closure evidence

`plans/closure/agentic-retrieval-integrity/032-status.md` records the implementation commits, a
requirement-to-evidence matrix for each acceptance criterion, the focused and broad command
outcomes including anything unrun, the unchanged-safety evidence, the documentation set that was
updated, known limitations, residual risk, and the recommendation. The closure commit carries this
plan, the closure record, the roadmap status, and the registry row together.
