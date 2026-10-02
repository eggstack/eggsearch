# Eggsearch Active Planning Registry

This file is the compact control surface for active interim planning.
Detailed requirements and completed history remain in source roadmaps,
implementation plans, `plans/closure/`, and Git history.

Canonical direction remains in:

- `plans/000-long-term-specification.md`
- `plans/001-terminology-and-domain-model.md`
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md`

Historical `phase-*.md` plans and pre-migration roadmaps are archived under
`plans/archive/` and MUST NOT be extended.

## Status vocabulary

- **proposed** — roadmap or plan exists but is not approved for execution.
- **ready** — dependencies and interfaces are satisfied; plan may be handed off.
- **active** — implementation or closure work is in progress.
- **blocked** — a named dependency or evidence requirement prevents progress.
- **closing** — implementation landed and closure evidence is being gathered.
- **closed** — closure record accepted.
- **conditionally closed** — substantial work landed, but a named correctness or operational evidence condition remains.
- **superseded** — replaced by another document.
- **archived** — no longer active and retained for traceability.

## Active subsystem roadmaps

| Subsystem | Status | Roadmap | Current milestone | Dependencies or blockers |
|---|---|---|---|---|
| Search capability and provider evidence | closed | `plans/subsystems/search-capability-roadmap.md` | M001-M005 closed | None. Pre-migration baseline `e645a3fe` (`eggsearch` 0.3.7). |
| CodeGG legacy search parity corrective | active (M004 blocked, M005 corrective ready) | `plans/subsystems/codegg-legacy-search-parity-corrective-addendum.md` | M001-M002 closed; M003 conditionally closed; M004 blocked (crate 0.4.0 + tag `v0.4.0` at `81a2e49` published, binary release failed at the pinned v3 attest step, run `37065679022`); M005 ready | M005 owns the 0.4.1 attest repair and the exact-candidate/provenance/install evidence that clears M003's publication condition. No code blocker remains. |
| Binary distribution, install/update, deployment | closed | `plans/subsystems/binary-distribution-deployment-roadmap.md` | M001-M006 closed | None. First binary release `v0.3.9` at `0cbbeee7`; qualify `34653366561`, release `34655458760`. |
| Maintenance, consolidation, CodeGG quality | closed | `plans/subsystems/maintenance-codegg-quality-roadmap.md` | M001-M005 closed | None. Baseline `4a713ff8`. |
| HTTP transport consolidation | closed | `plans/subsystems/transport-consolidation-roadmap.md` | M001-M003 closed | None. `eggfetch-core 0.2.0` at `bac6f49f`; qualify `35692096012`. |
| Performance optimization and footprint | closed | `plans/subsystems/performance-optimization-roadmap.md` | M001-M005 closed | None. Corrective candidate `0af540b8`; qualify `35542118569`. |
| Optional outbound routing (egress) | closed | `plans/subsystems/optional-outbound-routing-roadmap.md` | M001-M004 closed | None. Terminal baseline `6414a72`; hardened qualify `35810222447`. No egress-enabled binary published. Outcome B remains the runtime baseline. |
| MCP tool-surface consolidation | closed | `plans/subsystems/tool-surface-consolidation-roadmap.md` | M001-M007 closed | M001-M007 closed. Current control points `plans/closure/mcp-tool-surface-consolidation/001-status.md` through `007-status.md`. |
| Dependency evidence hardening | closed | `plans/subsystems/dependency-evidence-hardening-roadmap.md` | M010 corrective closed | M001-M010 closed. Current control point `plans/closure/dependency-evidence-hardening/010-status.md`; implementation `23676cc`, CI `36253474005`. |
| Repository security, supply-chain, and maintenance hardening | active | `plans/subsystems/repository-hardening-roadmap.md` | M008 corrective closed; M007 conditional; M002 conditional; M006 blocked | M008 closed at `44f38ab` (CI `37036453017`, egress `37036467646`, release qualify `37036489273`). M007/M001 still await scheduled security evidence; M002 awaits first tagged-release provenance; M006 remains externally blocked on chromiumoxide. |

## Dependency-ready implementation plans

| Subsystem | Milestone | Status | Implementation plan | Dependencies / handoff note |
|---|---|---|---|---|
| CodeGG legacy search parity corrective | M001 keyless/source-specific provider parity | closed | `plans/implementation/codegg-legacy-search-parity/001-keyless-source-provider-parity.md` | Closed at `e9103b4`; closure `plans/closure/codegg-legacy-search-parity/001-status.md`. Added Wikipedia, arXiv, PubMed, HN Algolia, and GitHub repository discovery plus the additive `structured_api` provider kind; inventory 37 -> 42. |
| CodeGG legacy search parity corrective | M002 credentialed provider parity | closed | `plans/implementation/codegg-legacy-search-parity/002-credentialed-provider-parity.md` | Closed at `5233315`; closure `plans/closure/codegg-legacy-search-parity/002-status.md`. Added opt-in `serpapi` and `kagi` (current v1, terms gate passed); inventory 42 -> 44. |
| CodeGG legacy search parity corrective | M003 CodeGG retirement handoff + qualification | conditionally closed | `plans/implementation/codegg-legacy-search-parity/003-codegg-retirement-handoff-and-qualification.md` | Conditionally closed at `8e5ec75`; closure `plans/closure/codegg-legacy-search-parity/003-status.md`. Publication condition still open after M004's attest stop; M005 delivers the immutable 0.4.1 release carrying the identical surface. |
| CodeGG legacy search parity corrective | M004 eggsearch 0.4.0 publication + downstream release handoff | blocked (corrective pass required) | `plans/implementation/codegg-legacy-search-parity/004-v0.4.0-release-publication-and-downstream-handoff.md` | Release-prep `81a2e49`, qualify run `37063695085` green, crate 0.4.0 published, tag `v0.4.0` at `81a2e49`; release run `37065679022` failed at the pinned v3 attest step. Closure `plans/closure/codegg-legacy-search-parity/004-status.md`. `v0.4.0` stays crate+tag only; corrective M005 registered. |
| CodeGG legacy search parity corrective | M005 eggsearch 0.4.1 corrective publication + downstream release handoff | ready | `plans/implementation/codegg-legacy-search-parity/005-v0.4.1-corrective-release.md` | Dependencies satisfied. Repair the attest step, rerun release qualification on the exact 0.4.1 SHA, publish crates.io -> exact tag -> seven-target immutable GitHub Release, capture attestation/install evidence, close M004/M003, and unblock CodeGG parity adoption at `>=0.4.1`. |
| Tool-surface consolidation | M001 contract and disclosure model | closed | `plans/implementation/mcp-tool-surface-consolidation/001-contract-and-disclosure-model.md` | Closed at `4b7e725`; closure `plans/closure/mcp-tool-surface-consolidation/001-status.md`. |
| Tool-surface consolidation | M002 agent-facing schema slimming | closed | `plans/implementation/mcp-tool-surface-consolidation/002-agent-facing-schema-slimming.md` | Closed at `b1e6ea7`; closure `plans/closure/mcp-tool-surface-consolidation/002-status.md`. |
| Tool-surface consolidation | M003 MCP 2026 protocol and error contract | closed | `plans/implementation/mcp-tool-surface-consolidation/003-mcp-2026-protocol-and-error-contract.md` | Closed at `c102ac6`; closure `plans/closure/mcp-tool-surface-consolidation/003-status.md`. |
| Tool-surface consolidation | M004 result projection and context budget | closed | `plans/implementation/mcp-tool-surface-consolidation/004-agent-result-projection-and-context-budget.md` | Closed at `a905712`; closure `plans/closure/mcp-tool-surface-consolidation/004-status.md`. |
| Tool-surface consolidation | M005 CodeGG progressive-disclosure integration | closed | `plans/implementation/mcp-tool-surface-consolidation/005-codegg-progressive-disclosure-integration.md` | Closed at `5a99bc2` (eggsearch-side contract); closure `plans/closure/mcp-tool-surface-consolidation/005-status.md`. |
| Tool-surface consolidation | M006 agentic evaluation | closed | `plans/implementation/mcp-tool-surface-consolidation/006-agentic-tool-surface-evaluation.md` | Closed with `plans/closure/mcp-tool-surface-consolidation/006-status.md`; candidate `fbf8457`. |
| Tool-surface consolidation | M007 maintenance decomposition and overlap ratchet | closed | `plans/implementation/mcp-tool-surface-consolidation/007-maintenance-decomposition-and-overlap-ratchet.md` | Closed with `plans/closure/mcp-tool-surface-consolidation/007-status.md`; candidate `85a44b7`. |
| Dependency evidence hardening | M001 typed evidence and applicability trust boundary | closed | `plans/implementation/dependency-evidence-hardening/001-typed-evidence-and-applicability-boundary.md` | Closed at `bce32e7`; closure `plans/closure/dependency-evidence-hardening/001-status.md`. |
| Dependency evidence hardening | M002 Cargo/Go/Python + dispatch | closed | `plans/implementation/dependency-evidence-hardening/002-core-ecosystem-and-dispatch-correctness.md` | Closed at `e1464f6`; closure `plans/closure/dependency-evidence-hardening/002-status.md`. |
| Dependency evidence hardening | M003 .NET/JVM structured correctness | closed | `plans/implementation/dependency-evidence-hardening/003-dotnet-jvm-structured-correctness.md` | Closed at `0d934f5`; closure `plans/closure/dependency-evidence-hardening/003-status.md`. |
| Dependency evidence hardening | M004 Ruby/Composer provenance | closed | `plans/implementation/dependency-evidence-hardening/004-ruby-composer-lock-provenance.md` | Closed at `fff47bf`; closure `plans/closure/dependency-evidence-hardening/004-status.md`. |
| Dependency evidence hardening | M005 JavaScript lockfiles | closed | `plans/implementation/dependency-evidence-hardening/005-javascript-lockfile-modernization.md` | Closed at `be389a0`; closure `plans/closure/dependency-evidence-hardening/005-status.md`. |
| Dependency evidence hardening | M006 Actions/OCI references | closed | `plans/implementation/dependency-evidence-hardening/006-actions-oci-reference-semantics.md` | Closed at `f0eb6a1`; closure `plans/closure/dependency-evidence-hardening/006-status.md`. |
| Dependency evidence hardening | M007 budgets/diagnostics/qualification | closed | `plans/implementation/dependency-evidence-hardening/007-budgets-diagnostics-and-adversarial-qualification.md` | Closed at `18b4c03`; closure `plans/closure/dependency-evidence-hardening/007-status.md`. |
| Dependency evidence hardening | M008 provenance/identity/parse-status corrective | closed | `plans/implementation/dependency-evidence-hardening/009-corrective-applicability-provenance-and-parse-status.md` | Implementation `3fd3076`; closure `plans/closure/dependency-evidence-hardening/008-status.md`. |
| Dependency evidence hardening | M009 final closure reconciliation | closed | `plans/implementation/dependency-evidence-hardening/010-final-closure-and-registry-reconciliation.md` | Closure `plans/closure/dependency-evidence-hardening/009-status.md`; exact candidate `12d8f9b`, CI `36217887427`. |
| Dependency evidence hardening | M010 explicit request ecosystem identity | closed | `plans/implementation/dependency-evidence-hardening/011-explicit-request-ecosystem-identity-consistency.md` | Implementation `23676cc`; closure `plans/closure/dependency-evidence-hardening/010-status.md`, CI `36253474005`. |
| Repository hardening | M001 dependency remediation + policy gate | conditionally closed via M007 | `plans/implementation/repository-hardening/001-dependency-security-and-policy-gate.md` | Historical closure `plans/closure/repository-hardening/001-status.md`; M007 corrects missing scheduled-workflow evidence without rewriting history. |
| Repository hardening | M002 CI/release supply-chain provenance | conditionally closed | `plans/implementation/repository-hardening/002-ci-release-supply-chain-provenance.md` | Closure `plans/closure/repository-hardening/002-status.md`; first tagged release must record generated/verified attestation and immutable behavior. |
| Repository hardening | M003 rmcp HTTP verification on eggfetch | closed | `plans/implementation/repository-hardening/003-rmcp-http-verification-on-eggfetch.md` | Closure `plans/closure/repository-hardening/003-status.md`; default graph reqwest-free. |
| Repository hardening | M004 process/unsafe boundary hardening | closed through M007 | `plans/implementation/repository-hardening/004-process-and-unsafe-boundary-hardening.md` | Historical closure `plans/closure/repository-hardening/004-status.md`; M007 extends the static invariant to all production Rust sources. |
| Repository hardening | M005 forge safety maintenance decomposition | closed | `plans/implementation/repository-hardening/005-forge-safety-maintenance-decomposition.md` | M004 hard dependency cleared; closure `plans/closure/repository-hardening/005-status.md`. |
| Repository hardening | M006 chromiumoxide zero-reqwest closure | blocked | `plans/implementation/repository-hardening/006-chromiumoxide-zero-reqwest-closure.md` | M003 is closed; remaining blocker is an upstream chromiumoxide release allowing launch-only build without reqwest. |
| Repository hardening | M007 corrective closure evidence + process ratchet | conditionally closed | `plans/implementation/repository-hardening/007-corrective-closure-evidence-and-process-ratchet.md` | No hard dependency. Conditionally closed with one operational condition: first successful dependency-security workflow run with event `schedule`. Closure `plans/closure/repository-hardening/007-status.md`. |
| Repository hardening | M008 Windows startup + cross-target release portability | closed | `plans/implementation/repository-hardening/008-windows-startup-cross-target-portability-corrective.md` | Closed at `44f38ab`; closure `plans/closure/repository-hardening/008-status.md`. CI `37036453017`, egress `37036467646`, release qualify `37036489273` all green on the exact candidate. |

Sequencing overview: `plans/implementation/mcp-tool-surface-consolidation/000-overview-and-sequencing.md`.
Handoff checklist: `plans/implementation/mcp-tool-surface-consolidation/008-implementation-handoff-checklist.md`.

Dependency evidence sequencing: `plans/implementation/dependency-evidence-hardening/000-overview-and-sequencing.md`.

Repository hardening sequencing: `plans/implementation/repository-hardening/000-overview-and-sequencing.md`.
Dependency evidence handoff checklist: `plans/implementation/dependency-evidence-hardening/008-implementation-handoff-checklist.md`.
Corrective closure sequence: `plans/implementation/dependency-evidence-hardening/009-corrective-applicability-provenance-and-parse-status.md` then `plans/implementation/dependency-evidence-hardening/010-final-closure-and-registry-reconciliation.md`.

## Current execution order and dependency gates

**Tool-surface gate:** M001-M007 closed. Do not run evaluation
against pre-consolidation bytes and present it as consolidation
evidence.

**Closed-workstream gate:** phases 1-28 (now `plans/archive/phase-*.md`) are
closed historical evidence. Their closure records live in `plans/closure/`
per subsystem. Do not reopen them for new scope; register new milestones
under the owning subsystem roadmap instead.

## Blocked work

CodeGG legacy search parity M003 remains conditionally closed at `8e5ec75`
with its publication condition still open. M004 executed the publication
sequence to its stop point: release-prep `81a2e49`, green local gates, green
`mode=qualify` run `37063695085`, published crates.io `0.4.0`, tag `v0.4.0` at
`81a2e49`. The tag-triggered release run `37065679022` went green across
preflight and all seven targets but failed deterministically in the release
assembler at the pinned `actions/attest` v3 step
(`predicate-type must be provided`); no `v0.4.0` GitHub Release exists, so no
binary, provenance, or install evidence was produced and no gate was weakened
to pretend otherwise. `v0.4.0` permanently remains crate+tag only. The
corrective release is registered as dependency-ready M005 in
`plans/implementation/codegg-legacy-search-parity/005-v0.4.1-corrective-release.md`,
which repairs the attest step and publishes immutable `v0.4.1` carrying the
identical parity surface. M004 is therefore `blocked` (corrective pass
required) rather than closed; M003's condition transfers to the 0.4.1
immutable release. All provider work is complete: `google_news` is the only
deliberate retirement. Successful M005 closure clears M003/M004 and authorizes
the downstream CodeGG harness to pin `>=0.4.1` and begin its registered
legacy-backend retirement sequence.

Tool-surface M001-M007 are closed; no tool-surface blocker remains.

Dependency evidence M001-M010 are closed; no dependency-evidence blocker remains.

Repository hardening M008 is closed at `44f38ab` (closure `plans/closure/repository-hardening/008-status.md`) with green ordinary Windows CI, seven-target egress qualification, and release-binaries qualification. M007 remains the corrective evidence control point for M001's missing scheduled-workflow evidence and M004's repository-wide process/unsafe ratchet. M001 is conditionally closed via M007 pending the first scheduled run. M002 remains conditionally closed on
first-release operational evidence. M003 and M005 remain closed. M004 is closed through M007. M006's M003
dependency is satisfied; it remains blocked only on an upstream chromiumoxide
release that makes HTTP discovery / reqwest optional without regressing
`Browser::launch`.

## Closure work and current control points

| Subsystem | Status | Controlling evidence |
|---|---|---|
| Search capability | closed | `plans/closure/search-capability/001-status.md`; archived phases 1-5 |
| Binary distribution and deployment | closed | `plans/closure/binary-distribution-deployment/001-status.md`; archived phases 6-10, 16 |
| Maintenance and CodeGG quality | closed | `plans/closure/maintenance-codegg-quality/001-status.md`; archived phases 11-15 |
| Transport consolidation | closed | `plans/closure/transport-consolidation/001-status.md`; archived phases 17-18, 24 |
| Performance optimization | closed | `plans/closure/performance-optimization/001-status.md`; archived phases 19-23 |
| Optional outbound routing | closed | `plans/closure/optional-outbound-routing/001-status.md`; archived phases 25-28 |
| Dependency evidence hardening | closed | `plans/closure/dependency-evidence-hardening/010-status.md`; M010 corrective evidence in `010-status.md`, prior terminal evidence in `009-status.md` |
| MCP tool-surface consolidation | closed | `plans/closure/mcp-tool-surface-consolidation/007-status.md`; M001-M006 evidence in `001-status.md` through `006-status.md` |
| Repository hardening | active; M008 closed, M007 conditional, M002 conditional, M006 blocked | M008 closed at `44f38ab` (`008-status.md`); M007 remains conditional evidence control point. Historical closure records `001-status.md` through `008-status.md`. |
| CodeGG legacy search parity | active; M001/M002 closed, M003 conditional, M004 blocked, M005 ready | `plans/closure/codegg-legacy-search-parity/004-status.md`; M005 plan `plans/implementation/codegg-legacy-search-parity/005-v0.4.1-corrective-release.md`; M004 plan `plans/implementation/codegg-legacy-search-parity/004-v0.4.0-release-publication-and-downstream-handoff.md`; prior control point `plans/closure/codegg-legacy-search-parity/003-status.md`; implementations `e9103b4` (M001), `5233315` (M002), `8e5ec75` (M003), `81a2e49` (M004 release-prep) |

## Closure rule

A milestone is `closed` only when its own acceptance criteria have been
exercised against the exact candidate and its closure record, roadmap status,
and registry entry are updated in the same closure commit. Corrective work is
a new plan referencing the original milestone and closure record, never a
silent amendment. Qualification is SHA-specific; re-qualify a different
eventual release candidate before publication.
