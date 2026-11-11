# M003 — Markdown-First Fetch Negotiation

Status: closing
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M003
Primary class: capability
Baseline: `cd81f4d5e966364918fe2be25485b8ebbf424e71`
References: `architecture/fetch.md` § 2–5; `src/fetch/client.rs`; `src/fetch/cache.rs`; `src/fetch/detect.rs`; `src/fetch/render/markdown_source.rs`; `src/core/fetch.rs`.
Dependencies: none hard. M002 is independent (different cache layer).

## Objective

Request and prefer Markdown representations when an origin supports content negotiation, reducing bytes and tokens per fetched page while preserving extraction fidelity, link extraction, caching, and safety bounds. Default-on, because the happy path should be the efficient path.

Cloudflare's Markdown for Agents (live February 2026) converts HTML to Markdown at the edge for `Accept: text/markdown`, and reports up to roughly 80% token reduction; a February 2026 survey found most agents do not send the header. Eggsearch currently always requests HTML and parses it.

## Non-goals

- No `.md` path rewriting in the default path. Cloudflare added it for agents that omit the header; a second request per fetch is not the happy path. A bounded docs-host fallback may be a later milestone.
- No negotiation for PDFs, JSON/XML APIs, code-host raw URLs, or `extract_mode = "metadata_only"` — page metadata lives in the HTML head and would be lost.
- No browser-transport negotiation change.
- No new tool argument and no per-call opt-out in this milestone.

## Invariants that cannot regress

- SSRF validation, manual redirect handling, per-hop address pinning, origin permits, and byte caps are unchanged; the extra request count is bounded to two per fetch.
- Cache isolation and freshness rules are unchanged. A markdown variant must never be served for an HTML-only key or vice versa.
- Sanitization tiers, trust framing, and truncation flags apply exactly as today.
- `include_links` still returns links; `metadata_only` still returns title and description.
- Responses remain `external_untrusted` with the same warning set.

## Expected production changes

1. **Accept policy.** A single constant policy string (`text/markdown;q=1.0, text/html;q=0.9, */*;q=0.1`) applied in `FetchClient` when the target is HTML-ish and `extract_mode` is not `MetadataOnly`. Never applied to PDF, JSON, XML, or code-host raw targets.
2. **Cache key dimension.** Add `accept_policy` (a short versioned token such as `md1`) to `RawCacheKey`, and accept `Vary: Accept` in `should_cache_response` because the value is fixed server policy rather than caller input. Bump the derived `renderer_version` so HTML-derived entries cannot alias a markdown body.
3. **Representation metadata.** Record `representation: markdown | html` on the fetch response and derived entry; surface `x-markdown-tokens` and `x-original-tokens` as optional `FetchRenderMetadata` fields when present.
4. **Bounded fallback.** If a markdown-accepting request yields a non-2xx status or an empty extracted body, retry the same URL once with the HTML policy on the same origin permit. Two requests maximum per fetch, recorded in telemetry.
5. **Markdown link extraction.** Extract links from the markdown AST (`pulldown-cmark` link events) so `include_links` is not silently empty on negotiated responses.
6. **Evidence gate.** Default-on requires a recorded A/B over the existing extraction-matrix pairs comparing characters, tokens, heading/table/code-block fidelity, and focus-chunk hit rate. Table fidelity is the expected risk.

## Storage and compatibility effects

- Raw cache entries gain a policy dimension; existing entries are simply not reused, which is safe.
- Additive response fields only. No schema removal, no tool argument change.
- Origins that ignore the header are unaffected; origins that 406 are handled by the fallback.

## Ordered work packages

1. Implement the Accept policy, target classification, and request wiring with httpmock assertions on the sent header.
2. Implement the cache dimension, `Vary: Accept` acceptance, and renderer-version bump; prove isolation in both directions.
3. Implement fallback on non-2xx and empty-body outcomes with a request-count assertion.
4. Implement markdown link extraction and `metadata_only` exclusion.
5. Parse token headers into render metadata.
6. Run the extraction-matrix A/B and record results in `docs/retrieval-quality-extraction-matrix.md`.
7. Update `architecture/fetch.md` (negotiation, cache dimension), `docs/features.md`, and `docs/test-inventory.md`.

## Verification

- `cargo test --locked --features mock --test web_fetch_integration --test extract_fetch_contract --test fetch_safety`
- `cargo test --locked --all-features --test property_fetch_limits --test property_fetch_response --test property_fetch_url_edge`
- `cargo test --locked --features mock --test retrieval_quality`
- `make check`; `make docs-check`

## Acceptance and stop conditions

- A negotiating origin returns markdown, is cached under the new key, and produces fewer bytes than the HTML path on the same URL.
- `include_links`, `metadata_only`, PDF, and code-host raw fetches are demonstrably unaffected.
- A 406 or empty markdown response falls back once and still returns usable content.
- Extraction-matrix evidence shows no material fidelity loss; if tables or code blocks regress materially, stop and reopen the default decision rather than shipping the regression.
- Stop if the change requires a new dependency, weakens SSRF, or needs a per-call schema field.

## Closure evidence

`plans/closure/agentic-retrieval-integrity/003-status.md` records the candidate SHA, the A/B table (bytes, tokens, fidelity, focus hit rate) per pair, cache-isolation tests, fallback request counts, and the explicit default-on decision.
