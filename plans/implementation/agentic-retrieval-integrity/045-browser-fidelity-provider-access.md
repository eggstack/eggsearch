# M045 — Browser-Fidelity Provider Access

Status: active (work package 1 complete; work package 2 designed, not started)
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

## Work package 2 — browser-rendered SERP access (not started)

A browser passes both origins because it executes their JavaScript. The product already ships a
browser behind the `browser` feature for `fetch`. The proposed slice:

- Render the provider SERP in the existing browser and extract result containers from the rendered
  DOM, instead of requesting the HTML with the HTTP client.
- Cache the browser session per origin so a completed wall (Brave's PoW) is not repaid on every
  search; reuse must respect the existing one-solve-per-window and deadline rules.
- Keep the HTTP client path as the default and the fallback when the browser is unavailable, and
  report which path served the result.
- Bound the render with the existing render timeout and byte/character caps.
- Change no tool, argument, or schema property. A provider id or capability flag is only added if
  the rendered path is a separate selectable provider.

Open design questions for the implementer: whether the rendered path is a new provider or a
fallback inside the existing HTML engines; how a browser handle reaches `SearchEngine::search`
without a dependency inversion (`fetch` is currently independent of `meta`); and how the browser
feature stays optional for builds without it.

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

## Acceptance and stop conditions

- Work package 1: a mock 429 challenge yields `Blocked` with "challenge page detected" and "HTTP
  429"; an empty 500 stays `BadStatus`; the live release build shows the same change.
- Work package 2: a browser-rendered Brave and Yahoo search returns results on this egress, with the
  render bounded and the HTTP path preserved.
- **Stop:** the rendered path cannot be added without changing the ten-tool surface, inverting the
  `fetch`/`meta` dependency, or making the browser mandatory. Record the blocker and propose a
  separate decision.

## Required closure evidence

Work package 1 is recorded in this milestone's closure when work package 2 lands, or in a separate
record if the milestone is split. Either way the record carries the browser observations, the live
before/after classification, the mock coverage, the gates, the hosted run, and residual risk.
