# M051 — Live-Surface Reliability and Robustness Correctives

Status: closed
Date: 2026-11-25
Closed: 2026-11-26
Subsystem: agentic retrieval integrity
Milestone class: correctness (reliability and robustness)
Baseline: `8fe39cd` on `docs/user-facing-polish-20261123`
Closure: `plans/closure/agentic-retrieval-integrity/051-status.md`
Decision authority: ADR-0006 (keyless retrieval is first-class); ADR-0019 (bounded challenge solving and browser escalation); `architecture/engines.md` § Scraper Fidelity; `docs/tool-matrix.md` error taxonomy; M049 evidence (Mojeek policy); M050 closure record

References: `src/meta/probe.rs`; `src/meta/provider_diagnostics.rs`;
`src/mcp/tools/provider_status.rs`; `src/meta/adapter/mod.rs`;
`src/meta/adapter/error.rs`; `src/fetch/origin.rs`; `src/fetch/classify.rs`;
`src/mcp/tools/web_fetch.rs`; `src/core/config.rs`;
`src/mcp/tools/repo_map.rs`; `docs/provider-setup.md`;
`docs/features.md`; `docs/tool-matrix.md`; `architecture/engines.md`;
`architecture/testing.md`; `skills/eggsearch-dev/SKILL.md`.

## Objective

Close the reliability and robustness gaps confirmed by the post-M050 review
of the deferred areas. This is a corrective program, not a feature milestone:
it adds no tool, alias, provider, or default-set change.

The review also corrects an earlier measurement error. Every pre-M050 live
run used the debug binary, where CPU-bound proof-of-work is far slower than
production. Re-measured on a release binary, Startpage and Mojeek both solve;
the real remaining issues are probe fidelity, cold-solve budgeting, rate-limit
cooldowns, a JavaScript-gate classification gap, parse diagnostics, one
contradictory `repo_map` warning, and a configuration-default trap.

## Evidence (release binary, 2026-11-25, this egress)

| Observation | Measurement |
|---|---|
| Probe uses a fixed 5 s budget (`PROBE_PER_PROVIDER_TIMEOUT_MS`, `src/meta/probe.rs:23`) while searches allow 8 s (`[search].timeout_ms`) | Probe run 1: `yahoo` timeout at 5002 ms, `brave` success at 4786 ms. Searches: `yahoo` 4.674 s, `brave` 2.9–6.9 s |
| Probe failures increment the same health counters as searches (`record_failure`, `src/meta/provider_diagnostics.rs:278`; threshold 3, timeout cooldown 15 s) | Three consecutive probes can enter cooldown from diagnostic traffic alone; `docs/provider-setup.md:812` documents the 5 s budget and the health update |
| Caller `timeout_ms` cannot extend the budget: `effective_timeout = min(caller, config)` (`src/meta/adapter/mod.rs:375`) | `timeout_ms: 20000` under an 8 s config still failed at 8.03 s |
| A cold challenge solve is marginal against the default budget | Startpage 8 s config: first call failed at 8.03 s, next call succeeded in 0.44 s from the cached pass cookie. With `[search].timeout_ms = 30000`: two cold processes succeeded at 5.04 s and 5.28 s |
| Release solves both challenge walls | Startpage cold 5.0–5.3 s; Mojeek 1.69 s (ALTCHA), 0.0 s cached |
| `Retry-After` is parsed only in the fetch layer (`src/fetch/origin.rs:383`); engine 429s use a fixed 60 s rate-limit cooldown | NVD returned 429 under a security fan-out; three isolated calls succeeded |
| `render: "auto"` misses short JavaScript gates (`is_javascript_shell`, `src/fetch/classify.rs:119`, requires `text_len < 50` or a root div/script ratio) | The JS fixture returned ~90 chars ("Either use a headless browser…") over HTTP in 0.35 s with no escalation; explicit `render: "browser"` serves it |
| Unknown HTML drift is reported as `ParseFailed` with no page fingerprint | Intermittent `[parse_error] engine 'duckduckgo' parse failed: result container 'div.result' missing` under `security_search` fan-out |
| `repo_map` emits the generic `no_native_tree_provider` notice beside a specific forge failure | M050 release check: `repo_ref_not_found` + the generic notice in the same response |
| `[search]` defaults are struct-level: `providers` is `#[serde(default)]` (`src/core/config.rs:203`) | `[search]` without `[search.providers]` disables every traditional provider and fails validation; partial `[search.challenge]`/`[search.routing]` snippets fail on missing fields |

Startpage is enabled but **not** in `default_providers` (`duckduckgo`,
`brave`, `yahoo`), and Mojeek is disabled by default. Neither affects an
ordinary search; that is why the provider packages below do not change the
default set.

## Contract

- The nine advertised tools, three callable aliases, their schemas, and the
  stable error-code vocabulary are unchanged. Additive diagnostic fields are
  permitted; new wire strings require the owning doc to name them.
- Probes stay diagnostic. They report success, failure class, latency, and
  skip codes, but they never write routing health (consecutive failures or
  cooldown). Only real searches do.
- A caller's `timeout_ms` continues to shorten, never extend, the configured
  budget. Documentation must say so where the field is described.
- A cold challenged provider may cost one extra attempt; the pass cookie must
  survive a deadline miss so the retry does not solve twice. No solve may run
  past the request deadline.
- A 429 with `Retry-After` sets a cooldown matching the header within a
  documented clamp; without the header the existing class default applies.
- `render: "http_only"` never renders. Interactive challenges keep precedence
  over any new JavaScript-shell marker.
- Mode Off still performs zero remote dispatch; the local-only paths are
  unchanged.
- Every new bound is explicit and sanitized; no page body is echoed into a
  model-facing payload beyond the existing caps.

## Work packages

### WP1 — Probe deadline and health isolation (S)

- Derive the per-provider probe deadline from `[search].timeout_ms` instead of
  the 5 s constant, and let `provider_status` pass it through
  `ProviderProbeRequest.timeout_per_provider_ms`.
- Stop mutating routing health from probe outcomes. Keep the outcome fields
  (`success`, `failure_class`, `latency_ms`) and the aggregate counts.
  If a probe timestamp is still wanted, add an additive `last_probe_at`.
- Update `docs/provider-setup.md:812` (probe policy) and the cooldown table so
  the separation is stated once.
- Tests: a mock engine sleeping 6 s under an 8 s search timeout → probe
  succeeds; three probe timeouts leave `is_in_cooldown` false and a routed
  search still selects the provider; an explicit override is honored.
- Acceptance: a healthy provider is never reported as a probe timeout at the
  default budget, and diagnostics cannot cause a cooldown.

### WP2 — Cold challenge-solve contract (S)

- Prove and encode the observed behavior: a solve that completes after the
  dispatch deadline still caches the pass cookie, so the next call replays in
  under a second. If the cache write can be cancelled, move it before the
  replay await.
- Document the cold-start expectation (one possible retry) and the warm-up
  path (`provider_status(probe = true)` after WP1) in
  `docs/provider-setup.md` and `architecture/engines.md`.
- Tests: a mock Anubis wall whose solve outlasts the request budget → the first
  call fails as `blocked`/timeout, the second call succeeds without a second
  solve (solve mock hit count stays 1); a warm cookie keeps the first call
  inside budget.
- Acceptance: at most one cold attempt per challenged provider; no unbounded
  solve. **Stop:** if the cookie is not cached after a deadline miss, do not
  redesign dispatch here; register a separate solve-lifecycle milestone.

### WP3 — Rate-limit cooldown from `Retry-After` (M)

- Carry the header on the engine error. Prefer an additive
  `EngineError::RateLimited { engine, retry_after_ms }` (or an optional field on
  `BadStatus`) produced by the shared response path, reusing
  `parse_retry_after` from the fetch layer.
- Record a cooldown from the parsed value with an explicit clamp (suggested
  300 s), falling back to the 60 s class default when absent or malformed.
  Start with NVD; the shared path applies to any engine that sends the header.
- Tests: `httpmock` 429 with `Retry-After: 7` → `failure_class = rate_limited`
  and cooldown ≈ 7 s; malformed header → 60 s; no retry within the same
  request.
- Acceptance: a 429 cooldown never exceeds the documented clamp or ignores a
  valid header. **Stop:** if the error-type change forces unrelated engine
  rewrites, narrow to an adapter-side header read for NVD only.

### WP4 — Auto render for short JavaScript gates (S)

- Extend the classifier with a bounded explicit-instruction marker set
  (`enable javascript`, `javascript is required`, `use a headless browser`)
  when `text_len < 200`, after the interactive-challenge and
  non-interactive-verification checks.
- Add a captured fixture for the live page and a negative case: a long
  documentation page that merely mentions JavaScript stays
  `UsefulContent`.
- Tests: classifier unit cases; a fetch integration test with a local page
  under `render: "auto"` returns `transport: "browser"`; `http_only` keeps the
  HTTP result.
- Acceptance: the captured gate escalates once; no over-escalation on the
  negative fixture.

### WP5 — Parse-failure fingerprint (S)

- Append a bounded fingerprint to `EngineError::ParseFailed`: the result
  container name, the observed `<title>` when present (sanitized, ≤ 80 chars),
  and a 16-hex body digest. No page body text enters the payload.
- Route it through the existing bounded warning path; keep
  `architecture/engines.md` § Scraper Fidelity authoritative.
- Tests: a shifted synthetic page yields a stable digest and a bounded
  message; control characters are stripped; two different bodies produce
  different digests.
- Acceptance: a parse failure is diagnosable from logs alone, and the message
  stays within the existing bound.

### WP6 — `repo_map` specific-over-generic warning (S)

- When the forge branch adds a specific structured warning, remove the
  fallback's generic `no_native_tree_provider` notice; keep the generic notice
  only when no specific failure exists (for example an unsupported host).
- Correct `repo_map_fallback_mode_warns_no_native_provider`: its current
  premise passes `host: github` and asserts the generic notice, which is the
  behavior this package removes.
- Tests: explicit host + 404 → `repo_ref_not_found` and no generic notice;
  inferred host + 404 → guidance plus the specific code; unsupported host →
  generic notice retained.
- Acceptance: one failure, one explanation.

### WP7 — Field-level `[search]` defaults (S)

- Add `default_provider_map()` and use it for
  `#[serde(default = "default_provider_map")]` on `SearchSection.providers`;
  add field-level defaults for `mode`, `default_max_results`,
  `max_results_cap`, `max_query_chars`, and `timeout_ms`.
- An explicitly present empty `[search.providers]` table stays authoritative
  and empty; only an absent table inherits the built-in set.
- Tests: `[search]` with only `timeout_ms` keeps the built-in providers;
  `[search.routing]` alone parses; an explicit empty providers table still
  yields the existing validation error; serialized defaults are unchanged.
- Acceptance: every documented snippet loads standalone, and an explicit
  empty set is never silently repopulated.

### WP8 — Release-mode live verification target (S)

- Add `make live-release-smoke` (or an equivalent documented command) that
  runs the existing ignored MCP smoke optimized:
  `cargo test --release --locked --features live-mcp-smoke --test mcp_http live_canonical_tools_and_aliases_work_over_mcp_http -- --ignored --nocapture`.
- Document in `architecture/testing.md`, `docs/test-inventory.md`, and
  `skills/eggsearch-dev/SKILL.md` that challenge-solver and PoW timings are
  only meaningful on a release build; the debug live smoke remains the wiring
  check.
- Acceptance: the target passes on this egress and the docs name it as the
  authoritative solver measurement.

## Ordering and dependencies

1. WP1 (probe), WP4 (auto render), WP7 (config) — independent, smallest blast
   radius, highest signal.
2. WP6 (repo_map) — independent, closes an M050 residual.
3. WP2 (challenge contract) — after WP1 so the warm-up path is usable.
4. WP3 (Retry-After) — after WP1 so health-recording changes land once.
5. WP5 (parse fingerprint) — independent; can run in parallel with WP3.
6. WP8 (release target) — last, so it verifies the preceding packages.

Dependencies are soft except: WP2's warm-up measurement uses WP1's probe
budget, and WP3 touches the health-recording seam that WP1 also edits; land
WP1 first to avoid conflicting rewrites.

## Verification

Focused, per package:

- WP1: `cargo test --locked --all-features --lib meta::probe` and
  `--test provider_routing`.
- WP2: a new mock-wall case in `tests/web_search_integration.rs` or the
  engine unit tests.
- WP3: `httpmock` cases in `tests/provider_routing.rs` /
  `tests/mcp_tools.rs`.
- WP4: `cargo test --locked --all-features --lib fetch::classify` plus a
  `tests/web_fetch_integration.rs` case.
- WP5: `tests/engine_parse_conformance.rs` and the engine unit tests.
- WP6: `cargo test --locked --features mock --test repo_workflow repo_map_`.
- WP7: `cargo test --locked --features mock --test config_validation`.
- WP8: `make live-release-smoke`.

Broad: `RUSTUP_TOOLCHAIN=1.89.0 make check`, `make docs-check`,
`make packaging-check`.

Live: rebuild `cargo build --release --locked --features documents`, repeat the
review's table (probe, cold/warm challenge, 429, auto render), and record it in
the closure record.

## Acceptance and stop conditions

Milestone acceptance:

- Probe outcomes match search reality at the default budget and never enter a
  cooldown.
- A cold challenged provider costs at most one retry and never solves twice.
- A 429 with `Retry-After` sets a matching, clamped cooldown.
- The captured JavaScript gate escalates under `auto`; the negative fixture
  does not.
- A parse failure carries a bounded fingerprint.
- `repo_map` never pairs a specific failure with the generic notice.
- Documented `[search]` snippets parse standalone while an explicit empty
  provider table stays empty.
- `make check`, `make docs-check`, and `make packaging-check` pass.

Stop and split rules:

- A package that grows beyond roughly ten source files, needs a new tool or
  argument, or needs a dispatcher redesign is removed from this milestone and
  registered as its own plan. Do not silently widen the contract.
- WP2 stops rather than redesigning dispatch.
- WP3 stops and narrows to NVD if the error type spreads.
- WP1 stops if probe isolation would change provider-selection semantics;
  diagnostics must remain advisory.

## Non-goals

- A Mojeek API or compliance decision. M049's evidence stands: `/search` is
  robots-disallowed and the API is the supported route; the engine stays
  disabled by default.
- Promoting Startpage into `default_providers`, or changing the default
  provider set at all.
- Multi-host `repo_map` inference. The GitHub default plus the M050 guidance
  is the accepted terminal state; callers pass `host` for other forges.
- Dispatcher straggler cutoff or a per-provider result deadline. It needs a
  completeness-policy decision and its own measurement; register separately if
  tail latency becomes a priority.
- Broad reclassification of missing result containers as `blocked`. A marker
  is added only from a captured page, under the existing four-outcome rule.
- Paid providers, proxy rotation, or user-agent spoofing.

## Risks

- Removing probe health writes changes a documented diagnostic behavior; the
  doc update and tests must land with it.
- A `Retry-After` error field spreads across engine constructors; keep the
  change additive and start with NVD.
- Short-page JavaScript markers can over-escalate; the 200-char bound and the
  interactive-challenge precedence are the guard.
- Field-level config defaults could mask a deliberate empty provider set; the
  present-but-empty rule and its test are the guard.
- Release-mode live runs are slow (optimized build, network); they stay opt-in
  and outside `make check`.

## Closure evidence

Per-package focused test output, the release live table, the docs diff, the
`make check` / `make docs-check` / `make packaging-check` results, and the
candidate SHA, recorded in
`plans/closure/agentic-retrieval-integrity/051-status.md`.
