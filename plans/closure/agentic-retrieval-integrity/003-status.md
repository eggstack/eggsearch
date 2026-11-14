# M003 Markdown-First Fetch Negotiation — Closure Status

Status: closed (default-on)

Source plan: `plans/archive/implementation/agentic-retrieval-integrity/003-markdown-first-fetch-negotiation.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Planning baseline: `cd81f4d5e966364918fe2be25485b8ebbf424e71` (origin/main, 2026-11-10 UTC)

Implementation candidate: `c9912fc0e4824c4ec6b1231be2057fb4cd0ba8ab`, the implementation commit immediately preceding this
closure commit. The closure record, roadmap status, and registry entry moved in the same closure
commit.

## Result

HTML-ish fetches now send `Accept: text/markdown;q=1.0, text/html;q=0.9, */*;q=0.1` by default.
`src/fetch/accept.rs` classifies the target before the request; `src/fetch/negotiate.rs` owns the
bounded fallback.

| Target | Policy |
|---|---|
| Extension-less, unknown, `.html`/`.php`-style pages | `md1` (markdown) |
| `metadata_only` | `html1` (page metadata lives in the HTML head) |
| PDF and binary/media/archive/font/office targets | `html1` |
| JSON/JSONL/XML/RSS/Atom/CSV/TSV/YAML/TOML/notebook/diff/patch and text/source/config extensions | `html1` |
| Code-host URL rewritten to raw content | `html1` |
| Browser transport | unchanged, `representation: html` |

Cache: `RawCacheKey` gained `accept_policy` (`md1`/`html1`), `renderer_version` moved 1 → 2, and
`should_cache_response` now accepts `Vary: Accept` while still rejecting `Authorization`, `Cookie`,
`User-Agent`, and mixed combinations. Tests prove isolation in both directions and across a 304
revalidation, including a mock that only answers when both the markdown `Accept` and `If-None-Match`
match (a wrong-policy mock stays at zero hits).

Metadata: responses carry `representation: markdown | html`; `FetchRenderMetadata` exposes
`x-markdown-tokens` / `x-original-tokens` when the origin sends them. `include_links` works on
markdown bodies through a `pulldown-cmark` link-event extractor with the same resolution,
classification, and 100-link cap as the HTML path. `render_markdown_source` previously dropped
tables and list items entirely; both now render.

## Fallback

Triggers: HTTP 406/415, an unsupported content type, or a `text/markdown` response whose extracted
body is empty. Never for `metadata_only`, PDF, JSON/XML, code-host raw targets, other non-2xx
statuses, network errors, content-too-large, or an origin that ignores the header and returns HTML
or plain text (already usable).

Two requests maximum per fetch, on the existing origin-permit and redirect-safety path. Tests assert
`rejecting.hits() == 1 && fallback.hits() == 1` with `attempt_count == 2`, the same for an empty
markdown body, and `html_policy.hits() == 0` with `attempt_count == 1` for an ignored header. The
discarded markdown attempt is never stored, so a 406 cannot poison the `md1` key.

## Controlled A/B

One logical document served in both representations through `run_web_fetch`:

| Representation | Body chars | Extracted block chars | Approx tokens (chars/4) | Headings | Table rows | Code lines | Links | Focus chunk |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| markdown | 356 | 251 | 63 | 2 | 3 | 4 | 2 | hit |
| html | 560 | 251 | 63 | 2 | 3 | 4 | 2 | hit |

Markdown body 36.4% smaller; extracted text, headings, table cells, code lines, link URLs, and the
focus chunk identical. Assertions fail if `markdown_bytes >= html_bytes`, if block chars exceed the
HTML path, or if any table row, code line, outline entry, or link URL changes.

Live curl spot check (not an end-to-end Eggsearch run): two Cloudflare-fronted origins returned
`text/markdown` (7,031 and 18,334 bytes) for the markdown policy versus `text/html` (213,421 and
317,846 bytes), with `vary: accept-encoding, accept`; neither sent `x-markdown-tokens`. The HTML
bodies include scripts, styles, and navigation, so that ratio overstates savings. Table and
code-block fidelity is evidenced by the controlled fixture, not the spot check.

## Verification (combined candidate)

- `cargo test --locked --features mock --test web_fetch_integration --test extract_fetch_contract
  --test fetch_safety`: 70 + 24 + 135 passed (`web_fetch_integration` is 79 under `--all-features`, where the PDF and browser arms add tests).
- `cargo test --locked --all-features --test property_fetch_limits --test property_fetch_response
  --test property_fetch_url_edge`: 11 + 18 + 20 passed.
- `cargo test --locked --features mock --test retrieval_quality`: 13 passed, 2 ignored.
- `cargo test --locked --all-features --test mcp_tools --test static_guards`: 5 + 60 passed.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: clean.
- `make check`: passed through the full `--all-features` suite (5,848 passed, 26 ignored, 0 failed)
  and hygiene; stops at the pre-existing `dependency-policy` advisories. `Cargo.lock` unchanged.

## Safety evidence

SSRF validation, manual redirects, per-hop address pinning, origin permits, and byte caps are
unchanged. No new dependency. No new tool argument and no per-call opt-out. Sanitization tiers,
trust framing, and truncation flags apply exactly as before. `metadata_only`, PDF, and code-host raw
fetches never negotiate.

## Deviations from the plan

- The fallback is narrower than "a non-2xx status". It fires on 406/415, unsupported content type,
  and empty markdown bodies only. Retrying every 404/5xx under a second policy would double traffic
  on failures that a different representation cannot fix, and the plan itself requires ignored
  headers to stay unaffected.
- `src/fetch/cache.rs` hit its size ratchet; its test module moved verbatim to
  `src/fetch/cache/tests.rs` and the ratchet entry was removed (757 lines).
- `src/fetch/negotiate.rs` was split out of `src/fetch/client.rs` for the same reason.

## Residual risk

- An origin that always answers 406/415 pays two requests per uncached fetch and never earns an
  `md1` cache hit. A negotiation-failed marker in the raw cache would fix that; out of scope here.
- `text` for a markdown representation is the raw markdown body, so inline syntax is present. The
  extracted block text is the comparable fidelity surface.
- Live evidence is curl-observed, not end-to-end. No live `x-markdown-tokens` header was observed,
  so that path is covered only by unit tests.
