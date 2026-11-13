# M014 — Pass-Cookie Lifetime and Egress Binding

Status: closed
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M014
Primary class: capability
Baseline: `dace41c3a75307ea35b745d695a11630ec6495c0` (the worktree also carries a modified ADR-0009 and an untracked ADR-0010; M013 and M016 edit `src/meta/challenge/` in parallel, so re-read `git status` and the directory before editing)
References: ADR-0009 § Decision; ADR-0010 § Decision; `architecture/engines.md` § Challenge solving; `src/meta/challenge/mod.rs`; `src/meta/challenge/anubis.rs`; `src/meta/challenge/altcha.rs`; `src/meta/engines/startpage.rs`; `src/meta/engines/mojeek.rs`; `src/core/config/challenge.rs`; `src/fetch/egress.rs`; `src/commands/doctor.rs`; `docs/config.md` § Challenge Solving; `architecture/maintenance.md` § File size ratchets.
Dependencies: none (roadmap). Interface only: M015 owns solve and cache counters, so this milestone measures by inspecting the in-process cache instead of adding a counter.

## Objective

Derive the pass-cookie reuse window from the lifetime the upstream actually published, clamped by `cookie_ttl_seconds`, and decide plus document what happens when a pass token is bound to the client's egress address. A repeat search must reuse a cookie for as long as it is valid and must never present a token the server has already told us is dead.

Baseline evidence: `startpage.rs:94` and `mojeek.rs:81` store the solved cookie for `Duration::from_secs(cookie_ttl_seconds)` regardless of what the response said, because `SolveOutcome::Solved { cookie: String }` drops the attributes that `extract_cookie` (`anubis.rs:397`, `altcha.rs:286`) is already holding. The only new input in this milestone is a response header; no configuration key is added.

The point of the change is fewer solves per unit of work: a clamp that never binds and a derivation that never bites change nothing. The measurement in § Verification exists to prove which of those happened, and the closure record states it even when the answer is "no upstream published a lifetime".

## Non-goals

- No new configuration key, and no change to `enabled`, `max_difficulty`, `max_attempts`, `timeout_ms`, or the validated bounds of `cookie_ttl_seconds`.
- No reading the Anubis JWT `exp` claim. The `Set-Cookie` attribute is the direct statement; a token payload is a second, unverified source. If a provider turns out to publish neither attribute, that is a stop condition, not licence to parse the token.
- No egress detection and no route-dependent TTL policy. `JWT_RESTRICTION_HEADER` is a server-side deployment variable; the client observes only its effect.
- No proactive refresh. A cookie is never re-solved ahead of expiry to hide a stale window; that would add requests the operator did not ask for.
- No change to solver coverage, budgets, difficulty handling, `error_class` wire strings, `provider_status`, MCP tool surface, or response shape.
- No persistence, no cross-provider sharing, no `Debug` or log exposure of a cookie value, no new fixture containing one.
- No timer, sleep, or background task; time enters the resolver as a supplied value.

## Invariants that cannot regress

- Ten MCP tools, stable FNV-1a IDs, sanitization tiers, trust framing, and deadline bounds are unchanged.
- A disabled, exhausted, unsupported, or above-ceiling challenge still returns `EngineError::Blocked` → `error_class: blocked` → `FailureClass::Blocked`, never `Ok(vec![])`; a solved page that does not parse is still `ParseFailed`.
- At most one solve per provider per search request, and a reuse window that never exceeds `cookie_ttl_seconds`; `cookie_ttl_seconds = 0` disables reuse outright.
- A missing, malformed, or past expiry never yields indefinite reuse. Absent attributes fall back to `cookie_ttl_seconds`; a present-but-unusable attribute yields no reuse.
- Cookie values never appear in logs, errors, `provider_status`, diagnostics, `Debug`, fixtures, or the closure record. New fixtures carry attribute names and values only, never a `name=value` pair.
- The cache stays process-local, keyed per provider, bounded by the entry cap, never persisted, never shared across providers; provider isolation and cap eviction keep their current tests.
- Every solve and verify response is still read through `read_bounded_body`; no bare `.text()`/`.bytes()` is introduced.
- No sleep-based test. `now` is a parameter.
- Ordinary files stay under 1,600 lines / 80 KB and inside their ratchets; no explanatory code comments; `cargo fmt` clean.

## Expected production changes

1. **New `src/meta/challenge/cookie.rs`** — the single owner of pass-cookie lifetime parsing, so the two scheme modules stop carrying duplicate copies:
   - `pub enum PublishedLifetime { Unstated, MaxAge(u64), ExpiresAt(SystemTime), Malformed }`, `Clone + Copy + PartialEq + Eq`, with no raw attribute text inside it.
   - `pub fn parse_issued(headers: &http::HeaderMap, name: &str) -> Option<(String, PublishedLifetime)>`. The value is read from the cookie-pair segment only, not from an attribute that happens to start with the same text; attribute names match case-insensitively; `Expires` is parsed as IMF-fixdate (`%a, %d %b %Y %H:%M:%S GMT`) with `chrono`, which is already a direct dependency, so no dependency-policy delta exists. An attribute that is present but unusable is `Malformed`, never "absent".
   - `pub fn reuse_ttl(lifetime: PublishedLifetime, now: SystemTime, ceiling: Duration) -> Duration`, where `Duration::ZERO` means "do not reuse" and matches `PassCookieCache::put`'s existing zero rule.

   Resolution order, applied in this sequence:

   1. `ceiling.is_zero()` → `ZERO`. `cookie_ttl_seconds = 0` wins over every published lifetime.
   2. `Unstated` (neither `Max-Age` nor `Expires`) → `ceiling`, which is today's behaviour exactly.
   3. `Malformed` → `ZERO` (fail closed, never indefinite).
   4. `MaxAge(0)` → `ZERO`; `MaxAge(secs)` → `min(secs, ceiling)`.
   5. `ExpiresAt(at)` → `at.duration_since(now)`, clamped by `ceiling`; an expiry at or before `now` → `ZERO`.
   6. `Max-Age` takes precedence over `Expires` when both are present (RFC 6265 §5.3), including when `Expires` is already in the past.

   | Response attribute | `cookie_ttl_seconds` | Stored TTL |
   |---|---|---|
   | neither | 240 | 240 s (unchanged) |
   | `Max-Age=45` | 240 | 45 s |
   | `Max-Age=3600` | 240 | 240 s |
   | `Max-Age=3600` | 0 | none |
   | `Max-Age=0` | 240 | none |
   | `Expires` 60 s ahead | 240 | 60 s |
   | `Expires` in the past | 240 | none |
   | `Max-Age=abc` | 240 | none |
   | `Max-Age=3600`, past `Expires` | 240 | 240 s |

2. **`SolveOutcome::Solved`** gains the lifetime: `Solved { cookie: String, lifetime: PublishedLifetime }`. The hand-written `Debug` keeps printing `<redacted>` for the value and prints only the attribute kind.

3. **`anubis.rs` / `altcha.rs`** delete their private `extract_cookie` copies and call `cookie::parse_issued` at `anubis.rs:321` and `altcha.rs:270`, returning the parsed lifetime with the value.

4. **`PassCookieCache::remove(&self, provider: &str)`** — per-provider eviction. `clear()` wipes every provider and must not be used for this.

5. **`startpage.rs` / `mojeek.rs`** resolve and store, then invalidate on rejection:
   ```rust
   let ttl = cookie::reuse_ttl(lifetime, SystemTime::now(), Duration::from_secs(challenge.cookie_ttl_seconds));
   pass_cookie_cache().put(ENGINE, &cookie, ttl);
   ```
   and, when the replay is also challenged, `pass_cookie_cache().remove(ENGINE)` before returning `blocked(BOUND_COOKIE_REJECTED)`.

6. **`src/commands/doctor.rs`** (`collect_warnings`) emits one warning when `cfg.egress.is_routed()` and `cfg.search.challenge.enabled` and `cookie_ttl_seconds > 0`: `[egress] is routed and [search.challenge] reuses pass cookies; a challenge token bound to the client's egress address is re-challenged or rejected when the exit address changes — set [search.challenge].cookie_ttl_seconds = 0 if the route does not keep one exit address per process`. No new `WarningCode`, no MCP response field, no validation failure.

7. **Documentation.** `docs/config.md` § Challenge Solving gains the attribute-derivation rule plus the egress paragraph; `architecture/engines.md` § Challenge solving gains the invariant and the mechanism; `architecture/maintenance.md` § ownership line for bounded challenge solving names `cookie.rs`; `CHANGELOG.md` gets one `[Unreleased]` entry; `docs/test-inventory.md` § Test Counts is refreshed because two aggregate numbers move.

**Egress decision: keep using the cached cookie, bound the consequence, and document it. Do not detect, and do not shorten the TTL because a route exists.** Detection is rejected because the restriction is opaque to the client: shortening every routed deployment's TTL would penalise the common fixed single-hop route to insure against a rotating exit that eggsearch neither configures nor can observe, and eggsearch already forbids proxy rotation as a capability. The smallest change that keeps "accept a possible re-challenge" from degrading into "blocked for the whole TTL" is item 5's eviction on rejection: the failing search still reports `blocked` with `pass cookie rejected`, and the next search starts cold instead of retrying a token the server just refused.

## Storage, protocol, and compatibility effects

No configuration key, TOML shape, MCP tool, schema, wire string, or ID changes; a config written before this milestone keeps its exact meaning, and the only new input is a response header. Cache shape is unchanged — same map, same entry cap, same `Instant`-based expiry — so eviction ordering and provider isolation are untouched.

Two observable effects, both intended: a cookie whose published lifetime is shorter than the ceiling is no longer presented after it has died (one fewer request and one fewer `pass cookie rejected` per stale window), and a cookie whose published lifetime is longer is still capped at `cookie_ttl_seconds`, so nothing reuses longer than the operator allowed. `Expires` is absolute wall-clock time converted once to a duration, so a backwards system-clock step can extend reuse by at most the ceiling — bounded, never indefinite.

Egress behaviour is unchanged in production: `src/meta/engines/mod.rs:1286` routes fixed provider upstreams, dynamic fetch stays direct, and no route information reaches the resolver.

## Ordered work packages

1. `challenge/cookie.rs`: parser and resolver with pure unit tests driven by a fixed `now` (no sleeps). Cover every row of the table, plus attribute-name case, the cookie-pair-versus-attribute value rule, and `Max-Age` precedence over a past `Expires`.
2. `SolveOutcome` widening plus both scheme call sites; update the existing in-module matches in `anubis.rs`/`altcha.rs` and delete the duplicated helpers. Prove `Debug` still redacts the value.
3. `PassCookieCache::remove` plus a unit test that it drops one provider and leaves the others.
4. Startpage wiring, including eviction on rejection. Flow tests against the mock: `Max-Age=0` on the pass cookie still returns rows but leaves no cache entry; `Max-Age=300` with a 240 s ceiling leaves one; a rejected replay with `cookie_ttl_seconds = 60` leaves none (today's `rejected_pass_cookie_blocks_without_leaking_it` uses `cookie_ttl_seconds = 0`, which cannot prove eviction — extend it or add a sibling).
5. Mojeek wiring with the mirror tests.
6. Egress guard: `PassCookieCache::remove` on rejection is already wired in 4 and 5; add the `doctor` warning and its test (present when both sections are set, absent when either is off).
7. Docs batch: `docs/config.md`, `architecture/engines.md`, `architecture/maintenance.md`, `CHANGELOG.md`, `docs/test-inventory.md`.
8. Live measurement and its record in `tests/fixtures/engines/README.md`.

## Verification

```bash
cargo fmt --all
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features --lib meta::challenge
cargo test --locked --all-features --test config_validation
cargo test --locked --all-features --test static_guards
cargo test --locked --all-features
```

The canonical gate `make check` must also pass before the commit that closes the milestone.

Required test evidence, each named so a missing one is visible:

- Published `Max-Age` shorter than the ceiling wins; longer is clamped to the ceiling.
- `Expires` in the past returns no reuse; `Expires` in the future is honoured and clamped.
- Neither attribute present falls back to `cookie_ttl_seconds`.
- `cookie_ttl_seconds = 0` disables reuse even for a long-lived cookie.
- Malformed `Max-Age` and malformed `Expires` each return no reuse.
- Provider isolation and entry-cap eviction still hold, plus a mixed-TTL case where only the expired provider's entry is dropped.
- No cookie value appears in any error, `Debug`, or diagnostic output asserted by the test, including the malformed-attribute path.
- Disabled path: one request, `blocked`, no solve, unchanged (`startpage.rs:413` / `mojeek.rs:379` remain green).

Recorded live step, opt-in and not asserted. One `#[ignore]`d test in `tests/web_search_integration.rs` drives `startpage::search` (or `mojeek::search`) repeatedly inside one process so the process-local cache is actually shared, reads `pass_cookie_cache().get(<id>)` before and after each call, and counts a solve when the entry was absent before the call or its value rotated between calls. It prints only the tick index, wall milliseconds, result class, and the solve count — never a cookie value.

```bash
EGGSEARCH_CHALLENGE_LIVE_PROVIDER=startpage \
EGGSEARCH_CHALLENGE_LIVE_SEARCHES=40 \
EGGSEARCH_CHALLENGE_LIVE_INTERVAL_SECS=30 \
EGGSEARCH_CHALLENGE_LIVE_TTL_SECONDS=240 \
cargo test --release --locked --all-features --test web_search_integration \
  -- --ignored --nocapture challenge_lifetime_live
```

Two arms over the same window. Arm A is the derivation active. Arm B sets `cookie_ttl_seconds = 240`; when the published lifetime is at or above 240 the clamp binds and Arm B reproduces today's fixed rule exactly, so the two arms differ only by the derivation. When the published lifetime is below 240 the clamp cannot reproduce the old rule, and the comparison is the measured Arm A count against the deterministic fixed-timer count `floor(window / 240) + 1`. Record the captured attribute set per provider with the value redacted (attribute names and values only, for example `Max-Age=300`, `HttpOnly`), the arm counts, the total wall time, and the timestamps. If a pass cookie carries neither attribute, record that plainly: the measured effect for that provider is then zero and the milestone's benefit exists only for the provider that publishes a lifetime.

## Acceptance and stop conditions

- `[search.challenge] enabled = false`: byte-for-byte current behaviour, one request, `blocked`, no cache read or write.
- With derivation active and a published lifetime shorter than the ceiling, no expired cookie is presented: the request after expiry starts from a challenge page and solves once, instead of sending a dead token first.
- With a published lifetime at or above the ceiling, the reuse window equals `cookie_ttl_seconds`; the change is not a widening.
- `cookie_ttl_seconds = 0` stores nothing regardless of a published 3600 s lifetime, and every search re-solves.
- `Max-Age=0`, a past `Expires`, and a malformed attribute each leave the cache empty immediately, provable without sleeping.
- A rejected replay reports `blocked` with `pass cookie rejected`, leaks no value, and leaves no entry behind.
- `eggsearch doctor` warns exactly when a routed egress and challenge reuse are both configured.
- The live record contains both arms and at least one provider whose solve count fell, or an explicit statement that no provider publishes a lifetime and closure is conditional.
- Stop and return for a decision rather than widening scope if: neither upstream publishes `Max-Age` or `Expires` on its pass cookie; an upstream publishes a lifetime in a form this parser rejects; the work needs a new configuration key; or a change would alter `error_class`, `provider_status`, or the tool surface.

## Closure evidence

`plans/closure/agentic-retrieval-integrity/014-status.md` must record the implementation candidate SHA; the exact commands and observed results above including the canonical gate; the resolved-TTL table per provider from the live capture, with values redacted; both measurement arms with counts, wall time, window length, and timestamps; the disabled-path equivalence evidence; the egress decision as implemented (eviction on rejection, doctor warning text, both doc locations); the residual risks (system-clock steps, an egress exit that varies per connection, attribute drift, and a provider that publishes no lifetime); and the roadmap plus `plans/registry.md` row updates carried in the same commit. Anything measured as "no effect" is recorded as such, not omitted.
