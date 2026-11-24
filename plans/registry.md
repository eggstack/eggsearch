# Eggsearch Active Planning Registry

This is the control surface for active interim work. Detailed requirements and
completed evidence live in the linked roadmaps, implementation plans, closure
records, and archive.

Canonical direction:

- `plans/000-long-term-specification.md`
- `plans/001-terminology-and-domain-model.md`
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md`

Closed or superseded implementation plans are archived under
`plans/archive/implementation/`; historical phase plans remain under
`plans/archive/`. Do not extend archived plans.

## Status vocabulary

`proposed` · `ready` · `active` · `blocked` · `closing` · `closed` ·
`conditionally closed` · `superseded` · `archived`

## Active and proposed subsystem roadmaps

| Subsystem | Status | Roadmap | Current control point or blocker |
|---|---|---|---|
| Repository security, supply-chain, and maintenance hardening | active; M001, M002, M003, M004, M005, M007, M008, M009, M010, and M011 closed; M006 superseded by ADR-0014 | `plans/subsystems/repository-hardening-roadmap.md` | M007: `plans/archive/implementation/repository-hardening/007-corrective-closure-evidence-and-process-ratchet.md` (closed by M010); M011 closure: `plans/closure/repository-hardening/011-status.md`; chromiumoxide `reqwest` edge accepted and guard-bounded. |
| Python PyPI distribution and prebuilt SBC wheels | active; M001–M003, M005 closed; M004 blocked; M006 active (ADR-0024) | `plans/subsystems/python-pypi-distribution-roadmap.md` | M004 awaits physical ARMv7 evidence; M005 closure is recorded in `plans/closure/python-pypi-distribution/005-status.md`; M006: `plans/implementation/python-pypi-distribution/006-browser-enabled-wheels.md` (browser M006 dependency and hosted qualification remain). |
| Retrieval quality evidence extension | closing (M007) | `plans/subsystems/retrieval-quality-evidence-extension-roadmap.md` | Local evidence is complete; hosted Linux, macOS, and Windows CI is required on the merged candidate. |
| Agentic retrieval integrity and context economy | active; M001–M048 closed; M049 active | `plans/subsystems/agentic-retrieval-integrity-roadmap.md` | M049: `plans/implementation/agentic-retrieval-integrity/049-non-success-block-classification.md` (canonical gate outstanding); M047/M048 remain closed with their closure records. |
| PDF ingestion and extraction fidelity | closing | `plans/subsystems/pdf-ingestion-fidelity-roadmap.md` | M001/M002/M007 are implemented with closure records; M006 qualification remains open; M008 is active by user approval against the existing M001 implementation, with M001 exact-candidate status reconciliation still open; M003–M005 are ready. |
| Adaptive fetch target routing and artifacts | active (M001 active; M002 and M003 closed) | `plans/subsystems/adaptive-fetch-routing-roadmap.md` | M002 closed on `24f045729f8409dadc96b4c2455253d8a9c7b6ec`; see `plans/closure/adaptive-fetch-routing/002-status.md`. M001's implementation remains committed on that candidate but its closure record is outstanding. M003 closed on candidate `59ff64ad00940573ee520ecf19d7b5c0abcb1c87`; see `plans/closure/adaptive-fetch-routing/003-status.md`. Local PDF behavior remains owned by PDF M008. |
| Browser artifact availability and render bounds | active (M001–M005 acceptance complete; M006 active; PR #16 merged) | `plans/subsystems/browser-artifact-availability-roadmap.md` | M006: `plans/implementation/browser-artifact-availability/006-absent-browser-degradation-contract.md`; hosted no-Chrome qualification remains. |
| Knowledge-delta research | active; M001–M007 closed | `plans/subsystems/knowledge-delta-roadmap.md` | Closure evidence is linked from the roadmap. |

## Active and dependency-ready implementation plans

| Subsystem | Milestone | Status | Plan | Gate |
|---|---|---|---|---|
| Python PyPI distribution | M004 physical SBC benchmarking | blocked | `plans/implementation/python-pypi-distribution/004-post-publication-sbc-benchmarking.md` | ARMv7 32-bit physical testing is deferred; do not compile on SBCs. |
| Browser artifact availability | M006 absent-browser degradation contract | active | `plans/implementation/browser-artifact-availability/006-absent-browser-degradation-contract.md` | Hosted no-Chrome qualification and exact-candidate closure remain. |
| Python PyPI distribution | M006 browser-enabled wheels | active | `plans/implementation/python-pypi-distribution/006-browser-enabled-wheels.md` | Hard dependency: browser M006; hosted six-target wheel and sdist qualification remain. |
| Agentic retrieval integrity | M049 non-success block classification for Mojeek | active | `plans/implementation/agentic-retrieval-integrity/049-non-success-block-classification.md` | Work package 1 is implemented; the canonical gate remains outstanding. |
| Retrieval quality evidence extension | M007 expanded live and consumer acceptance | active | `plans/implementation/retrieval-quality-evidence-extension/001-expanded-live-and-consumer-acceptance.md` | No M006 amendment or production behavior change. |
| Retrieval quality evidence extension | M007 cross-domain, extraction, and resource qualification | closing | `plans/implementation/retrieval-quality-evidence-extension/002-cross-domain-extraction-and-resource-qualification.md` | Hosted Linux, macOS, and Windows CI remains. |
| PDF ingestion | M001 PDF fetch contract and budgets | closing | `plans/implementation/pdf-ingestion-fidelity/001-pdf-fetch-contract-and-budgets.md` | Closure record is present. |
| PDF ingestion | M002 captured PDF corpus and fidelity lane | closing | `plans/implementation/pdf-ingestion-fidelity/002-captured-pdf-corpus-and-fidelity-lane.md` | Closure record is present. |
| PDF ingestion | M003 text-stream fidelity | ready | `plans/implementation/pdf-ingestion-fidelity/003-text-stream-fidelity.md` | Hard dependency: M002. |
| PDF ingestion | M004 structural reconstruction and quality | ready | `plans/implementation/pdf-ingestion-fidelity/004-structural-reconstruction-and-quality-signal.md` | Hard dependency: M003. |
| PDF ingestion | M005 chunking, focus, and payload economy | ready | `plans/implementation/pdf-ingestion-fidelity/005-pdf-chunking-focus-and-payload-economy.md` | Hard dependency: M002. |
| PDF ingestion | M006 artifact availability | closing | `plans/implementation/pdf-ingestion-fidelity/006-artifact-availability.md` | macOS arm64 size/native qualification, installer smoke, realistic-mix qualification, and wheel matrix remain. |
| PDF ingestion | M007 hostile-input and memory bounds | closing | `plans/implementation/pdf-ingestion-fidelity/007-hostile-input-and-memory-bounds.md` | Closure record is present. |
| PDF ingestion | M008 local workspace and batch ingestion | active | `plans/implementation/pdf-ingestion-fidelity/008-local-workspace-and-batch-ingestion.md` | Hard dependency M001 remains `closing`; work proceeds against its existing implementation by explicit user approval. Soft dependency M005 is not required. |
| Adaptive fetch routing | M001 target-aware fetch routing | active | `plans/implementation/adaptive-fetch-routing/001-target-aware-fetch-routing.md` | Committed on candidate `24f045729f8409dadc96b4c2455253d8a9c7b6ec`; soft integration dependency: PDF M008 for local-PDF end-to-end coverage. Its closure record remains outstanding. |

## Closure rule

A milestone is `closed` only when its acceptance criteria pass on the exact
candidate and its closure record, roadmap status, and registry entry are updated
together. Corrective work gets a new plan; qualification is SHA-specific.
