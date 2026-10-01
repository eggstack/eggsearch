# Plan 001 — Keyless Source Provider Parity

Status: ready

Source roadmap:

- `plans/subsystems/codegg-legacy-search-parity-corrective-addendum.md`

Milestone: M001 keyless/source-specific provider parity

Primary class: capability + compatibility corrective

Planning baseline: `d16a6c6adb04a5b6e2ccb8fec68987e80a452875`

Relevant long-term requirements:

- `plans/000-long-term-specification.md` §§2–5
- `plans/001-terminology-and-domain-model.md` §2
- `plans/003-planning-process.md`

Hard dependencies: none.

## 1. Objective

Add the source-specific, keyless capabilities needed to let CodeGG retire its in-tree external-search fallback without losing useful explicit-source behavior:

- `wikipedia`
- `arxiv`
- `pubmed`
- `hn_algolia`
- `github_repositories`

All five must use the existing eggsearch provider/engine architecture, remain outside `default_providers`, and be fully visible through provider inventory/status/diagnostics.

This plan also freezes the deliberate non-migration of CodeGG's `google_news` RSS provider: news retrieval remains supported, but no undocumented Google News endpoint is imported into eggsearch.

## 2. Current implementation evidence

At the planning baseline:

- `KNOWN_PROVIDER_IDS` contains 37 providers.
- `src/meta/engines/` contains one engine per remote provider with shared bounded Eggfetch transport.
- `build_default_engines` is the construction owner.
- `ProviderCapabilities` has 24 explicit flags; unsupported behavior must remain explicit.
- `provider_status` and CLI provider diagnostics are generated from the same descriptors.
- `tests/provider_capability_contract.rs` pins inventory and truthful native capability claims.
- `tests/docs_provider_inventory.rs` rejects unknown provider names in documented TOML.
- `quick-xml` is already in the repository dependency graph, so arXiv Atom parsing does not justify regex/XML parsing or a second XML stack.
- optional-key routing already exists for `firecrawl_developer`; it can be generalized narrowly for providers that are keyless but accept an operator key only to raise rate limits.

CodeGG's implementations are evidence of required user-facing behavior, not code to copy. The upstream implementations must follow eggsearch's transport, sanitation, retry, cooldown, status, and capability contracts.

## 3. Upstream contracts to revalidate at implementation time

Use current official interfaces:

- MediaWiki search: https://www.mediawiki.org/wiki/API:Search
- arXiv API: https://info.arxiv.org/help/api/
- PubMed E-utilities: https://www.ncbi.nlm.nih.gov/books/NBK25501/
- HN Search API: https://hn.algolia.com/api
- GitHub repository search: https://docs.github.com/rest/search/search#search-repositories

Do not use historical CodeGG endpoint comments as authority when they disagree with current provider documentation.

## 4. Invariants

- No MCP tool is added or renamed.
- MCP tools continue calling `MetadataSearchAdapter`, never engines directly.
- New engines share the existing `Arc<eggfetch_core::Client>`.
- Every response body is bounded by the existing engine body reader.
- Provider output is sanitized and converted into normal `SearchResult` / `SourceCard` data.
- Provider-specific failures remain scoped to that provider and feed existing health/cooldown behavior.
- No new provider enters `default_providers`.
- Keyless provider availability does not depend on environment variables.
- Optional keys may raise limits but must not be required for routing.
- Network-free tests remain authoritative.
- No capability bit is set merely because eggsearch can approximate it locally.

## 5. Provider-specific work

### 5.1 `wikipedia`

Implement a structured MediaWiki Action API engine.

Required behavior:

- query `action=query&list=search` (or a documented equivalent) against the Wikimedia API;
- produce canonical article URLs, title, bounded snippet, and source id;
- strip/normalize API-provided markup through eggsearch sanitation rather than a bespoke HTML stripper;
- claim only capabilities actually represented by the request contract; basic explicit-source search is sufficient for M001;
- keep it routable when explicitly selected, but not part of default fan-out.

### 5.2 `arxiv`

Implement the public arXiv metadata search API.

Required behavior:

- use the documented query interface and Atom response;
- parse Atom with the existing XML stack, not regex;
- map title, abstract/summary, canonical article URL, published/updated timestamp when available;
- advertise `supports_scholarly_search`;
- advertise result timestamps only if the returned timestamp is actually preserved in `SearchResult::published_at`;
- obey arXiv's current repeated-request spacing guidance with one provider-owned shared limiter/gate so concurrent calls cannot bypass it;
- add the attribution required by current arXiv API guidance to operator-facing documentation;
- do not add bulk-harvest behavior.

If eggsearch's current engine orchestration cannot express a safe shared spacing gate without introducing generic scheduler complexity, implement the smallest provider-local synchronization primitive and document it in `architecture/engines.md`.

### 5.3 `pubmed`

Implement PubMed through NCBI E-utilities.

Required behavior:

- use `ESearch` for PMIDs and one bounded `ESummary` batch for result metadata;
- map title, PubMed URL, useful citation metadata, and publication timestamp when available;
- advertise `supports_scholarly_search`;
- preserve provider-scoped errors if the summary phase fails rather than returning fabricated partial metadata;
- identify requests truthfully as eggsearch; do not copy CodeGG's `example.invalid` email;
- support an optional NCBI API key only if current E-utilities guidance and the existing optional-credential seam can do so without making keyless routing dependent on a key;
- use bounded per-call result counts appropriate for interactive agent retrieval.

If current NCBI policy requires contact information beyond a truthful tool/User-Agent identity, stop and add the smallest typed operator config necessary; do not hardcode maintainer or fake user data.

### 5.4 `hn_algolia`

Implement HN Search API retrieval.

Required behavior:

- use the documented relevance search endpoint for ordinary search;
- constrain to story results for parity with CodeGG unless a broader behavior is explicitly justified;
- map title, original URL or canonical HN item URL, bounded text/snippet, and creation timestamp;
- map freshness/date bounds to documented API filters before claiming native freshness support;
- advertise result timestamps when preserved;
- do not broaden this provider into Reddit/Lobsters/community aggregation.

### 5.5 `github_repositories`

Implement repository discovery using GitHub's official repository search endpoint.

Required behavior:

- produce repository title/full name, canonical HTML URL, description, and bounded useful metadata;
- keyless routing must work at GitHub's unauthenticated limits;
- an optional operator `GITHUB_TOKEN` MAY be attached through the existing optional-key mechanism to raise limits;
- a missing token must never make the provider unroutable;
- handle GitHub rate-limit responses through existing provider failure/cooldown semantics;
- do not claim `supports_code_search` or `supports_repo_indexing`; repository discovery is not file/code search;
- do not replace the existing `github_code`, `github_issues`, or `github_releases` providers.

## 6. Provider kind and inventory changes

The existing `ProviderKind::JsonApi` name is not truthful for arXiv Atom/XML.

Add one additive provider-kind representation for structured non-HTML APIs, for example `structured_api`, and use it only where it improves truthfulness. Do not rename/remove existing serialized variants in this corrective.

Update:

- `KNOWN_PROVIDER_IDS`;
- optional API provider inventory if PubMed/GitHub optional keys are adopted;
- `built_in_provider_descriptor`;
- provider display names and capability records;
- engine modules/exports;
- `build_default_engines`;
- config provider booleans/defaults;
- provider-status/probe construction as required.

Expected provider count after M001 alone: 42.

## 7. Explicit Google News disposition

Do not implement `google_news` in M001.

Record in `docs/codegg-integration.md` that:

- the exact legacy source ID has no accepted eggsearch provider;
- news capability remains available through supported providers with `intent = "news"`;
- downstream CodeGG must remove/reject the exact `google_news` provider hint rather than remap it silently.

If implementation research discovers a current documented Google-supported search/news API that matches the old behavior, stop and create a separate provider plan rather than widening M001.

## 8. Ordered work packages

### A — inventory and provider-kind preparation

- extend provider ID/kind/descriptor contracts;
- add network-free descriptor tests;
- preserve all existing provider IDs and serialization behavior.

### B — Wikipedia and HN engines

- implement request shaping/parsers;
- add malformed/empty/oversized response fixtures;
- integrate with builder/status.

### C — scholarly engines

- implement arXiv and PubMed with provider-specific rate/identity requirements;
- preserve publication timestamps;
- add scholarly capability fixtures.

### D — GitHub repository discovery

- implement keyless + optional-key construction;
- qualify rate-limit/degradation behavior;
- keep repository discovery distinct from code/indexing providers.

### E — routing/config/docs integration

- enable explicit provider selection;
- keep all five out of default fan-out;
- update provider setup, engines, config, CodeGG integration docs, and test inventory.

## 9. Focused verification

At minimum:

```bash
cargo test --locked --all-features --test provider_capability_contract
cargo test --locked --all-features --test docs_provider_inventory
cargo test --locked --features mock --test web_search_integration
cargo test --locked --features mock --test provider_routing
cargo test --locked --all-features --test dispatch_fault_injection
cargo test --locked --all-features --test static_guards
make docs-check
make check
```

Add parser/request fixtures to existing behavioral suites or module tests; do not create one CI target per provider.

Optional live probes may be run manually against the public endpoints, but M001 closure must not depend on public-network CI.

## 10. Acceptance criteria

M001 closes only when:

- all five provider IDs are in the canonical inventory;
- explicit provider selection reaches each engine through the normal adapter;
- all providers remain outside `default_providers`;
- arXiv repeated-call policy cannot be bypassed by concurrent calls;
- PubMed uses no fake contact identity;
- GitHub repository search works keyless and any optional key remains optional;
- scholarly/timestamp capability flags match actual preserved behavior;
- malformed/oversized/provider-failure fixtures remain bounded and provider-scoped;
- provider inventory/count/docs are consistent;
- Google News RSS non-migration is documented explicitly;
- `make check` is green on the implementation candidate.

## 11. Stop conditions

Stop and split work if:

- a provider requires a new MCP tool or response type;
- PubMed compliance requires a broad credential/contact subsystem;
- arXiv request pacing requires a general scheduler redesign;
- GitHub repository discovery cannot remain independent of forge tree/indexing ownership;
- adding `structured_api` causes a non-additive provider-status compatibility break;
- an upstream API no longer offers a supported public contract.

## 12. Closure evidence required

Create `plans/closure/codegg-legacy-search-parity/001-status.md` with:

- implementation SHA(s);
- final provider ID/count table;
- request/response fixture evidence per provider;
- capability matrix;
- arXiv pacing evidence;
- PubMed identity/optional-key evidence;
- GitHub keyless/optional-key evidence;
- provider-status/config/docs evidence;
- exact Google News non-migration disposition;
- all focused/broad verification outcomes;
- unresolved findings by severity.
