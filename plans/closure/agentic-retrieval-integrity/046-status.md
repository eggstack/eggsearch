# M046 Fetch Browser-Escalation Parity — Closure Evidence

Status: closed

Source plan: `plans/archive/implementation/agentic-retrieval-integrity/046-fetch-browser-escalation-parity.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M046

Decision authority: accepted ADR-0006 (keyless HTML retrieval is first-class), ADR-0019 §2
(script-completed interstitials) and §5 (browser-like identity), and ADR-0002 (transport
ownership). No new ADR, tool, argument, schema property, configuration key, provider id, or
dependency.

## Exact code candidate

Implementation candidate `20c0672ad835796a22b6f4e830757a97753ca49f` on
`feat/auto-20261117-044c4885`, baseline `364014c6fd9c29e6dd082915bd334ccf9bc148dd` (planning
baseline `96f42a9`). Every command and probe below ran against that tree. This closure commit
carries this record, the plan status, the roadmap status, and the registry row together; it changes
no source.

## Defect

M045 taught the search engines to treat a `429`/`5xx` from a browser-gated origin as a reason to
render once. The fetch surfaces never inherited the rule, because the rule was unreachable there.
Measured on the M045 release build, this egress:

| Call | Pre-change result |
|---|---|
| `web_fetch` `render: "auto"`, `search.brave.com/search?q=…` | `upstream_failed: http_status: 429` in 0.4 s |
| `web_fetch` `render: "auto"`, `search.yahoo.com/search?p=…` | `upstream_failed: http_status: 500` in 0.1 s |
| `web_fetch` `render: "browser"`, `search.yahoo.com/search?p=…` | `200`, `transport: "browser"`, 3.7 s |
| `eggsearch fetch` on `search.brave.com/…` | `Error: fetch failed: http_status: HTTP error: 429` |

`src/fetch/client.rs` returns `FetchError::HttpStatus` for every status outside `200..300`
before the automatic escalation decision in `src/mcp/tools/web_fetch.rs` runs. That trigger was
wall-shaped **2xx** content, so a `429` or `500` never produced a response to classify and the
browser was never asked. The CLI `fetch` command had no browser path at all.

## Change

1. `src/fetch/challenge.rs`: `is_browser_gated_status(status)` — `429` and `500..=599`, the one
   definition of the class.
2. `src/meta/engines/render.rs`: `is_browser_gated` delegates its `BadStatus` arm to that
   predicate instead of restating the range.
3. `src/fetch/browser/classify.rs`: `rendered_result_serves(status, challenge_completed)` — the
   return rule shared by every escalated surface.
4. `src/fetch/browser/types.rs`: `BrowserConfig::from_settings(&[fetch.browser])`, so the server
   and the CLI read one operator setting into one value; `src/mcp/state.rs` now uses it.
5. `src/mcp/tools/web_fetch_escalation.rs` (new): `gated_status` and `escalate_gated_status` —
   the availability and 2 s floor, the render through the existing `run_browser_fetch` path, the
   return rule, the origin-success record, and the completion counter.
6. `src/mcp/tools/web_fetch.rs`: escalation at the `FetchError::HttpStatus` seam under `auto`,
   before the retry and before any origin failure is recorded; a gated status suppresses the retry;
   the annotated fallback error names the attempted escalation.
7. `src/mcp/tools/web_fetch_window.rs` (new): the pure address-window and page-chunk helpers,
   moved so `web_fetch.rs` stays under the 1,600-line guard (1,496 lines; 198 and 131 for the two
   new modules).
8. `src/commands/fetch.rs`: the CLI builds the process browser lifecycle from `[fetch.browser]`,
   honours the configured policy, renders directly under `browser`, and escalates under `auto`
   through the same transport and return rule. No new flag.
9. Tests: six integration cases in `tests/web_fetch_integration.rs`, three library unit tests
   (`is_browser_gated_status`, `rendered_result_serves`, `from_settings`), and one CLI unit test
   for the policy mapping. The three tests that assert the process-global completion counter now
   share `COMPLETION_COUNTER`; without it two tests that each snapshot, run, and assert a delta
   race whenever the harness runs them in parallel (observed once, then 25 consecutive clean runs).

## Deviations from the plan, and why

- **Where the status predicate lives.** The plan named `src/fetch/browser/classify.rs`.
  `src/fetch/browser/**` is compiled only under the `browser` feature, while
  `meta::engines::render::is_browser_gated` compiles without it, so a definition there would force
  either a second copy or a feature-gated fork. The predicate is a pure status class with no
  browser dependency, so it lives in `src/fetch/challenge.rs`; the plan's actual requirement —
  exactly one definition of the class — is preserved.
- **Where the shared escalation helper lives.** The plan preferred `src/challenge_replay/` and made
  that conditional. `run_browser_fetch` owns the MCP global and per-origin permits and the
  mock-browser override, and it lives in `src/mcp/tools/common.rs`; `challenge_replay` is a
  library module above `fetch` and `meta` only, so calling it from there would invert layering.
  What is shared across surfaces is therefore the classification, the return rule, the
  DOM→response conversion, the completion counter, and the configuration mapping; the roughly
  40-line orchestration (deadline floor, timeout, error annotation) is duplicated in
  `src/commands/fetch.rs` and recorded here as the residual cost of that rule.
- **`web_fetch_window.rs`** is an incidental decomposition: the escalation added 140 lines to a
  module the layout guard caps at 1,600.

## Requirement-to-evidence mapping

| # | Acceptance criterion | Evidence |
|---|---|---|
| 1 | A browser-gated `429`/`5xx` read through `fetch` returns the document | `web_fetch_auto_escalates_a_gated_429_to_the_browser`, `web_fetch_auto_escalates_a_gated_500_to_the_browser`; live table below |
| 2 | A `200` wall-shaped page escalates exactly as before | The existing auto-escalation suite is unchanged and green: `web_fetch_auto_counts_a_wall_the_browser_completed`, `web_fetch_auto_returns_an_unresolved_wall_marked_not_as_a_document`, `web_fetch_auto_marks_a_verification_interstitial_that_never_resolves` |
| 3 | Refusals never render | `web_fetch_auto_does_not_escalate_a_refusal_status` (402/403/404, zero render calls); the existing 401 test; live 403 below |
| 4 | `http_only` is unchanged | `web_fetch_http_only_never_escalates_a_gated_status`; existing `web_fetch_render_http_only_never_uses_browser` |
| 5 | No browser, or `enabled = false`, keeps the error and names the unavailable escalation | `web_fetch_auto_names_the_unavailable_escalation` asserts the `http_status` code survives and the message carries `browser escalation unavailable` |
| 6 | A genuine `5xx` outage is not converted into content | `web_fetch_auto_keeps_the_error_when_the_render_did_not_improve`: the render ends on `500` with no completed wall, and the original `http_status` error is returned naming `browser render ended on HTTP 500` |
| 7 | One escalation per call; solver first | `calls == 1` in both gated tests; the existing solver-first tests still pass and the solver runs in the `Ok` arm before any classification |
| 8 | A served origin is not charged; completions only on a completed wall | `web_fetch_auto_escalates_a_gated_429_to_the_browser` sets `origin_circuit_failure_threshold = 1` and asserts the circuit is closed after a served escalation; the `500` served case asserts `browser_completions` is unchanged, the `429` case asserts `+1` |
| 9 | One definition of the status class | `fetch::challenge::tests::browser_gated_statuses_are_the_two_the_origins_serve_to_clients`; `fetch::browser::classify::tests::a_render_serves_only_when_it_improved_on_the_status`; `fetch::browser::types::tests::settings_map_to_the_browser_configuration` |
| 10 | `fetch` stays independent of `meta`; the browser stays optional | `cargo check --no-default-features` inside `make check`; the renderer and lifecycle are `None` without the feature; `static_guards` 67 passed |
| 11 | The CLI honours the policy and gains no flag | `commands::fetch::tests::cli_render_policy_reads_the_operator_setting`; live CLI table below |
| 12 | Gates pass on the exact candidate | `make check` exit 0: 6 577 passed, 0 failed, 28 ignored; `cargo test --features mock` 6 550 passed / 5 ignored; `make docs-check` exit 0; `static_guards` 67; engine unit tests 723; `fetch::browser` 99; `fetch::challenge` 5; CLI fetch tests 8; `web_fetch_integration` 128 passed / 1 ignored under both feature combinations |
| 13 | Hosted qualification is green on the exact candidate | Table below |

## Live before and after

Release build at the candidate, one MCP stdio server process per configuration, `render: "auto"`,
`max_chars: 4000`:

| URL | Before (M045 tree) | After |
|---|---|---|
| `search.brave.com/search?q=rust+programming+language` | `http_status: 429` in 0.4 s | 3 091 chars in 6.1 s, `transport: "browser"`, `browser_escalated: true`, document status 429 (the status the origin serves) |
| `search.yahoo.com/search?p=rust+programming+language` | `http_status: 500` in 0.1 s | 3 591 chars in 3.8 s, `transport: "browser"`, `browser_escalated: true`, status 200 |
| `https://httpbin.org/status/403` | — | `upstream_failed: http_status: HTTP error: 403` in 0.1 s, no render and no escalation note |

CLI `eggsearch fetch`, release build at the candidate:

| Policy | URL | Result |
|---|---|---|
| `auto` (default) | Brave | exit 0, `Status: 429`, `Fetched: true`, rendered Brave page content |
| `auto` (default) | Yahoo | exit 0, `Status: 200`, `Fetched: true`, rendered Yahoo page content |
| `http_only` | Brave | exit 1, `Error: fetch failed: http_status: HTTP error: 429 HTTP 429` |
| `http_only` | Yahoo | exit 1, `Error: fetch failed: http_status: HTTP error: 500 HTTP 500` |
| `auto` | `httpbin.org/status/403` | exit 1, `http_status` 403 with no render |

## Hosted qualification

Candidate `20c0672ad835796a22b6f4e830757a97753ca49f` on `feat/auto-20261117-044c4885`:

| Workflow | Run | Result |
|---|---|---|
| CI | `44948700800` | success — linux `ci`, macOS canonical verification, and windows portability all passed |
| Python native qualification | `44948700855` | success |
| Egress feature qualification | `44948700830` | success |
| Release drift guard | `44948700801` | success |

## Residual risk

- **Cost on a genuine outage.** A `5xx` that is a real outage now pays one browser launch per call
  before the original error is returned, until an origin-gated memory exists. It is bounded by the
  call deadline and the 2 s floor.
- **A genuine `429` rate limit** also pays one render. The error still surfaces when the render does
  not improve on the status, and the message names the render's outcome.
- **CLI orchestration duplication.** The CLI repeats the deadline, timeout, and error-annotation
  code around the shared decision functions. Moving it would require `challenge_replay` to depend on
  `src/mcp`; the duplication is the smaller cost.
- **The CLI reports no transport.** It prints the document's own status and content, so a rendered
  Brave page shows `Status: 429` with `Fetched: true`; `transport` and `browser_escalated` stay
  MCP-only because the plan kept the CLI surface unchanged. The `http_only` contrast is the
  observable proof that the document came from the browser.
- **The CLI has no automated end-to-end harness**, so its acceptance is live-manual evidence.
- **An error page that renders as ordinary content** is still indistinguishable from a document
  once rendered; M045 recorded that, and this milestone does not guess from a title or status.
- **No session-scoped cookie reuse and no per-origin "known browser-gated" memory**; both remain
  deferred from M045.

## Verification commands

```sh
RUSTUP_TOOLCHAIN=1.89.0 make check          # exit 0: 6 577 passed, 0 failed, 28 ignored
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock        # 6 550 passed, 5 ignored
RUSTUP_TOOLCHAIN=1.89.0 make docs-check     # exit 0
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test static_guards
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test web_fetch_integration
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib engines::
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --bin eggsearch commands::fetch
RUSTUP_TOOLCHAIN=1.89.0 cargo build --release --locked --all-features
python3 packaging/check-planning-consistency.py
./packaging/check-contract.sh
```
