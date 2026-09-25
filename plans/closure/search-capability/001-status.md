# Search Capability — Closure Status

Status: closed

Source implementation plans:

- `plans/archive/phase-1-provider-request-contract-and-brave-realization.md`
- `plans/archive/phase-2-extractive-evidence-and-fetch-control.md`
- `plans/archive/phase-3-firecrawl-developer-index.md`
- `plans/archive/phase-4-exa-semantic-search-provider.md`
- `plans/archive/phase-5-tavily-provider-and-closure.md`

Source subsystem roadmap:

- `plans/subsystems/search-capability-roadmap.md`

Repository baseline reviewed: `e645a3fe42090fb7b7e1ce8639681fe69878f57b` (`eggsearch` 0.3.7)

Implementation commits or pull requests:

- Historical phase landings through 0.3.7; history preserved in Git.

## 1. Executive finding

The five-milestone capability workstream is complete: query-constraint
fidelity, extractive evidence, and three new provider evidence classes landed
without changing the trust model or keyless baseline.

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| Provider request fidelity | `tests/provider_request_contract.rs` | pass | Historical phase-1 evidence |
| Excerpt/cache/focus controls | `tests/extract_fetch_contract.rs` | pass | Historical phase-2 evidence |
| Developer Index | provider adapter + regression tests | pass | Historical phase-3 evidence |
| Exa semantic provider | `tests/exa.rs`-lineage coverage | pass | Historical phase-4 evidence |
| Tavily provider + closure | `tests/provider_workstream_regression.rs` | pass | Historical phase-5 evidence |

## 3. Production implementation evidence

`EngineSearchRequest` carries constraints into provider calls; RRF +
reranking preserved; excerpts/timestamps/focus/cache controls additive and
backward-compatible; Brave/Exa/Tavily native-vs-local matrix documented.

## 4. Verification executed

Historical: routine gates on the 0.3.7 baseline; CodeGG downstream wrapper
review. No unresolved network-dependent gate remains; credentialed checks
stay provider-scoped skips.

## 5. Invariant review

Content-derived IDs, sanitization, bounded I/O, and capability-skip
explicitness hold per the archived phase records.

## 6. Failure and recovery review

Provider failures remain provider-local; no global failure on missing
credentials.

## 7. Migration and compatibility review

Additive fields only; CodeGG wrappers updated explicitly where needed.

## 8. Security review

Trust pipeline and bounded fetch unchanged.

## 9. Documentation and operations

Provider-setup native-vs-local matrix and tool-matrix updated in the
historical pass.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| low | Research Index deferred | No passage/citation-graph ops | New evidence-based plan if promoted |

## 11. Roadmap disposition

Milestones M001-M005 closed; no successor in this workstream.

## 12. Registry updates

Covered by the planning-convention migration; roadmap marked closed.
