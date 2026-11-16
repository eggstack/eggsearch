# M015 Challenge Solvability Observability — Closure Status

Status: closed

Source plan: `plans/archive/implementation/agentic-retrieval-integrity/015-challenge-solvability-observability.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Planning baseline: `dace41c3a75307ea35b745d695a11630ec6495c0`

Implementation candidate and closure commit: the commit that carries this record.

## Result

`src/meta/challenge/ledger.rs` adds a bounded, process-local solve ledger and `provider_status`
reports it on every call as a top-level `challenge_solver` object.

- `SolveLedger` owns `Mutex<BTreeMap<String, ProviderSolveStats>>`, a process-global
  `solve_ledger()`, `record_outcome`, `record_cookie_lookup`, and a serializable snapshot.
  `MAX_LEDGER_PROVIDERS = 4`, `MAX_LEDGER_BYTES = 4096`, `u64` saturating counters.
- `solves_attempted == solves_succeeded + refusals_total + solves_errored` holds by construction:
  one recording call increments the attempt and exactly one outcome under one lock.
- The refusal vocabulary is a fixed ten-key object, always emitted in full: `difficulty above
  ceiling`, `attempts exhausted`, `wall clock exhausted`, `unsupported scheme`,
  `malformed parameters`, `pass cookie absent`, `pass cookie rejected`, `challenge expired`,
  `already spent`, `unsafe base prefix`. The last two are now produced (M013); `challenge expired`
  remains a zero-only forward slot.
- One recording seam: `anubis::solve_and_verify` and `altcha::solve_and_verify` are thin wrappers
  over an inner terminal value; `finish_solve` records and maps. A dynamic `Blocked { reason, key }`
  keeps the exact ceiling/projection sentence while recording an attributable key.
- `challenge::lookup_pass_cookie` records one hit or miss and is called only inside the existing
  `if challenge.enabled` branch, so a disabled solver records nothing.
- `provider_status` always inserts `challenge_solver` (`enabled`, `untracked_providers`,
  `providers`), even when disabled; `src/mcp/output_schema.rs` gains exactly
  `"challenge_solver": {"type": "object"}` and the full advertisement pins move by that property.

## Live evidence (2026-11-13, release binary, one process, `mojeek`, `cookie_ttl_seconds = 0`)

Before any solve:

```json
{"enabled":true,"providers":{},"untracked_providers":0}
```

After `web_search {"query":"rust programming language","providers":["mojeek"]}` returned 3 rows:

```json
{"enabled":true,"providers":{"mojeek":{"cookie_hits":0,"cookie_misses":1,
 "last_candidates":244,"last_wall_clock_ms":476,
 "refusals":{"already spent":0,"attempts exhausted":0,"challenge expired":0,
  "difficulty above ceiling":0,"malformed parameters":0,"pass cookie absent":0,
  "pass cookie rejected":0,"unsafe base prefix":0,"unsupported scheme":0,
  "wall clock exhausted":0},
 "solves_attempted":1,"solves_errored":0,"solves_succeeded":1}},
 "untracked_providers":0}
```

The zero TTL produced one miss and one solve per request, as the plan predicts, and every value in
`providers` is numeric.

## Verification

```bash
cargo fmt --all -- --check                    # clean
cargo clippy --locked --all-targets --all-features -- -D warnings   # clean
cargo check --locked --no-default-features    # clean
cargo test --locked --all-features --lib meta::challenge            # 82 passed
cargo test --locked --all-features --test provider_routing          # 97 passed (block shape + canary)
cargo test --locked --all-features --test mcp_tool_contract         # 21 passed (re-pinned)
cargo test --locked --all-features --test mcp_schema_slimming       # 20 passed (74,487 total)
cargo test --locked --all-features                                 # 6,291 passed / 28 ignored
```

Ledger unit tests cover the arithmetic identity, per-bound distinguishability, the fixed ten-key
set, an exhaustive-over-`BOUND_*` source scan (adding a bound without a key fails), saturation, the
entry cap and `untracked_providers`, the 4,096-byte bound, and secret-free numeric serialization.
Engine flow tests cover solved, each reachable bound, errored, hit, miss, and zero TTL on both
engines.

## Deviations recorded

- The refusal key set is ten (not the plan's nine) because M013 landed `unsafe base prefix`.
- The projection refusal records `unsafe base prefix`; `Blocked` carries the key while keeping the
  dynamic sentence.
- `untracked_providers` counts dropped records, not distinct ids, to keep the ledger bounded.
- The exhaustive bound test is an `include_str!` scan in `ledger.rs` rather than a static guard.
- The `provider_routing` empty-map and exercised-shape assertions are one sequential test because
  the ledger is process-global and integration tests run in parallel.
- Advertisement pins move only for the full profile (+37 bytes, +9 estimated tokens); the compact
  profile strips `output_schema`.


Hosted CI: run `43472592990` on `0019f48` passed all three jobs — Ubuntu `ci` (12m11s),
Windows all-features check, and macOS canonical verification plus filesystem portability.

## Residual risk

- Counters are process-lifetime totals that reset on restart and are not rates; a long-running
  process aggregates every solve since start.
- `challenge expired` has no producer today; `already spent` and `unsafe base prefix` start moving
  only with M013, which is closed.
- A fifth provider id is not tracked and increments `untracked_providers` once per dropped record.
- The ledger is diagnostic only and never influences routing, health, cooldown, or classification.
