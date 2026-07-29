# Search Engines Deep Dive

**Path:** `src/meta/engines/`
**Purpose:** 38 vendored search engine implementations — the upstream data sources for the metasearch adapter.

---

## Engine Trait

All engines implement `SearchEngine`:

```rust
trait SearchEngine: Send + Sync {
    fn name(&self) -> &'static str;
    fn search(&self, query: &str, max_results: usize, timeout: Duration)
        -> BoxFuture<Result<Vec<SearchResult>, EngineError>>;
    fn supports_role(&self, role: &EvidenceRole) -> bool { true }
    fn advisory_capabilities(&self) -> AdvisoryCapabilities { Default::default() }
    fn lookup_advisory(&self, vuln_id: &str, timeout: Duration)
        -> BoxFuture<Result<Option<VulnerabilityMetadata>, EngineError>> { ... }
    fn query_advisories_by_package(&self, ecosystem: &str, package: &str, version: Option<&str>, max_results: usize, timeout: Duration)
        -> BoxFuture<Result<Vec<VulnerabilityMetadata>, EngineError>> { ... }
}
```

### Advisory Capabilities

OSV, GitHub Advisory, RustSec, and NVD override `advisory_capabilities()` to declare native `lookup_by_id` and/or `query_by_package` support. Generic engines retain the default (no native advisory capability).

### Role Support

`supports_role()` returns `true` by default (conservative: assume all roles are reachable via generic search). Override to return `false` for roles this engine provably cannot serve.

---

## Engine Categories

### HTML Scrapers (5)

| Engine | File | Notes |
|--------|------|-------|
| DuckDuckGo | `duckduckgo.rs` | Generic web search, no API key |
| Startpage | `startpage.rs` | Privacy-focused, no API key |
| Yahoo | `yahoo.rs` | Generic web search, no API key |
| Mojeek | `mojeek.rs` | Independent index, no API key |
| Brave (HTML) | `brave.rs` | HTML scraping fallback |

HTML scrapers report `ProviderCapabilities::none()` — conservative capability flags.

### JSON APIs (5)

| Engine | File | Notes |
|--------|------|-------|
| SearXNG | `searxng.rs` | Self-hosted metasearch, requires `base_url` |
| OSV | `osv.rs` | Advisory search + native `lookup_by_id` + `query_by_package` |
| NVD | `nvd.rs` | NIST advisory data, native `lookup_by_id` |
| CISA KEV | `cisa_kev.rs` | Known Exploited Vulnerabilities catalog |
| RustSec | `rustsec.rs` | Rust security advisories, native `lookup_by_id` |

### API-Key Providers (13)

| Engine | File | Notes |
|--------|------|-------|
| Brave API | `brave_api.rs` | Brave Search API |
| GitHub Code | `github_code.rs` | GitHub code search |
| GitHub Issues | `github_issues.rs` | GitHub issue search |
| GitHub Releases | `github_releases.rs` | GitHub release search |
| GitLab Code | `gitlab_code.rs` | GitLab code search |
| GitLab Issues | `gitlab_issues.rs` | GitLab issue search |
| GitLab Releases | `gitlab_releases.rs` | GitLab release search |
| Gitea Code | `gitea_code.rs` | Gitea/Forgejo code search (requires `base_url`) |
| Gitea Issues | `gitea_issues.rs` | Gitea/Forgejo issue search |
| Gitea Releases | `gitea_releases.rs` | Gitea/Forgejo release search |
| GitHub Advisory | `github_advisory.rs` | GitHub Security Advisories, native `lookup_by_id` + `query_by_package` |
| Semantic Scholar | `semantic_scholar.rs` | Academic paper search |
| Sourcegraph | `sourcegraph.rs` | Code search |

### Package Registries (8)

| Engine | File | Notes |
|--------|------|-------|
| crates.io | `crates_io.rs` | Rust packages |
| PyPI | `pypi.rs` | Python packages |
| npm | `npm_registry.rs` | JavaScript packages |
| Go Proxy | `go_pkg.rs` | Go packages |
| Maven Central | `maven_central.rs` | Java packages |
| NuGet | `nuget.rs` | .NET packages |
| RubyGems | `rubygems.rs` | Ruby packages |
| Packagist | `packagist.rs` | PHP packages |

### Scholarly (2)

| Engine | File | Notes |
|--------|------|-------|
| OpenAlex | `openalex.rs` | Academic metadata |
| Crossref | `crossref.rs` | DOI-based scholarly search |

---

## Shared Infrastructure

### Models (`models.rs`)

- `SearchResult`: title, url, snippet, source_engine, metadata
- `ResultMetadata`: enum — `None`, `Issue(IssueMetadata)`, `Release(ReleaseMetadata)`, `Advisory(VulnerabilityMetadata)`, `CodeSearch(CodeSearchMetadata)`
- `ResultMetadata::merge()`: idempotent, order-independent — structured variant always wins over `None`; same-variant merges keep the richer row
- `AggregatedResult`: title, url, snippet, engines (multi-provider), score, metadata

### Error (`error.rs`)

`EngineError` enum: `Timeout`, `Http`, `BadStatus`, `ParseFailed`, `NetworkError` — all carry `engine: &'static str`.

### Normalizer (`normalizer.rs`)

URL normalization for deduplication:
- Strips tracking params (`utm_*`, `fbclid`, `gclid`, `ref`, `source`)
- Strips fragments
- Strips locale prefixes (`/en/`, `/en-US/`)
- Strips index files (`index.html`, `index.htm`, `index.php`)
- Sorts query parameters
- Lowercases scheme and host
- Preserves non-default ports

### Bounded Body Reading

`read_bounded_body()` streams HTTP responses with a hard byte cap. Checks `Content-Length` upfront, streams at most `max_bytes + 1` bytes, aborts on overflow.

### HTTP Client

`build_http_client()` creates a `reqwest::Client` with gzip/brotli, configurable user agent (default: browser-like UA for HTML scrapers), no cookie store (MCP server should not persist cookies across sessions).

---

## Engine Error Classes

| Error | Mapping | Cooldown |
|-------|---------|----------|
| `Timeout` | `FailureClass::Timeout` | 15s |
| `BadStatus(429)` | `FailureClass::RateLimited` | 60s |
| `BadStatus(5xx)` | `FailureClass::HttpStatus` | 30s |
| `Http` | `FailureClass::NetworkError` | 30s |
| `ParseFailed` | `FailureClass::ParseError` | 30s |
| `NetworkError` | `FailureClass::NetworkError` | 30s |
| Panic (via `catch_unwind`) | `FailureClass::Panic` | 30s |

---

## Adding a New Engine

1. Create `src/meta/engines/<name>.rs` implementing `SearchEngine`
2. Add struct definition and `impl SearchEngine` in `mod.rs`
3. Add provider ID to `KNOWN_PROVIDER_IDS` in `src/core/provider.rs`
4. Add `ProviderKind` and `ProviderCapabilities` in `src/core/provider.rs`
5. Wire into `MetadataSearchAdapter::build_engines()` in `src/meta/adapter.rs`

---

**Back to:** [overview.md](overview.md)
