# M045 — Browser-Fidelity Provider Access

Status: closing (work packages 1 and 2 implemented; local gates and hosted qualification pending)
Date: 2026-11-21
Subsystem: agentic retrieval integrity
Milestone class: correctness (keyless HTML provider access)
Baseline: `3038345f1b0fe90ca79a6b8d7928c5d5e8500a22` on `feat/auto-20261117-044c4885`
Decision authority: accepted ADR-0006 (keyless retrieval is first-class) and ADR-0019 §5
(browser-like identity). No new ADR.

References: `plans/000-long-term-specification.md` §2; `plans/003-planning-process.md` §7;
`plans/adrs/ADR-0006*`, `plans/adrs/ADR-0019-broadened-challenge-coverage-and-browser-identity.md`;
`architecture/engines.md` § Scraper Fidelity; `architecture/fetch.md`; `docs/provider-setup.md`.

## Objective

Make keyless HTML providers usable when the origin gates access on browser execution, instead of
reporting a rate limit or a transport failure for a wall a browser passes.

## Evidence (2026-11-21, this egress, in-app browser)

A real browser passes both origins that our HTTP client fails:

1. **Brave.** `search.brave.com/search?q=…` serves a proof-of-work captcha
   ("Verifying you're not a bot", "Switch to traditional CAPTCHA", `/help/pow-captcha`). The
   browser completed it in a few seconds and returned a full results page; a second search in the
   same profile returned results immediately from the stored pass cookie. Our client receives
   **HTTP 429 with the same 80 KiB challenge document**, and `src/meta/engines/brave.rs` classified
   the status without reading the body, so the outcome was `rate_limited`.
2. **Yahoo.** `search.yahoo.com/search?p=…` rendered a complete results page in the browser with no
   visible challenge. Our client receives `500` with a zero-byte body over HTTP/2, and a five-hop
   `_bv/v.gif` bot-verification redirect chain ending in `500` over HTTP/1.1. Full Chrome headers,
   `sec-ch-ua`, `sec-fetch-*`, matching user agent, compression, a seeded cookie jar, and a referer
   do not change it, so the difference is below the header layer (TLS/HTTP2 fingerprint or the
   scripted verification).
3. Curl is not a valid oracle for these origins: it fails where the browser succeeds. Browser
   observations are the reference.

## Work package 1 — challenge responses on non-success statuses (complete)

- `src/meta/engines/mod.rs` adds `classify_search_failure()` and `read_search_body()`: the body is
  read for any status and the shape's blocked markers decide between `Blocked` (reason names the
  marker and the status) and `BadStatus`.
- `brave.rs` and `yahoo.rs` use it; Brave's markers now match the live page ("Verifying you're not a
  bot", "Quick check before you continue searching.", "Switch to traditional CAPTCHA").
- `brave.rs` gains `search_at()` so the behaviour is covered with a mock 429 challenge and a mock
  empty 500.

This fixes the classification defect. It does not produce results, because the challenge still has
to be completed.

## Work package 2 — browser-rendered SERP access (implemented)

A browser passes both origins because it executes their JavaScript. The product already ships a
browser behind the `browser` feature for `fetch`. Two observations from this egress fixed the
design: a launched Chromium that announces `HeadlessChrome/…` is refused outright by Yahoo, and
Brave's proof-of-work page does not advance on its own — it completes only after its own `Verify`
control is clicked, then returns results in about 1.5 s.

Resolved design questions:

- **Fallback, not a new provider.** `brave` and `yahoo` keep their provider ids, capabilities, and
  positions in `default_providers`. No tool, argument, schema property, configuration key, or
  provider id is added.
- **Contract in `meta`, implementation above `meta` and `fetch`.** `meta::engines::render` owns
  `SerpRenderer`, `RenderedSerp`, and the escalation predicate `is_browser_gated`.
  `src/serp_render/browser.rs` implements it over the shared `BrowserLifecycle`. `src/fetch/**`
  stays independent of `src/meta/**`, and `challenge_replay` keeps its own ownership.
- **Optional browser.** The implementation is behind the `browser` feature and the renderer is
  `None` unless `[fetch.browser].enabled` is true and a browser was discovered. A build without
  the feature reports the HTTP failure exactly as before.
- **One render per search, inside the request deadline.** The render budget is the engine request
  timeout, and the lifecycle's navigation and verification waits are clamped to it.
- **Reported path.** A render-served batch logs `provider result page served by browser render`
  at debug level with the final URL and whether a wall was completed; a failed render keeps the
  HTTP reason in the provider error, never a partial result set.

What changed:

- `src/fetch/browser/lifecycle.rs` passes `--user-agent` built from the discovered major version
  and host platform, so the browser presents the identity of the Chrome it is, and passes every
  valued flag to chromiumoxide as a `(key, value)` pair. Before this slice a literal
  `"--key=value"` argument became `----key=value` and was silently ignored, so `--user-data-dir`
  and the autoplay policy never reached Chrome.
- `src/fetch/browser/classify.rs` lets a wall marker outrank the document status and limits the
  served-page override to `202`/`429`. Interactive markers are now widget-specific, so a page that
  offers a traditional CAPTCHA beside its scripted challenge is not refused.
- `src/fetch/browser/navigate.rs` clicks the page's own scripted-verification control at most once
  per render, inside `verification_wait_ms`.
- `src/meta/engines/render.rs`, `src/meta/engines/brave.rs`, `src/meta/engines/yahoo.rs`,
  `src/meta/adapter/builders.rs`, `src/meta/adapter/mod.rs`, `src/serp_render/`, and
  `src/mcp/state.rs` carry the contract, the fallback, and the wiring.
- `brave.rs` rebuilds a result destination from `cite.snippet-url`: the rendered page links to an
  opaque `/go/v1#…` token, and the displayed URL is the only readable statement of the target.

## Non-goals

- No interactive CAPTCHA answering, OCR, third-party solving service, credential bypass, proxy
  rotation, or user-agent spoofing beyond ADR-0019's browser-like identity.
- No change to the four-outcome contract or the ten-tool surface.
- No default-provider change; Brave and Yahoo stay where they are until the rendered path is
  qualified.

## Verification

```sh
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib engines::brave
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib engines::yahoo
RUSTUP_TOOLCHAIN=1.89.0 make check
RUSTUP_TOOLCHAIN=1.89.0 make docs-check
```

Work package 1 additionally needs a live release-build check that Brave now reports `blocked` with
the status in the reason instead of `rate_limited`.

Work package 2 adds:

```sh
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib engines::
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test static_guards
RUSTUP_TOOLCHAIN=1.89.0 make check
RUSTUP_TOOLCHAIN=1.89.0 make docs-check
```

and a live release-build provider matrix on this egress: both providers return results for two
unrelated queries, and the same origins render through `web_fetch` with `render: "browser"`
carrying `transport: "browser"`. Mock coverage holds the behaviour without a network: a 429
challenge and a 500 body are each served from a fixture renderer, a renderer that cannot render
keeps the HTTP reason, and a served HTTP page never consults the browser.

## Acceptance and stop conditions

- Work package 1: a mock 429 challenge yields `Blocked` with "challenge page detected" and "HTTP
  429"; an empty 500 stays `BadStatus`; the live release build shows the same change.
- Work package 2: a browser-rendered Brave and Yahoo search returns results on this egress, with the
  render bounded and the HTTP path preserved.
- **Stop:** the rendered path cannot be added without changing the ten-tool surface, inverting the
  `fetch`/`meta` dependency, or making the browser mandatory. Record the blocker and propose a
  separate decision. *(Not triggered: no tool, argument, schema property, configuration key, or
  provider id changed; `src/fetch/**` still imports nothing from `src/meta/**`; the renderer is
  `None` without the feature, without a discovered browser, or with the browser disabled.)*

## Required closure evidence

Work package 1 is recorded in this milestone's closure when work package 2 lands, or in a separate
record if the milestone is split. Either way the record carries the browser observations, the live
before/after classification, the mock coverage, the gates, the hosted run, and residual risk.
