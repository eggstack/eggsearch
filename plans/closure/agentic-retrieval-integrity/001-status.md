# M001 Scraper Fidelity Contract — Closure Status

Status: closed

Source plan: `plans/archive/implementation/agentic-retrieval-integrity/001-scraper-fidelity-contract.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Planning baseline: `cd81f4d5e966364918fe2be25485b8ebbf424e71` (origin/main, 2026-11-10 UTC)

Implementation candidate: `c9912fc0e4824c4ec6b1231be2057fb4cd0ba8ab`, the implementation commit immediately preceding this
closure commit. The closure record, roadmap status, and registry entry moved in the same closure
commit.

## Result

All five HTML scrapers (`brave`, `duckduckgo`, `mojeek`, `startpage`, `yahoo`) now return one of
four outcomes instead of collapsing two of them into `Ok(vec![])`:

| Outcome | Return |
|---|---|
| Rows parsed | `Ok(rows)` |
| Documented empty-state marker matched | `Ok(vec![])` |
| Structural break | `Err(ParseFailed { reason })` naming the container |
| Challenge / consent / anti-bot page | `Err(Blocked { reason })` |

`src/meta/engines/mod.rs` owns `ScrapeShape`, `PageMarker`, and `finalize_scrape()`, so the decision
has one implementation; each scraper declares a `SHAPE` constant beside its parser. Text markers are
matched against rendered text with `<script>`/`<style>` excluded. `EngineError::Blocked` maps to
`ErrorClass::Blocked` (`blocked`) and `FailureClass::Blocked`; `ParseError` maps to
`parse_shape = shifted`.

`provider_status` reports `parse_shape` (`ok`, `empty`, `shifted`) per observed provider, derived
from the most recent classified outcome and omitted when unobserved.

## Fixtures

15 fixtures under `tests/fixtures/engines/`, provenance recorded in the fixture README:

- Live-captured (2026-11-10 UTC, status and byte counts recorded): `brave/results.html`,
  `brave/empty.html`, `duckduckgo/blocked.html` (202 anomaly page), `mojeek/blocked.html` (CAPTCHA
  page), `startpage/blocked.html` (302 → CDN block page).
- Synthetic reconstructions (10): DuckDuckGo, Mojeek, Startpage, and Yahoo results/empty pages,
  because those origins refused automated requests (202/302/403/500) from this network; plus
  `brave/blocked.html` and `yahoo/blocked.html`, which could not be provoked on demand.

No fixture claims a capture that did not happen. Each file is a trimmed, redacted fragment (224–1,467
bytes) limited to what the parser needs.

## Why the original verification missed it

Baseline engine tests used miniature hand-written HTML that always contained the expected
containers, no captured provider markup existed, and no test asserted the empty-state path. A
zero-result search and a renamed CSS class were therefore indistinguishable to every existing test.

## Before/after ledger outcome (simulated drift)

- Before: drifted container → `Ok(vec![])` → `SuccessZeroResults` → `retrieval_status` reported
  `no_matching_evidence_found`, i.e. confident absence.
- After: drifted container → `ParseFailed { reason: "result container 'div.result' missing" }` →
  attempt outcome `Failed`, `error_class: "parse_error"`, `providers_failed` entry and
  `[parse_error] …` warning → dimension `absence_kind: provider_failed` (indeterminate), never
  `no_matching_evidence_found`.
- A blocked page follows the same failure path with `error_class: "blocked"` and a rate-limit
  cooldown.
- A genuine zero-result search still reports `SuccessZeroResults` and
  `no_matching_evidence_found`; `web_search_zero_results_stays_success` asserts it.

## Verification (combined candidate)

- `cargo test --locked --features mock --test engine_parse_conformance`: 6 passed.
- `cargo test --locked --features mock --test provider_probe_conformance --test provider_routing`:
  20 + 87 passed (4 new `parse_shape` tests).
- `cargo test --locked --features mock --test web_search_integration --test keyless_core`: 70 + 23
  passed, including the blocked end-to-end case, the parse-failure ledger assertions, and the
  genuine-zero-result invariant.
- `cargo test --locked --all-features --test dispatch_fault_injection --test retrieval_attempt_ledger
  --test provider_capability_contract`: 34 + 46 + 23 passed.
- `cargo test --locked --all-features --test static_guards`: 60 passed.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: clean.
- `make check`: fmt, clippy, feature-check, the full `--all-features` suite (5,848 passed, 26
  ignored, 0 failed), and hygiene passed. It stops at `dependency-policy`, which fails on
  pre-existing advisories (`RUSTSEC-2026-0462` for rmcp 3.2.0; yanked `async-executor 1.14.0`).
  `Cargo.lock` is unchanged.

## Safety evidence

No new dependency, no ranking or RRF change, no tool-schema change, no retry widening, no user-agent
spoofing, no CAPTCHA solving, and no proxy rotation. Engine requests keep `read_bounded_body` and the
existing byte caps. Ten MCP tools, stable IDs, sanitization tiers, and trust framing are untouched.
`error_class` gained one additive value; no existing wire string changed.

## Residual risk

- Empty-state markers for DuckDuckGo, Mojeek, Startpage, and Yahoo are reconstructions, not
  observations. A wrong marker degrades to `parse_error` (failure, never false absence), but the
  first live capture should confirm them.
- Text markers are English-only; a localized empty or block page misses its marker and degrades to
  `parse_error`. That is the safe direction but unverified against localized markup.
- `provider_status` has no projection allowlist, so `parse_shape` appears in all detail levels. That
  is additive and diagnostic-only.
- `parse_shape` records the most recent shape-bearing outcome, not a drift-agreement window.
