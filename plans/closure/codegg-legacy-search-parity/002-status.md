# CodeGG Legacy Search Parity M002 — Credentialed Provider Parity Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/codegg-legacy-search-parity/002-credentialed-provider-parity.md`

Source subsystem roadmap:

- `plans/subsystems/codegg-legacy-search-parity-corrective-addendum.md`

Planning baseline: `d16a6c6adb04a5b6e2ccb8fec68987e80a452875`
(documented provider inventory 37). Implementation baseline (head before this
milestone): `7beeaec` (M001 closed; inventory 42).

Implementation commit:

- `5233315a` — add credentialed SerpApi and Kagi search providers (M002)

Predecessor closure: `plans/closure/codegg-legacy-search-parity/001-status.md`
(M001, implementation `e9103b4`).

Applicable ADR: none. M002 reuses `ProviderKind::ApiKey`, the existing
required-credential `[search.api.<id>]` seam, and the existing
`build_default_engines` construction path; it introduces no new
cross-milestone architecture decision and no new `ProviderKind` variant.

## 1. Executive finding

M002 is closed. Both planned credentialed general-search providers exist in the
canonical inventory, are implemented against each provider's **current**
documented contract rather than CodeGG's legacy HTTP paths, are explicit opt-in
engines with operator-owned credentials, and are excluded from
`default_providers`. The Kagi terms gate passed with two binding constraints
that are enforced structurally and covered by tests. The legacy Kagi v0
endpoint does not exist anywhere in production code or docs, and SerpApi is
called without buying any extra SERP vertical, experimental mode, or guessed
parameter. `make check` is green on the implementation candidate.

## 2. Contracts revalidated before implementation (reviewed 2026-10-02)

Recorded in the plan as §3.1 before any production code was written, as §3
requires. Sources: `https://serpapi.com/search-api`, `https://serpapi.com/pricing`,
`https://help.kagi.com/kagi/api/search.html`,
`https://help.kagi.com/kagi/api/quick-start.html`, `https://kagi.com/api/docs`
plus its published OpenAPI bundle, and `https://kagi.com/privacy/api` (Kagi API
Terms, effective 2026-09-22). CodeGG source was not used as API authority.

### 2.1 Kagi terms gate — pass

The API Terms grant a limited, non-exclusive, non-transferable license to call
the API and use returned Results "in your own applications and services,
including sending Results to an AI model to answer your users' queries", and
state that integrating Results that way "is not a prohibited transfer". A
local, user-operated MCP server using the operator's own key is therefore
compatible: it is not resale, sublicensing, proxying, publication, a bulk feed,
a dataset, or a substitute for direct API access. The Terms require a separate
paid API arrangement (`https://kagi.com/api/pricing`), which is documented as
operator responsibility.

Two Terms constraints bind the implementation and are evidenced in §6:

1. **No persistent store, cache, or derivative index of Results** beyond the
   transient caching needed to operate the search.
2. **No circumventing of rate limits, quotas, or access controls**, which
   includes no retry amplification.

### 2.2 Recorded material deviations from the plan

| Plan text | Current reality | Narrow adaptation |
|-----------|------------------|-------------------|
| §6 "current documented Kagi Search API v1 endpoint and authentication" | v1 is `POST /api/v1/search` with a JSON body and `Authorization: Bearer`; the `GET …?q=` + `Bot` example on the help page is legacy v0 style | Use `POST` + `Bearer`; three independent current sources (v1 quick start, OpenAPI `securitySchemes` = `bearer`, Kagi's own hosted MCP server) agree |
| §5 "map … freshness/date constraints only where the current API represents them faithfully" | SerpApi's current engine page documents `tbs` only generically, without value syntax; per-engine pages document their own date parameters separately | Do not send `tbs` and do not advertise native freshness for `serpapi`; freshness stays local approximation rather than a guess |
| §5 "map query and bounded result-count semantics" | No documented result-count parameter for the Google engine | Bounded local truncation of `organic_results` |
| §5/§6 config snippets also set `[search.providers] <id> = true` | `effective_provider_ids` filters every `API_PROVIDER_IDS` member out of the `[search.providers]` map, so that line is inert for API-key providers | Docs show only the contract that works (`[search.api.<id>]`), matching `brave_api`/`exa`/`tavily` |
| §7 "expected provider inventory after M001 + M002: 44" | Both providers were accepted | Inventory is 44 |

No new provider kind was needed: `ProviderKind::ApiKey` (frozen by M001) is the
correct kind for both, exactly as the M001 closure predicted.

## 3. Final provider inventory

`KNOWN_PROVIDER_IDS` grew 42 -> 44. No existing id, serialized capability flag,
or `provider_status` wire value was removed or renamed. `API_PROVIDER_IDS` grew
15 -> 17.

| Provider ID | Kind | Credential posture | Native capabilities claimed | Engine |
|-------------|------|--------------------|-----------------------------|--------|
| `serpapi` | `api_key` | required `SERPAPI_API_KEY` | `safe_search`, `language`, `region` | `SerpapiEngine` (`src/meta/engines/serpapi.rs`) |
| `kagi` | `api_key` | required `KAGI_API_KEY` | `safe_search`, `freshness`, `region`, `domain_filters`, `result_timestamps` | `KagiEngine` (`src/meta/engines/kagi.rs`) |

Deliberately **not** claimed by `serpapi`: `freshness` (undocumented `tbs`
value syntax), `domain_filters` (no site-restriction parameter on this engine),
`news` (would require the separately billed `tbm=nws` vertical), and
`result_timestamps` (`organic_results` carry no documented per-result date).
Deliberately **not** claimed by `kagi`: `language` (no language field exists
upstream) and `news` (the news workflow is a separate result collection this
engine never requests).

## 4. Request/response fixture evidence per provider

All fixtures are network-free (`httpmock` local servers or in-module
constants). Suite: `tests/provider_request_contract.rs` plus module tests in
each new engine file.

| Provider | Happy-path fixture | Failure/limit fixtures |
|----------|--------------------|------------------------|
| `serpapi` | `serpapi_wire_contract_pins_google_and_asks_for_no_extras`: asserts `GET /search`, `engine=google`, `q`, `safe=active`, `hl=en-us`, `gl=us`, `api_key`; asserts `tbm`/`async`/`no_cache`/`zero_trace`/`json_restrictor`/`num`/`tbs` are all absent; maps `organic_results` and enforces the local result budget | 429 with an out-of-credits envelope -> `BadStatus` preserving `429` and one attempt; 401 -> `BadStatus` preserving `401`; 2 MiB+1 body -> `too large`; an unparseable `base_url` still produces a credential-free error. Module tests cover whitespace-normalized titles/snippets, URL-segment title fallback, discarded non-HTTP links, and a 200 error envelope yielding zero source cards |
| `kagi` | `kagi_wire_contract_uses_post_v1_bearer_and_the_search_workflow`: asserts `POST /search`, `Authorization: Bearer`, and the JSON body `query`/`workflow=search`/`limit`/`safe_search`/`filters.region`/`filters.after`/`filters.before`/`lens.sites_included`; asserts `extract`, `personalizations`, `format`, `page`, and `timeout` are absent; asserts only `data.search` becomes source cards and `data.news`/`data.code`/`data.interesting_finds` do not | 429 -> `BadStatus` preserving `429` with `assert_hits(1)` (no retry); malformed JSON -> `ParseFailed`; a 200 error envelope with `data: null` -> zero results, no fabricated cards; 2 MiB+1 body -> `too large`; an unparseable `base_url` produces a credential-free error. Module tests cover the freshness cutoffs for all four levels, exact-range precedence, unparsable range drop, lens normalization/dedup, `limit` clamping, and the v1 base/path construction |

Every response body in both engines is forged only through the shared
`read_bounded_body()` (2 MiB cap with an upfront `Content-Length` check), and
every failure is a provider-scoped `EngineError` feeding the existing
health/cooldown path (`429 -> RateLimited`, everything else -> `HttpStatus`).

## 5. Capability matrix evidence

- `tests/provider_capability_contract.rs` grew 17 -> 22 tests. New tests:
  `serpapi_native_enforcement_matches_documented_contract`,
  `kagi_native_enforcement_matches_documented_contract`,
  `credentialed_providers_require_operator_credentials`,
  `credentialed_providers_stay_out_of_default_fan_out`,
  `credentialed_providers_build_only_with_a_resolvable_key`; and
  `domain_filters_are_native_only_for_exa_kagi_and_tavily` was updated to the
  new authoritative set.
- Domain-filter exclusivity now reads `["exa", "kagi", "tavily"]`, derived from
  descriptors rather than hardcoded per provider, so a future claim without a
  request mapping fails the suite.
- Timestamp claims are backed by real preservation: `kagi`
  `data.search[].time` is written into `SearchResult::published_at` and
  asserted in the fixture (with an unparseable value yielding `None`).
- `serpapi` freshness is explicitly pinned as **false** in the contract test
  with the reason recorded, so a future change cannot silently re-add an
  unverified `tbs` mapping without a deliberate test edit.

## 6. Credential, quota, and terms-compliance evidence

- **Credential never in any error path.** SerpApi authenticates with a query
  parameter by upstream contract, so the engine hands the base URL to the
  transport first and adds `api_key` through the parameter builder, which
  cannot fail with a URL-bearing error; the only URL-carrying transport error
  is raised while parsing the base URL, before the key exists. Both engines
  assert this explicitly, including the invalid-`base_url` case, in
  `provider_request_contract`.
- **Quota is provider-scoped, never global.** `credentialed_quota_failure_is_provider_scoped_and_cools_down`
  in `tests/dispatch_fault_injection.rs` shows a 429 from `serpapi` and `kagi`
  plus a 401 from `brave_api` leaving the search answerable from a fourth
  provider, reporting only the three failures, classifying them as
  `rate_limited` / `rate_limited` / `http_status`, and entering cooldown with
  reason `rate limited` only after the documented threshold — so an
  authentication failure is never mislabelled a rate limit.
- **No retry amplification (Kagi Terms).** `eggfetch-core` is built without the
  `logical-retry` feature, so a quota response yields exactly one request
  attempt; the Kagi fixture asserts `hits == 1` on the quota mock, which would
  fail if any retry or circuit policy re-issued the request.
- **No Results persistence (Kagi Terms).** Neither engine caches, stores, or
  indexes provider results; both return `Vec<SearchResult>` only, and no
  provider-specific cache, index, or export path was added. The only
  persistence in the product is the user-requested evidence-bundle artifact,
  which is a caller-visible output rather than a Kagi index or feed. This is
  documented in `docs/provider-setup.md` and `docs/codegg-integration.md`.
- **Static guards.** `tests/static_guards.rs` pins that the Kagi engine contains
  only the v1 base URL and the `/search` suffix (no v0 fragment), that the
  SerpApi engine sends no extra vertical, experimental mode, or guessed
  parameter while still sending `engine=google` and `api_key`, and that the Kagi
  engine requests no billed extras and still uses the bearer scheme. The docs
  guard also asserts none of `docs/provider-setup.md`, `docs/config.md`, or
  `docs/codegg-integration.md` documents the legacy v0 endpoint.

## 7. Configuration, credential, and skip-state evidence

- Both ids are in `API_PROVIDER_IDS` with `ProviderKind::ApiKey` and
  `requires_api_key: true`; `credential_requirement` reports `Required` and
  `is_optional_api_provider` is false for both.
- Neither id appears in `SearchSection::default().providers` or
  `default_providers`. `credentialed_providers_stay_out_of_default_fan_out`
  proves that with no config they are not in `effective_provider_ids` and not
  available, and that even once configured with a resolvable key they stay out
  of default fan-out while remaining reachable through
  `resolve_providers(&["<id>"])`.
- `credentialed_providers_build_only_with_a_resolvable_key` proves the engine
  builder emits a typed `missing_api_key` skip when the referenced environment
  variable does not resolve, and constructs the engine once it does.
- `provider_status` behavior is pinned in `tests/provider_routing.rs`: default
  config reports `enabled=false`, `configured=false`, `routable=false`, and a
  typed skip code; a config with a resolvable key reports
  `enabled=true`, `configured=true`, `routable=true`, and a null skip code.
- Inventory counts are pinned in five suites (`provider_capability_contract`,
  `provider_routing`, `provider_workstream_regression`, `exa`, `tavily`,
  `firecrawl_developer`) and in the operator docs.

## 8. Migration and CodeGG handoff evidence

- `docs/codegg-integration.md` now maps both legacy ids one to one to the new
  eggsearch ids, replacing the previous "none / do not pass this id" rows.
- The harness rules for the retired stack state that both are opt-in
  credentialed providers enabled only through `[search.api.<id>]`, that
  `[search.providers].<id>` does not enable an API-key provider, that neither
  joins `default_providers`, and that a missing key or a quota response
  degrades only that provider.
- Constraint truth is documented per provider, including the two non-obvious
  consequences for a migrating harness: `serpapi` does not enforce freshness,
  domain filters, or news, so a caller relying on the legacy behaviour must use
  a native-news provider or accept local approximation; and `kagi` exposes no
  language field, so language constraints remain local approximation.
- Kagi billing and terms obligations are stated for the operator, including the
  no-resale/no-index/no-rate-limit-circumvention constraints and the
  separate-API-billing requirement.
- The legacy `google_news` non-migration from M001 is unchanged.

## 9. Documentation and operational evidence

Updated in the same change: `docs/provider-setup.md` (two new API-key provider
sections with config, request discipline, capability truth, and terms/billing
notes; category table and count), `docs/config.md` (44-provider table, two rows),
`docs/codegg-integration.md` (migration table, harness rules, constraint truth),
`docs/test-inventory.md` (per-suite counts and focus for all five touched
suites), `architecture/engines.md` (43-engine/44-provider counts, construction
paragraph, widened native-enforcement matrix and its rules, the Kagi/SerpApi
entries), `architecture/core.md` (counts, optional-key list, native-enforcement
matrix), `architecture/overview.md` (counts, generic-web engine list),
`architecture/config.md` (17 required-key ids, three optional-key ids),
`architecture/testing.md` (suite focus for `provider_routing` and
`provider_capability_contract`), `README.md` (count), `AGENTS.md` (native
enforcement matrix, mirroring the code), `skills/eggsearch-architecture/SKILL.md`
(engine/provider counts), and `CHANGELOG.md` under `Unreleased/Added`.

Canonical long-term documents were reconciled in the **closure** commit, not
during implementation: `plans/000-long-term-specification.md` §5 (44 known
provider ids; `serpapi` safe-search/language/region; `kagi`
freshness/region/domain/timestamps; domain-filter set now
`exa`/`kagi`/`tavily`; `serpapi` freshness explicitly local) and
`plans/001-terminology-and-domain-model.md` §2 (44 IDs). No long-term
requirement or invariant was rewritten to fit the implementation — only the
matrix entries the plan itself directed to be documented against actual
request mappings.

## 10. Verification executed

| Gate | Command | Result |
|------|---------|--------|
| Format | `cargo fmt --check` | pass |
| Lint | `cargo clippy --locked --all-targets --all-features -- -D warnings` | pass, no warnings |
| No-default build | `cargo check --locked --no-default-features` | pass |
| All-features tests | `cargo test --locked --all-features` | 5,598 passed, 23 ignored (81 suites) |
| Mock-feature tests | `cargo test --locked --features mock` | 5,329 passed, 1 ignored (81 suites) |
| Focused capability | `cargo test --locked --all-features --test provider_capability_contract` | 22 passed |
| Focused routing | `cargo test --locked --all-features --test provider_routing` | 65 passed |
| Focused wire contract | `cargo test --locked --all-features --test provider_request_contract` | 36 passed |
| Focused fault injection | `cargo test --locked --all-features --test dispatch_fault_injection` | 33 passed |
| Focused static guards | `cargo test --locked --all-features --test static_guards` | 57 passed |
| Docs inventory | `cargo test --locked --all-features --test docs_provider_inventory` | pass |
| Config validation | `cargo test --locked --all-features --test config_validation` | pass |
| Hygiene | `make hygiene` | pass |
| Dependency policy | `make dependency-policy` | pass |
| Packaging contract | `make packaging-check` | pass (incl. `planning-consistency: ok`) |

All suites were run keyless: no test requires a real credential or network
access, and no credential environment variable was set for the run.

**Not run:** `make docs-check` fails on local rustdoc 1.99.0 with
`redundant_explicit_links` on the pre-existing intra-doc link in
`src/core/focus.rs:3`. This is the same pre-existing toolchain failure recorded
in the M001 closure, reproduced in a clean worktree at `16f6442`; M002 does not
touch that file, and `make check` (the M002 acceptance gate) does not include
`docs-check`. Live provider calls were not performed: M002's acceptance criteria
are contract, fixture, and configuration evidence, and the plan makes live calls
optional maintainer evidence that must not become CI.

## 11. Invariant and compatibility review

| Invariant (plan §4) | Status | Evidence |
|----------------------|--------|----------|
| No new MCP tool or response schema | held | Only provider ids, descriptors, and two engine modules; the tool surface is unchanged (`docs_tool_names` passes) |
| Both providers disabled unless explicitly enabled | held | Required-credential API providers; no `[search.providers]` entry; §7 |
| Neither in `default_providers` | held | `credentialed_providers_stay_out_of_default_fan_out` |
| Missing/empty credentials produce provider-scoped skips | held | `credentialed_providers_build_only_with_a_resolvable_key`; `provider_status` skips in `provider_routing` |
| Credentials never in logs, error bodies, evidence, cache keys, or provenance | held | §6; credential-free error assertions for both engines, including request-build failures |
| All HTTP through the shared engine client and bounded helpers | held | Both engines use `build_http_client`-provided `Arc<Client>` and `read_bounded_body()`; static guard asserts the bounded reader is present |
| Failures participate in health/cooldown | held | `credentialed_quota_failure_is_provider_scoped_and_cools_down` |
| Untrusted text sanitized | held | Titles/snippets pass `normalize_whitespace` + `truncate_at_word`; non-HTTP URLs are dropped before card construction |
| Native flags reflect only fields actually mapped upstream | held | §3 and §5; `serpapi` freshness pinned false with reason |
| Tests network-free and credential-free | held | `httpmock` + in-module fixtures only |
| No CodeGG-specific aliases in the MCP schema | held | Ids are upstream provider names; no tool surface change |

Compatibility: additive only. Two new `KNOWN_PROVIDER_IDS` /
`API_PROVIDER_IDS` members, two new descriptor arms, two new engine modules, two
builder arms, two struct/`SearchEngine` impl pairs. No existing
`provider_status` field, capability flag, kind wire value, or default list
changed, so existing harnesses keep working; a harness that already sends
`providers: ["serpapi"]`/`["kagi"]` to a previous release previously received
`unknown_provider` and now routes once configured.

## 12. Unresolved findings by severity

| Severity | Finding | Disposition |
|----------|---------|-------------|
| Low, accepted limitation | SerpApi has no native freshness, domain-filter, news, or result-timestamp enforcement, so those constraints are less strong than the retired legacy stack's behavior for callers who relied on them. This is deliberate: each claim would require either an undocumented parameter value or a separately billed vertical. | Accepted, documented in `docs/provider-setup.md`, `docs/codegg-integration.md`, and the native-enforcement matrix |
| Low, accepted limitation | Kagi's `filters.after`/`filters.before` boundary inclusivity is upstream-defined; eggsearch passes the caller's own bounds through unchanged rather than shifting them by a day. | Accepted, documented in `architecture/engines.md` and the engine doc comment |
| Low, accepted limitation | SerpApi's result budget is a local truncation of one page (~10 organic results), so `max_results` above the page size cannot be filled by a single call. eggsearch does not paginate: `start`-based pagination would multiply operator cost per search. | Accepted, documented; the bounded local truncation is asserted in fixtures |
| Low, accepted limitation | Kagi has no language parameter, so `web_search.language` is not natively enforced for it and is not claimed. | Accepted, documented |
| Low, operational | Kagi v1 authentication differs between two current Kagi surfaces (help page `Bot` vs. OpenAPI/quick-start/hosted-MCP `Bearer`). eggsearch implements `Bearer` per three independent current sources. If Kagi were to remove `Bearer` for v1, the provider would return 401 and degrade to a provider-scoped auth failure. | Recorded; the static guard pins the chosen scheme so any future change is a deliberate edit |
| Low, operational | `make docs-check` fails on local rustdoc 1.99.0 at `src/core/focus.rs:3` (pre-existing, reproduced in a clean worktree at `16f6442`). | Out of scope for M002; needs its own corrective pass if the toolchain baseline moves |
| Informational | `src/core/provider.rs` remains a flat descriptor table now past 2,800 lines after two more descriptor arms. It has no size ratchet today and stays under the byte ceiling. | Noted again for a future maintenance plan (recorded in the M001 closure) |
| Informational | SerpApi's `json_restrictor` could reduce payload size, but it is an undocumented-for-this-engine field with strict failure modes; the 2 MiB bounded read already covers over-read. | Deliberately not used |

## 13. Roadmap disposition and registry updates

- M002 status: `active` -> `closed`; the plan header now records the closure
  record and the implementation SHA.
- Subsystem roadmap `codegg-legacy-search-parity-corrective-addendum.md`: M002
  marked closed with its closure record as the control point, the §2 migration
  rows updated to show both providers as migrated, and the §5 count reconciled
  to the actual 44 registered ids.
- **M003 is now unblocked** by M002's closure. Its remaining dependency is
  operational, not a code dependency: a qualifying tagged release must exist
  before downstream CodeGG can retire its own credentialed provider clients and
  the `google_news` hint.
- Registry updated in the same commit: the dependency-ready row for M002
  (closed, with closure record and implementation SHA), the M003 row (now
  blocked only on the tagged release), the active-roadmap row, and the
  blocked-work section.

Recommendation: **closed**. Every M002 acceptance criterion is satisfied on the
exact candidate `5233315a`: SerpApi is implemented through the current
supported API, Kagi is implemented through current v1 with a documented terms
disposition, no legacy Kagi v0 endpoint exists in code or docs, inventory,
config, and status accurately reflect the resulting provider set, missing
credentials degrade only the selected provider, capability flags match actual
request mappings, both providers remain opt-in, the documentation gives a
retiring harness enough to remove its direct credentialed provider clients, and
`make check` passes.
