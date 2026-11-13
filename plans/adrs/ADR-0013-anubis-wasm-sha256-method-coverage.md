# ADR-0013: Anubis WASM `sha256` Method Coverage

Status: accepted
Date: 2026-11-13
Decision owners: project maintainers
Affected subsystems: search capability; keyless HTML engines; hardening and cost policy; provider diagnostics.
Related: `plans/adrs/ADR-0009-bounded-challenge-solving.md`; `plans/adrs/ADR-0010-challenge-solver-coverage-and-cost-policy.md`; `plans/implementation/agentic-retrieval-integrity/028-anubis-wasm-sha256-method.md`; Anubis `wasm/pow/sha256/src/lib.rs`.

## Context and forces

Anubis v1.28 added WebAssembly proof-of-work methods: `sha256`, `argon2id`, and
`hashx`. For the `sha256` method the published challenge uses the same
`#anubis_challenge` document, but the proof differs from `fast`:

- the challenge data is the hex-decoded `randomData` bytes, not a UTF-8 string;
- the nonce is a `u32` appended as four bytes, little-endian when the last data byte
  is at least 128 and big-endian otherwise;
- the hash is `SHA-256(data || nonce_bytes)`;
- difficulty counts leading zero **bits**, not hex nibbles;
- the submission carries `nonce`, the hex `response`, `id`, `redir`, and
  `elapsedTime`, like `fast`.

ADR-0010's evaluated set is `fast`, `slow`, `preact`, `metarefresh`, and ALTCHA. It
explicitly says adding a method requires evaluating it against ADR-0009's prohibition
list and recording the outcome; it also requires evaluating any change that
materially reduces per-candidate cost.

`sha256` is the same cryptographic primitive and the same per-candidate cost as
`fast`. The differences are the input construction, the four-byte nonce, the
bit-based difficulty, and the method label. It is a published, non-interactive
proof-of-work. `argon2id` is memory-hard and roughly one to two orders of magnitude
slower per candidate; `hashx` is ASIC-resistant and needs a new `hashx` crate. Those
are separate cost and dependency questions and must not be smuggled in with
`sha256`.

## Considered alternatives

1. **Refuse every WASM method.** Current behaviour, and the default until this ADR is
   accepted. Rejected as the permanent answer: `sha256` is the cheapest variant and
   uses a dependency already present.
2. **Execute the site's served `.wasm` module in a runtime.** Rejected. It means
   running remote code, adds a runtime dependency, and conflicts with the bounded-I/O
   and dependency posture.
3. **Reimplement named algorithms.** Selected for `sha256` only. `argon2id` and
   `hashx` need their own evaluations before any implementation.
4. **Permit all three WASM methods at once.** Rejected. It conflates a cheap SHA-256
   variant with a memory-hard KDF and an ASIC-resistant function whose cost and
   dependency profiles are different.

## Decision

- Permit Anubis WASM `sha256` as a published SHA-256 proof-of-work under ADR-0010's
  cost policy.
- Construction: data is the hex-decoded `randomData` (bounded before decoding); the
  nonce is a `u32` searched from zero; the four nonce bytes are little-endian iff the
  last data byte is at least 128, otherwise big-endian; the hash is
  `SHA-256(data || nonce_bytes)`; difficulty is the number of leading zero bits; the
  submission carries the decimal `nonce`, the hex `response`, `id`, `redir`, and
  `elapsedTime`.
- Ceiling mapping: the operator's `max_difficulty` remains measured in
  `fast`-equivalent nibbles. A `sha256` challenge is allowed when
  `ceil(bits / 4) <= max_difficulty`. Cost projection uses `2^bits` expected
  candidates; the attempt counter counts candidates; the search stays single-threaded
  and single-process.
- No new dependency: `sha2` is already a direct dependency. No WASM runtime, no
  remote module execution, no browser.
- `argon2id` and `hashx` are **not** permitted by this ADR. They remain
  `unsupported scheme` until a separate ADR evaluates their cost, dependency, and
  coverage impact.
- Every ADR-0009 and ADR-0010 prohibition stands unchanged.

## Consequences and tradeoffs

Advantages: covers the cheapest WASM variant with the same primitive and cost as
`fast`; adds no dependency; removes a false `unsupported scheme` for a method the
client can solve honestly.

Tradeoffs: difficulty units differ, so the operator ceiling is an approximation
(`ceil(bits / 4)`) that must be documented. A `sha256` difficulty of 24 is roughly
`fast` difficulty 6 and may be out of reach inside a sane deadline; it stays blocked.
The method is new in v1.28 and disabled by default upstream, so a live deployment may
not be reachable for closure evidence; the implementation must be known-answer
verified and recorded as synthetic-only if that happens.

## Compatibility, security, and migration

No MCP tool, response-schema, or configuration-key change. The method is additive and
still opt-in through `[search.challenge].enabled`; unknown methods still fail closed.
The ten-key refusal vocabulary is unchanged. The solver performs only local hashing
plus the requests the engine already makes, behind the existing bounded-body,
deadline, SSRF, and redirect rules. Challenge data, nonces, and pass cookies remain
secret-like and must never appear in logs, errors, warnings, `Debug`, or fixtures.

## Verification and status

Adoption authorizes the method; it does not change behaviour. Implementation lands
through M028 with known-answer vectors for both endianness branches, bit-difficulty
ceiling and projection tests, a nonce-exhaustion test, a mock challenge/replay test,
and a live record if a deployment publishing `sha256` is reachable. Until M028 closes,
`sha256` continues to report `unsupported scheme`.
