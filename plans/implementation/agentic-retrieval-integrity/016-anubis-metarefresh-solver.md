# M016 — Anubis `metarefresh` Solver

Status: closed
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M016
Primary class: capability
Baseline: `dace41c3a75307ea35b745d695a11630ec6495c0` (the worktree also carries the uncommitted ADR-0010 file and its edits to ADR-0009 and the roadmap; re-read `git status` before starting)
References: ADR-0010 (the authority for this scope extension); ADR-0009 (superseded in part); `architecture/engines.md` § Challenge solving; `src/meta/challenge/anubis.rs`; `src/meta/challenge/mod.rs`; `src/meta/engines/startpage.rs`; `src/core/config/challenge.rs`; `docs/features.md`.
Dependencies: none hard. Interface to M010/M011 (the `SolveBudget`/`SolveOutcome` framework and the `anubis.rs` dispatch as they exist at the baseline) and to M013, which owns the `slow` alias. M014 and M015 touch the same boundary, so rebase against whichever lands first rather than replicating their work here.

## Objective

Add a third solved method to the existing Anubis solver: the no-JavaScript `metarefresh` path, dispatched on `challenge.method == "metarefresh"` alongside `fast` and `preact` in `src/meta/challenge/anubis.rs`, under ADR-0010's decision that `metarefresh` is permitted "as a time challenge, not as a proof-of-work".

Current implementation evidence. `AnubisChallenge::parse` (`src/meta/challenge/anubis.rs:86`) reads `script#anubis_challenge` and `script#preact_info`, then dispatches on the published `challenge.method` at `:105`–`:119`; `fast` and `preact` are the only accepted strings and everything else returns `BOUND_UNSUPPORTED_SCHEME`. `solve()` (`:176`) matches the method again, `solve_and_verify` (`:238`) computes one deadline at `:256` and honours the `preact` minimum elapsed time through `preact_wait` (`:335`), and `verify` (`:282`) matches the method a third time to choose the submission URL (`:293`). `SolveBudget` (`src/meta/challenge/mod.rs:26`) exposes `attempts_used`, `max_attempts`, `max_difficulty`, `remaining`, `allows_difficulty`, `check_difficulty`, and `attempt`; it exposes no deadline accessor. `src/meta/engines/startpage.rs:56` already drives detect → solve → replay → parse method-agnostically, and its blocked markers (`:26`–`:31`) are `#anubis_challenge`, `#anubis_version`, and the access-denied title.

The method performs no hashing at all. `fast` costs `16^difficulty` expected SHA-256 candidates; `metarefresh` costs one wait plus one request. Published reference behaviour places the wait at approximately `800 ms × difficulty`, against `preact`'s `80 ms × difficulty`, but that figure is a hypothesis about the served page, never a value this build computes.

## Non-goals

- No `slow` mapping and no prefix-tolerant script-id matching; M013 owns both. `slow` keeps returning `BOUND_UNSUPPORTED_SCHEME` until M013 lands, and the two changes must not be folded together.
- No change to `fast` or `preact` solving, waiting, submission, or classification beyond the shared deadline argument described below.
- No new configuration key, no change to the `[search.challenge]` table or its validation, no new dependency, and no `Cargo.toml` edit.
- No new `BOUND_*` name in `src/meta/challenge/mod.rs`; M015 will enumerate bounds, so reusing the existing names keeps its surface stable.
- No parallel or concurrent solving, no browser delegation, no user-agent change, no proxy rotation, no retry widening.
- No speculation about the submission shape. If the served page does not publish the value, the field, or the target this plan assumes, stop and return for a decision rather than inventing one.
- No change to `SolveOutcome`: if the observed flow does not yield a pass cookie, the provider stays `blocked` and the milestone stops.

## Invariants that cannot regress

- Disabled, above-ceiling, unsupported-method, and malformed-input paths all classify `EngineError::Blocked` — at the solver boundary `SolveOutcome::Refused { bound }`, at the engine boundary `reason: "challenge not solved: <bound>"` — never empty results.
- A `metarefresh` challenge whose submission succeeds but whose replayed page does not parse is `ParseFailed`, not `Blocked` and not empty.
- The difficulty ceiling is compared before any wait, before any request, and before any hashing, using `ceiling_refusal`'s exact operator-visible sentence; a missing or non-numeric `rules.difficulty` is malformed.
- The response deadline wins over the solve budget. A wait that does not fit the remaining budget is refused, never truncated and never partially slept.
- One solve and one replay per provider per request, and one solve per `cookie_ttl_seconds`; the new arm contains no loop, so it cannot become a retry loop.
- Every upstream request stays behind `read_bounded_body` / `read_bounded_response` and the existing body caps.
- Pass cookies and the echoed answer token are secrets-like: memory only, `Debug`-redacted, never in fixtures, logs, `provider_status`, diagnostics, or `EngineError` text.
- The echoed payload contains values the upstream published in that challenge response and nothing else — no request header, no cookie, no user agent, no operator configuration.

## Expected production changes

1. **Method and parse.** New `pub const METHOD_METAREFRESH: &str = "metarefresh"`, a `AnubisMethod::Metarefresh` variant, and a `from_metarefresh` constructor called from the existing `match method` in `parse`. It reads `rules.difficulty` with the existing `numeric_difficulty`, the published answer token with the existing `required_str` semantics, and the published `redir` with the existing `optional_str`. It must not require `challenge.randomData`, which belongs to `fast`.
2. **Elapsed requirement discovery.** The wait is read from the served document, never derived from difficulty. Primary source: the `<meta http-equiv="refresh" content="…">` element in the document that carries the challenge, whose numeric leading field is the requirement in seconds per the HTML meaning of the attribute. If the published JSON payload also carries the wait as a numeric field, that value is read and must agree; disagreement is `BOUND_MALFORMED`. Fail closed with `BOUND_MALFORMED` when: no meta refresh exists, the content is not a bare finite non-negative decimal optionally followed by `; url=<target>`, the value is non-finite or negative, the JSON wait is present but non-numeric, or the two sources disagree. Convert with `Duration::try_from_secs_f64` and treat a conversion error as `BOUND_MALFORMED`; never call the panicking `from_secs_f64`. The `0.8 × difficulty` hypothesis is used only as the fixture's expected value and as the closure record's comparison; a live value that differs is recorded, not "corrected".
3. **Wait.** `solve_and_verify` extends its existing `preact`-only wait block into a match over `Preact | Metarefresh`. `metarefresh_wait(requirement, deadline)` returns `Err(BOUND_WALL_CLOCK)` when the deadline has passed or the requirement exceeds the remaining time, and otherwise the exact required duration; the caller sleeps that duration once. A requirement longer than the budget refuses with the named bound `wall clock exhausted` rather than sleeping past the deadline.
4. **Solve and submission.** `solve()` gains the `Metarefresh` arm, which returns the published answer token verbatim as `AnubisSolution::response` with `nonce: 0`, performs no hashing, and consumes no attempt from the budget. `verify` gains the matching arm. The submission target is the URL the challenge published, resolved against `base_url`; a relative target is joined to `base_url`, and a target whose scheme and authority are not those of `base_url` is refused as `BOUND_MALFORMED` rather than followed. A published target that cannot be resolved, or a challenge that publishes none, is `BOUND_MALFORMED` — the arm never synthesises a path. Query parameters come from the published target; any field the client must supply (an `elapsedTime`-style timing value) is carried as a fixed constant derived deterministically from the published requirement, never from measured wall time.
5. **Remaining-budget cap.** `verify` takes the `Instant` deadline that `solve_and_verify` already computes instead of a second full `timeout`, and sets the transport timeout from `deadline.saturating_duration_since(Instant::now())`. If that remaining duration is zero, it returns `Refused { BOUND_WALL_CLOCK }` without sending. This is one private-argument change inside `anubis.rs`; it makes the existing M011 invariant ("the response deadline wins over the solve budget") true for a method that spends seconds before submitting, and it leaves the existing `fast` and `preact` tests green because their waits sit far inside their timeouts. A residual transport timeout still maps through `map_request_error` to `EngineError::Timeout`; do not reclassify it, which would change existing behaviour.
6. **Cookie capture.** Unchanged: capture `spchal-auth` from the submission response, `read_bounded_body` the response, and classify a missing cookie as `BOUND_MISSING_COOKIE` on success or `BOUND_COOKIE_REJECTED` otherwise.
7. **Constraints that must hold.** Single-threaded, no spawn, no new dependency, no configuration change. No code comments. `anubis.rs` is 947 lines / 33,151 bytes at the baseline against the ordinary-file ceiling of 1,600 lines / 80 KB, so the addition fits without a ratchet entry; keep the file under the ceiling rather than adding new modules. Every internal `match` on `AnubisMethod` gains its arm; `clippy -D warnings` fails on a non-exhaustive match.
8. **Detection needs no production change.** `startpage.rs` already treats `#anubis_challenge` as a blocked marker and already routes any solved challenge through the same solve → replay → `finalize_scrape` path. `startpage.rs` gains tests only. If the captured `metarefresh` page does not carry that marker, stop and return for a decision; adding a speculative marker is out of scope.

**The published-values-only echo rule.** This method is a reflection channel by design, so the payload is defined by source rather than by field name. (a) The echo is built by a builder that takes `&AnubisChallenge` and nothing else: no `HeaderMap`, no cookie jar, no pass-cookie cache entry, no `ChallengeSection`, and no `request_url` reaches it. (b) The only fields it may emit are the parsed published values — the answer token, the published `id` when the target does not already carry it, the published `redir`, and the published timing field. (c) The answer token is emitted verbatim: no hashing, trimming, case folding, or normalisation, only percent-encoding required for query transport. (d) The only `Cookie` header permitted is `challenge_cookies`, the set the upstream itself published in that same challenge response, exactly as `preact` does today; no cookie from the search request, the cache, or a client jar may be sent. (e) Nothing from the request — user agent, `Accept*`, `Referer`, locale, operator config — is echoed, because every one of those is a value the server did not publish for this challenge. (f) The answer token and cookies never appear in `Debug`, `Display`, error text, or logs.

## Storage, protocol, and compatibility effects

No MCP tool, response-schema, wire-string, or `provider_status` change. `[search.challenge]` keeps its five keys and meanings, so an ADR-0009 config keeps its meaning and no migration exists. The only additive surface is one public enum variant and one public `METHOD_*` constant inside a module tree that carries no semver library guarantee, plus one private function signature inside `anubis.rs`. `metarefresh` becomes solvable only when `enabled = true` and the published difficulty is at or below `max_difficulty`; with the shipped `timeout_ms = 2000`, a wait of `0.8 × difficulty` seconds fits only up to difficulty 2, so difficulties 3 and above refuse with `wall clock exhausted` until the operator raises `timeout_ms` — a documentation consequence, not a new knob. No new network destination: the submission target must be same-origin with the upstream that issued the challenge.

## Ordered work packages

1. Capture the served `metarefresh` page shape, or a synthetic reconstruction of it, into the fixture used by the tests: the meta refresh content and unit, the answer-token field name, the published target shape, and whether a pass cookie is set. Record the observed values for the closure record; stop if the answer token, the wait unit, or the cookie outcome cannot be established.
2. Add the method constant, enum variant, and `from_metarefresh` parsing with fail-closed branches for every malformed shape above.
3. Add `metarefresh_wait` and wire the wait into `solve_and_verify`'s existing deadline block; refuse instead of sleeping when the requirement does not fit.
4. Add the `verify` arm and the remaining-budget cap; keep `fast` and `preact` behaviour identical apart from that cap.
5. Add the published-values-only echo builder with the signature constraint, and route the token through it.
6. Solver-level tests: dispatch, unchanged `fast`/`preact` dispatch, wait-beyond-budget refusal, above-ceiling refusal before any wait, malformed and truncated pages, echo contents, redaction, attempt-meter non-consumption.
7. Engine-level tests in `startpage.rs`: a `metarefresh` challenge solves and replays exactly once, a disabled section stays `blocked` with one request, and a junk replay is `ParseFailed`.
8. Docs: `architecture/engines.md` § Challenge solving gains the `metarefresh` row and the ADR-0010 method-coverage sentence (replacing the current "fail closed on any other method" wording and the ADR-0009-only paragraph); `docs/features.md:96` stops saying two schemes; `docs/test-inventory.md` count rows move with the added tests; `CHANGELOG.md` gets one append-only entry.

## Verification

```bash
cargo fmt --all
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features --lib meta::challenge
cargo test --locked --all-features --test engine_parse_conformance
cargo test --locked --all-features --test static_guards
cargo test --locked --all-features
```

`make check` is the canonical gate and covers the same set plus hygiene, dependency-policy, and packaging checks; the block above is this milestone's required evidence.

Live, opt-in, recorded rather than asserted:

```bash
eggsearch --config /tmp/metarefresh.toml search "rust programming language" --providers startpage --json
```

with `[search.challenge] enabled = true` and `timeout_ms` large enough for the published wait. Record the observed method, the published wait, the wall time, and the provider status in `tests/fixtures/engines/README.md`.

A live `metarefresh` deployment may not be reachable from the implementation environment, and a supported deployment may not publish the method at all during the milestone. If the synthetic fixture is the only evidence, the closure record must say so explicitly and describe the method as unproven in the field; it must not present fixture-only evidence as live verification, and it must not claim the method works against a deployment that was never observed.

## Acceptance and stop conditions

- The synthetic `metarefresh` page parses, reports `AnubisMethod::Metarefresh`, and dispatches to the new arm; the `fast` and `preact` vectors and dispatch tests are unchanged.
- The submitted payload contains exactly the published values. A test seeds sentinel values into the user agent, request headers, cookies, and `ChallengeSection`, and asserts every sentinel is absent from the submitted URL, the submitted headers, the challenge `Debug` output, and the error text.
- A requirement longer than `timeout_ms` yields `Refused { BOUND_WALL_CLOCK }` and `EngineError::Blocked` without sleeping past the deadline and without sending a request.
- A difficulty above `max_difficulty` yields `EngineError::Blocked` carrying the exact `ceiling_refusal` sentence, with zero submissions and no wait.
- A malformed, negative, non-finite, missing, or conflicting wait, a missing answer token, an unresolvable or cross-origin target, and a truncated document all yield `Refused { BOUND_MALFORMED }` and no request.
- One submission and one replay per request; `attempts_used()` stays `0` for this method, proving `max_attempts` does not bound it and that the attempt ceiling is replaced by the wall clock, the difficulty ceiling, and the structural single attempt.
- `enabled = false` keeps the current behaviour byte for byte: one request, `blocked`, no solver call.
- No new configuration key, no new dependency, no `Cargo.toml` edit, no new `BOUND_*` name, no parallel work, no code comments, and `anubis.rs` stays inside 1,600 lines / 80 KB.
- Stop and return for a decision if the served page publishes the wait in an ambiguous unit, publishes no answer token, publishes a cross-origin target, sets no pass cookie after the publish step, or serves the method without the `#anubis_challenge` marker.

## Closure evidence

`plans/closure/agentic-retrieval-integrity/016-status.md` must record: the implementation candidate SHA; the exact verification commands with their results; the captured or reconstructed `metarefresh` page shape with the published wait and its unit, compared against the `0.8 × difficulty` hypothesis; the observed submission target and answer-token field; the sentinel-absence evidence for the echo rule; the above-ceiling, wait-beyond-budget, and malformed refusals with their exact bound names; the `attempts_used() == 0` evidence; the unchanged disabled-path result; whether any live deployment was reachable, and if not, the explicit statement that the method is unproven in the field; and residual risk (unit or field drift in the served page, the fact that shipped `timeout_ms` cannot honour difficulties of 3 and above, the absent `slow` mapping owned by M013, and the unchanged prohibition list from ADR-0009).
