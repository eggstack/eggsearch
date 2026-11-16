# M026 — Anubis Challenge Extensions Fail Closed

Status: closed
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M026
Primary class: invariant (corrective)
Baseline: `fd2634e`
References: ADR-0009; ADR-0010; ADR-0012 (accepted); `src/meta/challenge/anubis.rs`; `src/meta/challenge/ledger.rs`; `src/meta/challenge/mod.rs`; `architecture/engines.md` § Challenge solving; `docs/tool-matrix.md` (`provider_status`); `docs/config.md` § Challenge Solving.
Dependencies: none hard. This plan authorizes no extension; it only makes an
unsupported extension fail before a doomed submission. M027 depends on this plan and
on ADR-0012.

## Objective

Anubis v1.28 added `challenge.extensions`. The list names conditions that must
validate after the method proof and before a clearance JWT is issued. The open-source
registry ships `css-load`; the proprietary `headless` extension is a different class.

The solver currently ignores the field. For a page that declares an extension, the
method proof is accepted and the pass-challenge endpoint then rejects the request at
the extension validator. The ledger records `pass cookie rejected`, which tells the
operator the proof was wrong. The proof was correct; the extension was never
performed. That misclassification is the defect this plan fixes.

The fix is deliberately minimal: parse `extensions`, and if the list is non-empty,
refuse before any wait, hash, or request, with a sentence that names the extension
list and an existing refusal key. It does not authorize any extension. M027, under
ADR-0012, is the separate plan that may support `css-load`.

**Why the original verification missed it.** `extensions` arrived with the v1.28
challenge registry, after M011's and M016's fixtures were written. The Startpage page
carries no `extensions` key and the live v1.28.3 page carries `null`, so no test
exercised a non-empty list. The solver's success path stopped at the method proof, and
the endpoint's extension rejection was attributed to the proof. The regression test is
a non-empty list that must perform zero submissions.

## Current implementation evidence

- `AnubisChallenge::parse` reads `rules.difficulty`, `challenge.method`, `id`,
  `randomData`, `redir`, and `spent`; it never reads `challenge.extensions`.
- The live v1.28.3 page observed on 2026-11-13 carries `"extensions": null`. The
  Startpage v1.26.4 page carries no `extensions` key at all. Both must remain
  accepted.
- The fixed refusal vocabulary is ten keys in `src/meta/challenge/ledger.rs`; the
  `provider_status.challenge_solver` block advertises those keys and
  `tests/tool_surface_live.rs` pins the advertisement size. This plan does not add a
  key.
- The existing `blocked_step(reason, key)` seam carries a precise operator sentence
  while recording an attributable key; it is the right shape for this refusal.

## Non-goals

- No extension is authorized by this plan. `css-load` support is M027 under
  ADR-0012; `headless`/BotStopper remains prohibited.
- No new ledger key, no `provider_status` schema change, no new configuration key.
- No change to any method solver, the difficulty ceiling, the attempt budget, the
  cookie cache, or the fetch path.
- No attempt to parse or execute the extension's page instructions. The page's
  `<style>`/`<script>` content is untrusted markup.
- No retry: a refused extension is terminal for that attempt.

## Invariants that cannot regress

- `extensions` absent and `null` are both treated as the empty list and remain
  solvable.
- A non-empty list fails closed before `check_ceiling`, `check_projection`, any wait,
  any hash, and any HTTP request.
- The refusal sentence names at most a bounded number of extension names, each
  bounded and sanitized; control characters, quotes, and arbitrary long strings are
  never echoed.
- The recorded key is an existing key (`unsupported scheme`); the ten-key vocabulary
  and the advertised `challenge_solver` object are unchanged.
- The failure is `EngineError::Blocked` / `blocked`, never an empty result set.
- No cookie, challenge ID, nonce, or token appears in the sentence, logs, warnings,
  or `Debug`.
- The four ceilings and the one-solve-per-TTL rule are unchanged.

## Expected production changes

1. **Parse the field.** Add `extensions: Vec<String>` to `AnubisChallenge` and read
   `challenge.extensions` in `parse`:
   - absent or `null` → empty;
   - an array of strings → keep at most 8 names, each at most 64 bytes, rejecting
     control characters and empty names as `malformed parameters`;
   - any other type → `malformed parameters`.
2. **Refuse before work.** In `solve_and_verify_inner`, after parsing and before
   `check_ceiling`, if `extensions` is non-empty, return
   `blocked_step(sentence, RefusalReason::UnsupportedScheme)` with a sentence such as
   `anubis challenge declares unsupported extensions [css-load]`. Use the parsed,
   sanitized names only.
3. **Keep the vocabulary.** Do not add a ledger key. Document that an extension
   refusal is attributed to `unsupported scheme` because the build does not support
   that extension, and that the sentence carries the specific cause.
4. **Tests.**
   - `extensions` absent, `null`, and `[]` all solve.
   - `["css-load"]` and `["css-load","headless"]` block with zero pass requests and
     zero hashing attempts; the sentence names the list.
   - An unknown name blocks the same way.
   - More than 8 names, a name over 64 bytes, a control character, a non-string
     element, and a non-array type fail closed without echoing raw text.
   - The ledger's `unsupported scheme` counter increments; the other nine stay zero.
   - A canary asserts no cookie, ID, or token appears in the sentence.
5. **Docs.** `architecture/engines.md` § Challenge solving (extensions fail closed
   until an accepted ADR names them), `docs/tool-matrix.md` (`provider_status`
   attribution), `CHANGELOG.md`.

## Ordered work packages

1. Parse and bound `extensions`, with unit tests.
2. Refusal seam and ledger attribution, with zero-request tests.
3. Sentence sanitization and canary tests.
4. Documentation and the closure record.

## Verification

```bash
cargo fmt --all
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features --lib meta::challenge
cargo test --locked --all-features --test dispatch_fault_injection
cargo test --locked --all-features --test tool_surface_live
cargo test --locked --all-features --test static_guards
cargo test --locked --all-features
```

Live, recorded rather than asserted: fetch a page that declares `css-load` if one is
reachable, and record that the solver refuses before submission with the named
sentence and `unsupported scheme`; if none is reachable, keep the synthetic fixture
and record that limitation. The live v1.28.3 page with `extensions: null` must
continue to solve.

## Compatibility and closure evidence

No MCP tool, response-schema, configuration-key, or ledger-key change. The ten-key
refusal vocabulary and the advertised `challenge_solver` object are preserved; an
extension refusal is attributed to the existing `unsupported scheme` bucket. A page
with `extensions` absent or `null` behaves exactly as today, so there is no migration.

Required closure evidence: the implementation commit; the focused and full test
outputs; the ledger delta showing only `unsupported scheme` incremented; the sanitized
sentence assertion; the live `extensions: null` regression; the documentation updates;
and the residual risk if a deployment makes a non-empty extension mandatory. The
record must state that no extension is authorized by this milestone.

## Acceptance and stop conditions

- A page with `extensions: null` or no field solves exactly as today.
- A page with a non-empty list performs zero pass requests and zero hashing attempts,
  returns `blocked`, records `unsupported scheme`, and names the extension list in
  the sentence.
- No new ledger key or schema field appears; `provider_status` remains shape-stable.
- No raw extension text, cookie, or token is echoed.
- Stop and return for a decision if a live deployment makes `extensions` a required
  field for every challenge, or if the refusal sentence cannot be bounded without
  losing the operator's ability to identify the cause. Do not authorize an extension
  here; that is M027's decision under ADR-0012.
