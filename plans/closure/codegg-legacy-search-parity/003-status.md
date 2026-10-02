# CodeGG Legacy Search Parity M003 — Compatibility Qualification and Downstream Handoff

Status: conditionally closed

Qualification candidate: `8e5ec755dae2198a06bc881ecd740597b4da5440`

Source implementation plan:

- `plans/implementation/codegg-legacy-search-parity/003-codegg-retirement-handoff-and-qualification.md`

Source subsystem roadmap:

- `plans/subsystems/codegg-legacy-search-parity-corrective-addendum.md`

Planning baseline: `d16a6c6adb04a5b6e2ccb8fec68987e80a452875` (documented provider
inventory 37). Predecessor closures: M001 `e9103b4`, M002 `5233315`.

Open operational condition: **tagged release publication of the qualifying
surface (v0.4.0)**. One condition, nothing else. See §6.

Applicable ADR: none. M003 adds no MCP tool, no response field, no provider, and
no new cross-milestone architecture decision. Its two production changes are a
correctness fix to existing routing (§3) and a robustness fix to the existing
updater (§8).

## 1. Executive finding

M003 is **conditionally closed**. Every acceptance criterion in plan §9 that
eggsearch controls is met on candidate `8e5ec75`: all 16 legacy CodeGG provider
hints have an exact mapping or an explicit disposition, provider
IDs/count/config/docs/tests agree, an explicit provider request can no longer be
silently converted into automatic routing, the Google News RSS hint is
explicitly retired, the Kagi disposition is final, the CodeGG integration docs
define a minimum downstream version, the ten-tool surface is unchanged, and
`make check` and `make release-check` both pass.

Qualification did not run as a rubber stamp. It found and fixed **three
production defects** (§3, all in one corrective pass) and **three defects that
had been blocking the required verification gates** across two prior closures
(§8). Two of the six are genuine latent bugs that would have shipped.

The one thing not done is cutting the release, which plan §6 explicitly
forbids doing merely to make this plan look closed.

## 2. Migration and disposition matrix (plan §2)

The matrix is derived from implemented provider ids, not from the plan's
expected mapping. Every id below exists in `KNOWN_PROVIDER_IDS`
(`src/core/provider.rs:13`), and the count is asserted by
`tests/provider_capability_contract.rs`, `tests/provider_routing.rs`,
`tests/provider_workstream_regression.rs`, and `tests/docs_provider_inventory.rs`.
The plan's expected mapping turned out to be correct for all 16 hints; the work
was proving it and classifying each row.

| Legacy CodeGG hint | eggsearch id | Class | Evidence |
|--------------------|--------------|-------|----------|
| `auto` | *(omit `providers`)* | `routing` | `resolve_provider_routing` treats an empty list as automatic routing (`src/meta/provider_diagnostics.rs`); an explicit list never reaches that branch (§3) |
| `duckduckgo` | `duckduckgo` | `exact` | descriptor present; journey 1-class coverage in `provider_routing` |
| `mojeek` | `mojeek` | `exact` | descriptor present |
| `openalex` | `openalex` | `exact` | descriptor present (pre-existing) |
| `brave` | `brave` | `exact` | descriptor present; keyless HTML-scrape engine |
| `brave_api` | `brave_api` | `exact` | descriptor present; required-credential; `supports_news: true` |
| `exa` | `exa` | `exact` | descriptor present; required-credential |
| `tavily` | `tavily` | `exact` | descriptor present; required-credential; `supports_news: true` |
| `wikipedia` | `wikipedia` | `exact` | M001 |
| `arxiv` | `arxiv` | `exact` | M001 |
| `pubmed` | `pubmed` | `exact` | M001; optional `NCBI_API_KEY` never gates routing |
| `hn_algolia` | `hn_algolia` | `exact` | M001 |
| `github` (repository discovery) | `github_repositories` | `renamed` | M001; same upstream source, renamed to avoid collision with `github_code` / `github_issues` / `github_releases` |
| `serpapi` | `serpapi` | `exact` | M002; current Google Search contract, `engine=google` pinned |
| `kagi` | `kagi` | `equivalent` | M002; **not** contract-identical — current v1 only |
| `google_news` (RSS) | none | `retired` | no accepted provider; no stand-in id exists |

Class definitions are stated in the docs because the classes are not
interchangeable: the matrix must never imply that an unrelated general provider
is an exact replacement for a removed source. `kagi` is the only `equivalent`
row and `google_news` the only `retired` row. There is deliberately **no**
`capability`-class row carrying a provider id, because no provider can stand in
for the retired hint; news is reached by declaring `intent: "news"` and, where
source control is required, naming a provider that natively advertises news.

News-capable providers were derived from the descriptors, not from prose: exactly
two ids set `supports_news: true`, `brave_api` and `tavily`
(`src/core/provider.rs:705` and the tavily arm). That pair is the only accurate
migration target for a caller that needs news *and* source control.

## 3. Explicit-provider failure semantics (plan §3) — corrective pass

Plan §3 made this a stop condition. The audit confirmed the good news and
found three real defects.

**Confirmed sound.** An explicit `providers` list can never fall back to
automatic routing. The proof is structural, not incidental: the only
"query everything" branch in the whole path is `selected_engines(&[])`
(`src/meta/adapter/mod.rs`), and after an explicit request `req.providers` is
overwritten with the routing decision's `selected_providers`, which for a
non-empty explicit list is always a non-empty deduplicated copy of the request.
The empty branch is therefore unreachable. The CLI is stricter still, using
`anyhow::bail!` on the same conditions.

### 3.1 Defect 1 — silent omission (medium, fixed)

`resolve_explicit_providers` had no branch for a provider that is known, enabled,
and config-available but for which the adapter built no engine. The old
`if is_known && !available { .. } else if !is_known { .. }` chain had no `else`,
so such an id fell through both arms, was placed in `selected_providers`, and
was then dropped a layer down by `select_engines` with only a `warn!`.

The observable failure was a **successful** response that lied:

```json
{"results": [], "providers_queried": [], "providers_failed": [],
 "routing_decision": {"selected_providers": ["local_workspace"],
                      "reason": "using explicitly requested providers"}}
```

`retrieval_summary: null`, no skip record, `isError` false. A model-facing agent
would read this as "searched local_workspace, found nothing". This is the silent
omission plan §3 forbids, and it is exactly the class §10 names a stop
condition. Reachable via `[search.providers] local_workspace = true`, which
config validation accepts because the id is in `KNOWN_PROVIDER_IDS`.

This was reproduced with a failing test **before** the fix
(`web_search_provider_override_with_unbuildable_known_id_errors`, in
`tests/web_search_integration.rs`), which is what confirmed the diagnosis rather
than a code reading.

### 3.2 Defect 2 — missing credentials mislabeled as disabled (low-medium, fixed)

A required-credential provider whose `[search.api.<id>]` entry is enabled but
whose credential environment variable is unset or empty produced
`provider is disabled: <id>`. The suggested repair — flipping a
`[search.providers]` boolean — has **no effect** for an `API_PROVIDER_IDS`
member, because `provider_is_available` routes those ids through
`api_provider_is_configured` only. The operator was sent to fix the wrong thing.

Now classified as `MissingCredential` and the message names the exact variable
to populate. Discriminated by a new `AppConfig::api_provider_credential_missing`
that mirrors `api_provider_is_configured` exactly, so the two cannot drift.

### 3.3 Defect 3 — wrong stable error code (low, fixed)

When every explicitly selected provider was queried and all failed,
`web_search` used `ToolError::internal`, which sets
`ToolErrorCode::Internal` directly and therefore **bypasses**
`inferred_code_for_legacy` — including the `"all providers failed"` branch that
already existed at `src/mcp/tools/common.rs`. An upstream outage was reported as
`internal_error`, which `docs/tool-matrix.md` reserves for server faults. Now
`ToolError::upstream_failed`, yielding the stable `upstream_failed` code. (Note
that legacy inference would *also* have mis-mapped it: the message contains
"providers", and the `"provider"` arm is tested before the
`"all providers failed"` arm.)

### 3.4 Resulting classification

Each requested id now resolves to exactly one typed outcome, via a new
`ProviderRoutingError::skip_code()` that maps onto the existing
`ProviderSkipCode` vocabulary — so the error type, the wire vocabulary, and the
documentation all agree:

| Condition | Variant | `ProviderSkipCode` | Tool code |
|-----------|---------|--------------------|-----------|
| Not a known id | `UnknownProvider` | `unknown_provider` | `provider_unavailable` |
| Known, not enabled/configured | `DisabledProvider` | `disabled_by_user` | `provider_unavailable` |
| Enabled, credential unset | `MissingCredential` | `missing_api_key` | `provider_unavailable` |
| Enabled, configured, no engine | `NotBuilt` | `not_built` | `provider_unavailable` |
| All selected providers queried and failed | *(response)* | — | `upstream_failed` |

In **non-strict** mode each class is now recorded as a typed skip in
`skipped_providers` with `partial` set, instead of being accepted into
`selected_providers` with `skipped_providers: vec![]`. That path previously had
no test at all and was a latent trap: flipping any call site to non-strict would
have reintroduced silent omission.

`repo_search` also stopped re-spelling provider error messages and now defers to
`Display`, removing a duplicated string surface that had already drifted from the
shared `Display` impl.

### 3.5 Negative-path evidence

Twelve new tests, all on the real tool surface or the real public API:

- `tests/provider_routing.rs`: `enabled_but_unbuildable_is_rejected_as_not_built`,
  `enabled_api_provider_without_credential_is_a_missing_credential`,
  `known_but_unenabled_is_still_reported_as_disabled`,
  `unknown_provider_still_reports_unknown`,
  `non_strict_mode_records_typed_skips_instead_of_silent_omission`,
  `fully_routable_request_selects_exactly_what_was_requested`,
  `explicit_disabled_provider_is_rejected_not_routed_to_defaults`,
  `explicit_missing_credential_names_the_environment_variable`,
  `explicit_known_but_unbuildable_provider_is_rejected`,
  `explicit_unroutable_provider_never_narrows_to_other_providers`,
  `explicit_provider_failure_surfaces_upstream_failed_not_internal`.
- `tests/web_search_integration.rs`:
  `web_search_provider_override_with_unbuildable_known_id_errors` (the
  pre-fix reproduction).

A mixed list containing one unroutable id fails the whole request rather than
narrowing to the remainder, which is the correct reading of "the caller asked
for both" and is now pinned.

## 4. Qualification scope (plan §5)

All ten required journeys execute through the same adapter/tool surface used by
MCP — the real `run_web_search` / `run_repo_search` entry points, real
`MetadataSearchAdapter`, real `resolve_provider_routing`, real dispatch, real
tool response serialization. No journey instantiates an engine as its only
evidence.

Journeys 6, 7, and 8 drive the **real engines** against a local `httpmock`
server with a fake operator credential, so credential resolution, routing,
dispatch, HTTP request shaping, response parsing, and the tool response are all
covered end to end. Journeys 1–4 and 5 use the local mock harness registered
under the real provider id, which is the correct tool when a provider has no
operator base-URL override (`wikipedia`, `arxiv`, `hn_algolia` ship without one);
their wire contracts are separately pinned by M001 fixtures.

| # | Journey | Test | Strength |
|---|---------|------|----------|
| 1 | explicit Wikipedia search | `journey_wikipedia_explicit_search` | mock engine, real id; asserts it is the only provider queried |
| 2 | explicit arXiv scholarly | `journey_arxiv_explicit_scholarly_search` | as above |
| 3 | explicit PubMed scholarly | `journey_pubmed_explicit_scholarly_search` | as above |
| 4 | explicit HN search | `journey_hn_algolia_explicit_search` | as above |
| 5 | explicit GitHub repository discovery | `journey_github_repositories_explicit_discovery` (`repo_workflow`) | `repo_search` surface; asserts `groups` contain the repository |
| 6 | explicit SerpAPI, fake credential | `journey_serpapi_explicit_search_with_configured_credential` | **real `SerpapiEngine` + httpmock**; asserts `engine=google`, direct link used rather than the Google redirect, one hit, no widening |
| 7 | explicit Kagi, fake credential | `journey_kagi_explicit_search_with_configured_credential` | **real `KagiEngine` + httpmock**; asserts bearer auth, `data.news` does not leak into cards |
| 8 | news intent, supported provider | `journey_news_intent_uses_a_supported_news_provider` | **real `BraveApiEngine` + httpmock**; asserts the request reaches the *news* vertical, not the general web vertical |
| 9 | unknown provider | `web_search_unknown_provider_returns_error`, `unknown_provider_still_reports_unknown` | errors, never routes to defaults |
| 10 | missing-credential provider | `explicit_missing_credential_names_the_environment_variable` | names the variable; asserted **not** to say "is disabled" |

Journeys 1–4 additionally prove a property worth stating: selecting a keyless
source provider explicitly does **not** widen to other enabled providers, so a
retiring harness gets the source it asked for.

## 5. CodeGG integration contract (plan §4)

Two new *Downstream Retirement Contract* sections cover the eight items plan §4
requires:

- `docs/codegg-integration.md` — the operator/harness-facing document, with the
  matrix, per-id `provider_status` expectations, the four typed rejection
  outcomes, the retirement checklist, and the explicit statement that CodeGG
  native wrappers remain the model-facing API.
- `architecture/codegg-contract.md` §12 — the machine-readable-facing
  architecture contract, fixing the disposition classes, the failure semantics
  table, the stable ten-tool statement, and the retirement prerequisites.

Both state the minimum qualifying version, the provider-status inventory
expectations for the eleven ids the migration depends on, the Google News
non-migration guidance, the Kagi disposition, and that CodeGG may remove
`backend="builtin"` and `fallback_to_builtin` once it pins that release.

**No CodeGG-specific field was added to any MCP request or response.** The
stable surface is still exactly ten tools, verified by `tests/docs_tool_names.rs`
and the `mcp_tools` suite; `providers` and `timeout_ms` remain accepted-but-hidden
from the advertised schema, and the contract explicitly tells harnesses to map
their own intents onto the canonical `goal` / `profile` / `workflow` / `sources`
fields instead.

## 6. Release and version handoff (plan §6)

| Item | Value |
|------|-------|
| Qualification candidate SHA | `8e5ec755dae2198a06bc881ecd740597b4da5440` |
| Intended release version | **0.4.0** (minor: additive provider surface) |
| Current published version | 0.3.9 (`v0.3.9` tag; `eggsearch --version` → `eggsearch 0.3.9`) |
| Expected `--version` output once published | `eggsearch 0.4.0` |
| Final `provider_status` inventory | 44 ids; the 11 migration-relevant ids listed in §2 and in both contracts |
| Release note | `CHANGELOG.md` → `## [Unreleased]` → `### Added`, the "CodeGG external-search parity contract, frozen for 0.4.0" entry |

**`Cargo.toml` was deliberately not bumped to 0.4.0 during M003.** Plan §6 says
M003 "establishes a release candidate" and that the release must not be cut
merely to make the plan look closed; the bump, the changelog cut, qualification
via Actions, `cargo publish`, and tagging are the release action governed by
`docs/release.md` and `skills/eggsearch-release`. Declaring 0.4.0 here keeps the
handoff evidence exact while leaving the maintainer in control of when the
version actually ships. The dry run in §8 validated the current 0.3.9 manifest
including all added files, so nothing about packaging is unverified.

**Operational condition: tagged release publication of v0.4.0.** Downstream
CodeGG parity adoption remains blocked until it clears. This is the only open
item in the milestone.

## 7. Documentation reconciliation (plan §7)

All twelve named files were inspected. Factual state at candidate `8e5ec75`:

| File | State | Action |
|------|-------|--------|
| `plans/000-long-term-specification.md` | 44 ids, correct capabilities | verified, unchanged |
| `plans/001-terminology-and-domain-model.md` | 44 ids | verified, unchanged |
| `architecture/engines.md` | 43 engines / 44 providers, widened native-enforcement matrix | verified, unchanged (M002) |
| `architecture/meta.md` | no numeric inventory claim | verified |
| `architecture/config.md` | 44 built-in, 17 required-key, 3 optional-key | verified, unchanged (M002) |
| `architecture/codegg-contract.md` | — | **§12 Downstream Retirement Contract added** |
| `architecture/maintenance.md` | — | provider_diagnostics next-slice note added (§9) |
| `architecture/testing.md` | stale suite count | `76` → `78`; `provider_routing` focus extended |
| `docs/provider-setup.md` | 44-provider setup | verified, unchanged (M002) |
| `docs/codegg-integration.md` | — | matrix upgraded with disposition classes; **Downstream Retirement Contract added**; TOC corrected (the list had two items numbered 7) |
| `docs/tool-matrix.md` | schema budget, no inventory claim | verified |
| `docs/test-inventory.md` | — | suite focus updated for the three extended suites |
| `README.md`, `AGENTS.md` | 44 providers, native-enforcement matrix | verified, unchanged (M002) |

No historical closure record was rewritten; M001 and M002 records remain
accurate as of their candidates.

## 8. Verification executed (plan §8)

All on the exact qualification candidate `8e5ec75`.

| Gate | Command | Result |
|------|---------|--------|
| Format | `cargo fmt --all -- --check` | pass |
| Lint | `cargo clippy --locked --all-targets --all-features -- -D warnings` | pass, no warnings |
| No-default build | `cargo check --locked --no-default-features` | pass |
| All-features tests | `cargo test --locked --all-features` | 5,620 passed, 23 ignored (81 suites) |
| Mock-feature tests | `cargo test --locked --features mock` | 5,351 passed, 1 ignored (81 suites) |
| Focused: web search | `cargo test --locked --features mock --test web_search_integration` | 56 passed |
| Focused: routing | `cargo test --locked --features mock --test provider_routing` | 83 passed |
| Focused: capability | `cargo test --locked --all-features --test provider_capability_contract` | 22 passed |
| Focused: docs inventory | `cargo test --locked --all-features --test docs_provider_inventory` | pass |
| Focused: fault injection | `cargo test --locked --all-features --test dispatch_fault_injection` | 33 passed |
| Focused: static guards | `cargo test --locked --all-features --test static_guards` | 57 passed |
| Docs build | `make docs-check` | **pass** (previously failing; §8.1) |
| Hygiene | `make hygiene` | pass |
| Dependency policy | `make dependency-policy` | pass |
| Packaging contract | `make packaging-check` | pass, incl. `planning-consistency: ok` |
| Full routine gate | `make check` | **exit 0**, twice |
| Release gate | `make release-check` | **exit 0**, twice — includes release-candidate validation, docs build, release build, and `cargo publish --dry-run --locked` |

`make release-check` passing is itself a milestone: it could not pass at any
point during M001 or M002, and three separate defects had to be fixed to get
there (§8.1–§8.3). Live provider calls were not performed; plan §5 makes them
supplementary, and no test in this milestone requires network or credentials.

### 8.1 `make docs-check` was failing (fixed)

`redundant_explicit_links` on the module doc of `src/core/focus.rs:3`, under
`RUSTDOCFLAGS="-D warnings"`. Because `release-check` includes `docs-check`, and
plan §9 requires `release-check` to pass, this blocked M003 acceptance
outright — deferring it a third consecutive closure was not tenable. Fixed by
dropping the explicit link destination, which the label already resolves (the
module imports `FetchDocument`). One line, no behavior change.

Three sibling links in `evidence_bundle.rs`, `local_backend.rs`, and
`adapter/status.rs` share the shape but are **not** redundant — their
destinations do not resolve from the referring module, so the explicit path is
load-bearing. An initial sweep matched them and broke all three; they were
reverted. This is recorded because the near-miss is the kind of thing a future
maintainer will re-attempt.

### 8.2 Updater `ETXTBSY` flake (fixed — latent bug)

`make check` and `make release-check` failed intermittently. Both M001 and M002
recorded a failing gate and deferred it, without root-causing it. Capturing the
actual error (rather than assuming a timeout, as a first hypothesis suggested)
identified it as:

```
CandidateExecution("Text file busy (os error 26)")
```

from `Command::new(candidate).spawn()` in `verify_candidate`. That is `ETXTBSY`.
The candidate is written, flushed, and `sync_all`'d, and `tempfile::TempPath`
holds no file handle (verified in the vendored source), so no handle is leaking —
the kernel can still report a just-closed writer's file as open for writing. It
reproduces only under the load of ~3,400 parallel lib tests, which is why two
prior closures saw it as an unexplainable mystery across two different tests.

The spawn is extracted into `spawn_candidate`, which retries `ETXTBSY` up to 10
times with a 50ms backoff; every other spawn error is still returned
immediately. This is also a real robustness improvement for users: a downloaded
binary on a busy filesystem can hit the same race, where the failure mode was an
aborted update. Two tests pin that a non-busy error is not retried and that a
valid candidate still spawns. Eight consecutive full-suite runs after the fix
showed no reproduction.

### 8.3 Proptest with a contradictory premise (fixed — latent bug)

`property_fetch_limits::validate_url_accepts_valid_public_http` failed on
`host = "a0.lan"`. The **product is correct**: `src/fetch/limits.rs:420`
deliberately rejects private and reserved names — `.lan`, `.local`,
`.internal`, `.corp`, `.private`, `.home`, `.home.arpa`, `.invalid`, `.test`,
`.localhost` — as an SSRF guard, and has its own unit test for it. The proptest
strategy, however, minted any host matching `[a-z][a-z0-9.-]+\.[a-z]{2,}`,
so it could generate a name the product is *required* to block and then assert
it must be public. The property was unsatisfiable, and proptest had recorded the
case in `tests/property_fetch_limits.proptest-regressions`, after which it
replayed deterministically on every run.

The HTTP strategy now draws its TLD from a public list, matching the discipline
`https_url_strategy` already used in the same file. The property under test is
unchanged: genuinely public hosts must not be reported as private. The sibling
property suites deliberately keep unrestricted TLDs — they assert on URL shape
and deterministic identity, not on public/private classification — and were
verified to still pass.

The regressions file is intentionally kept and committed; proptest recommends
versioning failure cases, and four sibling regression files are already tracked.
It was added by commit `7e2bebb` as an unintended side effect of running the
suite, and is now an accurate record of a real latent defect.

## 9. Invariant and compatibility review

| Invariant | Status | Evidence |
|-----------|--------|----------|
| No new MCP tool, response field, or schema change | held | `docs_tool_names` and `mcp_tools` green; diff adds no wire surface |
| No CodeGG-specific field in the schema | held | §5; the contract forbids it explicitly |
| Exactly ten tools | held | unchanged from M002 |
| Provider inventory consistent across code, config, docs, tests | held | 44 ids; six suites plus `docs_provider_inventory` assert it |
| Explicit provider never becomes automatic routing | **fixed to hold** | §3.1; was silently dropping an unbuildable provider |
| Typed, actionable explicit-provider failures | **improved** | §3.4; missing credentials now name the variable |
| Capability flags match real request mappings | held | unchanged in M003; `provider_capability_contract` green |
| Bounded untrusted I/O; no `reqwest` in production | held | no production I/O added in M003 |
| Sanitization of untrusted text | held | no new untrusted text path |
| Deterministic IDs unchanged | held | no identity change |
| Additive-only evolution | held | one new error-variant set and one new config predicate, both internal; `provider_status` wire values unchanged |
| Ordinary file ceilings and ratchets | held | `provider_diagnostics.rs` hit its 2,200-line ratchet and was brought back under it by relocating tests to the behavioral suite; `maintenance.md` records the next slice |

Compatibility is additive. A harness that already sent
`providers: ["wikipedia"]` and received `unknown_provider` now routes. A harness
that sent a mixed list with one unroutable id previously got a silent partial
result and now gets an error — that is a deliberate, documented behavior change
and the reason the failure semantics are stated so explicitly in both contracts.

## 10. Unresolved findings by severity

| Severity | Finding | Disposition |
|----------|---------|-------------|
| Low, operational | v0.4.0 is not published, so downstream CodeGG cannot yet consume the contract. | **The single open condition** for this milestone. The repository-side work is complete. |
| Low, accepted limitation | `kagi` is an `equivalent` mapping, not exact: current v1 only, `data.search` only, no language field. A harness that depended on v0 field parity must adapt. | Documented in both contracts and in the changelog entry. Not fixable without abandoning the deprecated endpoint. |
| Low, accepted limitation | `serpapi` does not natively enforce freshness, domain filters, news, or result timestamps, so those stay local approximation. | Documented; carried from M002. |
| Low, accepted limitation | `google_news` has no replacement. Callers needing news *and* a specific source are limited to `brave_api` / `tavily`. | Documented as an explicit non-migration. Adding a `google_news` provider would require a currently documented upstream contract, which does not exist. |
| Low, information | `src/core/provider.rs` is a flat descriptor table now past 2,800 lines with 44 arms and no size ratchet. | Recorded again (also noted in M001/M002). The M003 change to descriptor-adjacent logic was minimal and deliberate; a descriptor-table refactor belongs to a future maintenance plan. |
| Low, information | `provider_diagnostics.rs` is 2,168 of its 2,200-line ratchet after the §3 fix. | Headroom is now thin. `maintenance.md` records the next slice: extract the explicit-provider rejection classification. |
| Informational | Three intra-doc links in `evidence_bundle.rs` / `local_backend.rs` / `adapter/status.rs` look like the redundant-link shape but are not. | Deliberately left alone; noted so the near-miss is not re-attempted. |

No medium-or-higher finding is open. Every defect found during qualification
was fixed before closure.

## 11. Registry and roadmap disposition

- Plan 003 status `blocked` → `conditionally closed`, naming the open condition
  and this record.
- Subsystem roadmap: M003 marked conditionally closed; the roadmap's remaining
  work is now entirely operational (publish v0.4.0), with the CodeGG retirement
  workstream explicitly downstream of the tag.
- `plans/registry.md`: the M003 row, the active-roadmap row, and the blocked-work
  section all record the conditional closure and the single condition. The
  "blocked work" section now names the release publication rather than a code
  dependency, because there is no code dependency left.

Recommendation: **conditionally closed**. The repository-side work M003 owns is
complete and verified on candidate `8e5ec75`, and the only thing standing
between this milestone and full closure is the publication of a tagged release —
which is a maintainer release action, not a gap in this workstream.
