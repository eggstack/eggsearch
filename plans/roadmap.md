# eggsearch Coding-Agent Retrieval Roadmap

## Purpose

This roadmap defines the next line of work for eggsearch as a search, fetch, and evidence-retrieval substrate for coding agents, especially codegg. The current repo already has the right foundation: generic web metasearch, explicit URL fetch, structured repository search, repository file fetch, bounded batch fetch, security advisory search, research-oriented search, provider diagnostics, code evidence metadata, local workspace search, package-aware retrieval, and exact-error mode. The next objective is to improve precision, latency, routing transparency, and agent-loop ergonomics.

The goal is not to turn eggsearch into an autonomous crawler, browser, summarizer, or heavyweight persistent index. The server should preserve its current discipline: search discovers candidate evidence, fetch is explicit and bounded, remote text remains untrusted, provider failures are visible, local trust boundaries are explicit, and partial results are returned when deadlines are hit.

## Current baseline to preserve

The following invariants should remain stable unless a deliberate major-version change is made:

- `web_search` remains the generic metasearch discovery tool returning compact `SourceCard` results.
- `web_fetch` remains explicit single-URL retrieval with bounded redirects, byte/character caps, structured document rendering, link extraction, and untrusted-content framing.
- `repo_search` remains the primary structured repository discovery tool, rather than splitting into host-specific tools such as `github_search` and `gitlab_search`.
- `repo_fetch` remains the precise repository file/span fetch primitive.
- `batch_fetch` remains a bounded fan-out tool over explicit URLs or structured repo locators, never a crawler.
- `security_search` remains the structured advisory/vulnerability retrieval path.
- `research_search` remains bounded multi-source evidence discovery for architectural and technical questions.
- `provider_status` remains the diagnostic surface for hosts and humans.
- External snippets and fetched page text are always data, not instructions.

## Design principles

### Prefer typed evidence over generic snippets

Coding agents should not need to infer from title text whether a result is a source file, README, issue, changelog, advisory, release note, test, example, registry page, or official documentation. Results should expose deterministic metadata, source kind, source role, provider provenance, quality signals, rank reasons, and suggested next fetch actions.

### Keep the tool surface small and composable

Additions should prefer richer request/response types and a small number of high-leverage tools over many host-specific tools. Repository and package concepts should be host-neutral where possible. Host-specific behavior should live behind provider adapters and capability metadata.

### Make provider limitations explicit

When repo filters, language filters, path hints, symbol hints, freshness, issue search, release search, or advisory lookup cannot be enforced by the selected providers, responses should say so in structured telemetry and warnings. Agents should be able to distinguish “no result exists” from “the active providers could not search that dimension precisely.”

### Optimize for codegg loops

Codegg should be able to select a coding/security/research profile, inspect provider capabilities, obtain a repository map, search for typed evidence, fetch exact spans, batch-fetch selected evidence, and hand an evidence bundle across manager/coder/reviewer agents without provider-specific prompt gymnastics.

### Defer heavyweight indexing

Do not introduce a persistent large index, browser runtime, JavaScript execution, or ML ranking dependency in this track. Start with deterministic metadata, provider APIs, bounded tree/list calls, line/span extraction, local workspace identity, provider telemetry, package metadata, and advisory/version reasoning.

## Roadmap overview

### Phase 1: Contract cleanup and agent-facing semantics

Correct the contract between documentation, MCP schemas, and implementation. The highest-priority issue is `repo_search`: documentation describes repo-only discovery, while implementation currently requires a non-empty query. The preferred fix is to allow `repo_search` when either a non-empty query or a resolved repository locator is provided. Repo-only calls should produce default structural discovery subqueries.

This phase should also update MCP tool descriptions and initialize instructions so agents know when to use `repo_search`, `repo_fetch`, `batch_fetch`, `security_search`, `research_search`, and exact-error mode.

Primary outcome: agents can reliably call `repo_search` for repo-only discovery and receive consistent documented behavior.

### Phase 2: Repository map and structural discovery

Add a bounded `repo_map` or `repo_overview` capability. This should retrieve repository structure without cloning: default branch, important root files, manifests, package/workspace layout, source roots, examples, tests, docs, CI workflows, security policy, changelog, releases, and likely entrypoints. Native provider APIs should be preferred, with search-based fallback when unavailable.

Primary outcome: codegg can quickly understand where to look in a repository before issuing targeted source fetches.

### Phase 3: Suggested-fetch ranking overhaul

Replace fixed first-card-per-group suggested fetch selection with a scoring pipeline. Suggested fetches should be ranked by expected information gain using source kind, code evidence confidence, source role, line anchors, symbol/error matches, authority, freshness, group quality, diversity, and whether a structured `repo_fetch` request is available.

Primary outcome: agents read the highest-value evidence first and waste fewer turns fetching generic pages.

### Phase 4: Parallel subquery dispatch and latency control

Refactor specialized search dispatch from sequential subquery execution into priority-aware bounded parallel execution over `(subquery, provider)` jobs. Preserve deterministic response ordering, global deadlines, per-provider failure accounting, and partial-result semantics.

Primary outcome: repo/security/research searches use their deadline more efficiently and return useful evidence sooner.

### Phase 5: Symbol/span-aware repository fetch

Extend `repo_fetch` so callers can fetch around a symbol, match text, or provider text-match line rather than only explicit line ranges. Add deterministic block expansion for common languages and configuration formats. Integrate code evidence line/context metadata into structured fetch locators.

Primary outcome: agents can request “the definition/implementation around this match” without first guessing line ranges.

### Phase 6: Local workspace identity and trust-aware routing

Make local workspace search repository-aware. Detect Git remotes, worktree roots, current branch/commit, dirty state, and package manifests. When a requested remote repo matches a configured local checkout, prefer local trusted evidence and annotate it with remote identity and worktree state.

Primary outcome: codegg reads the user’s actual local checkout instead of stale remote files when the repo is available locally.

### Phase 7: Provider routing, diagnostics, and adaptive degradation

Promote provider/profile routing into a shared response concept across tools. Add process-local provider health snapshots: recent failure class, latency, rate-limit status, last success, and cooldown state. Use this for transparent routing and to avoid repeatedly selecting degraded providers.

Primary outcome: agents and UIs can see why a search degraded and which providers actually enforced the requested capability.

### Phase 8: Package and ecosystem expansion

Expand package-aware retrieval beyond crates.io, PyPI, and npm. Add Go modules, Maven/Gradle, NuGet, RubyGems, Packagist, Docker/OCI images, and GitHub Actions. Keep resolution metadata-only: no dependency solving and no artifact downloading by default.

Primary outcome: coding agents can start from package coordinates across common ecosystems and receive registry/docs/source/release/security evidence.

### Phase 9: Security reasoning and dependency applicability

Deepen security retrieval from advisory discovery to package/version applicability. Parse common dependency and lock files, compare affected/fixed ranges, and return affected/not affected/unknown with reasons and confidence. Preserve the distinction between advisory applicability and deployment exploitability.

Primary outcome: agents can answer “does this dependency version appear affected?” with traceable advisory evidence.

### Phase 10: Evidence bundles for multi-agent handoff

Introduce a deterministic non-summarizing evidence bundle object. It should combine search metadata, selected fetches, fetched spans, source quality, trust markers, unresolved gaps, and stable source IDs. The bundle should be suitable for handoff from codegg manager to coder to reviewer agents.

Primary outcome: multi-agent workflows can reuse the same evidence without repeating search/fetch work.

### Phase 11: Code-host coverage completion

Complete raw-source fetch transforms for Codeberg, Gitea, and Forgejo, matching the existing GitHub/GitLab browser-to-raw source behavior. Preserve SSRF validation, redirect safety, and explicit transform metadata.

Primary outcome: source-file fetching is consistent across supported code hosts.

### Phase 12: Quality, benchmarks, and regression corpus

Build an offline mocked evaluation corpus plus optional live smoke tests. Cover API lookup, symbol search, exact-error investigation, migration planning, CVE/package triage, local workspace lookup, repository mapping, and architectural research. Record expected source types, top suggested fetches, warnings, and unacceptable regressions.

Primary outcome: ranking, routing, warnings, and evidence quality do not regress silently.

## Dependency graph

Phase 1 should land first because it corrects public behavior and documentation. Phase 2 should follow because repository mapping gives agents a better starting point for almost every codebase task. Phase 3 depends on the existing evidence metadata and benefits from Phase 2’s structural signals. Phase 4 can be implemented after Phase 3 or partly in parallel, but it should preserve Phase 3’s deterministic ranking semantics. Phase 5 depends on stable repo-fetch locators and code evidence fields. Phase 6 depends on local workspace search and repo-fetch trust labels already present in the baseline.

Phases 7 through 12 can be scheduled after the first six phases. Provider diagnostics and package/security expansion should be kept modular so they do not block repository search improvements.

## Compatibility requirements

All phases must preserve existing MCP tools and response fields unless a major-version change is explicitly chosen. New fields should use serde defaults and skip-empty serialization where appropriate. Minimal old request shapes should continue to work. Unknown provider IDs supplied explicitly should remain strict validation errors. Profile-based provider degradation should remain non-fatal and visible. Remote content must remain `external_untrusted`; local workspace content must remain explicitly distinguished.

## Success criteria for the whole track

A codegg agent should be able to do the following without provider-specific prompt gymnastics:

- Inspect a repository by name only and receive a structural map plus suggested next fetches.
- Search a repository for a symbol or error and receive exact candidate files/spans.
- Fetch a precise source range or enclosing block with stable line numbers and canonical provenance.
- Prefer local workspace evidence when the target repo is checked out locally.
- Search package coordinates by ecosystem/version and receive docs, registry, source, release, and advisory evidence.
- Determine whether selected providers actually enforced requested repo/path/language/freshness/security constraints.
- Batch-fetch high-value evidence without crawler-like behavior.
- Hand an evidence bundle across codegg agents with stable source IDs, trust markers, and unresolved gaps.
