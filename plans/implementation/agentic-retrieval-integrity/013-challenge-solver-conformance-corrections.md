# M013 — Challenge Solver Conformance Corrections

Status: closed
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M013
Primary class: invariant (corrective)
Baseline: `dace41c3a75307ea35b745d695a11630ec6495c0`
References: ADR-0010; ADR-0009; `plans/implementation/agentic-retrieval-integrity/010-challenge-solving-framework-and-altcha.md`; `011-anubis-solver-and-difficulty-ceiling.md`; `012-startpage-payload-extraction.md`; `plans/closure/agentic-retrieval-integrity/011-status.md`; `architecture/engines.md` § Challenge solving; `src/meta/challenge/mod.rs`; `src/meta/challenge/anubis.rs`; `src/meta/engines/startpage.rs`; `src/meta/engines/mojeek.rs`; `src/core/config/challenge.rs`; `tests/fixtures/engines/README.md`.
Dependencies: none hard. ADR-0010 is accepted and authorizes the coverage implemented here; ADR-0010 and the roadmap M013 row are uncommitted in the handoff worktree and must land with this milestone. M010–M012 are closed at the baseline. Independent of M014 (pass-cookie lifetime), M015 (solvability observability), and M016 (`metarefresh`).

## Objective

Correct five conformance defects in the shipped Anubis solver so that it follows the contract the upstream publishes, and so that every refusal happens before any hashing rather than at the deadline. This is a corrective pass under `plans/003-planning-process.md` §7, not an amendment to M011: the defects below were shipped by M011 and are visible in the baseline code, and each needs a regression test that fails at the baseline.

| # | Defect | Baseline evidence | Consequence |
|---|---|---|---|
| 1 | `anubis_base_prefix` ignored | `PASS_CHALLENGE_PATH` is a constant (`src/meta/challenge/anubis.rs:26`) and `endpoint()` joins only base plus path (`:345`) | A prefixed deployment is answered at the wrong route, so the solver reports `pass cookie rejected` — a reason that is not true |
| 2 | `slow` refused as unsupported | dispatch accepts only `fast` and `preact` (`anubis.rs:109-120`); `slow` is the *negative* control in `unknown_method_fails_closed` (`:617`) | ADR-0010 makes `slow` an in-scope alias of `fast` with an identical scheme, so a solvable deployment reports `unsupported scheme` |
| 3 | `spent` unchecked | `parse()` reads `id`, `randomData`, `method`, `redir` and never `spent` (`anubis.rs:88-137`), while the live payload carries `"spent": false` | An already-spent challenge is solved and submitted, spending the budget and misreporting the outcome |
| 4 | whole deadline burned instead of failing fast | `check_ceiling` compares only `rules.difficulty` (`anubis.rs:167`); the only other guard is a per-attempt check in `SolveBudget::attempt` (`mod.rs:86-95`) | The expected cost is computable up front but the solver hashes until `wall clock exhausted` or `attempts exhausted`, with no projection in the reason |
| 5 | published `redir` passed through unvalidated | `resolved_redir` returns any non-empty published string (`anubis.rs:156-162`); `preact_url` treats anything not starting `http(s)` as a base-relative path (`:349-359`) | The defect class recorded as CVE-2025-54414 on this endpoint; and a `preact` absolute `redir` would receive the challenge cookies off-origin |

Why M011's verification did not catch these. Its fixtures modelled one shape — a synthetic `fast` page with a `redir`, no prefix script, and `spent: false` — so the prefix script, the `slow` label, `spent`, and the `redir` value were never varied. `slow` appeared only as the unknown-method negative control, which asserts the defect as correct behaviour. The budget tests asserted "refuses eventually" (`attempts exhausted`, `wall clock exhausted`) and never that a refusal is arithmetic; the fail-fast property was not stated. `redir` was corrected once, after the live capture, but only for *presence* (optional versus mandatory), not for its value. And only `fast` was exercised against a live deployment.

## Non-goals

- **`elapsedTime` stays the constant `1`.** ADR-0009's determinism bullet requires a fixed constant for any client-supplied timing field the upstream does not verify, and the live difficulty-6 Startpage solve recorded in `plans/closure/agentic-retrieval-integrity/011-status.md` (cookie observed 2026-11-11, end-to-end 5/5 rows on 2026-11-12 UTC) succeeded while submitting `elapsedTime=1`. That demonstrates the field is not validated as a minimum. Changing it would require a superseding record, not a milestone.
- **ALTCHA's `solution.time` stays `150`** for the identical reason; `src/meta/challenge/altcha.rs` behaviour is out of scope.
- No `metarefresh` solver (M016), no pass-cookie lifetime change (M014), no new observability or counter surface (M015).
- No new configuration key: `src/core/config/challenge.rs` and every validated range stay exactly as they are.
- No new dependency, no new MCP tool, no new `provider_status` field, no `error_class` wire-string change.
- No parallel, batched, threaded, GPU, or distributed nonce search (ADR-0010 forbids it).
- No interactive CAPTCHA, OCR, solving service, proxy rotation, user-agent change, or retry widening.
- No claim that `slow`, `preact`, or a prefixed deployment has been observed live. Only `fast` has been; the plan states below how that gap closes or where it is carried.

## Invariants that cannot regress

- Ten MCP tools, stable FNV-1a IDs, sanitization tiers, trust framing, and deadline bounds are unchanged.
- Disabled, above-ceiling, spent, unsupported, malformed, and budget-exhausted states all remain `EngineError::Blocked` → `error_class: blocked` → `FailureClass::Blocked`, never `Ok(vec![])`.
- A solved-but-unparseable page stays `ParseFailed`, never `Blocked` and never empty.
- Every upstream request stays behind `read_bounded_body`/`read_bounded_response`; the pass-challenge request keeps redirect following disabled.
- Cookie values, nonces, salts, derived keys, and the challenge id never appear in logs, `provider_status`, diagnostics, fixtures, or `Debug` output. The base prefix joins that redaction list.
- Solving is attempted at most once per provider per search request and at most once per `cookie_ttl_seconds`; no correction adds a request on any refusal path.
- The nonce search stays single-threaded and deterministic: nonce ascending from `0`, `elapsedTime = 1`, `solution.time = 150`, no randomness anywhere.
- The projection is an admission check only. It never bypasses, widens, or replaces the per-attempt ceiling, and a challenge it admits still stops on `max_attempts` or the wall clock.
- For today's shipped Startpage shape (empty prefix, `fast`, `spent: false`, no `redir`) the emitted pass-challenge request is byte-identical to the baseline: same path, same five query parameters.
- Cache isolation, process-local-only storage, and the disabled path's single-request behaviour are unchanged.

## Expected production changes

### 1. `anubis_base_prefix` — parse, validate, prepend

- Add `pub const BASE_PREFIX_SCRIPT_ID: &str = "anubis_base_prefix"` and a `base_prefix: String` field on `AnubisChallenge`, populated in `parse()` from `script#anubis_base_prefix`.

  **Already observed, so do not re-derive it.** A live Startpage wall captured on 2026-11-11 emits
  the element in exactly the assumed form, JSON-quoted and empty:

  ```html
  <script id="anubis_base_prefix" type="application/json">""
  </script><script id="anubis_public_url" type="application/json">""
  </script>
  ```

  That confirms both the element id and that the payload is a JSON string rather than bare text,
  which is the stop condition below. The capture lives outside the repo (`/tmp`), so it is not
  committed evidence; the fixture work package should commit a stripped copy. `anubis_public_url`
  is a sibling element the solver currently ignores — record whether it matters for path
  construction rather than assuming it does not.
- The script body is a JSON string (Anubis publishes `""` by default). Absent element → empty prefix, which is the published default and keeps existing captures and fixtures valid. Present but not a JSON string → `BOUND_MALFORMED`. Present and unusable → a new named bound, never silently ignored and never downgraded to empty.
- New bound in `src/meta/challenge/mod.rs`: `pub const BOUND_BASE_PREFIX: &str = "unsafe base prefix"`, so the engine reports `challenge not solved: unsafe base prefix`.
- Validation, applied to the trimmed value: at most 256 bytes; empty is valid; otherwise it must begin with `/`, must not begin with `//`, and must contain no `:` (scheme delimiter), `?`, `#`, backslash, ASCII control or whitespace character, and no `..` path segment. One trailing `/` is normalised away before joining; any other `//` is refused.
- Join as `{base_without_trailing_slash}{prefix}{PASS_CHALLENGE_PATH}`. With an empty prefix the result is byte-identical to `endpoint(base_url, PASS_CHALLENGE_PATH)`, which the existing Startpage replay test already pins.
- Validation happens in `parse()`, before the difficulty ceiling, so the refusal costs no hashing and no request. It applies to both methods: an unusable published control value means the page is not trusted, even though only `fast` consumes it.

### 2. `slow` — dispatch through the `fast` path

- Add `pub const METHOD_SLOW: &str = "slow"` documented as the deprecated alias of `fast` using the identical scheme (ADR-0010), and match `"fast" | "slow" => AnubisMethod::Fast`. No new enum variant: the solution, the submission, and the request are the fast ones, and the alias must not fork behaviour.
- A genuinely unregistered method still fails closed as `BOUND_UNSUPPORTED_SCHEME`. Repurpose the two negatives that currently use `slow` (`anubis.rs:617-656`, and `UNKNOWN_METHOD_PAGE` at `src/meta/engines/startpage.rs:372`) to a synthetic unregistered name, and add positives proving `slow` produces the same digest, the same nonce, and the same pass-challenge request as `fast` from the same `randomData` and difficulty.

### 3. `spent` — refuse an already-spent challenge

- Parse `challenge.spent` as `Option<bool>`: absent → `false`; `true` → refuse; a present non-boolean → `BOUND_MALFORMED`.
- New bound: `pub const BOUND_SPENT: &str = "already spent"`, so the engine reports `challenge not solved: already spent`. The check also applies to the `preact_info` document when it publishes the field.
- Refusal is `SolveOutcome::Refused` from `parse()`, therefore zero attempts and zero HTTP requests.

### 4. Pre-flight projection — refuse before hashing

- Add `pub const CANDIDATE_NANOS: u64 = 130` in `anubis.rs`. The derivation is the recorded release measurement in `plans/closure/agentic-retrieval-integrity/011-status.md` § Measured cost: 3.05 s for 23,070,878 candidates ≈ 0.13 µs per candidate, repeated in `architecture/engines.md` § Challenge solving. The derivation belongs in the constant's doc comment and the architecture section, not in an inline comment; if the closure measurement lands more than ~3× away, change the constant and the recorded source together rather than tuning silently.
- Pure helpers, so the strings are unit-testable without a server: `expected_candidates(difficulty) = 16^difficulty` (u128), `compact_count(n)` with round-half-up and the largest `K`/`M`/`G`/`T` unit that leaves a mantissa below `1_000`, and `check_projection(&budget) -> Result<(), String>` mirroring `ceiling_refusal`.
- `src/meta/challenge/mod.rs`: `SolveBudget` gains `timeout()` (the configured Duration, stored at construction) and `remaining_time()` (saturating `deadline - now`). No other shape change; it stays `Copy`.
- Refusal rules, `fast` and `slow` only (`preact` keeps its existing bounded-wait refusal):

| Condition | Exact sentence |
|---|---|
| expected candidates exceed the attempt ceiling | `anubis difficulty {d} projects {compact} candidates, beyond the configured attempt ceiling {max_attempts}` |
| expected wall clock exceeds the budget still available | `anubis difficulty {d} projects {compact} candidates (~{projected_ms} ms), beyond the configured wall-clock budget {timeout_ms} ms` |

  Both sentences name the projection and the operative ceiling, so an operator can compare them without arithmetic; the second is the wall-clock analogue of the first. Canonical rendering for the difficulty-7 case: `anubis difficulty 7 projects 268M candidates, beyond the configured attempt ceiling 2000000`. Comparison is strict (`expected > ceiling`), so a budget exactly at the expectation is admitted; the attempt ceiling is still enforced per candidate.
- Order inside `solve_and_verify`: parse (spent, prefix, method) → `check_ceiling` → `check_projection` → `preact` wait → solve → submit. Both ceiling and projection return `EngineError::Blocked` directly, as the ceiling does today, because `SolveOutcome::Refused` cannot carry a dynamic sentence.

### 5. `redir` — validate before it reaches the query string

- Replace the pass-through in `resolved_redir` with: absent or empty → `request_url` (unchanged); any ASCII control character, whitespace, CR, or LF → `request_url`; an absolute URL parsed with the existing `url` dependency whose scheme is `http`/`https` **and** whose origin (scheme, host, effective port) equals `base_url`'s origin → accepted verbatim; a root-relative path starting with a single `/`, with no `..` segment and no control characters → accepted as a path; anything else (`javascript:`, `data:`, `file:`, `//host/x`, backslash-prefixed, cross-origin absolute) → `request_url`.
- Consequences that must be asserted: no non-`http(s)` scheme can reach the `redir` query parameter; the origin bound means the challenge cookies can never be sent to a foreign origin on the `preact` submission, which is the one method where `redir` is the request target. Percent-encoded traversal is not decoded locally and is left to the server's normalisation, which is outside this milestone.
- `preact_url` consumes the validated value only, so both methods share one rule.

### 6. Tests, docs, and the two evidence gaps

- Tests live in `src/meta/challenge/anubis.rs` (contract-level, mock transport) and `src/meta/engines/startpage.rs` (engine-level `search_at` flows). Every new Startpage test takes `CHALLENGE_TEST_LOCK` and clears `pass_cookie_cache()` first, because the cache is process-wide.
- Engine-level coverage required by the roadmap exit condition: a page carrying `anubis_base_prefix = "/anubis"` solves and issues `GET /anubis/.within.website/x/cmd/anubis/api/pass-challenge`; a page publishing `method = "slow"` solves; a spent page is `Blocked` with the named reason; a difficulty-7 page at a raised ceiling and `max_attempts = 2000000` is `Blocked` with the projection sentence and zero pass-challenge hits.
- `src/meta/engines/mojeek.rs` and `src/core/config/challenge.rs` are not modified. `src/meta/challenge/altcha.rs` is not modified.
- Docs updated in the same change: `architecture/engines.md` § Challenge solving (registry coverage with the `slow` alias, the prefix contract, `spent`, the projection and its constant, the `redir` rule, and the unchanged `elapsedTime` determinism), `docs/config.md` § Challenge Solving (projection refusal and the three-knob relationship it implies), `docs/provider-setup.md` (Startpage and Mojeek sections), `docs/features.md` (solver outcome list), `CHANGELOG.md` § Unreleased / `### Fixed`.
- ADR-0010's `Related:` line currently names `013-anubis-conformance-and-fail-fast.md` and `017-anubis-metarefresh-solver.md`; the roadmap registers these as `013-challenge-solver-conformance-corrections.md` and M016. Correct the two stale filenames only — no decision text changes — and note it in the closure record.
- No new test suite is added, so `docs/test-inventory.md` and `architecture/testing.md` are unchanged.
- If the added tests would push `anubis.rs` past 1,600 lines / 80 KB, move its `#[cfg(test)] mod tests` into `src/meta/challenge/anubis_tests.rs` declared as `#[cfg(test)] #[path = "anubis_tests.rs"] mod tests;`. Do not raise a ceiling.

## Storage, protocol, and compatibility effects

No configuration surface change: `[search.challenge]` keeps its five keys, defaults, and validated ranges, so a config written for ADR-0009/ADR-0010 parses and behaves identically. No MCP tool, response schema, `provider_status` field, or `error_class` wire string changes. The new sentences appear only in `EngineError::Blocked.reason`, the channel that already carries `anubis difficulty 6 exceeds configured ceiling 5`; they are operator-visible diagnostics, not a contract enum. No persistence, no migration, no dependency delta, no MSRV change.

Two deliberate behaviour changes, both classification improvements for a page the solver previously mishandled: an unusable published prefix now reports `unsafe base prefix` instead of `pass cookie rejected`, and a spent challenge reports `already spent` instead of attempting the submission. For the shipped Startpage shape nothing observable changes: the disabled path issues one request and reports `blocked`; the enabled path issues the same pass-challenge URL and query as the baseline, including `elapsedTime=1`.

## Ordered work packages

1. `mod.rs`: `BOUND_BASE_PREFIX`, `BOUND_SPENT`, `SolveBudget::timeout()`, `SolveBudget::remaining_time()`, and unit tests for saturation and for the quoted timeout.
2. Prefix: parser, validator, join, and redaction in `Debug`; unit tests for absent, `""`, `/anubis`, trailing-slash normalisation, `//host`, scheme, `..`, CR/LF, whitespace, over-length, JSON non-string, plus a mock-transport test asserting the exact requested path.
3. `slow` alias plus the repurposed unregistered-method negatives in `anubis.rs` and `startpage.rs`; positive equivalence test against `fast`.
4. `spent`: parse, refuse, and tests for `true`, absent, and non-boolean.
5. Projection: `expected_candidates`, `compact_count`, both refusal helpers, the constant, and tests for the exact sentences at difficulty 7 / `2000000`, the wall-clock branch at difficulty 5 with a 50 ms budget, an admitted case, and `attempts_used() == 0` with zero HTTP hits on every refusal.
6. `redir`: validator plus origin comparison; table test over `javascript:`, `data:`, `file:`, `//host/x`, `\host`, `..`, CR/LF, cross-origin absolute, same-origin absolute, and `https`/`path` acceptances; a `preact` test proving a cross-origin value falls back to the request URL.
7. Engine-level Startpage tests for the four flows listed above, asserting exact `Blocked` reasons, `pass-challenge` hit counts, and the byte-identical baseline path.
8. Docs, ADR-0010 link correction, changelog, and the live recording (or its explicit absence) in `tests/fixtures/engines/README.md`.

Each of work packages 2–6 must land a test that fails against `dace41c3a75307ea35b745d695a11630ec6495c0`; the closure record names that test per defect.

## Verification

```bash
cargo fmt --all
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features --lib meta::challenge
cargo test --locked --all-features --test engine_parse_conformance
cargo test --locked --all-features --test static_guards
cargo test --locked --all-features
```

Focused and documentation gates the change also needs:

```bash
cargo test --locked --all-features --lib meta::engines::startpage
make docs-check
python3 packaging/check-planning-consistency.py
```

Opt-in live recording. Build a release binary — the 0.13 µs constant is a release figure and debug builds are about two orders of magnitude slower, which would make every projection refusal misleading:

```bash
cargo build --release
target/release/eggsearch --config /tmp/m013-live.toml search "rust programming language" --providers startpage --json
```

with `[search] timeout_ms = 8000`, `[search.providers] startpage = true`, and `[search.challenge] enabled = true`, `max_difficulty = 6`, `max_attempts = 32000000`, `timeout_ms = 8000`. Record the published difficulty, the attempt count actually used, the wall clock, and the row count in `tests/fixtures/engines/README.md` under a dated "Challenge solver conformance" heading. This run must reproduce the baseline's live result, proving the prefix join, `slow` dispatch, `spent` default, projection admission, and `redir` fallback did not disturb the working path.

Also attempt, in the same session, a deployment that publishes `anubis_base_prefix` with a non-empty value, one that publishes `method = "slow"`, and one that publishes `preact` (no shipped provider is known to publish any of the three). For each observation record the challenge page shape with ids, nonces, and cookies stripped, the requested path, and the outcome. When none is reachable, record exactly that — "no prefixed, `slow`, or `preact` deployment reachable on <date>; those paths are synthetic-fixture verified only" — and carry it as a named residual. Do not describe a synthetic test as a live observation, and do not add an unregistered provider to reach one.

## Acceptance and stop conditions

- All five corrections have a regression test that fails at the baseline and passes at the candidate.
- The shipped Startpage shape is unchanged: same pass-challenge path, same five query parameters, `elapsedTime=1`, and the existing `enabled_challenge_solves_and_replays_once` test passes without edits to its expectations.
- A prefixed challenge page solves end to end against the mock transport and requests `/anubis/.within.website/x/cmd/anubis/api/pass-challenge`; a `slow` page produces the same solution as the equivalent `fast` page.
- `spent: true`, an unusable prefix, an unregistered method, an above-ceiling difficulty, and a projection breach are each `EngineError::Blocked` → `error_class: blocked`, with the named reason, with `attempts_used() == 0` for the arithmetic refusals, and with zero upstream requests on that path.
- No cookie, nonce, salt, derived key, challenge id, or base prefix value appears in any log line, `provider_status` entry, error string, fixture, or `Debug` rendering.
- The disabled path still issues exactly one search request and reports `blocked`; a solved-but-unparseable page still reports `ParseFailed`.
- Stop and return for a decision if: a live deployment publishes `anubis_base_prefix` unquoted rather than as a JSON string (do not widen the parser silently); the prefix turns out to be a full origin rather than a path prefix; a live `slow` or `preact` observation shows a submission shape that differs from the `fast` path; or `spent` proves to be a field the server does not act on. Any of those contradicts the contract this plan encodes.
- Do not raise `max_difficulty`'s default, change a validation range, or add a configuration key to make an acceptance case pass.

## Closure evidence

`plans/closure/agentic-retrieval-integrity/013-status.md` must record: the implementation candidate SHA; the defect-to-test matrix with the baseline failing run and the candidate passing run for each of the five corrections; the exact commands above with their results; the accepted and rejected prefix table; the two projection sentences as emitted; the re-derived per-candidate constant from the release measurement and its comparison with `130`; the live Startpage record with its date, difficulty, attempt count, wall clock, and row count; the explicit status of the prefixed, `slow`, and `preact` observations (recorded or carried); evidence that no secret material appears in diagnostics; why M011's verification missed each defect, in one line per defect; and residual risk — unreachable prefixed deployment, unverified `slow` and `preact` paths, Anubis version and difficulty drift, the server-side `redir` class that CVE-2025-54414 records, and the projection's expected-value admission line (a budget at exactly the expectation is admitted and remains probabilistic). The closure commit also carries the roadmap M013 status, the registry row, the committed ADR-0010, and the ADR-0010 `Related:` link correction.
