# M010 — Bounded Challenge-Solving Framework and ALTCHA (Mojeek)

Status: closed
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M010
Primary class: capability
Baseline: `d751bd7471a647e651ee41b8e51199c66d04d01a` (the worktree also carries the uncommitted retrieval-defaults change from the prior turn; re-read `git status` before starting)
References: ADR-0009; `architecture/engines.md` § Challenge solving; `src/meta/engines/mod.rs`; `src/meta/engines/mojeek.rs`; `src/core/config.rs`; `src/meta/engines/error.rs`; `src/meta/adapter/error.rs`.
Dependencies: none. M011 builds on this milestone's framework; M012 is independent of it.

## Objective

Add one shared, bounded, opt-in challenge solver to the engine layer and use it to restore `mojeek` as a working keyless upstream, without changing what happens when the capability is disabled or a budget is exhausted.

## Non-goals

- No interactive CAPTCHA, visual puzzle, OCR, third-party or human solving service, proxy rotation, user-agent spoofing, or retry widening.
- No Anubis/Startpage work; that is M011, which extends this milestone's framework.
- No new MCP tool, no new `provider_status` field, no change to `error_class` wire strings.
- No default-on behaviour. The capability ships disabled.
- No persistence of pass cookies or challenge payloads to disk, fixtures, logs, or diagnostics.

## Invariants that cannot regress

- Ten MCP tools, stable FNV-1a IDs, sanitization tiers, trust framing, and deadline bounds are unchanged.
- A disabled or exhausted solver still produces `EngineError::Blocked` → `error_class: blocked` → `FailureClass::Blocked`, never `Ok(vec![])`.
- A solved request runs through the existing parser and `finalize_scrape()`; a solved but unparseable page is `ParseFailed`, not `Blocked`.
- Every upstream request stays behind `read_bounded_body`/`read_bounded_response` and existing caps.
- Solving is attempted at most once per provider per search request and at most once per `cookie_ttl_seconds`.
- Cookie values never appear in logs, fixtures, `provider_status`, or error messages.

## Expected production changes

1. **Configuration.** New `[search.challenge]` table: `enabled` (default `false`), `max_difficulty` (default `5`, validated `1..=10`), `max_attempts` (default `2_000_000`, validated `1..=100_000_000`), `timeout_ms` (default `2000`, validated `100..=30_000`), `cookie_ttl_seconds` (default `240`, validated `0..=3600`, `0` disables reuse). Wired through `AppConfig` and `SearchSection`, with the same three-defaults-agree discipline used by `fetch.pdf_enabled`.
2. **Shared framework.** New `src/meta/challenge/mod.rs`: a `SolveBudget` value (attempts, wall clock, difficulty ceiling), a `SolveOutcome` (solved with cookie / refused with a named bound / unsupported scheme), and a process-local bounded `PassCookieCache` keyed by provider id with a TTL and an entry cap. The cache is never persisted and never shared across providers.
3. **ALTCHA solver.** New `src/meta/challenge/altcha.rs`: parse the published parameters (`algorithm`, `cost`, `keyLength`, `keyPrefix`, `salt`, `nonce`), search `counter` from zero with `PBKDF2-HMAC-SHA256(password = nonce ‖ counter_be32, salt, cost, keyLength)` until the derived key starts with `keyPrefix`, bounded by `max_attempts` and `timeout_ms`; submit
   `base64({"challenge": <challenge>, "solution": {"counter": <c>, "derivedKey": <hex>, "time": 150}})`
   as the multipart field `altcha` to `POST /captcha/verify`; capture the `chllg` cookie from the response. `time` is a fixed constant, not a random value, so a recorded challenge always produces the same answer.
4. **Mojeek integration.** `src/meta/engines/mojeek.rs` detects the challenge on a search response (existing `blocked_markers` already cover the CAPTCHA page), obtains a pass cookie through the framework when enabled, replays it on one repeat request, and otherwise returns the existing `Blocked` result with a reason naming the bound.
5. **Dependencies.** Add offline, pure-computation crates for PBKDF2 and base64 (`pbkdf2` and `base64`). `sha2` is already a direct dependency. The dependency policy check must pass; no network, TLS, browser, or service dependency is permitted.

## Storage, protocol, and compatibility effects

No schema change and no wire-string change. `[search.challenge]` is additive configuration; a config file without it keeps today's behaviour exactly. `SearchSection` field additions are serde-defaulted so existing configs still parse. The pass-cookie cache is process memory only and dies with the process.

## Ordered work packages

1. Config table, defaults, validation, and the docs row; prove three default sites agree.
2. `challenge/mod.rs`: budget type, outcome type, cache with cap and TTL, plus unit tests for expiry and eviction.
3. `challenge/altcha.rs`: parse, solve, submit, capture cookie; deterministic vector tests using fixed parameters with a known counter, plus a negative vector that must not match.
4. `mojeek.rs` wiring: detect → solve when enabled → replay once → parse; disabled path unchanged.
5. Budget-exhaustion and fail-closed paths with their tests (`max_attempts`, `timeout_ms`, unsupported `algorithm`, malformed parameters).
6. Mock-transport flow test: challenge response → verify → result page, asserting the cookie is sent once and the search yields rows.
7. Docs: `docs/config.md`, `docs/provider-setup.md` (Mojeek section), `docs/features.md`, `architecture/config.md`, `architecture/engines.md` cross-reference, `docs/test-inventory.md`, `CHANGELOG.md`.

## Verification

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features --test engine_parse_conformance
cargo test --locked --all-features --test web_search_integration
cargo test --locked --all-features --test static_guards
./packaging/check-dependency-policy.sh
./packaging/check-repo-hygiene.sh
python3 packaging/check-planning-consistency.py
cargo test --locked --all-features
```

Live, opt-in, recorded rather than asserted:

```bash
eggsearch --config /tmp/challenge.toml search "rust programming language" --providers mojeek --json
```

with `[search.challenge] enabled = true` and `[search.providers] mojeek = true`. Record the observed counter, wall time, and status in `tests/fixtures/engines/README.md`.

## Acceptance and stop conditions

- `enabled = false`: byte-for-byte the current behaviour, still `blocked`, no extra request.
- `enabled = true` and a legitimate Mojeek challenge: a live search returns parsed rows; the solve stays inside `max_attempts` and `timeout_ms`; the cookie is reused within the TTL and re-solved after it.
- Any ceiling breach returns `blocked` with a reason naming the ceiling, within the search deadline.
- No cookie, nonce, salt, or derived key appears in logs, fixtures, or diagnostics.
- Stop and return for a decision if the upstream changes method or publishes a scheme other than `PBKDF2/SHA-256`; do not add a new scheme inside this milestone.

## Closure evidence

`plans/closure/agentic-retrieval-integrity/010-status.md` must record: the implementation candidate SHA, the exact test commands and results, measured solve cost against the configured ceilings, the live verification record with timestamps, the disabled-path equivalence evidence, and residual risk (upstream terms, parameter drift, and difficulty variance).
