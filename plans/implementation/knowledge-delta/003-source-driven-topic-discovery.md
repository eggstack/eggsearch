# M003 — Source-Driven Topic Discovery and Delta Recall

Status: proposed
Date: 2026-11-19
Revised: 2026-11-20 — proposed contract. Not implementation authorization.
Subsystem: knowledge-delta
Milestone class: capability
Planning baseline: `0f52c8fc812c99e8aee75f515f1b1d8dd32a9789` on `feat/auto-20261117-044c4885`
Roadmap: `plans/subsystems/knowledge-delta-roadmap.md` M003

References: `plans/000-long-term-specification.md` §§3–4, 7–8;
`plans/001-terminology-and-domain-model.md` §§1–3;
`plans/003-planning-process.md`; ADR-0006, ADR-0016;
proposed ADR-0020 and ADR-0021.

## Objective and readiness

Find candidate developments **not named** in the caller's topic by
walking one source-backed step in the local evidence index. Do not
grow a crawler, a synonym list, or a model loop. Every expanded
delta must cite an indexed source. The brief stays small.

**Hard dependency:** M002 closed and its schema stable.
**Interface dependency:** the M001 request and result schema. This
milestone adds an optional field and does not change omitted-field
behavior. Proposed, not ready.

## Current state and testable gap

A topic such as "free-threaded Python" can miss a compatibility
change whose package name is absent from the query. M001 searches
the topic it was given. M002 stores selected evidence and looks it
up lexically. This milestone adds a bounded edge from an indexed
subject to another indexed subject. It does not fetch new hosts.

## Invariants and non-goals

- An edge requires a source ID and span in an already indexed
  document. Edges are deterministic. They are not model assertions.
- Expansion reads the index only. It does not follow links, refresh
  feeds, open a new host, or start a browser.
- Expansion is not verification and not proof the model lacks a
  fact. Preserve the ADR-0020 date rule, trust, conflicts, and IDs.
- Hard limits on subject fan-out, edges per subject, elapsed time,
  and output size. No embeddings and no local LLM.
- No synonym dictionary. Identity is package name, repository
  identifier, or version already present in the indexed document.
- A no-new-evidence answer stays possible and is labeled.

## Ordered work packages

1. **Typed edges.** Extend the accepted M002 schema with `subjects`
   and `subject_evidence_edges`. Extract package names, repository
   identifiers, and versions from structured metadata and titles
   already stored. Keep an ambiguous token as an unverified
   candidate, not as an edge. Hash the edge with the M001 identity
   functions. Do not build an open entity graph.
2. **Derivation.** Create edges only from documents M002 already
   accepted as public. Cap edges per subject. Drop generic tokens.
   Co-occurrence is not a compatibility or causal relation.
3. **Query.** When `related_topics` is true, run M001 direct
   retrieval first, then at most one local step to indexed
   neighbors, then FTS lookup of those neighbors' dated events.
   Do not fetch a URL just because an edge names it. Preserve
   direct results. Report when expansion added a delta.
4. **Compatibility.** `related_topics` defaults false. Omitted, the
   result matches M001 plus any M002 mode the caller already asked
   for. Report `direct_vs_expanded`, `edge_source_ids`, and
   index freshness. Do not change other tool schemas.
5. **Fixture gate.** Build a network-free omission fixture: the
   topic string does not contain the event name; the index contains
   a source-backed edge to that name; the expected `kdelta_` ID is
   known. Under the same output cap, the expanded result contains
   that delta and the direct M001 result does not. Every expanded
   delta has `edge_source_ids`. Negative controls — no edge,
   generic co-occurrence, and an unchanged topic — add no expanded
   delta. A missing source span, a new host, or a date-class
   regression fails the fixture. Live precision and recall may be
   reported. They do not gate closure. Do not call this lookup a
   cache.
6. **Docs.** Update the roadmap, `architecture/research.md`,
   `architecture/evidence-workflow.md`, `architecture/testing.md`,
   `docs/agent-workflows.md`, `docs/config.md`,
   `docs/test-inventory.md`, and CodeGG guidance if tool use
   changes.

## Failure, timeout, and recovery

A corrupt edge, an unresolvable ID, or a saturated budget degrades
to M002 FTS lookup and, if the caller allowed network, M001 live
retrieval, with an explicit warning. Cancellation stops the local
walk inside the original deadline. Do not grow the graph on retry.

## Verification (exact implementation candidate)

`tests/knowledge_delta_discovery.rs` holds the omission fixture and
the negative controls, plus property cases for token collisions,
edge amplification, and deterministic pruning. Do not put these
cases in `tests/research_workflow.rs`. Extend `tests/mcp_tools.rs`
and `tests/static_guards.rs`. Add the suite row to
`architecture/testing.md`. Focused:
`cargo test --locked --features mock --test knowledge_delta_discovery`;
`cargo test --locked --features mock --test mcp_tools`;
`cargo test --locked --features mock --test static_guards`.
Broad: `make check` and `make docs-check`. No live network in
the default gate.

## Acceptance, stop, and closure

The capability passes only when the omission fixture and the
negative controls meet the rule in work package 5. There is no
numeric threshold to choose after seeing a score.

**Stop:** M002 unclosed; a fetch of a new host; a synonym list; a
crawler, embedding, or in-process LLM; an edge without a source
span; or a regression of the ADR-0020 date rule.

Closure record `plans/closure/knowledge-delta/003-status.md` must
contain the candidate SHA, the fixture definition, the direct
versus expanded IDs, negative-control results, any ungated live
measurements labeled as ungated, bounds, security evidence, docs
and client gates, residual risk, and the recommendation. Update
the registry and roadmap in the same change. This plan is not
completion.
