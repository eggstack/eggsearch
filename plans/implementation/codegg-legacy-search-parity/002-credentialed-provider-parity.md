# Plan 002 — Credentialed Provider Parity

Status: active

Hard dependency satisfied: M001 closed at
`e9103b4c50743037552dd0ad94eee003c7ea3d48`; see
`plans/closure/codegg-legacy-search-parity/001-status.md`.

Closure record: `plans/closure/codegg-legacy-search-parity/002-status.md`

Source roadmap:

- `plans/subsystems/codegg-legacy-search-parity-corrective-addendum.md`

Milestone: M002 credentialed provider parity

Primary class: capability + compatibility corrective

Planning baseline: `d16a6c6adb04a5b6e2ccb8fec68987e80a452875`

Hard dependency:

- M001 keyless/source-specific provider parity must close first so provider-kind,
  optional-credential, inventory, and status interfaces are frozen.

Relevant long-term requirements:

- `plans/000-long-term-specification.md` §§2–5
- `plans/001-terminology-and-domain-model.md` §2
- `plans/003-planning-process.md`

## 1. Objective

Move the two useful credentialed general-search providers still stranded in
CodeGG's compatibility backend into eggsearch:

- `serpapi`
- `kagi`

Both providers must be explicit opt-in engines using operator-owned credentials.
Neither becomes a default provider, profile requirement, or CodeGG-specific
special case.

The implementation must use each provider's current supported API rather than
copying the legacy CodeGG HTTP contract.

## 2. Current implementation evidence

CodeGG's legacy backend currently owns:

- SerpAPI through `https://serpapi.com/search` with
  `SERPAPI_API_KEY`, `engine=google`, query, and result count.
- Kagi through the historical `https://kagi.com/api/search` endpoint with
  `KAGI_API_KEY`.

The Kagi path is stale relative to the current documented v1 Search API. It
must not be copied into eggsearch.

Eggsearch already has the required generic infrastructure:

- required API-provider configuration under `[search.api.<id>]`;
- typed missing-key skips;
- one shared Eggfetch transport;
- bounded provider body reads;
- provider health/cooldown tracking;
- provider-neutral `SearchResult` and `SourceCard`;
- explicit capability descriptors and provider-status reporting.

## 3. Contracts to revalidate immediately before implementation

Use current official/provider documentation:

- Kagi Search API: https://help.kagi.com/kagi/api/search.html
- Kagi API terms/privacy: https://kagi.com/privacy/api
- SerpAPI Google Search API: https://serpapi.com/search-api

Do not use CodeGG source as API authority.

If an upstream provider materially changes pricing, authentication, permitted
use, or response contract between planning and implementation, record the
change and adapt the plan narrowly before coding.

### 3.1 Revalidation outcome (reviewed 2026-10-02)

Sources reviewed: `https://serpapi.com/search-api`,
`https://serpapi.com/pricing`, `https://help.kagi.com/kagi/api/search.html`,
`https://help.kagi.com/kagi/api/quick-start.html`,
`https://kagi.com/api/docs` and its published OpenAPI bundle, and
`https://kagi.com/privacy/api` (Kagi API Terms, effective 2026-09-22). CodeGG
source was not used as API authority.

**Kagi v1 request contract (narrow adaptation to §6).** The current v1 search
endpoint is `POST https://kagi.com/api/v1/search` with a JSON body
(`query` required; optional `workflow`, `limit`, `page`, `safe_search`,
`filters.region`, `filters.after`, `filters.before`, `lens`, `lens_id`,
`timeout`, `format`, `extract`, `personalizations`) and
`Authorization: Bearer <key>`. The `GET .../api/v1/search?q=` example with an
`Authorization: Bot <key>` header still shown on the help page is the legacy
v0 style; the v1 OpenAPI spec, the v1 quick start, the generated official
clients, and Kagi's own hosted MCP server all use `Bearer`. Implementation uses
`POST` + `Bearer` and never calls the v0 endpoint.

**Kagi response contract.** Results for the `search` workflow arrive in
`data.search[]` with `url`, `title`, `snippet`, and `time`. The same response
may also carry non-search collections (`data.news`, `data.code`,
`data.interesting_finds`, `data.related_search`, `data.infobox`, ...).
Implementation always requests `workflow: "search"`, parses only
`data.search`, and ignores every other collection rather than coercing it into
source cards. `extract` (extra billed page extraction) and `personalizations`
are never sent. Errors are `4xx`/`5xx` with a JSON `error[]` envelope; the
status alone is classified, matching existing engine behavior.

**Kagi terms gate: pass, with two binding implementation constraints.** The
API Terms grant a non-exclusive, non-transferable license to call the API and
use returned Results "in your own applications and services, including sending
Results to an AI model to answer your users' queries", and state that
integrating Results this way "is not a prohibited transfer". A local,
user-operated MCP server using the operator's own key is not resale,
sublicensing, proxying, a bulk feed, or a substitute for direct API access, so
the intended integration is compatible. The Terms bind the implementation in
two ways that must be evidenced at closure: (1) no persistent store, cache, or
derivative index of Results beyond the transient caching needed to operate the
search, and (2) no circumventing of rate limits, quotas, or access controls,
including no retry amplification. `eggfetch-core` is built without
`logical-retry`, so a quota response yields exactly one request attempt and a
provider-scoped `RateLimited` outcome with health cooldown.

**Kagi billing.** API usage is metered and billed separately from any Kagi
subscription (pay-as-you-go, `https://kagi.com/api/pricing`); an account and
payment method are required. Operator responsibility for a valid, funded Kagi
API key is documented, and the terms explicitly require keeping the key
secret, which matches eggsearch's env-indirection contract.

**SerpAPI request contract.** `GET https://serpapi.com/search` with
`engine=google` sent explicitly, `q` for the query, `api_key` as a documented
query parameter, `safe=active|off`, `hl` for language, `gl` for country, and
`start` for offset. No `num`-style result-count parameter is documented for
this engine, so the result budget is enforced by bounded local truncation of
`organic_results`.

**SerpAPI freshness boundary (narrow adaptation to §5).** The current engine
page documents `tbs` only generically ("advanced search parameters ... dates")
and does not document its value syntax; per-engine pages for other engines
document their own date parameters separately. Sending an unverified `tbs`
value would be a guess, so eggsearch does not send it and does not advertise
native freshness for `serpapi`; freshness and explicit date ranges stay local
approximation.

**SerpAPI credential handling.** Because the credential is a query parameter by
upstream contract, the engine accepts the base URL from the transport first and
adds `api_key` (and every other parameter) through the parameter builder, so no
error path can embed the credential in a message: the only URL-carrying
transport error is raised while parsing the base URL, before the key exists.

**SerpAPI error contract.** SerpAPI publishes no status-code reference page;
documented failures use a `{"error": "..."}` JSON envelope (visible in the
published Search Archive API examples). Implementation classifies by HTTP
status (`429` -> provider-scoped rate-limited outcome, other non-2xx ->
provider-scoped failure) and does not parse or echo error bodies, so no
undocumented string matching and no credential-adjacent body text reaches
diagnostics.

**Configuration contract correction (narrow adaptation to §5/§6).** The
snippets in this plan also set `[search.providers] serpapi = true`. In this
codebase that boolean is inert for API-key providers: `effective_provider_ids`
excludes every `API_PROVIDER_IDS` member from the `[search.providers]` map, and
routing is controlled solely by `[search.api.<id>]`. Documentation will show
only the contract that actually works, matching `brave_api`/`exa`/`tavily`.

**Inventory.** Both providers are accepted, so the post-M002 inventory is 44
registered provider IDs.

## 4. Invariants

- No MCP tool or response schema is added for these providers.
- Both providers remain disabled unless explicitly enabled/configured.
- Neither provider is added to `default_providers`.
- Missing/empty credentials produce normal provider-scoped skip diagnostics.
- Credentials never appear in logs, error bodies, structured evidence, cache
  keys, or provenance.
- All HTTP uses the shared engine client and bounded response helpers.
- Provider failures participate in existing health/cooldown behavior.
- Untrusted result text is sanitized through normal eggsearch paths.
- Native capability flags reflect only fields actually mapped upstream.
- Tests remain network-free and never require real credentials.
- No CodeGG-specific provider aliases are added to the eggsearch MCP schema.

## 5. SerpAPI implementation

Add provider ID `serpapi` as a required-credential API provider.

Configuration contract:

```toml
[search.providers]
serpapi = true

[search.api.serpapi]
enabled = true
api_key_env = "SERPAPI_API_KEY"
```

Required behavior:

- use the current documented Google Search API contract;
- set the provider engine explicitly rather than relying on a mutable upstream
  default;
- map query and bounded result-count semantics;
- map safe-search, language/locale, region, freshness/date constraints only
  where the current API represents them faithfully;
- retain result title, URL, snippet and result timestamp only where the
  provider actually returns usable timestamp evidence;
- preserve provider-scoped quota/rate-limit errors and do not convert them into
  global search failure;
- allow a test-only/configurable base URL only if this matches the repository's
  existing API-provider testing pattern.

Do not request generated answers or unrelated SERP verticals merely because
SerpAPI exposes them.

## 6. Kagi implementation

Add provider ID `kagi` as a required-credential API provider.

Configuration contract:

```toml
[search.providers]
kagi = true

[search.api.kagi]
enabled = true
api_key_env = "KAGI_API_KEY"
```

Required behavior:

- use the current documented Kagi Search API v1 endpoint and authentication;
- never call CodeGG's historical v0 endpoint;
- map only documented search request fields and parse only documented response
  types needed for ordinary source discovery;
- ignore or reject non-search result variants explicitly rather than coercing
  them into false source cards;
- keep Kagi outside all default/profile fan-out unless a later independent
  retrieval-quality plan provides evidence for changing that policy;
- document operator responsibility for a valid Kagi API account/key.

### Kagi terms gate

Before implementation, review the current Kagi API terms for a local,
user-operated MCP server using the user's own credential.

The implementation is allowed only if that usage remains compatible with the
current terms and does not make eggsearch a credential-sharing, resale, or
bulk-feed service.

If compliance is ambiguous or incompatible:

1. do not ship a Kagi engine;
2. record Kagi as a deliberate non-migration in the M002 closure;
3. keep CodeGG retirement unblocked if all other retained capability criteria
   are satisfied and CodeGG removes the Kagi provider hint with an actionable
   migration error.

Do not preserve an obsolete implementation merely for nominal parity.

## 7. Inventory and capability work

If both providers land:

- add `serpapi` and `kagi` to `KNOWN_PROVIDER_IDS`;
- add both to `API_PROVIDER_IDS`;
- update descriptors/display names/provider kinds;
- register construction in `build_default_engines`;
- extend config/provider booleans and diagnostics;
- extend provider-capability tests and docs-inventory tests.

Expected provider inventory after M001 + M002: 44.

If Kagi is dispositioned out, the final inventory is 43 and all docs/tests must
use that actual value rather than retaining a planned count.

## 8. Ordered work packages

### A — contract/terms revalidation

- verify current endpoints, auth, request parameters, rate/quota semantics;
- record Kagi terms disposition before production code is committed.

### B — SerpAPI engine

- request builder + response parser;
- error/rate-limit mapping;
- bounded fixture coverage;
- descriptor/config/status integration.

### C — Kagi engine if terms gate passes

- current-v1 request builder + response parser;
- variant handling and bounded fixture coverage;
- descriptor/config/status integration.

### D — capability and configuration truth pass

- advertise only actual native enforcement;
- prove missing credentials are typed skips;
- prove neither provider enters default fan-out.

### E — documentation and CodeGG handoff preparation

- provider setup/config docs;
- engines/meta docs;
- CodeGG integration migration table draft;
- test inventory updates.

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

Required negative fixtures:

- missing SerpAPI key -> provider-scoped missing-key state;
- missing Kagi key -> provider-scoped missing-key state if Kagi lands;
- quota/rate response -> provider failure/cooldown path, no global failure;
- malformed/oversized response -> bounded provider-scoped failure;
- credential values absent from diagnostic/error snapshots;
- neither provider appears in default fan-out.

Live calls are optional maintainer evidence only and must not become CI.

## 10. Acceptance criteria

M002 closes when:

- SerpAPI is implemented through the current supported API;
- Kagi is either implemented through current v1 with a documented terms
  disposition or explicitly rejected as a migration target;
- no legacy Kagi v0 endpoint exists in eggsearch production code/docs;
- provider inventory/config/status accurately reflect the resulting provider
  set;
- missing credentials degrade only the selected provider;
- capability flags match actual request mappings;
- both providers remain opt-in;
- documentation contains enough information for CodeGG to remove its direct
  credentialed provider clients;
- `make check` passes on the implementation candidate.

## 11. Stop conditions

Stop and split/replan if:

- either provider requires a new MCP tool;
- Kagi terms do not permit the intended user-operated integration;
- a provider requires a new shared credential service rather than the existing
  env-indirection contract;
- a provider response requires a broad new evidence schema;
- implementation would add provider-specific retry/cache machinery beside the
  existing adapter infrastructure.

## 12. Closure evidence required

Create `plans/closure/codegg-legacy-search-parity/002-status.md` with:

- exact implementation SHA(s);
- current provider/API documentation date reviewed;
- Kagi terms disposition;
- final provider inventory/count;
- config/credential/skip-state evidence;
- capability matrix for SerpAPI and Kagi if present;
- malformed/quota/credential-redaction fixture outcomes;
- focused/broad verification results;
- unresolved findings by severity.
