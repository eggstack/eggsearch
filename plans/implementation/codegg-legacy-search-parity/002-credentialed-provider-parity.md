# Plan 002 — Credentialed Provider Parity

Status: blocked

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
