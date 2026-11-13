# M028 — Anubis WASM `sha256` Method Coverage

Status: closed
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M028
Primary class: capability (method coverage)
Baseline: `fd2634e`
References: ADR-0009; ADR-0010; ADR-0013 (accepted); `src/meta/challenge/anubis.rs`; `src/meta/challenge/mod.rs`; `src/meta/engines/startpage.rs`; `src/mcp/tools/web_fetch.rs`; `architecture/engines.md` § Challenge solving; `docs/config.md` § Challenge Solving; `docs/provider-setup.md`.
Dependencies: hard ADR-0013 acceptance. M026 should be closed first so an unknown
`extensions` list cannot reach the new method. This plan must not begin until
ADR-0013 is accepted.

## Objective

Support the Anubis WASM `sha256` method, the cheapest member of the v1.28 WebAssembly
family. The proof is a SHA-256 nonce search, but the construction differs from `fast`:

- the data is the hex-decoded `randomData`, not the UTF-8 hex string;
- the nonce is a `u32` appended as four bytes;
- the four bytes are little-endian when the last data byte is at least 128, otherwise
  big-endian;
- the hash is `SHA-256(data || nonce_bytes)`;
- difficulty is the number of leading zero **bits**;
- the submission carries the decimal `nonce`, the hex `response`, `id`, `redir`, and
  `elapsedTime`.

The method is published through the same `#anubis_challenge` document, so parsing is
the existing shape plus a new method label. The plan does **not** support `argon2id`
or `hashx`; they remain `unsupported scheme` until a separate ADR and plan.

## Current implementation evidence

- `AnubisMethod` has `Fast`, `Preact`, and `Metarefresh`. The parser dispatches
  `METHOD_FAST`/`METHOD_SLOW`, `METHOD_PREACT`, and `METHOD_METAREFRESH`; every other
  method is `BOUND_UNSUPPORTED_SCHEME`.
- `expected_candidates(difficulty)` and `has_leading_zero_hex` are written for hex
  nibbles. They must not be reused for bit difficulty without a separate path.
- `SolveBudget` counts candidates, enforces `max_attempts`, a wall clock, and
  `max_difficulty`. The nonce search is single-threaded per ADR-0010.
- `sha2` is already a direct dependency. No WASM runtime or new crate is needed for
  this method.
- The live deployments observed on 2026-11-13 publish `fast` or `metarefresh`;
  no `sha256` deployment is known. A live capture is desirable but not a closure
  requirement; the closure record must say so explicitly.

## Non-goals

- No `argon2id`, no `hashx`, no executing the site's served `.wasm` module, no WASM
  runtime dependency.
- No parallel, batched, GPU, or distributed search; ADR-0010's single-threaded policy
  stands.
- No new configuration key. `max_difficulty` keeps its fast-equivalent nibble
  meaning; the bit mapping is documented.
- No new MCP tool, response field, or ledger key.
- No change to `fast`, `slow`, `preact`, `metarefresh`, ALTCHA, or the fetch path.
- No auto-detection of a method from the page beyond the published
  `challenge.method`/`rules.algorithm` values.

## Invariants that cannot regress

- The method label must be `sha256`; a page publishing `fast`, `slow`, `preact`, or
  `metarefresh` follows its existing path. `argon2id` and `hashx` still fail closed.
- `randomData` is valid hex and is bounded before decoding (at most 4 KiB decoded).
- The nonce is a `u32`; the endianness rule is exactly the last-data-byte test; no
  other byte order is tried.
- Difficulty is leading zero bits, compared on the raw 32-byte digest, not the hex
  string.
- The ceiling is `ceil(bits / 4) <= max_difficulty`; the projection is `2^bits`
  expected candidates and must be checked before hashing.
- Every candidate consumes one attempt; the wall clock and attempt ceilings stop the
  search; a nonce wrap or exhaustion is a named refusal, not a panic.
- The submission carries only `id`, `response`, `nonce`, `redir`, and `elapsedTime`;
  no cookie, header, or operator value is added.
- One submission per solve; the response body is bounded; the cookie handling stays
  M024/M025's.
- Ten tools, the refusal vocabulary, and `provider_status` shape are unchanged.

## Expected production changes

1. **Parser.** Add `METHOD_SHA256` and `AnubisMethod::WasmSha256`. Dispatch on the
   published method (prefer `challenge.method`, fall back to `rules.algorithm` when
   absent). Reuse the existing `id`, `randomData`, `redir`, `spent`, and `extensions`
   handling. Decode `randomData` to bytes at parse or solve time; invalid hex is
   `malformed parameters`.
2. **Bit difficulty helpers.** Add `leading_zero_bits(digest, bits) -> bool` and
   `expected_bit_candidates(bits) -> Option<u128>` (or a saturating count) that do not
   share the nibble helper. A bit count of 128 or more is refused as unprojectable,
   because `2^128` does not fit `u128`.
3. **Ceiling mapping.** `check_ceiling` for `WasmSha256` compares
   `bits.div_ceil(4)` with `max_difficulty`. Document that `max_difficulty = 4`
   admits a 16-bit challenge and rejects 17 bits; `max_difficulty = 6` admits 24 bits.
4. **Projection.** `check_projection` uses `2^bits` expected candidates and the same
   `CANDIDATE_NANOS` budget estimate as `fast`. Refuse before hashing when the
   projection exceeds `max_attempts` or the remaining wall clock.
5. **Solver.** Search `nonce: u32` from zero. Build the four nonce bytes with the
   endianness rule, hash `data || nonce_bytes`, check the leading zero bits, and
   return the hex digest and nonce. Increment the budget per candidate. A
   `u32::MAX` wrap is `attempts exhausted` (or a named exhaustion bound).
6. **Verification.** Submit `id`, `response`, `nonce`, `redir`, and `elapsedTime` to
   the existing pass-challenge URL and reuse the existing cookie handling.
7. **Tests.**
   - Known-answer vectors for both endianness branches: fixed data, nonce, expected
     digest, and a low difficulty that passes.
   - Bit counting: 1–32 bits across byte boundaries, including a digest with a
     partial final byte.
   - Ceiling mapping: 16 bits → 4 nibbles, 17 bits → 5, 24 bits → 6.
   - Projection: a high bit count refuses before hashing; a low count proceeds.
   - Invalid hex, empty data, and oversized data fail closed.
   - Nonce/attempt exhaustion is a named refusal; no panic.
   - Submission URL shape and a canary that no cookie appears in it.
   - Mock engine and fetch flows: a `sha256` challenge page solves and replays once.
   - `argon2id` and `hashx` remain `unsupported scheme`.
8. **Docs.** `architecture/engines.md` § Challenge solving (method and difficulty
   table), `docs/config.md` § Challenge Solving (`max_difficulty` is fast-equivalent),
   `docs/provider-setup.md` (Anubis method coverage), `CHANGELOG.md`.

## Ordered work packages

1. Parser and bit-difficulty helpers, with known-answer tests.
2. Ceiling mapping and projection, with refusal tests.
3. Solver and submission, with mock engine/fetch tests.
4. Documentation and the closure record, including the live-capture limitation.

## Verification

```bash
cargo fmt --all
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features --lib meta::challenge
cargo test --locked --all-features --lib meta::engines::startpage
cargo test --locked --all-features --test web_fetch_integration
cargo test --locked --all-features --test dispatch_fault_injection
cargo test --locked --all-features --test static_guards
cargo test --locked --all-features
```

Live, recorded rather than asserted: find a deployment publishing
`"algorithm":"sha256"` (v1.28+ with the method enabled) and solve it with
`[search.challenge]` enabled. Record the bit difficulty, candidate count, wall time,
and whether the document was returned. If none is reachable, record that the method is
synthetic-only; do not claim a live pass.

## Compatibility and closure evidence

No MCP tool, response-schema, or configuration-key change. `max_difficulty` keeps its
fast-equivalent nibble meaning, documented with the `ceil(bits/4)` mapping; a
deployment publishing `fast`, `slow`, `preact`, `metarefresh`, `argon2id`, or `hashx`
behaves exactly as today. `argon2id` and `hashx` remain refused. No new dependency is
introduced.

Required closure evidence: the implementation commit; the known-answer vectors for
both endianness branches; the bit-difficulty, ceiling, projection, and exhaustion
tests; the mock engine and fetch flows; a live `sha256` record if a deployment is
reachable, or the named synthetic-only limitation if it is not; the documentation
updates; and the residual risk if the live construction differs from the published
WASM source. The record must state the observed difficulty unit, not just that the
tests passed.

## Acceptance and stop conditions

- A `sha256` challenge with known-answer data solves and submits `nonce` and the hex
  `response`; the mock replay returns the document.
- Both endianness branches are covered by known-answer tests.
- The ceiling admits 16 bits at `max_difficulty = 4` and rejects 17 bits; the
  projection refuses an unaffordable bit count before hashing.
- `argon2id` and `hashx` still report `unsupported scheme`.
- No new dependency, key, tool, or response field.
- Stop and return for a decision if the live method's construction differs from the
  published `wasm/pow/sha256` source (for example a different nonce width, salt, or
  bit convention), if the bit-to-nibble ceiling is too lossy for operators, or if a
  live deployment requires `argon2id`/`hashx` to pass. Do not add those methods under
  this plan.
