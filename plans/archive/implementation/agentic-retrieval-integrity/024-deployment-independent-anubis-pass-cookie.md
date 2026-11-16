# M024 — Deployment-Independent Anubis Pass-Cookie Replay

Status: closed
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M024
Primary class: invariant (corrective to M011 and M023)
Baseline: `fd2634e`
References: ADR-0009; ADR-0010; `src/meta/challenge/cookie.rs`; `src/meta/challenge/anubis.rs`; `src/meta/challenge/mod.rs`; `src/meta/engines/startpage.rs`; `src/mcp/tools/web_fetch.rs`; `architecture/engines.md` § Challenge solving; `architecture/fetch.md` § Challenge pages on the HTTP path; `docs/config.md` § Challenge Solving.
Dependencies: none hard. M011, M013, and M023 are closed; M014 is conditionally
closed, and this plan reuses its existing `PassCookieCache` interface without
depending on the unrun two-arm solve-count comparison that left M014 conditional.

## Objective

Stop identifying the Anubis clearance cookie by a hardcoded name. The name is a
deployment setting, not a protocol constant, and the current code therefore fails on
deployments that work correctly.

Anubis derives cookie names in `cmd/anubis/main.go`:

```text
CookieName         = --cookie-prefix + "-auth"
TestCookieName     = --cookie-prefix + "-cookie-verification"
--cookie-prefix    = "techaro.lol-anubis" by default
--dynamic-cookie-suffix = true by default
```

The dynamic suffix appends a short hash of the cookie settings to every browser
cookie. Three live deployments observed on 2026-11-13 use three different names:

| Deployment | Version | Clearance cookie | Verification cookie |
|---|---|---|---|
| startpage.com | v1.26.4 (rebranded) | `spchal-auth` | `spchal-cookie-verification` |
| lore.kernel.org | v1.26.2 | `techaro.lol-anubis-auth` | `techaro.lol-anubis-cookie-verification` |
| anubis.techaro.lol | v1.28.3 | `techaro.lol-anubis-auth-<suffix>` (configured; verification observed as `…-347ddb4a`) | `techaro.lol-anubis-cookie-verification-347ddb4a` |

The current solver parses `spchal-auth` only:

- `PASS_COOKIE_NAME = "spchal-auth"` in `src/meta/challenge/anubis.rs`.
- `cookie::parse_issued(resp.headers(), PASS_COOKIE_NAME)` in the Anubis `verify`
  step.
- `format!("{}={cookie}", anubis::PASS_COOKIE_NAME)` in
  `src/meta/engines/startpage.rs` and in `src/mcp/tools/web_fetch.rs`.

A valid solve against a default Anubis deployment therefore yields
`pass cookie rejected`, because the pass-challenge response is a 302 carrying a name
the solver never checks. If the status were a 200 with no recognised pair, the same
hole would record `pass cookie absent`. If the name changes by a suffix rotation, the
cached cookie is evicted and the next search re-solves for no reason. `web_fetch` on
such a deployment returns the wall even though the proof was accepted.

The fix is to stop naming the cookie: take every non-deletion cookie pair the
pass-challenge response set, replay those pairs verbatim, and derive the reuse window
from the longest published lifetime among them. The cookie becomes opaque data, like
the challenge ID.

**Why the original verification missed it.** M011 and M023 were exercised against
Startpage, whose configured prefix is `spchal`, so the hardcoded name matched every
live and mock case. No default-Anubis deployment was in the test set, and the unit
fixtures reused the same name, so the defect could not surface. The regression test is
a pass response that sets a different name, not a stronger assertion on the Startpage
case.

## Current implementation evidence

- `PublishedLifetime`, `parse_issued(headers, name)`, and `reuse_ttl` live in
  `src/meta/challenge/cookie.rs`. The parser returns the value only; the name is
  supplied by the caller.
- `SolveOutcome::Solved { cookie, lifetime }` documents `cookie` as a pass-cookie
  *value*, and `PassCookieCache` stores that value per provider.
- `src/meta/engines/startpage.rs` builds the replay header from
  `anubis::PASS_COOKIE_NAME` and stores the bare value in the cache.
- `src/meta/challenge/altcha.rs` also uses `cookie::parse_issued(headers, COOKIE_NAME)`
  with the published `chllg` name; `src/meta/engines/mojeek.rs` builds
  `chllg=<value>` at replay. ALTCHA must keep working.
- The fetch replay in `src/mcp/tools/web_fetch.rs` already captures every
  `Set-Cookie` pair from the challenge response and sends them to `pass-challenge`;
  only the *response* cookie is named.

## Non-goals

- No change to which challenge methods are solved, and no method coverage change.
- No new configuration key and no cookie-name knob. A knob would preserve the same
  class of bug and put deployment detail on the operator.
- No change to the `[search.challenge]` opt-in, the four ceilings, or the refusal
  vocabulary.
- No cross-call persistence on the fetch path: M023's one-solve, one-replay rule
  stays; the fetch response cookie remains request-local.
- No change to ALTCHA's `chllg` scheme beyond expressing its issued cookie as a
  replayable pair.
- No user-agent, TLS, proxy, or browser change.

## Invariants that cannot regress

- `src/fetch/**` stays independent of `src/meta/**`; the fetch orchestration remains
  in `src/mcp/tools/web_fetch.rs`.
- Only cookie pairs from the pass-challenge response are replayed. Challenge-page
  cookies are forwarded only to the pass request, never to the final replay.
- Empty values and deletion cookies (`Max-Age=0`, or `Expires` in the past) are never
  replayed and never extend the cache TTL.
- Duplicate names are resolved deterministically (last non-deletion pair wins) and
  the joined header is bounded.
- The reuse window is the longest usable published lifetime among the replayed pairs,
  clamped by `cookie_ttl_seconds`; `Unstated` falls back to the ceiling; a malformed
  lifetime stores nothing.
- One solve and one replay per fetch; at most one solve per provider per TTL on the
  engine path.
- Cookie values, JWT segments, challenge IDs, and nonces never appear in logs,
  errors, warnings, `Debug`, fixtures, or the cache's `Debug`.
- Ten MCP tools, no argument or schema change, no `provider_status` shape change; the
  ten-key refusal vocabulary is unchanged.
- Disabled, refused, above-ceiling, malformed, or still-challenged keeps the current
  marked response.

## Expected production changes

1. **Pair parser.** Add a name-independent parser in `src/meta/challenge/cookie.rs`
   that walks every `Set-Cookie` header, keeps the `name=value` pair when the value is
   non-empty and the cookie is not a deletion, deduplicates by name, joins the pairs
   with `; `, and returns the joined header plus the published lifetimes. Keep
   `parse_issued(headers, name)` unchanged for ALTCHA: its `Max-Age=0` case expects a
   solved outcome with a zero reuse TTL and one replay, not an absent cookie, so ALTCHA
   must not be routed through the deletion-filtering pair parser.
2. **Replayable outcome.** Document `SolveOutcome::Solved.cookie` as a replayable
   `Cookie` header value (`name=value` pairs joined with `; `), not a bare value.
   Update the `Debug` redaction so the whole header stays redacted.
3. **Cache storage.** `PassCookieCache` stores the replayable header string for both
   schemes. ALTCHA's solver builds `chllg=<published value>` from its known name at
   issue time; Anubis stores the joined pass-response pairs. Replay sites send the
   cached string directly, with no name formatting.
4. **Anubis solver.** Replace the named `parse_issued` call in the Anubis `verify`
   step with the pair parser. Resolve the reuse TTL as the maximum `reuse_ttl` across
   the returned lifetimes, clamped by `cookie_ttl_seconds`.
5. **Engine replay.** `src/meta/engines/startpage.rs` sends the cached header value
   directly. The existing cache eviction on a challenged replay is unchanged.
6. **Fetch replay.** `src/mcp/tools/web_fetch.rs` passes the issued pair header to
   `fetch_with_cookie` unchanged. The first-hop-only rule and the one-replay rule are
   unchanged.
7. **Tests.**
   - Parser: Startpage names, default Anubis names, a dynamic suffix, a short-lived
     referer cookie, a deletion cookie, duplicate names, malformed attributes,
     empty-only and deletion-only responses.
   - Solver: a mock pass response that sets `techaro.lol-anubis-auth` solves and
     returns that pair; a dynamic-suffix name does the same.
   - Engine: a default-name pass response replays once and caches the pair; a rejected
     replay evicts it.
   - Fetch: a default-name pass response replays once and returns the document; a
     canary asserts the cookie value appears nowhere in the payload or warnings.
   - ALTCHA: the `chllg` solve, cache, replay, and rejection behaviour is unchanged;
     the tests that assert the cached string are updated to the pair form
     (`chllg=<value>`), including the `Max-Age=0` case that still performs one replay
     and stores no entry.
8. **Docs.** `architecture/engines.md` § Challenge solving (cookie identity is
   opaque), `architecture/fetch.md` § Challenge pages on the HTTP path,
   `docs/config.md` § Challenge Solving, and `CHANGELOG.md`.

## Ordered work packages

1. Pair parser and its unit tests in `src/meta/challenge/cookie.rs`.
2. `SolveOutcome` and cache semantics, plus the ALTCHA conversion and tests.
3. Anubis solver, Startpage engine, and `web_fetch` replay, with mock and canary tests.
4. Documentation and the closure record.

## Verification

```bash
cargo fmt --all
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features --lib meta::challenge
cargo test --locked --all-features --lib meta::engines::startpage
cargo test --locked --all-features --lib meta::engines::mojeek
cargo test --locked --all-features --test web_fetch_integration
cargo test --locked --all-features --test mcp_tools
cargo test --locked --all-features --test static_guards
cargo test --locked --all-features
```

Live, recorded rather than asserted: `web_fetch` against a Startpage query with
challenge solving enabled still returns the SERP; a second fetch against a
default-Anubis deployment (for example `https://lore.kernel.org/`) returns the page
or records the exact refusal. Record the cookie name observed in the pass response,
the wall time, and whether the returned body is the document or the wall.

## Compatibility and closure evidence

No MCP tool, response-schema, or configuration-key change. The cache stores a
replayable header string instead of a bare value; ALTCHA's producer and consumer are
updated together, so there is no mixed-format state to migrate. A deployment-specific
name is data, not a contract, so no operator migration is required.

Required closure evidence: the implementation commit; the focused and full test
outputs; a live record for Startpage and for one default-Anubis deployment, or the
exact refusal if none is reachable; the documentation updates; and the residual risk
if no default-Anubis deployment can be exercised. The record must state which cookie
names were observed, not just that the tests passed.

## Acceptance and stop conditions

- A pass response setting `spchal-auth` still yields a solved outcome and a replay
  carrying that pair.
- A pass response setting `techaro.lol-anubis-auth` yields a solved outcome and a
  replay carrying that pair, with no source change per deployment.
- A dynamic-suffix name (`...-auth-<hash>`) works the same way.
- A pass response that sets only a deletion cookie yields a named refusal
  (`pass cookie absent` on a 200, `pass cookie rejected` on a redirect) and no replay
  is sent.
- The cache TTL follows the longest non-deletion lifetime and is clamped by
  `cookie_ttl_seconds`; a malformed lifetime stores nothing.
- No runtime replay path formats a fixed cookie name.
- ALTCHA's `chllg` behaviour is unchanged, including the one-replay, no-cache
  `Max-Age=0` case; only the cached-string assertions move to the pair form.
- Stop and return for a decision if the pass response cannot be distinguished from
  unrelated cookies without a name (for example several JWT-shaped pairs with
  different lifetimes), or if unifying the cache would break ALTCHA's published-name
  contract.
