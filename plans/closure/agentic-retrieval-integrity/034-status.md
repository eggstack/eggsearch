# M034 Fetch-Side Stale Pass-Cookie Recovery — Closure Evidence

Status: closed

Source plan: `plans/implementation/agentic-retrieval-integrity/034-fetch-side-stale-pass-cookie-recovery.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Architecture decision: `plans/adrs/ADR-0018-fetch-path-stale-pass-cookie-recovery.md` (accepted 2026-11-19)

## Scope

M034 corrected M033's known limitation #1. A stored pass pair set the upstream refused used to
end the call that found it: the entry was evicted, the marked challenge page was returned, and the
*next* fetch solved from cold. A caller therefore had to await a second fetch to get a document.

The refusal already carried everything the recovery needed, because the call still held the
challenge page it was about to solve. The reuse path now reports the refusal, and the same call
continues into the ordinary solve path with the challenge page it already holds, then the ordinary
replay, then the ordinary storage of the freshly issued pair set.

The rule was written into `architecture/fetch.md` as the "Stale pair set" paragraph before the
code, in the commit that registered the milestone, so it is reviewable independently of the
implementation. ADR-0018 records the scope decision it needed: ADR-0017's clause 5 forbids a call
from both replaying a stored pair set and then solving, so the in-call recovery needed a new
decision rather than an amendment to an accepted one.

## Exact code candidate

`5cf21e8065c77d5609e15a6ed1347033da14b13b` — the implementation commit, carrying the continuation in
`src/challenge_replay/mod.rs`, the three behavioral tests in `tests/web_fetch_integration.rs`,
and the documentation set. Every command recorded below ran against this exact candidate. The
closure commit carries this record, the plan status, the roadmap status, and the registry row
together.

The paper-first rule and the scope decision are in the preceding commit, `653af9f`.

## Requirement-to-evidence mapping

| # | Acceptance criterion | Evidence |
|---|---|---|
| 1 | A fetch that meets a cached pair set the upstream refuses returns the document in the same call | `tests/web_fetch_integration.rs::web_fetch_recovers_in_call_when_the_upstream_rejects_the_stored_pair_set`. The origin holds a stored pair set the upstream refuses; the refusal mock is hit **once**, and the same call returns the document with `manual_interaction_required: false` and no challenge warning. The rejected pair set is never offered again. |
| 2 | The refused pair set is presented to the upstream at most once and is evicted | The same test. The mock that matches only the refused pair set is hit **exactly one** time, and the next fetch replays the freshly issued pair set instead, proving the refused one was evicted and is not presented twice. |
| 3 | The call performs exactly one solve | The same test. The published challenge endpoint is hit **once** and the verification endpoint **once**, and the ledger for the `fetch` label is not needed to see it: a second solve would have hit both endpoints again. |
| 4 | A recovery that is refused, that errors, or that exhausts the deadline leaves the original response exactly as it was | `tests/web_fetch_integration.rs::web_fetch_keeps_the_original_response_when_the_recovery_fails`. The upstream refuses the stored pair set and then refuses the recovery's own replay as well. The call returns the original marked response with its challenge warning unchanged, the origin is left with **no** entry, and the next fetch starts cold and solves again. |
| 5 | A fetch with the opt-in disabled is unchanged: no solve, no replay, the marked page | `tests/web_fetch_integration.rs::web_fetch_does_not_reuse_a_stored_pair_set_when_solving_is_disabled`. The origin holds a stored pair set and solving is off: the pair set is **never presented** (its mock is hit zero times), no challenge endpoint and no verification endpoint is hit, the wall is met once, and the stored pair set is left untouched because the cache was never consulted. |
| 6 | The origin key, the same-origin gate, the first-hop-only attachment, and the reuse window are unchanged | The M033 suite is unchanged and still passes: `::web_fetch_reuses_a_pass_cookie_for_a_second_challenge_to_the_same_origin`, `::web_fetch_reuses_an_anubis_pass_cookie_for_the_same_origin`, `::web_fetch_never_replays_a_pass_cookie_to_another_origin`, `::web_fetch_solves_again_after_the_published_lifetime_expires`, `::web_fetch_solves_again_when_the_published_lifetime_is_malformed`, and `::web_fetch_solves_again_when_the_reuse_ceiling_is_zero`. The origin key itself is unchanged in `src/challenge_replay/cache.rs`. |
| 7 | No cookie material appears in any log, error, warning, `Debug` output, fixture, or diagnostic payload | The recovery test serializes the payload and asserts it carries neither the fresh pair set, nor the refused pair set, nor the cookie name. `src/challenge_replay/cache.rs::tests::debug_redacts_the_stored_value` still asserts that the cache's `Debug` rendering carries neither the stored value nor its cookie name. The new `CachedReplay` enum carries no cookie material in any variant. |
| 8 | The broad gate passes on the exact candidate, and every test that was run is recorded with its outcome | See Verification below. |
| 9 | The M033 live qualification against a real protected origin is recorded, with the observed results stated plainly | See "Live qualification against a real protected origin" below. The reuse was observed live; the recovery itself was not observable live and is recorded as such. |

## Live qualification against a real protected origin

This closes M033's known limitation #7, which recorded that no live qualification against a real
protected origin had been run.

### The origin

`https://git.disroot.org/` — a real Forgejo instance behind **Anubis v1.25.0**. The published
challenge was captured directly from the upstream:

```json
{"rules":{"algorithm":"fast","difficulty":2},
 "challenge":{"issuedAt":"2026-11-19T17:18:49.189265678+01:00",
              "metadata":{"User-Agent":"Mozilla/5.0 (...) Chrome/126.0 Safari/537.36",
                          "X-Real-Ip":"96.236.59.113"},
              "id":"01a1f49a-82e5-73f8-bb21-a1dd00fc3b45","method":"fast",
              "policyRuleHash":"ac980f49c4d35fab","difficulty":2,"spent":false}}
```

Difficulty **2** is inside the shipped ceiling of 5, and the method is `fast`, which ADR-0017's
permitted class already admits. The challenge metadata binds the challenge to the client
`User-Agent` and to the client address.

### The configuration

The qualification used a local, uncommitted configuration in `/tmp`. It differs from the shipped
defaults in exactly two ways: `[search.challenge].enabled = true` (the existing opt-in), and
`[fetch].user_agent` set to a browser user agent, because the upstream's policy challenges a
browser-shaped request and serves the real document to anything else. **No shipped default was
changed.** The browser user agent is a property of the qualification origin, not of eggsearch;
ADR-0009's user-agent-spoofing prohibition is not engaged, because the operator explicitly
configured the user agent that eggsearch sends unchanged.

### The observation — pass-cookie reuse

One MCP server process, three `fetch` calls to the same origin, 30 seconds apart, each with
`cache_policy: "bypass"` and `render: "http_only"`. The server was the release binary built from
this exact candidate with `cargo build --locked --release --all-features`:

| Fetch | Wall met | Solve | Document returned | Wall time | `provider_status` `solves_attempted` |
|---|---|---|---|---|---|
| 1 | yes | **1 attempt, 1 success** | yes | 1143 ms | 1 |
| 2 | no | **none** | yes | 466 ms | **1** |
| 3 | no | **none** | yes | 464 ms | **1** |

Every fetch returned the real Forgejo document, "Disroot Forgejo: Brace yourself, merge conflicts
ahead.", with `manual_interaction_required: false`. The solve count stayed at **one** across all
three fetches: the second and third fetches replayed the stored pair set and performed no solve.
The reuse also shows in the latency: 1143 ms for the cold fetch against 466 ms and 464 ms for
the two reuse fetches.

### The control — the wall was still being served

The reuse observation only means something if the upstream was still challenging eggsearch at the
time. A control run of the same binary, the same configuration, and the same origin, immediately
after, with only `[search.challenge].enabled = false` changed:

| Fetch | Result | Wall time | `manual_interaction_required` | Warnings |
|---|---|---|---|---|
| 1 | Anubis wall, "Making sure you're not a bot!" | 415 ms | **true** | `fetch_challenge_page` |
| 2 | Anubis wall, "Making sure you're not a bot!" | 117 ms | **true** | `fetch_challenge_page` |

The upstream was still challenging eggsearch throughout the observation window. The documents in
the enabled run therefore came from eggsearch's stored pass-cookie replay, not from the upstream
relaxing. The CLI cross-check agrees: `eggsearch --config <qual> fetch https://git.disroot.org/`
returns the real document with solving enabled and the Anubis wall with solving disabled.

### What the live run did not demonstrate

The upstream never **refused** a stored pair set during the observation window. The pass cookie is
bound to the client user agent and client address and remained valid for the whole window, so the
M034 recovery could not be exercised against this origin. The recovery itself is therefore proven
by deterministic loopback evidence only; the live evidence covers M033's reuse.

The ledger reported `cookie_hits: 0` and `cookie_misses: 0` for the `fetch` label, confirming
M033's known limitation #5: the fetch-side cache is not recorded in the solve ledger, so the reuse
is observable through the solve count and the latency, not through a cache counter.

### An operational note from the qualification

With the shipped `[fetch].browser.policy` default of `auto`, the MCP `fetch` path escalates to
the browser for a challenge page **before** the solver runs, as ADR-0017 requires. Headless Chrome
received Anubis's "Oh noes!" error page from this origin, which is not a challenge page, so the
solver was never reached and no solve was recorded. The qualification therefore pinned
`render: "http_only"` to exercise the solver path. This is a property of the upstream's
behaviour toward Chrome, not of the solver.

## Verification

Using the pinned Rust 1.89 toolchain on candidate `5cf21e8065c77d5609e15a6ed1347033da14b13b`:

- `make check`: exit 0 — 85 suites, 6,422 tests, 0 failed, 28 ignored. This includes fmt, clippy
  with `-D warnings`, the no-default-feature check, the all-features test run, repository hygiene,
  the dependency policy, and the packaging contract checks. The packaging contract reported
  `planning-consistency: ok`, `workflow-pin-check: ok (9 workflow files)`, `eggpack qualifier
  contract: ok (25 producer jobs, 7 targets)`, and the egress qualification contract agreeing with
  `packaging/release-targets.txt`.
- `make docs-check`: exit 0 (`cargo doc --locked --all-features --no-deps`).
- `cargo test --locked --all-features --test web_fetch_integration`: 115 passed, 0 failed, 1 ignored.
- `cargo test --locked --all-features --test static_guards`: 67 passed, 0 failed.
- `cargo test --locked --all-features --lib challenge_replay`: 29 passed, 0 failed.
- `cargo test --locked --all-features --lib meta::challenge`: 118 passed, 0 failed.
- `cargo test --locked --all-features --test repo_workflow`: 99 passed, 0 failed.
- `cargo test --locked --all-features --bins`: 18 passed, 0 failed.
- `cargo build --locked --release --all-features`: exit 0. The release binary from this exact
  candidate is the one the live qualification above ran.
- `git diff --check`: clean.

Nothing was deliberately skipped and nothing was deliberately left unrun. The live qualification's
network traffic is the only traffic this milestone generated; no live test is part of the default
gate, and the live observation never substitutes for a deterministic test.

## Additional guard evidence

- `tests/static_guards.rs::fetch_pass_cookie_cache_is_process_local_and_has_one_owner` still passes
  and is unchanged: exactly one file defines the fetch-side cache, no file outside
  `src/challenge_replay/` touches it, and the cache module still references neither the
  filesystem nor a serialization path, so a pass cookie can never be persisted.
- `tests/static_guards.rs::fetch_layer_never_imports_the_challenge_solver` still passes: the fetch
  layer still neither imports the solver nor the shared module, so the recovery added no
  dependency from `src/fetch/**` to `src/meta/**`.
- `tests/static_guards.rs::challenge_solve_orchestration_has_one_owner` still passes: the MCP and
  CLI layers still never call the solver directly. The continuation lives in the one shared module.
- `tests/static_guards.rs::altcha_fetch_solve_has_one_owner` still passes: the ALTCHA solve still
  has exactly one owner.
- The recovery reuses the existing solve and replay paths. It added no module, no configuration
  key, and no cache surface, so the single-owner guards over those seams are unaffected.

## Compatibility and migration effects

- No MCP tool, argument, or advertised-schema change. The canonical advertised inventory is
  unchanged and the byte-pinned advertisement still matches.
- No configuration key is added, removed, or renamed, and no shipped default changes.
  `[search.challenge].enabled` remains the single opt-in and remains off by default, and
  `cookie_ttl_seconds` keeps its default of 240 seconds.
- `Cargo.toml` is unchanged, so no dependency moved.
- The refusal vocabulary in `src/meta/challenge/ledger.rs` keeps its ten keys. No new bound name
  was introduced, and the `provider_status` `challenge_solver` block is unchanged.
- `src/meta/challenge/cookie.rs`, M014's lifetime rules, and the search-side `PassCookieCache`
  are unchanged. The fetch-side cache and its origin key are unchanged.
- The search side is unchanged. `src/meta/engines/startpage.rs` still removes the provider entry
  and reports `blocked` with `pass cookie rejected` on a rejected replay; that behavior is
  explicitly out of scope for this milestone and is not touched.

## Security evidence

- The permitted class did not widen. ADR-0009's prohibition list is unchanged and unexercised: no
  interactive CAPTCHA, OCR, solving service, credential bypass, proxy rotation, user-agent
  spoofing, or retry widening was added. No scheme was added to the fetch path.
- The recovery is not retry widening. It adds exactly one upstream request to a call that would
  otherwise have paid a full solve — the probe that discovered the staleness. It does not retry a
  failed request, does not widen any attempt budget, and does not conceal an outage.
- A refused pair set is evicted before the solve, so it is never offered to the upstream again, and
  the pair set a fresh solve stores is the only entry the origin holds.
- The solve stays inside `max_difficulty`, `max_attempts`, `timeout_ms`, and the remaining call
  deadline. The refused pair set costs no solve attempt of its own, because no solve produced it.
- A transport error, a timeout, and a non-2xx status are not refusals. The stored pair set is kept,
  no solve is attempted, and the call leaves the original response exactly as it was, so a
  transient failure never spends the solve budget and a usable pair set is never discarded because
  of one.
- The reuse window is unchanged and is never extended by the recovery: the fresh pair set is
  stored under the same published-lifetime and ceiling rules as any other.
- The origin key, the same-origin gate, and the first-hop-only attachment of the replayed cookie
  are unchanged, so a pair set is still never replayed to an origin other than the one that issued
  it and a redirect still cannot forward it.
- The cache stays process-local, bounded, and never written to disk or serialized into a response,
  a diagnostic, or a fixture.
- No cookie value, nonce, salt, challenge id, or pass token reaches a log, an error, a warning, a
  `Debug` output, a fixture, or a `provider_status` block.

## Observable behaviour changes

Two, both intended:

1. A fetch that meets a stored pair set the upstream refuses now returns the document instead of
   the marked challenge page. Before this change the same call returned the marked page and the
   next fetch solved; after it the same call solves and returns the document. The number of
   upstream requests in that call rises by one, because the refused probe is still made.
2. A refused pair set is still evicted, exactly as before. The eviction is unchanged; only what
   the call does after it changed.

## Known limitations and residual risk

1. The in-call recovery could not be exercised against a real protected origin. The upstream never
   refused a stored pair set during the live observation window, because its pass cookie is bound
   to the client user agent and client address and remained valid throughout. The recovery is
   proven by deterministic loopback evidence only. See "Live qualification against a real
   protected origin" above.
2. A transport error, a timeout, and a non-2xx status are not treated as refusals. An upstream
   that refuses a stale pair set with a bare 403 instead of a challenge page therefore keeps the
   entry, and each such fetch pays the refused probe. This is deliberate: an error does not prove
   that the pair set is stale, and evicting on an error would discard a usable pair set against a
   flapping upstream.
3. The refusal is detected from the challenge page the replay produced. A refusal that is not a
   challenge page is invisible to the module, exactly as before this milestone.
4. The reuse window is not refreshed on a hit. The entry expires at the lifetime published at
   solve time even when the upstream issues a newer cookie during the replay.
5. The cache is per-process. A restart loses every entry, and a multi-process deployment does not
   share one.
6. The fetch-side reuse is still not recorded in the solve ledger, so `provider_status` reports
   no fetch-side cookie hit or miss counter. The live qualification had to observe the reuse
   through the solve count and the latency instead.
7. The live qualification observed the reuse only through the solve count and the latency. The
   measured saving is therefore a count of avoided solves at one real origin, not a measured
   production latency figure across a fleet.
8. The qualification required a browser user agent, because the origin's policy serves the real
   document to a non-browser-shaped request. eggsearch sends the operator's configured user agent
   unchanged and adds no evasion headers; the qualification configuration was local and is not a
   shipped default.
9. The search side still does not self-heal. A rejected pass cookie on the search path still
   reports `blocked` with `pass cookie rejected` and the next search re-solves.

## Recommendation

Accept M034 and close it. The milestone's criteria are met with deterministic loopback evidence:
a refused pair set is presented at most once, evicted before the solve, and recovered inside the
same call with exactly one solve and one replay of a solved pair set; a failed recovery leaves the
original response and the origin exactly as they were; the origin key, the same-origin gate, the
first-hop-only attachment, the reuse window, the tool surface, the configuration surface, the
refusal vocabulary, and M014's rules are unchanged; and ADR-0009's prohibition list is untouched.

The separate M033 qualification gap is now closed as well: the pass-cookie reuse was observed
against a real protected origin, with a control proving the upstream was still challenging
eggsearch at the time, and a measured latency difference between the cold fetch and the two reuse
fetches.
