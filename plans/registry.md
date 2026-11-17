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
| Repository security, supply-chain, and maintenance hardening | active; M001, M002, M003, M004, M005, M007, M008, M009, M010, and M011 closed; M006 superseded by ADR-0014 | `plans/subsystems/repository-hardening-roadmap.md` | M011 closure: `plans/closure/repository-hardening/011-status.md`; chromiumoxide `reqwest` edge is accepted and guard-bounded. |
| Python PyPI distribution and prebuilt SBC wheels | active; M001–M003 closed; M004 blocked | `plans/subsystems/python-pypi-distribution-roadmap.md` | ARMv7 physical evidence is deferred; keep that wheel experimental. See `plans/closure/python-pypi-distribution/004-status.md`. |
| Retrieval quality evidence extension | closing (M007) | `plans/subsystems/retrieval-quality-evidence-extension-roadmap.md` | Local evidence is complete; hosted Linux, macOS, and Windows CI is required on the merged candidate. |
| Agentic retrieval integrity and context economy | active; M014 conditionally closed | `plans/subsystems/agentic-retrieval-integrity-roadmap.md` | M014's solve-count comparison remains unrun. M006–M012 are dependency-ready without plans; M008/M009 remain proposed. |
| PDF ingestion and extraction fidelity | closing | `plans/subsystems/pdf-ingestion-fidelity-roadmap.md` | M001/M002/M007 are implemented with closure records; M006 qualification remains open; M008 is active by user approval against the existing M001 implementation, with M001 exact-candidate status reconciliation still open; M003–M005 are ready. |
| Adaptive fetch target routing and artifacts | active (M001 active; M002 closed; M003 ready) | `plans/subsystems/adaptive-fetch-routing-roadmap.md` | M002 closed on `24f045729f8409dadc96b4c2455253d8a9c7b6ec`; see `plans/closure/adaptive-fetch-routing/002-status.md`. M001's implementation is committed on the same candidate but its closure record is outstanding. M003 is ready with implementation research recorded; ADR-0015 was accepted on 2026-11-17. Local PDF behavior remains owned by PDF M008. |
| Browser artifact availability and render bounds | closing (M001–M005 acceptance complete; PR #16 merged as `aea34ee`) | `plans/subsystems/browser-artifact-availability-roadmap.md` | M003 acceptance passed on exact SHA `fb99a43734bd4f2329bdfa2ba4315b403c2cd985`: local fixture, macOS arm64 installer/release smoke, live ten-tool MCP, hosted CI, egress, Eggpack drift/qualification, Windows wrappers, and Python full matrix (`43916322850`, attempt 4; all 34 jobs passed). All seven Eggpack consumer validations, `required_gate`, and aggregate passed; Eggpack target sizes and Python wheel sizes are in the closure record. PR-linked Python run `43897293446`, attempt 7, passed all 34 jobs and all displayed PR checks pass on the same SHA. PR #16 merged as `aea34eeb89749e3196712d9e9b9741f75c58e760` on 2026-11-16T21:05:25Z. See M003 closure and M004/M005 plans. |

## Active and dependency-ready implementation plans

| Subsystem | Milestone | Status | Plan | Gate |
|---|---|---|---|---|
| Repository hardening | M006 chromiumoxide dependency decision | superseded | `plans/archive/implementation/repository-hardening/006-chromiumoxide-zero-reqwest-closure.md` | Superseded by ADR-0014 §6. |
| Repository hardening | M007 corrective closure evidence | closed by M010 | `plans/archive/implementation/repository-hardening/007-corrective-closure-evidence-and-process-ratchet.md` | Required schedule evidence is recorded in `plans/closure/repository-hardening/010-status.md`. |
| Python PyPI distribution | M004 physical SBC benchmarking | blocked | `plans/implementation/python-pypi-distribution/004-post-publication-sbc-benchmarking.md` | ARMv7 32-bit physical testing is deferred; do not compile on SBCs. |
| Retrieval quality evidence extension | M007 expanded live and consumer acceptance | active | `plans/implementation/retrieval-quality-evidence-extension/001-expanded-live-and-consumer-acceptance.md` | No M006 amendment or production behavior change. |
| Retrieval quality evidence extension | M007 cross-domain, extraction, and resource qualification | closing | `plans/implementation/retrieval-quality-evidence-extension/002-cross-domain-extraction-and-resource-qualification.md` | Hosted Linux, macOS, and Windows CI remains. |
| Agentic retrieval integrity | M014 pass-cookie lifetime and egress binding | conditionally closed | `plans/implementation/agentic-retrieval-integrity/014-pass-cookie-lifetime-and-egress-binding.md` | The default solve-count comparison remains unrun; details in its closure record. |
| PDF ingestion | M001 PDF fetch contract and budgets | closing | `plans/implementation/pdf-ingestion-fidelity/001-pdf-fetch-contract-and-budgets.md` | Closure record is present. |
| PDF ingestion | M002 captured PDF corpus and fidelity lane | closing | `plans/implementation/pdf-ingestion-fidelity/002-captured-pdf-corpus-and-fidelity-lane.md` | Closure record is present. |
| PDF ingestion | M003 text-stream fidelity | ready | `plans/implementation/pdf-ingestion-fidelity/003-text-stream-fidelity.md` | Hard dependency: M002. |
| PDF ingestion | M004 structural reconstruction and quality | ready | `plans/implementation/pdf-ingestion-fidelity/004-structural-reconstruction-and-quality-signal.md` | Hard dependency: M003. |
| PDF ingestion | M005 chunking, focus, and payload economy | ready | `plans/implementation/pdf-ingestion-fidelity/005-pdf-chunking-focus-and-payload-economy.md` | Hard dependency: M002. |
| PDF ingestion | M006 artifact availability | closing | `plans/implementation/pdf-ingestion-fidelity/006-artifact-availability.md` | macOS arm64 size/native qualification, installer smoke, realistic-mix qualification, and wheel matrix remain. |
| PDF ingestion | M007 hostile-input and memory bounds | closing | `plans/implementation/pdf-ingestion-fidelity/007-hostile-input-and-memory-bounds.md` | Closure record is present. |
| PDF ingestion | M008 local workspace and batch ingestion | active | `plans/implementation/pdf-ingestion-fidelity/008-local-workspace-and-batch-ingestion.md` | Hard dependency M001 remains `closing`; work proceeds against its existing implementation by explicit user approval. Soft dependency M005 is not required. |
| Adaptive fetch routing | M001 target-aware fetch routing | active | `plans/implementation/adaptive-fetch-routing/001-target-aware-fetch-routing.md` | Committed on candidate `24f045729f8409dadc96b4c2455253d8a9c7b6ec`; soft integration dependency: PDF M008 for local-PDF end-to-end coverage. Its closure record remains outstanding. |
| Adaptive fetch routing | M002 batch per-target capability parity | closed | `plans/implementation/adaptive-fetch-routing/002-batch-per-target-capability-parity.md` | Closed on candidate `24f045729f8409dadc96b4c2455253d8a9c7b6ec`; `make check` passed with 85 suites and 6,347 tests, and `make docs-check` passed. See `plans/closure/adaptive-fetch-routing/002-status.md`. |
| Adaptive fetch routing | M003 opt-in webpage screenshot artifacts | ready | `plans/implementation/adaptive-fetch-routing/003-opt-in-webpage-screenshot-artifacts.md` | M002 is closed and ADR-0015 is accepted. Implementation research is recorded in the plan; serializer/transport and browser-fidelity tests remain implementation gates. |
| Browser artifact availability | M003 distribution and live qualification | closed | `plans/implementation/browser-artifact-availability/003-distribution-and-live-qualification.md` | All M003 criteria pass on `fb99a43734bd4f2329bdfa2ba4315b403c2cd985`: local realistic-mix (including request-13 cap/request-14 relaunch), macOS arm64 installer/release smoke, live ten-tool MCP, hosted CI/egress/drift, Eggpack qualification and Windows wrappers, and Python full matrix (`43916322850`, attempt 4; all 34 jobs passed). Eggpack target sizes and Python wheel sizes are recorded in the closure. PR-linked Python run `43897293446`, attempt 7, passed all 34 jobs, and all displayed PR checks pass. PR #16 merged as `aea34ee` on 2026-11-16T21:05:25Z; see M003 closure and M004/M005 plans.
| Browser artifact availability | M004 M003 qualification corrective pass | closed | `plans/implementation/browser-artifact-availability/004-m003-qualification-corrective-pass.md` | The Python schema and supported runner corrections pass on `fb99a43734bd4f2329bdfa2ba4315b403c2cd985`; the Windows installer mapping test normalizes checkout line endings. Hosted CI, egress, drift, Windows wrappers, Eggpack qualification, and the full 34-job Python matrix (`43916322850`, attempt 4) all pass. Exact-candidate evidence is in the M003 closure record. |
| Browser artifact availability | M005 session-cap initial render accounting | closed | `plans/implementation/browser-artifact-availability/005-session-cap-initial-render-accounting.md` | Local exact-candidate fixture on `fb99a43734bd4f2329bdfa2ba4315b403c2cd985` confirms cap teardown after render 13 and relaunch on render 14; hosted CI, egress, drift, Windows wrappers, Eggpack qualification, and the full 34-job Python matrix (`43916322850`, attempt 4) pass. Exact-candidate evidence is in the M003 closure record. |

## Closure rule

A milestone is `closed` only when its acceptance criteria pass on the exact
candidate and its closure record, roadmap status, and registry entry are updated
together. Corrective work gets a new plan; qualification is SHA-specific.
