# Agentic Retrieval Integrity and Context Economy Roadmap

Status: active (M001–M005 closed at `c9912fc0e4824c4ec6b1231be2057fb4cd0ba8ab`; M006 conditionally closed; M007–M012 closed)
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

Each milestone needs its own `plans/closure/agentic-retrieval-integrity/NNN-status.md` recording implementation SHA, tests and outputs, measured deltas, unchanged safety evidence, failures, and residual risk. All five records exist and name implementation candidate `c9912fc0e4824c4ec6b1231be2057fb4cd0ba8ab`; the closure commit carries the records, roadmap status, and registry rows together. The roadmap may only mark a milestone closed when its closure record exists and the registry row is updated in the same commit. No milestone is pre-closed. M001 is corrective: it references the scraper behaviour observed at baseline and must record why the existing verification did not catch it. M006–M012 are implemented and carry closure records. Each closes only through its own record,
and M012's fixture came from a solved session rather than a guessed page shape — which is what
exposed M011's mandatory-`redir` defect.

- M006 is `conditionally closed`: routing is implemented, tested, and evidence-backed, but
  `intent_mode` ships `off` because the tier-1 lane does not model the shipped default provider set.
  The condition is a shipped-set measurement, not an implementation gap.
- M007–M012 are `closed`: implementation, docs, evidence, closure records, this status, and the
  registry rows all landed in a single closure commit.
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
6. **Challenges are solved under bounds, not delegated.** ADR-0009 governs. Only published computational proof-of-work (Anubis, ALTCHA) is solvable, only when the operator enables it, and only inside the difficulty, attempt, wall-clock, and solve-rate ceilings. The difficulty ceiling is operator configuration, not a hardcoded engine constant, so the cost/availability trade is explicit. Unsolved, unsupported, and budget-exhausted challenges stay `blocked`; interactive CAPTCHA, OCR, solving services, proxy rotation, user-agent spoofing, and retry widening stay prohibited.
