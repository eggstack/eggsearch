# M032 ALTCHA on the General Fetch Path — Closure Evidence

Status: closed

Source plan: `plans/archive/implementation/agentic-retrieval-integrity/032-altcha-on-the-general-fetch-path.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Architecture decision: `plans/adrs/ADR-0017-fetch-path-challenge-resolution.md` (accepted 2026-11-19)

## Scope

M032 extended the bounded challenge solve from the Anubis family to ALTCHA PBKDF2 challenges on the
general fetch path. It added the ALTCHA branch to the shared `src/challenge_replay/` module that
M031 established, gave that branch the page-shaped derivation it needs — which challenge document
the page publishes, and which same-origin endpoint verifies a solution — and kept one ALTCHA solve
implementation shared by the engine side and the fetch side.

The derivation rule was written into `architecture/fetch.md` before the code, so it is reviewable
independently of the implementation.

## Exact code candidate

`421d4b2bbb81f22d3e74be1f7f3ff22a5d63c54e` — the implementation commit, carrying the new shared
ALTCHA module, the solver refactor, the two static guards, the new tests, and the documentation
set. Every command recorded below ran against this exact candidate. The closure commit carries this
record, the plan status, the roadmap status, and the registry row together.

## Requirement-to-evidence mapping

| # | Acceptance criterion | Evidence |
|---|---|---|
| 1 | A general fetch against a loopback ALTCHA wall, with the opt-in enabled, returns the replayed document with `manual_interaction_required: false` and no `fetch_challenge_page` warning | `tests/web_fetch_integration.rs::web_fetch_solves_a_published_altcha_challenge_and_returns_the_document`. The fixture serves `<altcha-widget challenge="/captcha/challenge">` as the wall, the published PBKDF2/SHA-256 document at the challenge endpoint, a `chllg` pass cookie at the verification endpoint, and the document at the cookie-bearing replay. The test asserts the replayed text, `manual_interaction_required: false`, no challenge warning, and exactly one wall request, one challenge request, one verification, and one replay. Suite: 106 passed, 0 failed, 1 ignored. |
| 1 (other surfaces) | The same behaviour holds on every fetch surface | `tests/repo_workflow.rs::repo_fetch_remote_solves_a_published_altcha_challenge` (99 passed, 0 failed) and `src/commands/fetch.rs::fetch_response_solves_a_published_altcha_challenge_when_enabled` (18 passed, 0 failed) drive the same loopback fixture through the remote `repo_fetch` path and the CLI. The CLI test also asserts the JSON payload reports `manual_interaction_required: false`. |
| 2 | A cross-origin published endpoint fails closed as `unsupported scheme` with zero verification requests | `tests/web_fetch_integration.rs::web_fetch_refuses_a_cross_origin_altcha_endpoint`. The wall publishes `challenge="https://other.example/captcha/challenge"`; the challenge endpoint, the verification endpoint, and the replay are each hit zero times, and the original marked response is returned. The bound name is asserted at the derivation level: `src/challenge_replay/altcha.rs::tests::rejects_every_cross_origin_or_malformed_endpoint` expects `BOUND_UNSUPPORTED_SCHEME` (`"unsupported scheme"`). The property test `src/challenge_replay/altcha.rs::tests::a_derived_endpoint_is_never_cross_origin` proves no published value can yield a cross-origin endpoint. |
| 3 | A malformed published payload fails closed as `malformed parameters` before any solve | `tests/web_fetch_integration.rs::web_fetch_refuses_a_malformed_altcha_payload`. The challenge endpoint returns `{"parameters":{"algorithm":"PBKDF2/SHA-256"}}`; the verification endpoint is hit zero times, so the refusal happens before any submission. The bound name is asserted by `src/meta/challenge/altcha.rs::tests::malformed_parameters_are_refused`, which expects `BOUND_MALFORMED` (`"malformed parameters"`). |
| 4 | An unsupported published algorithm fails closed as `unsupported scheme` | `tests/web_fetch_integration.rs::web_fetch_refuses_an_unsupported_altcha_algorithm`. The published algorithm is `PBKDF2/SHA-512`; the verification endpoint is hit zero times and the wall stays marked. The bound name is asserted by `src/meta/challenge/altcha.rs::tests::unsupported_algorithm_is_named`, which expects `BOUND_UNSUPPORTED_SCHEME`. |
| 5 | An ALTCHA solve that exceeds `max_attempts` or `timeout_ms` is refused with the operative ceiling named, and the original response is returned unchanged | `tests/web_fetch_integration.rs::web_fetch_keeps_the_mark_when_the_altcha_budget_refuses`. With `max_attempts = 8` and an unreachable key prefix, the solve stops at the attempt ceiling, the verification endpoint is hit zero times, no replay is issued, and the original response keeps its marking. The operative ceilings are named at the solver level: `src/meta/challenge/altcha.rs::tests::negative_vector_exhausts_attempts` expects `BOUND_ATTEMPTS` (`"attempts exhausted"`) and `::wall_clock_ceiling_is_enforced_by_cost` expects `BOUND_WALL_CLOCK` (`"wall clock exhausted"`). Both land in the fixed refusal vocabulary reported by `provider_status`. |
| 6 | At most one solve and one replay occur per fetch call on every surface | The hit counts in the web-fetch, `repo_fetch`, and CLI tests assert wall 1, challenge 1, verification 1, and replay 1. The disabled-solver and refusal tests assert the opposite: zero challenge requests, zero verifications, and zero replays. |
| 7 | The operator documentation states that `max_attempts` and `timeout_ms` bound an ALTCHA solve and that `max_difficulty` does not gate it | `docs/config.md` § Challenge Solving now states that `max_difficulty` gates the Anubis family only, that an ALTCHA solve is bounded by `max_attempts` (which counts PBKDF2 counter iterations) and by `timeout_ms`, and that the published PBKDF2 `cost` multiplies the work each iteration performs. `docs/features.md` § Limits repeats the statement, and `architecture/fetch.md` § Challenge pages on the HTTP path states it in the module-ownership paragraph. |
| 8 | The broad gate passes on the exact candidate, and every test that was run is recorded with its outcome | See Verification below. |

## Additional guard evidence

- `tests/static_guards.rs::altcha_fetch_solve_has_one_owner` fails closed unless exactly one file
  owns the fetch-path ALTCHA solve, and fails closed if any file outside `src/challenge_replay/`
  and `src/meta/` calls the ALTCHA solver. It counts the owner, so it cannot pass vacuously.
- `tests/static_guards.rs::altcha_endpoint_derivation_is_same_origin_only` fails closed unless the
  derivation keeps its same-origin gate and the two existing refusal keys, and fails closed if the
  derivation carries a hardcoded request target.
- `tests/static_guards.rs::fetch_layer_never_imports_the_challenge_solver` still passes: no file
  under `src/fetch/` references the solver or the shared module.
- `tests/static_guards.rs::challenge_solve_orchestration_has_one_owner` still passes: the MCP and
  CLI layers still never call the solver directly.

## Verification

Using the pinned Rust 1.89 toolchain on candidate `421d4b2bbb81f22d3e74be1f7f3ff22a5d63c54e`:

- `make check`: exit 0 — 85 suites, 6,399 tests, 0 failed, 28 ignored. This includes fmt, clippy
  with `-D warnings`, the no-default-feature check, the all-features test run, repository hygiene,
  the dependency policy, and the packaging contract checks.
- `cargo test --locked --all-features --test web_fetch_integration`: 106 passed, 0 failed, 1 ignored.
- `cargo test --locked --all-features --test repo_workflow`: 99 passed, 0 failed.
- `cargo test --locked --all-features --test batch_fetch_retrieval`: 23 passed, 0 failed.
- `cargo test --locked --all-features --test static_guards`: 66 passed, 0 failed.
- `cargo test --locked --all-features --test mcp_tools`: 28 passed, 0 failed.
- `cargo test --locked --all-features --bin eggsearch`: 18 passed, 0 failed.
- `cargo test --locked --all-features --lib challenge_replay`: 16 passed, 0 failed.
- `cargo test --locked --all-features --lib meta::challenge`: 118 passed, 0 failed.
- `cargo test --locked --all-features --lib mojeek`: 23 passed, 0 failed.
- `make docs-check`: exit 0 (`cargo doc --locked --all-features --no-deps`).
- `make packaging-check`: exit 0, including `planning-consistency: ok`, the workflow pin check over
  nine workflow files, the Eggpack qualifier contract over 25 producer jobs and 7 targets, and the
  egress qualification contract.
- `git diff --check`: clean.

All new tests use loopback `httpmock` fixtures. No public upstream was contacted and no fresh live
traffic was generated for M032. Live qualification against a real ALTCHA deployment was not run and
is recorded below as a residual risk rather than claimed as evidence.

## Compatibility and migration evidence

- No MCP tool, argument, or advertised-schema change. The canonical advertised inventory is
  unchanged and the byte-pinned advertisement still matches.
- No configuration key is added, removed, or renamed, and no shipped default changes.
  `[search.challenge].enabled` remains the single opt-in and remains off by default.
- `Cargo.toml` is unchanged, so no dependency moved. `scraper` and `url` were already direct
  dependencies.
- The refusal vocabulary in `src/meta/challenge/ledger.rs` keeps its ten keys. No new bound name
  was introduced.
- The `repo_fetch` response and the CLI JSON payload are unchanged by M032; they already carried
  `manual_interaction_required` and `structured_warnings` from M031.
- With the opt-in off, every surface behaves exactly as before: the disabled-solver tests assert a
  marked response, no challenge request, and no replay.

## Security evidence

- The permitted class did not widen. ADR-0009's prohibition list is unchanged and unexercised: no
  interactive CAPTCHA, OCR, solving service, credential bypass, proxy rotation, or user-agent
  spoofing was added. The only scheme solved on the fetch path is the already-evaluated ALTCHA
  PBKDF2 family.
- A published endpoint is resolved with `Url::join` against the challenge page's own URL, never by
  concatenating page text into a request URL. The resolved target must keep the page's scheme,
  carry no credentials, no query, and no fragment, share the page's origin, and match the fixed
  allowed path shape. Every one of those checks runs before a request is issued.
- The cross-origin property is not merely asserted case by case: a `proptest` over arbitrary
  published values proves that a derived endpoint is never cross-origin.
- One solve and one replay per fetch call, inside `max_attempts`, `timeout_ms`, and the call
  deadline. The replayed cookie is attached on the first hop only.
- The published PBKDF2 `cost` is the work factor of one counter iteration, and it is the one
  published parameter that can overrun the wall-clock ceiling on its own. A cost above
  `MAX_PBKDF2_COST` is refused as `malformed parameters` rather than started, so an attacker
  cannot pin a worker with an unbounded derivation.
- A refused solve, a malformed payload, a cross-origin endpoint, an exhausted deadline, or a replay
  that is itself a challenge leaves the original response exactly as it was.
- No cookie, salt, nonce, derived key, challenge payload, or pass token reaches a log, an error, a
  warning, a `Debug` output, or a fixture. `web_fetch_solved_altcha_payload_carries_no_cookie_material`
  asserts that the pass cookie, the cookie name, and the published key prefix are all absent from the
  response payload.
- The fetch layer remains free of the solver, so no bounded-body, SSRF, redirect, or sanitization
  rule was bypassed to reach a challenge solve.

## Observable behaviour changes, recorded explicitly

1. **A published ALTCHA challenge whose PBKDF2 `cost` exceeds `MAX_PBKDF2_COST` (1,000,000) is
   refused as `malformed parameters` instead of being solved.** The bound lives in the shared
   parser, so it applies to the engine-side solver too: the Mojeek path now refuses a cost above
   1,000,000. Mojeek's measured cost is 8,000, so no live behaviour changes. This is the only change
   to an existing solver behavior, and it is a strict tightening.
2. **A general fetch now attempts an ALTCHA solve when the opt-in is enabled.** A page that was
   previously marked `fetch_challenge_page` and returned as a stub may now be solved and replayed
   when the challenge fits the ceilings. With the opt-in off, the marked response is unchanged.
3. **`docs/config.md` no longer claims that `batch_fetch` does not solve challenges.** That claim
   was stale after M031 routed batch web items through the shared web-fetch handler. M031 corrected
   `architecture/fetch.md` but left the operator documentation stale; M032 corrected it, and the
   statement now matches the proven behavior.

## Known limitations and residual risk

- **The derivation needs an `altcha-widget` element.** A wall that publishes its challenge
  another way — a server-rendered form that posts the payload to its own action, for instance — is
  detected and marked but not solved. The classical application form is out of scope by design: its
  action is not an ALTCHA verification endpoint, and posting to an arbitrary form action risks a
  state change.
- **The pass cookie name is fixed to `chllg`.** A deployment that issues its pass cookie under a
  different name is refused as `pass cookie absent`. The name was not generalised because M032
  scoped the endpoint derivation and the solve, and an unexpected name costs a bounded refusal
  rather than a wrong replay.
- **Only the v2 challenge shape is parsed.** A legacy v1 document — `challenge`, `salt`, and
  `algorithm` at the top level — is refused as `malformed parameters`.
- **The endpoint path shape is fixed.** A challenge endpoint must end in exactly `challenge` and a
  verification endpoint in exactly `verify`, in at most four segments. A deployment that publishes
  `/captcha/pow` or `/captcha/challenge.json` is refused as `unsupported scheme`.
- **The verification body format is unchanged.** The solver still posts a hand-built
  `multipart/form-data` body with the `altcha` field, matching the SearXNG reference
  implementation and the Mojeek deployment. A deployment whose verification route accepts only
  `application/json` would refuse the submission, which the solver reports as `pass cookie
  absent`.
- **`max_difficulty` does not gate ALTCHA.** That is by design, is stated in the operator
  documentation, and follows from ALTCHA publishing no difficulty.
- **The evidence is loopback-fixture evidence.** It proves the contract, not live upstream
  behaviour. A live ALTCHA deployment may publish a shape this rule refuses, or a cost above the
  ceiling, and the rule would then decline the solve rather than fail open.
- **M033 is still open.** A fetch-issued pass cookie is still never reused; every challenged fetch
  pays a full solve.
- **Anubis `argon2id` and `hashx` remain `unsupported scheme`**, by ADR-0013 and ADR-0017
  clause 10, with their own ADR still owed.

## Recommendation

Closed. M032 meets every acceptance criterion on the exact candidate above. The closure commit
carries this record, the plan, the roadmap status, and the registry row together. M033 remains open
and registered.
