# M003 — Source-Driven Topic Discovery and Delta Recall

Status: proposed
Draft: yes — blocked on M002 closure
Date: 2026-11-19
Subsystem: knowledge-delta
Milestone class: capability
Planning baseline: `0f52c8fc812c99e8aee75f515f1b1d8dd32a9789` on `feat/auto-20261117-044c4885`
Roadmap: `plans/subsystems/knowledge-delta-roadmap.md` M003

References: `plans/000-long-term-specification.md` §§3–4, 7–8;
`plans/001-terminology-and-domain-model.md` §§1–3;
`plans/003-planning-process.md`; ADR-0006, ADR-0016;
proposed ADR-0020 and ADR-0021.

## Objective and readiness

Improve recall of pertinent developments **not named** in the caller's
topic by using source-derived, bounded subject relationships from the
local evidence corpus. Do not grow a crawler or introduce a model loop.
Evidence must be attributable to fetched, indexed, authorized sources,
and the output must remain a small, verifiable brief.

**Hard dependency:** M002 closed and its persisted index/schema
contract stable. **Interface dependency:** M001 stable
`knowledge_brief` request/result schema. Proposed, not ready.

## Current state and testable gap

Without a topic history, a model asking about "free-threaded Python"
may omit the recent compatibility change of an unnamed package.
M001's topic-anchored queries cannot reliably discover all such
changes. M002 will persist selected evidence but initially use direct
FTS queries. This milestone adds a bounded bridge between topics,
named subjects and independently dated change records without claiming
semantic understanding.

## Invariants / non-goals

- Relationships require supporting source IDs/spans and are created
  deterministically, never from a generated assertion or instruction.
- Expansion only queries **already indexed** documents and explicit
  operator-approved feed/API subscriptions under M002 rules. No
  recursive HTML crawling, arbitrary link following, uncontrolled
  discovery of new hosts, or autonomous browser interaction.
- Relevance expansion is not source verification or proof a model
  lacks a fact. Preserve date precision, trust, conflicts, coverage,
  stable identities and source citations.
- Maintain explicit hard limits on subject fan-out, edge count,
  per-query expansion, index bytes, elapsed time and output tokens.
  No embedding index or local language model requirement.
- A no-new-evidence answer remains possible and honestly labeled.

## Ordered work packages

1. **Typed relation model.** Extend the index with source-backed
   `subjects` and `subject_evidence_edges`. Recognize stable
   package names, repository identifiers, versions, common synonyms
   and dated release/change markers from bounded structured metadata,
   titles/headings and source excerpts; retain ambiguous relations as
   unverified candidates. Persist provenance and a deterministic
   content hash, not an unconstrained inferred entity graph.
2. **Bounded graph derivation.** Construct relations only from
   already selected/fetched public documents; avoid arbitrary web
   exploration. Cap edges per subject/source, reject high-frequency
   generic tokens and low-specificity names, and prune by supported
   recency/quality. Never equate textual co-occurrence with a verified
   causal or compatibility relation.
3. **Query planning.** After M001's direct retrieval, expand locally
   at most one relation step to high-quality subject neighbors,
   retrieve candidate dated events from FTS5 with stable ranking,
   then optionally request a bounded *existing* approved source
   refresh. Preserve direct-query results and signal when expansion
   contributed useful evidence. Disallow a refresh from selecting a
   new domain based solely on untrusted page links.
4. **Controls and diagnostics.** Introduce opt-in `related_topics`
   or equivalent `knowledge_brief` mode with configurable max
   neighbors/events/time, and report `direct_vs_expanded`,
   `edge_source_ids`, candidate reason codes and index freshness.
   Defaults must be conservative and maintain M001 compatibility.
5. **Comparative evaluation.** Build at least 50 held-out dated
   fixtures with query omissions: model-facing topic lacks the
   package/event name but the source-backed local corpus contains
   it. Evaluate M001 direct retrieval, M002 lexical cache, and M003
   graph-assisted retrieval under equal output limits. Report
   precision@k, recall@k, evidence-backed novel hits, added noise,
   incorrect date classification, added cost and latency. Include
   adversarial low-quality co-occurrence and unchanged-topic
   negative controls.
6. **Docs and compatibility.** Update the knowledge-delta roadmap,
   `architecture/research.md`, `architecture/evidence-workflow.md`,
   `docs/agent-workflows.md`, `docs/config.md`,
   `docs/test-inventory.md`, and CodeGG consumer guidance if
   tool usage changes. Do not change other canonical tool schemas.

## Failure, timeout and recovery

Partial graph corruption, unresolvable references or saturated budgets
degrade to M002 lexical lookup and/or M001 bounded live retrieval with
explicit warnings. Event-source spans are resolved under the existing
trust and index policies. Expired or unavailable feed metadata does not
imply current state. Avoid repeated retries and silent graph growth;
cancellation interrupts traversal/refresh within the original deadline.

## Verification (exact implementation candidate)

Create a network-free `tests/knowledge_delta_discovery.rs` fixture
suite and add relevant property/fuzz cases for alias cycles, token
collisions, malformed relations, edge amplification, and deterministic
pruning. Extend `tests/research_evidence_corpus.rs`,
`tests/mcp_tools.rs`, `tests/evidence_contract.rs` and existing
static guards. Focused commands:
`cargo test --locked --features mock --test knowledge_delta_discovery`;
`cargo test --locked --features mock --test research_evidence_corpus`;
`cargo test --locked --features mock --test mcp_tools`;
`cargo test --locked --features mock --test static_guards`.
Broad: `make check`, `make docs-check` and index/memory resource
regression harness. No live network is required for the default gate.

## Acceptance, stop, and closure

The capability passes only when held-out omission cases show measurable
additional **source-backed** relevant deltas over the lexical/direct
baseline under matched budgets, without materially degrading
negative-control precision or violating hard bounds. Record actual
thresholds and results before release; no fabricated improvement claim.

**Stop:** M002 unclosed; implementation needs recursive crawling,
in-process LLM/embeddings, or unbounded source exploration; graph
edge provenance cannot be maintained; date/coverage semantics regress;
or resource constraints fail.

Closure record `plans/closure/knowledge-delta/003-status.md` must
contain candidate SHA, corpus definition, ablation metrics, explicit
negative-control findings, source/cutoff/date validity, bounds and
security evidence, docs/client gates, test outcomes, residual risk and
final recommendation. Update registry and roadmap atomically with
closure. No completion is implied by this draft.
