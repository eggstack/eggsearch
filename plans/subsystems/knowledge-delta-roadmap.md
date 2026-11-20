# Knowledge-Delta Research Roadmap

Status: M001 ready; M002 and M003 proposed
Date: 2026-11-19
Revised: 2026-11-20 — ADR-0020 and ADR-0021 accepted. M001 is not started. M002 stays proposed.
Planning baseline: `0f52c8fc812c99e8aee75f515f1b1d8dd32a9789` on `feat/auto-20261117-044c4885`
Owner: research retrieval and evidence handoff, with optional local evidence persistence

References: `plans/000-long-term-specification.md` §§1–4, 6–8;
`plans/001-terminology-and-domain-model.md` §§1–3;
`plans/002-long-term-roadmap.md`;
`plans/003-planning-process.md`;
ADR-0001, ADR-0006, ADR-0016; accepted ADR-0020; accepted ADR-0021.

## 1. Purpose and ownership boundary

Surface **candidate** dated changes relevant to a topic and a model's
published knowledge cutoff, including related events that a stale model may
not name. Stay deterministic and keyless by default. The agent interprets
the evidence; eggsearch does not run an LLM or claim knowledge of the
agent's internal memory.

Own a typed knowledge-delta planner on the existing search adapter, a
bundled cutoff snapshot, a bounded evidence selector, and optionally a
persistent local index of evidence a live brief already selected. Do not
own provider execution, fetch-transport policy, general research ranking,
or the agent's orchestration loop. New external sources must go through
existing provider and fetch seams.

## 2. Work classification

- **Invariant:** deterministic output identities and tie ordering; the
  ADR-0020 cutoff comparison rule; bounded I/O, payload, and context cost;
  untrusted-source framing; no false claims of model ignorance, event date,
  source verification, or search completeness; a `live` call that does
  not become a write when an index is enabled.
- **Capability:** a cutoff-aware, source-backed brief; later, one-step
  discovery of an indexed subject the topic did not name.
- **Infrastructure:** bundled cutoff snapshot, typed planner, date
  classifier, optional local index.
- **Polish:** operator diagnostics and storage maintenance after the
  capability gates pass. A catalog importer, feed crawler, and refresh
  scheduler are not polish. They are out of this roadmap.

## 3. Current state

At the planning baseline, `research_search` provides typed subqueries,
source-role grouping, RRF results, claim and gap analysis, and a retrieval
ledger. `Freshness` and `SearchDateRange` are provider filters, not a
cutoff judgment. `fetch` supports bounded extraction. Existing caches are
process-local and are not an event index. No canonical `knowledge_brief`
exists. The specification advertises nine tools and forbids recursive
crawling and answer-generating agents. `AGENTS.md` still says "exactly
10 tools"; the specification and the code-derived inventory control.

## 4. Target architecture

`model ID + topic -> cutoff resolver -> bounded typed subqueries ->
adapter search -> approved bounded fetch -> ADR-0020 classification ->
ranked candidate deltas -> compact MCP handoff`.

The planner is new. It uses `MetadataSearchAdapter`,
`RetrievalAttemptSet`, and `FetchCandidateBuilder`. It is not a
`ResearchWorkflow` variant. The comparison rule lives in ADR-0020.
Snippet prose is not a date source.

Enabling a local index does not change this path. `live` neither reads
nor writes the index. `prefer_live` and `index` are opt-in modes
defined by ADR-0021. Indexed pages and cutoff metadata cannot open a new
network destination or expand privileges. Cross-topic edges are derived
only from already indexed public documents. There is no feed crawler and
no in-server LLM loop.

Cutoff precedence: caller override, operator `[knowledge_delta.cutoffs]`,
bundled snapshot (provider-official, then Models.dev, then OpenRouter),
then unknown. The snapshot records source URL, verification date, and
precision. No runtime importer, and no inference from model family. A
cutoff filters dated evidence. It is not proof of what a model knows.

## 5. Dependency graph

```text
ADR-0020 accepted 2026-11-20 ── hard ──> M001 knowledge brief
M001 closed ───────────────── hard ──> M002 local index
ADR-0021 accepted 2026-11-20 ─ hard ──> M002 local index
M002 closed ───────────────── hard ──> M003 topic discovery
M001 request schema ──────── interface ──> CodeGG and eggui consumers
default-target footprint ──── operational ──> calling M002 shippable
```

ADR-0021 was accepted 2026-11-20. The engine is bundled rusqlite. The
macOS spike did not qualify Linux or Windows; that waiver is in the ADR
and is not M002 work. Those unrun targets remain the operational
footprint gate. ADR-0020 is accepted, so M001 is `ready` and not started.
M002 stays `proposed` because M001 is not closed. M003 stays `proposed`. CodeGG and
eggui may pass `model_id` against the M001 schema. They must not send `scope` or a non-live `refresh`. That
parallel work is not asserted complete.

## 6. Ordered milestones

| Milestone | Primary class | Status | Handoff plan | Dependency and visible exit |
|---|---|---|---|---|
| M001 — Cutoff-aware knowledge brief | capability | ready | `plans/implementation/knowledge-delta/001-cutoff-aware-knowledge-brief.md` | Hard dependency satisfied: ADR-0020 accepted 2026-11-20. Not started. Exit remains a bounded source-backed brief that applies the ADR-0020 date rule and is callable over MCP. |
| M002 — Optional local evidence index | infrastructure | proposed | `plans/implementation/knowledge-delta/002-local-evidence-index.md` | Hard: M001 closed. ADR-0021 accepted 2026-11-20 names bundled rusqlite. Not started. The accepted engine survives restart, does not change a live call, and refuses authenticated bodies. |
| M003 — Source-driven topic discovery | capability | proposed | `plans/implementation/knowledge-delta/003-source-driven-topic-discovery.md` | Hard: M002 closed. The omission fixture returns the labeled indexed delta that direct retrieval missed. Negative controls add none. No crawler or LLM. |

## 7. Cross-cutting requirements

**Protocol and consumers.** M001 changes the advertised inventory only
under accepted ADR-0020. Keep the nine incumbent tools and both fetch
aliases. Full and Research advertise `knowledge_brief`. Code and
Minimal do not. Update contracts, CodeGG docs, the SDK facade, Python
parity, presets, and full/compact schema measurements together. The
implementation change, not this roadmap, amends `plans/000` and adds the
`plans/002` pointer.

**Safety and privacy.** Use existing SSRF, redirect, origin, credential,
and browser-profile policy. Store only selected anonymous public evidence
that `should_cache_response` would accept. Keep untrusted markers.
Partition any index by `anonymous` or the opaque `prof_` ID. Record
policy denials as retrieval outcomes.

**Storage.** M001 has no store. M002 adds opt-in versioned storage for
the engine ADR-0021 accepts. Failures on `live` leave the call
unchanged. `index` and `prefer_live` report degraded coverage. No
extra binary SKU. No feed table.

**Failure and resources.** Bound fan-out, bytes, characters, and
wall-clock time. Unknown metadata, a filter that was not applied, missing
dates, provider failures, a stale index, and partial coverage stay
visible. A no-match result is not a verified absence of developments.
`truncated` means excerpt text was clipped.

**Observability.** Return cutoff provenance, attempts, coverage, basis,
precision, source IDs, truncation, and warnings through the existing
warning-prefix table. No persisted raw prompts or secrets by default.
New delta IDs are `kdelta_` FNV-1a IDs. Source IDs stay `src_`.

**Documentation.** On implementation, update `architecture/research.md`,
`architecture/evidence-workflow.md`, `architecture/mcp.md`,
`architecture/codegg-contract.md`, `architecture/testing.md`,
`docs/tool-matrix.md`, `docs/config.md`, `docs/agent-workflows.md`,
`docs/test-inventory.md`, and the registry. Do not copy those rules
into `AGENTS.md` beyond the tool-count sentence the implementation
change must correct.

## 8. Verification and risks

M001 gates on the ADR-0020 comparison fixtures, not on a recall
threshold. An equal-budget comparison with `research_search` may be
reported and does not gate closure. M003 gates on the omission fixture
and its negative controls. Live precision and recall, if measured, are
labeled ungated.

Key risks: provider dates are often publication dates, not event dates;
model cutoffs are incomplete; a plausible source is not a corroborated
fact; query expansion can add noise; SQLite may enlarge the default
binary. A feed importer is not a mitigation available to these
milestones. Do not add one to close them.

No recursive crawler, autonomous browser interaction, background LLM,
embedding requirement, general-purpose `site_map`, private-data archive,
or new public MCP listener is authorized.

## 9. Completion rule

A milestone is closed only after its exact-candidate implementation meets
the stated exit, the focused and broad gates, the security evidence that
applies, a closure record under
`plans/closure/knowledge-delta/NNN-status.md`, and synchronized registry
and roadmap updates. ADR-0020 was accepted 2026-11-20. M001 is ready and
not started. ADR-0021 was accepted 2026-11-20 with bundled rusqlite.
M002 remains proposed until M001 is closed. The incomplete spike does
not reopen that acceptance.
