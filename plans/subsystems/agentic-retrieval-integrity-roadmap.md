# Agentic Retrieval Integrity and Context Economy Roadmap

Status: closed (M001–M023 closed; M006's named condition discharged by M017)
Planning baseline: `cd81f4d5e966364918fe2be25485b8ebbf424e71` (origin/main, 2026-11-10 UTC)

References: `plans/000-long-term-specification.md` §1, 3, 5, 7, 8; `plans/001-terminology-and-domain-model.md` §1–5; `plans/002-long-term-roadmap.md`; `plans/003-planning-process.md`; accepted ADR-0001, ADR-0006.

Prior closed evidence: `plans/subsystems/deterministic-retrieval-quality-roadmap.md` (M001–M006 closed, including the M004 methodology limitation and the M006 comparative report `docs/retrieval-quality-comparison.md`); `plans/subsystems/standalone-mcp-local-inference-roadmap.md` (M001–M008 closed, compact remains opt-in with a mixed quality disposition); `plans/subsystems/tool-surface-consolidation-roadmap.md` (M001–M007 closed); `plans/subsystems/search-capability-roadmap.md`.

## 1. Purpose and ownership

Close the gap between retrieval **plumbing** and retrieval **policy**. The adapter, dispatch, fetch, cache, and evidence layers are ahead of the layer that decides what to ask, which candidate to keep, which provider to route, and how many bytes the model receives.

Eggsearch owns scraper fidelity, candidate generation/routing policy, fusion and selection, fetch negotiation, response economy, evidence metadata, and MCP advertisement. Hosts own model choice, prompting, tool disclosure, and answer synthesis. This workstream adds no inference, no crawler, and no vendor mandate.

### Work classification

- **Invariant:** keyless HTML metasearch first-class and no-key functional (ADR-0006); inference-free deterministic retrieval; ten MCP tools; stable IDs; sanitization and trust framing; explicit absence-versus-failure ledger; Mode-Off zero-remote-dispatch; bounded I/O and deadlines.
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

## 3. Invariants and non-goals

- HTML scrape engines remain first-class retrieval paths per ADR-0006. Do not replace them with API dependencies, hide them, or label them legacy. Bounded computational challenge solving is allowed under ADR-0009 (proof-of-work only, opt-in, ceilinged); interactive CAPTCHA solving, OCR, solving services, proxy rotation, user-agent spoofing, and unbounded retries remain prohibited.
- No Eggsearch-side inference: no embeddings, vector index, LLM reranker, generated answer, or hosted classification in any execution path. Existing opt-in semantic **providers** are unchanged upstream sources.
- No new MCP tools, no removal or rename of existing tools, no schema-breaking change. Advertisement profiles may hide tools; hidden tools still execute.
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
ADR-0011 fetch document addressing ──> M019 fetch document budget ──> M020 offset addressing and cursor retirement ──> M021 non-HTML document-scoped addressable text
```

## 6. Milestones and exit boundaries

| Milestone | Class | Handoff | Dependencies | User-visible exit |
|---|---|---|---|---|
| M001 — scraper fidelity contract | invariant (corrective) | `plans/implementation/agentic-retrieval-integrity/001-scraper-fidelity-contract.md` | none | A broken upstream parser reports provider failure, never absence; captured fixtures gate every scraper change |
| M002 — search-result cache and coalescing | infrastructure | `002-search-result-cache-and-coalescing.md` | none | Repeated identical search calls within the TTL do not re-hit providers and return identical payloads |
| M003 — markdown-first fetch negotiation | capability | `003-markdown-first-fetch-negotiation.md` | none | Cooperative origins return markdown; tokens drop; caching and fallback stay correct |
| M004 — candidate snapshot ranking benchmark | infrastructure (corrective to DRQ M004) | `004-candidate-snapshot-ranking-benchmark.md` | none | `make eval-retrieval-quality` exercises the real selection path and detects a seeded regression |
| M005 — deterministic intent derivation | infrastructure | `005-deterministic-intent-derivation.md` | none | Intent is derived from the query with fixed precedence and reported; no new required argument |
| M006 — intent-aware provider routing | capability | `plans/implementation/agentic-retrieval-integrity/006-intent-aware-provider-routing.md` | hard M004, M005 | Default routing includes intent-appropriate providers; acceptance evidence recorded before the default flips |
| M007 — cross-result excerpt economy | capability | `plans/implementation/agentic-retrieval-integrity/007-cross-result-excerpt-economy.md` | hard M004 | One response-level excerpt budget with redundancy suppression and reported collapse counts |
| M008 — tool advertisement presets | capability | `plans/implementation/agentic-retrieval-integrity/008-tool-advertisement-presets.md` | interface ADR-0001 ten-tool contract | Config-selected presets shrink `tools/list`; hidden tools still execute; `next_actions` stay callable |
| M009 — long-running progress and pagination | infrastructure | `plans/implementation/agentic-retrieval-integrity/009-long-running-progress-and-pagination.md` | interface MCP 2026-07-28 Tasks extension | Bounded progress reporting and a resumable cursor for long research and fetch payloads |
| M010 — bounded challenge-solving framework and ALTCHA | capability | `plans/implementation/agentic-retrieval-integrity/010-challenge-solving-framework-and-altcha.md` | none (ADR-0009 accepted) | With `[search.challenge]` enabled, Mojeek returns rows inside configured ceilings; disabled, it stays `blocked` |
| M011 — Anubis solver and operator difficulty ceiling | capability | `plans/implementation/agentic-retrieval-integrity/011-anubis-solver-and-difficulty-ceiling.md` | hard M010 | Raising `max_difficulty` is the supported way to pay for a harder challenge; the default ceiling leaves Startpage `blocked` with a named reason |
| M012 — Startpage payload extraction | capability | `plans/implementation/agentic-retrieval-integrity/012-startpage-payload-extraction.md` | hard M011 (capture and live proof) | A solved Startpage SERP yields parsed rows; unsolved stays `blocked`, drift stays `parse_error` |
| M013 — challenge solver conformance corrections | invariant (corrective) | `plans/implementation/agentic-retrieval-integrity/013-challenge-solver-conformance-corrections.md` | none (ADR-0010 accepted) | A prefixed deployment and a `slow` deployment both solve; a spent, over-budget, or unprojectable challenge fails closed with a named reason before any hashing |
| M014 — pass-cookie lifetime and egress binding | capability | `plans/implementation/agentic-retrieval-integrity/014-pass-cookie-lifetime-and-egress-binding.md` | none | A cookie is reused for the lifetime the upstream actually published, clamped by configuration, so repeat searches re-solve far less often |
| M015 — challenge solvability observability | capability | `plans/implementation/agentic-retrieval-integrity/015-challenge-solvability-observability.md` | none | An operator can see solve attempts, outcomes by bound, and cookie cache hits without reading logs, and no secret material appears |
| M016 — Anubis `metarefresh` solver | capability | `plans/implementation/agentic-retrieval-integrity/016-anubis-metarefresh-solver.md` | ADR-0010 | A deployment offering only the no-JavaScript method is reachable with zero hashing, and the echoed payload contains only published values |
| M017 — routing shipped-set evidence and keyword predicate | invariant (corrective to M006) | `plans/implementation/agentic-retrieval-integrity/017-routing-shipped-set-evidence-and-keyword-predicate.md` | hard M004, M005, M006 | The shipped default provider set is measured, so `intent_mode` is decided by evidence; and no routing entry can be one that rejects a keyword query |
| M018 — pagination consistency across pages | invariant (corrective to M009) | `plans/implementation/agentic-retrieval-integrity/018-pagination-consistency-across-pages.md` | hard M009; discharged by M020 | Pages carry comparable content (offset windows), `focus` and aggregates state their scope consistently, and no two pages can contradict each other |
| M019 — fetch document budget and truncation semantics | invariant (corrective) | `plans/implementation/agentic-retrieval-integrity/019-fetch-document-budget-and-truncation-semantics.md` | none (ADR-0011 accepted) | The extracted document exceeds one caller's window, and truncation is reported as two distinct facts so a caller is never told "not truncated" while holding less than the document contains |
| M020 — offset addressing and cursor retirement | capability (interface) | `plans/implementation/agentic-retrieval-integrity/020-offset-addressing-and-cursor-retirement.md` | hard M019 (closed) | A caller reads any window of a fetched document in one call by re-specifying the request with `offset`/`limit`; no opaque token remains in any advertised schema |
| M023 — fetch-side Anubis solve and replay | capability | `plans/implementation/agentic-retrieval-integrity/023-fetch-side-anubis-solve-and-replay.md` | hard M022; interface M014 | With `[search.challenge]` enabled, a fetched Anubis wall is solved once and replayed so the caller receives the document; otherwise the page stays marked |
| M022 — fetch-side challenge classification | invariant (corrective) | `plans/implementation/agentic-retrieval-integrity/022-fetch-side-challenge-classification.md` | none (ADR-0009/ADR-0010 scope unchanged) | A plain HTTP fetch of an Anubis or ALTCHA wall reports `manual_interaction_required: true` with a `fetch_challenge_page` warning instead of a short ordinary document; `render: "auto"` escalates it when a browser is available |
| M021 — non-HTML document-scoped addressable text | invariant (corrective; discharges M020's named condition) | `plans/implementation/agentic-retrieval-integrity/021-non-html-document-scoped-addressable-text.md` | hard M020 (closed) | For every representation, `text_total` is the extracted document rather than the caller's window, so an addressed read never reports a document as complete while text remains unreachable |

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

Deferred without a plan: answer synthesis, embeddings, semantic local code search, recursive crawling, OCR, proxy rotation, and any default-feature change.

## 9. Closure and status

Each milestone needs its own `plans/closure/agentic-retrieval-integrity/NNN-status.md` recording implementation SHA, tests and outputs, measured deltas, unchanged safety evidence, failures, and residual risk. All five records exist and name implementation candidate `c9912fc0e4824c4ec6b1231be2057fb4cd0ba8ab`; the closure commit carries the records, roadmap status, and registry rows together. The roadmap may only mark a milestone closed when its closure record exists and the registry row is updated in the same commit. No milestone is pre-closed. M001 is corrective: it references the scraper behaviour observed at baseline and must record why the existing verification did not catch it. M006–M012, M013, and M016 are implemented and carry closure records; M022 is a fetch-path corrective with its own record. Each closes only through its own record,
and M012's fixture came from a solved session rather than a guessed page shape — which is what
exposed M011's mandatory-`redir` defect.

- M006 is `closed` via M017: the tier-1 lane measures the shipped default provider set for all
  seven disclosed intents, no case regresses, the `security-nvd-first` control is detected, and
  `intent_mode` therefore ships `apply`. Evidence: `plans/closure/agentic-retrieval-integrity/006-status.md`.
- M007–M012 are `closed`: implementation, docs, evidence, closure records, this status, and the
  registry rows all landed in a single closure commit.
- M014 is `closed`: the reuse window is the upstream-published `Max-Age`/`Expires` clamped by
  `cookie_ttl_seconds`, a rejected cookie is evicted, and `doctor` warns when a routed egress meets
  reuse. A live 21-minute run re-solved after the published expiry instead of replaying a dead
  token. Evidence: `plans/closure/agentic-retrieval-integrity/014-status.md`.
- M015 is `closed`: `provider_status` always reports a bounded `challenge_solver` block with a fixed
  ten-key refusal vocabulary, cookie hits/misses, and the last attempt's candidates and wall time.
  Evidence: `plans/closure/agentic-retrieval-integrity/015-status.md`.
- M023 is `closed`: with `[search.challenge]` enabled, `web_fetch` solves a detected Anubis
  challenge once and replays with `spchal-auth`, returning the real document; the Startpage SERP
  was fetched live in 2.32 s with no challenge warning. Evidence:
  `plans/closure/agentic-retrieval-integrity/023-status.md`.
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

## 10. Recorded decisions

These shapes are decided for this workstream. Changing one requires a new milestone or an explicit superseding decision, not a silent deviation.

1. **Scrapers are first-class.** ADR-0006 governs. Reliability work targets parser fidelity, fixtures, and classification; it never replaces a keyless engine with an API or labels it legacy.
2. **Measurement before policy.** Candidate snapshots are tiered: a committed structured-synthetic set as the CI gate, a small committed recorded-real set with provenance, and a larger live-recorded set generated into `target/snapshots/` for release qualification. Only the first two are tracked. A harness that cannot detect a seeded regression is rejected.
3. **Defaults follow the happy path.** Markdown negotiation is default-on with a bounded single fallback, not an opt-in flag. Intent derivation runs without a new required argument and becomes behaviour-affecting only after snapshot non-regression.
4. **Tools may be hidden, never removed.** Advertisement presets are a server configuration knob that filters `tools/list`; hidden tools still execute, `next_actions` are filtered to the active preset, and the ten-tool contract is unchanged.
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
