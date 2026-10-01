# CodeGG Legacy Search Parity M001 — Keyless Source Provider Parity Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/codegg-legacy-search-parity/001-keyless-source-provider-parity.md`

Source subsystem roadmap:

- `plans/subsystems/codegg-legacy-search-parity-corrective-addendum.md`

Planning baseline: `d16a6c6adb04a5b6e2ccb8fec68987e80a452875`
(documented provider inventory 37). Implementation baseline (head before this
milestone): `16f6442fd1fee658f4eb4ff39232c7b82e6ea23f`.

Implementation commit:

- `e9103b4c50743037552dd0ad94eee003c7ea3d48` — add keyless source-specific
  search providers (M001)

Applicable ADR: none. M001 changes no durable cross-milestone architecture
decision; the provider-kind and optional-key seams it extends were already
established by the search-capability roadmap.

## 1. Executive finding

M001 is closed. All five planned provider IDs (`wikipedia`, `arxiv`, `pubmed`,
`hn_algolia`, `github_repositories`) exist in the canonical inventory, are
built by the normal engine builder with no credentials, are reachable through
explicit provider selection, and are excluded from `default_providers`. The
arXiv repeated-request policy cannot be bypassed by concurrent calls, PubMed
sends no fabricated contact identity, GitHub repository discovery works
keyless with an optional token, and capability flags match preserved behavior.
`make check` is green on the implementation candidate. The legacy
`google_news` RSS source has an explicit, documented non-migration disposition
and no provider id.

## 2. Final provider inventory

`KNOWN_PROVIDER_IDS` grew 37 -> 42. No existing id, serialized capability flag,
or `provider_status` wire value was removed or renamed.

| Provider ID | Kind | Credential posture | Native capabilities claimed | Engine |
|-------------|------|--------------------|-----------------------------|--------|
| `wikipedia` | `json_api` | keyless | `result_timestamps` | `WikipediaEngine` (`src/meta/engines/wikipedia.rs`) |
| `arxiv` | `structured_api` | keyless | `scholarly_search`, `result_timestamps` | `ArxivEngine` (`src/meta/engines/arxiv.rs`) |
| `pubmed` | `json_api` | optional `NCBI_API_KEY`, optional `NCBI_API_EMAIL` | `scholarly_search`, `result_timestamps` | `PubmedEngine` (`src/meta/engines/pubmed.rs`) |
| `hn_algolia` | `json_api` | keyless | `freshness`, `result_timestamps` | `HnAlgoliaEngine` (`src/meta/engines/hn_algolia.rs`) |
| `github_repositories` | `json_api` | optional `GITHUB_TOKEN` | `result_timestamps` | `GithubRepositoriesEngine` (`src/meta/engines/github_repositories.rs`) |

Explicitly **not** claimed by any of the five: `safe_search`, `language`,
`region`, `domain_filters`, `news`, `code_search`, `repo_indexing`,
`issue_search`, `release_search`, `doi_lookup`, `package_metadata`, and every
security flag. `github_repositories` in particular does not claim
`code_search` or `repo_indexing`; repository discovery is not file/code
search, and `github_code` / `github_issues` / `github_releases` are untouched.

Expected-vs-actual count: the plan predicted 42 after M001 alone; actual is 42.

## 3. Provider-model and inventory changes

- `ProviderKind::StructuredApi` added additively in `src/core/provider.rs`,
  serialized as `structured_api`, used only by `arxiv` (Atom/XML). Existing
  variants `html_scrape` / `json_api` / `api_key` / `local` keep their exact
  wire values; `structured_api_kind_serializes_additively` pins that, and
  `arxiv_is_the_only_structured_api_provider_kind` pins exclusive use.
  `src/commands/providers.rs` and `src/commands/doctor.rs` render the new kind
  string.
- `OPTIONAL_API_PROVIDER_IDS` now holds `firecrawl_developer`, `pubmed`,
  `github_repositories`. `CredentialRequirement::Optional` and
  `provider_configured_state(..) == true` keep both keyless.
- `SearchSection::default().providers` gains the five ids with value `false`,
  so availability never depends on environment variables and default fan-out
  is unchanged (`["duckduckgo", "startpage", "yahoo"]`).
- `build_default_engines` constructs all five from the existing shared
  `Arc<eggfetch_core::Client>`; `pubmed` and `github_repositories` additionally
  read an optional `base_url` from their `[search.api.<id>]` section through
  one shared `configured_base_url()` helper.
- `provider_status`, `eggsearch providers`, `doctor`, and the shared probe
  service are descriptor-driven and needed no per-provider code; the
  probe/status contract is verified by `provider_routing`.

## 4. Request/response fixture evidence per provider

All fixtures are network-free (`httpmock` local servers or in-module
constants). Suite: `tests/provider_request_contract.rs` plus module tests in
each new engine file.

| Provider | Happy-path fixture | Failure/limit fixtures |
|----------|--------------------|------------------------|
| `wikipedia` | `wikipedia_request_and_response_contract`: asserts `action=query`, `list=search`, `srsearch`, `srlimit`, `format`, `formatversion=2`; maps `<span class="searchmatch">` markup to plain text and a `srprop=timestamp` to `published_at` | 503 -> `EngineError::BadStatus` naming the engine; 2 MiB+1 body -> `too large`; module tests cover API `error` body, empty `query`, unparseable JSON, and title/URL encoding |
| `hn_algolia` | `hn_algolia_request_and_response_contract`: asserts `query`, `tags=story`, `hitsPerPage`; external URL preferred, `news.ycombinator.com/item?id=` fallback, `created_at` timestamp | 429 -> typed engine error; `{"hits": 5}` -> `invalid JSON` fail-closed; module tests cover empty hits, discarded non-HTTP URLs, budget clamp, and freshness/date-range filter mapping |
| `arxiv` | `arxiv_request_response_and_pacing_contract`: asserts `search_query=all:…`, `max_results`; parses an Atom feed and strips the `v2` version suffix; measures that a repeat call passes through the pacing gate | 503 -> typed engine error; truncated `<feed><entry><title>x</title>` -> `truncated` fail-closed; module tests cover CDATA summaries, end-name mismatch, nested-entry, and unparseable documents |
| `pubmed` | `pubmed_runs_one_esearch_and_one_esummary`: exactly one `esearch.fcgi` hit and one `esummary.fcgi` hit, `db=pubmed`, `tool=eggsearch`; maps title/authors/journal/`epubdate` | A 200 response with no `result` section fails the whole call (no fabricated partial metadata); 503 on `esearch` -> typed engine error; module tests cover `retmax` clamp, unknown/unparseable date shapes, unknown uids, and empty result sets |
| `github_repositories` | `github_repositories_routes_keyless_and_optional_token`: the keyless mock matches only when **no** `authorization` header is present; a second mock asserts `Bearer <token>` when the optional key is supplied | 403 with `x-ratelimit-remaining: 0` -> typed engine error through the standard failure path; 2 MiB+1 body -> `too large`; module tests cover missing descriptions (metadata fallback), non-HTTP urls, and `sort=updated&order=desc` ordering params |

Every response body in all five engines is forged only through the shared
`read_bounded_body()` (2 MiB cap with an upfront `Content-Length` check), and
every failure is a provider-scoped `EngineError` that feeds the existing
health/cooldown path (`429 -> RateLimited`, everything else -> `HttpStatus`).

## 5. Capability matrix evidence

- `tests/provider_capability_contract.rs` grew 8 -> 17 tests. New tests:
  `keyless_source_providers_are_inventory_and_keyless`,
  `keyless_source_providers_stay_out_of_default_fan_out`,
  `scholarly_providers_advertise_scholarly_search`,
  `keyless_source_providers_preserve_result_timestamps`,
  `hn_algolia_freshness_is_native_but_others_are_not`,
  `github_repositories_is_discovery_not_code_or_indexing`,
  `arxiv_is_the_only_structured_api_provider_kind`,
  `optional_keys_never_gate_keyless_routing_for_new_providers`,
  `keyless_source_providers_build_and_route_keyless`.
- Timestamp claims are backed by real preservation, not approximation:
  `wikipedia` `srprop=timestamp`, `arxiv` `published` (fallback `updated`),
  `pubmed` `epubdate` (fallback `pubdate`, normalized from `2024 Feb 3` /
  `2024 Feb` / `2024`), `hn_algolia` `created_at`, `github_repositories`
  `pushed_at` — all written into `SearchResult::published_at` and asserted in
  fixtures.
- `hn_algolia` is the only new provider with native freshness: relative
  `day|week|month|year` and exact `YYYY-MM-DD` ranges map to
  `numericFilters=created_at_i>…` / `created_at_i<=…`, with an exact range
  taking precedence. The other four do not claim it.
- Domain-filter exclusivity still holds: only `exa` and `tavily` advertise
  `supports_domain_filters`.

## 6. arXiv pacing evidence

Requirement: obey the current arXiv API terms ("no more than one request every
three seconds, and limit requests to a single connection at a time") in a way
concurrent calls cannot bypass.

- `src/meta/engines/arxiv.rs` implements `RequestGate` over
  `tokio::sync::Mutex<Option<Instant>>`. Acquiring sleeps until at least
  `interval` has elapsed since the previous request **started**; the returned
  `GateTurn` keeps the lock so only one request is ever in flight.
- `arxiv::shared_gate()` is a process-wide `OnceLock<Arc<RequestGate>>` with a
  3 s interval, and `ArxivEngine` holds that shared instance, so the policy is
  process-wide rather than per engine instance.
- Evidence: module test `gate_serializes_and_spaces_concurrent_turns` runs four
  concurrent turns through one gate and asserts zero overlap plus a
  `>= interval` gap between every consecutive grant instant;
  `shared_gate_is_process_wide` asserts pointer identity and the 3 s interval.
  `tests/provider_request_contract.rs::arxiv_request_response_and_pacing_contract`
  proves the engine path itself goes through the gate by timing two sequential
  `arxiv::search` calls against a mock server.
- This is the smallest provider-local primitive: no generic scheduler, no
  global rate-limiter service, and no change to the shared `eggfetch-core`
  client. It is documented in `architecture/engines.md` under "arXiv Pacing
  Gate" and in `docs/provider-setup.md`.

## 7. PubMed identity and optional-key evidence

- Every E-utilities request sends `tool=eggsearch` plus a real
  `eggsearch/<version> (+https://github.com/eggstack/eggsearch)` User-Agent.
  No fabricated, maintainer, or `example.invalid` identity exists in the tree
  (`identity_is_truthful_and_never_fabricated` asserts the User-Agent has no
  `example.com` contact and that absent optional values emit no `api_key` or
  `email` parameter).
- NCBI's usage policy asks for contact information, so the operator can supply
  a truthful one through the optional `NCBI_API_EMAIL` environment variable
  read once in `build_default_engines`; without it the `email` parameter is
  omitted rather than fabricated. This follows the existing direct-env
  precedent (`NVD_API_KEY`, `SEMANTIC_SCHOLAR_API_KEY`, `SOURCEGRAPH_API_KEY`)
  and avoids adding a credential field to the shared config model for a
  non-secret contact identity. Documented in `docs/provider-setup.md`.
- Optional key: `[search.api.pubmed] api_key_env = "NCBI_API_KEY"`. Absent or
  empty credentials fall back keyless with a startup warning and never yield
  `missing_api_key` (`optional_keys_never_gate_keyless_routing_for_new_providers`,
  plus the shared `optional_api_key`/`optional_api_key_misconfigured` seam).
- Per-call result counts are bounded (`retmax` clamped to 1..=25) for
  interactive agent retrieval, and the metadata phase is a single bounded
  `esummary` batch.

## 8. GitHub keyless and optional-key evidence

- `github_repositories_request...` in-module tests plus
  `github_repositories_routes_keyless_and_optional_token` prove both paths: the
  keyless mock matches only when no `authorization` header is present, and a
  second mock asserts `Authorization: Bearer <token>` when the optional key is
  supplied through `[search.api.github_repositories] api_key_env =
  "GITHUB_TOKEN"`.
- A missing token never makes the provider unroutable and never produces
  `missing_api_key`; rate-limit responses (403/429) become provider-scoped
  engine errors that feed the existing health/cooldown path.
- Requests carry `Accept: application/vnd.github+json` and
  `X-GitHub-Api-Version: 2022-11-28` and are ordered `sort=updated&order=desc`
  for deterministic freshness.
- The provider is independent of forge tree/indexing ownership: it only calls
  the search endpoint and claims no code-search or indexing capability.

## 9. Provider status, config, and docs evidence

- `tests/provider_routing.rs`: `provider_status_lists_all_known_providers` now
  asserts 42 ids including the five new ones;
  `keyless_source_providers_report_routable_when_explicitly_enabled` builds a
  real `ServerState` with the five enabled and asserts each resolves to exactly
  one engine through `select_engines`, is `routable`, and is never `default`;
  `keyless_source_providers_are_skipped_when_disabled` asserts the default
  config reports `disabled_by_user` (not a network failure) and selects no
  engine.
- `tests/config_validation.rs` known-id list extended with the five ids.
- `tests/provider_workstream_regression.rs` inventory count 37 -> 42 plus
  keyless/optional-credential posture assertions.
- `tests/docs_provider_inventory.rs` passes unchanged (all provider names used
  in documented TOML blocks exist in the code inventory).
- `tests/static_guards.rs` passes unchanged; the new engines already satisfy
  the shared transport, bounded-body, and automatic-decompression guards, and
  no new file entered any ratchet list.
- Documentation updated in the same change: `docs/provider-setup.md` (new
  "Source-Specific Keyless Providers" section, arXiv pacing + attribution
  notes, ten-category summary), `docs/config.md` (42-provider table, new kind
  note), `docs/codegg-integration.md` (one-to-one legacy migration table and
  the `google_news` retirement disposition), `architecture/engines.md`
  (inventory, construction, native-enforcement matrix, arXiv pacing gate
  design), `architecture/core.md`, `architecture/overview.md`,
  `architecture/config.md`, `README.md`, `AGENTS.md`, `docs/test-inventory.md`,
  and `skills/eggsearch-architecture/SKILL.md`.
- Canonical long-term documents were reconciled in the **closure** commit, not
  silently during implementation: `plans/000-long-term-specification.md` §5
  (42 known provider ids, structured-API sources, `hn_algolia` native
  freshness) and `plans/001-terminology-and-domain-model.md` §2 (42 IDs). No
  long-term requirement or invariant was rewritten to fit the implementation —
  only the counts the addendum explicitly said to update when the code
  inventory changes.

## 10. Google News disposition (exact)

`google_news` is **not** implemented and has **no** accepted eggsearch provider.
No current documented Google News search/RSS API contract was found during
this milestone, and no undocumented endpoint was imported. Recorded in
`docs/codegg-integration.md` under "Legacy External-Search Provider Migration":

- the exact legacy source id has no accepted eggsearch provider;
- news capability remains available through providers that truthfully
  advertise native news support (`brave_api`, `tavily`) via
  `intent: "news"` on `web_search`;
- the downstream CodeGG harness must remove or explicitly reject the exact
  `google_news` provider hint; `resolve_providers` rejects unknown ids with a
  typed error, and silent remapping to an unrelated engine is forbidden.

No stop condition from plan §11 was triggered; no separate Google provider
plan was created because no supported public contract was discovered.

## 11. Verification executed

All gates run locally against implementation candidate
`e9103b4c50743037552dd0ad94eee003c7ea3d48` (rustc/cargo 1.99.0 locally; CI pins
Rust 1.89).

| Gate | Result |
|---|---|
| `cargo fmt --check` | Pass |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` | Pass; no issues |
| `cargo check --locked --no-default-features` | Pass |
| `cargo test --locked --all-features` | Pass; 5,549 tests, 23 ignored, 81 suites |
| `cargo test --locked --features mock` | Pass; 5,280 tests, 1 ignored, 81 suites |
| `cargo test --locked --all-features --test provider_capability_contract` | Pass; 17 tests |
| `cargo test --locked --all-features --test docs_provider_inventory` | Pass; 1 test |
| `cargo test --locked --features mock --test web_search_integration` | Pass; 55 tests |
| `cargo test --locked --features mock --test provider_routing` | Pass; 63 tests |
| `cargo test --locked --features mock --test provider_request_contract` | Pass; 31 tests |
| `cargo test --locked --all-features --test dispatch_fault_injection` | Pass; 32 tests |
| `cargo test --locked --all-features --test static_guards` | Pass; 54 tests |
| `./packaging/check-repo-hygiene.sh` | Pass |
| `./packaging/check-dependency-policy.sh` | Pass; default graph reqwest-free, all-features reqwest chromiumoxide-only |
| `./packaging/check-contract.sh` | Pass; planning-consistency ok |
| `make check` | **Pass** (fmt, clippy, feature-check, tests, hygiene, dependency-policy, packaging) |
| `make docs-check` | **Fail — pre-existing and unrelated** (see §13, finding L1) |
| Hosted repository CI | Not yet run for this candidate; the local `make check` gate is the recorded evidence |

No network access was required by any test. No live probe against the five
public endpoints was run, and none is required for M001 closure.

## 12. Invariant and compatibility review

| Invariant (plan §4 / addendum §3) | Evidence |
|---|---|
| No MCP tool added or renamed | `docs_tool_names` / `mcp_tool_contract` / `mcp_projection` suites unchanged and green; tool count still 10 |
| Tools call `MetadataSearchAdapter`, never engines | New engines are only reachable through `build_default_engines` -> adapter; no `mcp::tools` file changed |
| New engines share the existing `Arc<eggfetch_core::Client>` | Every engine struct holds the cloned shared client; no new HTTP stack, no `reqwest` in production source (dependency-policy guard) |
| Bounded response bodies | All five use `read_bounded_body` with a 2 MiB cap; oversized fixtures prove rejection |
| Output sanitized and converted to `SearchResult` / `SourceCard` | Engines emit `SearchResult`; the adapter's `sanitize_field` pipeline is unchanged. API HTML fragments are reduced to plain text through `fetch::extract::HtmlExtractor` instead of a bespoke stripper |
| Provider-scoped failures feed health/cooldown | `EngineError` values only; wire fixtures cover 503/429/403/malformed/oversized |
| No new provider in `default_providers` | `keyless_source_providers_stay_out_of_default_fan_out` plus the real `ServerState` routing test |
| Keyless availability independent of environment variables | All five ship `requires_api_key: false`; `provider_configured_state(..) == true` asserted |
| Optional keys raise limits but never gate routing | `optional_keys_never_gate_keyless_routing_for_new_providers`; `missing_api_key` impossible for the two optional-key providers |
| Network-free, credential-free tests | Every new test uses local mocks, in-module fixtures, or `#[tokio::test]` short-circuits |
| No capability bit set merely because eggsearch could approximate it | `keyless_source_providers_are_inventory_and_keyless` asserts the absent flags; freshness claimed only by `hn_algolia`, where the parameter is actually mapped |
| Stable IDs, RRF, tie ordering, evidence bundles, cache, SSRF policy | Untouched; corpus and evidence suites green |
| Additive schema evolution only | `ProviderKind::StructuredApi` is a new variant; no removal or rename |

## 13. Unresolved findings by severity

| Severity | Finding | Disposition |
|---|---|---|
| Critical/High | None identified | — |
| Medium | None identified | — |
| Low, accepted limitation | `pubmed` has no request-pacing gate. Its two E-utilities calls are sequential inside one search and the existing `multiquery_provider_concurrency` cap (default 2) bounds concurrent searches, but a caller that raises that cap could exceed NCBI's unauthenticated 3 requests/second guidance. The plan required pacing only for arXiv, so no gate was added; upstream non-compliance would surface as provider-scoped 429s and cooldown. | Accepted and documented in `architecture/engines.md` |
| Low, accepted limitation | arXiv's 3 s spacing means a second arXiv call in the same process can consume part of the per-engine timeout while queued at the gate. This is inherent to the upstream policy, not a scheduler defect. | Accepted and documented |
| Low, accepted limitation | Wikipedia is the English edition only, and `pubmed`'s result snippet is synthesized citation metadata (ESummary returns no abstract). Both are documented in `docs/provider-setup.md`. | Accepted |
| Low, operational | `make docs-check` fails on local rustdoc 1.99.0 with `redundant_explicit_links` on a pre-existing intra-doc link in `src/core/focus.rs:3`. Verified pre-existing by running the same command in a clean worktree at `16f6442`; `src/core/focus.rs` is untouched by M001 and CI pins Rust 1.89. `make check` (the M001 acceptance gate) does not include `docs-check` and is green. | Out of scope for M001; needs its own corrective pass if the toolchain baseline moves |
| Informational | `src/core/provider.rs` is now 2,741 lines after five descriptor arms plus tests. It carries no size ratchet today and stays under the byte ceiling, but the flat descriptor table is the next obvious decomposition candidate. | Noted for a future maintenance plan |

## 14. Roadmap disposition and registry updates

- M001 status: `ready` -> `closed`; the plan header now records the closure
  record and implementation SHA.
- Subsystem roadmap `codegg-legacy-search-parity-corrective-addendum.md`:
  M001 marked closed with the closure record as its control point; the
  inventory counts in §2/§5 reconciled with the code (42 registered ids after
  M001; 44 remains the target only if `serpapi` and `kagi` are accepted).
- **M002 is now unblocked**: it was hard-blocked solely on the M001
  provider-model interface (provider kinds, optional-key inventory, descriptor
  and builder seams), which is now frozen and closed. `serpapi` and `kagi`
  should follow the same optional/required credential seams and reuse
  `ProviderKind::ApiKey`; M002 does not need a new provider-kind variant unless
  a genuinely new transport shape appears.
- **M003 remains blocked** on M002 plus the operational dependency of a
  qualifying tagged release, so the CodeGG retirement handoff is unchanged.
- Registry updated: dependency-ready table rows for M001 (closed) and M002
  (ready), the active-roadmap row, and the blocked-work section.

Recommendation: **closed**. Every M001 acceptance criterion is satisfied on
the exact candidate `e9103b4`, and the following plans are correctly
unblocked rather than newly created.
