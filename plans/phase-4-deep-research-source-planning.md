# Phase 4: Deep Research Source Planning and Evidence Grouping

## Objective

Add a bounded research-planning layer for codegg's deep research agent. The goal is to help an agent scope difficult architectural and technical questions by returning a diverse, source-grouped, provenance-preserving set of candidate sources and suggested fetch targets.

This phase should not make eggsearch a research agent. Eggsearch should not synthesize final answers, recursively browse, crawl arbitrary links, or summarize fetched pages. It should plan bounded subqueries, execute metasearch, classify and group sources, expose subquery provenance, and recommend a small set of explicit URLs for codegg to fetch with `web_fetch`.

The preferred implementation is a new MCP tool named `research_search`.

## Non-goals

Do not execute JavaScript.

Do not crawl linked pages.

Do not automatically fetch search results.

Do not synthesize conclusions, recommendations, or architectural decisions.

Do not generate long model-style research reports inside eggsearch.

Do not maintain persistent research state across calls.

Do not replace generic `web_search`. This phase adds a structured planning layer for cases where flat search is too weak.

## User-facing behavior

A codegg deep research agent should be able to ask:

```json
{
  "query": "compare QUIC vs WebSocket IPC for a coding agent daemon",
  "research_domain": "software_architecture",
  "desired_source_types": ["specifications", "official_docs", "reference_implementations", "benchmarks", "security_considerations", "design_discussions"],
  "include_counterpoints": true,
  "include_primary_sources": true,
  "include_recent_discussion": true,
  "freshness": "year",
  "max_results": 32,
  "max_groups": 10,
  "max_per_group": 5,
  "timeout_ms": 10000
}
```

The response should contain:

- The bounded subqueries eggsearch used.
- Grouped source candidates.
- Source-quality/evidence metadata.
- Provider failures and warnings.
- Suggested fetches ranked by likely information gain and source diversity.

The response should not contain a final answer to the research question.

## Proposed public types

Add core types, likely in:

- `src/core/research.rs`
- or `src/core/research_search.rs`

Export through `src/core/mod.rs`.

### `ResearchSearchRequest`

Recommended fields:

```rust
pub struct ResearchSearchRequest {
    pub query: String,
    pub research_domain: Option<ResearchDomain>,
    pub desired_source_types: Vec<ResearchSourceType>,
    pub include_counterpoints: Option<bool>,
    pub include_primary_sources: Option<bool>,
    pub include_recent_discussion: Option<bool>,
    pub include_security_considerations: Option<bool>,
    pub max_results: Option<usize>,
    pub max_groups: Option<usize>,
    pub max_per_group: Option<usize>,
    pub freshness: Freshness,
    pub timeout_ms: Option<u64>,
    pub providers: Vec<String>,
}
```

`research_domain` should be a hint, not a hard ontology. Recommended initial values:

```rust
pub enum ResearchDomain {
    General,
    SoftwareArchitecture,
    ApiDesign,
    DistributedSystems,
    Security,
    Performance,
    LanguageEcosystem,
    MachineLearning,
    Infrastructure,
}
```

`ResearchSourceType` should express the kind of evidence requested:

```rust
pub enum ResearchSourceType {
    PrimarySources,
    OfficialDocs,
    Specifications,
    ReferenceImplementations,
    DesignDiscussions,
    Benchmarks,
    SecurityConsiderations,
    IssueThreads,
    ReleaseNotes,
    AcademicOrFormalSources,
    RecentNews,
    CommunityDiscussion,
    Counterpoints,
}
```

Validation rules:

- `query` must be non-empty after trimming.
- `max_results`, `max_groups`, and `max_per_group` must be positive when present.
- Bound the number of desired source types. Suggested cap: 12.
- Bound total generated subqueries. Suggested cap: 8.

### `ResearchSubquery`

Expose the exact subqueries used so codegg can understand how the source set was built.

```rust
pub struct ResearchSubquery {
    pub id: String,
    pub source_type: ResearchSourceType,
    pub query: String,
    pub intent: SearchIntent,
    pub freshness: Freshness,
}
```

### `ResearchResultGroupKind`

Recommended groups:

```rust
pub enum ResearchResultGroupKind {
    PrimarySources,
    OfficialDocs,
    Specifications,
    ReferenceImplementations,
    DesignDiscussions,
    Benchmarks,
    SecurityConsiderations,
    IssueThreads,
    ReleaseNotes,
    AcademicOrFormalSources,
    RecentNews,
    CommunityDiscussion,
    Counterpoints,
    Unknown,
}
```

### `EvidenceQuality`

Add a lightweight evidence-quality classification. This is not a trust label for instruction-following. External content remains untrusted data. Evidence quality helps rank and group sources.

```rust
pub enum EvidenceQuality {
    OfficialPrimary,
    MaintainerPrimary,
    StandardsOrSpecification,
    VendorPrimary,
    PackageRegistry,
    AcademicOrFormal,
    BenchmarkOrMeasurement,
    SecurityAdvisory,
    CommunityDiscussion,
    NewsOrPress,
    BlogOrTutorial,
    Unknown,
}
```

If Phase 5 later introduces a unified source-quality taxonomy, this phase can use a local research-specific enum first, then refactor.

### `ResearchResultGroup`

```rust
pub struct ResearchResultGroup {
    pub kind: ResearchResultGroupKind,
    pub label: String,
    pub results: Vec<SourceCard>,
    pub truncated: bool,
}
```

### `ResearchSuggestedFetch`

```rust
pub struct ResearchSuggestedFetch {
    pub url: String,
    pub group: ResearchResultGroupKind,
    pub expected_kind: SourceKind,
    pub evidence_quality: EvidenceQuality,
    pub reason: String,
    pub recommended_extract_mode: Option<ExtractMode>,
    pub priority: u8,
}
```

Reason strings should be deterministic enum-like values, such as:

- `primary_source`
- `official_docs`
- `specification`
- `reference_implementation`
- `benchmark`
- `security_consideration`
- `active_design_discussion`
- `counterpoint`
- `diversity_source`

### `ResearchSearchResponse`

```rust
pub struct ResearchSearchResponse {
    pub query: String,
    pub mode: String,
    pub research_domain: ResearchDomain,
    pub subqueries: Vec<ResearchSubquery>,
    pub groups: Vec<ResearchResultGroup>,
    pub suggested_fetches: Vec<ResearchSuggestedFetch>,
    pub providers_queried: Vec<String>,
    pub providers_failed: Vec<ProviderFailure>,
    pub warnings: Vec<SearchWarning>,
    pub trust_markers: TrustMarkers,
}
```

## MCP tool surface

Add a new `research_search` MCP tool in `src/mcp/*`.

Tool behavior:

- Validate request.
- Generate bounded subqueries from requested source types and research domain.
- Execute search using existing provider fan-out where possible.
- Deduplicate and group results.
- Apply source diversity constraints.
- Return suggested fetches.
- Do not call `web_fetch` internally.
- Do not summarize or synthesize page contents.

The tool description should make clear that it is a source-planning tool for research agents, not an autonomous research worker.

## Planner design

Add a research planner module, likely:

- `src/meta/research_planner.rs`
- or `src/meta/research.rs`

The planner should turn a request into a bounded list of subqueries. It should be deterministic and transparent.

Recommended subquery templates:

### Primary sources

```
{query} official docs source repository maintainer
```

Use `intent = docs` or `intent = code` depending on domain/source type.

### Official docs

```
{query} official documentation API reference guide
```

Use `intent = docs`.

### Specifications

```
{query} specification RFC standard protocol spec
```

Use `intent = docs` or `web`.

### Reference implementations

```
{query} reference implementation github source code examples
```

Use `intent = code`.

### Design discussions

```
{query} design discussion proposal issue RFC discussion
```

Use `intent = issues` or `web`.

### Benchmarks

```
{query} benchmark performance latency throughput comparison
```

Use `intent = web`.

### Security considerations

```
{query} security considerations threat model vulnerability hardening
```

Use `intent = security`.

### Issue threads

```
{query} issue discussion bug regression pull request github gitlab
```

Use `intent = issues`.

### Release notes

```
{query} release notes changelog migration breaking changes
```

Use `intent = releases`.

### Academic/formal sources

```
{query} paper formal analysis arxiv conference proceedings
```

Use `intent = web`.

### Recent news

```
{query} recent update announcement news
```

Use `intent = news`.

### Community discussion

```
{query} discussion forum reddit stack overflow users experience
```

Use `intent = web`.

### Counterpoints

```
{query} drawbacks limitations tradeoffs criticism alternatives
```

Use `intent = web`.

Bounds:

- Generate only requested source types plus a small default set.
- Suggested default set: primary sources, official docs, reference implementations, design discussions, security considerations, counterpoints.
- Maximum subqueries: 8.
- If desired source types exceed cap, prefer primary/official/spec/security/reference/counterpoint categories.

## Execution strategy

Implementation options:

1. Construct internal `WebSearchRequest`s for each subquery and call existing adapter paths.
2. Refactor shared lower-level search fan-out to avoid repeated warning/provider accounting.

Prefer option 1 for minimal risk, unless it causes unacceptable duplication. Preserve total operation bounds.

Timeout behavior:

- One request-level timeout should bound the entire `research_search` operation.
- Avoid multiplying timeout by subquery count.
- If some subqueries time out, return partial groups and warnings.

Result bounds:

- Bound candidate results per subquery.
- Deduplicate globally by normalized URL.
- Enforce `max_per_group` and `max_results` after grouping/diversity ranking.

## Grouping rules

Group by deterministic source classification:

- `PrimarySources`: official project domains, standards bodies, maintainer repos, vendor docs.
- `OfficialDocs`: `SourceKind::OfficialDocs`, package docs, official docs domains.
- `Specifications`: RFCs, W3C/WHATWG/IETF/standards bodies, protocol specs, formal docs.
- `ReferenceImplementations`: source repository/source file/example implementation results.
- `DesignDiscussions`: issues, PRs, proposals, RFC discussions, design docs.
- `Benchmarks`: title/path/snippet/domain suggests benchmark/measurement/performance comparison.
- `SecurityConsiderations`: advisories, security docs, threat models, hardening guides.
- `IssueThreads`: issues/PRs not already classified as design discussions.
- `ReleaseNotes`: releases/changelogs/migration notes.
- `AcademicOrFormalSources`: arXiv, ACM/IEEE, conference pages, papers, formal reports.
- `RecentNews`: news source kind or news domains.
- `CommunityDiscussion`: forums, Stack Overflow, Reddit, discussion boards.
- `Counterpoints`: query expansion source type is counterpoints or title/snippet indicates drawbacks/limitations/tradeoffs.
- `Unknown`: fallback.

A result should have one primary group. Track original subquery/source type internally for ranking and optional debug metadata.

## Source diversity and deduplication

Research mode should prevent one domain from dominating.

Recommended algorithm:

1. Deduplicate by normalized URL.
2. Group candidates.
3. Within each group, sort by base score plus deterministic source-quality boosts.
4. Apply per-domain soft cap for suggested fetches. Suggested cap: max 2 per domain across `suggested_fetches`, unless the domain is clearly primary/official and no equivalent alternatives exist.
5. Preserve high-quality official sources while still including counterpoints and diverse implementations.

Do not remove all same-domain results from groups; only enforce stricter diversity in `suggested_fetches` and final ordering.

## Evidence-quality classification

Implement deterministic classification using:

- `SourceKind`.
- Domain priors.
- Code-host metadata.
- Issue/release metadata.
- URL path/title keywords.
- Provider ID.

Examples:

- IETF RFC: `StandardsOrSpecification`.
- docs.rs or official project docs: `OfficialPrimary` or `OfficialDocs` depending enum choice.
- GitHub source in maintainer repo: `MaintainerPrimary`.
- Crates.io/PyPI/npm: `PackageRegistry`.
- OSV/NVD/GHSA/RustSec/CISA: `SecurityAdvisory`.
- arXiv/ACM/IEEE: `AcademicOrFormal`.
- benchmark article/repo: `BenchmarkOrMeasurement`.
- Stack Overflow/Reddit/forum: `CommunityDiscussion`.
- blog/tutorial: `BlogOrTutorial`.

If classification is uncertain, use `Unknown` and avoid over-ranking it.

## Suggested fetches

Generate a small list of high-information sources. Suggested default: 8.

Priority order should consider:

1. Primary/official source for the core topic.
2. Specification or formal reference if present.
3. Reference implementation/source code.
4. Design discussion/proposal.
5. Benchmark/measurement source.
6. Security consideration source.
7. Counterpoint or limitations source.
8. Recent discussion/news if requested.

Each suggested fetch should include:

- URL.
- Group.
- Expected source kind.
- Evidence quality.
- Reason.
- Recommended extract mode.
- Priority.

Recommended extract modes:

- Official docs/spec/design docs: `markdown`.
- Source code: default/code rendering through `web_fetch` detection.
- PDFs: default if PDF feature is enabled; otherwise still suggest but warn if PDF extraction unavailable.
- Forums/news/blogs: `markdown`.

## Warning behavior

Add warnings for:

- `research_search_is_discovery_only`: optional if docs are clear; avoid noisy warning on every call unless needed.
- `subquery_cap_applied`: desired source types exceeded bounded query cap.
- `provider_lacks_requested_capability`: selected providers cannot enforce requested source type/freshness.
- `freshness_approximate`: freshness was requested but only some provider results have timestamps.
- `source_quality_heuristic`: evidence-quality classification is heuristic when not provider-native.
- `pdf_fetch_unavailable`: suggested fetch includes PDFs but build lacks PDF feature, if detectable.
- `partial_results`: one or more subqueries/providers failed or timed out.

Warnings should remain concise and stable.

## Tests

Core request tests:

- Empty query is rejected.
- Zero limits are rejected.
- Desired source type cap is enforced.
- Defaults include a useful source-type set.
- Explicit providers are preserved.

Planner tests:

- Each source type generates the expected subquery template.
- Query expansion is bounded.
- Counterpoints are generated only when requested or included in desired source types.
- Security subquery uses `SearchIntent::Security`.
- Reference implementation subquery uses `SearchIntent::Code`.
- Release notes subquery uses `SearchIntent::Releases`.

Grouping tests:

- RFC/spec URL groups as `Specifications`.
- Docs URL groups as `OfficialDocs`.
- GitHub source file/repo groups as `ReferenceImplementations` or `PrimarySources` depending rules.
- Issue/PR groups as `DesignDiscussions` or `IssueThreads`.
- Release/changelog groups as `ReleaseNotes`.
- Security advisory/hardening guide groups as `SecurityConsiderations`.
- Benchmark URL/title groups as `Benchmarks`.
- Forum/Q&A URL groups as `CommunityDiscussion`.
- Counterpoint subquery result groups as `Counterpoints` when not better classified.

Diversity tests:

- Suggested fetches do not all come from the same domain when alternatives exist.
- Official/primary sources are still retained even with domain cap pressure.
- Deduplication collapses equivalent URLs across subqueries.

MCP tests:

- `research_search` returns subqueries, groups, suggested fetches, provider status fields, warnings, and trust markers.
- Provider failure in one subquery still preserves successful groups.
- The tool does not invoke `web_fetch` internally.

Mocked workflow tests:

- `compare QUIC vs WebSocket IPC for a coding agent daemon` yields specs/docs/reference implementations/security/counterpoints groups.
- `distributed tracing architecture rust tokio tower` yields docs/source/design/benchmarks/community groups.

## Documentation

Update README with a `research_search` section.

Include:

- Purpose and non-goals.
- Example request.
- Example grouped response excerpt.
- Explanation that subqueries are transparent and bounded.
- Explanation that suggested fetches are not automatically fetched.
- Fallback guidance: use generic `web_search` when `research_search` is unavailable.

Update CHANGELOG under Unreleased.

## Implementation order

1. Add research core types and exports.
2. Add research planner with unit tests.
3. Add grouping/evidence-quality classifier with unit tests.
4. Add adapter orchestration for bounded subquery execution.
5. Add suggested-fetch generation and diversity constraints.
6. Add MCP `research_search` tool.
7. Add warning handling.
8. Add README/CHANGELOG documentation.
9. Run full tests and fix clippy/doc warnings.

## Validation commands

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test
```

Any live-provider tests must be opt-in and excluded from normal CI.

## Acceptance criteria

This phase is complete when:

- `research_search` exists as an explicit MCP tool or equivalent explicit source-planning interface.
- It returns transparent bounded subqueries.
- It groups source candidates by evidence type.
- It returns suggested fetches without fetching automatically.
- It applies deduplication and source-diversity constraints.
- It preserves provider failures, warnings, and trust markers.
- It does not synthesize final research answers.
- Generic `web_search` remains available and simple.
- Tests cover planning, grouping, diversity, warnings, and MCP response shape.
- Full local test suite passes.

## Handoff notes

This phase should make codegg's deep research agent more efficient without moving reasoning into eggsearch. Keep the boundary sharp: eggsearch plans and retrieves candidate sources; codegg fetches, reads, compares, and synthesizes. If the implementation starts needing hidden state, recursive crawling, or long prose synthesis, it is drifting out of scope.
