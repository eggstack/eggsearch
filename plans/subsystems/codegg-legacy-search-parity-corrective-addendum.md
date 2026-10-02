# CodeGG Legacy Search Parity Corrective Addendum

Status: active

Repository planning baseline: `d16a6c6adb04a5b6e2ccb8fec68987e80a452875`

Controlling planning/process:

- `plans/000-long-term-specification.md` §§2–5
- `plans/001-terminology-and-domain-model.md` §2
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md`
- closed predecessor: `plans/subsystems/search-capability-roadmap.md`
- closed CodeGG-quality predecessor: `plans/subsystems/maintenance-codegg-quality-roadmap.md`

Cross-repository consumer evidence:

- `dbowm91/codegg/src/search/*` — explicit compatibility-only provider stack
- `dbowm91/codegg/src/search_backend/legacy.rs`
- `dbowm91/codegg/src/tool/webfetch.rs::execute_builtin`
- `dbowm91/codegg/plans/closure/search-eggsearch-integration/002-status.md`
- CodeGG currently pins eggsearch 0.3.9 and bundles it as `codegg-eggsearch`.

## 1. Purpose

CodeGG can now retire its in-tree external-search fallback only if eggsearch either owns the useful source-specific capabilities that remain unique to that fallback or records an explicit, evidence-backed disposition for capabilities that should not be carried forward.

This addendum owns that upstream parity work. It does not reopen the closed search-capability roadmap or change eggsearch's ten-tool MCP contract. It adds provider engines behind the existing `MetadataSearchAdapter` and produces a precise compatibility handoff for CodeGG.

The target is one external-search owner:

```text
CodeGG stable wrappers
        |
        v
eggsearch MCP (sole external retrieval owner)
        |
        +-- existing metasearch/providers
        +-- source-specific compatibility providers retained on merit
        `-- explicit unsupported/deprecated source dispositions
```

## 2. Fresh audit finding

The legacy CodeGG stack still contains provider behavior not represented by eggsearch 0.3.9:

| CodeGG legacy provider | eggsearch 0.3.9 state | Corrective disposition |
|---|---|---|
| `duckduckgo` | present | no work |
| `mojeek` | present | no work |
| `openalex` | present | no work |
| `brave` | present as `brave`/ `brave_api` | CodeGG mapping cleanup only |
| `exa` | present | no work |
| `tavily` | present | no work |
| `wikipedia` | absent | added in M001 (`wikipedia`, keyless) |
| `arxiv` | absent | added in M001 (`arxiv`, keyless) |
| `pubmed` | absent | added in M001 (`pubmed`, keyless + optional key) |
| `hn_algolia` | absent | added in M001 (`hn_algolia`, keyless) |
| `github` repository discovery | code/issues/releases exist, repository discovery absent | added in M001 (`github_repositories`, keyless + optional token) |
| `serpapi` | absent | added in M002 (`serpapi`, opt-in API-key) |
| `kagi` | absent; CodeGG uses obsolete v0 endpoint | added in M002 (`kagi`, opt-in API-key on current v1, terms guardrails) |
| `google_news` RSS | absent; no current documented Google News search/RSS API contract found | do not add; preserve news capability through supported news engines and make CodeGG retire the exact source hint explicitly |

Current authoritative upstream interfaces reviewed for planning include:

- arXiv public API and current API terms/manual: https://info.arxiv.org/help/api/
- NCBI E-utilities / PubMed: https://www.ncbi.nlm.nih.gov/books/NBK25501/
- MediaWiki Action API search: https://www.mediawiki.org/wiki/API:Search
- HN Search API: https://hn.algolia.com/api
- Kagi Search API v1: https://help.kagi.com/kagi/api/search.html
- Kagi API terms: https://kagi.com/privacy/api
- SerpAPI Google Search API: https://serpapi.com/search-api
- GitHub repository search REST API: https://docs.github.com/rest/search/search#search-repositories

The implementation MUST re-check these contracts at handoff time rather than copying historical CodeGG request shapes.

## 3. Invariants

- The stable MCP tool count remains exactly ten.
- All new providers are engines behind `MetadataSearchAdapter`; MCP tools never call them directly.
- Provider failures remain provider-scoped and never make the whole server unhealthy.
- Missing optional credentials remain typed skips/degradation, not startup failure.
- All response bodies are read through eggsearch's bounded transport helpers; no bare `.text()`, `.bytes()`, or `.json()` ownership is introduced.
- All untrusted fields flow through the normal sanitization/trust pipeline.
- Stable IDs, RRF aggregation, deterministic tie ordering, evidence-bundle semantics, cache semantics, and fetch SSRF policy do not change.
- New source-specific providers MUST NOT be added to `default_providers` merely for CodeGG parity. Keyless compatibility providers may be enabled/routable by default but are invoked only through explicit provider selection or an intentionally changed profile.
- Tests remain network-free and credential-free.
- No provider is documented as natively enforcing a capability unless the actual upstream request maps and enforces it.
- No stale CodeGG endpoint is copied upstream merely to claim parity.

## 4. Provider design decisions

### 4.1 Keyless/source-specific providers to retain

`wikipedia`, `arxiv`, `pubmed`, `hn_algolia`, and `github_repositories` provide materially distinct source selection that is useful independently of CodeGG.

They should become normal eggsearch provider IDs.

Expected default posture:

- known and routable without credentials;
- not members of `default_providers`;
- explicit selection works through existing `providers` request fields;
- `pubmed` and `github_repositories` MAY consume optional operator API keys through the existing optional-credential mechanism if doing so only raises upstream limits and never gates keyless routing.

For arXiv, the engine must obey the public API's repeated-call spacing guidance and include the required attribution in operator documentation. Do not reproduce CodeGG's regex XML parsing when `quick-xml` is already available.

For PubMed, do not copy CodeGG's `example.invalid` contact identity. Use a truthful eggsearch tool identity and only add contact/config fields if current NCBI guidance actually requires them. Any optional NCBI API key remains operator supplied.

### 4.2 Credentialed providers

`serpapi` is a straightforward opt-in API provider using `SERPAPI_API_KEY`.

`kagi` must use the current v1 API, not CodeGG's deprecated `https://kagi.com/api/search` v0 contract. It remains opt-in through `KAGI_API_KEY`, is never a default provider, and must respect current Kagi API terms: operator-owned credentials, no resale/bulk-feed behavior, no attempt to turn eggsearch into a credential-sharing proxy, and no persistence of raw Kagi payloads beyond normal eggsearch evidence/cache policy.

If implementation-time terms review shows that a generic local MCP integration cannot comply, stop Kagi implementation and record a non-migration disposition rather than preserving the obsolete CodeGG endpoint.

**Outcome (M002):** the terms review passed, so Kagi was implemented rather than dispositioned out. The Kagi API Terms permit using returned Results in your own applications and services, explicitly including forwarding them to an AI model, and integrating them that way is not a prohibited transfer. The two binding constraints were enforced and tested: neither engine caches or persists provider results, and quota responses are terminal provider-scoped failures with no retry amplification.

### 4.3 Google News RSS

Do not add a `google_news` provider solely to preserve CodeGG's undocumented RSS endpoint.

The retained capability is news retrieval, already represented by providers that truthfully advertise native news support (currently `brave_api` and `tavily`, with additional providers only when their current contracts justify it).

The CodeGG handoff must state that historical `provider = "google_news"` is retired with an actionable migration to `intent = "news"` plus explicit supported providers where source control is required. Silent remapping is forbidden.

## 5. Provider model/schema impact

The inventory is 44 provider IDs after M002 (was 37 at baseline, 42 after
M001). Both remaining providers were accepted:

- + `wikipedia`
- + `arxiv`
- + `pubmed`
- + `hn_algolia`
- + `github_repositories`
- + `serpapi`
- + `kagi`

M001 added the additive `ProviderKind::StructuredApi` variant rather than falsely
classifying arXiv as `JsonApi` or `HtmlScrape`, used it for `arxiv` only, and
documented the additive `provider_status` serialization effect in
`docs/config.md`, `architecture/core.md`, and `docs/codegg-integration.md`.
Existing wire values (`html_scrape`, `json_api`, `api_key`, `local`) are unchanged.

No new `ProviderCapabilities` flag is required for repository discovery. A provider may be useful through explicit `web_search` selection without pretending to support code search or repository indexing.

## 6. Milestones and dependencies

```text
M001 keyless/source-specific parity
        |
        +-------------------+
        | hard              | hard
        v                   v
M002 credentialed parity   (consumer research may proceed)
        |
        +---------+
                  v
M003 parity qualification + CodeGG retirement handoff
                  |
                  v
tagged eggsearch release carrying the accepted surface
                  |
                  v
CodeGG legacy-backend retirement workstream
```

### M001 — Keyless and source-specific provider parity

Status: closed at `e9103b4c50743037552dd0ad94eee003c7ea3d48`.

Plan:

- `plans/implementation/codegg-legacy-search-parity/001-keyless-source-provider-parity.md`

Closure:

- `plans/closure/codegg-legacy-search-parity/001-status.md`

Adds and qualifies `wikipedia`, `arxiv`, `pubmed`, `hn_algolia`, and
`github_repositories`, including the additive `structured_api` provider kind.
The provider-model interfaces this milestone was expected to freeze are now
closed evidence, which was the only hard dependency M002 had.

### M002 — Credentialed provider parity

Status: closed at `523331509936d72c9d3eb327e602b444048d6372`.

Plan:

- `plans/implementation/codegg-legacy-search-parity/002-credentialed-provider-parity.md`

Closure:

- `plans/closure/codegg-legacy-search-parity/002-status.md`

Adds and qualifies `serpapi` and current-v1 `kagi` as explicit opt-in
credentialed providers. Contracts and terms were revalidated against current
provider documentation before any production code, and the narrow deviations
(SerpApi freshness left unclaimed because `tbs` value syntax is undocumented;
Kagi v1 `POST` + `Bearer` rather than the legacy `GET` + `Bot` example) are
recorded in the plan and the closure record. The Kagi terms gate passed with
two enforced constraints: no results persistence and no rate-limit
circumvention.

### M003 — Compatibility qualification and downstream handoff

Status: conditionally closed at `8e5ec755dae2198a06bc881ecd740597b4da5440`,
with one open operational condition: publication of tagged release **v0.4.0**.

Plan:

- `plans/implementation/codegg-legacy-search-parity/003-codegg-retirement-handoff-and-qualification.md`

Closure:

- `plans/closure/codegg-legacy-search-parity/003-status.md`

Freezes the provider/disposition matrix, updates CodeGG integration documentation, runs the full provider-contract/documentation gates, and records the exact release candidate that CodeGG may consume. It does not modify the CodeGG repository.

Qualification was not a rubber stamp. It found and fixed three production
defects in explicit-provider handling (an unbuildable provider was silently
dropped while the response claimed it had been selected; a missing credential
was mislabeled as a disabled provider whose suggested repair does not work for
API-key providers; an all-providers-failed search reported `internal_error`
instead of `upstream_failed`), plus three defects that had been blocking the
required verification gates across two prior closures (a rustdoc lint failing
`docs-check`, an `ETXTBSY` updater flake, and a proptest whose generator
contradicted the SSRF guard). `make check` and `make release-check` now both
pass, which was not true during M001 or M002.

All ten required user journeys execute through the real MCP tool surface; the
credentialed and news journeys drive the real engines against a local HTTP
fixture server. The frozen matrix has no `capability`-class stand-in: the only
retirement is `google_news`, and news is reached through `intent: "news"` on
`brave_api` or `tavily`, the only two providers advertising native news.

Downstream CodeGG retirement remains blocked until the v0.4.0 tag exists. Repository-hardening M008 is closed at `44f38ab` with green Windows/release qualification, so that publication may now proceed to exact-candidate qualification. This does not add a second CodeGG M003 closure condition: M003 remains conditional on publication, while M008 was a prerequisite for performing that publication safely. The
retiring harness may then pin `>=0.4.0`, verify the eleven migration-relevant
ids via `provider_status`, delete its own external-search clients, and remove
`backend="builtin"` and `fallback_to_builtin`.

## 7. Verification strategy

Each provider implementation must include deterministic parser/request-shaping fixtures and extend the existing behavioral suites rather than creating provider-specific CI lanes.

Minimum workstream gates:

```bash
cargo fmt --all -- --check
cargo test --locked --features mock --test web_search_integration
cargo test --locked --features mock --test provider_routing
cargo test --locked --all-features --test provider_capability_contract
cargo test --locked --all-features --test docs_provider_inventory
cargo test --locked --all-features --test static_guards
make docs-check
make check
```

Run `make release-check` for M003 closure.

Live provider probes are optional local qualification only and MUST NOT become network-dependent CI.

## 8. Documentation obligations

As implementation lands, update as applicable:

- `architecture/engines.md`
- `architecture/meta.md`
- `architecture/config.md`
- `architecture/codegg-contract.md`
- `docs/provider-setup.md`
- `docs/codegg-integration.md`
- `docs/tool-matrix.md`
- `docs/test-inventory.md`
- `README.md`
- `AGENTS.md`
- provider inventory/count statements in canonical planning documents when the code inventory actually changes.

Do not rewrite historical closure records for the earlier search-capability workstream.

## 9. Completion definition

This corrective workstream is closed when:

- every retained source-specific CodeGG legacy capability is either implemented in eggsearch or has an explicit non-migration disposition;
- the seven planned provider IDs, if accepted, are fully represented in inventory/config/status/docs/tests;
- explicit provider selection never silently falls back to an unrelated provider;
- Google News RSS is not carried forward without a documented supported contract;
- Kagi uses the current v1 API or is explicitly dispositioned out;
- CodeGG integration docs contain a one-to-one legacy provider migration table;
- `make check` and `make release-check` pass on the exact M003 closure candidate;
- a release candidate/tag boundary is identified for downstream CodeGG consumption.

## 10. Milestone status

| Milestone | Status | Implementation plan | Blocker |
|---|---|---|---|
| M001 keyless/source-specific provider parity | closed | `plans/implementation/codegg-legacy-search-parity/001-keyless-source-provider-parity.md` | none; closure `plans/closure/codegg-legacy-search-parity/001-status.md` |
| M002 credentialed provider parity | closed | `plans/implementation/codegg-legacy-search-parity/002-credentialed-provider-parity.md` | none; closure `plans/closure/codegg-legacy-search-parity/002-status.md` at `5233315` |
| M003 CodeGG retirement handoff + qualification | conditionally closed | `plans/implementation/codegg-legacy-search-parity/003-codegg-retirement-handoff-and-qualification.md` | single closure condition is publication of tagged release v0.4.0; repository-hardening M008 is closed, so publication may proceed; closure `plans/closure/codegg-legacy-search-parity/003-status.md` at `8e5ec75` |
