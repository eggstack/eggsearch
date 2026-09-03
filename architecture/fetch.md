# HTTP Fetch & Extraction Deep Dive

**Location:** `src/fetch/` (10 top-level files + 2 subdirectories)
**Purpose:** Fetch HTTP(S) URLs, enforce limits, extract readable content, and render HTML. Independent of the metasearch adapter.

---

## Module Map

| File | Responsibility |
|------|---------------|
| `mod.rs` | Module declarations and re-exports |
| `client.rs` | `FetchClient` — HTTP fetch client with limits enforcement |
| `extract.rs` | `HtmlExtractor`, `extract_content()` — HTML→text/markdown extraction |
| `detect.rs` | Content type detection |
| `limits.rs` | `FetchLimits`, `validate_fetch_target()` — URL/size validation |
| `cache.rs` | `FetchCache` — two-tier LRU cache (raw + derived), `CacheScope`, `CacheFreshness` |
| `origin.rs` | `OriginController` — per-origin concurrency, circuit breaker, retry policy |
| `types.rs` | `FetchError`/`FetchErrorKind` |
| `span.rs` | `SelectedSpan` — symbol/span-aware block expansion for `repo_fetch` |
| `pdf.rs` | PDF text extraction via `lopdf` (feature-gated `pdf`) |

### Subdirectories

| Dir | Files | Responsibility |
|-----|-------|---------------|
| `fetch/browser/` | 8 files | Headless Chrome/Chromium rendering via CDP |
| `fetch/render/` | 7 files | HTML structural rendering (blocks, text, markdown) |

---

## FetchClient (`client.rs`)

The HTTP fetch client. Handles:

- **Request construction** — headers, timeouts, redirects
- **Limits enforcement** — max bytes, max time, content-type validation
- **Response classification** — success, redirect, error
- **Bounded body reading** — never reads unbounded responses

### Key Methods

```rust
impl FetchClient {
    async fn fetch(&self, request: WebFetchRequest) -> Result<WebFetchResponse>;
}
```

---

## Content Extraction (`extract.rs`)

HTML→text/markdown extraction pipeline:

1. **Parse HTML** — `scraper` crate with CSS selectors
2. **Extract readable content** — remove scripts, styles, nav
3. **Convert to text/markdown** — `pulldown-cmark` for markdown
4. **Extract metadata** — title, description, links
5. **Bound output** — `FetchLimits` enforces max chars

### Extraction Modes

| Mode | Output |
|------|--------|
| `Text` | Plain text |
| `Markdown` | Markdown-formatted text |
| `MetadataOnly` | Title, description, links only |

---

## Two-Tier Cache (`cache.rs`)

### Raw Cache
- Stores original bounded transport bytes (HTML/PDF/text) or bounded rendered browser DOM
- Keyed by canonical URL + scope; a fresh raw hit can be re-derived locally for changed extraction params
- Preserves original format for re-extraction

### Derived Cache
- Stores extracted/sanitized content including the structured `FetchDocument` with stable chunks
- Keyed by scope + raw hash + extraction params
- Avoids re-extraction for same content

### Cache Scopes

| Scope | Purpose |
|-------|---------|
| `Anonymous` | Default shared cache |
| `Profile(opaque_id)` | Browser profile-scoped cache (opaque IDs, never display names) |

### Cache Policy (`FetchCachePolicy`, `max_cache_age_seconds`)

Agent-visible controls on `web_fetch` (and per web item on `batch_fetch`):

| Policy | Behavior |
|--------|----------|
| `default` | Serve a fresh eligible entry; otherwise revalidate with validators when possible, else fetch |
| `bypass` | Skip cache reads; network fetch still populates the cache unless the origin forbids storage |
| `refresh` | Never serve solely for being locally fresh; revalidate with `ETag`/`Last-Modified` when available, else fetch |

`max_cache_age_seconds` (0-2,592,000) is an upper bound on acceptable entry age that only tightens origin freshness; `0` forces revalidation without disabling storage. Neither control bypasses SSRF, redirect, origin-concurrency, profile-isolation, content, or sanitization policy. `CacheStatus` reports `hit`, `revalidated`, `miss` (fresh fetch, including after refresh), `bypassed`, or `not_cacheable`. Conditional revalidation treats HTTP 304 as a revalidation signal (not a redirect).

### Cache Freshness

`CacheFreshness::is_fresh()` consults `no_store`/`no_cache`, `max-age` vs `fetched_at`, and `Expires`. Entries without origin freshness headers get the configured default TTL. Batch fetch stores set `fetched_at` like single fetch stores do, so batch entries are equally eligible for hits.

Responses with `Vary: *` or any request header other than `Accept-Encoding` are not cached because the cache does not retain those request-header variants. This conservative rule also applies when supported and unsupported `Vary` tokens are mixed.

## Focused Fetch (`core/focus.rs`)

`select_focus_chunks()` ranks the already-extracted `FetchDocument` chunks against a caller `focus` query with dependency-free lexical scoring (normalized token overlap, exact-phrase boost, heading-path overlap, case-sensitive code-symbol boost; stable tie-break by document order), expands picks to scoring neighbors within the chunk cap, and enforces chunk/character budgets in document order. No embeddings, no model calls, no extra URL traversal. The `FocusedFetchSelection` (`chunks`, `truncated`, `total_chars`) is additive on `WebFetchResponse`; focus projection never enters the raw or derived cache keys.

---

## Origin Controller (`origin.rs`)

Per-origin request management:

- **Concurrency limits** — max parallel requests per origin
- **Circuit breaker** — detect and handle failing origins
- **Retry policy** — exponential backoff for transient failures
- **Failure classification** — distinguish permanent vs transient errors

### Key Types

```rust
struct OriginController {
    // Per-origin state
}

struct OriginPolicy {
    max_concurrency: usize,
    circuit_breaker_threshold: u32,
    retry_backoff: Duration,
}
```

---

## Fetch Limits (`limits.rs`)

URL and content validation:

| Limit | Default | Purpose |
|-------|---------|---------|
| `max_chars` | 50,000 | Max extracted content length |
| `timeout_ms` | 30,000 | Request timeout |
| `max_redirects` | 10 | Max redirect hops |
| `allowed_schemes` | [https] | Only HTTPS by default |

### URL Validation

```rust
fn validate_fetch_target(url: &str, limits: &FetchLimits) -> Result<()>;
```

Checks:
- Valid URL format
- Allowed scheme (https)
- No localhost/private IPs
- No SSRF vectors

---

## Browser Rendering (`fetch/browser/`)

Headless Chrome/Chromium via Chrome DevTools Protocol (feature-gated `browser`).

| File | Responsibility |
|------|---------------|
| `mod.rs` | Module declarations |
| `types.rs` | `BrowserConfig`, `BrowserAvailability`, `BrowserFamily` |
| `discover.rs` | `discover_browser()` — find Chrome/Chromium binary |
| `lifecycle.rs` | `BrowserLifecycle` — process management, startup/shutdown |
| `navigate.rs` | Page navigation, wait strategies |
| `intercept.rs` | Request interception, blocking |
| `classify.rs` | Response classification |
| `profiles.rs` | `ProfileManager` — persistent browser profiles |

### Browser Capabilities

- JavaScript-heavy page rendering
- SPA content extraction
- Cookie/session persistence via profiles
- Request interception and blocking

### Key Constants

```rust
DEFAULT_NAVIGATION_TIMEOUT_MS: 30_000
DEFAULT_POST_LOAD_WAIT_MS: 2_000
DEFAULT_STARTUP_TIMEOUT_MS: 15_000
MAX_GLOBAL_CONCURRENCY: 8
MAX_PER_ORIGIN_CONCURRENCY: 3
```

---

## HTML Rendering (`fetch/render/`)

Structural HTML rendering pipeline:

| File | Responsibility |
|------|---------------|
| `mod.rs` | Module declarations |
| `blocks.rs` | Block decomposition (headings, paragraphs, lists, code) |
| `text.rs` | Plain text extraction |
| `markdown.rs` | Markdown rendering |
| `markdown_source.rs` | Source-aware markdown rendering |
| `code.rs` | Code block handling |
| `csv.rs` | CSV table rendering |
| `notebook.rs` | Jupyter notebook rendering |

### Render Pipeline

```
HTML
  → Block decomposition (ego-tree traversal)
  → Block classification (heading, paragraph, code, list, table)
  → Per-block rendering (text, markdown, code)
  → Output assembly
```

---

## PDF Extraction (`pdf.rs`)

Feature-gated (`pdf`). Uses `lopdf` for text extraction:

- Parse PDF structure
- Extract text blocks
- Preserve reading order
- Bound output length

---

## Span Selection (`span.rs`)

Symbol/span-aware block expansion for `repo_fetch`:

- Extract specific line ranges
- Expand to enclosing symbols (functions, structs, etc.)
- Context-aware code extraction

---

## Error Handling

```rust
enum FetchError {
    Timeout,
    NetworkError(String),
    HttpError { status: u16 },
    ContentTooLarge,
    UnsupportedContentType,
    ExtractionFailed,
    BrowserError(String),
    PdfExtractionFailed,
}
```

---

## Security Considerations

- **SSRF prevention** — `validate_fetch_target()` blocks localhost/private IPs
- **Bounded reads** — never read unbounded response bodies
- **Redirect limits** — max redirect hops to prevent loops
- **Content-type validation** — only process expected content types
- **Circuit breakers** — prevent cascading failures

---

[← Back to Overview](overview.md)
