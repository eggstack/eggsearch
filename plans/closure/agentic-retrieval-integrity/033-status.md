# M033 Fetch-Side Pass-Cookie Reuse — Closure Evidence

Status: closed

Source plan: `plans/archive/implementation/agentic-retrieval-integrity/033-fetch-side-pass-cookie-reuse.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Architecture decision: `plans/adrs/ADR-0017-fetch-path-challenge-resolution.md` (accepted 2026-11-19)

## Scope

M033 added pass-cookie reuse to the fetch path. A pass cookie a fetch issued is now reused for a
later fetch to the same origin instead of being solved again every time, and that reuse is bounded
by the upstream-published lifetime and the operator's `cookie_ttl_seconds` ceiling exactly as the
search-side reuse is.

The rule was written into `architecture/fetch.md` as a new section, "Fetch-side pass-cookie
reuse", before the code, so it is reviewable independently of the implementation.

## Exact code candidate

`5a98682bda4a68ec55dc9e28409fd8269d1f284d` — the implementation commit, carrying the new
`src/challenge_replay/cache.rs`, the reuse wiring in the shared orchestration module, the
`doctor` warning extension, the new static guard, the new tests, and the documentation set. Every
command recorded below ran against this exact candidate. The closure commit carries this record,
the plan status, the roadmap status, and the registry row together.

## Requirement-to-evidence mapping

| # | Acceptance criterion | Evidence |
|---|---|---|
| 1 | A second challenged fetch to the same origin replays the cached pair set and performs no solve, proven by a loopback test that counts solve attempts | `tests/web_fetch_integration.rs::web_fetch_reuses_a_pass_cookie_for_a_second_challenge_to_the_same_origin`. Two fetches to the same URL: the verification endpoint is hit **once**, the published challenge endpoint once, the wall twice, and the cookie-bearing replay twice. Both fetches return the replayed document with `manual_interaction_required: false` and no challenge warning. The Anubis family is proven by the sibling test `::web_fetch_reuses_an_anubis_pass_cookie_for_the_same_origin`, where the pass-challenge endpoint is hit once across two fetches. Suite: 113 passed, 0 failed, 1 ignored. |
| 2 | A fetch after the published lifetime expires solves again rather than replaying an expired cookie | `tests/web_fetch_integration.rs::web_fetch_solves_again_after_the_published_lifetime_expires`. The pass response publishes `Max-Age=1`; after the entry's one-second window elapses, the next fetch solves again — the verification endpoint is hit **twice** and the challenge endpoint twice, proving no expired pair set was replayed. The expiry mechanism itself is proven without a sleep at `src/challenge_replay/cache.rs::tests::an_expired_entry_is_dropped_on_read`. |
| 3 | A fetch with a malformed published lifetime solves again and stores nothing | `tests/web_fetch_integration.rs::web_fetch_solves_again_when_the_published_lifetime_is_malformed`. The pass response publishes `Max-Age=not-a-number`; the verification endpoint is hit **twice** across two fetches, so the malformed lifetime stored nothing and reused nothing. The bound is M014's unchanged rule, asserted by `src/meta/challenge/cookie.rs::tests::malformed_max_age_stores_nothing`, which resolves a malformed `Max-Age` to a zero reuse window. |
| 4 | A replay rejected by the upstream evicts the entry, and the next fetch solves again | `tests/web_fetch_integration.rs::web_fetch_evicts_a_pass_cookie_the_upstream_rejects`. The first fetch solves and stores the pair set. The upstream then refuses it: the cookie-bearing replay comes back as a challenge page, so the entry is evicted and the original marked response is returned unchanged. The next fetch starts cold and solves again — the verification endpoint is hit **twice** in total. The eviction primitive is proven directly by `src/challenge_replay/cache.rs::tests::removing_one_origin_leaves_the_rest`. |
| 5 | A cached pair set is never replayed to a different origin, and a test asserts the request carrying it targets the issuing origin | `tests/web_fetch_integration.rs::web_fetch_never_replays_a_pass_cookie_to_another_origin`. Two loopback origins, each with its own pass cookie. The second server carries a mock that matches the **first** origin's pair set; it is hit **zero** times, while the second origin's own replay mock and its verification endpoint are each hit once. The key itself is scheme, host, and effective port, proven by `src/challenge_replay/cache.rs::tests::the_origin_key_is_scheme_host_and_effective_port`, `::a_default_port_and_an_explicit_one_share_a_key`, and `::a_non_http_url_has_no_origin_key`. |
| 6 | The cache is bounded, and a saturation test proves the bound holds | `src/challenge_replay/cache.rs::tests::the_entry_cap_holds` fills a two-entry cache with three origins and asserts the count stays at the cap; `::the_byte_ceiling_holds` asserts the byte total stays under the ceiling; `::an_oversized_pair_set_is_not_stored` asserts a single oversized pair set is not stored at all. The bounds are explicit constants, `DEFAULT_CAPACITY = 32` and `DEFAULT_MAX_BYTES = 64 * 1024`. |
| 7 | The `doctor` warning fires exactly once when routed egress meets cookie reuse, and its text covers the fetch-side cache | `src/commands/doctor.rs::tests::egress_with_cookie_reuse_warns_exactly_once` asserts exactly one `[egress]` warning with routed egress, enabled challenge solving, and a non-zero reuse window; that the count drops to zero when `cookie_ttl_seconds = 0` or when solving is disabled; that the remediation text still reads `cookie_ttl_seconds = 0`; and that the warning text names the fetch path. The warning is unchanged in count — the fetch-side cache extends the existing warning rather than adding one. |
| 8 | No cookie material appears in any log, error, warning, `Debug` output, fixture, or diagnostic payload | `src/challenge_replay/cache.rs::tests::debug_redacts_the_stored_value` asserts the cache's `Debug` rendering carries neither the stored value nor its cookie name. `tests/web_fetch_integration.rs::web_fetch_reuses_a_pass_cookie_for_a_second_challenge_to_the_same_origin` asserts the reused fetch's serialized payload contains neither the pass cookie value nor the cookie name. The cache stores the value only in its in-memory entry and never writes it to a response, a log, a diagnostic, or a fixture. |
| 9 | The broad gate passes on the exact candidate, and every test that was run is recorded with its outcome | See Verification below. |

## Additional guard evidence

- `tests/static_guards.rs::fetch_pass_cookie_cache_is_process_local_and_has_one_owner` fails
  closed unless exactly one file defines the fetch-side cache, fails closed if any file outside
  `src/challenge_replay/` touches it, and fails closed if the cache module references the
  filesystem, a serialization path, or a temporary-file helper — that is, if a pass cookie could
  ever be persisted to disk. It counts the owner and the walked files, so it cannot pass
  vacuously.
- `tests/static_guards.rs::fetch_layer_never_imports_the_challenge_solver` still passes: no file
  under `src/fetch/` references the solver or the shared module.
- `tests/static_guards.rs::challenge_solve_orchestration_has_one_owner` still passes: the MCP and
  CLI layers still never call the solver directly.
- `tests/static_guards.rs::altcha_fetch_solve_has_one_owner` still passes: the ALTCHA solve still
  has exactly one owner.

## Verification

Using the pinned Rust 1.89 toolchain on candidate `5a98682bda4a68ec55dc9e28409fd8269d1f284d`:

- `make check`: exit 0 — 85 suites, 6,420 tests, 0 failed, 28 ignored. This includes fmt, clippy
  with `-D warnings`, the no-default-feature check, the all-features test run, repository hygiene,
  the dependency policy, and the packaging contract checks.
- `make docs-check`: exit 0 (`cargo doc --locked --all-features --no-deps`).
- `make packaging-check`: exit 0, including `planning-consistency: ok`, the workflow pin check over
  nine workflow files, the Eggpack qualifier contract over 25 producer jobs and 7 targets, and the
  egress qualification contract.
- `cargo test --locked --all-features --test web_fetch_integration`: 113 passed, 0 failed, 1 ignored.
- `cargo test --locked --all-features --test repo_workflow`: 99 passed, 0 failed.
- `cargo test --locked --all-features --test static_guards`: 67 passed, 0 failed.
- `cargo test --locked --all-features --lib challenge_replay`: 29 passed, 0 failed.
- `cargo test --locked --all-features --lib meta::challenge`: 118 passed, 0 failed.
- `cargo test --locked --all-features --lib meta::engines::startpage`: 51 passed, 0 failed.
- `cargo test --locked --all-features --bins`: 18 passed, 0 failed.
- `git diff --check`: clean.

All new tests use loopback `httpmock` fixtures. No public upstream was contacted and no fresh live
traffic was generated for M033. A live two-call observation on a real protected origin was not run
and is recorded below as a residual risk rather than claimed as evidence.

## Compatibility and migration effects

- No MCP tool, argument, or advertised-schema change. The canonical advertised inventory is
  unchanged and the byte-pinned advertisement still matches.
- No configuration key is added, removed, or renamed, and no shipped default changes.
  `[search.challenge].enabled` remains the single opt-in and remains off by default, and
  `cookie_ttl_seconds` keeps its default of 240 seconds.
- `Cargo.toml` is unchanged, so no dependency moved.
- The refusal vocabulary in `src/meta/challenge/ledger.rs` keeps its ten keys. No new bound name
  was introduced, and the fetch-side reuse records nothing new in the solve ledger, so the
  `provider_status` `challenge_solver` block is unchanged.
- `src/meta/challenge/cookie.rs`, M014's lifetime rules, and the search-side `PassCookieCache`
  are unchanged. The fetch-side cache reuses M014's `reuse_ttl` resolution rather than
  re-implementing it.
- With the opt-in off, every surface behaves exactly as before: the disabled-solver tests assert a
  marked response, no challenge request, and no replay, and the cache is never consulted.

## Security evidence

- The permitted class did not widen. ADR-0009's prohibition list is unchanged and unexercised: no
  interactive CAPTCHA, OCR, solving service, credential bypass, proxy rotation, user-agent
  spoofing, or retry widening was added. No scheme was added to the fetch path.
- A pair set is reused only for the origin that issued it. The key is the scheme, host, and
  effective port of the request URL, and an entry is stored only when the origin that served the
  challenge page is that same origin. A fetch that crossed an origin stores nothing and reuses
  nothing, because the issuing origin cannot be established without following the redirect.
- The replayed pair set still travels on the first hop only, so a redirect cannot forward it to
  another origin.
- The reuse window never exceeds an applicable published lifetime and never exceeds
  `cookie_ttl_seconds`. An unstated lifetime keeps the configured-ceiling fallback; a malformed
  lifetime, a zero lifetime, and an expiry at or before now store nothing.
- A pair set the upstream rejects is evicted, so the next fetch starts cold. The cache never holds
  a token the server has already refused.
- The cache is process-local. It is bounded in entry count (32 origins) and in bytes (64 KiB), it
  is never written to disk, never persisted to a cache directory, and never serialized into a
  response, a diagnostic, or a fixture. A static guard fails closed if the cache module gains a
  filesystem or serialization path.
- The single-replay rule is unchanged: one solve and one replay per fetch call. A reused pair set
  performs no solve, and a fetch never both reuses a pair set and solves in the same call.
- No cookie value, nonce, salt, challenge id, or pass token reaches a log, an error, a warning, a
  `Debug` output, a fixture, or a `provider_status` block.

## Observable behaviour changes

Three, all intended:

1. A second challenged fetch to the same origin now replays the stored pair set and performs no
   solve. This is a latency and cost reduction, not a correctness change.
2. A cached pair set the upstream rejects is evicted, so the fetch that met the rejection returns
   the original marked response and the next fetch solves again.
3. The `doctor` egress warning text now names the fetch path alongside the search upstreams. The
   warning count is unchanged.

## Known limitations and residual risk

1. A stale pair set costs one degraded fetch. When the upstream's challenge changes, the stored
   pair set is rejected; that fetch returns the original marked response, the entry is evicted, and
   the next fetch solves. The design is bounded, but the first fetch after a challenge change is
   not a document.
2. Reuse does not apply to a fetch that crossed an origin. A redirect to another host, scheme, or
   port stores nothing and reuses nothing.
3. The reuse window is not refreshed on a hit. The entry expires at the lifetime published at solve
   time even when the upstream issues a newer cookie during the replay, so reuse is never longer
   than the original publication.
4. The cache is per-process. A restart loses every entry, and a multi-process deployment does not
   share one.
5. The fetch-side reuse is not recorded in the solve ledger, so `provider_status` reports no
   fetch-side cookie hit or miss counter. This keeps the reported surface unchanged, and it means
   the reuse is observable only through latency and upstream request counts.
6. The reuse covers the Anubis and ALTCHA families only, which are the only families the fetch path
   solves.
7. No live qualification against a real protected origin was run. All evidence is deterministic
   loopback evidence. The measured saving is therefore a count of avoided solves in a fixture, not
   a measured production latency figure.
8. The fetch-side cache is process-local and keyed by origin, so a loopback port recycled between
   tests in one test binary can carry a stale entry across tests. This was observed as a real
   failure during implementation and is fixed by a per-origin reset in the shared challenge test
   helper (`challenge_server()`) rather than by a production behaviour change.

## Recommendation

Accept M033 and close it. The milestone's criteria are met with deterministic loopback evidence:
reuse is bounded by the published lifetime and the operator ceiling, is confined to the issuing
origin, evicts on rejection, is process-local and bounded, and leaves the tool surface, the
configuration surface, the refusal vocabulary, and M014's rules unchanged. The one known
first-fetch degradation after a challenge change is bounded, self-correcting, and consistent with
the search-side behaviour M014 already established.
