# ADR-0018: Fetch-Path Stale Pass-Cookie Recovery

Status: accepted

Date: 2026-11-19

Accepted: 2026-11-19

Decision owners: project maintainers

Related decisions: ADR-0009, ADR-0010, ADR-0017

Affected subsystem roadmap: plans/subsystems/agentic-retrieval-integrity-roadmap.md

## Context and forces

ADR-0017 established one shared solve-and-replay contract for every fetch surface: at most one
solve and one replay per fetch call, and every other outcome leaves the original response exactly
as it was. Its clause 8 authorized reuse of a fetch-side pass cookie for the same origin under
M014's lifetime rules, and required eviction when the upstream rejects it. M033 implemented that
reuse and recorded the consequence it left behind as known limitation #1.

The measured state of the repository at baseline `2c1a7225ac22caa95fc21a78d9016f78ce3cb807`:

- `src/challenge_replay/mod.rs` consults the origin-keyed cache before it solves. A hit replays the
  request once with the stored pair set; a miss solves as usual.
- `replay_cached` detects the refusal from the response the replay already produced: when the
  replayed response is itself a challenge page, the entry is evicted and the function returns
  `None`.
- The caller therefore keeps the original marked challenge page. The entry is gone, so the *next*
  fetch solves from cold and succeeds.
- The information needed to recover is already in hand at the eviction site. The call still holds
  the challenge page it was about to solve, and the refusal tells it the stored pair set is stale.
  No extra request, no extra detection, and no second challenge page are needed.

The degradation is a cost to the caller, not only to the upstream: a caller that meets a stale
pair set cannot obtain the document in that call and must await another fetch. On the MCP surface
that means the model issues the same fetch twice and waits for the second answer.

ADR-0017's clause 5 is the tension. It states "at most one solve and one replay per fetch call" and
that "a replay that is itself a challenge leaves the original response exactly as it is". A literal
reading forbids a call from both replaying a stored pair set and then solving, because the replay
budget would be spent twice. The clause 8 reuse path is the one place a stored pair set is
presented without a solve, so the two clauses overlap on exactly this case.

The forces are: (a) remove a caller-visible degradation without weakening any bound that exists to
contain cost or risk; (b) keep ADR-0009's prohibition list untouched; (c) keep one implementation of
the sequence for every fetch surface; (d) keep the reuse window, the origin key, and the
same-origin gate exactly as M033 and M014 left them; (e) record the refined bound as a durable
contract rather than leaving it to an implementation's reading.

## Considered alternatives

1. Leave the degradation in place. Rejected: the caller pays a second fetch for a document it could
   have received in the first call, and the model has no way to know that the second fetch will
   behave differently.
2. Evict the refused entry and return the marked page, as today. Rejected: this is the limitation
   itself. It is correct but not sufficient; it leaves the caller waiting.
3. Retry the refused pair set, widening the attempt budget until it is accepted. Rejected:
   ADR-0009 prohibits retry widening, and it presents a pair set the upstream already refused more
   than once.
4. Evict and always solve from cold on the next call, without an in-call recovery. Rejected: same
   as alternative 2.
5. Never reuse a stored pair set, so the call always solves and never meets a refusal. Rejected:
   this undoes M033 and pays a full solve on every fetch to a protected origin.
6. Proactively expire every stored pair set after a fixed short interval. Rejected: the upstream's
   actual lifetime is unknown to the client, so a guessed interval either discards usable pair sets
   or leaves the same stale-pair-set window open.
7. Treat an error or a non-2xx status as a refusal and evict on it. Rejected: a transient failure
   does not prove that the pair set is stale. Evicting on an error would discard a usable pair set
   and make the cache thrash against a flapping upstream.
8. Perform the in-call recovery and spend the call's single solve and single replay on the freshly
   issued pair set, presenting the refused pair set once as a reuse probe. Selected: it removes the
   degradation, keeps every cost bound, and keeps one implementation of the sequence.

## Decision

1. **ADR-0009's prohibition list is unchanged.** Interactive CAPTCHA and visual puzzle answering,
   OCR of challenge images, third-party or human solving services, credential or login bypass,
   session or cookie theft, proxy rotation, user-agent spoofing, and retry widening to conceal an
   outage remain prohibited. Nothing in this ADR reopens them, and no milestone may weaken them.
2. **The fetch path may recover in call from a refused stored pair set.** When a replay with a
   stored pair set comes back as a challenge page, the entry is evicted and the same call
   continues into the ordinary solve path with the challenge page it already holds, then the
   ordinary replay, then the ordinary storage of the freshly issued pair set. The caller receives
   the document in that call.
3. **The bound is refined, not weakened.** The surviving bound is: at most one solve, and at most
   one replay of a solved pair set, per fetch call. The refused pair set may be presented once as a
   reuse probe. The probe is not a solve and not a replay of a solved pair set, because no solve
   produced it; it is never presented a second time; and the call's single solve produces the pair
   set its single replay uses. ADR-0017's clause 5 sentence "a replay that is itself a challenge
   leaves the original response exactly as it is" continues to hold for the recovery's own replay.
4. **The refusal signal is unchanged.** A refusal is a replayed response that is itself a challenge
   page. A transport error, a timeout, and a non-2xx status are not refusals: the stored pair set
   is kept, the recovery is not attempted, and the call leaves the original response exactly as it
   was. A transient failure therefore never spends the call's solve budget, and a usable pair set is
   never discarded because of one.
5. **The eviction is unconditional and happens before the solve.** A refused pair set is never
   offered to the upstream again, and the pair set a fresh solve stores is the only entry the origin
   holds.
6. **The recovery runs inside the call's existing budget.** The solve stays within
   `max_difficulty`, `max_attempts`, `timeout_ms`, and the remaining call deadline. A solve
   that refuses, errors, exhausts the deadline, or replays into another challenge page leaves the
   original response exactly as it was, and the origin is left with no entry.
7. **The reuse window, the origin key, and the same-origin gate are unchanged.** M014's published
   lifetime rules, the origin key derived from the request URL, the rule that a fetch which crossed
   an origin stores nothing and reuses nothing, and the first-hop-only attachment of the replayed
   cookie are exactly as M033 left them. The recovery never extends the reuse window: the fresh
   pair set is stored under the same published-lifetime and ceiling rules as any other.
8. **The recovery is not retry widening.** It adds exactly one upstream request to a call that
   would otherwise have paid a full solve: the probe that discovered the staleness. It does not
   retry a failed request, does not widen any attempt budget, and does not conceal an outage.
9. **The secret-handling rules are unchanged.** No cookie, nonce, salt, challenge id, or pass token
   appears in a log, an error, a warning, a `Debug` output, a fixture, or a `provider_status`
   block. The cache stays process-local, is never written to disk, and is never serialized into a
   response, a diagnostic, or a fixture.
10. **The search side is unchanged.** The search-side `PassCookieCache` and its rejection behavior
    are untouched. This ADR governs the fetch path only.
11. **The surface and configuration surface are unchanged.** No MCP tool, argument, or advertised
    schema changes; no configuration key is added, removed, or renamed; no shipped default changes;
    no dependency moves. Solving stays opt-in under the single existing key
    `[search.challenge].enabled`, and with the opt-in off the cache is never consulted and no
    solve is attempted.

## Consequences and tradeoffs

Advantages: a fetch that meets a stale pair set returns the document in the same call, so no
caller has to await a second fetch; the refusal that already costs one degraded fetch becomes the
signal that starts the ordinary solve path instead of its endpoint; every cost bound still binds,
because the refused pair set costs no solve attempt of its own.

Tradeoffs: a call that meets a stale pair set makes one more upstream request than a call that
meets a cold cache, because the probe is still made; the bound in ADR-0017's clause 5 needs this
ADR's refinement to be read correctly, so the two documents must be read together; and the recovery
adds one more way for a fetch to reach the solver, which is why the opt-in, the ceilings, and the
deadline continue to gate it.

## Compatibility, security, and migration

No MCP tool, argument, or advertised-schema change, and the byte-pinned advertisement still matches.
No configuration key is added, removed, or renamed, and the shipped defaults are unchanged.
`Cargo.toml` is unchanged, so no dependency moved. The refusal vocabulary keeps its ten keys and
the `provider_status` `challenge_solver` block is unchanged.

The observable behavior change is bounded and documented: a fetch that meets a refused stored pair
set now returns the document instead of the marked challenge page. Before this decision the same
call returned the marked page and the next fetch solved; after it the same call solves and returns
the document. The number of upstream requests in that call rises by one, because the refused probe
is still made.

Every solve stays behind the existing bounded-body, deadline, SSRF, and redirect rules, and every
request continues through the existing sanitization and trust-framing pipeline. Pass cookies remain
secret-like.

## Verification and status

Adoption authorizes the in-call recovery and the refined bound. It does not change behavior.
Implementation lands through M034, which corrects M033's known limitation #1. The milestone closes
on its own exact candidate with its own closure record under
`plans/closure/agentic-retrieval-integrity/`, and the roadmap and registry rows update in the same
closure commit.
