# Knowledge-Delta Research Roadmap

Status: proposed
Draft: yes — planning only, pending architecture decisions
Date: 2026-11-19
Planning baseline: `0f52c8fc812c99e8aee75f515f1b1d8dd32a9789` on `feat/auto-20261117-044c4885`
Owner: research retrieval and evidence handoff, with optional local evidence persistence

References: `plans/000-long-term-specification.md` §§1–4, 6–8;
`plans/001-terminology-and-domain-model.md` §§1–3;
`plans/002-long-term-roadmap.md`;
`plans/003-planning-process.md`;
ADR-0001, ADR-0006, ADR-0016; proposed ADR-0020 and ADR-0021.

## 1. Purpose and ownership boundary

Surface **candidate** dated changes relevant to a topic and a model's
published knowledge cutoff, including related events that a stale model may
not name. Stay deterministic and keyless by default. The agent interprets
the evidence; eggsearch does not run an LLM or claim knowledge of the
agent's internal memory.

Own a typed knowledge-delta workflow on the existing search adapter, a
model-cutoff registry, a bounded evidence selector, and optionally a
persistent local event/source index. Do not own provider execution,
fetch-transport policy, general research ranking, or the agent's orchestration
loop. New external sources must go through existing provider/fetch seams.

## 2. Work classification

- **Invariant:** deterministic output identities and tie ordering; exact
  cutoff provenance/precision; bounded I/O, payload, and context cost;
  untrusted-source framing; no false claims of model ignorance, event date,
  source verification, or search completeness; existing tool compatibility.
- **Capability:** a cutoff-aware, source-backed brief for CodeGG, eggui and
  other MCP callers; later local history recall and cross-topic discovery.
- **Infrastructure:** model metadata registry, typed planner, date-evidence
  selector, optional SQLite/FTS5 index, source and topic relationships.
- **Polish:** corpus tooling, operator diagnostics, storage maintenance, and
  preset/disclosure ergonomics after the capability gates pass.

## 3. Current state

At the planning baseline, `research_search` provides typed subqueries,
source-role grouping, RRF results, claim/conflict/gap analysis, and a retrieval
ledger; `web_search` exposes freshness/date filters with enforcement
telemetry, and `fetch` supports bounded extraction and focus windows.
Existing caches are process-local and do not constitute a persistent
event-history index. No canonical `knowledge_brief` operation exists.
`plans/000-long-term-specification.md` specifies **nine** advertised
canonical tools and forbids recursive crawling and answer-generating agents.
Older prose may still reference the historical ten-tool inventory; actual
code plus the normative spec controls implementation.

## 4. Target architecture

`model ID + topic -> cutoff resolver -> bounded typed subqueries ->
adapter search -> approved bounded fetch/excerpts -> date/provenance
classification -> ranked candidate deltas -> compact MCP handoff`.

When enabled, a local evidence index supplies historical candidates before
live refresh and permits lexical query expansion using observed subjects.
Neither indexed pages nor remotely supplied cutoff metadata can initiate
a new network destination or expand execution privileges. Cross-topic
relationships are derived only from already approved, bounded evidence;
there is no autonomous link traversal or in-server LLM loop.

Cutoff resolver precedence: explicit caller override (with provenance),
operator-configured alias/override, provider-official verified record,
Models.dev snapshot, OpenRouter catalog, then unknown. Importers record
source URL, last verification and date precision; no automatic inference
from model family or guessed release date. A model-specific cutoff is a
temporal filtering hint, not proof of model knowledge/ignorance.

## 5. Dependency graph

```text
proposed ADR-0020 accepted ── hard ──> M001 knowledge brief
M001 closed ───────────────── hard ──> M002 local index
proposed ADR-0021 accepted ── hard ──> M002 local index
M002 closed ───────────────── hard ──> M003 topic discovery
M001 response/index schema ─ interface ──> CodeGG and eggui consumers
release footprint evidence ─ operational ──> M002 distribution
```

All three plans are `proposed` drafts, **not ready** while their hard
dependencies remain open. CodeGG/eggui model-ID passing may be implemented
against the interface contract in parallel but is not asserted complete.

## 6. Ordered milestones

| Milestone | Primary class | Status | Handoff plan | Dependency and visible exit |
|---|---|---|---|---|
| M001 — Cutoff-aware knowledge brief | capability | proposed | `plans/implementation/knowledge-delta/001-cutoff-aware-knowledge-brief.md` | Hard: ADR-0020 accepted. A bounded source-backed brief with explicit cutoff and dating semantics is callable over MCP. |
| M002 — Optional local evidence index | infrastructure | proposed | `plans/implementation/knowledge-delta/002-local-evidence-index.md` | Hard: M001 closed; ADR-0021 accepted. Local index survives restart, enforces retention, reports stale/offline state, and does not weaken live mode. |
| M003 — Source-driven topic discovery | capability | proposed | `plans/implementation/knowledge-delta/003-source-driven-topic-discovery.md` | Hard: M002 closed. Bounded related-topic recall finds withheld developments beyond lexical topic matches, without crawling or LLM inference. |

## 7. Cross-cutting requirements

**Protocol and consumers:** M001 changes the advertised inventory only
under accepted ADR-0020; maintain the nine incumbent tools and both
legacy fetch aliases. Update tool contracts, CodeGG documentation, SDK and
Python parity, tool presets, and full/compact schema measurements together.

**Safety/privacy:** Enforce existing fetch target SSRF, redirect, origin,
credential and browser-profile policies. Store only selected external public
evidence, retain its untrusted markers, validate cutoff metadata and date
parses, prevent localhost/private-network traversal, and record policy
denials as retrieval outcomes.

**Storage and migrations:** M001 has no persistent store. M002 adds only
opt-in versioned storage; failures degrade explicitly to bounded live search
or a marked partial result, not silent freshness claims. No extra binary SKU.

**Failure and resources:** Every phase has bounded fan-out, byte/character,
wall-clock and index budgets. Unknown metadata, unsupported freshness
filters, missing dates, provider failures, stale indexes, and partial
coverage remain visible. Never equate a no-match result with a verified
absence of developments.

**Observability:** Return selected cutoff provenance, attempts, coverage,
date basis, source IDs, truncation, and warnings. No persisted raw prompts
or secrets by default. Stable IDs remain content-derived.

**Documentation and operations:** Maintain
`architecture/research.md`, `architecture/evidence-workflow.md`,
`architecture/mcp.md`, `architecture/codegg-contract.md`,
`docs/tool-matrix.md`, `docs/config.md`, `docs/agent-workflows.md`,
`docs/test-inventory.md` and planning registry on implementation.
Contributors should follow owner docs, not copy rules into `AGENTS.md`.

## 8. Verification and risks

Build a dated fixture corpus spanning software releases, compatibility,
infrastructure, public policy and science, with intentionally ambiguous,
missing and conflicting dates. Compare `knowledge_brief` against the
existing `research_search` baseline under equal provider/fetch budgets:
evidence-backed recall@k, precision@k, source-link integrity, post-cutoff
date-classification accuracy, provider requests, latency, and payload size.
Provide negative controls (no material changes) and withheld related-topic
cases. Report measurements rather than assuming superiority.

Key risks: provider date metadata is publication rather than event date;
model cutoffs are incomplete; plausible sources are not corroborated facts;
broad query expansion can become noisy; SQLite may enlarge binaries; and
continuous feeds can leak interests or violate upstream policy.

No recursive crawler, autonomous browser interaction, background LLM,
embedding requirement, general-purpose `site_map`, private-data archive,
or new public MCP listener is authorized.

## 9. Completion rule

A milestone is closed only after its exact-candidate implementation meets
the stated visible acceptance, focused and broad gates, cross-platform/
security evidence where applicable, a closure record under
`plans/closure/knowledge-delta/NNN-status.md`, and synchronized registry
and roadmap updates. This roadmap remains proposed until decisions and
readiness gates are satisfied.
