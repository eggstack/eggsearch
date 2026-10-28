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
| CodeGG legacy search parity corrective | closed | `plans/subsystems/codegg-legacy-search-parity-corrective-addendum.md` | Historical M001-M005 closed. Immutable `v0.4.1` published (fix-prep `32769d6`, qualify `37070932877`, release `37072825644` incl. attest, 16 assets, provenance/install evidence); `v0.4.0` remains crate+tag only. Closure `plans/closure/codegg-legacy-search-parity/005-status.md`. | Original provider-parity adoption is complete. The subsequently discovered Mode-Off local repository dispatch defect is separately owned by `plans/subsystems/mode-off-repository-locality-corrective-roadmap.md`; fixed eggsearch `v0.4.3` is now published, while downstream C002 paired requalification remains required before legacy-backend deletion. Historical first-release provenance evidence remains valid. |
| Mode-Off local repository dispatch corrective | closed (M001-M003 closed) | `plans/subsystems/mode-off-repository-locality-corrective-roadmap.md` | M001 closed at `cc0c9c7`; M003 hosted run `38476463235` qualified candidate `5e7b853`; M002 published crate `0.4.3` and tag `v0.4.3` at that SHA, with producer run `38480449328`, provenance run `38483437078`, and release-backed zero-call evidence | M001, M002, and M003 closure records: `plans/closure/mode-off-repository-locality-corrective/001-status.md` through `003-status.md`. The eggsearch handoff is documented. CodeGG C002 and legacy backend retirement remain consumer-owned. |
| Binary distribution, install/update, deployment | closed | `plans/subsystems/binary-distribution-deployment-roadmap.md` | M001-M006 closed | None. First binary release `v0.3.9` at `0cbbeee7`; qualify `34653366561`, release `34655458760`. |
| Eggpack release adoption | closed | `plans/subsystems/eggpack-release-adoption-roadmap.md` | M001 closed | Closure `plans/closure/eggpack-release-adoption/001-status.md`. Paired with `eggstack/eggpack@325d44e` Ecosystem M003b (`plans/closure/ecosystem-adoption/003b-status.md`). Implementation `eabbf80`; hosted run `37531104901` staged `v0.4.2` as draft `405157598` with the exact 19-asset inventory, all seven targets built/qualified/consumer-validated, glibc 2.17 re-proven on all three GNU targets, ARMv7 required runtime validator passed, Windows ARM64 required and green, no OIDC in generated jobs, legacy writer deleted. Provenance run `37561960304` then attested the staged bytes and the draft was published 2026-10-07, discharging the named condition. Open Medium: release build is not byte-reproducible across attempts |
| Maintenance, consolidation, CodeGG quality | closed | `plans/subsystems/maintenance-codegg-quality-roadmap.md` | M001-M005 closed | None. Baseline `4a713ff8`. |
| HTTP transport consolidation | closed | `plans/subsystems/transport-consolidation-roadmap.md` | M001-M003 closed | None. `eggfetch-core 0.2.0` at `bac6f49f`; qualify `35692096012`. |
| Performance optimization and footprint | closed | `plans/subsystems/performance-optimization-roadmap.md` | M001-M005 closed | None. Corrective candidate `0af540b8`; qualify `35542118569`. |
| Optional outbound routing (egress) | closed | `plans/subsystems/optional-outbound-routing-roadmap.md` | M001-M004 closed | None. Terminal baseline `6414a72`; hardened qualify `35810222447`. No egress-enabled binary published. Outcome B remains the runtime baseline. |
| MCP tool-surface consolidation | closed | `plans/subsystems/tool-surface-consolidation-roadmap.md` | M001-M007 closed | M001-M007 closed. Current control points `plans/closure/mcp-tool-surface-consolidation/001-status.md` through `007-status.md`. |
| Dependency evidence hardening | closed | `plans/subsystems/dependency-evidence-hardening-roadmap.md` | M010 corrective closed | M001-M010 closed. Current control point `plans/closure/dependency-evidence-hardening/010-status.md`; implementation `23676cc`, CI `36253474005`. |
| Repository security, supply-chain, and maintenance hardening | active | `plans/subsystems/repository-hardening-roadmap.md` | M001/M002/M007/M008/M009/M010 closed; M006 blocked | Scheduled dependency-security run `38290328560` succeeded; M010 records M001/M007 closure. M002 closed by M009; M006 remains externally blocked on chromiumoxide. |
| Python native library for agents | closed (M001-M008 closed; unpublished by design) | `plans/subsystems/python-native-library-roadmap.md` | M008 closed at `067468a`, qualifying run `38641009246` (27/27 incl. audit) | ADR-0005 accepted. Runs `38522548258`, `38608502185`/`38611932990`/`38614592873`, `38637266178`, and `38638066765` are historical/deficiency/supporting evidence only. Publication separately authorized. |
| Python PyPI distribution and prebuilt SBC wheels | active (M001/M002/M003 active; M004 blocked) | `plans/subsystems/python-pypi-distribution-roadmap.md` | M001 Python 3.15 qualification and M002 hosted ARMv7 wheel are implemented pending hosted exact-SHA runs; M003 gated release workflow is implemented; M004 benchmark harness is prepared | Historical Python M001-M008 stay closed. Maintainer must configure the PyPI Trusted Publisher and GitHub `pypi` approval environment before M003 publication. Do not compile on SBCs; physical SBC benchmarking starts after publication. |

## Dependency-ready implementation plans

| Subsystem | Milestone | Status | Implementation plan | Dependencies / handoff note |
|---|---|---|---|---|
| Python PyPI distribution | M001 Python 3.15 / PyO3 ABI3 requalification | **active** | `plans/implementation/python-pypi-distribution/001-python315-toolchain-and-abi3.md` | PyO3/async and Maturin upgraded; 3.11–3.15 × five established platforms is in CI; exact-SHA hosted qualification remains. No publication. |
| Python PyPI distribution | M002 hosted ARMv7 and Linux wheel expansion | **active** | `plans/implementation/python-pypi-distribution/002-hosted-armv7-and-linux-wheel-expansion.md` | Pinned manylinux cross-build and manylinux 2.31 QEMU install jobs added. Artifact tag/ELF/runtime qualification remains; optional musllinux wheels are deferred and unclaimed. **No SBC compilation**. |
| Python PyPI distribution | M003 controlled first PyPI release | **active** (publish gated) | `plans/implementation/python-pypi-distribution/003-first-pypi-release-and-provenance.md` | Exact-run artifact verification and reviewer-gated OIDC publication workflow added. Requires green M001/M002, PyPI Trusted Publisher setup, and GitHub environment approval. |
| Python PyPI distribution | M004 physical SBC benchmarking after publication | **blocked** | `plans/implementation/python-pypi-distribution/004-post-publication-sbc-benchmarking.md` | Operational blocker: M003 public PyPI release. No pre-publication SBC build/benchmark. |
| Eggpack release adoption | M001 seven-target Eggpack producer cutover | closed | `plans/implementation/eggpack-release-adoption/001-seven-target-eggpack-producer-cutover.md` | Paired producer plan `eggstack/eggpack: plans/implementation/ecosystem-adoption/003b-eggsearch-seven-target-eggpack-producer-cutover.md` (`325d44e`). Implementation `eabbf80`; run `37531104901`; draft `405157598`; provenance `37561960304`; published 2026-10-07. Eggpack production code was not edited |
| CodeGG legacy search parity corrective | M001 keyless/source-specific provider parity | closed | `plans/implementation/codegg-legacy-search-parity/001-keyless-source-provider-parity.md` | Closed at `e9103b4`; closure `plans/closure/codegg-legacy-search-parity/001-status.md`. Added Wikipedia, arXiv, PubMed, HN Algolia, and GitHub repository discovery plus the additive `structured_api` provider kind; inventory 37 -> 42. |
| CodeGG legacy search parity corrective | M002 credentialed provider parity | closed | `plans/implementation/codegg-legacy-search-parity/002-credentialed-provider-parity.md` | Closed at `5233315`; closure `plans/closure/codegg-legacy-search-parity/002-status.md`. Added opt-in `serpapi` and `kagi` (current v1, terms gate passed); inventory 42 -> 44. |
| CodeGG legacy search parity corrective | M003 CodeGG retirement handoff + qualification | closed | `plans/implementation/codegg-legacy-search-parity/003-codegg-retirement-handoff-and-qualification.md` | Closed at `8e5ec75` plus M005 reconciliation: sole publication condition cleared by immutable 0.4.1 release. Historical closure `plans/closure/codegg-legacy-search-parity/003-status.md`; reconciliation in `plans/closure/codegg-legacy-search-parity/005-status.md`. |
| CodeGG legacy search parity corrective | M004 eggsearch 0.4.0 publication + downstream release handoff | closed | `plans/implementation/codegg-legacy-search-parity/004-v0.4.0-release-publication-and-downstream-handoff.md` | Publication boundary satisfied by immutable 0.4.1 carrying the identical surface (`v0.4.0` stays crate+tag only). Closure `plans/closure/codegg-legacy-search-parity/004-status.md`; reconciliation in `plans/closure/codegg-legacy-search-parity/005-status.md`. |
| CodeGG legacy search parity corrective | M005 eggsearch 0.4.1 corrective publication + downstream release handoff | closed | `plans/implementation/codegg-legacy-search-parity/005-v0.4.1-corrective-release.md` | Fix-prep `32769d6`, qualify run `37070932877` green, crates.io 0.4.1, tag `v0.4.1` at `32769d6`, release run `37072825644` green incl. attest + verification, immutable 16-asset release, provenance/install evidence, external smoke green. Closure `plans/closure/codegg-legacy-search-parity/005-status.md`. |
| Mode-Off local repository dispatch corrective | M001 explicit remote-denial seam + zero-call regression | closed | `plans/implementation/mode-off-repository-locality-corrective/001-explicit-zero-remote-dispatch-boundary.md` | Implementation `cc0c9c7b27e0706a3bab5f5d7cb137e64e9eaa3d`; closure `plans/closure/mode-off-repository-locality-corrective/001-status.md`; `make check` and `make docs-check` passed. |
| Mode-Off local repository dispatch corrective | M002 corrected immutable patch publication and CodeGG handoff | closed | `plans/implementation/mode-off-repository-locality-corrective/002-immutable-corrected-release-and-handoff.md` | Crate `0.4.3`, tag `v0.4.3`, producer run `38480449328`, provenance run `38483437078`, and closure `plans/closure/mode-off-repository-locality-corrective/002-status.md`; CodeGG C002 remains consumer-owned. |
| Mode-Off local repository dispatch corrective | M003 non-publishing exact-SHA Eggpack qualifier | closed | `plans/implementation/mode-off-repository-locality-corrective/003-prepublication-exact-sha-qualifier.md` | Closed after run `38476463235` passed all seven target builds, qualifications, validators, required gate, and aggregate for exact candidate `5e7b853e7eebf29c58f84959805c8774279d7ea4`; closure `plans/closure/mode-off-repository-locality-corrective/003-status.md`. |
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
| Repository hardening | M001 dependency remediation + policy gate | closed via M007/M010 | `plans/implementation/repository-hardening/001-dependency-security-and-policy-gate.md` | Closure `plans/closure/repository-hardening/001-status.md`; M010 records first successful schedule run `38290328560`. |
| Repository hardening | M002 CI/release supply-chain provenance | closed by M009 | `plans/implementation/repository-hardening/002-ci-release-supply-chain-provenance.md` | Original closure `plans/closure/repository-hardening/002-status.md`; reconciliation `plans/closure/repository-hardening/009-status.md`; v0.4.1 first-tagged-release attestation and immutability evidence in `plans/closure/codegg-legacy-search-parity/005-status.md` §2. |
| Repository hardening | M003 rmcp HTTP verification on eggfetch | closed | `plans/implementation/repository-hardening/003-rmcp-http-verification-on-eggfetch.md` | Closure `plans/closure/repository-hardening/003-status.md`; default graph reqwest-free. |
| Repository hardening | M004 process/unsafe boundary hardening | closed through M007 | `plans/implementation/repository-hardening/004-process-and-unsafe-boundary-hardening.md` | Historical closure `plans/closure/repository-hardening/004-status.md`; M007 extends the static invariant to all production Rust sources. |
| Repository hardening | M005 forge safety maintenance decomposition | closed | `plans/implementation/repository-hardening/005-forge-safety-maintenance-decomposition.md` | M004 hard dependency cleared; closure `plans/closure/repository-hardening/005-status.md`. |
| Repository hardening | M006 chromiumoxide zero-reqwest closure | blocked | `plans/implementation/repository-hardening/006-chromiumoxide-zero-reqwest-closure.md` | M003 is closed; remaining blocker is an upstream chromiumoxide release allowing launch-only build without reqwest. |
| Repository hardening | M007 corrective closure evidence + process ratchet | closed by M010 | `plans/implementation/repository-hardening/007-corrective-closure-evidence-and-process-ratchet.md` | Closure `plans/closure/repository-hardening/007-status.md`; first successful schedule run `38290328560` reconciled in `plans/closure/repository-hardening/010-status.md`. |
| Repository hardening | M008 Windows startup + cross-target release portability | closed | `plans/implementation/repository-hardening/008-windows-startup-cross-target-portability-corrective.md` | Closed at `44f38ab`; closure `plans/closure/repository-hardening/008-status.md`. CI `37036453017`, egress `37036467646`, release qualify `37036489273` all green on the exact candidate. |
| Repository hardening | M009 first tagged-release provenance reconciliation | closed | `plans/closure/repository-hardening/009-status.md` | Implementation `cf1e26a`; M002 closed after auditing v0.4.1 release run `37072825644` and its attestation/immutability evidence. |
| Repository hardening | M010 scheduled dependency-security evidence reconciliation | closed | `plans/closure/repository-hardening/010-status.md` | Closed at `31175e2`; first schedule-event run `38290328560` completed successfully on `90e996ef6c83c969e30c30a36bc9d968a85dc00f`. |
| Python native library | M001 shared Rust SDK facade/parity | **closed** | `plans/implementation/python-native-library/001-shared-sdk-facade-and-parity.md` | Candidate `5f686d02e15daa2c5db7ad6d889c06eaff3ceeee`; closure `plans/closure/python-native-library/001-status.md`. |
| Python native library | M002 PyO3 asyncio binding/lifecycle | **closed** | `plans/implementation/python-native-library/002-pyo3-async-execution-and-lifecycle.md` | Candidate `2392e72`; closure `plans/closure/python-native-library/002-status.md`; bounded conversion, cancellation/close, and ten-operation consumer smoke. |
| Python native library | M003 Python agent API, types, parity | **closed** | `plans/implementation/python-native-library/003-agent-api-types-and-compatibility.md` | Candidate `ca4d0d6`; closure `plans/closure/python-native-library/003-status.md`; typed ten-method API, schema coverage, Mode-Off agent flow. |
| Python native library | M004 wheels, sdist and release qualification | **closed** | `plans/implementation/python-native-library/004-wheel-sdist-and-release-qualification.md` | Candidate `faa1da2`; closure `plans/closure/python-native-library/004-status.md`; 5/5 builds, 20/20 smokes, offline sdist rebuild, digests; no publication. |
| Python native library | M005 in-flight cancellation/config corrective | **closed** | `plans/implementation/python-native-library/005-inflight-cancellation-and-default-config-corrective.md` | Candidate `baf00cf`; closure `plans/closure/python-native-library/005-status.md`; confirmed-in-flight cancellation/close with permit recovery plus opt-in default-config path. |
| Python native library | M006 M004 exact-SHA closure and registry reconciliation | **closed** | `plans/implementation/python-native-library/006-m004-exact-sha-distribution-closure.md` | Candidate `faa1da2`; closure `plans/closure/python-native-library/006-status.md`; qualifying run `38617598636`. |
| Python native library | M007 Windows safe-read corrective | **closed** | `plans/implementation/python-native-library/007-windows-safe-read-corrective.md` | Candidate `faa1da2`; closure `plans/closure/python-native-library/007-status.md`; non-unix root check repaired, proven by 4/4 Windows smokes. |
| Python native library | M008 advisory remediation and symlink-containment re-qualification | **closed** | `plans/implementation/python-native-library/008-rustls-and-symlink-containment-requalification.md` | Candidate `067468a`; closure `plans/closure/python-native-library/008-status.md`; rustls 0.23.46 in both lockfiles, walk-time link rejection with fail-closed unix pin, 27/27 re-qualification with green audit. |

Sequencing overview: `plans/implementation/mcp-tool-surface-consolidation/000-overview-and-sequencing.md`.
Handoff checklist: `plans/implementation/mcp-tool-surface-consolidation/008-implementation-handoff-checklist.md`.

Dependency evidence sequencing: `plans/implementation/dependency-evidence-hardening/000-overview-and-sequencing.md`.

Repository hardening sequencing: `plans/implementation/repository-hardening/000-overview-and-sequencing.md`.
Dependency evidence handoff checklist: `plans/implementation/dependency-evidence-hardening/008-implementation-handoff-checklist.md`.
Corrective closure sequence: `plans/implementation/dependency-evidence-hardening/009-corrective-applicability-provenance-and-parse-status.md` then `plans/implementation/dependency-evidence-hardening/010-final-closure-and-registry-reconciliation.md`.

## Current execution order and dependency gates

**Python native library gate:** ADR-0005 authorizes a narrow direct Rust SDK facade and Python PyO3 native package without broad Rust module stability. M001-M008 are closed at `067468a` with fully-green qualifying run `38641009246` (5/5 builds, 20/20 smokes, offline sdist rebuild, green audit, digests in `008-status.md`). Earlier runs are historical/deficiency/supporting evidence only. Initial Python qualification was nonpublishing; first PyPI publication is now separately planned under `python-pypi-distribution` M003 with explicit human release approval. Eggpack binary assets remain unchanged.

**Tool-surface gate:** M001-M007 closed. Do not run evaluation
against pre-consolidation bytes and present it as consolidation
evidence.

**Closed-workstream gate:** phases 1-28 (now `plans/archive/phase-*.md`) are
closed historical evidence. Their closure records live in `plans/closure/`
per subsystem. Do not reopen them for new scope; register new milestones
under the owning subsystem roadmap instead.

**Mode-Off locality correctness gate (new):** Upstream M001 is **closed** at
`cc0c9c7` with explicit no-remote-dispatch authority and counted zero provider
and package-resolver calls while retaining private local search. M003 is
**closed** after hosted run `38476463235` passed all seven target builds,
qualifications, consumer validators, required gate, and aggregate for exact
candidate `5e7b853e7eebf29c58f84959805c8774279d7ea4`. M002 is **closed** with
immutable crate `0.4.3`, tag `v0.4.3`, public release, provenance, and
release-backed zero-call evidence. The qualifier is non-publishing and derived
from the generated Eggpack producer. CodeGG paired requalification remains a
consumer-owned gate before legacy backend retirement; do not retry `v0.4.1`
against live remote providers or equate a CodeGG consumer check with eggsearch
release qualification. Roadmap:
`plans/subsystems/mode-off-repository-locality-corrective-roadmap.md`.

**Eggpack release-adoption gate:** `plans/implementation/eggpack-release-adoption/001-seven-target-eggpack-producer-cutover.md` is **closed** at `plans/closure/eggpack-release-adoption/001-status.md`, paired with Eggpack Ecosystem M003b (`eggstack/eggpack@325d44e`, closure `plans/closure/ecosystem-adoption/003b-status.md`). The generated producer path was qualified live before the old release writer was retired, in hosted run `37531104901`, which staged `v0.4.2` as draft `405157598` with the exact 19-asset inventory. Provenance run `37561960304` attested those staged bytes while the release was still a draft, and the release was then published on 2026-10-07. The historical Binary Distribution roadmap stays closed.

That publication step is what surfaced a Medium defect in the provenance workflow itself, recorded at closure §13: the job requested `contents: read`, and GitHub 404s a draft lookup for a token without push access, so the guard could never have passed in any release state. It had never been dispatched — zero prior runs. Fixed in `a033938` and re-run green. The same commit replaced a `check-contract.sh` guard that asserted the wrong property (forbidding a permission *string* rather than asserting *behaviour*); the check would have blocked its own fix.

## Blocked work

CodeGG legacy search parity is closed. M005 delivered the immutable `v0.4.1`
release carrying the frozen parity surface: fix-prep `32769d6`, green local
gates, green `mode=qualify` run `37070932877`, published crates.io `0.4.1`,
tag `v0.4.1` at `32769d6`, tag-triggered release run `37072825644` green
across preflight, all seven targets, and the assembler including the repaired
attest step and in-workflow verification, 16-asset release published with
SLSA provenance verified to repository/workflow/commit, and external
installer/update/MCP smoke green (closure
`plans/closure/codegg-legacy-search-parity/005-status.md`). `v0.4.0`
permanently remains crate+tag only. All provider work is complete:
`google_news` is the only deliberate retirement. The downstream CodeGG
harness may pin `>=0.4.1` for the historical parity contract, but
**legacy-backend deletion must not proceed** under the subsequently discovered
Mode-Off local repo remote-dispatch defect; the new Mode-Off corrective M001
and immutable release M002 plus CodeGG C002 requalification are required.

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
| Eggpack release adoption | closed; M001 closed | Closure `plans/closure/eggpack-release-adoption/001-status.md`; paired producer control point `eggstack/eggpack` Ecosystem M003b (`plans/closure/ecosystem-adoption/003b-status.md`); `v0.4.2` staged in run `37531104901`, attested in run `37561960304`, published 2026-10-07 |
| Maintenance and CodeGG quality | closed | `plans/closure/maintenance-codegg-quality/001-status.md`; archived phases 11-15 |
| Transport consolidation | closed | `plans/closure/transport-consolidation/001-status.md`; archived phases 17-18, 24 |
| Performance optimization | closed | `plans/closure/performance-optimization/001-status.md`; archived phases 19-23 |
| Optional outbound routing | closed | `plans/closure/optional-outbound-routing/001-status.md`; archived phases 25-28 |
| Dependency evidence hardening | closed | `plans/closure/dependency-evidence-hardening/010-status.md`; M010 corrective evidence in `010-status.md`, prior terminal evidence in `009-status.md` |
| MCP tool-surface consolidation | closed | `plans/closure/mcp-tool-surface-consolidation/007-status.md`; M001-M006 evidence in `001-status.md` through `006-status.md` |
| Repository hardening | active; M008 closed, M007 conditional, M002 conditional, M006 blocked | M008 closed at `44f38ab` (`008-status.md`); M007 remains conditional evidence control point. Historical closure records `001-status.md` through `008-status.md`. |
| CodeGG legacy search parity | closed; M001 through M005 closed | `plans/closure/codegg-legacy-search-parity/005-status.md`; prior control points `plans/closure/codegg-legacy-search-parity/004-status.md` and `003-status.md`; implementations `e9103b4` (M001), `5233315` (M002), `8e5ec75` (M003), `81a2e49` (M004 release-prep), `32769d6` (M005 fix-prep); historical provider-parity closure retained, new Mode-Off locality issue separately registered |
| Mode-Off local repository dispatch corrective | closed; M001-M003 closed | M001 closure `001-status.md`; M003 hosted qualification `38476463235`; M002 immutable crate/tag/release `v0.4.3`, producer run `38480449328`, provenance run `38483437078`, and closure `002-status.md`. CodeGG paired requalification remains consumer-owned. |
| Python native library | closed; M001-M008 closed | `plans/subsystems/python-native-library-roadmap.md`; closure records `001-status.md`–`008-status.md`; M008 closed at `067468a`, run `38641009246`; unpublished by design |
| Python PyPI distribution | active; M001/M002/M003 active; M004 blocked | `plans/subsystems/python-pypi-distribution-roadmap.md`; M001/M002/M003 implementations are prepared pending exact-SHA hosted qualification and PyPI setup; M004 benchmark harness is prepared, hardware evidence follows publication |

## Closure rule

A milestone is `closed` only when its own acceptance criteria have been
exercised against the exact candidate and its closure record, roadmap status,
and registry entry are updated in the same closure commit. Corrective work is
a new plan referencing the original milestone and closure record, never a
silent amendment. Qualification is SHA-specific; re-qualify a different
eventual release candidate before publication.
