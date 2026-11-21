# M001 — Cutoff-Aware Knowledge Brief — Closure

Status: closed
Recommendation: closed; acceptance criteria pass on the exact candidate.
Implementation candidate: c0f71909cfe2e6016512423e55b9d00fdb7220c5
Baseline: 0f52c8fc812c99e8aee75f515f1b1d8dd32a9789
Plan: plans/archive/implementation/knowledge-delta/001-cutoff-aware-knowledge-brief.md
Roadmap: plans/subsystems/knowledge-delta-roadmap.md M001
Related: accepted ADR-0020.

## Candidate disposition

The pushed implementation candidate is c0f71909cfe2e6016512423e55b9d00fdb7220c5. This record and the synchronized roadmap, registry, and plan archive are closure-only changes after that candidate; no implementation source changed after qualification.

## Acceptance matrix

| Criterion | Evidence on the candidate | Result |
|---|---|---|
| Callable source-backed brief with cutoff provenance and bounded output | src/mcp/tools/knowledge_brief.rs, src/meta/knowledge_delta/execute.rs, tests/knowledge_delta_brief.rs | pass |
| ADR-0020 precision/basis comparison, no snippet-date inference, deterministic kdelta identity | tests/property_knowledge_delta.rs: comparison_fixtures_match_adr_0020, unknown_cutoff_or_basis_stays_indeterminate, coarser_precision_is_not_folded_to_midnight, conflicting_bases_do_not_pick_a_winner; tests/property_identity.rs | pass |
| MCP contract, SDK facade, Python/schema parity, presets, IDs, and budgets | MCP/tool-contract, SDK, documentation, static-guard, and Python qualification gates | pass |
| Unknown cutoff, total retrieval failure, and read-only live behavior are explicit | tests/knowledge_delta_brief.rs: same_day_and_unknown_model_make_no_temporal_claim, malformed_override_and_total_failure_are_explicit, live_call_does_not_open_the_index_and_disabled_retain_is_rejected | pass |
| Security review and docs are complete | independent review of the corrected candidate found no remaining findings; architecture and operator/client docs are updated | pass |

## Verification

| Command | Result |
|---|---|
| RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test knowledge_delta_brief --test knowledge_delta_discovery | pass |
| RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --lib ordinary_http_response_with_false_manual_interaction_flag_is_cacheable | pass |
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

## Documentation

Updated architecture/research.md, architecture/mcp.md, architecture/evidence-workflow.md, architecture/codegg-contract.md, architecture/fetch.md, architecture/testing.md, docs/tool-matrix.md, docs/config.md, docs/agent-workflows.md, docs/codegg-integration.md, docs/test-inventory.md, and the AGENTS.md tool-count index.

## Limitations

A published cutoff does not prove what a model knows. Publication date is not event date; unknown precision or basis remains indeterminate. No numeric recall threshold or live recall claim gates closure.
