# Agentic Retrieval Integrity M051 — Live-Surface Reliability and Robustness Correctives — Status

Status: closed
Date: 2026-11-26
Milestone: M051 — Live-Surface Reliability and Robustness Correctives
Milestone class: correctness (reliability and robustness)
Candidate: working tree applied on `8fe39cd` (M051 change set; commit separate)
Baseline: `8fe39cd` on `docs/user-facing-polish-20261123`
Decision authority: ADR-0006 (keyless retrieval is first-class); ADR-0019 (bounded challenge solving and browser escalation); `architecture/engines.md` § Scraper Fidelity; `docs/tool-matrix.md` error taxonomy; M050 closure record

## Candidate and changes

The post-M050 review of the deliberately deferred areas produced eight bounded
packages. The candidate is the baseline plus this change set; it adds no tool,
alias, provider, argument, or default-set change, and keeps the advertised MCP
schemas byte-identical.

### WP1 — Probe deadline and health isolation

- `src/meta/probe.rs`: probes report success, failure class, http status, and
  latency only. Every `record_success`/`record_failure` write, including the
  aggregate-deadline path, is removed. `PROBE_PER_PROVIDER_TIMEOUT_MS` remains
  the fallback for bare requests.
- `src/mcp/tools/provider_status.rs`: both the sync and async paths pass
  `ProviderProbeRequest::timeout_per_provider_ms = Some([search].timeout_ms)`.
- `src/commands/doctor.rs`: the CLI probe passes the same configured budget.
- `tests/provider_probe_conformance.rs`: success and three-timeout cases now
  assert `Unknown` health and no cooldown; a 6 s engine succeeds under an 8 s
  budget; `provider_status` forwards the configured 3,210 ms budget; default
  routing still selects a provider after three probe timeouts.

### WP2 — Cold challenge-solve contract

- No production change was needed: `startpage.rs` already writes the pass
  cookie before the replay await, and `solve_timeout = min([search.challenge].timeout_ms,
  request timeout)`.
- `src/meta/engines/startpage_solve_tests.rs`: three encoded cases — a solve
  whose replay misses the budget still caches the cookie, so the second call
  succeeds with the solve-wall hit counts unchanged; a warm cookie keeps the
  first attempt inside a 600 ms budget; a solve never runs past the request
  budget and caches nothing when it cannot complete.
- `docs/provider-setup.md` and `architecture/engines.md`: one cold attempt,
  warm-up path, and the solve ceiling are documented.

### WP3 — Rate-limit cooldown from `Retry-After`

- `src/meta/engines/error.rs`: additive `EngineError::RateLimited {
  retry_after_ms }` plus `retry_after()`.
- `src/meta/engines/mod.rs` and `response.rs`: one header parser reuses
  `fetch::origin::parse_retry_after`; a marker-free `429` from the shared HTML
  path becomes `RateLimited`. `nvd.rs` emits the same variant.
- `src/meta/provider_diagnostics.rs`: `record_failure_with_retry_after` starts a
  cooldown immediately from a positive hint, clamped to 300 s; absent, zero, or
  malformed hints keep the 60 s class default and the 3-failure threshold.
  `adapter/status.rs` passes the hint through.
- Dispatch, security-search, probe, and render classification all map the new
  variant; `is_browser_gated` treats it as the gated `429` class.
- Tests: NVD `httpmock` 429 with `Retry-After: 7` and a malformed header;
  registry matching/clamping/threshold tests; an adapter-level search that
  enters a 7 s cooldown immediately.

### WP4 — Auto render for short JavaScript gates

- `src/fetch/classify.rs`: a page under 200 rendered characters that contains
  `enable javascript`, `javascript is required`, `requires javascript`, or
  `use a headless browser` is a JavaScript shell, checked after the
  interactive-challenge and non-interactive-verification markers.
- `tests/fixtures/fetch/js_gate.html` and `js_gate_negative.html` are the
  captured short gate and the negative long documentation page.
- `src/mcp/tools/tests.rs`: `auto` escalates the captured gate once to
  `transport: "browser"`; the long page and `http_only` never call the
  renderer; the classifier unit cases pin precedence and the 200-character
  bound.

### WP5 — Parse-failure fingerprint

- `src/meta/engines/mod.rs`: `finalize_scrape` appends
  `[title=<sanitized, <= 80 chars> digest=<16 hex>]` to `ParseFailed`. The
  digest is FNV-1a over the canonical node tree (document order, attributes
  sorted), because the HTML serializer's attribute order is not stable across
  parses. No page body text enters the message.
- Unit tests pin stability, difference across bodies, control-character
  stripping, the 80-character title bound, and the digest width.
- `tests/engine_parse_conformance.rs`: the seeded-drift case asserts the
  fingerprint and its determinism for all five scrapers.

### WP6 — `repo_map` specific-over-generic warning

- `src/mcp/tools/repo_map.rs`: the forge-failure branch removes the fallback's
  generic `no_native_tree_provider` string and structured notices, then records
  the specific failure once through the existing prefix table, so `warnings`
  and `structured_warnings` agree and no failure is duplicated.
- Tests: explicit host + 404 → `repo_ref_not_found` and no generic notice;
  inferred host + 404 → inference guidance plus the specific code; an
  unrecognized 500 → exactly one `no_native_tree_provider` entry naming
  `provider_unavailable`.
- The old `repo_map_fallback_mode_warns_no_native_provider` premise is
  corrected as required by the plan.

### WP7 — Field-level `[search]` defaults

- `src/core/config/search_defaults.rs` (new) owns the field defaults;
  `SearchSection` uses them for `mode`, `default_max_results`,
  `max_results_cap`, `max_query_chars`, `timeout_ms`, `default_providers`, and
  `providers`. `impl Default` calls the same functions.
- An explicitly present empty `[search.providers]` stays empty and still fails
  live-mode validation.
- Tests: `[search]` with only `timeout_ms` keeps the built-ins; `[search.routing]`
  alone parses and validates; an explicit empty provider table is not
  repopulated; serialized defaults round-trip.
- Splitting the defaults into `search_defaults.rs` keeps `config.rs` at 2,048
  lines, under its 2,055 ratchet.

### WP8 — Release-mode live verification target

- `Makefile`: `make live-release-smoke` runs the ignored `mcp_http` canonical
  tool/alias smoke with `--release`.
- `architecture/testing.md`, `docs/test-inventory.md`, and
  `skills/eggsearch-dev/SKILL.md` name it as the authoritative solver-timing
  measurement; the debug target remains the wiring check.

### Documentation

- `docs/provider-setup.md`: probe budget/no-health-write policy, the
  `Retry-After` cooldown row, the 15–300 s troubleshooting range, and the cold
  challenge-start note.
- `docs/tool-matrix.md`: probe semantics, the short-gate marker, and one
  explanation per `repo_map` failure.
- `docs/config.md`: partial-`[search]` inheritance and the empty-table rule;
  probe outcomes are advisory.
- `architecture/engines.md`: `RateLimited` in the error inventory, the parse
  fingerprint, the 429/cooldown path, and the cold-start contract.
- `CHANGELOG.md`: entries for each user-visible behavior change.
- `docs/test-inventory.md`: 6,652 all-features / 6,587 mock tests (ignored
  counts unchanged at 30 / 6).

## Requirement-to-evidence matrix

| Criterion | Evidence | Outcome |
|---|---|---|
| Probe outcomes match search reality at the default budget and never start a cooldown | `probe_accepts_an_engine_slower_than_the_old_five_second_constant`; `provider_status_probe_uses_the_configured_search_budget`; `probe_failures_never_enter_cooldown`; live probe table below | pass |
| A cold challenged provider costs at most one retry and never solves twice | `a_solve_that_outlives_the_replay_budget_still_caches_the_cookie` (wall/solve hit counts stay 1); `a_warm_cookie_keeps_the_first_attempt_inside_budget` | pass |
| A 429 with `Retry-After` sets a matching, clamped cooldown | NVD 429 tests; `retry_after_hint_starts_a_matching_cooldown_immediately`; `retry_after_hint_is_clamped_and_rate_limit_scoped`; `a_retry_after_rate_limit_starts_a_matching_cooldown` | pass |
| The captured JavaScript gate escalates under `auto`; the negative fixture does not | `auto_render_escalates_a_short_javascript_gate_once`; `auto_render_keeps_a_long_javascript_mention_on_the_http_path`; `http_only_never_escalates_a_short_javascript_gate` | pass |
| A parse failure carries a bounded fingerprint | `parse_failure_fingerprint_*` unit cases; `seeded_drift_reports_parse_failed` | pass |
| `repo_map` never pairs a specific failure with the generic notice | `repo_map_explicit_host_404_omits_the_generic_notice`; `repo_map_names_the_inferred_host_when_the_forge_lookup_fails`; `repo_map_unrecognized_forge_failure_reports_one_generic_notice` | pass |
| Documented `[search]` snippets parse standalone while an explicit empty provider table stays empty | `search_section_with_only_timeout_keeps_built_in_providers`; `routing_table_alone_parses_with_search_defaults`; `explicit_empty_provider_table_is_not_repopulated`; `docs_config_snippets` | pass |
| `make check`, `make docs-check`, and `make packaging-check` pass | gate table below | pass |

## Verification on this candidate

| Command | Outcome |
|---|---|
| `RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test config_validation` | pass: 105 passed |
| `RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test provider_probe_conformance` | pass: 23 passed |
| `RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test repo_workflow repo_map_` | pass: 14 passed |
| `RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test engine_parse_conformance` | pass: 6 passed |
| `RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test dispatch_fault_injection` | pass: 34 passed |
| `RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test engine_api_conformance` | pass: 7 passed |
| `RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test docs_config_snippets` | pass: 2 passed |
| `RUSTUP_TOOLCHAIN=1.89.0 make check` | pass (exit 0) |
| `RUSTUP_TOOLCHAIN=1.89.0 make docs-check` | pass (exit 0) |
| `RUSTUP_TOOLCHAIN=1.89.0 make packaging-check` | pass (exit 0) |
| `python3 packaging/check-planning-consistency.py` | `planning-consistency: ok` |
| `cargo test --locked --all-features -- --list` / `--features mock` | 6,652 / 6,587 tests |

The pinned MCP surface is unchanged: `mcp_tool_contract` keeps fingerprint
`3fa97faac725dc97` and 33,850 advertised bytes.

## Live confirmation (release binary, 2026-11-26, this egress)

`make live-release-smoke` passed in 85.53 s: nine advertised tools, three
aliases, default keyless providers, PDF extraction, and browser rendering.

| Observation | Measurement |
|---|---|
| Startpage cold at the default 8 s budget | 4.63 s, 3 results; second call 0.0 s from the cached cookie |
| Startpage with a 30 s budget | 9.81 s cold, 0.0 s warm (network variance; the default-budget run is the acceptance case) |
| Probe run 1 | brave 7,406 ms success; yahoo 4,813 ms success; duckduckgo 642 ms; startpage 291 ms; osv 0 ms |
| Probe run 2 | brave 6,918 ms success; yahoo 4,698 ms success; duckduckgo 525 ms; startpage 262 ms |
| Health after two probe runs | brave/yahoo/duckduckgo/startpage all `unknown`, 0 consecutive failures |
| Mojeek with a 20 s budget | 1.73 s, 3 results; second call 0.0 s |
| Auto render, captured short gate over loopback | escalation attempted; the real browser refuses loopback with `browser_unavailable: private/local network target blocked`, so the HTTP result is returned |
| Auto render, negative long page and `http_only` | no escalation attempt (`fetch_warning` only) |
| Review's live practice page | now 796 visible characters (documentation, not a short gate), so it correctly stays on HTTP |
| NVD with the provider enabled | 0.58 s and 0.27 s, no `429` observed |

The `transport: "browser"` assertion for the captured gate is covered by the
synthetic-renderer tool test; a real Chrome cannot serve a loopback target under
the browser's SSRF policy. A live `429` was not reproducible on demand; the
header path is covered deterministically at the NVD, registry, and adapter
layers.

## Deliberate deviations and residual risk

- The `ProviderStatusArgs.probe` doc comment was left byte-identical to keep the
  advertised schema pinned. The corrected probe semantics live in the operator
  docs, not in the schema description.
- `last_probe_at` was not added; the existing outcome fields already carry
  success, class, and latency, and no caller asked for a probe timestamp.
- The aggregate probe deadline remains 20 s; only the per-provider deadline is
  derived from the search budget.
- The browser SSRF policy, not the classifier, prevents a live loopback render;
  this is pre-existing policy and out of scope.
- `Retry-After` is honored on the shared HTML path and NVD. Other JSON engines
  keep their existing status handling; the variant is additive and can be
  adopted without a contract change.
