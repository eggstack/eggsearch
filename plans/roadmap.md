# eggsearch Codegg Optimization Roadmap

## Purpose

This roadmap defines the next major direction for eggsearch as a general-purpose MCP metasearch and fetch server that also exposes higher-value retrieval paths for codegg. The goal is not to replace generic web search. Generic `web_search`, explicit single-URL `web_fetch`, and `provider_status` remain the stable baseline because they are useful for broad agent workflows, lightweight research, and fallback discovery.

The codegg-specific goal is to optimize three recurring retrieval workloads without turning eggsearch into an autonomous crawler, browser, summarizer, or long-running research agent:

1. Code/API/repository understanding: search and fetch enough structured evidence for an agent to understand a library, API, project, repository, source file, symbol, changelog, issue, or migration path.
2. Security/CVE and defensive-programming research: retrieve authoritative vulnerability facts, advisory records, package/ecosystem impact, exploitability signals, affected/patched ranges, and defensive guidance.
3. Deep architectural research: help a research agent scope hard technical questions through bounded, provenance-preserving, source-grouped retrieval plans and evidence bundles.

The implementation should preserve the existing safety model: search is discovery-only, fetch is explicit and bounded, JavaScript is not executed, external content is framed/sanitized as untrusted data, provider failures are reported, and partial results are preserved where possible.

## Design principles

### Keep generic search first-class

The generic `web_search` path should remain useful for arbitrary questions. New specialized workflows should be additive. They may be exposed as new MCP tools, optional request fields, provider groups, or typed response metadata, but they should not degrade the simple query-to-source-card behavior that makes eggsearch useful as a general tool.

### Prefer typed retrieval over implicit autonomy

The specialized paths should not become hidden multi-step browsing loops. The server can plan bounded subqueries, classify sources, normalize metadata, and return suggested fetch targets, but it should not recursively crawl or synthesize conclusions. Codegg and its agents should remain responsible for interpretation and follow-up decisions.

### Preserve provenance and trust boundaries

Every result and fetched document should retain source URL, provider provenance, source kind, relevant metadata, truncation state, and trust markers. Specialized metadata should improve ranking and agent selection, not imply that external text is trusted as instructions.

### Optimize for smaller coding agents

One purpose of eggsearch is to give codegg smaller and cheaper models deterministic scaffolding. Output should be structured enough that a model can choose the next source without repeatedly inferring whether a result is official docs, source code, a package registry page, an advisory, a stale blog post, or an issue thread.

### Maintain a lightweight Rust binary

Avoid heavyweight browser runtimes, persistent crawlers, local web indexes, and large ML dependencies. Prefer API providers, deterministic parsers, source-card metadata, bounded fan-out, and small internal models of source quality.

## Current baseline to preserve

Eggsearch currently provides a compact and useful core:

- MCP over stdio with `web_search`, `web_fetch`, and `provider_status`.
- Generic metasearch over HTML providers and optional API-backed providers.
- RRF aggregation, deduplication, candidate-pool reranking, and partial-result preservation.
- Search intents for `web`, `docs`, `code`, `issues`, `releases`, `security`, and `news`.
- Repo-oriented query hint parsing for `repo:`, `org:`, `path:`, `file:`, `language:`, `symbol:`, and `host:`.
- Structured source-card metadata for source kind, domain, rank reasons, code metadata, issue metadata, and release metadata.
- Explicit bounded `web_fetch` with HTML/Markdown/code/plaintext/data-format rendering improvements and optional PDF extraction.
- Prompt-injection-aware framing and sanitization of external untrusted content.

This roadmap assumes those pieces remain intact and are hardened rather than replaced.

## Roadmap overview

### Phase 1: Baseline preservation and capability audit

Before adding specialized workflows, lock down the generic search/fetch contract and audit current provider capability reporting. The purpose of this phase is to make sure the new work is additive and does not regress the existing MCP shape.

Deliverables:

- Document the stable public contract for `web_search`, `web_fetch`, and `provider_status`.
- Add explicit tests showing generic `web_search` behavior remains intent-neutral when `intent = web`.
- Add regression tests for existing intent handling, freshness warnings, provider failures, sanitization, and max-result clamping.
- Audit provider descriptors for accuracy: freshness, code search, repo/org/path/language filters, issue search, release search, timestamp support, safe search, and domain filters.
- Add warning behavior for requested semantics that are not enforceable by selected providers, especially freshness, safe search, domain filtering, and source-type-specific retrieval.
- Establish a compatibility matrix for current and planned MCP tools so codegg can gate behavior based on server version/capabilities.

Success criteria:

- Existing generic use cases behave the same or better.
- New specialized phases can be implemented without overloading the semantics of the generic `web_search` path.
- Provider capabilities are accurate enough for codegg to make routing decisions.

### Phase 2: Repo/API/project retrieval bundles

Add a repo-oriented retrieval layer for code/API/project understanding. This should build on existing repo hint parsing, code-host URL metadata, GitHub code/issues/releases providers, and `web_fetch` source-file rendering.

The core improvement is to stop making codegg assemble repository context from a flat list of generic search results. Eggsearch should be able to return typed repo evidence groups such as official docs, package registry, repository root, README, examples, relevant source files, issues, releases, changelogs, and migration notes.

Candidate interface options:

- Add a new `repo_search` MCP tool.
- Or add a specialized `intent = repo_context`/`code_context` mode while preserving `web_search`.
- Or expose both: `web_search` for discovery and `repo_search` for structured repo bundles.

Recommended direction: add `repo_search` as a new explicit tool. This avoids making `web_search` too complex and gives codegg a clear capability boundary.

Deliverables:

- Define `RepoSearchRequest` with fields for query, host, owner, repo, org, path, file, language, symbol, include_docs, include_registry, include_issues, include_releases, include_examples, max_results, max_per_group, freshness, and timeout.
- Define `RepoSearchResponse` with grouped result sections rather than one flat list.
- Add deterministic grouping: `official_docs`, `package_registry`, `repository`, `readme`, `examples`, `source_files`, `issues`, `pull_requests`, `releases`, `migration_notes`, `community_discussion`, and `other`.
- Expand code-host metadata where useful for repository root, README-like files, examples directories, test directories, source files, tags, releases, issues, PRs, and commits.
- Add source-quality rank reasons that help codegg choose authoritative docs and primary source over SEO pages.
- Prefer native GitHub API providers when configured, but retain generic provider fallback when API keys are absent.
- Add tests using mocked provider results to ensure repo hints route and group correctly.

Success criteria:

- A query like `repo:tokio-rs/axum middleware symbol:Layer language:rust` returns a grouped evidence set with docs/source/issues/releases where available.
- A query like `rust crate axum middleware migration` can surface package registry, docs.rs, GitHub repo, releases, and relevant migration notes without codegg hand-crafting multiple searches.
- Generic `web_search` remains available and unchanged for broad queries.

### Phase 3: Security advisory and CVE retrieval

Add an authoritative security-research layer for vulnerability and defensive-programming workflows. The current `security` intent and `SecurityAdvisory` source kind are useful but insufficient for codegg security analysis because they do not normalize vulnerability facts.

Recommended direction: add a new `security_search` MCP tool and a `VulnerabilityMetadata` model that can also appear on generic `SourceCard` metadata when a result is recognized as an advisory.

Provider priorities:

- OSV API.
- GitHub Security Advisories / GHSA.
- NVD CVE API or NVD web/API-compatible adapter, depending on key requirements and rate limits.
- RustSec advisory database support, ideally via lightweight fetched index or API-compatible source.
- PyPA advisory / Python packaging advisory sources where practical.
- CISA KEV catalog.
- Vendor advisories through generic search/source classification, with later native adapters where high-value.

Deliverables:

- Define `VulnerabilityMetadata` with CVE IDs, GHSA IDs, OSV IDs, RustSec IDs, package ecosystem, package name, affected ranges, patched ranges, severity, CVSS vector/score, EPSS if available, KEV status, published/modified dates, references, advisory source, and withdrawn status.
- Define `SecuritySearchRequest` with query, ecosystem, package, version, cve_id, ghsa_id, osv_id, rustsec_id, severity_min, include_kev, include_exploit_context, include_defensive_guidance, max_results, and timeout.
- Define `SecuritySearchResponse` with grouped sections: `authoritative_advisories`, `vendor_advisories`, `package_advisories`, `kev_entries`, `patch_commits_or_releases`, `exploit_discussion`, `defensive_guidance`, and `general_context`.
- Add deterministic parsing for CVE/GHSA/OSV/RustSec identifiers in queries.
- Add version-range matching helpers where ecosystem semantics are tractable, starting with Rust crates and Python packages if feasible.
- Add freshness/timestamp handling for advisory update dates.
- Add clear warnings when severity, affected ranges, exploitability, or KEV status are unavailable rather than inferred.
- Add tests for identifier parsing, metadata normalization, grouping, and partial-provider failure behavior.

Success criteria:

- A query like `CVE-2024-... rust crate defensive guidance` returns normalized advisory metadata where available plus source cards for vendor/package/security context.
- A query like `package:openssl version:... ecosystem:debian` can report whether the server lacks ecosystem support rather than pretending to know.
- Codegg can distinguish authoritative vulnerability records from blogs, issue comments, and exploit chatter.

### Phase 4: Deep research source planning and evidence grouping

Add a bounded research-planning layer for difficult architectural questions. This should not synthesize final answers or crawl recursively. It should help codegg's deep research agent build a high-quality source set faster.

Recommended direction: add a `research_search` MCP tool that returns a source plan and grouped source candidates. The agent can then decide which URLs to fetch explicitly with `web_fetch`.

Deliverables:

- Define `ResearchSearchRequest` with query, research_domain, desired_source_types, freshness, max_results, max_groups, max_per_group, include_counterpoints, include_primary_sources, include_recent_discussion, and timeout.
- Define source groups such as `primary_sources`, `official_docs`, `specifications`, `reference_implementations`, `design_discussions`, `benchmarks`, `security_considerations`, `issue_threads`, `release_notes`, `academic_or_formal_sources`, `recent_news`, `community_discussion`, `counterpoints`, and `unknown`.
- Add deterministic query expansion templates for architectural research, such as docs/specs, implementation, design discussion, benchmark, security, failure modes, migration, and alternatives.
- Keep query expansion bounded and transparent by returning the subqueries used.
- Add source diversity constraints so one domain or one provider does not dominate the result set.
- Add evidence-quality metadata: primary/secondary/tertiary source, official/unofficial, maintainer-authored, vendor-authored, community, news, forum, unknown.
- Add `suggested_fetches` ranked by likely information gain, source quality, and diversity.
- Add tests showing that the tool returns grouped candidates and subquery provenance without fetching or summarizing pages.

Success criteria:

- A query like `compare QUIC vs WebSocket IPC for a coding agent daemon` returns a balanced source plan with specs/docs, implementation references, performance/security discussion, and alternatives.
- The response exposes enough metadata for codegg to fetch a small number of high-value sources rather than reading a flat search list.
- The tool remains bounded, non-crawling, and provenance-preserving.

### Phase 5: Unified source-quality taxonomy and ranking hardening

Introduce a richer source-quality model that all search modes can use. The existing `SourceKind` and `RankReason` model is useful, but codegg-specific workflows need more epistemic structure.

Deliverables:

- Add a `SourceAuthority` or `EvidenceClass` enum distinct from trust labels. External content remains untrusted as instructions, but can be classified as official documentation, official specification, package registry, maintainer-authored source, maintainer-authored issue/PR/release, vulnerability database, vendor advisory, government advisory, academic/formal source, benchmark, blog, forum, exploit PoC, news, content farm, or unknown.
- Add source-quality rank reasons: `official_source`, `primary_source`, `maintainer_source`, `registry_source`, `advisory_database`, `vendor_advisory`, `government_advisory`, `fresh_timestamp`, `diverse_domain`, `low_authority_source`, and `ambiguous_source`.
- Keep trust semantics separate: `external_untrusted` should continue to mean the content must not be treated as instructions.
- Add deterministic domain priors for common code and security sources: docs.rs, crates.io, PyPI, npm, MDN, GitHub, GitLab, Codeberg, OSV, NVD, GitHub Advisory Database, RustSec, CISA, vendor advisory domains, and official project domains where recognized.
- Add ranking tests that show official docs/advisories can be promoted without overwhelming multi-provider relevance evidence.
- Add warnings when authority classification is heuristic rather than provider-native.

Success criteria:

- Codegg can prioritize official docs over tutorials, advisory databases over blog summaries, and source/release records over secondary commentary.
- Ranking remains explainable through deterministic `rank_reasons` rather than opaque model-generated prose.

### Phase 6: Fetch integration for repo, security, and research workflows

Extend `web_fetch` integration so specialized search results can be fetched in the most useful representation without weakening the explicit URL boundary.

Deliverables:

- Add fetch helpers or metadata hints for source cards: recommended extract mode, expected document kind, code-host raw transformation eligibility, line anchor handling, and section/chunk targets.
- Ensure source-file fetches preserve line numbers, language, path, ref, and raw/browser URL transformation metadata.
- Ensure advisory fetches retain vulnerability IDs and advisory-source metadata when fetched from known advisory URLs.
- Add optional `target_fragment`, `target_lines`, or `target_chunk` support if it can be implemented without changing the single-URL boundary.
- Add tests for fetching URLs returned by repo/security/research modes, especially GitHub blob URLs, docs pages, release notes, advisories, JSON APIs, Markdown files, and PDFs when the feature is enabled.

Success criteria:

- A specialized search response can guide codegg to fetch the right URL in the right mode with minimal extra inference.
- The server still does not crawl linked pages or execute JavaScript.

### Phase 7: Codegg integration contract and ergonomic MCP surface

Finalize the interface that codegg should consume. This phase should reduce ambiguity and avoid making codegg rely on undocumented fields or provider-specific quirks.

Deliverables:

- Document recommended codegg usage patterns: generic search, repo search, security search, research search, explicit fetch, and provider status checks.
- Add stable JSON schema examples for each tool and response type.
- Add capability discovery output so codegg can detect whether specialized tools and providers are available.
- Add clear fallback behavior: if `repo_search` is unavailable use `web_search intent=code`; if `security_search` lacks native advisory providers use `web_search intent=security` with warnings; if `research_search` is unavailable use generic search with explicit source grouping in codegg.
- Add error taxonomy and warnings that are easy for codegg to show in the TUI.
- Add end-to-end mocked tests that simulate codegg workflows without live network calls.

Success criteria:

- Codegg can integrate specialized eggsearch features without hardcoding provider internals.
- The user can still invoke plain generic search when that is the right tool.
- Failure modes are visible and actionable rather than silent.

### Phase 8: Hardening, performance, and release closure

Close the roadmap with performance and safety hardening across all search modes.

Deliverables:

- Add bounded concurrency and per-provider timeout tests for new specialized tools.
- Add rate-limit behavior tests for API providers.
- Add cache policy review. Prefer small, explicit, TTL-bound caches for provider metadata or advisory indexes where useful; do not add an unbounded crawler cache.
- Add SSRF/private-network protections to any new fetch-adjacent path.
- Add prompt-injection marker scanning tests for all new response text fields.
- Add truncation and max-result tests for grouped responses.
- Add docs for configuration, API keys, provider enablement, and privacy/security considerations.
- Run full test suite and update README/CHANGELOG before release.

Success criteria:

- Specialized tools are bounded, testable, and safe under hostile external content.
- Generic search and fetch retain their original ergonomics.
- The release can be consumed by codegg as a stable MCP dependency.

## Suggested detailed plan files

Future handoff plans should be split as follows:

- `plans/phase-1-baseline-capability-audit.md`
- `plans/phase-2-repo-api-project-bundles.md`
- `plans/phase-3-security-advisory-cve-retrieval.md`
- `plans/phase-4-deep-research-source-planning.md`
- `plans/phase-5-source-quality-ranking.md`
- `plans/phase-6-fetch-integration-specialized-workflows.md`
- `plans/phase-7-codegg-integration-contract.md`
- `plans/phase-8-hardening-release-closure.md`

## Implementation notes

The highest-value early path is Phase 2, because repo/API/project understanding is the most common coding-agent workflow and the repo already has most of the primitives: intent hints, repo query parsing, source-kind classification, code-host metadata, GitHub code/issues/releases providers, and structured fetch output.

Phase 3 should follow closely because security work is high leverage but requires stronger correctness discipline. Do not infer vulnerability facts from generic snippets when an authoritative advisory provider is unavailable. Prefer explicit warnings and partial results.

Phase 4 should remain bounded and planner-like. A deep research agent benefits from source grouping, query transparency, diversity, and suggested fetches, but final synthesis belongs in codegg, not eggsearch.

Throughout the roadmap, avoid collapsing everything into one overloaded `web_search` tool. Generic search should stay simple. Specialized tools should exist because they encode structured retrieval semantics that are valuable to codegg and difficult for small models to reproduce reliably from flat search results.
