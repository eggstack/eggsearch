# M045 Browser-Fidelity Provider Access — Closure Evidence

Status: closed

Source plan: `plans/implementation/agentic-retrieval-integrity/045-browser-fidelity-provider-access.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M045

Decision authority: accepted ADR-0006 (keyless HTML retrieval is first-class) and
`plans/adrs/ADR-0019-broadened-challenge-coverage-and-browser-identity.md` §2 (script-completed
interstitials), §5 (browser-like identity). No new ADR, tool, argument, schema property,
configuration key, provider id, or dependency.

## Exact code candidate

Implementation candidate `364014c6fd9c29e6dd082915bd334ccf9bc148dd`, on `feat/auto-20261117-044c4885`. Every command and probe
below ran against that tree, except where a row states otherwise. This closure commit carries this
record, the plan status, the roadmap status, and the registry row together; it changes no source.

## Defect

Two shipped default providers returned nothing on this egress while a real browser returns a full
result page for the same query:

1. **Brave.** `search.brave.com` answers the HTTP client with `429` and an 80 KiB proof-of-work
   page. `brave.rs` classified the status before reading the body, so the outcome was
   `rate_limited` and the wall was never named.
2. **Yahoo.** `search.yahoo.com` answers the HTTP client with `500` and a zero-byte body. Full
   Chrome request headers, `sec-ch-ua`, `sec-fetch-*`, compression, a seeded cookie jar, and a
   matching user agent changed nothing; over HTTP/1.1 the request followed five `_bv/v.gif`
   verification hops and ended in the same `500`.

Work package 1 (candidate `9003981`) fixed the classification: the body is read for every status,
blocked markers decide `Blocked` versus `BadStatus`, and Brave's markers match the live document.
That change is included in this candidate and verified here; it produced no results on its own,
which is what work package 2 fixes.

## Root causes found (work package 2)

1. **Headless identity.** The launched Chromium announced `HeadlessChrome/157.0.0.0` in
   `User-Agent`. Yahoo refuses that token outright. Measured directly with the same installed
   Chrome: headless identity → `ERR_HTTP_RESPONSE_CODE_FAILURE` (the origin's `500`); ordinary
   `Chrome/157.0.0.0` identity → complete results page in under a second.
2. **A silently ignored flag set.** chromiumoxide's builder renders a `--key=value` pair from a
   `(key, value)` argument. Every valued flag was passed as one `"--key=value"` string and emitted
   as `----key=value`, which Chrome ignores, so `--user-data-dir`, `--autoplay-policy`, and the
   new identity flag never reached the process. This is why presenting a browser identity appeared
   not to matter when the same flag worked when Chrome was launched by hand.
3. **Status-first classification in the render loop.** `classify_response` settled a `429` as
   `RateLimited` before looking at the wall, so the browser never waited for a challenge served
   with a non-success status. Relatedly, "captcha" as a bare substring classified Brave's page as
   an interactive CAPTCHA because it *offers* "Switch to traditional CAPTCHA".
4. **Brave's wall does not self-complete.** With an ordinary identity the page waits on its own
   `Verify` control; nothing advances for 22 s until the control is clicked, and results then
   arrive in about 1.5 s.
5. **Opaque result links.** The rendered Brave result links to `/go/v1#<opaque token>`; the token
   is not decodable (three samples, 143–209 bytes, no printable run) and the displayed
   `cite.snippet-url` is the only readable destination.

## Change

1. `src/fetch/browser/lifecycle.rs`: the browser presents the identity of the Chrome build it is —
   `--user-agent` built from the discovered major version and host platform — and every valued flag
   is passed as a `(key, value)` pair. An unparseable detected version leaves the flag off.
2. `src/fetch/browser/classify.rs`: a wall marker outranks the document status; the served-page
   override is limited to `202`/`429`; interactive markers are widget-specific
   (`cf-turnstile`, `challenge-platform`, `g-recaptcha`, `h-captcha`, `hcaptcha.com`,
   `recaptcha/api`); a wall that clears while the document status stays `429` is `UsefulContent`.
3. `src/fetch/browser/navigate.rs`: one click of the page's own scripted-verification control per
   render, inside `verification_wait_ms`, limited to `verify` / `start verification` /
   `verify now` / `continue verification`.
4. `src/meta/engines/render.rs`: the `SerpRenderer` contract, `RenderedSerp`, `SerpRenderError`,
   `is_browser_gated`, and `render_blocked`.
5. `src/serp_render/browser.rs`: the implementation over the shared `BrowserLifecycle`, with the
   lifecycle's navigation and verification waits clamped to the request budget.
6. `brave.rs` / `yahoo.rs`: HTTP first, one render when the attempt is browser-gated, the existing
   parser on the rendered DOM. `brave.rs` rebuilds a destination from `cite.snippet-url`.
7. `builders.rs`, `adapter/mod.rs`, `state.rs`: the renderer is installed only when the browser
   feature is compiled, `[fetch.browser].enabled` is true, and a browser was discovered.

## Requirement-to-evidence mapping

| # | Acceptance criterion | Evidence |
|---|---|---|
| 1 | A challenge served with a non-success status is classified, not rate-limited | `brave::tests::challenge_served_with_429_is_blocked_not_a_status_failure`; live release `blocked: challenge page detected (Verifying you're not a bot) on HTTP 429`. |
| 2 | A browser-gated provider returns results on this egress | Release provider matrix: `brave` 5 results in 4.8–5.7 s; `yahoo` 5 results in 3.8–4.2 s, two unrelated queries each. |
| 3 | The render is bounded and the HTTP path is preserved | The budget is the engine request timeout and the lifecycle waits clamp to it; with `[fetch.browser].enabled = false` the same binary reports `brave` blocked in 0.3 s and `yahoo` `returned status 500` in 1.0 s, and `duckduckgo` is unaffected. |
| 4 | No tool, argument, schema property, configuration key, or provider id changes | `tests/static_guards.rs::stable_tool_registration_count_and_names`; no `KNOWN_PROVIDER_IDS` change; no new configuration key. |
| 5 | `fetch` stays independent of `meta` and the browser stays optional | The contract lives in `meta::engines::render`; the implementation is in `src/serp_render/`, above both; `cargo check --no-default-features` passes in `make check`; the renderer is `None` without the feature. |
| 6 | The rendered DOM still satisfies the four-outcome contract | Each engine parses the rendered body with its own `parse` and `finalize_scrape`; `engine_parse_conformance` and all engine unit tests pass. |
| 7 | A render that cannot run reports the HTTP reason, never partial results | `brave::tests::a_render_that_cannot_run_keeps_the_challenge_reason`; `brave::tests::a_served_page_never_consults_the_browser`; `yahoo::tests::an_empty_failure_status_is_served_by_the_browser_when_one_is_installed`. |
| 8 | The identity change is the browser's own, not a third party's | `lifecycle::tests::browser_identity_names_the_detected_chrome_build`; the UA carries the discovered major version and the host platform only. |
| 9 | Gates pass on the exact candidate | `make check` exit 0: 6 567 passed, 0 failed, 28 ignored; `cargo test --features mock` 6 540 passed / 5 ignored; `make docs-check` exit 0; `static_guards` 67 passed; engine unit tests 723 passed. |
| 10 | Hosted qualification is green on the exact candidate | CI `44873922320` (linux `ci`, windows portability, macOS canonical), Python native qualification `44873922304`, egress feature qualification `44873922285`, and release drift guard `44873922502` all succeeded. |

## Live before and after

Release build, one MCP stdio server process per configuration, five results requested.

| Provider | Before this milestone | After |
|---|---|---|
| `brave` | `rate_limited` on HTTP 429 (work package 1: `blocked` on HTTP 429) | 5 results in 4.8–5.7 s across two runs and two queries |
| `yahoo` | `returned status 500` | 5 results in 3.8–4.2 s across two runs and two queries |
| `duckduckgo` | 5 results | unchanged, 0.5–0.7 s |

Render path measured through `web_fetch` with `render: "browser"` on the same release build:

| Origin | Result |
|---|---|
| `search.yahoo.com` | status 200, `transport: "browser"`, title "rust programming language - Yahoo Search Results" |
| `search.brave.com` | `transport: "browser"`, final document carries the results page after one `Verify` click; document status stays `429`, which is the status the origin served, not a failure of the render |

## Hosted qualification

Candidate `364014c6fd9c29e6dd082915bd334ccf9bc148dd` on `feat/auto-20261117-044c4885`:

| Workflow | Run | Result |
|---|---|---|
| CI | `44873922320` | success — linux `ci`, windows portability, and macOS canonical verification all passed |
| Python native qualification | `44873922304` | success |
| Egress feature qualification | `44873922285` | success |
| Release drift guard | `44873922502` | success |

## Residual risk

- **Cost.** A browser-gated search pays one render (roughly 4–6 s measured) instead of a fraction of
  a second. It is bounded by the request deadline and the render caps, and it is paid only for the
  two providers that need it.
- **Identity is an arms race.** Origins can add fingerprint checks below the user-agent layer
  (TLS/JA4, canvas, proof-of-work variants). A future gate may need a different mechanism; the
  failure will surface as a provider-scoped `blocked` or `parse_error`, never as absence.
- **Displayed-URL reconstruction.** When Brave truncates a displayed URL, the reconstructed
  destination degrades to its exact prefix (often the origin). It never invents a path, but it can
  collapse two results from one site into one.
- **Per-search wall cost.** Each render uses a fresh browser context, so Brave's pass cookie does
  not carry between searches and the `Verify` click is paid per search. Session-scoped reuse
  remains available work, not a claim.
- **Origin flakiness.** Yahoo's WAF returned its `500` to the browser once in an early matrix run
  and not on the following runs; the failure is reported as the provider's HTTP status and never as
  an empty result set.

## Verification commands

```sh
RUSTUP_TOOLCHAIN=1.89.0 make check          # exit 0
RUSTUP_TOOLCHAIN=1.89.0 make docs-check     # exit 0
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib engines::
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test static_guards
RUSTUP_TOOLCHAIN=1.89.0 cargo build --release --locked --all-features
```
