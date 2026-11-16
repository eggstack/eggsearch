# M022 — Fetch-Side Challenge Classification

Status: closed
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M022
Primary class: invariant (corrective)
Baseline: `462fe6b` (the commit that closed M021)
References: ADR-0009; ADR-0010; `architecture/fetch.md`; `docs/tool-matrix.md` § `web_fetch`; `src/fetch/client.rs`; `src/fetch/browser/classify.rs`; `src/mcp/tools/web_fetch.rs`; `src/mcp/tools/batch_fetch_item.rs`; `src/core/warning.rs`.
Dependencies: none hard. Interface to M001 (four-outcome scraper contract, `error_class: blocked`) and to ADR-0009/ADR-0010 (solver scope, unchanged here).

## Objective

Make the plain-HTTP fetch path report an anti-bot challenge page as a challenge rather than as an ordinary short document, and let the same classification trigger the existing browser escalation when one is available.

Measured defect. Fetching `https://www.startpage.com/search?q=rust` with the default build returned:

```
status: 200, fetched: true, manual_interaction_required: false
document.text_chars_returned: 116
text: "Loading...\n\nAnubis could not load its JavaScript. The server may be overloaded."
```

The page is an Anubis wall. No warning, no flag, and no escalation: the browser-gated classifier in `src/mcp/tools/web_fetch.rs` never runs without the `browser` feature, and even when it runs `classify_response` recognises Cloudflare/Turnstile/`#app` shells but not Anubis or ALTCHA. An agent asking for the page text receives a two-paragraph stub and no signal that the document was never delivered.

Why the existing verification missed it. The engine-level contract (M001) classifies challenge pages for *search* engines, and the browser path classifies rendered pages. The plain HTTP fetch path had no equivalent, and the fetch tests used ordinary HTML fixtures. The `manual_interaction_required` field existed but was only populated from browser errors, so its success-path value was always `false`.

## Non-goals

- No challenge solving, waiting, token submission, or cookie replay on the fetch path. Solving stays engine-scoped and opt-in under ADR-0009/ADR-0010.
- No new MCP tool, no new tool argument, no change to the `[search.challenge]` or `[fetch]` tables, and no new dependency.
- No `blocked` error class for `web_fetch`: the caller asked for a URL, and the honest answer is "this body is a challenge", not a transport failure.
- No change to the browser classifier's existing dispositions, thresholds, or escalation behaviour for Cloudflare/Turnstile shells.
- No claim that a browser solves any particular wall; the browser test records what is observed.

## Invariants that cannot regress

- Ten MCP tools; `web_fetch` still returns `fetched: true` with the bounded body it received when the response is a challenge page.
- The browser path keeps priority: when a browser is available under `render: "auto"`, escalation runs before the challenge marking is final, and a successful render replaces the challenge payload.
- Detection is read-only over the bounded response body; it adds no request, no retry, and no delay.
- Warning text is sanitized and bounded through `AgentWarning::new`, and names the family, never a cookie or token.
- Ordinary pages, markdown responses, PDFs, and non-HTML bodies are never marked.
- No false positive on the word "loading" alone; the detector uses structural ids and challenge-specific phrases.

## Expected production changes

1. New ungated module `src/fetch/challenge.rs`: `ChallengeKind { Anubis, Altcha, Interactive, Verification }` with `as_str()`, and `detect(title: Option<&str>, body: &str) -> Option<ChallengeKind>`.
   - Anubis: `id="anubis_challenge"`, `id="anubis_version"`, `id="anubis_base_prefix"`, or the phrase `Anubis could not load its JavaScript`.
   - ALTCHA: `captcha-wrap`, the phrase `JavaScript is required to complete this challenge`, or a `captcha/challenge` reference.
   - Interactive: the existing Turnstile/reCAPTCHA/hCaptcha/`verify you are human`/`security check` markers.
   - Verification: `just a moment`, `checking your browser`, `verifying your browser`, `security verification`. The generic `please wait` marker is deliberately excluded to bound false positives.
2. `src/fetch/client.rs`: on the HTML extraction path, classify the bounded body; when a challenge is detected, set `manual_interaction_required: true` and push `AgentWarning::new(WarningCode::FetchChallengePage, "challenge page detected (<family>): the response is an anti-bot challenge, not the requested document")`.
3. `src/core/warning.rs`: new `WarningCode::FetchChallengePage` → `"fetch_challenge_page"`, severity `warning`, recommended action pointing at `render: "auto"` or a browser transport.
4. `src/mcp/tools/web_fetch.rs`: the browser escalation predicate (`render: "auto"`, browser available) also fires when `crate::fetch::challenge::detect` returns a family, so an Anubis or ALTCHA wall escalates instead of being returned as a stub. `render: "http_only"` keeps the classified body and the warning.
5. `src/mcp/tools/batch_fetch_item.rs`: propagate the response flag and warning into each item's payload rather than hardcoding `manual_interaction_required: false`.
6. Docs: `architecture/fetch.md` (new paragraph in the challenge section), `docs/tool-matrix.md` (`web_fetch` bullet), `docs/features.md` (challenge outcome list), `CHANGELOG.md` § Unreleased / `### Fixed`.

## Ordered work packages

1. `src/fetch/challenge.rs` with the detector and a table test (positive per family, negative for ordinary content).
2. Warning code plus its `as_str`, severity, and recommended-action arms, with a wire-string test.
3. Client marking with a unit test over a synthetic Anubis page and an ordinary page.
4. Escalation predicate extension; test that `http_only` never escalates and `auto` does.
5. Batch propagation and its behavioural test.
6. Docs, changelog, registry, roadmap, and closure record.

## Verification

```bash
cargo fmt --all
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features --test web_fetch_integration
cargo test --locked --all-features --test mcp_tools
cargo test --locked --all-features
python3 packaging/check-planning-consistency.py
```

Live, recorded rather than asserted:

```bash
target/release/eggsearch --config /tmp/base.toml fetch "https://www.startpage.com/search?q=rust" --max-chars 400
target/release/eggsearch --config /tmp/browser.toml fetch "https://www.startpage.com/search?q=rust" --render auto --max-chars 400
```

The first must report `manual_interaction_required: true` with `fetch_challenge_page`; the second must record whether the browser render replaced the wall or reported a manual-interaction error.

## Acceptance and stop conditions

- A synthetic Anubis page and a synthetic ALTCHA page are marked on the HTTP path with `manual_interaction_required: true` and one `fetch_challenge_page` warning naming the family.
- An ordinary HTML page and a markdown response are unmarked.
- `render: "http_only"` never escalates; `render: "auto"` with a browser available escalates a detected challenge exactly once.
- `fetched`, `status`, `text`, and `document` for a challenge page are otherwise unchanged, so a caller that ignores the signal sees exactly what it saw before.
- No new request is issued by detection, and no cookie or token appears in the warning text.
- Stop and return for a decision if a legitimate high-traffic page shape is marked by the detector, or if the escalation change causes an existing browser test to change disposition.

## Closure evidence

`plans/closure/agentic-retrieval-integrity/022-status.md` must record the implementation SHA; the detector's positive and negative cases; the HTTP-path payload before and after; the escalation evidence for `http_only` and `auto`; the live fetch results (or their explicit absence); and residual risk (family drift, pages that obfuscate challenge ids, and the fact that detection does not itself obtain the document).
