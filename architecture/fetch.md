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

### Typed target routing

The MCP tools keep their distinct request and response contracts. Their shared
internal `FetchLocator` resolution distinguishes the route before handing work
to its owning handler:

| Locator | Owning path | Boundary |
|---|---|---|
| `WebUrl` | `web_fetch` or a batch web item through the bounded fetch path | HTTP(S) validation, `FetchClient` SSRF/redirect policy, content detection, and existing PDF/HTML/browser behavior remain authoritative. |
| `Repo` | `repo_fetch` remote-source path | Structured host/owner/repo/ref/path and source-span semantics remain authoritative; a detected PDF retains the fetch document outcome and is not decoded as source code. |
| `Local` | `repo_fetch` workspace path | Root name and relative path only; authorized workspace resolution and `safe_read_file` remain the read boundary. Local PDF extraction belongs to PDF M008. |

Resolution is classification, not a transport shortcut: it cannot bypass the
owning handler's validation or security policy, and it does not expose a generic
MCP fetch schema. Batch preflight uses the same typed resolution before any
fetch starts. Web items carry `pdf`, `render`, and `browser_profile` controls
through the shared web-fetch path, alongside per-item cache and focus options;
repo items carry source selection through the repo-fetch handler. The trusted
`codegg/host-local-scope` metadata is forwarded unchanged to repo items, so
workspace reads retain the same authorized root and root-relative path boundary
as `repo_fetch`. Batch order, per-item failures, and aggregate budgets remain
owned by the batch envelope.

### Batch returned-character budget

The unified `fetch` envelope measures web and repo items with one deterministic
returned-character rule: Unicode scalar values in the top-level `text`, plus
`title`, `description`, and visible link URL/text metadata. Repo items also count
the string-valued locator and returned URL fields. The measure is applied to each
item before the caller's `response_detail` projection. Repo `document` and `lines`
are structural representations of the same text and are not counted a second
time.

When an item exceeds the remaining `max_total_chars`, its text is clipped
UTF-8-safely before omission. Repo locator and bounded metadata are retained;
only the top-level `text` is clipped, while the structural `document` and `lines`
remain unchanged. If the repo locator and metadata alone cannot fit, the item is
omitted with `response: null`, `ok: false`, `chars_returned: 0`, an item-specific
budget error, and `truncated: true`. Aggregate fetched/failed and telemetry counts
follow those item outcomes. A single `batch_total_budget_exhausted` warning (the
raw prefix; the structured entry carries the existing `fetch_content_truncated`
code) names the first item affected and the `max_total_chars` budget;
budget-limited truncation and omission are never silent.

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
  `pdf_ocr`, `include_media`, `renderer_version: 5`, and `sanitize_output`.
  Version 5 isolates document-scoped extraction; version 4 isolates the
  encrypted-PDF refusal behavior; version 3 isolates entries derived before
  PDF page classification stopped depending on `include_media`; version 2
  isolated entries written before representation negotiation existed.
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
  indirect `/Length` is resolved first by locating its `N G obj` header and
  reading the integer that follows it. If the reference resolves, the direct
  rule applies; if it cannot be resolved, a filtered stream is still refused
  instead of falling back, because `lopdf` resolves the reference and inflates
  the full declared extent, which the `endstream` fallback can under-measure
  when the compressed bytes contain that keyword.
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
  unresolvable indirect `/Length` on a filtered stream, or a width beyond eight
  bytes). No
  page metadata, text, or blocks are produced.

An `/Encrypt` document is refused before decryption is ever considered:
`Document::authenticate_password` validated a password but never replaced
ciphertext, and ciphertext cannot be measured by the scan. `pdf_password` stays
in the tool schema for wire compatibility and is documented as unsupported.
That outcome narrowing advanced the derived-cache renderer version to 4; the current version is 5 after document-scoped extraction.

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

Headless Chrome/Chromium over CDP. The `browser` feature is in the default set, so
the capability is compiled into default binaries and `[fetch.browser].policy`
defaults to `auto`; HTTP remains the hot path and rendering is escalation.
An explicit `render = "browser"` or `browser_profile` request reports the stable
codes `browser_not_compiled_in` (feature absent), `browser_disabled`
(`enabled = false`), and `browser_unavailable` (no discovered executable)
instead of silently performing an HTTP fetch. The tool-level codes are the
agent contract; the fetch-layer error variants of the same names remain reserved
for direct library use. `RenderPolicy::Auto` never errors:
when escalation was warranted but unavailable, the call returns the HTTP document
with the `browser_unavailable` structured warning and `browser_escalated: false`.

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
  and global/per-origin concurrency (1 / 4). `idle_timeout_ms` has a 90 s default
  and a hard 5 s floor enforced by configuration validation, not by clamping.
- **Shared bounded lifecycle**: one `BrowserLifecycle` per process, not one per
  call. `begin_render` takes the read side of the render gate and counts the
  overall render attempt. Only after `ensure_browser` succeeds is the attempt
  counted against that browser session; therefore the cold-starting render is
  request one, while a failed launch consumes no session capacity. `finish_render`
  drops the gate, touches activity, and — once
  `requests_this_session` reaches `max_requests` — takes the write side and
  closes the session for `SessionCap`, so the next render relaunches. An idle
  timer closes the browser and cancels its handler after
  `[fetch.browser].idle_timeout_ms` with no render in flight, which returns the
  process to its HTTP-only memory baseline instead of holding a browser for the
  process lifetime. Teardown is safe against a concurrent render and never
  disables the lifecycle.
- **Observable counters**: `BrowserLifecycleStatus` reports `resident`,
  `session_launches`, `renders`, `idle_teardowns`, and `session_cap_teardowns`
  as `provider_status.browser_capabilities.lifecycle`; the object is `null` when
  the feature is not compiled or `[fetch.browser].enabled = false`.

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
   error immediately. A script-driven wall polls the live classification every
   500 ms up to `verification_timeout_ms` and is returned marked when it does
   not resolve; see § Browser challenge completion.
4. DOM length is checked before and after `page.content()` against
   `max_dom_bytes`; the DOM converts through `browser_result_to_response` — the
   same render + detect + link + sanitize pipeline as HTTP, tagged
   `transport: "browser"`.

`intercept.rs` policy is fail-closed public-only: scheme allowlist, no
credentials, private-hostname/IP literal rejection (including IPv4-mapped and
IPv4-compatible IPv6 via the shared `limits::classify_ip`), and a 3 s DNS
resolution check rejecting private resolutions. It does not take
`allow_localhost` / `allow_private_network` overrides.

### Viewport screenshot capture

`include_screenshot = true` on `web_fetch` or a `batch_fetch` web item forces
`RenderPolicy::Browser`, even when the configured policy is `http_only`; an
explicit per-call `render = "http_only"` is a pre-I/O conflict. Capture runs
after normal DOM extraction and before the page closes, using the existing
1280×720 viewport. It requests PNG with no full-page extension, scrolling, input,
or other interaction. The PNG signature, IHDR dimensions, and encoded size are
checked before result assembly; width and height may not exceed 1280×720 and the
PNG may not exceed 4 MiB. Capture failures are explicit errors, not text-only
successes. Recognized PDF main responses skip capture and add a warning; repo
and local workspace targets never enter this browser path.

Screenshot requests bypass both cache reads and writes, regardless of the
requested cache policy. Image bytes travel outside `WebFetchResponse` and the
cache structures. The ordinary JSON response and its output schema are unchanged.

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
returned as a stub; `render: "http_only"` keeps the marked body. The remote
`repo_fetch` response always serializes `manual_interaction_required`; when a challenge page
is returned, `fetch_challenge_page` is surfaced through `structured_warnings`. Its workspace
path always sets the flag to false.

When `[search.challenge].enabled` is true, a detected Anubis or ALTCHA page can be solved
and replayed once. `src/challenge_replay/` owns the shared fetch-path detect → solve → replay →
classify sequence, combining the bounded fetch client with the solvers in
`src/meta/challenge/anubis.rs` and `src/meta/challenge/altcha.rs`. `web_fetch`, `batch_fetch`,
and unified `fetch` web items delegate through the shared web-fetch handler; the remote
`repo_fetch` path and CLI `eggsearch fetch` use the same orchestration. Each fetch-path solve
uses the ledger label `fetch` and is bounded by `max_attempts`, `timeout_ms`, and the remaining
call deadline. `max_difficulty` gates the Anubis family only; an ALTCHA solve publishes no
difficulty and is bounded by its PBKDF2 `cost`, the attempt ceiling, and the wall clock.

The solve replays the same request once with the cookie pairs the pass response issued, joined with
`; `. The pair set is opaque — its names are whatever the deployment publishes (`spchal-auth`,
`techaro.lol-anubis-auth`, or a dynamic suffix), never a name the client formats — and is sent on
the first hop only, so a redirect cannot forward it to another origin. It is never written to
disk; a pair set a fetch issued is reused from the process-local origin-keyed cache described
below. An
ALTCHA solve replays with the `chllg` pass cookie its verification endpoint issued. A
successful replay returns the document with `manual_interaction_required: false` and no
`fetch_challenge_page` warning. Solving disabled, no parseable published challenge, a refusal at
any ceiling, an error, an exhausted deadline, or a replay that is itself a challenge all leave the
original response marking unchanged, with at most one solve and one replay per call. For web fetch
the solver keeps priority: under `render: "auto"` a solvable Anubis or ALTCHA wall is solved and
replayed before any browser escalation is considered, and the browser receives only what the solver
left unsolved. `batch_fetch` inherits solve behavior through the shared web-fetch handler.

### ALTCHA endpoint derivation

An ALTCHA wall publishes its proof-of-work material in the widget markup, so a general fetch has to
derive two things from the page itself: the challenge document, and the endpoint that verifies a
solution. `src/challenge_replay/altcha.rs` owns both. The rule is fixed and bounded, and every
refusal happens before a request is issued.

**Accepted publication.** The page must carry an `altcha-widget` element, the upstream web
component. Two attributes are read from it:

| Purpose | Accepted attributes |
|---|---|
| Challenge material | `challenge` (upstream v2+: inline JSON or a URL), `challengeurl` (upstream v1) |
| Verification endpoint | `verifyurl` (upstream v1), `data-verify-url` |

A `challenge` value that starts with `{` is the published challenge document itself, read inline
and bounded to 64 KiB. Any other `challenge` value, and any `challengeurl` value, is a published
challenge endpoint. A page that publishes neither carries no solvable challenge.

**Endpoint form.** A published endpoint is accepted only when every one of these holds:

1. The trimmed value is at most 256 characters.
2. It resolves — with `Url::join` against the challenge page's own URL, never by string
   concatenation — to an absolute URL.
3. The resolved scheme is the challenge page's own scheme, and that scheme is `http` or `https`.
   A scheme change fails closed.
4. The resolved URL carries no credentials, no query, and no fragment.
5. The resolved origin — host and effective port — is the challenge page's own origin. A
   cross-origin target fails closed, which is what stops a page from steering a solve at another
   site.
6. The resolved path matches the fixed allowed shape: one to four segments, each at most 32
   characters drawn from `[A-Za-z0-9._~-]`, whose last segment is exactly `challenge` for a
   challenge endpoint and exactly `verify` for a verification endpoint.

A challenge endpoint that passes is requested with `Accept: application/json` and a bounded body of
at most 64 KiB.

**Verification endpoint.** A published verification endpoint is used as published, under the same
six rules. When the page publishes none, the verification endpoint is derived from the published
challenge endpoint by replacing its final `challenge` segment with `verify`, keeping the origin
and the prefix; a page that publishes an inline challenge and no verification endpoint falls back
to the canonical `/captcha/verify` on its own origin. The request URL is rebuilt from the
resolved URL's own parsed components; no page text is spliced into a URL string.

**Refusals.** A cross-origin, scheme-changing, credentialed, query-bearing, unparseable, or
over-long endpoint, and an unsupported published algorithm, fail closed as `unsupported scheme`. A
published challenge document that does not parse into the documented parameter set fails closed as
`malformed parameters`. The published PBKDF2 `cost` is part of that parameter set: it is the work
factor of one counter iteration, so a cost above the solver's ceiling is refused rather than
started. Both are existing refusal keys; the vocabulary does not grow. Every refusal leaves the
original response exactly as it was.

### Fetch-side pass-cookie reuse

A pass cookie the fetch path is issued is reused for a later fetch to the same origin instead of
being solved again every time. The reuse window is the same rule the search side already uses, and
the reuse itself is bounded by the same origin rule the ALTCHA derivation uses.

**Owner.** `src/challenge_replay/cache.rs` owns the cache. It is process-local: a stored pair set
lives in memory for the life of the process and is never written to disk, never persisted to a
cache directory, and never serialized into a response, a diagnostic, or a fixture. The cache is the
only place a fetch-path pair set is retained, and only `src/challenge_replay/` may reach it.

**Origin key.** The key is the origin of the *request* URL: scheme, host, and effective port. It is
derived without following a redirect. An entry is stored and looked up only when the origin that
served the challenge page is that same origin. A fetch that crossed an origin — a redirect to
another host, another scheme, or another port — has no reusable key, because the issuing origin
cannot be established without following the redirect; such a fetch stores nothing and reuses
nothing. A cached pair set is therefore never replayed to an origin other than the one that issued
it, and the replay itself still travels on the first hop only.

**Reuse window.** The window is M014's rule, unchanged. `src/meta/challenge/cookie.rs` resolves the
lifetime the upstream published — `Max-Age` over `Expires`, `Expires` in the past never usable —
and clamps it to `[search.challenge].cookie_ttl_seconds`. `cookie_ttl_seconds = 0` stores
nothing; an unstated lifetime falls back to the ceiling; a malformed lifetime, a zero lifetime, or
an expiry at or before now stores nothing. The shipped default is unchanged.

**Sequence.** On a detected Anubis or ALTCHA page the shared module consults the cache before it
solves. A hit replays the request once with the stored pair set and performs no solve. A miss
solves as usual and, when the replay returns the document, stores the issued pair set under its
published lifetime. The one-solve-one-replay rule is unchanged: a fetch never solves and replays
twice, and it never uses a cached cookie to avoid a solve inside the same call.

**Stale pair set.** A stored pair set the upstream refuses is not a final outcome for the call that
found it. When the replay comes back as a challenge page, the entry is evicted and the same call
falls through to the ordinary solve path, inside the same budget. The rejected pair set is never
presented a second time, and the call still performs at most one solve and one replay: the replay
budget is spent on the fresh pair set, not on the refused one. ADR-0018 records that bound.

The eviction is unconditional and happens before the solve, so a refused pair set is never offered
to the upstream again, and the entry a fresh solve stores is the only entry the origin holds.

A refusal is a challenge page, and nothing else. A transport error, a timeout, and a non-2xx
status are not refusals: the stored pair set is kept, no solve is attempted, and the call leaves
the original response exactly as it was. A transient failure therefore never spends the call's
solve budget, and a usable pair set is never discarded because of one.

This bounds the degradation M033 removed. A stale pair set used to cost one degraded fetch: the
call returned the marked challenge page, the entry was evicted, and the *next* fetch solved. Now the
call that discovers the stale pair set solves it, so a caller never has to await a second fetch to
get a document. The cost is one extra upstream request in that call — the refused replay — which
the ordinary solve path would have spent on the challenge page anyway.

The budget is unchanged and still binds. The refused replay and the fresh solve share the call's
single solve: a refused pair set costs no solve attempt of its own, because no solve was performed
for it. A solve that then refuses, errors, exhausts the deadline, or replays into another challenge
page leaves the original response exactly as it was, and the origin is left with no entry. The
fresh pair set is stored under the same published-lifetime and ceiling rules as any other, so the
reuse window is never extended by the recovery.

**Bounds.** The cache holds at most 32 origins and at most 64 KiB of stored pair-set bytes. Both
bounds are explicit: an insertion that would exceed either bound evicts the entry closest to
expiry first, and an insertion larger than the byte ceiling on its own is not stored at all.

### Browser challenge completion

The browser is a completion mechanism for script-driven walls. It is neither a replacement for the
cheap solver nor a way to return a wall as a document. Three rules fix the order and the outcome.

**Order of mechanisms.** When a plain HTTP fetch returns a wall and `render: "auto"` is in effect,
the solver runs first. `src/challenge_replay/` decides whether the response is a solvable Anubis or
ALTCHA page, and a solve that returns the document ends the call before any browser work is
considered. Only a response that is still a wall — solving disabled, no parseable challenge, a
refusal at a ceiling, a failed replay, or a family the solver does not own — reaches the browser,
under the same `browser` feature, availability check, and remaining-deadline floor as any other
auto escalation. A call therefore performs at most one solve-and-replay and at most one escalation,
in that order, and a solvable puzzle never reaches Chrome. `render: "http_only"` keeps the marked
body and never escalates; an explicit `render: "browser"` stays a direct render with its existing
semantics.

**Completion rule.** After the page loads and `post_load_wait_ms` elapses, the render classifies the
live page with `classify_response` over the current status, content type, title,
`document.body.innerText` length, and bounded body. A page classified `JavascriptShell` or
`NonInteractiveVerification`, or one `src/fetch/challenge.rs` recognises as a challenge family, is
treated as a script-driven wall: the render polls the same live state every 500 ms until the page
stops being one of those two dispositions, bounded by `verification_wait_ms` and the call deadline.
Completion is decided by that classification, never by the title, so a page that clears without
changing its title is no longer treated as unresolved and a legitimate page whose title happens to
carry a verification phrase is no longer treated as a wall. `InteractiveChallenge` remains an
immediate `manual_interaction_required` error and is never polled.

**Completed or marked.** A wall that resolves inside the wait returns the document. A wall that does
not resolve is returned, not errored: the response carries the rendered body,
`manual_interaction_required: true`, and the existing `fetch_challenge_page` structured warning when
`src/fetch/challenge.rs` recognises the family. A page that was never a wall keeps today's response
and `manual_interaction_required: false`, so a legitimate document is never reported as a
challenge. A wall completed by the browser is counted once in
`provider_status.challenge_solver.providers.fetch.browser_completions`; the counter is a count only
and carries no cookie, nonce, challenge id, pass token, or profile path.

A wall whose own failure page presents ordinary content is indistinguishable from a document once
rendered, so it is returned as one. That case is recorded as residual risk in the milestone closure
rather than guessed at from a title or a status code.

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
