# fetch Module Deep Dive

**Path:** `src/fetch/`
**Purpose:** HTTP fetch client, HTML content extraction, PDF extraction, span selection, SSRF protection.

The fetch module handles all outbound HTTP requests for `web_fetch`, `repo_fetch`, and `batch_fetch` tools.

---

## Submodule Inventory

| File | Responsibility |
|------|----------------|
| `client.rs` | `FetchClient` — reqwest-based HTTP client. Redirect revalidation, SSRF/localhost/private-network validation, code-host URL rewriting, content detection, extraction pipeline |
| `extract.rs` | `HtmlExtractor` — HTML content extraction via `scraper` crate. Link extraction with 15+ link kinds, bounded to `MAX_LINKS = 100` |
| `detect.rs` | Content-type detection, markdown/code/plain-text detection |
| `limits.rs` | `FetchLimits` struct, URL validation, DNS validation, private-network checks |
| `render/` | HTML structural rendering (7 submodules: blocks, code, csv, markdown, markdown_source, notebook, text). Converts HTML/code/diff/CSV/notebook content to `RenderedBlock` list |
| `span.rs` | `SelectedSpan` — symbol/span-aware block expansion for `repo_fetch` |
| `types.rs` | `FetchError`, `FetchErrorKind` — error types |
| `pdf.rs` | PDF text extraction (feature-gated: `pdf`), uses `lopdf`. Page selection, quality classification, document metadata, outline extraction |

---

## Security: SSRF Protection

The fetch client enforces strict network boundaries:

1. **DNS resolution** — Resolves hostname, rejects private/reserved IPs (RFC 1918, RFC 6890, loopback, link-local, multicast, documentation ranges, and IPv6 equivalents; full list in [safety.md](../safety.md))
2. **Redirect revalidation** — Each redirect is re-validated against SSRF rules
3. **Code-host rewriting** — GitHub/GitLab/Codeberg/Gitea/Forgejo URLs are rewritten to raw content endpoints before fetching
4. **Embedded credentials** — URLs with `user:pass@` are rejected
5. **Size limits** — `max_bytes` caps response body size
6. **Timeout limits** — `max_timeout` caps request duration

---

## Content Detection Pipeline

```
HTTP Response
  ├── Content-Type header check
  ├── URL extension check (.pdf, .md, .json, etc.)
  ├── Body sniffing (first bytes)
  └── Classification:
       ├── HTML → HtmlExtractor → RenderedBlock list
       ├── PDF → PdfExtractor (feature-gated)
       ├── Markdown → pass-through
       ├── Code → language detection
       └── Plain text → pass-through
```

### Document Kinds (16)

`FetchDocument` supports these document kinds:
- Html, PlainText, Markdown, Code, Json, Toml, Yaml
- Diff, Patch, Pdf (feature-gated), Notebook
- Csv, Xml, Rst, AsciiDoc
- Unknown

---

## HTML Extraction

`HtmlExtractor` uses the `scraper` crate to parse HTML and extract:

1. **Text content** — Visible text, stripped of scripts/styles
2. **Links** — Classified into 15 kinds:
   - `SamePageAnchor`, `SameDomain`, `External`, `Download`
   - `SourceCode` (GitHub/GitLab/Codeberg/Gitea)
   - `Documentation`, `ApiReference`, `Issue`, `PullRequest`
   - `Release`, `SecurityAdvisory`, `Pdf`, `Image`
   - `Feed`, `Other`
3. **Metadata** — Title, description, language
4. **Structure** — Block-based rendering with outline/chunks

### Block Rendering

HTML is converted to `RenderedBlock` list:
- Each block has a `BlockKind` (Paragraph, Heading, ListItem, Code, Table, etc.)
- Blocks are chunked for bounded output
- Outline entries provide navigation structure

---

## Span Selection

For `repo_fetch`, the fetch module supports **span-aware extraction**:

1. Parse line ranges from request
2. Expand to include enclosing symbols (functions, classes, modules)
3. Add context lines before/after
4. Select relevant blocks from the rendered document
5. Return focused content with code evidence metadata

---

## Redirect Handling

The fetch client handles redirects specially:

1. Code-host URL rewriting is applied once before the redirect loop
2. Follow redirect chain (up to limit)
3. **Revalidate each redirect** against SSRF rules
4. Track redirect chain for trust metadata

---

## Code-Host URL Rewriting

GitHub/GitLab/Codeberg/Gitea/Forgejo browser URLs are rewritten to raw content URLs:

| Source | Rewritten To |
|--------|-------------|
| `github.com/owner/repo/blob/...` | `raw.githubusercontent.com/owner/repo/...` |
| `gitlab.com/owner/repo/-/blob/...` | `gitlab.com/owner/repo/-/raw/...` |
| `codeberg.org/owner/repo/src/...` | `codeberg.org/owner/repo/raw/branch/...` |
| Gitea/Forgejo `/src/...` | Rewritten to raw endpoint (requires configured base URL) |

---

## Fetch Limits

```rust
struct FetchLimits {
    max_bytes: usize,       // response body size cap (default 2MB)
    max_chars_default: usize, // fallback char bound (default 12000)
    max_chars_cap: usize,   // hard upper bound on max_chars (default 50000)
    max_url_len: usize,     // URL length cap (default 8192)
    timeout_ms: u64,        // request timeout (default 8000ms)
    redirect_limit: usize,  // redirect chain limit (default 5)
    allow_private_network: bool, // allow RFC 1918 etc. (default false)
    allow_localhost: bool,  // allow loopback addresses (default false)
    pdf_enabled: bool,      // PDF extraction toggle
    pdf_max_pages: usize,   // max PDF pages (default 25)
    pdf_max_chars_per_page: usize, // per-page char cap (default 12000)
    pdf_max_total_chars: usize,    // total PDF char cap (default 50000)
}
```

All limits are bounded and configurable via `FetchSection` in config.

### text vs raw_text

- **`text`** — Framed (Tier 2), sanitized (Tier 3), bounded by `max_chars` (request clamped to `max_chars_cap`). This is the public field serialized in MCP output.
- **`raw_text`** — Tier-1 only (strip + bound at `max_chars_cap`). Internal use only (e.g., `repo_fetch` line/span selection). Not serialized in MCP output. Metadata fields `raw_text_chars_returned`, `raw_text_truncated`, and `raw_text_cap` track its bounds internally.

### Sanitization tiers

| Tier | What | When | Scope |
|------|------|------|-------|
| Tier 1 | Strip control chars + bound text | Always | title, description, body text, all blocks, outline titles |
| Tier 2 | `<<<EXTERNAL_UNTRUSTED>>>` framing | `sanitize_output = true` | title, description, body text |
| Tier 3 | Injection marker scan (7 patterns) | `sanitize_output = true` | title, description, body text |

---

## PDF Extraction

PDF extraction is feature-gated (`pdf` Cargo feature) and disabled by default. When enabled, it must also be activated via `[fetch].pdf_enabled = true` in config.

### Page Selection

The `web_fetch` tool accepts optional `pdf.pages` for page selection:

- **Syntax:** `"1"`, `"1,3,5"`, `"1-5"`, `"1,3,7-10"`
- **Indexing:** One-indexed (page 1 is the first page)
- **Ranges:** Reversed ranges are normalized (e.g., `5-3` becomes `3-5`)
- **Validation:** Out-of-range pages, page 0, and malformed input produce errors
- **Deduplication:** Duplicate pages are deduplicated; output is ascending document order
- **Cap:** Selected page count respects `pdf_max_pages`

### Quality Classification

Each extracted page receives a quality classification:

| Kind | Meaning |
|------|---------|
| `clean_text` | Page has readable, mostly clean Unicode text |
| `sparse_text` | Page has some text but it appears sparse or low-quality |
| `cid_corrupt` | Page text contains significant `(cid:NN)` tokens (CID-font corruption) |
| `scanned_or_image_only` | Page appears scanned or image-only with little extractable text |
| `blank` | Page has no extractable text and no image evidence |
| `extraction_failed` | Text extraction failed for this page |

Quality is advisory. A document-level `quality_score` in `[0.0, 1.0]` is computed as a page-weighted average. `content_ok` is `false` when all selected pages are unusable.

### Document Metadata

PDF extraction reads the Info dictionary for: title, author, subject, keywords, creator, producer, creation date, and modification date. Each field is bounded and control-character stripped.

### Outline/Bookmark Extraction

Document outlines (bookmarks) are extracted from the catalog outline tree when present. Extraction is bounded to 200 entries with a maximum nesting depth of 6. Malformed individual entries are skipped rather than failing the entire PDF.

### OCR Policy

The `pdf.pdf_ocr` field accepts `"never"` (default), `"auto"`, or `"always"`. Values other than `"never"` return a capability warning until OCR support is implemented in a future phase.

### Capability Reporting

`provider_status` reports:
- `pdf_text`: available/unavailable (matches `cfg!(feature = "pdf")`)
- `pdf_layout`: unavailable
- `pdf_ocr`: unavailable
- `browser_rendering`: available when `browser` feature is compiled, enabled in config, and a Chrome/Chromium executable is discovered

---

## Browser Rendering (Phase 4, Optional)

Browser rendering is an optional feature gated behind the `browser` Cargo feature. When enabled and configured, `web_fetch` can escalate from HTTP to headless Chrome/Chromium for pages that ordinary HTTP fetching cannot usefully render (e.g., JavaScript-heavy single-page apps).

### Submodule Inventory

| File | Responsibility |
|------|----------------|
| `browser/types.rs` | `RenderPolicy`, `FetchDisposition`, `TransportResponse`, `BrowserConfig`, `BrowserDiscovery` |
| `browser/discover.rs` | Browser executable discovery and validation (Linux/macOS candidates) |
| `browser/lifecycle.rs` | `BrowserLifecycle` — warm browser process management with one-process-per-server model |
| `browser/classify.rs` | `FetchDisposition` classification: useful content, JS shell, interactive challenge, non-interactive verification |
| `browser/intercept.rs` | Request URL policy checks — blocks localhost, private networks, embedded credentials |
| `browser/navigate.rs` | `browser_fetch` — navigation, DOM readiness heuristics, DOM extraction, challenge detection |

### Render Policy

```rust
pub enum RenderPolicy {
    HttpOnly,  // never launch Chrome (default)
    Auto,      // attempt HTTP first, escalate once for approved classifications
    Browser,   // use Chrome directly after validating target and origin gate
}
```

### Escalation Rules

Under `Auto`, escalation occurs only when:
1. HTTP response is successful or a recognized non-interactive verification response
2. Body is HTML
3. Classification is `JavascriptShell` or `NonInteractiveVerification`
4. Browser capability is available
5. Origin circuit permits the attempt
6. Logical request deadline has sufficient remaining time

At most one browser attempt follows the HTTP attempt. Browser failure returns a structured result; it does not loop back to HTTP.

### Browser Discovery

Discovery checks these candidates in order:
- Configured executable path (from `[fetch].browser.executable`)
- Linux: `/usr/bin/google-chrome-stable`, `/usr/bin/google-chrome`, `/usr/bin/chromium`, `/usr/bin/chromium-browser`, `/snap/bin/chromium`
- macOS: `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`, `~/Applications/...`, `/Applications/Chromium.app/...`
- PATH resolution: `google-chrome-stable`, `google-chrome`, `chromium`, `chromium-browser`

Discovered executables are validated with `--version` under a short timeout.

### Network Policy

Browser transport rejects:
- localhost and private IPv4/IPv6 addresses
- link-local, cloud metadata ranges
- non-HTTP(S) schemes
- embedded credentials

All observable requests must be intercepted and checked. Unsupported or uninspectable request behavior fails closed.

### Challenge Detection

| Classification | Indicators |
|---------------|------------|
| `InteractiveChallenge` | Turnstile/CAPTCHA iframe markers, "verify you are human", access denied titles |
| `NonInteractiveVerification` | "just a moment", "checking your browser", "please wait" |
| `JavascriptShell` | Empty root/app/next div with multiple scripts, low text content |

Interactive challenges return `ManualInteractionRequired` and are never solved. Non-interactive verifications get a bounded resolution window.

### Configuration

```toml
[fetch.browser]
enabled = false
policy = "http_only"       # http_only | auto | browser
executable = ""            # optional explicit path
startup_timeout_ms = 10000
navigation_timeout_ms = 20000
post_load_wait_ms = 1500
verification_wait_ms = 10000
max_requests = 100
max_dom_bytes = 4000000
global_concurrency = 1
per_origin_concurrency = 1
block_media = true
```

### What Browser Rendering Does NOT Do

- Download or install a browser
- Solve CAPTCHAs or click Turnstile controls
- Use the user's ordinary Chrome profile
- Rotate proxies or synthesize fingerprints
- Persist browser state across requests (unless using persistent profiles)
- Return screenshots in MCP responses
- Crawl recursively

## Persistent Browser Profiles (Phase 5, Optional)

Persistent browser profiles allow a local operator to establish a dedicated browser session for an origin requiring authentication or interactive verification.

### Profile Lifecycle

1. **Create:** CLI `browser-login` opens a headed Chrome at the specified origin. The operator logs in, completes MFA/CAPTCHAs, and closes the browser.
2. **Reuse:** MCP `web_fetch` with `browser_profile` uses the profile's Chrome data directory for headless rendering, preserving cookies and storage.
3. **Expiry:** If a profile-scoped fetch returns a login form or challenge, `browser_profile_requires_attention` is returned with the CLI command to re-establish the session.
4. **Remove:** CLI `browser-profiles remove` deletes the profile directory and invalidates its cache scope.

### Profile Metadata Model

```
$XDG_DATA_HOME/eggsearch/browser-profiles/
    <opaque-id>/
        profile.toml          # BrowserProfileMetadata
        chrome-data/          # Chrome's user data directory
```

`profile.toml` contains only non-secret metadata (display name, allowed origin, timestamps, browser version). Chrome owns all cookie/storage data under `chrome-data/`.

### Profile Constraints

- Profiles are disabled by default; explicit operator enablement required
- MCP callers cannot create, remove, or list profiles
- Each profile is restricted to its recorded exact origin
- One profile cannot be used concurrently by headed login and headless fetch (file lock)
- Opaque directory IDs prevent cache access by recreated profiles of the same display name
- Profile directories use owner-only permissions on Unix
- Symlinked profile directories are rejected

### Cache Partitioning

Profile-scoped fetches use `CacheScope::Profile(opaque_id)` which partitions both raw and derived cache tiers. Anonymous and profile cache entries never mix.

### Configuration

```toml
[fetch.browser.persistent_profiles]
enabled = false
profiles_dir = ""           # empty = platform default
allowed_profiles = []       # empty = all allowed
profile_process_timeout_ms = 30000
```

---

## Fetch Resilience

The fetch module includes resilience features for respectful and efficient HTTP fetching.

### Per-Origin Concurrency Control

Each origin (scheme + host + port) has a semaphore limiting concurrent in-flight requests. Default: 2 concurrent requests per origin. This prevents hammering a single server with parallel requests.

### Retry and Backoff

Automatic retries apply to narrow failure classes:
- **429 (Too Many Requests)** — after honoring `Retry-After` header
- **502/503/504** — server errors indicating transient issues
- **Network errors** — connection reset, DNS failures, broken pipe, EOF

Non-retryable failures (400, 401, 403, 404, etc.) are returned immediately without retry.

Retry uses bounded exponential backoff with jitter:
- Base delay: 250ms
- Max delay: 4s
- Jitter: random(0..cap)
- Max attempts: 2 (configurable)

Backoff sleep respects the remaining request deadline — it never sleeps past the configured timeout.

### Circuit Breaker

After 3 consecutive retryable failures for an origin, the circuit opens for 30-120 seconds. During this period:
- Requests fail fast with `origin_circuit_open` error
- Cache hits can still be served
- A successful request resets the circuit

### HTTP Conditional Revalidation

When a cached response has `ETag` or `Last-Modified` validators and becomes stale:
1. Conditional headers (`If-None-Match` or `If-Modified-Since`) are sent with the request
2. If the server responds with 304 (Not Modified), the cached body is reused with refreshed metadata
3. If the server responds with 200, the cache entry is fully updated

### Cache System

Two-tier in-memory LRU cache:

| Cache | Purpose | Key |
|-------|---------|-----|
| **Raw** | Response bytes + headers | URL + cache scope |
| **Derived** | Extracted content | Raw content hash + extraction params |

Cache scope prevents mixing anonymous and authenticated/profile content.

Cache directives honored:
- `no-store` — never cache
- `max-age` — freshness window
- `Expires` — fallback freshness
- `ETag` / `Last-Modified` — conditional revalidation
- `private` — not cached in anonymous scope; only cached in profile scope
- `Vary` — responses with unsupported Vary headers (anything other than `Accept-Encoding`) are not cached
- `no-cache` — forces revalidation on every use

When neither `max-age` nor `Expires` is present, a configurable `default_ttl_seconds` (default 900) is applied.

Challenge/error pages (403, 429, CAPTCHA, etc.) are never stored as successful content.

### Cache Key Separation

Derived cache keys include extraction parameters, so the same PDF with different page selections produces separate cache entries. This allows re-extracting a different page range from cached raw bytes without re-downloading.

### Configuration

```toml
[fetch]
retry_max_attempts = 2
retry_base_delay_ms = 250
retry_max_delay_ms = 4000
origin_http_concurrency = 2
origin_browser_concurrency = 1
origin_circuit_failure_threshold = 3
origin_circuit_duration_ms = 60000

[fetch.cache]
enabled = true
memory_max_entries = 256
memory_max_bytes = 67108864
derived_max_entries = 512
default_ttl_seconds = 900
```

### Non-Goals

The fetch resilience layer does not implement:
- Proxy rotation or IP rotation
- Automatic user-agent rotation
- Global crawl delay enforcement from robots.txt
- Distributed rate limiting
- Background prefetching or stale-while-revalidate workers

---

**Back to:** [overview.md](overview.md)
