# eggsearch Coding-Agent Search Roadmap

## Purpose

This roadmap defines the next improvement track for eggsearch as a search and fetch backend for coding agents, especially codegg. The repo already has the important foundation: a generic `web_search`, explicit bounded `web_fetch`, `provider_status`, `repo_search`, `security_search`, and `research_search`. The next objective is precision. Coding agents need fewer generic page candidates and more exact, typed, fetchable evidence: source spans, symbols, package versions, advisory facts, release context, and local workspace matches.

This roadmap does not turn eggsearch into an autonomous crawler, browser, summarizer, or long-running background indexer. The server should keep the current discipline: search is discovery-first, fetch is explicit and bounded, JavaScript is not executed, external content remains untrusted, provider failures are visible, and partial results are preserved. Specialized coding-agent features should be additive and host-neutral where possible.

## Current baseline to preserve

The current repo should preserve these invariants while adding coding-agent affordances:

- `web_search` remains a generic metasearch tool returning compact `SourceCard` results.
- `web_fetch` remains explicit single-URL retrieval with bounded redirects, byte/character caps, structured document rendering, link extraction, and untrusted-content framing.
- `repo_search` remains the primary structured repository discovery tool rather than splitting host-specific tools such as `github_search` and `gitlab_search`.
- `security_search` remains the authoritative advisory/vulnerability retrieval path.
- `research_search` remains a bounded multi-source evidence discovery path for architectural questions.
- `provider_status` remains the way codegg discovers enabled providers and capabilities.
- Search results never imply that external page text is trusted as instructions.

## Design principles

### Prefer typed evidence over prose inference

A coding agent should not have to infer from a title and snippet whether a result is a source file, a README, a test, an issue, a release note, or an advisory. It should receive deterministic metadata, rank reasons, source kind, provider provenance, and suggested fetch actions.

### Keep the tool surface small

Additions should prefer richer request/response types on `repo_search`, `web_fetch`, and a small number of new tools over a proliferation of host-specific tools. `repo_fetch` or `code_fetch` is justified because coding agents need exact repository objects and line ranges, not browser pages.

### Make provider limitations explicit

When repo hints, symbol hints, freshness, language filters, issue search, release search, or advisory lookup are requested but the selected providers cannot enforce them, responses should say so in structured warnings and per-result metadata.

### Optimize for codegg routing

Codegg should be able to inspect provider capabilities, choose a coding/security/research profile, decide whether a result is worth fetching, and request exact source spans without writing provider-specific query strings manually.

### Defer heavyweight indexing

Do not introduce a persistent large local index, browser runtime, or ML ranking dependency in the early phases. Start with typed metadata, fetch precision, provider profiles, package resolution, and optional lightweight local workspace indexing.

## Roadmap overview

### Phase 1: Exact code evidence metadata

Upgrade `SourceCard` and repo/code metadata so a repo result can represent exact code evidence, not only a relevant file URL. Add stable fields for raw URL, permalink URL, commit SHA when known, match line range, context line range, matched symbol, symbol kind, enclosing symbol, and match confidence. This phase should keep `web_search` backward-compatible while allowing `repo_search` and native code providers to attach richer `CodeEvidence`.

Primary outcome: `repo_search` can return “this span in this file is relevant” rather than only “this source file may be relevant.”

### Phase 2: `repo_fetch` / exact code fetch

Add an explicit repository fetch tool that accepts host/owner/repo/ref/path plus optional line ranges and context lines. It should return raw content with stable line numbers, syntax/language metadata, canonical browser URL, raw URL, permalink URL when possible, content hash, truncation metadata, and untrusted/local trust labels. This should reuse as much of `web_fetch` rendering and code-host URL transformation logic as possible, but expose a cleaner coding-agent interface.

Primary outcome: codegg can fetch exact source evidence without relying on browser URL rewriting or generic HTML extraction.

### Phase 3: Coding profiles and repo-search transparency

Add named provider/search profiles such as `generic`, `coding`, `security`, and `research`, and expose repo-search subquery telemetry. `repo_search` should include the generated subqueries, intended group, whether a native capability was needed, and providers used. This makes search behavior debuggable and gives codegg a stable high-level mode switch without manually specifying provider IDs on every call.

Primary outcome: codegg can select the right search behavior by profile and inspect why a repo search missed or found evidence.

### Phase 4: Package/version-aware repository retrieval

Add package coordinate resolution for major ecosystems, starting with Rust crates, Python packages, and npm packages. `repo_search` should accept `ecosystem`, `package`, `version`, and `version_requirement` fields and resolve registry page, docs, source repository, release/changelog context, and advisory context. This should bridge docs, releases, package registries, and OSV without requiring the agent to hand-craft several independent searches.

Primary outcome: coding agents can ask “how does this API work in this package version?” or “what changed between these versions?” and get typed evidence.

### Phase 5: Lightweight local workspace index and symbol enrichment

Add an optional local workspace search backend for configured repository roots. Start with deterministic file/path/text search and structured local cards. Then add tree-sitter-backed symbol extraction for supported languages, beginning with Rust and Python if dependency cost remains acceptable. Local results should be clearly marked as local workspace evidence and should not be mixed with external untrusted content without trust metadata.

Primary outcome: eggsearch can rank local workspace evidence alongside remote docs/issues/releases when codegg requests a coding search.

### Later phases

After the first five phases, future work should focus on bounded batch fetch, richer security-context integration into repo searches, exact error-message mode, expanded host-native providers, and result-quality/uncertainty metadata.

## Phase dependency graph

Phase 1 should land first because all later code search precision depends on richer evidence metadata. Phase 2 should follow because exact metadata is much less useful without exact fetch. Phase 3 can be implemented partly in parallel, but it benefits from knowing what Phase 1 and Phase 2 expose. Phase 4 should follow the first three phases so package resolution can produce high-quality suggested fetches. Phase 5 can proceed independently after the trust model and result metadata decisions in Phase 1 are stable.

## Compatibility requirements

All phases must preserve existing MCP tools and response fields unless a major version bump is explicitly chosen. New fields should use `#[serde(default, skip_serializing_if = ...)]` where appropriate. Tests should cover old minimal request shapes as well as new coding-agent request shapes. The README should document all new tool fields and clarify when behavior is a true native provider capability versus best-effort query planning.

## Success criteria for the whole track

A codegg agent should be able to do the following without provider-specific prompt gymnastics:

- Search a repo for a symbol and receive exact candidate files/spans.
- Fetch a precise source range with stable line numbers and canonical provenance.
- Search a package by ecosystem/version and receive docs, registry, source, releases, and advisories.
- Use a coding profile that prefers native code/issues/releases providers when configured and falls back honestly when not configured.
- Search local workspace files and symbols with clear trust/provenance boundaries.
- Inspect subqueries and warnings to determine whether a missed result was likely due to provider limitations, query planning, timeout, or no match.
