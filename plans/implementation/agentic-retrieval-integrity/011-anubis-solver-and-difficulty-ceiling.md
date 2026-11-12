# M011 — Anubis Solver and the Operator-Set Difficulty Ceiling

Status: ready
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M011
Primary class: capability
Baseline: `d751bd7471a647e651ee41b8e51199c66d04d01a` (re-read `git status` before starting)
References: ADR-0009; `architecture/engines.md` § Challenge solving; `src/meta/challenge/` (from M010); `src/meta/engines/startpage.rs`; `src/core/config.rs`; `src/meta/adapter/error.rs`.
Dependencies: hard M010 (shared framework, budget type, pass-cookie cache). Interface to M012, which consumes the pass cookie this milestone produces.

## Objective

Add the Anubis proof-of-work solvers (`fast` and `preact`) to the shared challenge framework, and make the difficulty ceiling an operator setting so a deployment can choose to pay the cost for a harder challenge instead of losing the provider.

## Non-goals

- No Startpage result extraction work; the current `div.result` parser is handled in M012.
- No user-agent spoofing to obtain an easier challenge class, no proxy rotation, no browser delegation.
- No parity claim with a specific SearXNG default: SearXNG hard-caps Startpage at `max_difficulty = 4`, which refuses the observed difficulty-6 wall. This milestone deliberately exposes the ceiling instead.
- No new scheme beyond Anubis `fast` and `preact`. An unknown `challenge.method` fails closed.
- No `provider_status` schema change.

## Invariants that cannot regress

- The disabled path and every ceiling breach still classify as `blocked`, never `empty` and never `parse_error`.
- The difficulty ceiling is compared against the value the server publishes for *this* request; a missing or non-numeric `rules.difficulty` fails closed.
- Solving is one attempt per provider per request and one per `cookie_ttl_seconds`; a solve is never a retry loop.
- The response deadline wins over the solve budget. A solve that cannot finish inside the deadline is abandoned and reported.
- Pass cookies (`spchal-auth`) are treated as secrets: memory only, never logged, never serialized into fixtures or diagnostics.
- The solver honours the honest configured user agent. It must not alter the UA, add evasion headers, or rotate egress.

## Expected production changes

1. **Anubis solver.** New `src/meta/challenge/anubis.rs`:
   - `fast`: read `script#anubis_challenge`, parse `rules.difficulty` and `challenge.{id,randomData,method,redir}`, search `nonce` from zero for `sha256(randomData ‖ decimal(nonce))` with `difficulty` leading hex zeros, then `GET {base}/.within.website/x/cmd/anubis/api/pass-challenge?id=…&response=<digest>&nonce=<n>&redir=<url>&elapsedTime=1` with the challenge cookies, and capture the returned `spchal-auth` cookie.
   - `preact`: read `script#preact_info`, compute `sha256(challenge)`, honour the server's minimum elapsed time (bounded by `timeout_ms`), and `GET {base}{redir}&result=<hex>`, capturing cookies.
   - Dispatch on `challenge.method`; anything else is refused as unsupported.
2. **Ceiling semantics.** `[search.challenge].max_difficulty` is compared before any work starts. Above the ceiling the engine returns `Blocked` with a reason that names both values, e.g. `anubis difficulty 6 exceeds configured ceiling 5`. This is the operator-visible knob; raising it is the supported way to opt into a costlier solve.
3. **Optional bounded parallelism.** The nonce space is embarrassingly parallel. `std::thread::scope` over `available_parallelism`, capped by configuration, may be used to meet the wall clock; it must still respect `max_attempts` in total and release every thread before returning. If it is not implemented, a difficulty-6 solve takes the measured single-core time (~6.7 s) and must fit `timeout_ms`.
4. **Startpage integration.** `src/meta/engines/startpage.rs` obtains a pass cookie through the framework when a challenge is detected and solving is enabled, replays it once, and otherwise keeps today's `blocked` behaviour.
5. **Documentation of the cost curve.** `docs/provider-setup.md` and `docs/config.md` state the measured curve — difficulty 4 ≈ 65.5K candidates ≈ 0.05 s, 5 ≈ 1.05M ≈ 0.4 s, 6 ≈ 16.8M ≈ 6.7 s single-core — and that the honest `eggsearch` user agent receives a harder class than a browser user agent, so the ceiling must be chosen for the honest agent.

## Storage, protocol, and compatibility effects

Additive configuration only; the M010 table gains no new required key. No wire-string or schema change. `max_difficulty` is validated `1..=10`; a value the upstream never publishes simply fails closed at compare time. The solve runs inside the existing per-request timeout and adds no new network destination beyond the upstream's own challenge endpoint.

## Ordered work packages

1. `anubis.rs` `fast` solve with deterministic fixtures: a synthetic challenge at difficulty 1–2 with a known nonce, plus a negative test whose nonce must not be accepted.
2. `pass-challenge` request and cookie capture, including the redirect-suppression behaviour the endpoint requires and a bounded body read.
3. `preact` solve with its minimum-elapsed-time honour and a test that the wait is bounded by `timeout_ms`.
4. Ceiling comparison and the exact refusal reason, with tests at, below, and above the ceiling.
5. Method dispatch fail-closed test for an unknown `challenge.method`.
6. Optional bounded parallelism behind a small internal helper, with a test that total attempts never exceed `max_attempts` and that threads join.
7. `startpage.rs` wiring: detect → solve when enabled → replay once → parse; disabled and above-ceiling paths unchanged.
8. Docs and changelog.

## Verification

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features --test engine_parse_conformance
cargo test --locked --all-features --test static_guards
cargo test --locked --all-features
```

Live, opt-in, recorded:

```bash
# ceiling below the published difficulty: must stay blocked and name the ceiling
eggsearch --config /tmp/anubis-5.toml search "rust programming language" --providers startpage --json
# ceiling raised for the cost the operator accepts
eggsearch --config /tmp/anubis-6.toml search "rust programming language" --providers startpage --json
```

## Acceptance and stop conditions

- Default configuration (`enabled = false` or `max_difficulty = 5`): Startpage remains `blocked`, with a reason naming the ceiling when the challenge is difficulty 6.
- `enabled = true`, `max_difficulty = 6`, `timeout_ms = 8000`: the solve completes, `spchal-auth` is captured and cached, and the repeat request is not challenged again within the TTL.
- The solve never exceeds `max_attempts`, `timeout_ms`, or the search deadline, and a breach is reported as `blocked`.
- No UA change, no proxy, no additional egress, and no cookie leakage.
- Stop and return for a decision if the upstream switches to a method other than `fast`/`preact`, or if the published difficulty exceeds the configured validation maximum.

## Closure evidence

`plans/closure/agentic-retrieval-integrity/011-status.md` must record: candidate SHA, the solved-difficulty and wall-clock measurements per ceiling, the above-ceiling refusal evidence, cookie-reuse evidence within and after the TTL, the unchanged disabled-path result, and residual risk (unknown future methods, difficulty escalation, and the fact that a solved Startpage page still may not parse until M012 lands).
