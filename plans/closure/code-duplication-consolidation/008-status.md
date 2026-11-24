# M008 SearchEngine Implementation Macro — Closure Evidence

Status: closed

Candidate: the working tree on `feat/auto-20261117-044c4885` above baseline
`fb9d6538f8ea2e21e08add43fb3f71ebe6ceaa64`, uncommitted because the request did not ask for a
commit. The canonical gate ran on exactly this tree; its output and the diff fingerprint are in
`plans/closure/code-duplication-consolidation/gate.md`.

Source plan: `plans/archive/implementation/code-duplication-consolidation/008-searchengine-implementation-macro.md`

Roadmap: `plans/subsystems/code-duplication-consolidation-roadmap.md` M008

Decision authority: no ADR. The `SearchEngine` trait and dispatch semantics are unchanged.

## Change

- `impl_engine!` in `src/meta/engines/mod.rs` expands `name()` and `search()` for a
  forwarding engine and keeps `Box::pin`, so the trait stays object-safe and `Send`.
- 37 forwarding impls use the macro across the five batches (HTML scrapers, API-key search,
  forges, registries, remainder).
- Two whole impls stay hand-written: `OsvEngine` (advisory capabilities, ID lookup, package
  query overrides) and `FirecrawlDeveloperEngine` (custom `search_batch` and `supports_role`,
  with `search` projecting batch results). A macro cannot express a partial impl.
- `src/meta/engines/mod.rs`: 1,604 → 1,311 lines (−293).

## Equivalence evidence

- The 39-name set is unchanged before and after: `arxiv`, `brave`, `brave_api`,
  `crates_io`, `crossref`, `duckduckgo`, `exa`, `firecrawl_developer`, `gitea_code`,
  `gitea_issues`, `gitea_releases`, `github_code`, `github_issues`, `github_releases`,
  `github_repositories`, `gitlab_code`, `gitlab_issues`, `gitlab_releases`, `go_pkg`,
  `hn_algolia`, `kagi`, `maven_central`, `mojeek`, `npm_registry`, `nuget`,
  `openalex`, `osv`, `packagist`, `pubmed`, `pypi`, `rubygems`, `searxng`,
  `semantic_scholar`, `serpapi`, `sourcegraph`, `startpage`, `tavily`, `wikipedia`,
  `yahoo`.
- After each of the five batches: `engines::` 602 passed and `provider_routing` 99 passed.
- Final commands: `engines::` 602, `provider_routing` 99, `engine_api_conformance` 7,
  `engine_parse_conformance` 6.
- The existing provider-status name-set test (`tests/provider_routing.rs`) plus a direct
  source comparison covered the constructed engine names; no duplicate test was added.
