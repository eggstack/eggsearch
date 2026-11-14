# M023 — Fetch-Side Anubis Solve and Replay

Status: closed
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M023
Primary class: capability
Baseline: `e856468` (the commit that closed M013/M016/M022)
References: ADR-0009; ADR-0010; `architecture/fetch.md` § Challenge pages on the HTTP path; `architecture/engines.md` § Challenge solving; `src/fetch/client.rs`; `src/fetch/negotiate.rs`; `src/mcp/tools/web_fetch.rs`; `src/meta/challenge/anubis.rs`; `docs/config.md` § Challenge Solving; `docs/tool-matrix.md`.
Dependencies: hard M022 (challenge detection) and M011/M013/M016 (the bounded Anubis solver). Interface to M014 (pass-cookie lifetime) for the reuse window of a fetch-issued cookie; this milestone does not persist it beyond the one replay.

## Objective

When the operator has opted into bounded challenge solving, `web_fetch` should get past an Anubis wall instead of returning the wall as a marked stub. The existing solver is reused unchanged: detect the Anubis page, solve the published challenge once inside the configured ceilings, replay the same request once with the `spchal-auth` pass cookie, and return the real document. If solving is disabled, refused, or still challenged on replay, M022's marking remains.

Measured feasibility (2026-11-13, release build, the probe used the shipped solver verbatim with the honest `eggsearch` user agent):

```
first   status=200 body_len=22044 challenge=true cookies_len=170
solved  attempts=4963858
replay  status=200 len=238715 serp_payload=true result_titles=11
```

The same URL with the engine's browser-like default user agent is served the SERP immediately, which is why this must be solved rather than fixed by changing the fetch user agent: M022 recorded the wall, and the sanctioned ADR-0010 mechanism is the solver.

## Non-goals

- No user-agent change, no browser-UA fallback, no per-request header switching. The fetch client sends the configured user agent exactly as today.
- No new configuration key. The operator's existing `[search.challenge].enabled` is the opt-in for bounded published proof-of-work, and it now covers challenge pages reached through `web_fetch` as well as search upstreams.
- No browser-path change: `render: "browser"` keeps its current fail-closed behaviour for interactive challenges.
- No `batch_fetch` wiring in this milestone (recorded residual), no ALTCHA or other family on the fetch path, no solver coverage change, no budget/default change.
- No persistence: the pass cookie lives only for the single replay in that call. Cross-call reuse on the fetch path is not introduced.
- No retry loop: at most one solve and one replay per fetch.
- No claim that a solved fetch is a general anti-bot bypass: it applies only when the page publishes an Anubis challenge the solver already supports.

## Invariants that cannot regress

- `src/fetch/**` stays independent of `src/meta/**`: the solve is orchestrated in `src/mcp/tools/web_fetch.rs`, which may depend on both. No `crate::meta` import is added under `src/fetch/`.
- Ten MCP tools, no argument change, no tool-surface fingerprint change; the only new response signal is the absence of `fetch_challenge_page` when the replay succeeded.
- Disabled, refused, above-ceiling, malformed, or replay-still-challenged all keep today's M022 behaviour: the bounded body, `manual_interaction_required: true`, one `fetch_challenge_page` warning, no leaked cookie.
- One solve and one replay, both inside `max_difficulty`, `max_attempts`, `timeout_ms`, and the call deadline; a second challenge page is never re-solved in the same call.
- Cookies, nonces, salts, challenge ids, and the pass token never appear in logs, errors, warnings, `Debug`, or fixtures.
- The final returned response is the one stored in the tool cache; a cached challenge page can never shadow a successful replay.
- Every request still runs through the existing SSRF, redirect, origin, bounded-body, sanitization, and trust-framing pipeline.

## Expected production changes

1. **Capture the challenge cookies.** `FetchClient::fetch_once` already owns the raw response headers. Return the joined `name=value` pairs from every `Set-Cookie` header (the same form `anubis::challenge_cookies` produces) alongside the body, and carry them on an internal `challenge_cookies: Option<String>` field of `WebFetchResponse` that is `#[serde(skip)]` and `#[schemars(skip)]`. No serialized field is added.
2. **One cookie-bearing request path.** Add `FetchClient::fetch_with_cookie(..., cookie: Option<&str>)`; the existing `fetch` and `fetch_with_origin_controller` delegate with `None`. The cookie is attached only on the first hop of the replay, exactly like the conditional cache headers, so a redirect cannot forward it cross-origin. Cache keys and negotiation are unchanged.
3. **Tool orchestration.** In `src/mcp/tools/web_fetch.rs`, after the HTTP fetch returns and before the payload is assembled: if `crate::fetch::challenge::detect` reports `Anubis` and `state.config.search.challenge.enabled`, build the meta HTTP client with the configured fetch user agent, derive the origin from the final URL, and call `crate::meta::challenge::anubis::solve_and_verify` with the bounded body, the request URL, the captured cookies, the remaining call deadline, and a `SolveBudget` from the configured `max_attempts`, `max_difficulty`, and `timeout_ms`. On `Solved`, call `fetch_with_cookie` once with `spchal-auth=<value>` and use the replay response when the replay is not itself a detected challenge. On `Refused`, `Err`, or a challenged replay, keep the original response unchanged.
4. **No new warning when solved.** A successful replay simply returns the document with `manual_interaction_required: false` and no `fetch_challenge_page`. Provenance is visible through `transport: "http"` and the real content.
5. **Tests.** Mock-transport coverage for: a solved Anubis page returns the real document with the pass request hit once and no challenge warning; `enabled = false` keeps the mark with zero pass requests; a budget refusal keeps the mark with zero pass requests; a replay that is still challenged keeps the mark and performs exactly one solve. A canary test asserts no cookie value appears in the payload or warnings.

## Ordered work packages

1. `FetchClient` cookie capture and `fetch_with_cookie`, with a unit test that the replay sends the header on the first hop only.
2. Tool orchestration behind `[search.challenge].enabled`, with the four mock flows.
3. Docs: `architecture/fetch.md` § Challenge pages on the HTTP path, `architecture/engines.md` § Challenge solving, `docs/config.md` § Challenge Solving, `docs/tool-matrix.md`, `CHANGELOG.md`.
4. Closure record and live record in `tests/fixtures/engines/README.md`.

## Verification

```bash
cargo fmt --all
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features --test web_fetch_integration
cargo test --locked --all-features --test static_guards
cargo test --locked --all-features
```

Live, recorded rather than asserted: `web_fetch` against `https://www.startpage.com/search?q=rust+programming+language` with `[search.challenge] enabled = true`, `max_difficulty = 6`, `max_attempts = 50000000`, `timeout_ms = 15000`, recording the wall time, candidate count, and whether the returned text holds the `UIStartpage.AppSerp` payload.

## Acceptance and stop conditions

- With solving enabled, a published Anubis challenge is solved once and the replay's document is returned; the payload has no `fetch_challenge_page` warning and `manual_interaction_required` is false.
- With solving disabled, one request is made, the mark is unchanged, and no solve or replay is attempted.
- A refusal (attempts, wall clock, or difficulty ceiling) leaves the original marked response and performs no replay.
- A replay that is still a challenge leaves the marked response; the call never loops.
- No cookie value appears in the payload, warnings, errors, or `Debug`.
- Stop and return for a decision if the challenge page cannot be parsed by the existing solver, if the replay would need a redirect-carrying cookie, or if the work needs a new configuration key or a fetch-side dependency on `src/meta`.
