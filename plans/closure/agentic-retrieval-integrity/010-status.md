# M010 Bounded Challenge-Solving Framework and ALTCHA — Closure Status

Status: closed

Source plan: `plans/archive/implementation/agentic-retrieval-integrity/010-challenge-solving-framework-and-altcha.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Policy: `plans/adrs/ADR-0009-bounded-challenge-solving.md`

Planning baseline: `d751bd7471a647e651ee41b8e51199c66d04d01a`

Implementation candidate and closure commit: the commit that carries this record. Implementation,
closure evidence, roadmap status, and the registry row all moved in that single commit on top of the
planning baseline above.

## Result

One shared, bounded, opt-in solver plus ALTCHA, restoring `mojeek` as a working keyless upstream.

`src/meta/challenge/mod.rs` owns the shared framework:

```rust
SolveBudget { max_attempts, deadline, max_difficulty }   // attempts + wall clock + difficulty
SolveOutcome                                             // Solved { cookie } | Refused { bound }
PassCookieCache                                          // provider-keyed, TTL, entry cap
pass_cookie_cache()                                      // process-wide accessor
```

`SolveOutcome` carries a redacting `Debug`, so a cookie value cannot reach a log line through
`{:?}`. Refusal reasons use the `BOUND_*` constants (`attempts exhausted`, `wall clock exhausted`,
`difficulty above ceiling`, `unsupported scheme`, `malformed parameters`) and every one of them is
safe to place in an `EngineError::Blocked` reason.

`src/meta/challenge/altcha.rs` implements the published scheme:

- decode `nonce`, `salt`, `keyPrefix` from hex;
- for `counter` from zero, derive `PBKDF2-HMAC-SHA256(password = nonce ‖ counter_be32, salt,
  iterations = cost, dklen = keyLength)` and stop when the derived key bytes start with the full
  `keyPrefix`;
- submit `base64(json({"challenge": …, "solution": {"counter", "derivedKey", "time": 150}}))` as
  the multipart field `altcha` to `POST /captcha/verify`, with a fixed boundary and an explicit
  `Content-Type`;
- capture `chllg` from `Set-Cookie` into the process-local cache.

`time` is the fixed literal `150`, not a random value, so a recorded challenge always produces the
same answer.

`src/meta/engines/mojeek.rs` detects the challenge before parsing, solves once when enabled, caches
the pass cookie, replays the search request exactly once with it, then parses. The disabled path
makes no extra request and attaches no cookie.

## Configuration

`[search.challenge]` is a serde-defaulted table, so a config file without it behaves exactly as
before ADR-0009. It lives in `src/core/config/challenge.rs` because `src/core/config.rs` is under a
1,900-line ratchet.

| Field | Default | Bounds |
|---|---:|---|
| `enabled` | `false` | — |
| `max_difficulty` | `5` | `1..=10` |
| `max_attempts` | `2_000_000` | `1..=100_000_000` |
| `timeout_ms` | `2_000` | `100..=30_000` |
| `cookie_ttl_seconds` | `240` | `0..=3_600` (`0` disables reuse) |

Three default sites agree: `ChallengeSection::default()`, the serde attribute defaults, and the
validator.

## Field evidence (2026-11-11 UTC)

A fresh live `GET https://www.mojeek.com/captcha/challenge` published
`algorithm = PBKDF2/SHA-256`, `cost = 8000`, `keyLength = 32`, and a 16-byte `keyPrefix`. The
reference algorithm resolved it at **counter 248** in **0.34 s** outside this repository.

That measurement corrects an assumption worth recording: the search space is the **counter range**,
not the prefix length. A 128-bit prefix cannot be brute-forced; the server picks a small counter and
publishes the derived key's prefix, so a long prefix costs a solver nothing and buys collision
resistance instead. An earlier note in `architecture/engines.md` recorded counter 292 for a different
captured challenge; the corrected figure for that capture is 291.

## Live verification (recorded, not asserted)

Release build, through the shipped client:

| Configuration | Observation |
|---|---|
| `enabled = false` | 0 results, `error_class: blocked`, one HTTP request, no cookie |
| `enabled = true`, `max_difficulty = 5`, `timeout_ms = 12000` | 5/5 parsed rows, no provider failures, 3.56 s wall clock |

A **debug** build does not complete the same solve inside a 12-second budget and reports
`challenge not solved: wall clock exhausted`. Unoptimised `sha2`/`pbkdf2` derivation is roughly two
orders of magnitude slower. The classification is correct in both builds; debug builds are simply
not a valid performance baseline for this capability. This is recorded in
`tests/fixtures/engines/README.md` so it is not rediscovered as a defect.

## Verification

- `cargo fmt --check`: clean.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: clean.
- `cargo check --locked --no-default-features`: clean.
- `cargo test --locked --all-features --lib meta::challenge`: 18 passed.
- `cargo test --locked --all-features --lib meta::engines::mojeek`: 13 passed.
- `cargo test --locked --all-features --test web_search_integration`: 72 passed.
- `cargo test --locked --all-features --test static_guards`: 61 passed.
- Full `cargo test --locked --all-features`: 5,955 passed, 0 failed, taken from an isolated worktree
  carrying only this milestone's changes, because the shared tree was being edited concurrently.

## Deviations

- **`src/core/config/challenge.rs` is a new file.** The plan assumed the table would live in
  `src/core/config.rs`; the 1,900-line ratchet made that impossible without concatenation. The
  split is the ratchet's intended remedy.
- **The eggfetch `multipart` feature was NOT enabled.** `static_guards::eggfetch_feature_budget_stays_bounded`
  forbids it. The verify POST hand-builds a `multipart/form-data` body with the fixed boundary
  `eggsearch-altcha-boundary` instead, which is also what makes the mock flow test byte-exact.
- **`src/mcp/state.rs` gained a one-line call-site update** to pass `&config.search.challenge` into
  the adapter constructor. `build_default_engines` keeps its previous signature and delegates with
  `ChallengeSection::default()`, so no existing caller changed.

## Residual risk

- Upstream parameter drift: `cost`, `keyLength`, or a scheme other than `PBKDF2/SHA-256` would fail
  closed as `unsupported scheme`, not silently mis-solve.
- Divergence from SearXNG: SearXNG passes a random `solution.time`; this implementation fixes it at
  `150` for determinism. If Mojeek ever verifies the elapsed time, that choice becomes a defect.
- Mojeek's own terms of service are the operator's responsibility; the capability is off by default
  and bounded by four ceilings.
- Debug builds cannot complete a full-cost solve inside the default budget. Tests use synthetic
  low-cost vectors so the suite stays fast; they do not measure live cost.
