# M055 — Fetch Extraction Completeness and Render-Path Status Correctives

Status: active
Date: 2026-11-26
Subsystem: agentic retrieval integrity
Milestone class: invariant (corrective program)
Baseline: commit `b04e996` (`feat/auto-20261126-82ca75e6`, release build 0.4.5, macOS arm64)
Corrects: M039 (`plans/archive/implementation/agentic-retrieval-integrity/039-fetch-emitted-payload-budget.md`), M042 (`plans/archive/implementation/agentic-retrieval-integrity/042-fetch-item-shape-diagnostics.md`), M052 (`plans/implementation/agentic-retrieval-integrity/052-agent-facing-contract-ergonomics.md`)
Companion plan: M054 (`plans/implementation/agentic-retrieval-integrity/054-agent-facing-payload-economy-correctives.md`) corrects payload size and representation; this plan corrects what the extractor keeps and how it reports what it kept.
Trial evidence: live stdio MCP sessions against the release binary on 2026-11-26, plus a deterministic fixture served from loopback with `[fetch].allow_localhost = true`; artifacts under `/tmp/egg-trial/` on the trial host (not committed).

Decision authority: ADR-0022 (agent-facing contract), AGENTS.md § Conventions (scraper fidelity: four outcomes; bounded I/O), `plans/003-planning-process.md` §7.

## Implementation status (2026-11-26, partial)

Status is `active`: WP0, WP1, WP3, and WP4 landed with tests; WP2 landed in part.

| WP | State | What landed | What remains |
|---|---|---|---|
| WP0 | done | the walk emits text runs that are direct children of an unrecognised container, interleaved with recursion into element children, so `div class="card"` holding `span`s produces paragraphs; a fixture asserts every marker exactly once, the `aside` excluded, and no `extraction_sparse` on a complete page | the proptest for duplicate emission over random nesting |
| WP1 | done | include-selector roots of any element type produce text through the same walk; a matched-but-empty selector emits `selector_no_blocks`; tests cover `.card-title`, `.card`, `li`, `p`, and an empty element; a root nested inside another matched root is walked once | - |
| WP2 | partial | `extraction_sparse` and `selector_no_blocks` typed codes, a bounded DOM-text estimate (100k character ceiling) measured against the same content root the walk uses and only for unscoped extraction, a 5% threshold, and the negative case asserted on a complete page; a matched-but-empty selector emits only `selector_no_blocks` | `render_suspected` for the `auto` policy, and the recorded control-page ratio table |
| WP3 | done | `BrowserFetchError::HttpStatus` plus a gate after challenge resolution and before DOM extraction, mapped to the HTTP transport's exact shape (`upstream_failed: http_status: HTTP error: 404`); challenge and script-wall pages keep precedence; a unit test pins the mapping | the browser-fixture 404 parity test through the injected-browser harness |
| WP4 | done | `representation` publishes `pdf_text` for PDF text and `document` for container documents including CSV, `html` is documented as including browser-rendered DOM, the field doc states the mapping, and `docs/tool-matrix.md` documents it | - |
## Review corrections (2026-11-26)

An independent review found scope defects in the new sparse signal and the new selector walk. Both are corrected and pinned by fixture tests.

- WP2: `bounded_dom_text_chars` started at `<body>` while extraction walked the selected content root, so `div`-based chrome outside `<main>` inflated the denominator and any selector-scoped extraction could be reported as a structural break. The denominator now comes from `select_content_root`, and the ratio is evaluated only when no include selectors scoped the extraction; a matched-but-empty selector emits `selector_no_blocks` alone instead of both codes.
- WP1: a root nested inside another matched root was walked twice, which duplicated its text once any element type began emitting blocks. Nested roots are now skipped.
- WP4: CSV reaches the raw-text branch and kept `representation: null`; it now sets `document`, matching the documented mapping.
- Tests: `selector_scoped_extraction_is_not_reported_as_sparse`, `nested_include_selectors_emit_text_once`, and `csv_fetch_reports_the_document_representation` in `tests/web_fetch_integration.rs`.

## Objective

Restore the promise that a successful `fetch` returns the page's actual content, and that a
reduced or failed extraction is visible as such. The trial found that the HTML extractor
emits blocks only for headings, paragraphs, list items, tables, and code, so any body
rendered in `div`/`span` disappears silently; that `include_selectors` reports matches
while returning nothing for the most natural selectors; and that the browser transport
returns HTTP error pages as successful fetches. All three produce `ok: true` responses
that are wrong in ways an agent cannot detect.

This plan records the evidence and authorises no code change. It does not change tool
inventory, argument acceptance, provider set, trust framing, or the payload budget.

## Evidence

All character counts are the JSON text content of one `tools/call` response or of the
`text` field inside it, measured on the release binary with keyless defaults.

| Finding | Measured evidence | Source | Why prior verification missed it |
|---|---|---|---|
| E1 (P1) Default extraction drops `div`/`span` body content. | Deterministic fixture (`/article.html`): text 320 chars contains `PARA_ALPHA` (paragraph in `<main>`), `ITEM_DELTA` (list item), and both table cells; it does **not** contain `CARD_BRAVO` or `CARD_CHARLIE`, which are a `div class="card"` holding two `span`s inside the same `<main>`. Live: `quotes.toscrape.com` serves 11,064 bytes with 10 `class="quote"` blocks (verified with curl + grep for the exact quote text), yet `fetch` returns 115 chars — headings and sidebar only — with `ok: true` and no content warning. | `src/fetch/render/blocks.rs:294-332` (`select_content_root` probes `main`/`article`/`[role=main]`/`body`), `src/fetch/render/blocks.rs:378-470` (`walk_node` emits blocks only for `h1..h6`, `p`, `li`, tables, `pre`) | Extraction tests assert that known text appears for pages built from block tags; no test asserts that a page whose body is `div`/`span` retains its text, so silent omission passes |
| E2 (P1) `include_selectors` reports matches and returns nothing. | Deterministic fixture: `.card` matched 1, `.card-title` matched 1, `li` matched 1 — each returned 84 chars (sentinels only, zero content), `ok: true`, no warning. Live: `lobste.rs` `.u-url` matched 25 and returned 68 chars. Positive control: `p` matched 2 and returned 245 chars with both paragraphs. Browser render of `quotes.toscrape.com/js/` with `.quote .text` matched 10 and returned 81 chars. | `src/fetch/render/blocks.rs:100-127` walks only `matched.roots`; `walk_node` has no arm for `div`/`span`/`a`/`li`-as-root, and `selector_no_match` is emitted only when `roots.is_empty()` (`src/fetch/render/blocks.rs:108-110`) | Selector tests cover parse/validation errors and block-level selectors; the matched-but-empty case has no assertion |
| E3 (P1) No coverage signal on a reduced extraction. | E1 and E2 both return `ok: true`, `matched_roots` present and non-zero, no warning. `matched_roots: 10` therefore reads as success while the payload is empty. Nothing in the response distinguishes "page is 115 chars" from "extractor dropped 5,000 chars". | `src/fetch/render/blocks.rs` warning set (`selector_no_match`, truncation only) | Warnings were specified for truncation and no-match, not for disproportionate extraction |
| E4 (P2) Browser transport returns HTTP error pages as success. | `https://quotes.toscrape.com/no-such-page` under `render: "browser"`: `status: 404`, `fetched: true`, `ok: true`, `text` = "Not Found / The requested URL was not found…" (221 chars). The same URL under `http_only` returns `ok: false` with `upstream_failed: http_status: HTTP error: 404 HTTP 404` and no text, so the two transports disagree about the same resource. | `src/fetch/browser/navigate.rs:379-383` captures status; `navigate.rs:509-524` carries it into the result without classification; `src/fetch/client.rs:559-561` is the HTTP-path rejection | Browser tests cover navigation, policy, and challenge paths; none asserts status classification parity with HTTP |
| E5 (P2) Browser rendering does not repair the block-only limitation. | `quotes.toscrape.com/js/` renders quotes after JS (the page has no static quote text), but `render: "browser"` returned the same 104-char shell as `http_only`; `.quote .text` matched 10 in the rendered DOM and still produced 0 chars. The extractor runs on the serialized DOM through the same block-only walk. | `src/fetch/browser/navigate.rs:438-441` (`page.content()`), then the shared renderer | The browser path was verified for transport and screenshot, not for extraction parity with the DOM it captured |
| E6 (P2) `render: "auto"` does not escalate on a JS-only page and says nothing. | `quotes.toscrape.com/js/` under `render: "auto"` returned in 1 ms with the same 104-char shell as `http_only`, `browser_escalated: false`, and no warning. The page carries no static quote text, so the caller receives boilerplate with no signal that the content is missing. | `src/fetch/browser/escalation.rs` | Escalation is tested for challenge and script-wall pages, not for a page that is text-bearing but content-empty until JS runs |

Impact is layout-specific, not universal: table- and list-based pages survive. In the same
trial, Hacker News (4,079 chars) and `lobste.rs` (3,872 chars) retained their story titles,
Wikipedia retained its infobox and lead text (3,806 chars), and a GitHub repository page
retained 3,477 chars. The loss concentrates on modern card/SPA layouts that render body
copy in `div`/`span`, which is exactly the layout browser rendering exists to serve.

Confirmed positives to preserve: `browser` transport works (screenshot returned as an MCP
image block, 75,924 base64 chars ≈ 57 KB PNG, 5.7 s); an explicitly invalid
`[fetch.browser].executable` yields `browser_unavailable` naming the path and the config key
with no silent fallback; loopback targets are blocked under browser rendering even when
`allow_localhost = true` (policy bypass is not possible through the render path); and a
clean server shutdown left no new Chrome processes.

## Contract and invariants

- Keep the nine advertised tools, three aliases, argument sets, provider ids, default
  provider set, trust framing, and content-derived IDs unchanged.
- Preserve M039 and M054: added text counts toward `payload_chars`, `max_total_chars`
  still bounds the response, and clipping still degrades `document`/`lines` before
  requested text.
- Keep the four-outcome rule from AGENTS.md § Scraper Fidelity for extraction: results,
  explicit empty state, structural break, or blocked page. A reduced extraction is a
  structural break and must be machine-readable.
- Extraction stays deterministic and bounded: no new network I/O, no new dependency, no
  unbounded DOM walk, and every emitted block still passes the existing sanitizer and
  budget.
- Never widen a selector's meaning: `include_selectors` keeps selecting the same nodes;
  this plan only makes matched nodes produce output.
- Browser status classification must match the HTTP transport's rule (non-2xx is a
  failed item carrying the status), not invent a second rule.

## Work packages

### WP0 — Emit text for container and inline content (P1, M)

Make the default walk emit body text that is not inside a recognised block tag. A
container such as `div class="card"` that holds only `span`s must produce paragraph
blocks, and text must not be duplicated when both a container and its descendants are
visited. Preserve the existing block types and their order; do not turn the output into
one blob per container.

Acceptance: the deterministic fixture returns `CARD_BRAVO` and `CARD_CHARLIE` exactly once
each, still returns `PARA_ALPHA`/`ITEM_DELTA`/table cells exactly once each, and does not
emit the `aside` paragraph when `<main>` is selected. `quotes.toscrape.com` returns all 10
quotes (assert the 10 upstream sentences appear once each). The four control pages
(Hacker News, `lobste.rs`, Wikipedia HTTP, GitHub repo) keep their current content within
±5% characters.

Tests: extend `tests/web_fetch_integration.rs` with a committed HTML fixture (no network)
and an exact-count assertion per marker; add a proptest that no text node is emitted twice
for a randomly nested `div`/`span` tree.

### WP1 — Selector roots of any element type produce blocks (P1, S)

When an include selector matches an inline or container element, emit its text (inline
root) or walk its descendants (container root). When `matched_roots > 0` but no block is
produced, emit a typed warning naming the selector count; never return an empty payload
that reads as success.

Acceptance: fixture `.card`, `.card-title`, and `li` each return their text; `lobste.rs`
`.u-url` returns 25 titles; `p` behaviour is unchanged; `selector_no_blocks` appears only
when a match yields no text (e.g. an empty element).

Tests: extend `tests/web_fetch_integration.rs` for the fixture selectors and a no-text
match; extend the browser selector test in `tests/mcp_tools.rs` to assert the rendered-DOM
selector returns the 10 quote strings.

### WP2 — Report disproportionate extraction (P1, S)

Add one typed warning, `extraction_sparse`, when rendered text is a small fraction of the
document's text (bounded estimate over the same DOM, no second parse) or when a selector
matched and produced nothing. Include `text_chars`, `dom_text_chars`, and a ratio in the
warning payload, plus one recommended action. Choose the threshold from the control pages,
not from the failing page, and record it in the plan closure.

`render: "auto"` needs its own signal, because a JS-only page is text-sparse before and
after rendering and the ratio cannot catch it: when the resolved policy is `auto`, the
HTTP text is below the floor, and the DOM carries script markers, emit `render_suspected`
naming `render: "browser"` as the retry. Do not escalate silently — that would change
cost without the caller asking.

Acceptance: the fixture and `quotes.toscrape.com` emit `extraction_sparse`; Hacker News,
`lobste.rs`, Wikipedia HTTP, GitHub repo, and `example.com` do not; `quotes.toscrape.com/js/`
under `auto` emits `render_suspected` while the static page does not; each warning payload
is under 300 chars.

Tests: fixture-based positive and negative cases; assert the warning never fires on the
existing HTML corpus in `web_fetch_integration`.

### WP3 — Browser status parity with HTTP (P2, S)

Classify the captured final status before extraction: non-2xx becomes the same failed-item
shape the HTTP transport produces (`upstream_failed: http_status <code>`), with the error
body optionally retained only under diagnostic detail. Do not report `fetched: true` and
`ok: true` for an error document.

Acceptance: `no-such-page` under `render: "browser"` is `ok: false` with the status and
matches the HTTP-transport error text; 200 pages are unchanged; a 404 with
`continue_on_error: false` fails the call as it does today.

Tests: extend `tests/mcp_tools.rs` browser cases with a 404 fixture and assert parity
between `http_only` and `browser` classifications.

### WP4 — Representation label for non-HTML payloads (P3, S)

Set `representation` for browser (`dom` or `html`, one value, documented), PDF
(`pdf_text`), and container documents (`docx`/`xlsx`/`csv`), or state in the output schema
that the field is HTML-only. Today the field is `"html"` for browser and `null` for PDF and
documents, so an agent cannot tell what it received.

Acceptance: one documented value per content class; `outputSchema` agrees.

## Verification

```bash
make check
cargo test --locked --features mock --test web_fetch_integration
cargo test --locked --all-features --test mcp_tools
```

Plus a manual limited live re-trial (not CI) on the five pages named above, recording
extracted chars, marker counts, and whether `extraction_sparse` fired.

## Stop conditions

- Any change to tool inventory, aliases, accepted arguments, provider ids, default
  provider set, or trust framing: stop and split into a separate registered milestone.
- WP0 requiring a new dependency (for example a readability crate): stop; the existing
  `scraper` tree is sufficient for the block walk.
- WP2 threshold needing a page-specific exception list: stop and reconsider the signal
  instead of special-casing domains.
- WP3 changing the HTTP transport's own status rule: out of scope.

## Closure evidence required

Per-WP acceptance numbers from the fixture suite and the five-page live record; the
chosen `extraction_sparse` threshold with the control-page ratios that justify it; the
new warning code documented in `docs/tool-matrix.md` and `architecture/engines.md`
§ Scraper Fidelity; `CHANGELOG.md` entry; `docs/test-inventory.md` and
`architecture/testing.md` updated if a suite is added; routes in `plans/registry.md` and
the roadmap status updated in the same closure commit.

## Appendix — related findings routed elsewhere

1. **Payload duplication and brief projection** (one item emitted as up to 4.1× its
   readable text; `brief` dropping items): M054 WP0/WP1.
2. **Browser log volume and process lifecycle**: M054 WP7 and its appendix item 3. This
   trial added a per-render WARN, "Browser was not closed manually, it will be killed
   automatically in the background", and confirmed no new orphans after a clean shutdown.
3. **Wrong repair hint for persistent browser profiles**: the runtime says
   `[fetch.browser].persistent_profiles_enabled = true`, but the key is
   `[fetch.browser.persistent_profiles].enabled`; the flat key is silently ignored, so the
   instructed fix does nothing. Routed to M054 WP4 with the docs correction in
   `architecture/commands.md:275,328`.
4. **Capability-honest schema and rebuild hints**: M054 WP3/WP4.
