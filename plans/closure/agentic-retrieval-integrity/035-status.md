# M035 Browser Challenge Completion — Closure Evidence

Status: closed

Source plan: `plans/implementation/agentic-retrieval-integrity/035-browser-challenge-completion.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Architecture decision: `plans/adrs/ADR-0019-broadened-challenge-coverage-and-browser-identity.md`
(accepted 2026-11-19)

## Scope

M035 made the shipped browser a completion mechanism for script-driven anti-bot walls, and stopped
it from shadowing the cheap solver. It targets the five defects the plan traced:

1. Under `render: "auto"` the MCP fetch path escalated to Chrome before the solver, so a solvable
   wall was handed to a browser that had no reason to solve it.
2. Completion was decided by four title substrings (`just a moment`, `checking`, `verifying`,
   `please wait`), so a wall that cleared without changing its title stayed "unresolved" and a
   legitimate page whose title contained one of those strings stayed "a wall".
3. `classify_response` already existed and the wait loop did not use it.
4. `if !escalated` made the solver and the browser mutually exclusive, so no path let one
   mechanism hand off to the other inside a single call.
5. A wall the browser completed appeared in no counter.

The architecture rule landed first, in commit `e10fd93`, because
`architecture/fetch.md` previously recorded the opposite order ("a successful `render: "auto"`
render replaces the payload before the solver runs"). The rule now states the order of mechanisms,
the bounded completion wait, the live re-classification, and the completed-or-marked invariant.

## Exact code candidate

`55180790c6dc5dbb2ce0d94c186ef88cd7584f48` — the implementation commit, carrying the ordered
fetch path in `src/mcp/tools/web_fetch.rs`, the live re-classification in
`src/fetch/browser/navigate.rs`, the `is_script_wall` decision in
`src/fetch/browser/classify.rs`, the completion counter in `src/meta/challenge/ledger.rs` and
`src/challenge_replay/mod.rs`, the behavioral tests in `tests/web_fetch_integration.rs`, and the
documentation set. Every command recorded below ran against this exact candidate. The closure
commit carries this record, the plan status, the roadmap status, and the registry row together.

The paper-first rule is in the preceding commit, `e10fd93`.

## Requirement-to-evidence mapping

| # | Acceptance criterion | Evidence |
|---|---|---|
| 1 | A fetched Anubis or ALTCHA wall under `policy = auto` is solved, not escalated to the browser, and the document is returned; no browser process is launched for it | `tests/web_fetch_integration.rs::web_fetch_auto_solves_a_wall_before_escalating_to_the_browser`. The origin publishes an Anubis `fast` challenge, solving is enabled, and a mock browser is installed and available. The call returns the document with `manual_interaction_required: false`, no `fetch_challenge_page` warning, and `browser_escalated: false`. Call counts: wall **1**, pass-challenge **1**, replay **1**, mock browser **0**. The mock browser is configured to fail, so any call would be visible as a failure as well as a count. |
| 2 | A script-driven wall that resolves inside `verification_wait_ms` returns the document, and `provider_status` shows exactly one browser completion | The completion decision is unit-tested: `src/fetch/browser/classify.rs::tests::only_the_two_script_wall_dispositions_wait_for_completion` pins `JavascriptShell` and `NonInteractiveVerification` as the only dispositions that wait. The counting path is proven end to end by `tests/web_fetch_integration.rs::web_fetch_auto_counts_a_wall_the_browser_completed`: a mock browser returns a `UsefulContent` page flagged as a completed wall, the call returns the document with `manual_interaction_required: false`, the ledger's `fetch` entry rises by exactly **1**, and `provider_status.challenge_solver.providers.fetch.browser_completions` reports the same value. The 500 ms polling loop itself runs only inside a real Chrome render and is **not** exercised by any test in the default gate — see residual risk 1. |
| 3 | A wall that does not resolve returns the response with a challenge warning, and the response is never a bare wall | Two tests. `::web_fetch_auto_returns_an_unresolved_wall_marked_not_as_a_document`: a mock browser returns a `NonInteractiveVerification` page that still names the Anubis family; the call returns it with `manual_interaction_required: true`, exactly **one** `fetch_challenge_page` warning naming `anubis`, and the solver answered nothing (`pass` mock hit **0**). `::web_fetch_auto_marks_a_verification_interstitial_that_never_resolves`: the same shape without a family; the call returns it marked, with one warning naming the verification page. |
| 4 | An interactive challenge still returns `manual_interaction_required` and attempts no completion | `::web_fetch_auto_never_completes_an_interactive_challenge`. The mock browser reports `InteractiveChallenge`; the call returns the manual-interaction error whose message carries `manual_interaction_required`. The error path returns before `browser_result_to_response`, so no completion can be recorded. |
| 5 | No more than one solver attempt pair and one escalation occur per call, verified by call counts in the loopback fixtures | Criterion 1's counts: one wall request, one pass-challenge answer, one replay, and zero browser calls when the solver wins. `::web_fetch_auto_returns_the_marked_wall_when_the_deadline_leaves_no_render_time` proves the remaining-deadline floor: with 2.6 s of budget and a 1.2 s origin, the call returns the marked wall, emits `browser_unavailable`, and the mock browser is called **0** times. The escalation is a single arm in the `else` of the solver result, so it cannot run twice. |
| 6 | `make check` passes with 0 failures, keyless and browser-free; `make docs-check` passes | See Verification. `make check` exit 0 on the candidate, and `cargo check --locked --features mock` (no `browser`) also passes, so the non-browser branch of the ordering still compiles. |

Invariants from the plan:

- `src/fetch/**` still does not import `src/meta/**` or `src/challenge_replay/`.
  `tests/static_guards.rs::fetch_layer_never_imports_the_challenge_solver` passes, and the new
  code in `src/fetch/browser/navigate.rs` imports only `src/fetch/challenge.rs`, the same layer.
- The browser stays behind the `browser` feature: `cargo check --locked --features mock` compiles
  without it.
- No new dependency: `Cargo.toml` is unchanged.
- No cookie, nonce, salt, challenge id, pass token, or profile path reaches the counter, a log, an
  error, a warning, `Debug`, or a fixture. The counter is a single `u64`;
  `src/meta/challenge/ledger.rs::tests::snapshot_carries_no_secret_material` still walks every
  provider field and asserts numeric-only values.
- Files stay inside the size limits: `src/mcp/tools/web_fetch.rs` 1,485 lines / 64,549 bytes and
  `src/fetch/browser/navigate.rs` 878 lines / 30,369 bytes.

One planned step was not done as written: the plan named
`tests/dispatch_fault_injection.rs` for the unavailable-browser and expired-deadline paths. That
suite owns provider-failure injection (`architecture/testing.md`), while both paths are fetch
behavior already covered in `tests/web_fetch_integration.rs`
(`::web_fetch_auto_preserves_http_when_browser_unavailable`,
`::web_fetch_auto_without_rendering_reports_browser_unavailable`, and the deadline test above).
The coverage exists; its placement follows the suite-ownership rule.

## Observed ordering, before and after

| Condition | Before M035 | After M035 |
|---|---|---|
| `auto`, solvable Anubis wall, browser available | Escalation decided first: `escalated = true`, Chrome's page replaced the payload, the solver was skipped | Solver first: wall 1, pass 1, replay 1, browser 0, document returned, `browser_escalated: false` |
| `auto`, wall the solver declines, browser available | Browser first | Solver declines (no solve recorded), then exactly one escalation |
| `auto`, browser unavailable, solvable wall | `browser_unavailable` warning and the marked page; solver skipped | Solver still solves; no browser warning |
| `auto`, less than 2 s of budget left | Escalation skipped; marked page plus `browser_unavailable` | Unchanged: marked page plus `browser_unavailable`, browser 0 calls |
| `http_only` | Marked page, solver eligible | Unchanged |
| `browser` | Direct render, no solver | Unchanged; a completed wall is now counted |

The M034 live qualification recorded the "before" behaviour at a real origin: Chrome received
Anubis's "Oh noes!" error page, the call returned it as `escalated = true`, and no solve or
document was produced. The "after" behaviour is proven by the loopback fixtures above, not live.

## Verification

Using the pinned Rust 1.89 toolchain on candidate `55180790c6dc5dbb2ce0d94c186ef88cd7584f48`:

- `make check`: exit 0 — **85 suites, 6,430 tests, 0 failed, 28 ignored**. This includes fmt,
  clippy with `-D warnings`, the no-default-feature check, the all-features test run, repository
  hygiene, the dependency policy, and the packaging contract checks. The packaging contract
  reported `planning-consistency: ok`, `workflow-pin-check: ok (9 workflow files)`,
  `eggpack qualifier contract: ok (25 producer jobs, 7 targets)`, and the egress qualification
  contract agreeing with `packaging/release-targets.txt`.
- `make docs-check`: exit 0 (`cargo doc --locked --all-features --no-deps`).
- `cargo test --locked --all-features --lib`: 3,933 passed, 0 failed.
- `cargo test --locked --all-features --test web_fetch_integration`: 121 passed, 0 failed,
  1 ignored. The new tests are the six `web_fetch_auto_*` cases above.
- `cargo test --locked --all-features --test browser_transport`: 75 passed, 0 failed.
- `cargo test --locked --all-features --test provider_routing`: 99 passed, 0 failed; the
  `challenge_solver` block test now also asserts `browser_completions` and that it moves no
  solve counter.
- `cargo test --locked --all-features --test static_guards`: 67 passed, 0 failed.
- `cargo check --locked --features mock`: exit 0, proving the non-browser branch.
- `git diff --check`: clean.

Nothing was deliberately skipped. The suite total moved from 6,422 to 6,430: six new behavioral
tests, one classification unit test, and one ledger unit test.

## What was demonstrated live, and what was not

Nothing in M035 was demonstrated against a real browser. The completion wait, the 500 ms polling
cadence, the reload detection, and the re-read of the main-document status are exercised only when
Chrome renders a real wall, and the default gate is browser-free by design (the plan's stop
condition). The completion **decision** and the **counting** are covered deterministically with the
mock browser override; the polling loop around them is not.

No live browser record is claimed. A live attempt remains optional, and the M034 record's origin
behaviour ("Oh noes!" toward headless Chrome) makes a live completion demonstration dependent on a
wall that Chrome can actually pass.

## Known limitations and residual risk

1. The browser's completion wait is not covered by any test in the default gate. The loop runs
   only inside `navigate_and_extract`, which needs a real Chrome; the pure decision
   (`is_script_wall`) and the outcome marking are covered, the loop is not. A defect in the
   polling cadence or in the status re-read would be caught only by a live run.
2. A wall whose own failure page presents ordinary content is returned as a document. Anubis's
   "Oh noes!" page is the known example: after rendering, nothing in the content distinguishes it
   from a document, so the render reports `UsefulContent`. The architecture rule records this case
   rather than guessing from a title or status. The ordering fix makes it rarer (a solvable Anubis
   wall no longer reaches the browser), but it is not eliminated.
3. A `JavascriptShell` page that never fills in now waits up to `verification_wait_ms` (default
   10 s) before it is returned, under `auto` and `browser`. The previous behaviour returned after
   `post_load_wait_ms` (default 1.5 s). The page content returned is unchanged; the cost is
   latency on an empty SPA. No live measurement of the added latency was taken.
4. A wall whose only signal is a status code and headers — an AWS WAF Challenge `202` plus
   `x-amzn-waf-action`, for example — carries no marker that `src/fetch/challenge.rs` recognises.
   It is treated as a script shell: it may resolve and be counted as nothing, or be returned
   unmarked. The completed-or-marked invariant is therefore guaranteed only for walls with a
   detectable family or a verification disposition.
5. The counter is process-lifetime and the ledger tracks at most four provider labels. On a busy
   process the `fetch` entry can be dropped and the completion is then counted only in
   `untracked_providers`. It is a diagnostic, not a fleet-wide measure.
6. Only the MCP web-fetch path records completions, because it is the only surface that escalates.
   The CLI `eggsearch fetch` and `repo_fetch` never launch the browser, so a completion cannot
   occur there; the counter is consistent but not global.
7. `402 Payment Required` and `403` are still terminal: they are reported, never completed. The
   deadline test reinforces this — an unavailable render at a short deadline returns the marked
   wall rather than attempting anything.

## Compatibility and migration effects

- No MCP tool, argument, or advertised-schema change. `provider_status` gains one additive field
  inside an object the output schema already declares open (`"challenge_solver": {"type":
  "object"}`), and the documented field list in `docs/provider-setup.md` was updated.
- No configuration key is added, removed, or renamed, and no shipped default changes.
  `[search.challenge].enabled` is still opt-in and off by default; ADR-0019's default flip has no
  implementation milestone and is not part of this change.
- `RenderPolicy` semantics are unchanged. `http_only` still never escalates, `browser` is still a
  direct render, and `auto` still refuses to escalate on authentication, rate-limit, and policy
  refusals.
- `Cargo.toml` is unchanged, so no dependency moved and no new feature was introduced.
- The refusal vocabulary in `src/meta/challenge/ledger.rs` keeps its ten keys.

## Security evidence

- The permitted class did not widen. No interactive CAPTCHA, OCR, third-party solver, credential
  or login bypass, cookie theft, proxy rotation, retry widening, or in-process WASM execution was
  added. The browser still never clicks, types, or simulates input.
- The wall's own script runs only inside the existing sandboxed render, under the existing SSRF
  interceptor (every request re-validated), `max_requests`, `max_dom_bytes`, and the navigation,
  post-load, and verification timeouts.
- One solver attempt pair and one escalation per call, in that order; the escalation cannot retry.
- No secret material is counted, logged, or returned. The counter is numeric and the browser
  outcome carries no header or cookie value.
- `402` and `403` remain refusals that are reported, never completed.

## Recommendation

Accept M035 and close it. The criteria are met with deterministic loopback evidence: a solvable
wall never reaches Chrome, an unresolved wall is returned marked instead of as a bare document, an
interactive challenge stays manual, one completion is counted exactly once and reported through
`provider_status`, and the deadline and browser-unavailable paths keep the marked wall. The
completion polling loop itself remains proven only by construction and a live run is not claimed;
that is recorded above as residual risk 1, not as a satisfied acceptance criterion.
