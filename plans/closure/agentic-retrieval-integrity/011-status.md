# M011 Anubis Solver and the Operator Difficulty Ceiling — Closure Status

Status: closed

Source plan: `plans/archive/implementation/agentic-retrieval-integrity/011-anubis-solver-and-difficulty-ceiling.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Policy: `plans/adrs/ADR-0009-bounded-challenge-solving.md`

Planning baseline: `d751bd7471a647e651ee41b8e51199c66d04d01a`

Implementation candidate and closure commit: the commit that carries this record. Implementation,
closure evidence, roadmap status, and the registry row all moved in that single commit on top of the
planning baseline above.

## Result

`src/meta/challenge/anubis.rs` adds both published Anubis methods to the M010 framework, and the
difficulty ceiling becomes an explicit operator setting rather than a hardcoded engine constant.

```rust
AnubisMethod { Fast, Preact }
AnubisSolution { response, nonce }        // redacting Debug
AnubisChallenge::{parse, method, difficulty, check_ceiling, solve}
ceiling_refusal(published: u8, ceiling: u8) -> String
challenge_cookies(&HeaderMap) -> String
solve_and_verify(client, engine, base_url, challenge_page, challenge_cookies, timeout, budget)
```

Message dispatch is on `challenge.method`; any other value is refused as an unsupported scheme
rather than guessed.

## Ceiling semantics

The published `rules.difficulty` is compared **before any hashing work**. Above the ceiling the
engine returns `EngineError::Blocked` whose reason names both values verbatim:

```
anubis difficulty 6 exceeds configured ceiling 5
```

That string is the operator-visible knob. With the shipped `max_difficulty = 5`, a difficulty-6
wall is refused for this reason and `startpage` stays `blocked`; raising the ceiling is the supported
way to opt into the cost.

M010's `SolveOutcome::Refused { bound: &'static str }` cannot carry a dynamic sentence, so this
milestone returns `EngineError::Blocked` directly for the ceiling case instead of widening the
shared enum. All other refusals keep the M010 shape: `challenge not solved: <bound>`.

## Measured cost

`16^difficulty` expected SHA-256 candidates. Measured on one core with a release build:

| Difficulty | Candidates | Wall clock |
|---|---:|---:|
| 6 | 23,070,878 | 3.05 s |

That is roughly 0.13 µs per candidate — about three times faster than the earlier estimate of 0.4 µs,
which came from a Python harness. The consequence is that a difficulty-6 solve fits an 8-second
budget **without** the optional bounded parallelism work package; parallelism was therefore not
implemented. Total attempts remain bounded by `SolveBudget`, and a test asserts
`attempts_used() <= max_attempts()`.

## Corrected opt-in recipe

The plan's acceptance used `enabled = true`, `max_difficulty = 6`, `timeout_ms = 8000`. That recipe
is **insufficient** and the documentation was corrected:

- the shipped `max_attempts` default of 2,000,000 matches the shipped difficulty ceiling of 5, and
  23.07M candidates exceed it — the attempt ceiling binds before the wall clock;
- `[search].timeout_ms` must also be raised, because the response deadline wins over the solve
  budget.

Working opt-in: `enabled = true`, `max_difficulty = 6`, `max_attempts = 32000000`,
`[search.challenge].timeout_ms = 8000`, `[search].timeout_ms = 8000`.

This is recorded in `docs/provider-setup.md`, `docs/config.md`, and `architecture/engines.md`
alongside the measured cost curve.

## Cookie handling

Verified against the live wall on 2026-11-11: the Anubis challenge response sets exactly two cookies,
and `pass-challenge` requires both.

```
set-cookie: spchal-cookie-verification=<uuid>; Path=/; Domain=startpage.com; Secure; SameSite=None
set-cookie: sp_pow=<token>; Max-Age=300; HttpOnly; Path=/
```

The solver captures every `name=value` pair from the challenge response, replaying them joined with
`; ` in one `Cookie` header, and sends them to
`GET /.within.website/x/cmd/anubis/api/pass-challenge` with redirect following disabled and a
bounded body read. The returned `spchal-auth` value is stored in `pass_cookie_cache()` under
`startpage`, honouring `cookie_ttl_seconds`. No cookie jar is enabled; the module-level note in
`src/meta/engines/mod.rs` that this client has no cookie store remains true.

The `preact` path computes `sha256(challenge)`, honours the server minimum elapsed time
(`0.08 × difficulty`) bounded by `timeout_ms`, and submits `GET {base}{redir}&result=<hex>`.

## Defect found after landing, fixed here

The first implementation required `challenge.redir` as a mandatory payload field. The live Anubis
`fast` challenge publishes **no `redir` at all**:

```json
{"rules":{"algorithm":"fast","difficulty":6},"challenge":{"issuedAt":"…","id":"…","method":"fast","randomData":"…","policyRuleHash":"…","difficulty":6,"spent":false}}
```

So the shipped solver refused every real Startpage challenge as `malformed parameters` — a defect
that only a live capture could reveal. The correct behaviour is what SearXNG does: the redirect
target is the original request URL.

Fixed by making `redir` optional and resolving it from the request URL
(`AnubisChallenge::resolved_redir`), threading the real request URL
(`https://www.startpage.com/search?q=…`) through `solve_and_verify`. A published `redir` still wins
when present. Two regression tests pin both behaviours, one of them parsing the exact live payload
shape above.

## Live verification after the fix (2026-11-12 UTC)

Release build, through the shipped client:

| Configuration | Observation |
|---|---|
| `enabled = false` | 0 results, `blocked`, `challenge page detected (#anubis_challenge)` |
| `enabled = true`, `max_difficulty = 5` | 0 results, `blocked`, `anubis difficulty 6 exceeds configured ceiling 5` |
| `enabled = true`, `max_difficulty = 6`, `max_attempts = 32000000` | 5/5 rows, 3.88 s including the solve |

The middle row is the operator knob working exactly as specified; the last row is the capability
working end to end.

## Residual risk

## Verification

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: clean.
- `cargo check --locked --no-default-features`: succeeded (one dead-code warning in a concurrently
  edited `src/meta/routing.rs`, not this milestone).
- `cargo test --locked --all-features --lib meta::challenge`: 35 passed (17 new Anubis tests).
- `cargo test --locked --all-features --lib meta::engines::startpage`: 12 passed (7 new).
- `cargo test --locked --all-features --test engine_parse_conformance`: 6 passed.
- Full `cargo test --locked --all-features`: 6,075 passed, 2 failed, 27 ignored at the time of the
  run; both failures were concurrent-work items outside this milestone (an `mcp::advertisement`
  count assertion and a docs phantom-name guard), and both were fixed and re-verified afterwards.

## Residual risk

- Anubis difficulty is risk-scored per client. The honest `eggsearch` user agent received 6 while a
  browser user agent received 4; a future escalation to 7 or 8 would cost roughly 16× and 256× more
  and would be refused unless the operator raises both the ceiling and the attempt cap well past the
  validated maximum of 100,000,000.
- Unknown future methods fail closed, which is correct but means a method change silently costs the
  provider until a new solver lands.
- A solved Startpage page parses through M012's payload extractor; if the payload shape changes the provider degrades to `parse_error`, which is the intended honest outcome.
