# M028 Anubis WASM `sha256` Method Coverage — Closure Status

Status: closed

Source plan: `plans/implementation/agentic-retrieval-integrity/028-anubis-wasm-sha256-method.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Planning baseline: `fd2634e`

Implementation candidate and closure commit: the commit that carries this record.

## Result

ADR-0013 authorizes the cheapest member of the v1.28 WebAssembly family; `sha256` now solves under
the same ceilings as `fast`.

- `METHOD_SHA256` and `AnubisMethod::WasmSha256`. Parsing prefers `challenge.method` and falls back
  to `rules.algorithm`; `randomData` is hex-decoded at parse time, bounded at 4 KiB decoded, and
  invalid or empty hex is `malformed parameters`.
- Construction: `SHA-256(data || nonce_bytes)`, `nonce: u32` searched from zero, four bytes
  little-endian iff the last data byte is at least 128 and big-endian otherwise. Difficulty counts
  leading zero **bits** on the raw digest, with a bit-counting helper that does not reuse the
  nibble helper.
- Ceiling: `ceil(bits / 4) <= max_difficulty`, so 4 admits 16 bits and rejects 17, and 6 admits 24.
  Projection uses `2^bits`; 128 bits or more is refused as unprojectable before hashing.
- Submission: the existing pass-challenge URL carries `id`, `response` (hex digest), `nonce`,
  `redir`, and `elapsedTime`; cookie handling is M024's pair set. A nonce wrap or the attempt
  ceiling is a named refusal, never a panic or a loop.
- `argon2id` and `hashx` remain `unsupported scheme`; no WASM runtime, dependency, configuration
  key, ledger key, tool, or response field was added.

Known-answer vectors: data `abcd` (last byte `0xcd`, little-endian) at 8 bits finds nonce 42 with
digest `00ebbd2e622cc5a6c39c48707ddfd0bd679515e63fd28cbb0426e68448af0714`; data `ab01` (last byte
`0x01`, big-endian) at 2 bits finds nonce 0 with digest
`31c3eb71bf40a0e389a4fcd0e4a87d37623e48c76ea40b07965ae53d4bd2f019`. The vectors were recomputed from
the published `wasm/pow/sha256/src/lib.rs` construction, not from the client.

## Live evidence (2026-11-13 UTC)

No deployment publishing `"algorithm": "sha256"` was observed. The probed deployments published
`fast` or `metarefresh`, so the method is **synthetic-fixture verified only**. No live pass is
claimed; the residual risk is a live construction change in a future Anubis release.

## Verification

```bash
cargo test --locked --all-features --lib meta::challenge   # 118 passed; vectors, bit counting, ceiling, projection, invalid hex, exhaustion, submission, no-cookie canary
cargo test --locked --all-features --lib meta::engines::startpage  # 55 passed; sha256 engine solve and replay
cargo test --locked --all-features --test dispatch_fault_injection  # green in the full all-features run
```
cargo test --locked --all-features --no-fail-fast   # 6306 passed, 0 failed, 28 ignored (85 test binaries)
