# Fetch Deep Dive

**Location:** `src/fetch/` (10 top-level files + `browser/` + `render/`, 8 files each)
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
2. Clamp caller `max_chars` to `[1, max_chars_cap]`; keep a separate
   `max_chars_raw = max_chars_cap` budget for `raw_text` so internal consumers
   (`repo_fetch` line/span selection) see full source text while tool output
   stays clamped to the caller budget.
3. Loop: validate current URL with resolved addresses → send → on 3xx require a
   non-empty `Location`, resolve relative targets, reject past `redirect_limit`,
   re-validate the target (failures map to `RedirectTargetBlocked`), continue.
4. On the final response, harvest `etag` / `last-modified` / `cache-control` /
   `expires` / `vary` for the cache layer.
5. `Content-Length` pre-check: a declared length above `max_bytes` fails fast
   with `ContentTooLarge`. Unparseable lengths warn and fall through to the
   authoritative streaming cap.
6. Non-2xx becomes `HttpStatus`. Content-type gating runs twice from one source
   of truth: pre-body on headers + URL extension, then post-body with `%PDF-`
   magic sniff for mis-served PDFs.
7. Body streams through `append_bounded` (cap `max_bytes`, UTF-8-safe, sets
   `truncated`); `derive_from_raw` then builds the response, so cache raw hits
   reuse the exact rendering path as network fetches.

`fetch_conditional` repeats the loop with caller conditional headers for stale
revalidation; a 304 returns empty-body status so the cache layer can refresh
validators in place.

### Timeout / byte / redirect / content-type policy

| Policy | Rule |
|--------|------|
| Timeout | `FetchLimits::timeout_ms` (default 8000). DNS phase consumes at most half (1500 ms floor); the remainder covers the HTTP round-trip plus body streaming. Send errors mapping to timeout become `FetchError::Timeout(timeout_ms)`. |
| Bytes | `max_bytes` (default 2,000,000) caps the streamed body; `max_chars_default` (12,000) and `max_chars_cap` (50,000) cap extracted text and `raw_text` respectively. `max_url_len` is 8192 bytes. |
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

- `RawCacheKey { url, scope }` where `url` is the canonicalized form from
  `core::identity::canonicalize_url` (`build_raw_cache_key`).
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
  `pdf_ocr`, `include_media`, `renderer_version: 1`, and `sanitize_output`.
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
  scope, any `Vary` token besides `Accept-Encoding`, and image/audio/video plus
  `application/octet-stream` bodies.
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
| `markdown_source.rs` | `render_markdown_source` — pulldown-cmark parse of `.md` sources (tables, strikethrough) |

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

## 7. PDF Gate (`pdf.rs`, feature `pdf`)

Double-gated: the `pdf` Cargo feature must be compiled in **and**
`FetchLimits::pdf_enabled` must be true, else `PdfNotCompiledIn` / `PdfDisabled`
before any body bytes are retained. Detection combines Content-Type,
`.pdf` URL extension, and `%PDF-` magic.

`extract_pdf_text(bytes, max_chars, PdfLimits, PdfExtractOptions)` via `lopdf`:

- `PdfLimits { max_pages` (25), `max_chars_per_page` (12,000),
  `max_total_chars` (50,000) `}` bound pages, per-page text, and the running
  total (exceeding the total stops after the current page with a warning).
- Page selection parses `1-3,5` specs (`parse_pdf_page_spec`:
  1-indexed, sorted, deduplicated, range and `max_pages` caps) and validates
  against the real page count (`PdfPageOutOfRange`, `PdfPageCapExceeded`,
  `PdfPageSpecInvalid`).
- Encrypted documents require `password` (`RedactedString`) or fail with
  `PdfEncrypted`; unparseable bytes fail with `PdfParseError`. OCR policies
  `Auto` / `Always` fail closed with `PdfOcrUnavailable` — this build never
  performs OCR.
- Output is page-indexed `Paragraph` blocks plus a `Page N` outline, legacy
  `--- Page N ---` text, Info-dictionary metadata, outline extraction, and
  per-page quality (`CleanText` 1.0 down to `Blank` 0.0) rolled into
  `quality_score` and `content_ok`. All-blank documents return
  `PdfNoExtractableText`.
- `MetadataOnly` extract mode skips text extraction and returns fetch metadata
  with an empty PDF document shell.

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

`FetchError` (30 variants) with a payload-free `FetchErrorKind` mirror and
`error_code()` strings for MCP mapping: URL/SSRF (`InvalidUrl`,
`UnsupportedScheme`, `PrivateNetworkBlocked`, `UrlTooLong`,
`EmbeddedCredentialsBlocked`), redirects (`RedirectLimitExceeded`,
`RedirectTargetBlocked`, `InvalidRedirectLocation`), transport (`Timeout`,
`HttpStatus`, `NetworkError`, `ContentTooLarge`, `UnsupportedContentType`,
`ExtractError`), PDF (9 variants), browser (8 variants), and `Unknown`.

Invariants that hold across the subsystem:

- SSRF first: validation blocks localhost/private IPs and credentials before
  any socket, on the initial URL and every redirect, browser navigation,
  subresource, and final URL.
- Bounded I/O: streaming bodies cap at `max_bytes` with UTF-8-safe truncation;
  forge reads use `read_bounded_body()`, git work uses `run_bounded_command()`
  with process-group kill.
- All untrusted text passes `sanitize_field()`; stable IDs stay content-derived
  FNV-1a (`fetch_id`, `doc_id`); `CacheScope::Profile` uses opaque IDs; invalid
  explicit browser paths are `ExplicitPathInvalid` with no fallback.

---

- [Overview](overview.md) | [Core](core.md) | [Metasearch](meta.md) |
  [Engines](engines.md) | [MCP](mcp.md) | [Commands](commands.md) |
  [Config](config.md) | [Evidence & Workflow](evidence-workflow.md) |
  [Hardening](hardening.md) | [Testing](testing.md) | [Build](build.md) |
  [Maintenance](maintenance.md)
