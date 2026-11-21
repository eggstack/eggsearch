# M003 — Source-Driven Topic Discovery — Closure

Status: closed
Recommendation: closed; the omission fixture and required negative controls pass on the exact candidate.
Implementation candidate: c0f71909cfe2e6016512423e55b9d00fdb7220c5
Baseline: 0f52c8fc812c99e8aee75f515f1b1d8dd32a9789
Plan: plans/archive/implementation/knowledge-delta/003-source-driven-topic-discovery.md
Roadmap: plans/subsystems/knowledge-delta-roadmap.md M003
Dependencies: M001 and M002 closed on this candidate; accepted ADR-0020 and ADR-0021.

## Candidate disposition

The pushed implementation candidate is c0f71909cfe2e6016512423e55b9d00fdb7220c5. This closure record, archive, roadmap, and registry are documentation-only changes after the qualified implementation candidate.

## Fixture and acceptance matrix

The network-free omission fixture stores a bridge for topic widget-runtime and a source-backed neighbor for left-pad at https://github.com/left-pad/wheels. The topic omits left-pad. The direct M001 mock response has no deltas. With related_topics enabled, one local index step adds neighbor delta kdelta_2bbf0c0249cb4afe; that ID is absent from the direct result and present in the expanded result. The ID is derived from the canonical neighbor URL, publication basis, day precision, and 2024-07-02 date.

| Criterion | Evidence on the candidate | Result |
|---|---|---|
| Omission fixture recovers the named-missing indexed delta with provenance | tests/knowledge_delta_discovery.rs: omitted_name_is_found_only_through_a_source_backed_edge; direct IDs empty; expanded ID kdelta_2bbf0c0249cb4afe | pass |
| Expanded hit remains external-untrusted and carries edge/source span evidence | same test asserts expanded=true, external_untrusted, edge_source_ids, matching span source_id, and expansion=added | pass |
| Missing-edge and generic co-occurrence controls add no result | missing_edge_adds_no_expanded_delta; generic_text_and_missing_edge_add_no_expanded_delta | pass |
| Unchanged direct result is not counted as expansion; current cutoff is rechecked | unchanged_direct_topic_result_is_not_counted_as_expansion; indexed_deltas_are_rechecked_against_the_current_cutoff; expanded_index_hits_are_rechecked_against_the_current_cutoff | pass |
| Prefix collision is rejected and discovery stays a bounded local step | longer_identifier_prefix_does_not_alias_the_exact_topic; static guards and code paths use existing indexed edges only | pass |
| No new host, crawler, synonym list, embedding, or LLM path | network-free fixture, static guards, make check, and updated architecture/operator docs | pass |

## Verification

| Command | Result |
|---|---|
| RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test knowledge_delta_brief --test knowledge_delta_discovery | pass |
| RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test knowledge_delta_index | pass |
| RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test property_knowledge_delta --test property_identity | pass |
| RUSTUP_TOOLCHAIN=1.89.0 make check | pass |
| RUSTUP_TOOLCHAIN=1.89.0 make docs-check | pass |
| make packaging-check | pass |

## Hosted qualification on c0f71909cfe2e6016512423e55b9d00fdb7220c5

| Exact-SHA CI | [44745935736](https://github.com/eggstack/eggsearch/actions/runs/44745935736) | success |
| PR CI | [44745899450](https://github.com/eggstack/eggsearch/actions/runs/44745899450) | success |
| Exact-SHA Python qualification | [44745940949](https://github.com/eggstack/eggsearch/actions/runs/44745940949) | success |
| PR Python qualification | [44745899428](https://github.com/eggstack/eggsearch/actions/runs/44745899428), attempt 2 | success; canceled attempt 1 not counted |
| Eggpack exact-SHA qualification | [44745944881](https://github.com/eggstack/eggsearch/actions/runs/44745944881) | success |
| Release drift | [44745899462](https://github.com/eggstack/eggsearch/actions/runs/44745899462) | success |
| Egress qualification | [44745899759](https://github.com/eggstack/eggsearch/actions/runs/44745899759) | success |

## Documentation and limitations

Discovery and its fixture are documented in architecture/research.md, architecture/evidence-workflow.md, architecture/testing.md, docs/agent-workflows.md, docs/config.md, docs/test-inventory.md, and CodeGG guidance. Live precision/recall was not measured and is not a gate. An edge is co-occurrence/identity evidence from an indexed document, not a causal or compatibility assertion. No recursive traversal, new-host fetch, crawler, embedding, or in-process LLM is introduced.
