# Agentic Retrieval Integrity and Context Economy Roadmap

Status: active (M001–M048 closed; M049 active; M050–M052 closed)
Planning baseline: `4de20a0f0d748b89147829a51384e385a5bee388` (origin/main, 2026-11-24 UTC)

References: `plans/000-long-term-specification.md` §1, 3, 5, 7, 8; `plans/001-terminology-and-domain-model.md` §1–5; `plans/002-long-term-roadmap.md`; `plans/003-planning-process.md`; accepted ADR-0001, ADR-0006, ADR-0016.

Prior closed evidence: `plans/subsystems/deterministic-retrieval-quality-roadmap.md` (M001–M006 closed, including the M004 methodology limitation and the M006 comparative report `docs/retrieval-quality-comparison.md`); `plans/subsystems/standalone-mcp-local-inference-roadmap.md` (M001–M008 closed, compact remains opt-in with a mixed quality disposition); `plans/subsystems/tool-surface-consolidation-roadmap.md` (M001–M007 closed); `plans/subsystems/search-capability-roadmap.md`.

## 1. Purpose and ownership

Close the gap between retrieval **plumbing** and retrieval **policy**. The adapter, dispatch, fetch, cache, and evidence layers are ahead of the layer that decides what to ask, which candidate to keep, which provider to route, and how many bytes the model receives.

Eggsearch owns scraper fidelity, candidate generation/routing policy, fusion and selection, fetch negotiation, response economy, evidence metadata, and MCP advertisement. Hosts own model choice, prompting, tool disclosure, and answer synthesis. This workstream adds no inference, no crawler, and no vendor mandate.

### Work classification

- **Invariant:** keyless HTML metasearch first-class and no-key functional (ADR-0006); inference-free deterministic retrieval; the nine-tool canonical inventory established by ADR-0016 and M029, with only the two legacy fetch aliases callable but unadvertised; stable IDs; sanitization and trust framing; explicit absence-versus-failure ledger; Mode-Off zero-remote-dispatch; bounded I/O and deadlines.
- **Capability:** truthful empty-versus-broken provider state; lower token cost per useful evidence; intent-appropriate provider routing; smaller advertised tool surfaces.
- **Infrastructure:** search-result cache and coalescing; captured scraper fixtures; a candidate-snapshot ranking harness; deterministic intent derivation.
- **Polish:** diagnostics, telemetry, and documentation that do not establish a capability boundary.

## 2. Current state and gaps

The baseline has 44 provider IDs, ten MCP tools, RRF fusion, intent reranking, evidence postprocessing, two-tier fetch cache, and a keyless default of `duckduckgo`, `startpage`, `yahoo`. Five generic engines are HTML scrapers (`brave`, `duckduckgo`, `mojeek`, `startpage`, `yahoo`).

Measured or code-evidenced gaps:

- A scraper whose DOM shape changes returns `Ok(vec![])`; `EngineError::ParseFailed` is only produced for invalid selector syntax. Structural drift is therefore recorded as `success_zero_results` and surfaced to agents as absence. No captured-HTML fixtures exist.
- Search results are never cached and concurrent identical calls are not coalesced, so agent retry loops re-hit scrapers.
- `web_fetch` sends no `Accept` header, so markdown-for-agents negotiation (Cloudflare, February 2026) is unused; `should_cache_response` additionally rejects `Vary: Accept`, so negotiation cannot be cached naively.
- `tests/retrieval_quality.rs` scores a pre-merged fixture list and never calls `aggregate_rrf`; ranking changes cannot be measured, which is why DRQ M004 could only conclude "no lift established".
- `intent` rewrites the query and biases reranking but does not influence provider selection; `native_*_unavailable` warnings are the observable symptom.
- Compact advertisement is 18.5–19.4 KB for ten tools (roughly 4.8k tokens); the full profile still defaults `response_detail` to `diagnostic`.
- Excerpt budgets are per card, with no cross-result budget and no redundancy suppression.
- The Anubis clearance cookie is selected by the fixed name `spchal-auth`, but the name is the
  deployment's `--cookie-prefix` plus `-auth`, optionally suffixed by a hash of the cookie
  settings. Three live deployments observed on 2026-11-13 use `spchal-auth`,
  `techaro.lol-anubis-auth`, and a dynamically suffixed name, so a valid solve on a default
  deployment is reported as `pass cookie rejected` (a 302 with an unrecognised name) even though
  the proof was accepted.
- `metarefresh` is implemented against an invented synthetic shape: a
  `<meta http-equiv="refresh">` element and a `challenge` JSON field. A live Anubis v1.28.3 page
  published the refresh only in the `Refresh:` header and the echo token in `randomData`, so both
  assumptions fail.
- `challenge.extensions` is ignored. A page declaring `css-load` is solved, submitted, and rejected
  by the extension validator, and the operator sees `pass cookie rejected` instead of the
  unperformed extension.
- Anubis v1.28's WASM methods (`sha256`, `argon2id`, `hashx`) are outside ADR-0010's evaluated set
  and are refused as `unsupported scheme`; `sha256` is the same primitive and per-candidate cost as
  `fast`.
- The bounded solve reaches only the MCP web family. `repo_fetch`'s remote path contains no
  challenge handling at all, and the CLI `eggsearch fetch` command calls `FetchClient` directly,
  so its payload carries neither `structured_warnings` nor `manual_interaction_required` and an
  Anubis wall reaches the operator as ordinary content.
- `architecture/fetch.md` records that `batch_fetch` does not solve. That note predates the batch
  rework that routed batch web items through the shared web-fetch handler; the code path suggests
  batch now inherits the solve, but no test proves it either way.
- The fetch path detects ALTCHA but never solves it. The ALTCHA solver is coupled to fixed
  same-origin challenge and verify endpoints, so a general page's embedded challenge has no
  reviewed path to a solve.
- A fetch-side pass cookie is never reused. Every challenged fetch pays a full solve, and the
  upstream-published lifetime that M014 made safe to parse is not applied on the fetch path.
- The unified `fetch` envelope charges a repo item its whole serialized response. A
  10,574-character README serializes to 59,555 characters and is omitted at the default 50,000
  budget, while `repo_fetch` returns it; the omitted item still reports `ok: true`, counts as
  fetched, and carries no warning. A 66,928-character response for an eight-line slice of
  `std/src/lib.rs` (the full-file document is included) behaves the same at the default budget.
- Local `repo_search` uses the source-role bonus as an admission floor, so a query matching
  nothing returned five workspace README cards and `local_search_completed_no_matches` cannot
  fire while any README or implementation file exists. `include_local` defaults true even when the
  request names a remote repository, so a scoped query can return unrelated workspace files.
- `security_search` selects only `default_providers` for identifier queries, so an enabled OSV
  provider is never queried for a CVE; the response warns to enable a provider that is already
  enabled, a native result still emits `severity_unavailable`, and native vulnerabilities carry no
  summary, forcing a second fetch.
- The unified `fetch` envelope bounds the readable measure, not the emitted payload: a
  three-item windowed repo batch reported 12,223 characters but serialized 186,812 bytes, and an
  eight-line `std` window carried a 64,158-byte whole-file `document`.
- Local `repo_search` reads only `max_results * 2` path-ranked candidates (ten at defaults), so a
  content-only or symbol query for a symbol outside that window returns no matches although the
  file exists; `git grep` over the same tree takes 0.05 s.

## 3. Invariants and non-goals

- HTML scrape engines remain first-class retrieval paths per ADR-0006. Do not replace them with API dependencies, hide them, or label them legacy. Bounded computational challenge solving is allowed under ADR-0009 (proof-of-work only, opt-in, ceilinged); interactive CAPTCHA solving, OCR, solving services, proxy rotation, user-agent spoofing, and unbounded retries remain prohibited.
- Pass-cookie identity is opaque. The solver replays the cookie pair(s) the pass-challenge response published and never selects a cookie by a hardcoded name; a deployment's prefix and dynamic suffix are data, not protocol constants.
- A challenge extension is refused unless an accepted ADR names it. ADR-0012 proposes `css-load` only; `headless`/BotStopper and every interactive, visual, or code-executing extension stay prohibited.
- No Eggsearch-side inference: no embeddings, vector index, LLM reranker, generated answer, or hosted classification in any execution path. Existing opt-in semantic **providers** are unchanged upstream sources.
- No general MCP tool growth, removal, or rename and no unrelated schema-breaking change. The sole planned exception is M029 under ADR-0016: replace the advertised `web_fetch` and `batch_fetch` entries with one advertised `fetch`, while retaining both old names as callable, unadvertised compatibility aliases. Advertisement profiles may still hide tools; hidden tools still execute.
- Preserve stable FNV-1a IDs, RRF tie ordering unless an accepted measured change says otherwise, sanitization tiers, trust markers, per-provider attempt ledger, capability-skip semantics, deadlines, and Mode-Off remote denial.
- Do not change shipped default features, binary SKUs, or Python wheel behavior. A default behavior change requires its own measured acceptance inside the owning milestone.
- No recursive crawling, automatic link following, billing, or credential requirements in routine CI.

## 4. Measurement architecture

Three strictly separate lanes:

1. **Snapshot lane (routine CI, no network):** committed candidate snapshots replayed through the production selection functions, with frozen judgments. Gates every ranking change and provides negative controls.
2. **Opt-in live observation (manual/dispatch):** bounded keyless queries recording revision, configuration, locale, and outcome hashes. Never required for normal CI.
3. **Host model qualification (consumer-owned):** LM Studio and similar runs stay in the host harness; this workstream may hand off fixtures but does not claim model quality.

Acceptance rule: a selection, routing, or payload change is accepted only when the snapshot lane shows no regression and either a measured improvement or an explicitly recorded no-change decision. A harness that cannot detect a deliberately worse selector is itself a defect.

## 5. Milestone dependency graph

```text
M001 scraper fidelity ──────────────────────────────┐
M002 search result cache ───────────────────────────┤ (independent)
M003 markdown-first fetch ──────────────────────────┘
M004 candidate snapshot benchmark
  +--> M006 intent-aware provider routing (hard, with M005)
  +--> M007 cross-result excerpt economy (hard)
M005 deterministic intent derivation ──> M006
M008 tool advertisement presets (interface ADR-0001)
M009 long-running progress and pagination (interface MCP 2026-07-28 Tasks)
M010 challenge-solving framework + ALTCHA ──> M011 Anubis solver + difficulty ceiling ──> M012 Startpage payload extraction
ADR-0010 solver coverage and cost policy ──> M016 Anubis metarefresh
M013 challenge solver conformance corrections  (ADR-0010 coverage; independent)
M014 pass-cookie lifetime and egress binding    (independent)
M015 challenge solvability observability        (independent)
M017 routing shipped-set evidence               (discharges M006's named condition)
M018 pagination consistency across pages        (corrective to M009)
M022 fetch-side challenge classification        (corrective to the fetch path; independent)
M022 detection ──> M023 fetch-side Anubis solve and replay
M023 ──> M024 deployment-independent pass-cookie replay
M016 ──> M025 metarefresh presentation corrective
M026 unsupported-extension refusal  (independent)
M026 + ADR-0012 ──> M027 css-load extension
ADR-0013 ──> M028 wasm sha256 method
ADR-0016 ──> M029 unified fetch discovery and legacy aliases
ADR-0017 ──> M031 fetch-path challenge solve parity ──> M032 ALTCHA on the general fetch path
M031 ──> M033 fetch-side pass-cookie reuse ──> M034 fetch-side stale pass-cookie recovery
ADR-0018 ──> M034 fetch-side stale pass-cookie recovery
ADR-0019 ──> M035 browser challenge completion
ADR-0019 §4 ──> M044 challenge solving on by default (corrective to M035)
ADR-0006 + ADR-0019 §5 ──> M045 browser-fidelity provider access (closed)
                          └──> M046 fetch browser-escalation parity (closed; corrective to M045)
M039 ──> M047 projection-aware batch-fetch payload budget (closed; corrective)
M048 inline HTML whitespace fidelity (closed; independent)
M045 work package 1 ──> M049 Mojeek non-success block classification (active; canonical gate outstanding)
M029 unified fetch ──> M036 unified fetch repo-item budget and outcome
M006 repo workflow ──> M037 local repo-search relevance and scope
M006 routing ──> M038 security-search native advisory routing
M036 ──> M039 fetch emitted-payload budget (corrective)
M037 ──> M040 local content-aware candidate generation (corrective)
M037 + M040 ──> M043 local repo-search empty-result determinism (corrective; registered by the 2026-11-20 hosted qualification)
ADR-0022 §3 + M051 (closed) ──> M052 agent-facing contract ergonomics (closed; independent)
ADR-0019 ──> puzzle-class and identity milestones not yet registered
M014 acceptance reconciliation ──> M030 expiry-safety criterion (closed; path 1 accepted)
ADR-0011 fetch document addressing ──> M019 fetch document budget ──> M020 offset addressing and cursor retirement ──> M021 non-HTML document-scoped addressable text
```

## 6. Milestones and exit boundaries

| Milestone | Class | Handoff | Dependencies | User-visible exit |
|---|---|---|---|---|
| M001 — scraper fidelity contract | invariant (corrective) | `plans/archive/implementation/agentic-retrieval-integrity/001-scraper-fidelity-contract.md` | none | A broken upstream parser reports provider failure, never absence; captured fixtures gate every scraper change |
| M002 — search-result cache and coalescing | infrastructure | `002-search-result-cache-and-coalescing.md` | none | Repeated identical search calls within the TTL do not re-hit providers and return identical payloads |
| M003 — markdown-first fetch negotiation | capability | `003-markdown-first-fetch-negotiation.md` | none | Cooperative origins return markdown; tokens drop; caching and fallback stay correct |
| M004 — candidate snapshot ranking benchmark | infrastructure (corrective to DRQ M004) | `004-candidate-snapshot-ranking-benchmark.md` | none | `make eval-retrieval-quality` exercises the real selection path and detects a seeded regression |
| M005 — deterministic intent derivation | infrastructure | `005-deterministic-intent-derivation.md` | none | Intent is derived from the query with fixed precedence and reported; no new required argument |
| M006 — intent-aware provider routing | capability | `plans/archive/implementation/agentic-retrieval-integrity/006-intent-aware-provider-routing.md` | hard M004, M005 | Default routing includes intent-appropriate providers; acceptance evidence recorded before the default flips |
| M007 — cross-result excerpt economy | capability | `plans/archive/implementation/agentic-retrieval-integrity/007-cross-result-excerpt-economy.md` | hard M004 | One response-level excerpt budget with redundancy suppression and reported collapse counts |
| M008 — tool advertisement presets | capability | `plans/archive/implementation/agentic-retrieval-integrity/008-tool-advertisement-presets.md` | interface ADR-0001 ten-tool contract | Config-selected presets shrink `tools/list`; hidden tools still execute; `next_actions` stay callable |
| M009 — long-running progress and pagination | infrastructure | `plans/archive/implementation/agentic-retrieval-integrity/009-long-running-progress-and-pagination.md` | interface MCP 2026-07-28 Tasks extension | Bounded progress reporting and a resumable cursor for long research and fetch payloads |
| M010 — bounded challenge-solving framework and ALTCHA | capability | `plans/archive/implementation/agentic-retrieval-integrity/010-challenge-solving-framework-and-altcha.md` | none (ADR-0009 accepted) | With `[search.challenge]` enabled, Mojeek returns rows inside configured ceilings; disabled, it stays `blocked` |
| M011 — Anubis solver and operator difficulty ceiling | capability | `plans/archive/implementation/agentic-retrieval-integrity/011-anubis-solver-and-difficulty-ceiling.md` | hard M010 | Raising `max_difficulty` is the supported way to pay for a harder challenge; the default ceiling leaves Startpage `blocked` with a named reason |
| M012 — Startpage payload extraction | capability | `plans/archive/implementation/agentic-retrieval-integrity/012-startpage-payload-extraction.md` | hard M011 (capture and live proof) | A solved Startpage SERP yields parsed rows; unsolved stays `blocked`, drift stays `parse_error` |
| M013 — challenge solver conformance corrections | invariant (corrective) | `plans/archive/implementation/agentic-retrieval-integrity/013-challenge-solver-conformance-corrections.md` | none (ADR-0010 accepted) | A prefixed deployment and a `slow` deployment both solve; a spent, over-budget, or unprojectable challenge fails closed with a named reason before any hashing |
| M014 — pass-cookie lifetime and egress binding | capability | `plans/archive/implementation/agentic-retrieval-integrity/014-pass-cookie-lifetime-and-egress-binding.md` | none | A cookie is never reused beyond a usable published lifetime or the configured ceiling; expired or malformed lifetimes disable reuse, while providers publishing no lifetime retain the existing ceiling fallback; no solve-count reduction is claimed |
| M015 — challenge solvability observability | capability | `plans/archive/implementation/agentic-retrieval-integrity/015-challenge-solvability-observability.md` | none | An operator can see solve attempts, outcomes by bound, and cookie cache hits without reading logs, and no secret material appears |
| M016 — Anubis `metarefresh` solver | capability | `plans/archive/implementation/agentic-retrieval-integrity/016-anubis-metarefresh-solver.md` | ADR-0010 | A deployment offering only the no-JavaScript method is reachable with zero hashing, and the echoed payload contains only published values |
| M017 — routing shipped-set evidence and keyword predicate | invariant (corrective to M006) | `plans/archive/implementation/agentic-retrieval-integrity/017-routing-shipped-set-evidence-and-keyword-predicate.md` | hard M004, M005, M006 | The shipped default provider set is measured, so `intent_mode` is decided by evidence; and no routing entry can be one that rejects a keyword query |
| M018 — pagination consistency across pages | invariant (corrective to M009) | `plans/archive/implementation/agentic-retrieval-integrity/018-pagination-consistency-across-pages.md` | hard M009; discharged by M020 | Pages carry comparable content (offset windows), `focus` and aggregates state their scope consistently, and no two pages can contradict each other |
| M019 — fetch document budget and truncation semantics | invariant (corrective) | `plans/archive/implementation/agentic-retrieval-integrity/019-fetch-document-budget-and-truncation-semantics.md` | none (ADR-0011 accepted) | The extracted document exceeds one caller's window, and truncation is reported as two distinct facts so a caller is never told "not truncated" while holding less than the document contains |
| M020 — offset addressing and cursor retirement | capability (interface) | `plans/archive/implementation/agentic-retrieval-integrity/020-offset-addressing-and-cursor-retirement.md` | hard M019 (closed) | A caller reads any window of a fetched document in one call by re-specifying the request with `offset`/`limit`; no opaque token remains in any advertised schema |
| M023 — fetch-side Anubis solve and replay | capability | `plans/archive/implementation/agentic-retrieval-integrity/023-fetch-side-anubis-solve-and-replay.md` | hard M022; interface M014 | With `[search.challenge]` enabled, a fetched Anubis wall is solved once and replayed so the caller receives the document; otherwise the page stays marked |
| M022 — fetch-side challenge classification | invariant (corrective) | `plans/archive/implementation/agentic-retrieval-integrity/022-fetch-side-challenge-classification.md` | none (ADR-0009/ADR-0010 scope unchanged) | A plain HTTP fetch of an Anubis or ALTCHA wall reports `manual_interaction_required: true` with a `fetch_challenge_page` warning instead of a short ordinary document; `render: "auto"` escalates it when a browser is available |
| M021 — non-HTML document-scoped addressable text | invariant (corrective; discharges M020's named condition) | `plans/archive/implementation/agentic-retrieval-integrity/021-non-html-document-scoped-addressable-text.md` | hard M020 (closed) | For every representation, `text_total` is the extracted document rather than the caller's window, so an addressed read never reports a document as complete while text remains unreachable |
| M024 — deployment-independent Anubis pass-cookie | invariant (corrective to M011/M023) | `plans/archive/implementation/agentic-retrieval-integrity/024-deployment-independent-anubis-pass-cookie.md` | none; M011/M013/M023 closed, M014 conditional (cache interface only) | A pass response with any configured prefix or dynamic suffix is replayed and cached; no runtime code selects the cookie by a fixed name |
| M025 — `metarefresh` presentation corrective | invariant (corrective to M016) | `plans/archive/implementation/agentic-retrieval-integrity/025-metarefresh-presentation-corrective.md` | ADR-0010; M016 closed | A header-only or presentation-less `metarefresh` page solves from the published `randomData` and a derived wait |
| M026 — unsupported extension refusal | invariant (corrective) | `plans/archive/implementation/agentic-retrieval-integrity/026-anubis-extension-fail-closed.md` | none | A non-empty `extensions` list fails closed before submission with an attributable reason; no extension is authorized |
| M027 — `css-load` extension | capability | `plans/archive/implementation/agentic-retrieval-integrity/027-anubis-css-load-extension.md` | hard ADR-0012 acceptance; hard M026 | A `css-load` challenge is satisfied with one bounded same-origin GET; unknown and `headless` stay refused |
| M028 — WASM `sha256` method | capability | `plans/archive/implementation/agentic-retrieval-integrity/028-anubis-wasm-sha256-method.md` | hard ADR-0013 acceptance; M026 | A `sha256` challenge solves under the fast-equivalent ceiling; `argon2id`/`hashx` stay refused |
| M029 — unified fetch tool and legacy aliases | capability | `plans/archive/implementation/agentic-retrieval-integrity/029-unified-fetch-tool-and-legacy-aliases.md` | interface ADR-0016 accepted | One canonical nine-tool inventory advertises `fetch` for one or many web/repo items; `web_fetch` and `batch_fetch` remain callable with legacy behavior but are absent from discovery and `next_actions`; full and compact schemas are smaller than baseline |
| M030 — M014 expiry-safety acceptance reconciliation | invariant (corrective) | `plans/archive/implementation/agentic-retrieval-integrity/030-m014-solve-count-acceptance-reconciliation.md` | operational: user-selected path 1 | M014's expiry-safety criterion is evidenced; its original solve-count-reduction clause is superseded, not met, and no reduction is claimed |
| M031 — fetch-path challenge solve parity | capability | `plans/archive/implementation/agentic-retrieval-integrity/031-fetch-path-challenge-solve-parity.md` | hard ADR-0017 (accepted) | Every fetch surface that can observe a published challenge attempts one bounded solve and one replay through a single shared implementation; `repo_fetch` and the CLI gain the classification the MCP web family already has; the `batch_fetch` residual is proven by test and its stale note corrected |
| M032 — ALTCHA on the general fetch path | capability | `plans/archive/implementation/agentic-retrieval-integrity/032-altcha-on-the-general-fetch-path.md` | hard ADR-0017 (accepted); hard M031 | A general fetch that meets a published ALTCHA PBKDF2 challenge solves it under the same ceilings and replays once; a challenge or verify endpoint that cannot be derived same-origin fails closed as `unsupported scheme` |
| M033 — fetch-side pass-cookie reuse | capability | `plans/archive/implementation/agentic-retrieval-integrity/033-fetch-side-pass-cookie-reuse.md` | hard ADR-0017 (accepted); hard M031; interface M014 | A fetch-side pass cookie is reused for the issuing origin only, within its published lifetime and the operator ceiling, never past expiry, and is evicted when the upstream rejects it |
| M034 — fetch-side stale pass-cookie recovery | capability | `plans/archive/implementation/agentic-retrieval-integrity/034-fetch-side-stale-pass-cookie-recovery.md` | hard ADR-0017 (accepted); hard ADR-0018 (accepted); hard M031; hard M033 | A fetch that meets a refused cached pass cookie evicts it and recovers in the same call, so the caller never has to await a second fetch to get a document; one solve and one replay of a solved pair set per call still hold |
| M035 — browser challenge completion | capability | `plans/archive/implementation/agentic-retrieval-integrity/035-browser-challenge-completion.md` | hard ADR-0019 (accepted); hard M031, M034 | Under `policy = auto` a solvable wall reaches the solver instead of Chrome, a script-driven wall completes in the browser and returns the document, and an unresolved wall is returned marked rather than as a bare wall |
| M036 — unified fetch repo-item budget and outcome | invariant (corrective) | `plans/archive/implementation/agentic-retrieval-integrity/036-unified-fetch-repo-item-budget-and-outcome.md` | hard M029 (closed); interface adaptive M002 (closed) | A repo item is charged the same readable-text measure as a web item, truncates before it is omitted, and an omitted item is `ok: false`; no item reports success with no content |
| M037 — local repo-search relevance and scope | invariant (corrective) | `plans/archive/implementation/agentic-retrieval-integrity/037-local-repo-search-relevance-and-scope.md` | none; interface M006 (closed) | A local result requires a query, path/file, or symbol match; a no-match query returns an explicit empty state; a remote-scoped request includes local results only when `include_local: true` is explicit |
| M038 — security-search native advisory routing | capability (corrective) | `plans/archive/implementation/agentic-retrieval-integrity/038-security-search-native-advisory-routing.md` | none; interface M006 (closed) | An enabled native advisory provider is selected for identifier queries by default, the availability and severity warnings match reality, and a native vulnerability carries a bounded summary |
| M039 — fetch emitted-payload budget | invariant (corrective to M036) | `plans/archive/implementation/agentic-retrieval-integrity/039-fetch-emitted-payload-budget.md` | hard M036 (closed) | `max_total_chars` bounds the emitted payload; optional `document`/`lines` degrade before the requested text is clipped, and `payload_chars` reports the true emitted size |
| M040 — local content-aware candidate generation | capability (corrective to M037) | `plans/archive/implementation/agentic-retrieval-integrity/040-local-content-aware-candidate-generation.md` | hard M037 (closed) | A content or symbol query finds matches beyond the path-ranked window inside a scan bound; truncation and no-match describe the scan honestly |
| M041 — linked worktree identity resolution and prefer-local honesty | capability (corrective to M037, M040) | `plans/archive/implementation/agentic-retrieval-integrity/041-linked-worktree-identity-resolution.md` | hard M037 (closed); hard M040 (closed) | A checkout in a linked git worktree resolves its remote identity through the common directory, so `prefer_local` returns the local copy instead of silently falling back to a remote fetch; a skipped local copy is stated |
| M042 — fetch item shape diagnostics | invariant (corrective to M029) | `plans/archive/implementation/agentic-retrieval-integrity/042-fetch-item-shape-diagnostics.md` | hard M029 (closed) | A `fetch` item with a missing or unknown `type` tag returns an actionable error naming the accepted shapes; no fetch error names a private Rust type |
| M046 — fetch browser-escalation parity | correctness (corrective to M045) | `plans/archive/implementation/agentic-retrieval-integrity/046-fetch-browser-escalation-parity.md` | hard M045 (closed); interface ADR-0019 (accepted) | Closed at `20c0672ad835796a22b6f4e830757a97753ca49f`: `fetch`, `batch_fetch`, the `fetch` alias, and the CLI escalate once to the browser for the same `429`/`5xx` class that `web_search` already escalates, under a return rule that keeps a genuine outage as `http_status`; refusals, `http_only`, and the CLI surface (no new flag) are unchanged. Closure: `plans/closure/agentic-retrieval-integrity/046-status.md`. |
| M045 — browser-fidelity provider access | correctness | `plans/archive/implementation/agentic-retrieval-integrity/045-browser-fidelity-provider-access.md` | hard ADR-0006, ADR-0019 (accepted); interface M044 (closed) | Closed at `364014c6fd9c29e6dd082915bd334ccf9bc148dd`: a challenge served with a non-success status is classified as `blocked` with the status in the reason, and `brave` and `yahoo` fall back once to a bounded render of the shipped browser and parse the rendered DOM, so both return five results on this egress (4.8–5.7 s and 3.8–4.2 s). The browser presents its own Chrome identity instead of `HeadlessChrome/…`, and valued flags now reach Chromium as key/value pairs. No tool, argument, schema property, configuration key, or provider id changed. Closure: `plans/closure/agentic-retrieval-integrity/045-status.md`. |
| M044 — challenge solving on by default | invariant (corrective to M035, ADR-0019 §4) | `plans/archive/implementation/agentic-retrieval-integrity/044-challenge-solving-on-by-default.md` | hard ADR-0019 (accepted); interface M035 (closed) | Closed at `7b28436ce913bbed52bd7fdc3ba5cf14f9c85f70`: `[search.challenge].enabled` defaults to true at `max_difficulty 6`, `max_attempts 100000000`, `timeout_ms 10000`, so a challenged `mojeek` or `startpage` request returns results without operator configuration (1.6 s / 2.6 s first, then sub-second reuse); `enabled = false` restores the previous behaviour. Closure: `plans/closure/agentic-retrieval-integrity/044-status.md`. |
| M043 — local repo-search empty-result determinism | invariant (corrective to M037, M040) | `plans/archive/implementation/agentic-retrieval-integrity/043-local-repo-search-empty-result-determinism.md` | hard M037 (closed); hard M040 (closed); interface M041 (closed) | A local `repo_search` whose bounded content pass did not run or was clipped is distinguishable from a true no-match, and the content-only positive fixture is deterministic under saturated parallel load |
| M050 — repository tool live-surface corrective | invariant (corrective to M029, M042) | `plans/implementation/agentic-retrieval-integrity/050-repo-tool-live-surface-corrective.md` | hard M029 (closed); interface M042 (closed) | A live sweep of the twelve callable MCP names found that `repo_fetch` reported upstream failures as JSON-RPC internal errors, `repo_map` ignored its documented optional `host` and returned an empty map, and the `repo_map` finalizer discarded the specific forge-failure warning; all three close without changing the tool inventory, aliases, arguments, or error codes |
| M051 — live-surface reliability and robustness correctives | correctness (corrective program) | `plans/implementation/agentic-retrieval-integrity/051-live-surface-reliability-correctives.md` | interface M049 (active); interface M050 (closed) | A release-binary review of the areas M050 deliberately left open yields eight bounded packages: probe budget and health isolation, cold challenge-solve contract, `Retry-After` cooldowns, short JavaScript-gate escalation, parse-failure fingerprints, the `repo_map` generic-notice conflict, field-level `[search]` defaults, and a release-mode live target; no tool, argument, provider, or default-set change |
| M052 — agent-facing contract ergonomics | correctness (ergonomics corrective) | `plans/implementation/agentic-retrieval-integrity/052-agent-facing-contract-ergonomics.md` | no hard dependency; interface ADR-0022 §3; interface M051 (closed) | One goal spelling set is accepted by `repo_search`, `research_search`, and `security_search`; every tool that suggests a follow-up emits `next_actions` whose templates carry the values the response already knows; the advertised `probe` description matches the implementation; exactly one authorised fingerprint re-pin. No tool, alias, argument, provider, or default-set change |

M001–M005 are closed at `c9912fc0e4824c4ec6b1231be2057fb4cd0ba8ab` (baseline `cd81f4d`). Each closure record in
`plans/closure/agentic-retrieval-integrity/` records the implementation evidence, and the closure
commit carries the records, roadmap status, and registry rows together. M004's lane is the accepted
measurement gate, and M005's derived-intent reranking is applied on recorded tier-1 evidence rather
than assumed.

M006–M009 are now planned. M006 and M007 build on M004's measurement lane and M005's derived
intent. M009's blocking question is resolved: rmcp 3.9.0 ships the Tasks runtime
(`TaskManager`, `ServerHandler::get_task`/`update_task`/`cancel_task`, `CallToolResponse::Task`,
`ServerCapabilitiesBuilder::enable_tasks`) and `Peer<RoleServer>::notify_progress`, so the
milestone plans both a core-protocol progress path and a Tasks path gated on client capability,
with a self-describing cursor that needs no negotiation.

## 7. Cross-cutting concerns

- **Protocol and schema:** additive fields only. A new advertised value (for example an error class) must be documented where the existing taxonomy is documented and must not change existing wire strings.
- **Cache correctness:** any new cache dimension must be justified and must not weaken SSRF, redirect, origin, profile, content, or sanitization policy. Partial or deadline-interrupted results are never cached.
- **Bounds:** every new surface (cache entries, fixtures, excerpt budget, cursor payload) needs an explicit byte or count cap and a saturation test.
- **Fixtures and licensing:** captured HTML fixtures are third-party markup; keep them small, redacted of identifiers, and limited to what the parser needs. Snapshot corpora store titles, URLs, and rank only, never page bodies.
- **Documentation:** scraper-first status is stated in `architecture/engines.md` and surfaced as a pointer from `AGENTS.md` and the `skills/` entries. Do not restate the rule in several places.

## 8. Risks and deferred work

Upstream HTML drift is continuous; the fidelity contract reduces the blast radius but does not remove maintenance. Captured fixtures go stale by design and need a refresh path. Markdown negotiation depends on origin cooperation and may reduce table fidelity for some pages. Search caching risks serving stale results to a freshness-sensitive caller if the TTL is unbounded. Intent routing can regress exact queries if precedence is wrong. Advertisement presets can strand a model that needs a hidden tool if `next_actions` are not filtered. The Tasks extension may not be available through the current rmcp version.

Deferred without a plan: answer synthesis, embeddings, semantic local code search, recursive crawling, OCR, proxy rotation. ADR-0019 authorizes a default-on solver and a wider member set; M044 now implements the default-on solver, while the wider member set still has no milestone. Also deferred: the `make-challenge` refresh endpoint (a page re-fetch is sufficient), a rule-hash-keyed cookie cache (eviction already bounds the waste), and cross-origin or later-hop fetch cookies (the one-replay rule stays). Under ADR-0017's proposed decision two further items are recorded as deferred rather than undecided. ADR-0019 now authorizes Anubis `argon2id` and `hashx`, ALTCHA's wider digest set, browser challenge completion, and browser-like identity; M045 implements browser challenge completion and browser-like identity, while both Anubis WASM methods still report `unsupported scheme`. Upstream v1.28.2 (2026-10-23) ships them disabled by default while stating intent to enable them later, and no live deployment was observed publishing either, so demand is unproven; per-member cost evidence is still owed. Moving the solver into `src/core` is deferred until a consumer appears outside the MCP tool and CLI surfaces, because `core` is the pure, HTTP-free foundation and the solver needs transport. The 2026-11-19 review also recorded a capability-concentration risk without registering a milestone: `web_search`, `repo_search`, `security_search`, and `research_search` share the same keyless HTML engines, so a simultaneous block, rate-limit, or parse shift yields zero results on all of them. Failure reporting stays honest; a native fallback beyond the enabled advisory providers is deferred.

## 9. Closure and status

Each milestone needs its own `plans/closure/agentic-retrieval-integrity/NNN-status.md` recording implementation SHA, tests and outputs, measured deltas, unchanged safety evidence, failures, and residual risk. All five records exist and name implementation candidate `c9912fc0e4824c4ec6b1231be2057fb4cd0ba8ab`; the closure commit carries the records, roadmap status, and registry rows together. The roadmap may only mark a milestone closed when its closure record exists and the registry row is updated in the same commit. No milestone is pre-closed. M001 is corrective: it references the scraper behaviour observed at baseline and must record why the existing verification did not catch it. M006–M012, M013, and M016 are implemented and carry closure records; M022 is a fetch-path corrective with its own record. Each closes only through its own record,
and M012's fixture came from a solved session rather than a guessed page shape — which is what
exposed M011's mandatory-`redir` defect.

- M006 is `closed` via M017: the tier-1 lane measures the shipped default provider set for all
  seven disclosed intents, no case regresses, the `security-nvd-first` control is detected, and
  `intent_mode` therefore ships `apply`. Evidence: `plans/closure/agentic-retrieval-integrity/006-status.md`.
- M007–M012 are `closed`: implementation, docs, evidence, closure records, this status, and the
  registry rows all landed in a single closure commit.
- M014 is `closed` under the M030 path-1 decision: reuse is bounded by a usable upstream-published
  `Max-Age`/`Expires` and `cookie_ttl_seconds`; malformed/expired cookies disable reuse, rejected
  cookies are evicted, and `doctor` warns when routed egress meets reuse. The Startpage post-expiry
  run demonstrates safe expiry handling; it is not evidence of fewer solves. The planned solve-count
  comparison was not run and no reduction is claimed. Mojeek published no lifetime and keeps its
  existing ceiling fallback. Evidence: `plans/closure/agentic-retrieval-integrity/014-status.md`
  and `plans/closure/agentic-retrieval-integrity/030-status.md`.
- M029 is `closed` on implementation candidate `3416da6231702dcabc22b2c8e32e94dd88cb20e4`: the nine-tool schema, legacy MCP aliases, mixed web/workspace HTTP execution, schema reduction, pinned Rust 1.89 `make check`, `make docs-check`, and installed-wheel tests pass. The exact-SHA evidence, plan, roadmap, and registry status updates landed in the closure commit. Evidence: `plans/closure/agentic-retrieval-integrity/029-status.md`.
- M031 is `closed` on implementation candidate `7f7219beca61bce2247e7cf9bbaad8ef141467b6`: the bounded Anubis solve moved into the shared `src/challenge_replay/` module, the remote `repo_fetch` path and the CLI `eggsearch fetch` command now attempt a bounded solve and one replay, the recorded `batch_fetch` residual is proven by a behavioral test and its stale documentation corrected, and two static guards keep the fetch layer free of the solver and keep one shared implementation. Pinned Rust 1.89 `make check` passed with 85 suites and 6,374 tests, `make docs-check` passed, and the packaging and planning contracts passed. Fetch-path solves now record under the ledger label `fetch`. Evidence: `plans/closure/agentic-retrieval-integrity/031-status.md`.
- M032 is `closed` on implementation candidate `421d4b2bbb81f22d3e74be1f7f3ff22a5d63c54e`: the shared `src/challenge_replay/` module gained an ALTCHA branch beside the Anubis one, so a general fetch that meets a published ALTCHA PBKDF2 wall attempts one bounded solve and one replay on the MCP web family, the remote `repo_fetch` path, and the CLI. The new `src/challenge_replay/altcha.rs` derives the challenge document and the verification endpoint from the page's own widget publication, same-origin only, and refuses a cross-origin, scheme-changing, credentialed, query-bearing, over-long, or off-shape endpoint before any request is issued. One ALTCHA solve implementation serves the engine side and the fetch side; a published PBKDF2 `cost` above the solver's ceiling is refused as malformed rather than started, because the cost is the work factor of one counter iteration. Two static guards keep the ownership and the same-origin gate, and a property test proves a derived endpoint is never cross-origin. Pinned Rust 1.89 `make check` passed with 85 suites and 6,399 tests, `make docs-check` passed, and the packaging and planning contracts passed. Evidence: `plans/closure/agentic-retrieval-integrity/032-status.md`.
- M033 is `closed` on implementation candidate `5a98682bda4a68ec55dc9e28409fd8269d1f284d`: the shared `src/challenge_replay/` module gained a process-local, origin-keyed pass-cookie cache, so a second challenged fetch to the same origin replays the stored pair set and performs no solve, on the MCP web family, the remote `repo_fetch` path, and the CLI. The reuse window is M014's unchanged rule — the upstream-published lifetime clamped by `cookie_ttl_seconds` — and the cache is bounded to 32 origins and 64 KiB, is never written to disk, and evicts a pair set the upstream rejects. A fetch that crossed an origin stores nothing and reuses nothing, because the issuing origin cannot be established without following the redirect. A new static guard keeps the cache single-owner and disk-free, and the `doctor` egress warning now names the fetch path without adding a warning. Pinned Rust 1.89 `make check` passed with 85 suites and 6,420 tests, `make docs-check` passed, and the packaging and planning contracts passed. Evidence: `plans/closure/agentic-retrieval-integrity/033-status.md`.
- M030 is `closed`: the user selected path 1, re-scoping only M014's solve-count-reduction clause to expiry safety. Existing deterministic tests and the recorded Startpage expiry-boundary observation satisfy the replacement criterion; Mojeek's no-lifetime fallback is unchanged. Solve-count reduction remains unproven. The closure record, plan, roadmap, and registry carry this disposition together. Evidence: `plans/archive/implementation/agentic-retrieval-integrity/030-m014-solve-count-acceptance-reconciliation.md` and `plans/closure/agentic-retrieval-integrity/030-status.md`.
- M035 is `closed` on implementation candidate `55180790c6dc5dbb2ce0d94c186ef88cd7584f48`: under
  `policy = auto` the solver runs before the browser, so a solvable wall no longer reaches Chrome;
  a script-driven wall is completed by polling live `classify_response` instead of four title
  substrings; a wall that does not resolve is returned with `manual_interaction_required: true`
  and a `fetch_challenge_page` warning rather than as a bare document; and
  `provider_status.challenge_solver.providers.<label>` gains a `browser_completions` count. Pinned
  Rust 1.89 `make check` passed with 85 suites and 6,430 tests, `make docs-check` passed, and the
  packaging and planning contracts passed. The polling loop inside a real render is not in the
  default gate. Evidence: `plans/closure/agentic-retrieval-integrity/035-status.md`.
- M036 is `closed` on implementation candidate `76c1156a2c5418d4b9dab4493eddd9232bdd7b8a`
  (pre-commit fingerprint `103dbdf2…` over parent `6146341`): the
  unified `fetch` envelope charges a repo item by readable text plus bounded metadata instead of its
  serialized JSON, clips text before it omits an item, marks an omitted item `ok: false` with
  `response: null`, a budget error, and one structured warning, and names the missing field for an
  incomplete locator. Live probes: the 10,574-character rustfmt README returns at the default
  50,000 budget (10,863 measured characters) where it was omitted before, and an eight-line
  `std/src/lib.rs` slice returns 712 measured characters. Pinned Rust 1.89 `make check` passed
  with 85 suites and 6,445 tests, and `make docs-check` passed. Evidence:
  `plans/closure/agentic-retrieval-integrity/036-status.md`.
- M037 is `closed` on the same implementation candidate: the source-role bonus ranks candidates but no longer
  admits them, so a file needs a query, path/file, content, or symbol match; a no-match query in a
  non-empty workspace returns zero local cards with `local_search_completed_no_matches`; a
  remote-scoped request includes local results only with explicit `include_local: true`; and
  `local_search_truncated` means the result was actually truncated. Live probes: the nonsense query
  returns zero cards with the no-match warning (five README cards before), the scoped `VecDeque`
  request returns nine remote results and zero local cards, and content-only recall remains bounded
  by path/role candidate generation, recorded as residual risk. Pinned Rust 1.89 `make check`
  passed with 85 suites and 6,445 tests. Evidence:
  `plans/closure/agentic-retrieval-integrity/037-status.md`.
- M038 is `closed` on the same implementation candidate: an identifier-bearing `security_search` request with no
  explicit `providers` adds every provider the configuration already enables and that advertises
  native advisory lookup, so an enabled OSV answers a CVE query by default; explicit lists stay
  authoritative; `native_advisory_search_unavailable` and `severity_unavailable` describe what
  actually happened; and a native vulnerability carries an optional sanitized summary bounded to
  500 characters. Live probes: the default CVE query selected `osv` and returned one `critical`
  advisory (CVSS 10.0) with a summary and neither misleading warning; an explicit
  `providers: ["duckduckgo"]` stayed authoritative; a configuration with OSV disabled reported the
  provider as disabled. The additive field grows the pinned advertisement by 37 bytes (69,932 total,
  fingerprint `517d279f73742d36`; compact unchanged). Pinned Rust 1.89 `make check` passed with 85
  suites and 6,445 tests. Evidence:
  `plans/closure/agentic-retrieval-integrity/038-status.md`.
- M039 and M040 are `closed` on implementation candidate
  `10ed7ed08ac47d80e39ca676ea157ecf0728518c`. M039 bounds the emitted fetch payload: the
  three-item probe fell from 186,812 bytes to 37,313 measured characters while the requested
  text stayed intact, and structural drops are marked and warned. M040 adds the bounded local
  second pass: `has_relevance` went from 0 to 4 cards, a symbol hint found `src/process.rs:12`
  without a path hint, and a true miss reports no-match without truncation. Pinned Rust 1.89
  `make check` passed with 85 suites and 6,453 tests. Evidence:
  `plans/closure/agentic-retrieval-integrity/039-status.md` and
  `plans/closure/agentic-retrieval-integrity/040-status.md`.
- M041 and M042 are `closed` on implementation candidate
  `380a55cd7ea7c0b37dc6eff53851926e96e894d0`. M041 resolves a linked git worktree's remote
  identity through the shared `commondir` configuration, so `repo_fetch` with `prefer_local`
  returned the local copy instead of a silent remote fetch, and a skipped local copy is now
  stated; the normal clone is unchanged. M042 makes a malformed `fetch` item name the accepted
  web and repo shapes instead of a serde untagged-enum detail that named a private Rust type.
  Pinned Rust 1.89 `make check` and `make docs-check` passed. Evidence:
  `plans/closure/agentic-retrieval-integrity/041-status.md` and
  `plans/closure/agentic-retrieval-integrity/042-status.md`.
- M015 is `closed`: `provider_status` always reports a bounded `challenge_solver` block with a fixed
  ten-key refusal vocabulary, cookie hits/misses, and the last attempt's candidates and wall time.
  Evidence: `plans/closure/agentic-retrieval-integrity/015-status.md`.
- M023 is `closed`: with `[search.challenge]` enabled, `web_fetch` solves a detected Anubis
  challenge once and replays with `spchal-auth`, returning the real document; the Startpage SERP
  was fetched live in 2.32 s with no challenge warning. Evidence:
  `plans/closure/agentic-retrieval-integrity/023-status.md`.
- M024–M028 are `closed`. M024 makes pass-cookie identity opaque: the solver replays every
  non-deletion pair the pass response set and derives the reuse window from the longest published
  lifetime, so `spchal-auth`, `techaro.lol-anubis-auth`, and dynamic-suffix names work without a
  source or configuration change. M025 makes the `metarefresh` presentation optional and echoes
  `randomData`. M026 parses and bounds `challenge.extensions` and refuses any list it cannot perform,
  with no extension authorized. M027 implements `css-load` under the accepted ADR-0012 as one
  bounded same-origin GET. M028 implements WASM `sha256` under the accepted ADR-0013 with bit
  difficulty mapped by `ceil(bits / 4)`. Evidence:
  `plans/closure/agentic-retrieval-integrity/024-status.md`–`028-status.md`. The v1.28.3
  `anubis.techaro.lol` observation confirmed `randomData`, the optional presentation, `extensions:
  null`, and a dynamic-suffix clearance cookie; `css-load` and `sha256` have no observed deployment
  and are recorded as synthetic-only.
- M013 and M016 are `closed`: the solver now follows ADR-0010's published method registry
  (`slow` as the `fast` alias, `metarefresh`, base prefix, spent refusal, pre-flight cost
  projection, and origin-validated `redir`).
- M022 is `closed`: the plain HTTP fetch path classifies Anubis, ALTCHA, interactive, and
  verification challenge pages, marks them with `manual_interaction_required` and one
  `fetch_challenge_page` warning, and lets `render: "auto"` escalate a detected wall.
- M017 is `closed`: the shipped default provider set is measured in the tier-1 lane, no case
  regresses, the negative control is detected, `intent_mode` ships `apply`, and M006's named
  condition is discharged. The keyword-query allowlist excludes `rustsec` and reports
  `keyword_unsupported`. Evidence: `plans/closure/agentic-retrieval-integrity/017-status.md`.
- M018 is `closed` by discharge: M020 under ADR-0011 replaced its page-packing mechanism and
  emitted the two labelling contracts (`focus_scope`, `aggregate_scope`/`total_groups`/
  `returned_groups`). Evidence: `plans/closure/agentic-retrieval-integrity/018-status.md`.
- M019–M020 restructure fetch addressing under ADR-0011. They supersede the opaque replay cursor
  from M009 in part and the page-budget mechanism from M018 in part: the cursor was unreleased, so
  this is a replacement rather than a deprecation, and M018's `focus_scope`/`aggregate_scope`
  labelling survives while its chunk-packing mechanism is replaced by offset windows.
- M019 is `closed`: extraction runs at the document budget while `max_chars` bounds only the
  returned window, truncation reports document versus window facts separately, and HTML derived
  entries are shared across caller windows. Evidence:
  `plans/closure/agentic-retrieval-integrity/019-status.md`.
- M020 is `closed`: offset addressing and the cursor retirement landed, and
  `plans/closure/agentic-retrieval-integrity/020-status.md` carries the evidence and the discharged
  condition. The condition was recorded as `text_total` being document-scoped only for HTML and
  browser-rendered documents, with PDF and non-HTML bodies bounded by the caller window; M021's
  measurements showed the PDF half was recorded wrongly (PDF blocks are bounded by `PdfLimits` and
  its chunk is built from all blocks, so a six-page fixture reports `text_total` 12,599 against a
  12,000 window) and the non-HTML half was real.
- M021 is `closed`: it renders the non-HTML block set from the document-bounded text, so
  `text_total`, `next_offset` and offset windows mean the same for `text/plain`, CSV, code,
  notebooks and markdown source as for HTML, and an addressed read no longer claims completeness
  while text is unreachable. Evidence: `plans/closure/agentic-retrieval-integrity/021-status.md`.
  That record also carries the PDF findings this work exposed — flag conflation, the hardcoded
  `chunk_0`, the silent `max_pages` cut, and the whole-chunk-per-window payload — which belong to
  the PDF ingestion subsystem, whose ready M005 must be reconciled with it.
- M013–M018 were registered corrective and extension work from the 2026-11-12 live review.
  ADR-0010 is accepted; M013 (published-registry coverage, fail-fast projection, and the
  prefix/spent/`redir` conformance corrections) and M016 (`metarefresh`) are now closed, while
  M014, M015, M017, and M018 remain `ready`. Every method still lands through its own plan with
  fixtures and budget tests; coverage does not inherit permission from the registry.
- Two plan targets were found unreachable during implementation and are recorded rather than
  restated: M008's compact `minimal` preset is 66% of compact `full` rather than under half (the
  remainder is mandatory schema envelope), and M011's difficulty-6 opt-in requires raising
  `max_attempts` alongside `max_difficulty` and the timeouts because the shipped attempt default
  matches the shipped difficulty ceiling of 5.
- M047 and M048 are `closed` on implementation candidate
  `9528d4131591392c0d9f6ec5f0422903e4b7815b`; both closure records cite hosted CI run `45127251813`.
  Evidence: `plans/closure/agentic-retrieval-integrity/047-status.md` and
  `plans/closure/agentic-retrieval-integrity/048-status.md`.
- M050 is `closed` on implementation candidate
  `79ea653a124916de696637adda8ed95798153715`: a live sweep found that
  `repo_fetch` reported upstream failures as JSON-RPC internal errors,
  `repo_map` ignored its documented optional `host`, and the `repo_map`
  finalizer discarded specific forge-failure warnings. All three are fixed with
  three regression tests; the advertised surface grew by 30 bytes for the
  documented host default. Evidence:
  `plans/closure/agentic-retrieval-integrity/050-status.md`.
- M051 is `closed` on the working tree applied to `8fe39cd`: eight bounded
  reliability packages (probe budget and health isolation, cold challenge-solve
  contract, `Retry-After` cooldowns, short JavaScript-gate escalation,
  parse-failure fingerprints, `repo_map` warning consistency, field-level
  `[search]` defaults, and a release-mode live target) landed without changing
  the tool inventory, aliases, argument set, default provider set, or the pinned
  MCP schema surface. Evidence:
  `plans/closure/agentic-retrieval-integrity/051-status.md`.
- M052 is `closed` on commit `02c4163`: one `goal` vocabulary across the three
  search tools, `next_actions` on every tool that suggests a follow-up, follow-up
  templates that carry the values the response already knows,
  and one advertised-fingerprint re-pin (`3fa97faac725dc97` → `678e2a042ab5b39b`,
  33,850 → 33,983 bytes) for the `repo_map` field and the corrected `probe`
  description. No tool, alias, argument, provider, or default-set change. Evidence:
  `plans/closure/agentic-retrieval-integrity/052-status.md`.
- M049 remains `active`: work package 1 is implemented, but the canonical gate is outstanding. Keep
  its classification corrective scoped to the registered keyless behavior and do not infer a new
  paid-provider, credential, or default-enable task. Evidence and acceptance gates:
  `plans/implementation/agentic-retrieval-integrity/049-non-success-block-classification.md`.

## 10. Recorded decisions

These shapes are decided for this workstream. Changing one requires a new milestone or an explicit superseding decision, not a silent deviation.

1. **Scrapers are first-class.** ADR-0006 governs. Reliability work targets parser fidelity, fixtures, and classification; it never replaces a keyless engine with an API or labels it legacy.
2. **Measurement before policy.** Candidate snapshots are tiered: a committed structured-synthetic set as the CI gate, a small committed recorded-real set with provenance, and a larger live-recorded set generated into `target/snapshots/` for release qualification. Only the first two are tracked. A harness that cannot detect a seeded regression is rejected.
3. **Defaults follow the happy path.** Markdown negotiation is default-on with a bounded single fallback, not an opt-in flag. Intent derivation runs without a new required argument and becomes behaviour-affecting only after snapshot non-regression.
4. **Advertisement is the canonical tool surface; hidden tools remain callable.** M008's preset rule still applies to the tools it filters. M029 under ADR-0016 is the narrow exception to its historical ten-tool/no-removal assumption: the canonical inventory replaces the two advertised fetch names with `fetch`, while the old names remain callable aliases but never appear in `tools/list`, canonical metadata, or `next_actions`. No further inventory change is authorized by this exception.
5. **No objective argument.** Retrieval goal is derived deterministically from the query with fixed precedence and reported reasons; callers are never required to state it.
6. **Challenges are solved under bounds, not delegated.** ADR-0009 governs, with method coverage and the cost policy extended by ADR-0010 (see decision 7). Only published, evaluated challenge methods are solvable, only when the operator enables it, and only inside the difficulty, attempt, wall-clock, and solve-rate ceilings. The difficulty ceiling is operator configuration, not a hardcoded engine constant, so the cost/availability trade is explicit. Unsolved, unsupported, and budget-exhausted challenges stay `blocked`; interactive CAPTCHA, OCR, solving services, proxy rotation, user-agent spoofing, and retry widening stay prohibited.

7. **Solver coverage follows the published registry; cost is the control.** ADR-0010 governs.
   Every challenge method a supported deployment publishes is in scope once it has been evaluated
   against the prohibition list — `fast`, `slow` as an alias of `fast`, `preact`, and
   `metarefresh` — because refusing a method by name produces a false `unsupported scheme`. The
   nonce search stays single-threaded deliberately: a Rust solver already pays roughly 1/300th of
   the browser cost Anubis budgeted for, so the operator ceiling must remain the binding control on
   solve rate. Changing either half of that requires a superseding ADR, not a milestone.

8. **Documents are addressed by offsets; collections by index.** ADR-0011 governs. Reading a
   fetched document re-specifies the original request with `offset`/`limit`, with the plan key
   derived server-side and never echoed, so random access costs one call and no opaque state is
   carried. The classic objection to offset pagination — skipping and duplicating under concurrent
   mutation — does not apply, because a fetched document is immutable for its cache TTL and
   content-addressed by an FNV-1a hash that is already part of the cache key. `chunk_id` remains the
   citation address; offsets are the reading address. `text_total` must be truthful, so the document
   is extracted at the internal budget rather than the caller's window.

9. **Pass-cookie identity is opaque; extensions and methods are named, not inherited.** A clearance
   cookie is replayed and cached as the pair(s) the pass-challenge response published, never by a
   hardcoded name. A non-empty extension list fails closed unless an accepted ADR names the
   extension; ADR-0012 proposes `css-load` only, and ADR-0013 proposes WASM `sha256` only.
   `headless`/BotStopper, `argon2id`, and `hashx` remain outside the permitted set until their own
   evaluation.
