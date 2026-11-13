# Fetch Deep Dive

**Location:** `src/fetch/` (11 top-level files + `browser/` + `render/`, 8 files each)
**Purpose:** Bounded single-URL HTTP(S) fetch with SSRF enforcement, two-tier
cache, origin control, content detection, structural rendering, and sanitized
output. Independent of `meta`; consumed by `web_fetch`, `batch_fetch`,
`repo_fetch` follow-ups, and CLI `fetch`. Entry: `src/fetch/mod.rs`.

---

## 1. Bounded Pipeline

One URL flows through seven stages. Every stage is bounded; truncation is
recorded, never silent.

```
validate_fetch_target (SSRF)
  -> cache lookup (raw tier -> derived tier)
  -> FetchClient bounded HTTP (manual redirects + address pinning)
  -> content detection (classify_content_type + detect::classify)
  -> extraction / rendering (render/*, extract, pdf)
  -> span / focus projection (span.rs, core/focus.rs via core/fetch_policy.rs)
  -> sanitize (core/sanitize.rs, three tiers)
```

1. **Validate.** `limits::validate_url` (sync shape check) runs first;
   `limits::validate_fetch_target` / `validate_fetch_target_with_resolved_addrs`
   (async full check + DNS) runs per attempt and per redirect hop.
2. **Cache.** Raw hit with fresh validators short-circuits the network; a fresh
   raw body under changed extraction params re-derives locally via
   `FetchClient::derive_from_raw` without refetching (§3).
3. **Transport.** `FetchClient::fetch` (or `fetch_conditional` for revalidation)
   performs the bounded HTTP exchange (§2).
4. **Detect.** `classify_content_type` (HTML / PDF / text gate) decides whether
   the body is even readable; `detect::classify` picks the document kind,
   language, and line-preserving flag for the renderer (§5).
5. **Render.** HTML goes through `render/blocks.rs`; code, CSV, notebooks, and
   Markdown sources go through their dedicated renderers; PDFs take the
   feature-gated `pdf.rs` path (§6, §7).
6. **Project.** `repo_fetch` line/symbol selection uses `span::select_span`;
   `web_fetch` / `batch_fetch` focus queries project over the extracted
   `FetchDocument` via `core/fetch_policy.rs`. Both are post-extraction
   projections: they never change what is cached.
7. **Sanitize.** `client::sanitize_field` applies Tier 1 (control-char strip +
   length bound, always on) plus Tier 2 framing and Tier 3 injection-marker
   scan when `sanitize_output` is true. Every field records `TrustMarkers`;
   every response carries the `external_untrusted` warning.

Code-host source URLs are rewritten to raw content URLs before fetching; the
rewritten URL passes through the same validation pipeline, and the rewrite is
reported via `FetchTransform`.

---

## 2. FetchClient (`client.rs`)

`FetchClient { client, limits, user_agent, sanitize_output }` owns one shared
`eggfetch_core::Client` built by `build_transport_client`:

- `follow_redirects(false)` — redirects are **disabled** in transport and
  followed **manually**, one hop at a time.
- `client_timeout(timeout_ms)` sets pool / connect / write / read / total to the
  same budget, and each request builder re-applies it from the effective limits.
- Every hop resolves via `validate_fetch_target_with_resolved_addrs` and pins
  the request to that validated address snapshot, so the connect path cannot
  drift to a different DNS answer mid-attempt. DNS validation and SSRF
  authorization stay in `limits.rs`; eggfetch's resolved-route cache is
  transport reuse only.

### Per-fetch loop (`fetch`)

1. `validate_url` on the initial URL, then optional code-host rewrite.
2. Resolve two budgets: extraction runs at the document budget
   `document_chars = max_chars_cap`, while the returned `text` is projected at
   `window_chars = caller max_chars` (or `max_chars_default`), clamped to
   `max_chars_cap`. `raw_text` stays at `max_chars_cap` so internal consumers
   (`repo_fetch` line/span selection) see full source text.
   `document.document_truncated` reports extraction stopping at the document
   budget; `document.text_truncated` reports window clipping. `document.blocks`
   / `document.chunks` can therefore cover a document larger than one `text`
   window.
3. Loop: validate current URL with resolved addresses → send (with the selected `Accept` policy on
   every hop; conditional `ETag`/`Last-Modified` stay first-hop-only) → on 3xx require a non-empty
   `Location`, resolve relative targets, reject past `redirect_limit`, re-validate the target
   (failures map to `RedirectTargetBlocked`), continue.
4. On the final response, harvest `etag` / `last-modified` / `cache-control` /
   `expires` / `vary` for the cache layer.
5. `Content-Length` pre-check: a declared length above the effective body cap
   fails fast with `ContentTooLarge`. Unparseable lengths warn and fall through
   to the authoritative streaming cap.
6. Non-2xx becomes `HttpStatus`. Content-type gating runs twice from one source
   of truth: pre-body on headers + URL extension, then post-body with `%PDF-`
   magic sniff for mis-served PDFs.
7. Body streams through `append_bounded` (cap `FetchLimits::body_cap`, UTF-8-safe,
   sets `truncated`); `derive_from_raw` then builds the response, so cache raw
   hits reuse the exact rendering path as network fetches. A PDF body that was
   truncated at the cap is refused there as `ContentTooLarge` with the
   configured cap, never handed to the parser.

`fetch_conditional` repeats the loop with caller conditional headers for stale
revalidation; a 304 returns empty-body status so the cache layer can refresh
validators in place.

### Representation negotiation (`accept.rs`, `negotiate.rs`)

Every HTML-ish fetch asks for Markdown first, because the efficient representation is the happy
path:

`Accept: text/markdown;q=1.0, text/html;q=0.9, */*;q=0.1`

`accept.rs` classifies the target before the request: extension-less and unknown targets, plus
ordinary `.html`/`.php`-style pages, use the Markdown policy; PDF and binary/media/archive targets,
JSON/XML/CSV/YAML/TOML and source/config extensions, code-host URLs rewritten to raw content, and
`metadata_only` fetches use the plain HTML policy. `metadata_only` is excluded because page metadata
lives in the HTML head. Browser transport is unchanged.

`AcceptPolicy` (`md1` / `html1`) is the short policy token carried into the raw cache key.
`negotiate.rs` owns the bounded fallback: a Markdown attempt that returns 406/415, an unsupported
content type, or an empty extracted body is retried **once** with the HTML policy on the same
origin-permit and redirect-safety path. Two requests maximum per fetch; `attempt_count` records the
real count. An origin that ignores the header and answers HTML or plain text is already usable and
is not retried — retrying 404/5xx under a second policy would only double traffic. The discarded
Markdown attempt is never stored.

Responses carry `representation: markdown | html`, and `FetchRenderMetadata` surfaces
`x-markdown-tokens` / `x-original-tokens` when the origin sends them.

### Timeout / byte / redirect / content-type policy

| Policy | Rule |
|--------|------|
| Timeout | `FetchLimits::timeout_ms` (default 8000). DNS phase consumes at most half (1500 ms floor); the remainder covers the HTTP round-trip plus body streaming. Send errors mapping to timeout become `FetchError::Timeout(timeout_ms)`. |
| Bytes | `FetchLimits::body_cap` caps the streamed body: `pdf_max_bytes` (default 16,777,216) when the response is PDF-identified by Content-Type or a `.pdf` path, `max_bytes` (default 2,000,000) otherwise. `max_chars_default` (12,000) is the returned-text window when a caller omits `max_chars`, and `max_chars_cap` (50,000) is both the document-extraction budget and the `raw_text` bound. `max_url_len` is 8192 bytes. |
| Redirects | `redirect_limit` (default 5) counts followed hops; exceeding it returns `RedirectLimitExceeded`. Missing/empty `Location` is `InvalidRedirectLocation`. |
| Content-type | HTML (`text/html`, `application/xhtml+xml`), text (`text/*`, JSON/TOML/YAML/JS/XML families), and PDF (by type, `.pdf` extension, or magic) are accepted; anything else is `UnsupportedContentType`. PDF additionally requires the compile feature **and** `pdf_enabled` (`PdfNotCompiledIn` / `PdfDisabled`). |

### Widened-client reuse rule (`with_timeout_ms`)

`TimeoutOverrideMode::Shared` vs `Widened` compares requested against base
`timeout_ms`:

- Equal or shorter: clone the shared eggfetch client and tighten only the
  logical `FetchLimits.timeout_ms`; the request-level `client_timeout()` enforces
  the stricter deadline without rebuilding transport.
- Longer: build one widened client through the same `build_transport_client`
  helper, because the wider client-scoped connect timeout is required by
  eggfetch's resolved-route transport.

Batch setup resolves this adjusted client once per batch before spawning item
futures — one adjustment per call, not per item.

## 3. Two-Tier Cache (`cache.rs`)

`FetchCache` holds two independent LRU tiers behind an `operation_gate` plus
per-tier byte budgets with exact accounting (drift self-heals in `stats()`).

### Raw tier: transport bytes

- `RawCacheKey { url, scope, accept_policy }` where `url` is the canonicalized form from
  `core::identity::canonicalize_url` (`build_raw_cache_key`) and `accept_policy` is `md1` or
  `html1`. The policy is a cache dimension, so a Markdown body can never be served for an HTML key
  or vice versa, including after a 304 revalidation.
- `RawFetchCacheEntry` stores `final_url`, `status`, filtered headers, `body:
  Arc<[u8]>`, `fetched_at`, `freshness`, `validators`, `scope`, content-type,
  content-length, redirect count, `representation` (`Http` vs `BrowserDom`),
  `truncated`, and `browser_escalated`.
- Either bounded HTTP bytes or the bounded rendered browser DOM can populate
  the tier; both re-derive through the same rendering path.
- Over-size bodies are refused, not partially stored.

### Derived tier: extracted documents

- `DerivedCacheKey { scope, raw_content_hash, extraction_key }` where the hash
  is `xxh3_64(body)` (`build_raw_response_hash`) and `ExtractionCacheKey`
  covers `extract_mode`, exact `max_chars`, `include_links`, `pdf_pages`,
  `pdf_ocr`, `include_media`, `renderer_version: 4`, and `sanitize_output`.
  Version 3 isolates entries derived before PDF page classification stopped
  depending on `include_media`; version 2 isolated entries written before
  representation negotiation existed.
- `DerivedDocumentCacheEntry` stores the extracted title/description/text,
  `raw_text`, links, truncation flags, the structured `FetchDocument` (blocks,
  outline, chunks), trust markers, and transport tag.
- Internal hot paths share entries as `Arc` (`get_derived_shared`); the owned
  `get_derived` is a compatibility wrapper.
- A fresh raw hit under changed extraction params re-derives locally: same
  bytes, new derived key, no network I/O.

### Scopes and freshness

- `CacheScope::Anonymous` is the shared default; `CacheScope::Profile(ProfileId)`
  partitions browser-profile fetches by **opaque** profile ID (`prof_<fnv>`),
  never the display name. `invalidate_scope` scans both tiers under a write gate
  so profile deletion cannot leave a half-invalidated view.
- `CacheFreshness::from_headers` parses `cache-control` (`max-age`, `s-maxage`
  preferred per RFC 7234, `no-store`, `no-cache`, `private`), `expires`
  (RFC 2822 / 3339 / legacy-zone HTTP dates), `etag`, `last-modified`, and
  `vary`. `is_fresh` requires an explicit origin lifetime — entries without
  freshness headers are stale (no default TTL).
- `should_cache_response` refuses `no-store`, non-2xx, `private` in anonymous
  scope, any `Vary` token besides `Accept-Encoding` and `Accept`, and image/audio/video plus
  `application/octet-stream` bodies. `Accept` is supported because the request value is fixed
  server policy carried in the cache key, not caller input.
- Stale entries with validators revalidate with `If-None-Match` (preferred) or
  `If-Modified-Since` (`build_request_conditional_headers`); 304 merges only
  supplied fields per RFC 9111 §4.3.4 (`apply_304_headers`). `CacheStatus`
  reports `Hit`, `Revalidated`, `Miss`, `Bypassed`, or `NotCacheable`.

### Cache policy: reuse tightens only

Agent-visible `FetchCachePolicy` (`core/fetch.rs`, validated by
`core/fetch_policy.rs::validate_cache_age`) plus `max_cache_age_seconds`
(0–2,592,000) ride on `web_fetch` and per `batch_fetch` web item:

| Policy | Behavior |
|--------|----------|
| `default` | Serve a fresh eligible entry; else revalidate with validators when available, else fetch. |
| `bypass` | Skip cache reads; the network fetch still populates the cache unless the origin forbids storage. |
| `refresh` | Never serve on local freshness alone; revalidate with `ETag` / `Last-Modified` when available, else fetch. |

`max_cache_age_seconds` is an upper bound on acceptable entry age (`0` forces
revalidation without disabling storage). Neither control bypasses SSRF,
redirect, origin-concurrency, profile-isolation, content, or sanitization
policy. Conditional revalidation treats HTTP 304 as a revalidation signal, not
a redirect.

---

## 4. OriginController (`origin.rs`)

Per-origin (`OriginKey { scheme, host, port }`) concurrency, retry, and circuit
breaking shared by the fetch path:

- `OriginPolicy` defaults: `http_concurrency` 2, `browser_concurrency` 1,
  `retry_max_attempts` 2, `retry_base_delay_ms` 250, `retry_max_delay_ms` 4000,
  `circuit_failure_threshold` 3, `circuit_duration_ms` 60,000.
- `acquire` takes an owned semaphore permit after rejecting open circuits
  (`CircuitOpen { remaining_ms }`); `record_success` clears consecutive failures
  and any open circuit; `record_failure` applies jittered exponential backoff
  (`base * 2^count`, capped) and opens the circuit at the threshold (capped at
  120 s). `NonRetryable` resets the counter and returns `NoBackoff`.
- `acquire_bounded(key, wait)` wraps `acquire` in a deadline and returns
  `AcquireTimedOut { wait_ms }` instead of waiting forever, so a limiter can
  never outlive the request budget.
- The redirect loop holds **at most one** permit, for the origin currently being
  contacted: it is released and re-acquired only when the next hop changes
  origin. Holding one permit per hop would let a chain that revisits one
  cross-origin host consume the whole per-origin semaphore (`http_concurrency`
  2 by default, `redirect_limit` 5) and block permanently on the next acquire.
- Classification: `classify_http_status` (429 → `RateLimited`, 502–504 →
  `Retryable`), `classify_network_error` (reset/refused/DNS/broken-pipe/EOF →
  `Retryable`), plus typed eggfetch classifiers so timeouts and refused/DNS
  failures drive retryable backoff.
- `parse_retry_after` accepts delta-seconds or HTTP dates clamped to 300 s;
  `should_retry` enforces the attempt cap for `Retryable` / `RateLimited` only.
  State is LRU-capped by `max_entries` (oldest last-access evicted).

---

## 5. Content Detection and Link Extraction

### `detect.rs`

`classify(content_type, url, body) -> DetectedContent { kind, language,
line_preserving }` with strict priority: Content-Type header → URL extension →
byte heuristics. Header and extension tables cover Markdown, JSON (`+json`
suffix included), TOML, YAML, diff/patch, CSV/TSV, XML/RSS/Atom/SOAP, notebook
(`.ipynb`), AsciiDoc, RST, and per-language code types with language hints.
Unknown or `text/plain` bodies fall to an 8 KB / 200-line heuristic (shebang,
imports, `fn`/`def`/`func`/`struct`/`class` signals, brace depth) returning
`Code` or `PlainText`. The `line_preserving` flag selects the line-preserving
renderer in `FetchClient`.

### `extract.rs`

Legacy HTML helpers kept alongside the block pipeline: `HtmlExtractor` /
`extract_content` (title, meta description, body text with chrome-element
stripping, depth-capped recursion, non-UTF-8 lossy fallback with
`NON_UTF8_WARNING`) and `extract_links_from_html`. Link collection caps at
`MAX_LINKS = 100` and reports `(links, total_seen, truncated)`;
`classify_link` assigns `SamePageAnchor`, `Pdf`, `Image`, `SourceCode`,
`Download`, `Feed`, GitHub/GitLab `Issue` / `PullRequest` / `Release`,
`SecurityAdvisory`, `Documentation` / `ApiReference`, `SameDomain`, or
`External`, plus `same_domain` and `rel` fields.

`extract_links_from_markdown` mirrors that contract for a negotiated Markdown
representation: `pulldown-cmark` link events, the same resolution and
classification rules, the same `MAX_LINKS` cap, and the same
`(links, total_seen, truncated)` triple. Without it `include_links` would
silently return an empty list on every origin that honours `Accept:
text/markdown`.

---

## 6. Render Subsystem (`render/`, 8 files)

| File | Responsibility |
|------|---------------|
| `mod.rs` | Re-exports; `render_blocks` / `RenderedBlocks` entry |
| `blocks.rs` | HTML → `Vec<RenderedBlock>` + outline (headings, paragraphs, lists, code, tables, quotes, definitions) |
| `text.rs` | `render_blocks_text` — blocks → plain text |
| `markdown.rs` | `render_blocks_markdown` — blocks → Markdown (`#` headings, fenced code, `[text](url)` links) |
| `code.rs` | `render_code` / `render_diff` / `render_plaintext` — line-preserving renderers with 1-based line ranges |
| `csv.rs` | `render_csv` — bounded table preview (100-row cap, column-count header) |
| `notebook.rs` | `render_notebook` — Jupyter cells to `[cell N (type)]` blocks; never executes code, outputs skipped |
| `markdown_source.rs` | `render_markdown_source` — pulldown-cmark parse of `.md` sources (tables, tight/loose list items, strikethrough); previously dropped table and list-item content entirely |

`blocks::render_blocks(html, base_url, max_chars, markdown)` returns
`(title, description, RenderedBlocks, warnings, non_utf8)`:

- Content-root selection probes `main`, `article`, `[role=main]`, `body` in
  order and takes the first root yielding ≥1 block and ≥50 chars; sparse roots
  fall back to `body`.
- Skips chrome subtrees (`script`/`style`/`nav`/`header`/`footer`/`aside`,
  `hidden` / `aria-hidden`); headings build blocks plus outline entries with
  slug anchors; `<pre>` preserves whitespace with language detection; tables
  render as pipe Markdown with an irregular-row warning.
- Truncation is block-boundary-aware: whole blocks are kept while the character
  budget lasts, code blocks snap to a newline when past the halfway point, and
  stale outline entries are pruned. `text` vs `markdown` modes differ only in
  the final projection plus inline `` `code` `` / `[text](url)` collection.

Non-HTML bodies dispatch on `detect.rs`: notebooks, CSV, XML/RST/AsciiDoc,
Markdown sources, diffs, line-preserving code, or plain paragraphs. Code/diff
renderers split at 200 lines per block with per-block line ranges; CSV caps at
100 rows with a column/row header; notebooks cap at 200 cells and degrade to a
single `RawText` block for non-notebook JSON.

---

## 7. PDF Gate (`pdf.rs` + `pdf_quality.rs`, feature `pdf`)

Double-gated: the `pdf` Cargo feature must be compiled in **and**
`FetchLimits::pdf_enabled` must be true, else `PdfNotCompiledIn` / `PdfDisabled`
before any body bytes are retained. `PdfNotCompiledIn` names the feature, the
`docs/features.md#pdf-extraction` anchor, and `cargo build --features pdf`.

Detection combines Content-Type, `.pdf` URL extension, and `%PDF-` magic. The
first two also pick the body cap (`FetchLimits::body_cap` in `limits.rs`): such a
response is read under `pdf_max_bytes` (default 16 MiB, validated to
`1..=268_435_456`), everything else under `max_bytes`. A PDF recognised only by
magic reaches the body read without either signal and therefore stays under
`max_bytes`; the cap cannot be widened after bytes have been consumed. That
asymmetry is deliberate and documented, not widened.

Size is reported as size: a PDF refused by the `Content-Length` pre-check, or
truncated at the effective cap, returns `FetchError::ContentTooLarge` naming the
cap. A truncated body never reaches the parser, so a budget problem cannot be
reported as `PdfParseError`.

`detect.rs::classify` gained the `application/pdf` arm in the same change, so the
document classifier and the fetch gate cannot disagree about a PDF.

`extract_pdf_text(bytes, max_chars, PdfLimits, PdfExtractOptions)` via `lopdf`:

- `PdfLimits { max_pages` (25), `max_chars_per_page` (12,000),
  `max_total_chars` (50,000), `max_decompressed_bytes` (67,108,864) `}` bound
  pages, per-page text, the running total (exceeding the total stops after the
  current page with a warning), and the document's total stream expansion.
- Page selection parses `1-3,5` specs (`parse_pdf_page_spec`:
  1-indexed, sorted, deduplicated, range and `max_pages` caps) and validates
  against the real page count (`PdfPageOutOfRange`, `PdfPageCapExceeded`,
  `PdfPageSpecInvalid`).
- Encrypted documents fail with `PdfEncrypted` before any page access,
  whether or not `password` (`RedactedString`) is supplied; unparseable bytes
  fail with `PdfParseError`.
- `pdf_quality.rs` owns `classify_page_quality`, `count_cid_tokens`, and
  `count_page_images`. Image presence is computed for every extracted page
  regardless of `include_media`, so an image-only page classifies as
  `ScannedOrImageOnly` (0.10) at default settings; `include_media` only decides
  whether `image_count` is populated in page metadata.
- OCR policy: `Never` and `Auto` both extract, and `Auto` adds a per-page
  `PdfOcrUnavailable` structured warning for scanned pages. `Always` fails
  closed with `PdfOcrUnavailable` because it demands a capability the build does
  not have. No build performs OCR.
- Output is page-indexed `Paragraph` blocks plus a `Page N` outline, legacy
  `--- Page N ---` text, Info-dictionary metadata, outline extraction, and
  per-page quality (`CleanText` 1.0 down to `Blank` 0.0) rolled into
  `quality_score` and `content_ok`. All-blank documents return
  `PdfNoExtractableText`.
- `MetadataOnly` extract mode skips text extraction and returns fetch metadata
  with an empty PDF document shell.

`provider_status.pdf_capabilities` keeps its `{ compiled, configured, usable,
layout, ocr }` shape: `usable` is `compiled && configured`. The budget is a
limit, not a usability term, so it does not appear there.

### Bounded stream scan (`pdf_stream.rs`)

`scan_stream_budget(bytes, budget)` runs as the first statement of
`extract_pdf_text`, before `Document::load_mem`, and refuses the document at the
first breach. It is one forward pass over the raw buffer: no hash iteration, no
second parser, and identical bytes with identical configuration produce the
identical outcome.

- It finds each `stream` keyword (excluding `endstream`), takes the dictionary
  immediately before it, and reads `/Length`, `/Filter`, `/DecodeParms`,
  `/Type`, and `/W` from that dictionary only.
- The content region uses the declared `/Length` when it is a direct integer
  and `endstream` follows; otherwise it falls back to the next `endstream`. An
  indirect `/Length` on a filtered stream is refused instead of falling back:
  `lopdf` resolves the reference and inflates the full declared extent, which
  the `endstream` fallback can under-measure when the compressed bytes contain
  that keyword.
- `FlateDecode` is inflated through a reader capped at the stream's remaining
  allowance plus one inflate block, including `lopdf`'s raw-deflate retry for a
  zlib stream that produces nothing. `ASCII85Decode` is decoded under the same
  cap. A filter name `lopdf` cannot decode stops the chain: when it is the
  first stage the stream contributes nothing, because `decompressed_content`
  returns `Unimplemented` and every caller then uses the raw, input-bounded
  bytes; when it follows a decoded stage, the stages before it are still
  counted because `lopdf` inflates them before returning that error.
  `LZWDecode` cannot be measured without a second inflate implementation, so a
  stream using it is refused.
- The per-stream limit is `min(configured, remaining document allowance)`; the
  per-document limit is the configured total, so no distribution across streams
  can evade it.
- Declared geometry is checked before it is allocated: an xref stream whose
  `/W` field width exceeds eight bytes, or whose widest declared row exceeds
  the remaining allowance, is refused, because `decode_xref_stream` allocates
  `vec![0u8; width]` per field before reading an entry. A `/DecodeParms`
  predictor row (`Predictor` 10–15) that exceeds the remaining allowance is
  refused for the same reason.
- A breach is `FetchError::PdfDecompressionBoundExceeded { measured, budget }`
  with kind `pdf_decompression_bound_exceeded`. `measured` always exceeds
  `budget`: it is the accounted bytes for a measurement breach, and
  `budget + 1` for a structural refusal (an unmeasurable `LZWDecode` stream, an
  indirect `/Length` on a filtered stream, or a width beyond eight bytes). No
  page metadata, text, or blocks are produced.

An `/Encrypt` document is refused before decryption is ever considered:
`Document::authenticate_password` validated a password but never replaced
ciphertext, and ciphertext cannot be measured by the scan. `pdf_password` stays
in the tool schema for wire compatibility and is documented as unsupported.
That outcome narrowing is why the derived cache `renderer_version` is 4.

---

## 8. Egress Route (`egress.rs`, feature `egress`)

Opt-in listener-free HTTP/SOCKS proxy-chain dialer beneath eggfetch.
`EggressDialer` implements eggfetch's `Dialer` with
`OutboundConnector::connect_tcp_detailed` and typed error mapping
(Timeout→Timeout, Authentication→Authentication, Policy→Rejected,
DNS/refused/unreachable→Connection, TLS/protocol/other→Other).

- `build_connector` validates the `[egress]` chain: `http` / `socks4` /
  `socks5` schemes only, proxy passwords resolve from `password_env` at build
  time (missing/empty/incomplete credentials fail), and diagnostics stay
  redacted (`route_summary` never leaks usernames or env names).
- `apply_route` leaves direct builders untouched when no route is configured
  and **fails closed** when a route is configured without the `egress`
  feature (rebuild with `--features egress` or disable `[egress]`).
- Scope is provider search-engine upstreams only. Dynamic SSRF-pinned
  `FetchClient` paths never use this module; forge, package-resolver, updater,
  loopback health, rmcp, and browser traffic stay direct. Eggfetch keeps owning
  HTTP, pooling, destination TLS/SNI, decompression, redirect mechanics, and
  deadlines above the dialer. Prebuilt/default binaries exclude the feature.

---

## 9. Browser Rendering (`browser/`, 8 files, feature `browser`)

Headless Chrome/Chromium over CDP. Without the feature, browser paths fail with
`BrowserNotCompiledIn`; with the feature but `enabled = false`, with
`BrowserDisabled`.

| File | Responsibility |
|------|---------------|
| `mod.rs` | Module declarations + re-exports |
| `types.rs` | `BrowserConfig`, `RenderPolicy`, availability/discovery states, transport types, limits |
| `discover.rs` | `discover_browser` — explicit path vs auto-discovery |
| `lifecycle.rs` | `BrowserLifecycle` — process launch, restart budget, ephemeral cleanup |
| `navigate.rs` | `browser_fetch` / `browser_fetch_with_policy` + DOM→response conversion |
| `intercept.rs` | SSRF request interception (`is_request_allowed[_with_dns]`) |
| `classify.rs` | `classify_response` → `FetchDisposition` |
| `profiles.rs` | `ProfileManager` — persistent origin-scoped profiles |

### Discovery: explicit path never falls back

`discover_browser(configured_path)` returns `BrowserDiscoveryState`:

- Non-empty explicit path: validate it; success → `Available(Configured)`,
  anything else → `ExplicitPathInvalid { path }`. No auto-discovery fallback.
- Otherwise probe Linux well-known paths, macOS bundles (`~` expanded), then
  `PATH` binaries. Each candidate must exist, be a file, and survive a
  5 s `--version` probe. Nothing found → `NotFound`.

### Lifecycle: ephemeral vs persistent

- **Anonymous ephemeral** (`BrowserLifecycle::new`): incognito launch with a
  fresh `ctx-<nanos^pid^counter>` user-data dir (0700); `close`/drop removes
  the directory. Each navigation additionally mints and disposes an isolated
  browser context.
- **Profile-scoped persistent** (`for_persistent_profile`): launches against
  an existing profile `chrome-data` directory (must be a directory or launch
  fails); no per-navigation context isolation, no directory cleanup on close.
- Hardening flags are fixed: `--headless=new`, `--no-sandbox`,
  `--disable-default-args`, media autoplay gated by `block_media`, plus
  background-networking/sync/extension suppressions.
- `ensure_browser` allows exactly one restart and is safe under concurrent cold
  starts. `BrowserConfig` clamps timeouts (`startup` 10 s / max 60 s,
  `navigation` 20 s / max 120 s, `post_load` 1.5 s / max 30 s, `verification`
  10 s / max 60 s), `max_requests` (100 / 1000), `max_dom_bytes` (4 M / 50 M),
  and global/per-origin concurrency (1 / 4).

### Navigation with per-request SSRF

`browser_fetch_with_policy` rejects `RenderPolicy::HttpOnly`, validates the
initial URL with `is_request_allowed_with_dns`, then:

1. Installs a CDP `Fetch.requestPaused` interceptor that re-validates **every**
   request (navigation, redirects, subresources) — Chromium resolves DNS and
   follows redirects internally, so initial-URL validation alone would leave
   rebinding escapes.
2. Captures the main `Document` response status/content-type, then `goto` +
   `wait_for_navigation` under `navigation_timeout_ms`, sleeps
   `post_load_wait_ms`, and re-checks the post-redirect final URL before
   reading content.
3. `classify_response` maps status/title/text-length/body-snippet to
   `UsefulContent`, `JavascriptShell`, `NonInteractiveVerification`,
   `InteractiveChallenge`, and HTTP error dispositions. Interactive challenges
   error immediately; non-interactive verifications wait up to
   `verification_timeout_ms`.
4. DOM length is checked before and after `page.content()` against
   `max_dom_bytes`; the DOM converts through `browser_result_to_response` — the
   same render + detect + link + sanitize pipeline as HTTP, tagged
   `transport: "browser"`.

`intercept.rs` policy is fail-closed public-only: scheme allowlist, no
credentials, private-hostname/IP literal rejection (including IPv4-mapped and
IPv4-compatible IPv6 via the shared `limits::classify_ip`), and a 3 s DNS
resolution check rejecting private resolutions. It does not take
`allow_localhost` / `allow_private_network` overrides.

### Challenge pages on the HTTP path

Plain HTTP responses are classified before they are returned. `src/fetch/challenge.rs` recognises
Anubis (`anubis_challenge`, `anubis_version`, `anubis_base_prefix`, or the "could not load its
JavaScript" message), ALTCHA (`captcha-wrap` and the challenge phrase), generic interactive walls
(Turnstile, reCAPTCHA, hCaptcha markers), and verification interstitials. A detected page keeps its
bounded body, `status`, and `fetched: true` — it is not a transport failure — but the response sets
`manual_interaction_required: true` and carries one `fetch_challenge_page` structured warning naming
the family. Detection adds no request, retry, or delay, and ordinary pages, markdown bodies, and
PDFs are never marked. Under `render: "auto"` the same classification is an escalation trigger, so
a detected Anubis or ALTCHA wall is rendered by the browser when one is available instead of being
returned as a stub; `render: "http_only"` keeps the marked body.

When `[search.challenge].enabled` is true, a detected Anubis page is not merely marked.
`src/mcp/tools/web_fetch.rs` — the one module allowed to depend on both trees — solves the
published challenge once through the shared solver in `src/meta/challenge/anubis.rs` under
`max_difficulty`, `max_attempts`, `timeout_ms`, and the remaining call deadline, then replays the
same request once with the issued `spchal-auth` cookie. That cookie is sent on the first hop only,
so a redirect cannot forward it to another origin, and it is never stored: the response returned
and cached is the replay's document, with `transport: "http"`, `manual_interaction_required: false`,
and no `fetch_challenge_page` warning. Solving disabled, no parseable published challenge, a
refusal at any ceiling, an error, an exhausted deadline, or a replay that is itself a challenge all
leave today's marking exactly as it was, at most one solve and one replay per call. The browser
path keeps priority: a successful `render: "auto"` render replaces the payload before the solver
runs. `batch_fetch` does not solve (residual).

### Profiles: opaque IDs, origin-scoped logins

`ProfileManager` owns persistent logins under a 0700 root with per-profile
`chrome-data`, atomic `profile.toml` writes, `flock` locks (`ProfileBusy`),
symlink and path-escape rejection, an `allowed_profiles` allowlist, a
32-profile cap, and schema version 1 with browser-major compatibility checks.

- Display names are 1–64 ASCII `[A-Za-z0-9_-]`; origins must be bare
  `http(s)://host:port` (no paths, credentials, localhost, or loopback).
- The on-disk directory and every cache scope use the **opaque** ID
  `prof_<16-hex-fnv(display, origin)>` — display names never appear in paths
  or `CacheScope::Profile`. `resolve_for_origin` additionally binds each use to
  the profile's registered origin.

---

## 10. Span and Focus Projection

### `span.rs`: symbol-aware block expansion for `repo_fetch`

`select_span(lines, language, symbol, symbol_kind, match_text, explicit range,
expand_to_block, max_block_lines)` is deterministic and lexical — regex
definition matchers per language family (Rust, Python, JS/TS, Go, generic
brace-class for the rest), case-sensitive, with `Exact` / `Strong` / `Weak`
confidence and human-readable `reasons`:

- Explicit range without expansion → `ExplicitRange` verbatim.
- Explicit range with expansion → midpoint expanded to the enclosing block.
- Symbol hit → enclosing block (`SymbolDefinition`, or `SymbolReference` when
  the kind is unknown), including Rust attributes/doc-comments and Python
  decorators/comments above the definition.
- `match_text` → first case-insensitive hit plus a bounded context window
  (capped at 20 lines of context).
- Nothing provided → `WholeFileBounded`; provided-but-unfound → `None` (no
  match), never a silent whole-file fallback.

Expansion is structural: brace counting (Rust/Go/C-family/JS-with-braces),
indentation (Python), statement boundaries (braceless JS arrows), Markdown
heading sections, TOML tables, YAML key subtrees. Results clamp to
`max_block_lines` with `truncated_by_max_block_lines` recorded.

### Focus: deterministic lexical projection (`core/focus.rs` + `core/fetch_policy.rs` + `core/fetch_locator.rs`)

`select_focus_chunks` ranks the already-extracted `FetchDocument` chunks
against a caller `focus` query with dependency-free lexical scoring
(normalized token overlap, exact-phrase boost, heading-path overlap,
case-sensitive code-symbol boost, document-order tie-break), expands picks to
scoring neighbors within the chunk cap, and enforces chunk/character budgets in
document order. No embeddings, no model calls, no extra URL traversal. The
`FocusedFetchSelection` (`chunks`, `truncated`, `total_chars`) is **additive**
on `WebFetchResponse` and per-item `batch_fetch` payloads.

- Shared validation/projection lives in `core/fetch_policy.rs`:
  `validate_focus_query` (non-empty, length-capped),
  `validate_focus_max_chunks` / `validate_focus_max_chars`,
  `validate_focus_for_extract_mode` (focus with `metadata_only` is rejected),
  `focus_max_*_or_default`, and `apply_focus_to_document` (requires a fetched
  document plus a query). Batch repo items without a document project over
  deterministic line-window text.
- Shared locator semantics live in `core/fetch_locator.rs` (`FetchLocator ::
  WebUrl | Repo | Local` plus path/host validation) so batch, repo, and
  suggested-fetch conversions agree without collapsing into one generic tool.
- Focus projection **never enters raw or derived cache keys**: identical URLs
  share cache entries regardless of focus query, and different focus windows
  re-project from the same stored document.

---

## 11. Errors (`types.rs`) and Safety Invariants

`FetchError` (33 variants) with a payload-free `FetchErrorKind` mirror and
`error_code()` strings for MCP mapping: URL/SSRF (5: `InvalidUrl`,
`UnsupportedScheme`, `PrivateNetworkBlocked`, `UrlTooLong`,
`EmbeddedCredentialsBlocked`), redirects (3: `RedirectLimitExceeded`,
`RedirectTargetBlocked`, `InvalidRedirectLocation`), transport (6: `Timeout`,
`HttpStatus`, `NetworkError`, `ContentTooLarge`, `UnsupportedContentType`,
`ExtractError`), PDF (10 variants), browser (8 variants), and `Unknown` —
5 + 3 + 6 + 10 + 8 + 1 = 33. `pdf_decompression_bound_exceeded` is additive;
`pdf_parse_error` and every other PDF or fetch string keep their meaning.

Invariants that hold across the subsystem:

- SSRF first: validation blocks localhost/private IPs and credentials before
  any socket, on the initial URL and every redirect, browser navigation,
  subresource, and final URL.
- Bounded I/O: streamed bodies cap at `max_bytes` with UTF-8-safe truncation
  (`append_bounded`); git work uses `run_bounded_command()` with process-group
  kill. The crate's other outbound bodies stay bounded outside this module —
  engine responses through `read_bounded_body()`, forge responses through
  `read_with_budget()` under `ForgeReadBudget`; no bare `.text()` / `.bytes()`
  / `.json()`.
- No direct reqwest: every outbound HTTP request is an `eggfetch_core::Client`
  built here, so eggfetch keeps owning pooling, TLS/SNI, decompression,
  redirects, and deadlines. Enforced by the
  `no_direct_reqwest_in_production_source` static guard.
- PDF inflation is bounded before parse: `scan_stream_budget` measures stream
  expansion against `[fetch].pdf_max_decompressed_bytes`, and non-test
  `src/fetch/pdf.rs` contains no `decompressed_content` or `.decompress(` call
  (enforced by the `pdf_inflation_enters_only_through_the_bounded_scanner`
  static guard).
- All untrusted text passes `sanitize_field()`; stable IDs stay content-derived
  FNV-1a (`fetch_id`, `doc_id`); `CacheScope::Profile` uses opaque IDs; invalid
  explicit browser paths are `ExplicitPathInvalid` with no fallback.

---

- [Overview](overview.md) | [Core](core.md) | [Metasearch](meta.md) |
  [Engines](engines.md) | [MCP](mcp.md) | [Commands](commands.md) |
  [Config](config.md) | [Evidence & Workflow](evidence-workflow.md) |
  [Hardening](hardening.md) | [Testing](testing.md) | [Build](build.md) |
  [Maintenance](maintenance.md)
