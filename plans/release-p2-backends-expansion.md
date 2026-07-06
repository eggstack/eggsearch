# P2 Plan: Search Backend, Security Source, Registry, and Research Expansion

Status: handoff plan
Priority: P2 post-release roadmap
Area: provider expansion for coding agents, security search, and deep research

## Context

The current provider set is sufficient for first release: generic HTML metasearch, SearXNG, Brave API, GitHub/GitLab/Gitea code-host providers, OSV, and local workspace search. The next capability jump should come from structured backends that reduce reliance on generic web scraping for specialized workflows.

This plan scopes post-release backend additions. It is intentionally not a release blocker.

## Goals

1. Improve coding-agent search precision with native code/search backends.
2. Improve security search with first-class advisory and exploit-status sources.
3. Improve package-aware repo/security workflows with registry metadata providers.
4. Improve research workflows with scholarly/open-data APIs.
5. Preserve the unified provider abstraction and capability model.
6. Avoid hard-coding separate MCP tools per backend.

## Non-Goals

- Do not add backend-specific MCP tools unless the unified abstraction cannot represent the source.
- Do not require paid APIs for default install.
- Do not make external credentials mandatory.
- Do not introduce browser execution or crawling.
- Do not make live network tests mandatory in CI.

## Phase 1: Backend Capability Taxonomy

Before adding many providers, tighten backend classification.

### Tasks

1. Extend or audit `ProviderCapabilities` for these dimensions:
   - package metadata lookup
   - advisory lookup by id
   - advisory lookup by package/version
   - exploit/KEV status
   - scholarly paper search
   - DOI lookup
   - code symbol search
   - repository indexing backend
   - structured changelog/release metadata

2. Ensure capabilities distinguish provider-side filtering from client-side reranking:
   - native freshness filters
   - result timestamps
   - native language filters
   - native ecosystem filters

3. Update `provider_status` to expose new capabilities consistently.

4. Add tests requiring every `KNOWN_PROVIDER_IDS` entry to have an explicit descriptor and capability summary.

### Acceptance Criteria

- New provider additions can declare capabilities without ad hoc warning logic.
- Capability warnings remain accurate for `repo_search`, `security_search`, and `research_search`.

## Phase 2: Security Backends

### Priority Sources

1. GitHub Security Advisory / GHSA
2. NVD CVE API
3. CISA Known Exploited Vulnerabilities catalog
4. RustSec advisory database or API-compatible mirror
5. osv.dev is already present; keep it as a core source

### Provider IDs

Suggested ids:

- `github_advisory`
- `nvd`
- `cisa_kev`
- `rustsec`

### Integration Model

Security providers should plug into existing advisory methods on `SearchEngine` or a split advisory trait if the trait becomes too broad.

Current methods:

- `lookup_advisory(vuln_id, timeout)`
- `query_advisories_by_package(ecosystem, package, version, max_results, timeout)`

If the trait is becoming overloaded, introduce:

```rust
pub trait AdvisoryProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn lookup_advisory<'a>(&'a self, vuln_id: &'a str, timeout: Duration) -> BoxFuture<'a, Result<Option<VulnerabilityMetadata>, EngineError>>;
    fn query_by_package<'a>(&'a self, ecosystem: &'a str, package: &'a str, version: Option<&'a str>, max_results: usize, timeout: Duration) -> BoxFuture<'a, Result<Vec<VulnerabilityMetadata>, EngineError>>;
}
```

Keep adapter composition simple; do not split unless it reduces complexity.

### NVD Notes

- Support API key env var optionally.
- Use unauthenticated mode if acceptable, but rate limits should be documented.
- Normalize CVSS, CWE, CPE, references, published/modified timestamps.
- Treat NVD severity as advisory metadata, not exploitability proof.

### CISA KEV Notes

- KEV is not a vulnerability database; it is an exploited-in-the-wild signal.
- Model as enrichment keyed by CVE.
- Include due date and known ransomware campaign flag if available.
- Keep source provenance explicit.

### GHSA Notes

- GitHub advisory data can complement OSV for GitHub ecosystem metadata.
- Token may be required depending on API usage.
- Normalize GHSA IDs and CVE aliases.

### Tests

Use fixture JSON responses with httpmock. Do not require live API calls in CI.

Required tests:

- CVE lookup by id.
- GHSA lookup by id.
- Package/version query with affected/not affected applicability.
- KEV enrichment by CVE.
- Rate-limit or HTTP error classification.
- Provider capability descriptors.

## Phase 3: Registry Metadata Backends

### Priority Registries

1. crates.io
2. PyPI
3. npm
4. Go package index / proxy metadata
5. Maven Central
6. NuGet
7. RubyGems
8. Packagist
9. OCI registries, later and carefully
10. GitHub Actions marketplace metadata, later

### Provider IDs

Suggested ids:

- `crates_io`
- `pypi`
- `npm_registry`
- `go_pkg`
- `maven_central`
- `nuget`
- `rubygems`
- `packagist`

### Use Cases

- Resolve package homepage/repository/docs/changelog links.
- Resolve latest version and release timestamps.
- Resolve dependency metadata when available.
- Improve `repo_search` package-aware subqueries.
- Improve `security_search` package/version normalization.
- Improve migration-guide and changelog discovery.

### Integration Model

Consider a registry-specific trait rather than forcing registries into generic `SearchEngine` result pages.

Suggested trait:

```rust
pub trait PackageRegistryProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn ecosystem(&self) -> &'static str;
    fn lookup_package<'a>(&'a self, package: &'a PackageCoordinate, timeout: Duration) -> BoxFuture<'a, Result<Option<PackageMetadata>, EngineError>>;
}
```

Return normalized metadata, not raw registry response:

- package name
- normalized ecosystem
- latest version
- requested version
- repository URL
- homepage URL
- docs URL
- changelog URL
- license
- release timestamps when available
- deprecation/yanked status when available

### Tests

Use fixtures for each registry. For each provider:

- parse package metadata
- extract repository URL
- extract docs/homepage when present
- handle missing package
- handle yanked/deprecated flags when present
- enforce timeout/error classification

## Phase 4: Research Backends

### Priority Sources

1. OpenAlex
2. Crossref
3. Semantic Scholar
4. arXiv
5. PubMed / NCBI E-utilities, optional if biomedical research becomes important

### Provider IDs

Suggested ids:

- `openalex`
- `crossref`
- `semantic_scholar`
- `arxiv`
- `pubmed`

### Use Cases

- Research mode source diversification.
- DOI/title lookups.
- Paper metadata discovery.
- Citation/context clues.
- Architecture and systems research evidence.

### Integration Model

Research providers can be implemented as `SearchEngine` providers returning `SourceCard`-compatible results with structured metadata.

Add metadata fields if needed:

- DOI
- authors
- venue
- publication year/date
- abstract/snippet
- open access URL
- PDF URL, if safe and direct
- citation count, if available
- source database

### Tests

Use fixture JSON and no live API in CI.

Required tests:

- Query returns title/URL/snippet.
- DOI metadata normalized.
- Publication year/date extracted.
- Open access link is handled as a suggested fetch, not auto-fetched.
- Provider failures produce normal warning/failure telemetry.

## Phase 5: Code Search Backends

### Priority Backends

1. Sourcegraph API
2. Zoekt-compatible search endpoint
3. GitHub code search is already present; improve query mapping as needed
4. GitLab/Gitea improvements as needed

### Provider IDs

Suggested ids:

- `sourcegraph_code`
- `zoekt_code`

### Use Cases

- Large private monorepo search.
- Organization-wide symbol/path/language search.
- Faster code search than generic web fallback.
- Better codegg deep-research and API-understanding workflows.

### Sourcegraph Notes

- Token env var likely required.
- Base URL should be configurable.
- Support query mapping from repo/org/path/language/symbol hints.
- Preserve line ranges and repository metadata.

### Zoekt Notes

- Keep base URL configurable.
- Confirm query syntax and response format before implementation.
- Treat as optional private backend.

### Tests

- Query construction from repo hints.
- Parse file/path/line result metadata.
- Capability descriptor reports code/path/language support.
- Failure behavior and timeouts.

## Phase 6: Ranking and Evidence Quality

After adding structured backends, update ranking so native structured sources are preferred in specialized workflows.

### Tasks

1. Boost native advisory sources in `security_search`.
2. Boost registry metadata for package-coordinate queries.
3. Boost local workspace and native code-host results in `repo_search` when repo hints match.
4. Penalize generic web fallback when native providers answer the same group.
5. Keep source diversity for `research_search`; do not over-collapse to one API.

### Tests

- Native code result outranks generic search result for exact repo/path query.
- OSV/GHSA/NVD advisory result outranks blog result for CVE query.
- Registry package metadata appears in package-aware result groups.
- Research mode still returns diverse domains/providers.

## Phase 7: Documentation and Config Presets

Add provider setup docs for each new backend family.

For every provider, document:

- provider id
- provider kind
- required config
- required env var
- optional base URL
- API/rate-limit caveats
- supported capabilities
- example query/tool workflow

Update:

- `docs/provider-setup.md`
- `docs/tool-matrix.md`
- `docs/agent-workflows.md`
- `docs/config.md`
- README provider summary

## Global Acceptance Criteria

Each new backend is complete only when:

- It has a provider descriptor.
- It appears in `KNOWN_PROVIDER_IDS` if stable.
- It has config validation and provider-status diagnostics.
- It has fixture-based tests.
- It does not require live network in CI.
- It has docs with config examples.
- It emits clear warnings on unsupported query features.
- It participates in relevant profile defaults only when configured/routable.

## Suggested Implementation Order

1. Capability taxonomy update.
2. CISA KEV enrichment, because it is simple and high-value for defensive search.
3. NVD provider.
4. GHSA provider.
5. crates.io and PyPI registry metadata.
6. npm registry metadata.
7. OpenAlex or Crossref research provider.
8. Sourcegraph code backend.
9. Remaining registries.
10. Semantic Scholar/arXiv/PubMed as needed.

## Risk Notes

Avoid adding too many providers in one commit. Each provider should be independently testable, documented, and diagnosable through `provider_status`.

Treat API terms/rate limits carefully. Do not enable paid/API-key providers by default. Keep the default install functional with no credentials.
