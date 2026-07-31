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
- `browser_rendering`: unavailable

---

**Back to:** [overview.md](overview.md)
