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
| `render/` | HTML structural rendering (blocks, text, markdown). Converts HTML to `RenderedBlock` list |
| `span.rs` | `SelectedSpan` — symbol/span-aware block expansion for `repo_fetch` |
| `types.rs` | `FetchError`, `FetchErrorKind` — error types |
| `pdf.rs` | PDF text extraction (feature-gated: `pdf`), uses `lopdf` |

---

## Security: SSRF Protection

The fetch client enforces strict network boundaries:

1. **DNS resolution** — Resolves hostname, rejects private IPs (10.x, 172.16-31.x, 192.168.x, localhost, IPv6 loopback)
2. **Redirect revalidation** — Each redirect is re-validated against SSRF rules
3. **Code-host rewriting** — GitHub/GitLab/Codeberg URLs are rewritten to raw content endpoints before fetching
4. **Size limits** — `max_bytes` caps response body size
5. **Timeout limits** — `max_timeout` caps request duration
6. **Link count limits** — Extraction caps at `MAX_LINKS = 100` per page

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
- HTML, PlainText, Markdown, Code, JSON, TOML, YAML
- PDF (feature-gated), CSV, XML
- Image (metadata only), Binary (rejected)
- RepositoryFile, RepositoryDirectory
- LocalFile, LocalDirectory

---

## HTML Extraction

`HtmlExtractor` uses the `scraper` crate to parse HTML and extract:

1. **Text content** — Visible text, stripped of scripts/styles
2. **Links** — Classified into 15+ kinds:
   - `Navigation`, `Reference`, `Anchor`, `Image`
   - `CodeHost` (GitHub/GitLab/Codeberg)
   - `Package` (npm, PyPI, crates.io)
   - `Documentation`, `Download`
   - And more
3. **Metadata** — Title, description, language
4. **Structure** — Block-based rendering with outline/chunks

### Block Rendering

HTML is converted to `RenderedBlock` list:
- Each block has a `BlockKind` (Paragraph, Heading, List, Code, Table, etc.)
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

1. Follow redirect chain (up to limit)
2. **Revalidate each redirect** against SSRF rules
3. Track redirect chain for trust metadata
4. Rewrite code-host URLs at each step

---

## Code-Host URL Rewriting

GitHub/GitLab/Codeberg browser URLs are rewritten to raw content URLs:

| Source | Rewritten To |
|--------|-------------|
| `github.com/owner/repo/blob/...` | `raw.githubusercontent.com/owner/repo/...` |
| `gitlab.com/owner/repo/-/blob/...` | `gitlab.com/owner/repo/-/raw/...` |
| `codeberg.org/owner/repo/src/...` | `codeberg.org/owner/repo/raw/...` |

---

## Fetch Limits

```rust
struct FetchLimits {
    max_bytes: usize,      // response body size cap
    max_timeout: Duration, // request timeout
    max_redirects: usize,  // redirect chain limit
    max_links: usize,      // extracted link count cap
}
```

All limits are bounded and configurable via `FetchSection` in config.

---

**Back to:** [overview.md](overview.md)
