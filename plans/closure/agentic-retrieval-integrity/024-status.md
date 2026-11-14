# M024 Deployment-Independent Anubis Pass-Cookie Replay — Closure Status

Status: closed

Source plan: `plans/archive/implementation/agentic-retrieval-integrity/024-deployment-independent-anubis-pass-cookie.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Planning baseline: `fd2634e`

Implementation candidate and closure commit: the commit that carries this record.

## Result

The Anubis solver no longer selects the clearance cookie by a fixed name. The pass-challenge
response is reduced to every replayable `name=value` pair, joined with `; `, and replayed verbatim;
the reuse window is the longest usable published lifetime among the pairs, clamped by
`cookie_ttl_seconds`.

- `src/meta/challenge/cookie.rs`: new `parse_issued_pairs(&HeaderMap) -> IssuedCookies`, with
  `IssuedCookies::{header, lifetimes, reuse_lifetime, is_empty, len}` and
  `MAX_ISSUED_HEADER_BYTES = 4096`. Empty values and deletion cookies (`Max-Age=0`, past
  `Expires`) are dropped; duplicate names keep the first position and the last non-deletion value;
  the joined header keeps the longest fitting prefix. `parse_issued(headers, name)` is unchanged so
  ALTCHA's `Max-Age=0` one-replay/no-cache semantics survive.
- `src/meta/challenge/mod.rs`: `SolveOutcome::Solved.cookie` and `PassCookieCache` are documented as
  a replayable `Cookie` header value. `Debug` still redacts the whole value.
- `src/meta/challenge/anubis.rs`: `PASS_COOKIE_NAME` is deleted. `verify` parses the pair set,
  returns `issued.header()`, and carries `issued.reuse_lifetime(SystemTime::now())`.
- `src/meta/challenge/altcha.rs`: emits `chllg=<published value>` at issue time, so Mojeek's
  producer and consumer stay aligned. `Max-Age=0` still performs one replay and stores nothing.
- `src/meta/engines/startpage.rs`, `src/meta/engines/mojeek.rs`,
  `src/mcp/tools/web_fetch.rs`: replay the cached or issued header string directly; no runtime path
  formats a cookie name. A Startpage pass response that sets only a deletion cookie now refuses
  (`pass cookie rejected`, zero replay) because Anubis filtering drops it; ALTCHA keeps its
  documented exception.

Regression evidence: a pass response setting `spchal-auth`, `techaro.lol-anubis-auth`, or a
dynamic-suffix `...-auth-<hash>` all solve and replay with no source change; deletion-only responses
refuse; the joined header and lifetimes follow the parser contract.

## Live evidence (2026-11-13 UTC, protocol level)

`https://anubis.techaro.lol/docs/design/how-anubis-works/` (v1.28.3) served:

```text
set-cookie: techaro.lol-anubis-cookie-verification-347ddb4a=<uuid>; Path=/; Expires=...; Secure; SameSite=None; Partitioned
```

A metarefresh proof submitted with that cookie returned `302 location: /` and:

```text
set-cookie: techaro.lol-anubis-auth-347ddb4a=<JWT>; Path=/; Expires=...; Secure; SameSite=None; Partitioned
```

Neither name is a fixed constant; the pair parser replays both without a deployment-specific
configuration. No end-to-end `web_fetch` run was performed in this environment, so the record is
protocol-level plus synthetic, not a recorded tool-level live pass. `https://lore.kernel.org/` did
not challenge the honest `eggsearch/0.4.4` user agent on this run and yielded no cookie evidence.

## Verification

```bash
cargo test --locked --all-features --lib meta::challenge   # 118 passed
cargo test --locked --all-features --lib meta::engines::   # 714 passed
cargo test --locked --all-features --test web_fetch_integration  # 96 passed, 1 ignored; includes the default-name replay and canary tests
```
cargo test --locked --all-features --no-fail-fast   # 6306 passed, 0 failed, 28 ignored (85 test binaries)
