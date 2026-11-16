# M015 — Challenge Solvability Observability

Status: closed
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M015
Primary class: capability
Baseline: `dace41c3a75307ea35b745d695a11630ec6495c0` (the worktree also carries the uncommitted ADR-0010 addition and the roadmap/ADR-0009 edits from the prior turn; re-read `git status` before starting)
References: ADR-0009; ADR-0010; `architecture/engines.md` § Challenge solving; `src/meta/challenge/mod.rs`; `src/meta/challenge/{anubis,altcha}.rs`; `src/meta/engines/{startpage,mojeek}.rs`; `src/mcp/tools/provider_status.rs`; `src/mcp/output_schema.rs`; `src/mcp/projection.rs`; `tests/mcp_tool_contract.rs`; `tests/mcp_schema_slimming.rs`; `docs/provider-setup.md`; `docs/config.md`; `tests/fixtures/engines/README.md`.
Dependencies: none hard. Soft M013 is the future producer of `already spent` (its `BOUND_SPENT`), `unsafe base prefix` (its `BOUND_BASE_PREFIX`), and its projection refusal; M014 changes when a cached cookie is dropped, not the counter set. This block carries unproduced keys at zero and needs no interface contract from either.

## Objective

Make the cost of `[search.challenge]` visible in the diagnostics surface an operator already calls, without a new tool, without reading logs, and without exposing any secret material.

Today `provider_status` reports `blocked` and one reason string, and nothing else. An operator cannot tell whether the solver is disabled, above the difficulty ceiling, out of attempts, out of wall clock, facing an unsupported scheme, or holding a valid pass cookie. Anubis exposes `anubis_challenges_issued`, `anubis_challenges_validated`, `anubis_failed_validations`, and `anubis_challenge_time_taken_seconds` for exactly this question; eggsearch exposes nothing. The exit condition is that after one solve the operator can read attempts, the outcome by name, cookie-cache behaviour, and the last candidate count and wall-clock cost from one `provider_status` call.

## Non-goals

- No new MCP tool, no new `provider_status` argument, no tool rename or removal.
- No change to `error_class` wire strings, `FailureClass`, `parse_shape`, health status, cooldown, or routing decisions.
- No persistence, metrics endpoint, Prometheus/OpenTelemetry export, or log-format change.
- No per-request, per-subquery, or per-session detail and no history ring buffer: process-lifetime counters plus one last observation.
- No cookie, nonce, salt, challenge id, redirect target, derived key, or challenge body in the ledger, at any point, in any form.
- No change to any solve budget, ceiling, default, or engine classification path; observability does not alter behaviour.
- No new dependency.

## Invariants that cannot regress

- Exactly ten MCP tools; `provider_status` remains the diagnostics surface.
- An unsolved, unsupported, or budget-exhausted challenge still produces `EngineError::Blocked` → `error_class: blocked` → `FailureClass::Blocked`, never `Ok(vec![])`; a solved-but-unparseable page stays `parse_error`. Recording happens beside the outcome, never instead of it, and the Anubis ceiling refusal keeps its exact `anubis difficulty N exceeds configured ceiling M` text.
- `PassCookieCache` remains the only source of cookie reuse and keeps its TTL, capacity, provider isolation, and zero-TTL semantics. The ledger observes a lookup; it never stores a value.
- Solving stays opt-in and off by default. With `enabled = false` no lookup, solve, or ledger record happens, and engine behaviour is byte-identical.
- Every solve stays inside `max_attempts`, `timeout_ms`, `max_difficulty`, and the search deadline. Recording is O(1) and takes no lock inside the candidate or counter loop.
- Ordinary files under 1,600 lines / 80 KB; `src/meta/provider_diagnostics.rs` stays inside its 2,200-line / 81,920-byte ratchet and is not touched at all. No code comments, no new dependency.

## Expected production changes

1. **`src/meta/challenge/ledger.rs` (new).** A bounded, process-local ledger: `SolveLedger` owning `Mutex<BTreeMap<String, ProviderSolveStats>>`, a process-global `solve_ledger() -> &'static SolveLedger` mirroring `pass_cookie_cache()`, and a `Serialize` snapshot. API:

   ```text
   record_outcome(provider: &str, outcome: RecordedOutcome, candidates: u64, elapsed: Duration)
   record_cookie_lookup(provider: &str, hit: bool)
   snapshot() -> SolveLedgerSnapshot
   ```

   `RecordedOutcome` is `Solved`, `Refused(RefusalReason)`, or `Errored`. One `record_outcome` call increments `solves_attempted` and exactly one outcome counter under a single lock, so `solves_attempted == solves_succeeded + refusals_total + solves_errored` holds by construction and a panic records nothing. Every increment is `saturating_add`. Tests build a local `SolveLedger::new(cap)`; only a pointer-equality test touches the global, so the existing per-engine `CHALLENGE_TEST_LOCK`s stay unchanged and no test clears shared state.

2. **Refusal vocabulary is a fixed nine-key object**, always emitted in full so a client's key set never changes and a zero is itself evidence that the bound never fired:

   | Key | Producer |
   |---|---|
   | `difficulty above ceiling` | Anubis `check_ceiling` (today an `EngineError::Blocked`) |
   | `attempts exhausted` | `SolveBudget::attempt`, nonce/counter overflow |
   | `wall clock exhausted` | `SolveBudget::attempt`, `preact_wait` |
   | `unsupported scheme` | Anubis/ALTCHA parse |
   | `malformed parameters` | Anubis/ALTCHA parse, ALTCHA submission encode |
   | `pass cookie absent` | verify returned no pass cookie |
   | `pass cookie rejected` | verify returned a non-2xx |
   | `challenge expired` | no producer today, in M013, or in M014; reserved for a challenge-validity check |
   | `already spent` | M013's `BOUND_SPENT`; zero until then |

   `pass cookie absent` and `pass cookie rejected` are included because the solver emits them today; omitting them would make two real blocked reasons invisible. `challenge expired` and `already spent` are included because ADR-0010 names the spent case as a fail-closed outcome and ADR-0009 bounds challenge validity, and because the operator question must already have a slot when the producer lands. `challenge expired` is knowingly a zero-only key at every milestone currently planned: dropping it would leave no slot for a validity refusal without a schema change, and keeping it costs one `u64` in a fixed-key object.

   `RefusalReason::from_bound(&'static str)` maps the solver's `BOUND_*` constants to keys; an unmapped name increments `solves_errored` rather than inventing a JSON key, and an exhaustive unit test over every `BOUND_*` constant fails if a new bound is added without a key. That test is the trigger for M013: when `BOUND_SPENT` and `BOUND_BASE_PREFIX` land, the same change must add `already spent` (already present here, so it starts moving immediately) and a key for `unsafe base prefix`.

3. **One recording site per scheme.** `anubis::solve_and_verify` and `altcha::solve_and_verify` each become a thin wrapper over an inner function returning a private terminal value: `Solved(cookie)`, `Refused(RefusalReason)`, `Blocked { reason: String, key: RefusalReason }`, with any outer `Err` recorded as `Errored`. The wrapper maps `Blocked { reason, key }` to `RecordedOutcome::Refused(key)` before recording and then returns `Err(EngineError::Blocked { reason })`, so the ceiling path keeps today's exact `ceiling_refusal` text while the ledger still gets a named key. Carrying the ledger key *inside* the blocked terminal is what keeps a dynamic-sentence refusal attributable: M013's projection refusal returns `Err(Blocked)` with a sentence that no bound constant spells, and must record its own key rather than degrading into `errored`. `candidates` is `budget.attempts_used()`; `elapsed` is measured from a successful parse to the solver's decision, excluding every HTTP request, so the pair reports the computation `max_difficulty` buys rather than network latency. A refusal before any hashing (parse, ceiling, projection) reports `0` for both.

4. **Cookie accounting.** New `challenge::lookup_pass_cookie(provider) -> Option<String>` wraps `pass_cookie_cache().get()` and records one hit or one miss; `startpage.rs` and `mojeek.rs` call it in place of the raw read inside their existing `if challenge.enabled` branch, so a lookup is counted only when the cache is actually consulted. Storing a cookie records nothing, which makes `cookie_ttl_seconds = 0` show one miss per request, zero hits, and one solve per request.

5. **`provider_status` block.** A top-level additive object, sibling of `providers`, `health`, and `search_cache`, built in `build_payload_without_probe` from `crate::meta::challenge::solve_ledger().snapshot()`. Field set and types are fixed:

   ```json
   "challenge_solver": {
     "enabled": true,
     "untracked_providers": 0,
     "providers": {
       "mojeek": {
         "solves_attempted": 3,
         "solves_succeeded": 1,
         "solves_errored": 0,
         "refusals": {
           "difficulty above ceiling": 0,
           "attempts exhausted": 1,
           "wall clock exhausted": 0,
           "unsupported scheme": 0,
           "malformed parameters": 0,
           "pass cookie absent": 0,
           "pass cookie rejected": 1,
           "challenge expired": 0,
           "already spent": 0
         },
         "cookie_hits": 4,
         "cookie_misses": 3,
         "last_candidates": 291,
         "last_wall_clock_ms": 420
       }
     }
   }
   ```

   `enabled` is `bool`; `untracked_providers` is `u64`; `providers` is an object keyed by provider id; each entry is seven `u64` values (`solves_attempted`, `solves_succeeded`, `solves_errored`, `cookie_hits`, `cookie_misses`, `last_candidates`, `last_wall_clock_ms`) plus a nine-key `refusals` object of `u64`. `last_candidates` is the budget attempts consumed by the most recent attempt; `last_wall_clock_ms` is that attempt's bounded-computation milliseconds. Both describe the last attempt literally, including `0` when it ended before any computation.

6. **Always reported, including when disabled — decision.** The block is emitted on every `provider_status` call and is never conditional on `[search.challenge].enabled`. `search_cache` is omitted when its cache is absent because absence means "no cache configured"; here absence would make "solver disabled" indistinguishable from "field not built", and "the solver is disabled" is the most common answer to "why is this provider blocked". `enabled` inside the block means the operator never has to open the config file to read an empty `providers` map. The disabled cost is one ~40-byte object, and the block cannot grow past its caps.

7. **Schema and docs.** `src/mcp/output_schema.rs` gains exactly one `provider_status` property, `"challenge_solver": {"type": "object"}` (376 → ~413 bytes for that schema; per-schema ceiling 1,200). The envelope stays `additionalProperties: true`, so the inner shape is documented in prose rather than schema: `architecture/engines.md` § Challenge solving (shape, reset-on-restart, always-reported decision) and `docs/provider-setup.md`. One sentence in `docs/config.md` where `[search.challenge]` is documented, and the advertisement sizes quoted there are re-measured rather than hand-edited in one place only. Tool descriptions, annotations, and the `provider_status` contract text are unchanged, because changing them would grow the pinned advertisement beyond the schema delta.

## Storage, protocol, and compatibility effects

- Process-local memory only; no disk, no migration, no persistence. Every counter is a process-lifetime total that resets to zero on restart, and the block documents that sentence at the point an operator reads it.
- Monotonic within a process: counters never decrease and saturate at `u64::MAX` instead of wrapping.
- Additive schema; no existing key renamed, removed, or retyped. `parse_shape`, `health`, `health_views`, `error_class`, cooldown, and routing are untouched, and `providers` keeps its current entry shape.
- Bounds:

  | Bound | Value | Behaviour |
  |---|---|---|
  | Provider entries | 4 (`MAX_LEDGER_PROVIDERS`; twice the shipped challenge-capable engine count) | A fifth provider id is not tracked and increments `untracked_providers`; monotonic totals are never evicted, because eviction would make a total decrease |
  | Per-provider fields | 7 numeric + 9 fixed refusal keys | no dynamic keys, so a bound string can never become a JSON key |
  | Serialized block | 4,096 bytes (`MAX_LEDGER_BYTES`) | worst case is four saturated entries; asserted by test |
  | Counter range | `u64`, saturating | no wrap at `u64::MAX` |

- `provider_status` is outside projection: `src/mcp/projection.rs` matches only the search, fetch, batch, repo, and evidence envelopes, and the tool has no `response_detail` argument. The block therefore cannot reach a `compact` or `brief` search response, and no projection rule or limit changes.
- Clients tolerate one extra key at the envelope level because the advertised schema is permissive; clients that pin `tools/list` bytes do not, so the pin in `tests/mcp_tool_contract.rs` moves with the schema in the same change.

## Ordered work packages

1. `ledger.rs`: stats struct, saturating increments, fixed refusal map, snapshot serialization, entry cap, `untracked_providers`; unit tests for the arithmetic identity, per-bound distinguishability, saturation, entry cap, empty snapshot, and secret-free serialization.
2. `RefusalReason::from_bound` plus the exhaustive-over-`BOUND_*` test. No new constant is added to `mod.rs` and no existing constant string changes.
3. `anubis.rs` and `altcha.rs`: inner/wrapper split with the private terminal value, one recording point, parse-to-decision timing, `budget.attempts_used()` candidates; the ceiling path keeps `Err(Blocked)` and its exact message.
4. `challenge/mod.rs`: `lookup_pass_cookie`; swap the two engine call sites while leaving the `if challenge.enabled` gate where it is.
5. Engine flow tests: one solved, one per reachable baseline bound (`difficulty above ceiling`, `attempts exhausted`, `wall clock exhausted`, `unsupported scheme`, `malformed parameters`, `pass cookie absent`, `pass cookie rejected`), one errored, hit and miss within the TTL, and `cookie_ttl_seconds = 0`. Read the provider entry before and after and assert the delta, under the existing `CHALLENGE_TEST_LOCK`, so the two engine test modules cannot interfere.
6. `provider_status.rs`: build the block, set `enabled` from `state.config.search.challenge`, always insert. `output_schema.rs`: add the property.
7. `tests/provider_routing.rs`: block present when disabled with `enabled: false` and an empty `providers` map, documented key set and JSON types when exercised, existing per-provider keys unchanged, canary absence.
8. `tests/mcp_tool_contract.rs`: re-measure and update the full and compact advertisement byte counts and fingerprints pinned by `full_preset_advertisement_is_byte_identical_to_the_pre_change_surface`; the delta must be the added schema property only. Confirm `tests/mcp_schema_slimming.rs`'s 74,000-byte total still holds.
9. Docs: `architecture/engines.md` § Challenge solving, `docs/provider-setup.md`, `docs/config.md`, `docs/test-inventory.md` (per-suite counts move), `tests/fixtures/engines/README.md` (live record), `CHANGELOG.md`.

**Required tests:**

- Counters increment once per attempt and once per outcome: `solves_attempted` and exactly one outcome counter move per recorded outcome, and the identity `attempted == succeeded + refusals_total + errored` holds.
- Every documented refusal bound is observable and distinguishable: recording each of the nine increments that key by one and leaves the other eight at zero, and every `BOUND_*` constant maps to a documented key.
- Cache accounting matches the cache: a seeded cookie within the TTL gives a hit with no new solve; `cookie_ttl_seconds = 0` gives one miss per request, zero hits, and a solve per request.
- Saturation: a counter seeded at `u64::MAX` stays at `u64::MAX` after a further increment, without panic or wrap.
- No secret material in the serialized payload: with a canary pass cookie in the cache, no canary, nonce, salt, prefix, or derived key appears, and every value inside `providers` is a number.
- The block exists with an empty `providers` map before any solve, so a schema-stable client sees a stable key set rather than an absent object.

## Verification

```bash
cargo fmt --all
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features --test mcp_tools
cargo test --locked --all-features --test provider_routing
cargo test --locked --all-features --test static_guards
cargo test --locked --all-features
```

Also required, because this change moves a pinned advertisement and a schema budget:

```bash
cargo test --locked --all-features --test mcp_tool_contract
cargo test --locked --all-features --test mcp_schema_slimming
cargo test --locked --all-features --lib
make check
```

Live, opt-in, recorded rather than asserted. The ledger is process-local, so both reads must hit one server process — a second process would report an empty map. Write `/tmp/challenge-observability.toml`:

```toml
[search]
mode = "live"

[search.providers]
mojeek = true

[search.challenge]
enabled = true
max_difficulty = 5
timeout_ms = 12000
cookie_ttl_seconds = 0
```

```bash
eggsearch --config /tmp/challenge-observability.toml mcp serve --bind 127.0.0.1:11320 --path /mcp
```

In one MCP session (`initialize`, then `notifications/initialized`), call in order: `provider_status` with `{}` for the before state (`enabled` true, `providers` `{}`); `web_search` with `{"query":"rust programming language","providers":["mojeek"]}`; and `provider_status` with `{}` again for the after state (one `mojeek` entry).

Record in `tests/fixtures/engines/README.md` § Challenge solving: the timestamp, the before and after blocks verbatim, `solves_attempted`, `solves_succeeded`, `last_candidates`, `last_wall_clock_ms`, and whether a second search inside a non-zero TTL incremented `cookie_hits` without a new solve. `cookie_ttl_seconds = 0` is chosen for the recorded run so the miss-per-request rule is observable live as well. Nothing in the block needs redaction; the record must still avoid pasting a raw `provider_status` payload from a configuration that also has credentialed providers configured.

## Acceptance and stop conditions

- `provider_status` always contains `challenge_solver` with `enabled` reflecting configuration; `providers` is an empty object until the first solve.
- After one successful solve: `solves_attempted == 1`, `solves_succeeded == 1`, `last_candidates > 0`, `last_wall_clock_ms > 0`.
- After a ceiling refusal: `refusals["difficulty above ceiling"] == 1`, `last_candidates == 0`, and every other refusal counter is 0.
- Each of the seven reachable bounds is distinguishable in a mock flow test, and the two forward-declared keys exist with a zero value.
- Inside the TTL a replay adds `cookie_hits` and no solve; with `cookie_ttl_seconds = 0` every request adds one `cookie_misses` and one solve.
- Saturation holds at `u64::MAX`; the entry cap holds and `untracked_providers` accounts for the drop; the block serializes within 4,096 bytes.
- No secret material appears in the serialized payload, and no value inside `providers` is a string.
- Existing `provider_status` keys, `parse_shape`, error classes, health, cooldown, routing, and the ten-tool surface are unchanged; the only advertisement movement is the one added output-schema property and its re-measured byte pin.
- Stop and return for a decision if the ledger cannot stay inside the caps without dropping a documented reason, if recording would need state or a lock on the candidate-search hot path, or if any proposal needs per-request or per-session detail, persistence, or a metrics endpoint.

## Closure evidence

`plans/closure/agentic-retrieval-integrity/015-status.md` must record: the implementation candidate SHA; the exact test commands with per-suite counts; the recorded `challenge_solver` block before and after one live mojeek solve, with timestamps and the raw `last_candidates` and `last_wall_clock_ms`; the cache hit/miss observation including the zero-TTL run; the measured schema and advertisement byte delta with the updated pin values; the saturation, entry-cap, and 4,096-byte evidence; the canary-based secret-absence evidence; and residual risk (counters reset on restart, counters are totals and not a rate, two refusal keys stay zero until M013/M014, and a fifth provider id is untracked by design). The registry row, the roadmap status, and this closure record move in the same closure commit.
