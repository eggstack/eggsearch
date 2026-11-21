# M046 — Fetch Browser-Escalation Parity

Status: active
Date: 2026-11-21
Subsystem: agentic retrieval integrity
Milestone class: correctness (corrective to M045)
Baseline: `364014c6fd9c29e6dd082915bd334ccf9bc148dd` on `feat/auto-20261117-044c4885`
(planning baseline `96f42a9`)
Decision authority: accepted ADR-0006 (keyless retrieval is first-class), ADR-0019 §2
(script-completed interstitials) and §5 (browser-like identity), and ADR-0002 (transport
ownership). No new ADR.

References: `plans/000-long-term-specification.md` §2; `plans/003-planning-process.md` §7
(corrective passes); `plans/closure/agentic-retrieval-integrity/045-status.md`;
`architecture/fetch.md` § Browser challenge completion; `architecture/engines.md`
§ Browser-rendered access; `docs/tool-matrix.md` (render policy).

Dependencies: hard M045 (closed); interface ADR-0019 (accepted).

## Objective

Make `fetch` reach a browser-gated origin for the same reasons `web_search` does, so an agent
that reads a result URL — or any URL — gets the page instead of `http_status`, without knowing to
ask for `render: "browser"`.

## Defect

M045 taught the search engines to treat a `429`/`5xx` from a browser-gated origin as a reason to
render once. The fetch surfaces did not inherit that rule, because the rule is unreachable there.

Measured on the release build (`96f42a9` tree, this egress):

| Call | Result |
|---|---|
| `web_fetch` `render: "auto"`, `search.brave.com/search?q=…` | `upstream_failed: http_status: 429` in 0.4 s |
| `web_fetch` `render: "auto"`, `search.yahoo.com/search?p=…` | `upstream_failed: http_status: 500` in 0.1 s |
| `web_fetch` `render: "browser"`, `search.yahoo.com/search?p=…` | `200`, `transport: "browser"`, 3.7 s |
| `eggsearch fetch https://search.brave.com/…` | `Error: fetch failed: http_status: HTTP error: 429` |

`src/fetch/client.rs:435` returns `FetchError::HttpStatus` for any status outside `200..300`
before the automatic escalation decision in `src/mcp/tools/web_fetch.rs` (around line 949) runs.
The trigger there is wall-shaped **2xx** content: a recognised challenge family, a JavaScript
shell, or a non-interactive verification page. A `429` or `500` never produces a response to
classify, so the browser is never asked. The CLI `fetch` command has no browser path at all: it
builds a `FetchClient` and returns the same error.

The consequence is that the same origin, the same URL, and the same wall behave differently
depending on which tool the agent chose: `web_search` returns results, `fetch` returns
`http_status`.

## Why M045 did not catch it

M045's fetch evidence used `render: "browser"` deliberately, to prove the render pipeline, and its
search evidence used `web_search`. No check exercised the default `render: "auto"` path against a
non-2xx gate, which is the shape both origins serve, so the missing trigger was invisible to every
gate that ran.

## Invariants that cannot regress

- `src/fetch/**` stays independent of `src/meta/**`; `fetch` remains usable without the engine
  layer.
- The browser stays optional. Without the feature, without a discovered browser, or with
  `[fetch.browser].enabled = false`, `auto` keeps today's error and reports that escalation was
  unavailable.
- The bounded solver still runs before the browser, at most one solve and at most one escalation
  per call, inside the caller's deadline and the existing 2 s remaining-time floor.
- `http_only` never renders, and `browser` stays a direct render.
- Refusals are never answered: `401`, `403`, `402`, and `404` keep their current error and never
  reach the browser (ADR-0019 §6).
- The SSRF interceptor still validates every hop of a render; no new outbound path is introduced.
- A browser-served page must not charge the origin's HTTP circuit breaker as a failure.
- `transport: "browser"`, `browser_escalated: true`, and
  `provider_status.challenge_solver.providers.fetch.browser_completions` keep their current
  meaning.

## Work package 1 — one classification, and MCP fetch escalates on a gated status

1. **One predicate.** Move the status set into `src/fetch/browser/classify.rs` as
   `is_browser_gated_status(status: u16) -> bool` (`429` and `500..=599`), and have
   `meta::engines::render::is_browser_gated` call it for its `BadStatus` arm. `meta` may depend on
   `fetch`; the reverse stays forbidden, and there is then exactly one definition of the class.
2. **Escalate at the error seam.** In `run_web_fetch_inner`, when the HTTP attempt returns
   `FetchError::HttpStatus(status)`, `render_policy` is `Auto`, `is_browser_gated_status(status)`
   holds, a browser is available, no escalation has been spent on this call, and at least 2 s
   remain, run the same `run_browser_fetch` path the wall-shaped branch already uses — before the
   retry and backoff decision and before any origin failure is recorded.
3. **Return rule.** The rendered document replaces the error only when the render ended on a
   `2xx`/`3xx` document status or completed a recognised wall
   (`BrowserFetchResult::challenge_completed`). A `5xx` error page the browser merely re-rendered
   keeps today's `http_status` error, so a genuine outage is not converted into content. When the
   rule falls back to the error, the message names the attempted escalation.
4. **Retry interaction.** Under `Auto`, a gated status leaves the retry loop to the escalation
   instead of retrying; retrying an automated request meets the same wall (two live `429`s and a
   live `500` were measured before a browser was tried). Under `http_only`, retry behaviour is
   unchanged.
5. **Scope.** `web_fetch`, `batch_fetch` web items, and the `fetch` alias share this handler, so
   all three inherit the change without per-tool code.

## Work package 2 — CLI fetch honours the configured policy

1. `src/commands/fetch.rs` builds the process browser lifecycle from `[fetch.browser]` when the
   feature is compiled and the browser is enabled, and applies the configured policy exactly as the
   MCP path does (`auto` by default).
2. **No new CLI argument.** The policy is already an operator setting, and `fetch` exposes no
   per-call override for any other fetch behaviour. A `--render` flag is explicitly out of scope
   and remains a separate surface decision.
3. Sharing rule: prefer one shared escalation helper so the CLI does not reimplement the return
   rule. Extract into `src/challenge_replay/` (which already depends on `fetch` and `meta` and owns
   fetch-path orchestration) **only if** the MCP call site keeps its profile and screenshot handling
   untouched. If extraction would disturb those paths, the CLI calls
   `fetch::browser::browser_fetch_with_policy` plus `browser_result_to_response` directly and the
   duplication is recorded in the closure note. A layering inversion is never an option.
4. The CLI never gains profile-scoped or screenshot behaviour; those stay MCP-only.

## Expected production changes

| File | Change |
|---|---|
| `src/fetch/browser/classify.rs` | `is_browser_gated_status` and its unit tests |
| `src/meta/engines/render.rs` | `is_browser_gated` delegates the status arm |
| `src/mcp/tools/web_fetch.rs` | escalation at the `HttpStatus` seam, the return rule, retry interaction, error message |
| `src/challenge_replay/` | shared escalation helper, if the sharing rule allows it |
| `src/commands/fetch.rs`, `src/main.rs` | CLI policy wiring (no new flag) |
| `tests/web_fetch_integration.rs` | mock-browser escalation matrix |
| docs | `docs/tool-matrix.md`, `docs/features.md`, `architecture/fetch.md`, `docs/commands.md` if CLI output changes, `CHANGELOG.md`, `docs/test-inventory.md` |

## Non-goals

- No answer for `401`, `403`, `402`, or `404`; policy, authentication, payment, and not-found
  responses stay errors (ADR-0019 §6).
- No browser path for `repo_fetch`; forge hosts are API-addressed, and rendering a repository page
  is a different capability.
- No new MCP tool, schema property, provider, configuration key, or dependency.
- No new CLI argument.
- No session-scoped cookie or pass reuse, and no per-origin "known browser-gated" memory; both stay
  deferred from M045.
- No change to search escalation, the solver, or the four-outcome contract.

## Compatibility and migration

`auto` gains one behaviour: a `429`/`5xx` gated origin may now return a rendered document instead
of an error. Callers that keyed on `http_status` for those statuses see `transport: "browser"` and
a document instead; the rule in WP1 keeps any `5xx` that the render did not improve as the original
error. `http_only`, `browser`, the ten-tool surface, and every response field are unchanged.
The CLI gains behaviour but no flag.

## Verification

```sh
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test web_fetch_integration
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib fetch::browser
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib engines::render
RUSTUP_TOOLCHAIN=1.89.0 make check
RUSTUP_TOOLCHAIN=1.89.0 make docs-check
```

Focused cases, using the existing mock browser override so no network or Chrome is needed:

- a `429` and a `500` each escalate once and return the mock page;
- a `200` wall-shaped page escalates exactly as today (no regression);
- `403`, `402`, `404` do not escalate, under `auto` or `browser`;
- `http_only` never escalates;
- no browser available, or `[fetch.browser].enabled = false`, keeps the original `http_status`
  error and names the unavailable escalation;
- a rendered `5xx` page that completed no wall returns the original error;
- a successful escalation does not record an origin failure, and `browser_completions` increments
  only when a wall completed.

Live acceptance on the release build, this egress, recorded in the closure record:

- `web_fetch render: "auto"` on `search.brave.com` and `search.yahoo.com` returns the page with
  `transport: "browser"` and `browser_escalated: true`;
- `eggsearch fetch` returns the page for the same two URLs under the default policy, and the
  `429`/`500` error under `policy = "http_only"`;
- a known `403` origin still returns `http_status` and never renders.

## Acceptance and stop conditions

- A browser-gated `429`/`5xx` URL read through `web_fetch`, `batch_fetch`, `fetch`, or the CLI
  returns the document, bounded by the caller's deadline, with the transport reported.
- Refusal statuses never render; `http_only` is unchanged; a genuine `5xx` outage still errors.
- One escalation per call; solver-first ordering, profiles, screenshots, and SSRF policy are
  unchanged.
- **Stop:** if the return rule cannot be expressed without changing `FetchError`, the response
  schema, or the ten-tool surface, record the blocker and propose a separate decision instead of
  widening the error type.

## Required closure evidence

The closure record carries the pre-change measurements (the table in § Defect), the mock
escalation matrix, the live before/after for both origins and both surfaces, the refusal
non-escalation evidence, the gates, the hosted qualification on the exact candidate, and residual
risk. The residual list must state the cost of an escalation on a genuine `5xx` (one browser
launch per call, until an origin-gated memory exists) and that the CLI has no automated end-to-end
harness, so its acceptance is live-manual evidence.
