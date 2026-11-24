# M049 — Non-Success Block Classification for Mojeek

Status: active (work package 1 implemented; canonical gate outstanding)
Date: 2026-11-23
Subsystem: agentic retrieval integrity
Milestone class: correctness (provider error classification)
Baseline: `7ec251df2ba715705468f2eb1e5392fa87fc4d32` on `docs/user-facing-polish-20261123`
Decision authority: `architecture/engines.md` § Scraper Fidelity; M045 work package 1 establishes
the same rule for `brave` and `yahoo`. No new ADR.

References: `plans/000-long-term-specification.md` §2; `plans/003-planning-process.md` §7;
ADR-0006 (keyless retrieval is first-class); `architecture/engines.md` § Scraper Fidelity;
`docs/provider-setup.md`; M008 closure record.

## Objective

Make `mojeek` report a refusal that arrives with a non-success status as `blocked`, naming the
status and the matched marker, instead of `http_status`. This brings the engine in line with the
rule `architecture/engines.md` already states and that `read_search_body()` already implements.

## Evidence (2026-11-23, this egress)

One bounded, keyless request to `https://www.mojeek.com/search?q=Rust+programming+language+official+website`
with the shipped search user agent and `Accept-Language: en-US,en;q=0.9`:

- `HTTP/1.1 403 Forbidden`, `Content-Length: 363`, `Content-type: text/html; charset=UTF-8`,
  `Server: lighttpd/1.4.53`
- Body: `<title>403 - Forbidden</title>` … `<h2>Sorry your network appears to be sending automated
  queries so we can't process your search at this time.</h2>` … `/about/contact?refr=403`
- Body SHA-256 `50666d78ab85d7af911166fa712af887dedcf4a2751df8927992a4d253548691`

That body matches the `PageMarker::Text("your network appears to be sending automated queries")`
marker already declared in `mojeek.rs` `SHAPE.blocked_markers`. `send_search()` returned
`EngineError::BadStatus { status: 403 }` before reading the body, so the marker never ran. The same
observation is recorded against M008: thirteen of eighteen Mojeek attempts were `http_status`.

## Why Mojeek refuses this egress (research, 2026-11-23)

1. `https://www.mojeek.com/robots.txt` (live, HTTP 200) declares `User-agent: *` /
   `Disallow: /search`. The HTML search page is not offered to automated clients at all.
2. Mojeek gates `/search` with an ALTCHA proof-of-work wall served with **HTTP 200**
   (`div.captcha-wrap`), added 2026-06-23 and cited on their forum as a response to roughly 100
   automated searches per second.
3. Continued automated traffic escalates to a hard `403` carrying the exact page captured above.
   Mojeek's own forum documents the same page for datacenter and VPN egress and, separately, for
   single over-broad bot rules that fire on the query text.
4. The supported automated route is the Mojeek Search API (`https://api.mojeek.com/search`,
   `api_key`, JSON/XML, PAYG from £2 per 1,000 requests). Their terms permit automated access only
   by "an authorised Mojeek API user".

Conclusion: the `403` is not parser drift and not a solvable challenge. It is also **not**
network-wide and **not** user-agent-wide: that was this plan's first reading, and the controls below
disprove it.

## Mechanism controls (2026-11-23)

Public address `96.236.59.113`, identical for the in-app browser and the CLI.

| Client | Query | Outcome |
|---|---|---|
| in-app Chromium 148 (`… MiniMaxAgent/3.1.17 … Electron/42.11.16`) | `/` | `200` homepage |
| in-app Chromium | probe string | `403`, repeatable |
| in-app Chromium | `test`, `rust`, `official website`, `rust programming language`, a long benign sentence, `nq:test2352` | `200` ALTCHA wall |
| in-app Chromium | `nq:test2352`, reloaded and re-requested | `200` ALTCHA wall |
| curl, stale Windows Chrome/124 UA | probe string | `403`, byte-identical |
| curl, current macOS Chrome/148 UA + `sec-ch-ua` + `sec-fetch-*` | probe string | `403`, byte-identical |
| eggsearch release client | `nq:test2352` | five real results, no provider failures |
| eggsearch release client, post-fix | probe string | `blocked`, "…on HTTP 403" |

The outcome tracks the query, not the client. Changing the user-agent string, its version, or the
client hints changes nothing, so the shipped identity is not the cause and does not need to change.
Sustained automated bursts remain the documented escalation path, and that is the most likely
explanation for M008's thirteen refusals from its own 72-attempt cohort. Controls retained in
`plans/closure/retrieval-quality-evidence-extension/evidence/mojeek-403-mechanism-controls.json`.

## Work package 1 — classification (this milestone)

- `src/meta/engines/mojeek.rs` `send_search()` uses `super::read_search_body(resp, ENGINE, &SHAPE,
  MAX_BODY_BYTES, &[])` instead of short-circuiting on `!status.is_success()` and then reading the
  body. A non-success body that matches a blocked marker becomes
  `Blocked { reason: "challenge page detected (<marker>) on HTTP <status>" }`; a non-success body
  that matches no marker stays `BadStatus`.
- A `200` challenge page keeps its existing path: body returned, `is_challenge_page()` true, ALTCHA
  solver and single replay. A `403` block page no longer reaches the solver, because a network block
  is not a solvable challenge.
- Two unit tests: a `403` block page yields `Blocked` naming the marker and `HTTP 403` with zero
  solve or verify requests; an empty `403`/no-marker failure stays `BadStatus`.

M045 work package 1 covered `brave` (429 proof-of-work) and `yahoo` (500). `duckduckgo` and
`startpage` are unaffected in practice because their walls arrive with success statuses (`202` and
a `200` redirect target). They keep their current code; the shared helper is ready if that changes.

Records updated with the change: `tests/fixtures/engines/README.md` (capture provenance and why the
body must be read on a non-success status), `docs/provider-setup.md` (an operator caveat that the
solver cannot clear a network refusal), the M008 closure correction, this roadmap, and
`plans/registry.md`. The captured response is retained in
`plans/closure/retrieval-quality-evidence-extension/evidence/mojeek-network-block-403-capture.json`.

## Non-goals

- No provider default, enablement, or capability change. Mojeek stays disabled;
  defaults stay `[duckduckgo, brave, yahoo]`.
- No Mojeek API key, no paid provider, no credential.
- No browser escalation for Mojeek: a browser on the same egress receives the same network block.
- No rerun of the M008 held-out cohort.

## Acceptance and stop conditions

- A mock `403` carrying the captured block page yields `Blocked` whose reason contains
  `challenge page detected`, the marker text, and `HTTP 403`.
- A mock `403` with no marker stays `BadStatus { status: 403 }`.
- Mock `200` challenge-page, cached-cookie, rejected-cookie, and exhausted-attempt paths are
  unchanged.
- `cargo fmt --check`, `cargo clippy`, the `mock` feature test suites, and `make docs-check` pass.
- **Stop:** the classification cannot be reached without reading an unbounded body, or the change
  alters the advertised tool surface, provider enablement, or defaults.

## Recorded correction

`plans/closure/retrieval-quality-evidence-extension/008-status.md` gains an appended correction: its
thirteen Mojeek `http_status` observations are this class of refusal and classify as `blocked`. The
runner kept no bodies, so the correction states what was captured rather than claiming all thirteen
were byte-identical. The negative M008 outcome stands; the qualification gate is unchanged.
