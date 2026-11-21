# M031 Fetch-Path Challenge Solve Parity — Closure Evidence

Status: closed

Source plan: `plans/implementation/agentic-retrieval-integrity/031-fetch-path-challenge-solve-parity.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Architecture decision: `plans/adrs/ADR-0017-fetch-path-challenge-resolution.md` (accepted 2026-11-19)

## Scope

M031 moved the bounded Anubis solve out of the MCP web-fetch tool into one shared module and extended
it to the fetch surfaces that could not previously reach it: the remote `repo_fetch` path and the CLI
`eggsearch fetch` command. It proved the recorded `batch_fetch` residual with a test and corrected
the stale documentation that claimed the batch path did not solve.

## Exact code candidate

`7f7219beca61bce2247e7cf9bbaad8ef141467b6` — the implementation commit, carrying the shared module,
the three consumer wirings, the new tests, the two static guards, and the documentation set. Every
command recorded below ran against this exact candidate. The closure commit carries this record, the
plan status, the roadmap status, and the registry row together.

## Requirement-to-evidence mapping

| # | Acceptance criterion | Evidence |
|---|---|---|
| 1 | A batch web item against a published Anubis wall returns the replayed document with `manual_interaction_required: false` and no `fetch_challenge_page` warning, proven by a behavioral test | `tests/batch_fetch_retrieval.rs::batch_inherits_the_fetch_side_anubis_solve`. The loopback fixture asserts the wall is fetched once, the pass-challenge endpoint is called once, the replay is called once with the issued cookie, the item reports `manual_interaction_required: false`, no challenge warning survives, and the returned title is the replayed document. Suite: 23 passed, 0 failed. |
| 2 | `architecture/fetch.md` states the verified batch behavior and no longer claims `batch_fetch` does not solve | The section "Challenge pages on the HTTP path" now states that `batch_fetch` inherits solve behavior through the shared web-fetch handler, and it names `src/challenge_replay/` as the owner of the solve sequence. |
| 3 | `repo_fetch`'s remote path classifies a published Anubis wall and, with the opt-in enabled, returns the replayed document; with the opt-in disabled it returns the marked wall as before | `tests/repo_workflow.rs::repo_fetch_remote_solves_a_published_anubis_challenge`. The fixture serves the wall as `text/html` at the raw-source path, answers the pass-challenge endpoint, and asserts the replay carries the issued cookie and the response text is the replayed source. Suite: 98 passed, 0 failed. |
| 4 | The CLI `eggsearch fetch` command classifies a published Anubis wall and returns the replayed document and its challenge fields; with the opt-in disabled its output is unchanged apart from the two added fields | `src/commands/fetch.rs::fetch_response_solves_a_published_anubis_challenge_when_enabled` and `::fetch_response_marks_the_challenge_when_solving_is_disabled`. The enabled case asserts one solve attempt, one replay with the issued cookie, `manual_interaction_required: false`, and the replayed document text; the disabled case asserts zero pass-challenge requests, `manual_interaction_required: true`, and a `fetch_challenge_page` structured warning in the JSON payload. Binary suite: 16 passed, 0 failed. |
| 5 | One implementation of detect, solve, replay, and classify serves all three surfaces, and a static guard fails closed if a second copy appears | `src/challenge_replay/mod.rs` owns the sequence. `tests/static_guards.rs::challenge_solve_orchestration_has_one_owner` fails closed if any file under `src/mcp/` or `src/commands/` calls `solve_and_verify`, `anubis::`, or `altcha::`, or if any file under `src/meta/` depends on the shared module. Suite: 64 passed, 0 failed. |
| 6 | No file under `src/fetch/**` imports the solver or the shared orchestration module | `tests/static_guards.rs::fetch_layer_never_imports_the_challenge_solver` walks every production file under `src/fetch/` and fails closed on `challenge_replay`, `crate::meta`, `anubis::`, `altcha::`, or `solve_and_verify`. It also asserts the walk covered at least twenty files so the guard cannot pass vacuously. |
| 7 | The MCP web family is unchanged: the same response on a solved wall, the same marked response on a refused or disabled solve, the same browser-first ordering, at most one solve and one replay | `tests/web_fetch_integration.rs`: 99 passed, 0 failed, 1 ignored — the same pre-change result. The pre-existing Anubis tests (`web_fetch_solves_a_published_anubis_challenge_and_returns_the_document`, `web_fetch_with_solving_disabled_marks_the_anubis_challenge`, `web_fetch_replays_a_default_anubis_pass_cookie`, `web_fetch_solved_anubis_payload_carries_no_cookie_material`, and the metarefresh case) pass unchanged. The browser-first ordering is preserved: the shared solve runs only when the `render: "auto"` escalation did not replace the payload. |
| 8 | No configuration key, default, dependency, refusal key, advertised tool, or advertised schema changed | `Cargo.toml` is unchanged, so no dependency moved. `src/core/config/challenge.rs` is unchanged, so `[search.challenge]` keeps its keys, bounds, and its off-by-default opt-in. The refusal vocabulary in `src/meta/challenge/ledger.rs` keeps its ten keys. The advertised tool inventory and every advertised schema are unchanged — see the pinned-surface evidence below. |
| 9 | The broad gate passes on the exact candidate, and every test that was run is recorded with its outcome | See Verification below. |

## Verification

Using the pinned Rust 1.89 toolchain on candidate `7f7219beca61bce2247e7cf9bbaad8ef141467b6`:

- `make check`: exit 0 — 85 suites, 6,374 tests, 0 failed. This includes fmt, clippy with `-D warnings`, the no-default-feature check, the all-features test run, repository hygiene, the dependency policy, and the packaging contract checks.
- `cargo test --locked --all-features --test web_fetch_integration`: 99 passed, 0 failed, 1 ignored.
- `cargo test --locked --all-features --test batch_fetch_retrieval`: 23 passed, 0 failed.
- `cargo test --locked --all-features --test repo_workflow`: 98 passed, 0 failed.
- `cargo test --locked --all-features --test static_guards`: 64 passed, 0 failed.
- `cargo test --locked --all-features --bin eggsearch`: 16 passed, 0 failed.
- `cargo test --locked --all-features --lib challenge_replay`: 3 passed, 0 failed.
- `make docs-check`: exit 0 (`cargo doc --locked --all-features --no-deps`).
- `make packaging-check`: exit 0, including `planning-consistency: ok`, the workflow pin check over nine workflow files, the Eggpack qualifier contract over 25 producer jobs and 7 targets, and the egress qualification contract.
- `git diff --check`: clean.

All new tests use loopback `httpmock` fixtures. No public upstream was contacted and no fresh live traffic was generated for M031.

## Compatibility and migration evidence

- No MCP tool, argument, or advertised-schema change. The canonical advertised inventory is unchanged.
- The `repo_fetch` response gains two additive signals: `manual_interaction_required`, always serialized, and the `fetch_challenge_page` structured warning, forwarded from the fetch layer into `structured_warnings`. The workspace path is unchanged and always reports `manual_interaction_required: false`.
- The CLI JSON payload gains `structured_warnings` and `manual_interaction_required`. The human-readable output gains one line, printed only when a challenge page was returned.
- `[search.challenge].enabled` remains the single opt-in and remains off by default. With it off, every surface behaves exactly as before, which the disabled-case CLI test and the unchanged `web_fetch` suite both assert.

## Security evidence

- The permitted class did not widen. ADR-0009's prohibition list is unchanged and unexercised: no interactive CAPTCHA, OCR, solving service, credential bypass, proxy rotation, or user-agent spoofing was added. The only scheme solved on the fetch path is the already-evaluated Anubis family.
- One solve and one replay per fetch call, inside `max_difficulty`, `max_attempts`, `timeout_ms`, and the call deadline. The replayed cookie is attached on the first hop only, so a redirect cannot forward it to another origin.
- A refused solve, a transport error, an exhausted deadline, a malformed challenge, or a replay that is itself a challenge leaves the original response exactly as it was.
- No cookie, nonce, salt, challenge id, or pass token is logged, serialized into a response, or written to a fixture. The pre-existing `web_fetch_solved_anubis_payload_carries_no_cookie_material` test still passes.
- The fetch layer remains free of the solver, so no bounded-body, SSRF, redirect, or sanitization rule was bypassed to reach a challenge solve.

## Observable behaviour changes, recorded explicitly

1. **The fetch-path solve ledger label is `fetch`, not `web_fetch`.** The shared module records every fetch-path solve under one label, so `provider_status.challenge_solver.providers` now shows `fetch` where it previously showed `web_fetch`. Search upstreams keep their own labels. No test or document depended on the old key, the block's advertised schema is an open object, and the change keeps the whole fetch path inside the ledger's four-slot cap.
2. **`repo_fetch` responses are wider** by the two additive signals above.
3. **The CLI JSON payload is wider** by the two additive signals above.

## Known limitations and residual risk

- **ALTCHA is still not solved on the fetch path.** The fetch layer detects an ALTCHA wall and marks it; the ALTCHA solver remains engine-coupled to fixed same-origin challenge and verify endpoints. That is M032.
- **A fetch-issued pass cookie is still never reused.** Every challenged fetch pays a full solve. That is M033.
- **Anubis `argon2id` and `hashx` remain `unsupported scheme`**, by ADR-0013 and ADR-0017 clause 10. Upstream v1.28.2 ships them disabled by default with stated intent to enable them by default later, so an upstream default flip remains an open coverage risk with its own ADR still owed.
- **Challenge detection fires only on an HTML representation.** A challenge served as `text/plain` is not classified, on every surface. This is a property of the shared fetch-layer detection and is unchanged by M031.
- **Browser-first ordering applies to the MCP web path only.** `repo_fetch` and the CLI have no browser escalation, so a challenge page they cannot solve is reported as marked rather than rendered.
- **The evidence is loopback-fixture evidence.** It proves the contract, not live upstream behaviour. Anubis difficulty is risk-scored per client, so a live wall may still exceed the operator's ceilings.
- **One first attempt was reverted during implementation.** Adding `manual_interaction_required` to the declared `repo_fetch` output schema in `src/mcp/output_schema.rs` broke the pinned advertised surface (`full_preset_advertisement_is_byte_identical_to_the_pinned_surface`). The change was reverted because the plan forbids an advertised-schema change; the permissive output schema already permits the field. The pin did its job.

## Recommendation

Closed. M031 meets every acceptance criterion on the exact candidate above. The closure commit carries this
record, the plan, the roadmap status, and the registry row together. M032 and M033 remain open and
registered.
