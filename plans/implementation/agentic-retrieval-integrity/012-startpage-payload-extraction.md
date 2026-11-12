# M012 — Startpage Payload Extraction

Status: ready
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M012
Primary class: capability
Baseline: `d751bd7471a647e651ee41b8e51199c66d04d01a` (re-read `git status` before starting)
References: ADR-0009; ADR-0006; `architecture/engines.md` § Scraper Fidelity and § Challenge solving; `src/meta/engines/startpage.rs`; `src/meta/engines/mod.rs`; `tests/engine_parse_conformance.rs`.
Dependencies: hard M011 for capture and live verification (a solved session is the only way to see the SERP). Interface: the pass-cookie replay contract delivered by M011.

## Objective

Make a solved Startpage SERP parseable. The current parser expects `div.result` / `h2.wgl-title` markup, but the served page builds its results from an inline React payload; without this milestone, raising `max_difficulty` buys a pass cookie and still yields no rows.

## Non-goals

- No news, images, or video categories; only the web/mainline payload is in scope.
- No paging beyond the first response, no query-parameter surface extension.
- No new provider, no replacement of the DOM parser as the only path, and no new dependency (`serde_json` is already present).
- No behavioural change when a challenge is unsolved: the provider stays `blocked`.

## Invariants that cannot regress

- The four-outcome contract holds: rows, explicit empty state, `ParseFailed`, `Blocked`. A payload that fails to parse is a structural break, never an empty result.
- A challenge page (Anubis) is still classified `blocked` before any payload work happens.
- Extracted title and snippet text pass through the existing sanitizer and bounding path; payload fields are untrusted input.
- The response body stays behind `read_bounded_body` with the existing 2 MiB cap, and payload scanning is bounded by that buffer.
- Fixture provenance stays honest: a payload fixture may only be described as live-captured when it came from a solved session, with URL and UTC date recorded.

## Expected production changes

1. **Extraction module.** New `src/meta/engines/startpage_payload.rs` exposing `pub(super) fn parse_payload(html: &str, max_results: usize) -> Option<Vec<SearchResult>>`. It locates the embedded `React.createElement(UIStartpage.AppSerp<Categ>, {…})` argument, wraps it into a JSON object as the upstream does, and reads `render.presenter.regions.mainline[].results[]`, keeping entries whose `display_type` starts with `web`.
2. **Field mapping.** `clickUrl` → `url` (keep only `http(s)`), `title` → `title` (HTML-to-text, trimmed, non-empty required), `description` → `snippet` (HTML-to-text, whitespace-normalised, bounded). Entries without a usable URL or title are skipped, exactly like the DOM path.
3. **Parser choice.** `startpage.rs` tries the payload first when it is present; when it is absent it keeps the existing DOM selectors as a fallback. Both paths converge on `finalize_scrape(ENGINE, &document, &SHAPE, results)`, so empty and blocked classification is unchanged.
4. **Fixtures.** Add `tests/fixtures/engines/startpage/results.html` re-captured from a solved session, trimmed to two result blocks, with the payload reduced to the fields the parser reads and all tokens/identifiers removed. Keep the synthetic fixtures as the drift and blocked cases. Record provenance in `tests/fixtures/engines/README.md`.
5. **Conformance.** Extend the table-driven case for `startpage` with a payload-shaped fixture and a seeded drift that renames a payload field the parser requires, so the negative control proves the parser can notice shape loss.

## Storage, protocol, and compatibility effects

No schema, tool, or transport change. The additional module is private to the engine layer. Existing Startpage DOM fixtures keep working through the fallback path.

## Ordered work packages

1. Capture a solved SERP through the M011 pass cookie; store the raw body outside the repo (`target/`) and derive the committed fixture from it.
2. `startpage_payload.rs`: locator, JSON shaping, mainline walk, field mapping, with unit tests for happy path, missing payload, and partial entry.
3. Wire payload-first with DOM fallback in `startpage.rs`; prove both paths reach `finalize_scrape`.
4. Drift tests: renamed `clickUrl`/`title` field and a truncated payload must both yield `ParseFailed`, not empty.
5. Empty-state test: a solved page with no mainline results must map to the documented empty marker path, not a parse failure.
6. Conformance and fixture README updates.
7. Docs: `docs/provider-setup.md` Startpage section (what enabling now buys), `architecture/engines.md` § Challenge solving pointer, `docs/test-inventory.md`.

## Verification

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features --test engine_parse_conformance
cargo test --locked --all-features --lib meta::engines::startpage
cargo test --locked --all-features
```

Live, opt-in, recorded (requires M011's `enabled = true`, `max_difficulty = 6`, `timeout_ms = 8000`):

```bash
eggsearch --config /tmp/anubis-6.toml search "rust programming language" --providers startpage --json
```

## Acceptance and stop conditions

- With solving enabled and the ceiling permitting the published difficulty: a live Startpage search returns parsed rows with titles, URLs, and snippets, and `provider_status` reports `parse_shape = ok`.
- With solving disabled or above the ceiling: still `blocked`, with the ceiling reason where applicable — no regression from M011.
- A payload whose required fields are renamed yields `parse_error`; a solved page with genuinely no results yields the documented empty state.
- No cookie, token, or account identifier appears in a fixture.
- Stop and return for a decision if the payload shape differs materially from the recorded one (for example no `mainline` region), rather than guessing a second shape.

## Closure evidence

`plans/closure/agentic-retrieval-integrity/012-status.md` must record: candidate SHA, fixture provenance (URL, UTC date, HTTP status, solved-difficulty), the payload and drift test results, the live search output summary, and residual risk (payload churn, category coupling, and the fact that upstream may change both the wall and the payload independently).
