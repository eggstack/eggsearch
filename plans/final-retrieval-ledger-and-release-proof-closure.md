# Final Retrieval-Ledger and Release-Proof Closure

**Status:** implementation handoff

**Baseline reviewed:** `e05bf66b11a0a2a4967b3e597e1a85516f5360e4`

**Purpose:** close the remaining evidence-semantics, retrieval-accounting, conflict-provenance, native-adapter verification, and release-proof gaps after the final residual correctness pass.

This plan is intentionally narrow. The major runtime safety work is already in place: bounded Git subprocess execution, immediate process-group termination on cap breach, operation-wide forge response budgets, descriptor-relative local file opening, root-contained symlink following, security-card evidence-role materialization, workflow-resolution precedence, and entity-scoped conflict detection. This pass must preserve those guarantees while completing the remaining semantic and verification contracts.

---

## 1. Required outcome

At completion, Eggsearch must be able to demonstrate all of the following without qualification:

1. Every research subquery carries semantic evidence intent independent of its display or ordering identifier.
2. Every retrieval failure is associated with every evidence role affected by that attempt, not only the first role.
3. Every provider/subquery decision that materially affects coverage has a structured attempt outcome.
4. Native security lookups are represented in the same retrieval ledger as generic search subqueries.
5. Retrieval summaries preserve provider, subquery, role, outcome, result count, failure class, truncation, and timing where available.
6. Conflict records name only the sources whose values actually disagree.
7. Native GitHub, GitLab, Codeberg/Forgejo, and Gitea adapter paths have reproducible non-fallback smoke evidence.
8. CI and release evidence are tied to an explicitly defined release subject commit and durable run identifiers.
9. Performance and bounded-memory claims refer to the affected inventory, subprocess, forge, and evidence paths rather than unrelated serialization microbenchmarks.
10. The codegg-facing response fixtures prove the additive fields remain consumable and semantically consistent.

The repository must remain classified as a **provisional release candidate** until every release-blocking acceptance criterion in this plan is satisfied.

---

## 2. Scope

### In scope

- `src/core/research.rs`
- `src/core/research_planner.rs`
- `src/core/retrieval_status.rs`
- `src/core/evidence_postprocess.rs`
- `src/core/workflow_coverage.rs`
- `src/core/conflict.rs`
- `src/meta/dispatch.rs`
- `src/meta/adapter.rs`
- `src/meta/security_search.rs`
- provider-selection and capability-filtering code used by the four search tools
- MCP response schemas and serialization fixtures
- `tests/native_forge_smoke.rs`
- deterministic forge adapter contract tests
- retrieval, conflict, workflow, and codegg integration tests
- CI workflows, fuzz targets, benchmarks, and release evidence
- `docs/release-verification.md` and directly related architecture/safety documentation

### Non-goals

- adding new search backends
- changing ranking or reciprocal-rank-fusion behavior
- redesigning the provider-health subsystem
- introducing a persistent retrieval database
- changing established public fields incompatibly
- broad changes to local workspace indexing or forge transport
- changing the evidence-role taxonomy unless an existing role is demonstrably unable to represent a required native security source
- claiming Windows support
- implementing connection-time DNS pinning in this pass

All response changes should be additive. Existing fields may be deprecated only with explicit compatibility documentation and fixtures proving old clients continue to deserialize responses.

---

## 3. Cross-cutting invariants

The following invariants are release blocking:

- Semantic evidence intent must be assigned before provider dispatch.
- Opaque identifiers such as `rq_0` must never be used as the sole source of role inference.
- One provider/subquery execution produces one canonical attempt identity.
- A multi-role attempt may produce multiple role-specific coverage failures.
- No failure may be silently discarded because another provider returned results.
- No successful zero-result retrieval may be described as skipped, failed, or not queried.
- No provider that was never eligible or selected may be described as queried.
- A global deadline interruption must remain distinct from a provider-local timeout.
- Rate limiting must remain distinct from generic provider failure in attempt data.
- Partial success followed by truncation must remain distinguishable from complete success.
- Native advisory lookups must not use `if let Ok(...)` patterns that erase failure and zero-result semantics.
- Retrieval summaries must be derived from attempt records whenever an attempt exists.
- Conflict records must include at least two distinct source identities whose normalized values differ.
- Release evidence must never claim native execution when fallback mode ran.
- A test that lacks credentials or prerequisites must skip that test only; it must not terminate the entire test process successfully.
- Verification documentation must distinguish the code-bearing release subject from later evidence-only commits.

---

# Workstream A — Semantic research-subquery intent

## A.1 Problem

Research planning currently gives subqueries opaque identifiers such as `rq_0`, while the actual semantic source type remains in `ResearchSourceType`. Dispatch receives the opaque identifier as the subquery label and asks `map_provider_to_intended_roles(provider, label)` to infer roles. Since labels such as `rq_0` are not semantic, the role mapper falls through to provider-name heuristics or `UnknownOrWeakContext`.

This makes research attempt summaries and failure-aware workflow coverage unreliable even when the planner knew the exact purpose of the query.

## A.2 Required model

Separate three concepts that are currently conflated:

1. **stable subquery identity** — deterministic identifier used for ordering and correlation;
2. **semantic intent** — source type or intent used to derive evidence roles;
3. **query text** — the bounded provider query.

Recommended internal shape:

```rust
pub struct PlannedSubquery {
    pub id: String,
    pub semantic_label: SubquerySemanticLabel,
    pub intended_roles: Vec<EvidenceRole>,
    pub query: String,
    pub priority: i32,
}
```

The exact types may differ, but `id` must not be used to reconstruct semantic intent.

Prefer a typed semantic enum over free-form strings. It may wrap or map directly from existing types:

```rust
pub enum SubquerySemanticLabel {
    PrimarySources,
    OfficialDocumentation,
    Specifications,
    ReferenceImplementation,
    DesignDiscussion,
    Benchmark,
    SecurityConsiderations,
    IssueThread,
    ReleaseNotes,
    AcademicSource,
    RecentNews,
    CommunityDiscussion,
    Counterpoint,
    SourceCode,
    PackageRegistry,
    VendorGuidance,
    DefensiveGuidance,
    ExactError,
    ErrorCode,
}
```

Do not add a public enum merely to mirror an existing public `ResearchSourceType`. A private adapter-level type or direct mapping is sufficient.

## A.3 Explicit research mappings

Map every `ResearchSourceType` deterministically:

| Research source type | Required intended role(s) |
|---|---|
| `PrimarySources` | `OfficialDocumentation`, and `PrimaryImplementation` only where the query explicitly targets source repositories |
| `OfficialDocs` | `OfficialDocumentation` |
| `Specifications` | `InterfaceOrApiDefinition` or `ArchitectureOrDesignDocument`, according to the existing role taxonomy |
| `ReferenceImplementations` | `PrimaryImplementation` |
| `DesignDiscussions` | `PullRequestOrDesignReview` or `ArchitectureOrDesignDocument` |
| `Benchmarks` | `BenchmarkOrPerformanceEvidence` |
| `SecurityConsiderations` | `AuthoritativeSecurityAdvisory` and/or `VendorSecurityGuidance` only when the planned query actually requests those source classes; otherwise `ConfigurationOrFeatureGate` |
| `IssueThreads` | `IssueOrIncidentDiscussion` |
| `ReleaseNotes` | `ReleaseNoteOrChangelog` |
| `AcademicOrFormalSources` | `IndependentCorroboration` |
| `RecentNews` | `CommunityDiscussion` or the closest existing non-authoritative context role |
| `CommunityDiscussion` | `CommunityDiscussion` |
| `Counterpoints` | `CounterpointOrConflictingEvidence` |

Review the current evidence-role enum before implementation and use the closest established role. Do not create overlapping roles.

## A.4 Planner changes

`build_research_search_plan` must produce intended roles alongside each generated subquery.

Required behavior:

- `ResearchSubquery.id` remains stable and deterministic.
- The semantic source type is retained through dispatch.
- Role assignment is independent of provider ID.
- Provider capability may refine or reduce roles only when the provider cannot possibly supply them; it must not replace the planner's intent with a generic provider heuristic.
- If one subquery intentionally targets multiple evidence classes, preserve all roles.
- Role order must be deterministic.
- Duplicate roles must be removed before dispatch.

## A.5 Shared dispatch interface

Update the shared dispatch helper so callers provide `intended_roles` directly.

Do not call `map_provider_to_intended_roles` after semantic intent has already been established. Retain the mapper only for simple legacy paths that truly lack a typed planner, and add a deprecation comment explaining that it is a fallback.

Repo and security planners should also pass explicit semantic labels and roles rather than rely on reconstructed strings where practical.

## A.6 Tests

Add table-driven tests covering every `ResearchSourceType`.

Required tests:

1. every research source type produces a non-empty intended-role set;
2. `rq_0`, `rq_1`, and ordering changes do not affect role assignment;
3. identical semantic source types at different positions produce identical roles;
4. benchmark subqueries produce `BenchmarkOrPerformanceEvidence`;
5. official documentation produces `OfficialDocumentation`;
6. reference implementation produces `PrimaryImplementation`;
7. counterpoint produces `CounterpointOrConflictingEvidence`;
8. security consideration does not default to `UnknownOrWeakContext`;
9. provider-name changes do not alter planner-assigned roles;
10. a multi-role subquery retains all roles through dispatch and serialization;
11. explicit workflow changes coverage expectations without changing the semantic role of an individual subquery;
12. property tests randomize subquery order and assert role stability.

## A.7 Acceptance criteria

- No production research dispatch job derives its intended role solely from an `rq_*` identifier.
- Every planned research job has at least one deterministic intended role.
- Research retrieval summaries use those roles.
- Research coverage failures are assigned to the source type actually requested.
- Regression tests prove role assignment is stable under subquery reordering.

---

# Workstream B — Preserve all roles affected by a retrieval failure

## B.1 Problem

`RetrievalAttempt` supports multiple `intended_roles`, but failure conversion currently selects only `.first()`. A failed attempt intended to supply multiple required roles can therefore make one role indeterminate while incorrectly reporting another as definitively missing.

## B.2 Required API

Replace singular conversion:

```rust
fn to_retrieval_failure(&self) -> Option<RetrievalFailure>
```

with plural conversion:

```rust
fn to_retrieval_failures(&self) -> Vec<RetrievalFailure>
```

or an allocation-conscious iterator/small-vector equivalent.

Rules:

- failed, timed-out, rate-limited, and deadline-interrupted attempts produce one `RetrievalFailure` per distinct intended role;
- attempts with no intended roles produce one `UnknownOrWeakContext` failure only as a final fallback;
- zero-result success is not a provider failure;
- policy and capability skips map to their precise non-error failure kinds only where coverage semantics require them;
- truncation produces `ResultTruncatedByCap` for every role whose retrieval was incomplete;
- duplicate roles in one attempt do not create duplicate failures.

## B.3 Failure-kind mapping

Use this mapping:

| Attempt outcome | Retrieval failure kind |
|---|---|
| `Failed` | `ProviderFailed` |
| `TimedOut` | `DeadlinePreventedCompletion` or a dedicated provider-timeout kind only if added compatibly |
| `RateLimited` | `ProviderFailed`, while preserving `rate_limited` in structured attempt outcome and error class |
| `InterruptedByDeadline` | `DeadlinePreventedCompletion` |
| `SkippedByPolicy` | `ProviderSkippedByPolicy` |
| `SkippedCapabilityUnavailable` | `ProviderCapabilityUnavailable` |
| `TruncatedAfterPartialSuccess` | `ResultTruncatedByCap` |
| `SuccessZeroResults` | `NoMatchingEvidenceFound` only for coverage-gap analysis, not provider failure telemetry |
| `NotApplicable` | no missing-evidence failure unless the workflow model incorrectly requested that role |

Do not collapse the structured attempt outcome merely because the older `RetrievalFailureKind` taxonomy is coarser.

## B.4 Deduplication

Deduplicate failures using a deterministic composite key:

```text
(provider_id, subquery_id, evidence_role, failure_kind)
```

Do not deduplicate solely by provider ID. The same provider can fail one subquery and succeed another.

## B.5 Coverage semantics

Required status behavior:

- missing required role plus a failed attempt intended for that role → `IndeterminateDueToFailures`;
- missing required role plus completed zero-result attempts for that role → `Insufficient`;
- missing required role plus only policy skips → `Insufficient` or capability/policy-specific incomplete status according to current contract, but not provider failure;
- unrelated failed roles must not make otherwise complete required coverage indeterminate;
- recommended-role failures may reduce confidence and generate next actions without automatically changing required-role sufficiency;
- found role plus failed redundant provider for the same role must not make that role missing.

## B.6 Tests

Required tests:

1. one failed attempt with two intended required roles creates two failures;
2. duplicate intended roles create one failure per unique role;
3. empty intended roles produce one unknown-role failure;
4. one provider fails docs but succeeds source; only docs is affected;
5. one advisory attempt intended for advisory and vendor guidance fails; security coverage becomes indeterminate for both missing roles;
6. one role is found by another provider; redundant failure does not make that role missing;
7. rate limit remains `RateLimited` in attempt data while mapping to the documented coverage failure class;
8. deadline interruption is distinct from provider timeout in summary fields;
9. failure ordering is deterministic;
10. property tests assert failure count equals unique intended-role count for failure outcomes.

## B.7 Acceptance criteria

- No production call to `.first()` on `intended_roles` determines the complete failure set.
- Every affected required role receives a failure record.
- Coverage status changes only for roles actually affected.
- Failure serialization remains deterministic and additive.

---

# Workstream C — Complete production retrieval-attempt ledger

## C.1 Problem

The attempt enum defines policy skip, capability skip, not-applicable, and partial-truncation outcomes, but the reviewed production dispatcher emits only result, zero-result, generic failure, timeout, rate-limit, and global-deadline outcomes.

A schema variant that is never emitted does not satisfy the operational accounting contract.

## C.2 Ledger boundary

Define exactly what counts as a planned attempt.

A ledger entry is required when:

- a provider/subquery pair was selected for execution;
- an explicitly requested provider/subquery pair was rejected by policy;
- a selected provider lacks a capability required for that subquery;
- a planner deliberately marks a candidate as not applicable;
- a dispatched provider returns complete results, zero results, an error, timeout, or rate limit;
- a global deadline prevents a selected job from starting or completing;
- a response/result cap truncates an otherwise successful retrieval.

Do not emit attempt records for every installed provider. Only selected, explicitly requested, or deliberately excluded candidate pairs belong in the ledger.

## C.3 Selection-stage records

Provider selection currently occurs before dispatch. Extend selection output with structured exclusions, for example:

```rust
pub struct ProviderSelectionDecision {
    pub provider_id: String,
    pub subquery_id: String,
    pub intended_roles: Vec<EvidenceRole>,
    pub disposition: SelectionDisposition,
    pub reason_code: Option<String>,
}

pub enum SelectionDisposition {
    Selected,
    SkippedByPolicy,
    CapabilityUnavailable,
    NotApplicable,
}
```

The exact shape may differ. The key requirement is that selection decisions can become attempt records without inferring them from missing results.

## C.4 Dispatch-stage records

For every selected job:

- allocate a stable attempt identity before spawn;
- retain provider ID, subquery ID, intended roles, and a bounded query fingerprint;
- record start time;
- update outcome exactly once;
- preserve zero-result success;
- synthesize `InterruptedByDeadline` for pending and aborted jobs;
- record provider-local timeout separately from global deadline interruption;
- mark truncation if provider response, candidate cap, aggregation cap, or operation budget caused partial results.

Use first-terminal-write semantics so panic, deadline, and provider completion cannot produce duplicate terminal records.

## C.5 Truncation semantics

Emit `TruncatedAfterPartialSuccess` only when usable results were returned and completeness was limited.

Potential sources:

- provider explicitly reports truncation;
- result list exceeded requested candidate cap;
- forge operation returns partial entries due page or byte cap;
- aggregation retains only part of a provider/subquery result set;
- a global response cap is reached after some results are retained.

Do not mark a zero-result or failed request as partial success.

Add a bounded `truncation_reason` field if compatible, or retain reason in `error_class`/message with a stable code.

## C.6 Query fingerprinting

Never serialize raw sensitive queries merely for attempt correlation.

Use one of:

- stable hash of normalized query text;
- bounded semantic label plus hash;
- existing query-fingerprint utility.

Exact-error queries may contain credentials, file paths, tokens, or proprietary source fragments. The fingerprint must not preserve recoverable query content.

## C.7 Tool coverage

Complete attempt production for:

- `web_search` provider calls;
- `repo_search` planned subqueries;
- `research_search` planned subqueries;
- `security_search` generic subqueries;
- native security lookups in Workstream D;
- local workspace participation where it materially contributes to workflow coverage;
- native forge/repository paths where retrieval summary fields are exposed.

A single-query tool may synthesize one attempt per selected provider without using the multiquery dispatcher.

## C.8 Tests

Create a table-driven attempt matrix covering every enum variant.

Required tests:

1. complete success with results;
2. complete success with zero results;
3. provider failure;
4. provider-local timeout;
5. HTTP 429 rate limit;
6. explicit policy exclusion;
7. capability exclusion;
8. planner not-applicable decision;
9. pending job interrupted by global deadline;
10. running job interrupted by global deadline;
11. partial results truncated by candidate cap;
12. forge partial results truncated by byte budget;
13. same provider serving multiple subqueries produces distinct attempts;
14. selected job produces exactly one terminal record;
15. query fingerprint does not expose raw exact-error text;
16. attempt ordering is deterministic independent of completion order;
17. provider panic yields a failed terminal attempt rather than disappearing;
18. property tests generate job schedules and assert every selected job has exactly one terminal outcome.

## C.9 Acceptance criteria

- Every selected provider/subquery pair has exactly one terminal attempt.
- Every explicitly excluded candidate has the correct skip outcome.
- All declared attempt outcomes have at least one exercised production path or are removed from the public enum before release.
- Zero-result, policy skip, capability skip, provider timeout, global deadline, rate limit, and truncation are distinguishable in serialized output.
- No raw query secrets are added to telemetry.

---

# Workstream D — Native security retrievals in the ledger

## D.1 Problem

Direct CVE, GHSA, OSV, RustSec, package-advisory, and KEV operations execute outside the generic dispatcher. Several use `if let Ok(...)`, so failures and successful no-result lookups disappear from coverage and retrieval summaries.

These are the most authoritative security retrievals and must not be less observable than generic web search.

## D.2 Native attempt collector

Introduce a small request-scoped collector, for example:

```rust
struct NativeSecurityAttemptCollector {
    attempts: Vec<RetrievalAttempt>,
}
```

or simply append attempts through a helper that guarantees deterministic IDs and outcome mapping.

Every direct native operation must record:

- provider ID;
- semantic subquery ID;
- intended roles;
- normalized identifier class without leaking secrets;
- outcome;
- result count;
- error class;
- duration;
- deadline/truncation flags where relevant.

## D.3 Required subquery identifiers

Use stable semantic IDs, not loop indexes:

- `advisory_by_cve`
- `advisory_by_ghsa`
- `advisory_by_osv`
- `advisory_by_rustsec`
- `advisory_by_package`
- `kev_by_cve`
- `dependency_manifest_read`
- `applicability_evaluation` only if represented as retrieval rather than deterministic analysis

Append a bounded hash suffix when multiple identifiers of the same class must be distinguished without exposing the identifier in telemetry.

## D.4 Outcome rules

For each lookup:

- `Ok(Some(record))` → `SuccessWithResults`, count 1;
- `Ok(None)` → `SuccessZeroResults`;
- `Ok(vec![])` → `SuccessZeroResults`;
- `Ok(nonempty)` → `SuccessWithResults`, exact bounded count;
- timeout → `TimedOut`;
- HTTP 429 → `RateLimited`;
- other provider error → `Failed` with coarse error class;
- global request deadline → `InterruptedByDeadline`;
- result cap after some advisories → `TruncatedAfterPartialSuccess`.

Never discard an error with `if let Ok` or `.ok()?` in orchestration paths that affect coverage.

## D.5 Intended roles

Use actual native purpose:

- OSV/advisory lookup → `AuthoritativeSecurityAdvisory`;
- vendor advisory lookup → `VendorSecurityGuidance`;
- package query → `AuthoritativeSecurityAdvisory` and `ManifestOrDependencyMetadata` only when both are genuinely part of the retrieval;
- KEV lookup → use the closest established authoritative exploitation/advisory role; add a new role only if the current taxonomy cannot accurately represent it and all schema compatibility requirements are met;
- dependency file read → `ManifestOrDependencyMetadata`;
- defensive guidance fetch → `ConfigurationOrFeatureGate` or `VendorSecurityGuidance` according to source type.

Do not assign generic web discussion to authoritative advisory roles.

## D.6 Merge behavior

Merge native and generic attempts before:

- failure conversion;
- workflow coverage;
- retrieval summary generation;
- next-action generation;
- response serialization.

Deduplicate only exact duplicate executions. Native lookup and generic advisory-search attempts are different records even when they target the same role.

## D.7 Error visibility

Continue returning partial security results when appropriate, but add structured warnings tied to attempt outcomes.

Warnings should distinguish:

- authoritative provider unavailable;
- authoritative provider returned zero matches;
- package query could not execute due missing ecosystem/package;
- KEV lookup failed;
- dependency file could not be read safely;
- deadline prevented completion.

A missing authoritative result after a completed zero-result lookup is not equivalent to a failed lookup.

## D.8 Tests

Required tests:

1. CVE lookup found;
2. CVE lookup zero result;
3. GHSA lookup failure;
4. OSV package query rate limited;
5. package query returns multiple advisories;
6. package query truncates after partial success;
7. KEV found;
8. KEV absent;
9. KEV failure;
10. multiple identifiers produce distinct attempts;
11. duplicate identifiers are looked up once and create one attempt;
12. native advisory failure makes missing advisory coverage indeterminate;
13. native zero-result lookup makes missing advisory coverage insufficient, not indeterminate;
14. generic web success does not mask native advisory failure;
15. serialized security summary contains both generic and native attempts;
16. direct lookup errors are never silently discarded;
17. codegg fixture can distinguish no advisory found from advisory provider failed.

## D.9 Acceptance criteria

- Every executed native security operation has a terminal attempt.
- Native failures and zero results influence coverage correctly.
- Security retrieval summaries include native attempts.
- No authoritative lookup error is erased by orchestration control flow.
- Generic results cannot make failed authoritative retrieval appear complete.

---

# Workstream E — Complete attempt-derived retrieval summaries

## E.1 Problem

Attempt-derived summaries are now preferred, but each attempt is reduced to the first intended role and legacy `RetrievalDimensionStatus` fields omit several required dimensions.

## E.2 Additive summary schema

Extend `RetrievalDimensionStatus` additively with fields such as:

```rust
pub struct RetrievalDimensionStatus {
    pub evidence_role: EvidenceRole,
    pub absence_kind: EvidenceAbsenceKind,
    pub provider_id: Option<String>,
    pub message: String,
    pub query: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subquery_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt_outcome: Option<RetrievalAttemptOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
}
```

The exact field names may differ. Existing fields must retain their meaning.

## E.3 One attempt, multiple roles

For a multi-role attempt, emit one summary dimension per role, with the same provider/subquery/outcome identity.

Alternatively, add `intended_roles` as an array while retaining one primary `evidence_role`, but coverage and clients must not lose any role. The one-dimension-per-role approach is preferred because it aligns with existing role-based coverage.

## E.4 Summary construction

When attempts are present:

- do not infer success from aggregated card provider membership;
- do not label all successful providers as `PrimaryImplementation`;
- preserve successful zero results;
- preserve rate limit as a structured attempt outcome;
- preserve provider-local timeout versus global deadline;
- preserve truncation;
- preserve subquery identity;
- preserve exact bounded result count;
- derive `absence_kind` deterministically from outcome;
- use bounded, stable messages suitable for agents.

Retain the provider/card fallback only for legacy paths that genuinely cannot yet emit attempts. Add telemetry or a debug assertion identifying fallback usage so remaining legacy paths are visible.

## E.5 Aggregated summary fields

Review `ResponseRetrievalSummary` aggregation so it can report:

- attempted job count;
- completed job count;
- zero-result count;
- failed count;
- timed-out count;
- rate-limited count;
- policy-skipped count;
- capability-skipped count;
- deadline-interrupted count;
- truncated count;
- roles attempted;
- roles with complete successful retrieval;
- roles indeterminate due failure.

These aggregate fields should be additive and bounded.

## E.6 Next actions

Use attempt history when generating next actions:

- do not recommend retrying the exact same failed provider/subquery without a rationale;
- prefer an alternate eligible provider for the same role;
- after rate limiting, recommend a different provider or delayed retry rather than immediate identical retry;
- after zero results, recommend a narrower or alternate semantic query;
- after capability skip, recommend a provider that supports the capability;
- after truncation, recommend reducing scope or increasing an allowed cap when operator policy permits;
- use tool-registry-valid templates;
- preserve known source IDs where relevant;
- bound action count and deduplicate equivalent actions.

## E.7 Tests

Required tests:

1. one successful multi-role attempt creates dimensions for all roles;
2. zero-result summary retains `result_count=0` and `SuccessZeroResults`;
3. rate limit retains `RateLimited` and coarse coverage failure mapping;
4. provider timeout and global deadline serialize differently;
5. truncation is explicit;
6. subquery ID is retained;
7. fallback summary is not used when attempts exist;
8. summary ordering is deterministic by subquery/provider/role;
9. aggregate counts equal dimension-derived counts;
10. schema snapshots remain backward compatible;
11. old clients ignoring additive fields still deserialize;
12. codegg fixture consumes the enriched summary;
13. next actions avoid identical failed provider/query combinations;
14. property tests assert summary dimensions preserve every intended role.

## E.8 Acceptance criteria

- Attempt-derived summaries preserve all intended roles.
- Every summary dimension can be traced to a provider/subquery attempt.
- Result count, outcome, and truncation are structured, not only embedded in prose.
- No production multiquery path uses provider/card inference when attempts are available.
- Codegg integration fixtures validate the additive fields.

---

# Workstream F — Exact conflict-source attribution

## F.1 Problem

Conflict detection now requires distinct sources and compares normalized per-source sets, but emitted conflict records may include every card in an entity/package group rather than only the cards whose values form the disagreement.

This does not generally recreate the old false positive, but it weakens provenance and can lead agents to inspect unrelated sources.

## F.2 Comparison-group model

Group values by normalized value set while retaining source IDs:

```rust
struct NormalizedValueGroup {
    normalized_value: String,
    source_ids: BTreeSet<String>,
    provider_ids: BTreeSet<String>,
}
```

For set-valued fields, normalize the entire set before grouping.

A conflict exists when at least two normalized value groups remain after semantic comparability checks.

## F.3 Emitted source IDs

For each conflict:

- include only source IDs in the disagreeing value groups;
- do not include cards with no value for the compared field;
- do not include cards whose normalized value agrees with neither selected comparison if the conflict representation only carries two values;
- if more than two distinct values exist, either include all value groups in a bounded deterministic conflict or emit pairwise conflicts with stable deduplication;
- align `values` and source provenance deterministically;
- preserve provider IDs separately from source-card IDs where the schema supports them.

## F.4 Stable conflict identity

Compute conflict IDs from:

- canonical entity key;
- compared field;
- normalized value groups;
- sorted disagreeing source IDs.

The ID must be stable under input ordering and provider aggregation ordering.

## F.5 Vulnerability rules

For vulnerability fields:

- scope by canonical advisory ID, ecosystem, package, and field;
- compare patched-version sets per source;
- compare affected ranges per source only when syntax is semantically comparable;
- compare `published_at` only with `published_at`, not modified or withdrawn dates;
- skip cards missing the compared field;
- do not infer conflict from one card containing multiple valid patched versions.

## F.6 Repository rules

For mutable-versus-pinned conflicts:

- require identical normalized host, owner, and repository;
- source IDs must correspond to the mutable and pinned cards only;
- do not include unrelated repository cards from the same owner;
- keep `directly_comparable=false` unless content identity is actually compared.

## F.7 Tests

Required tests:

1. three cards where two disagree and one lacks the field; the third ID is excluded;
2. three cards where two agree and one differs; all actual participating IDs are represented correctly;
3. one card with multiple patched versions creates no conflict;
4. same patched-version set in different order creates no conflict;
5. different package under same advisory is excluded;
6. same repository name on different host is excluded;
7. conflict ID is stable under card permutation;
8. duplicate provider contributions to one card do not create extra sources;
9. more than two distinct normalized values are represented deterministically;
10. property tests assert every emitted source ID has one of the emitted compared values;
11. property tests assert every conflict has at least two distinct source IDs.

## F.8 Acceptance criteria

- Every conflict source ID corresponds to a card that supplied a disagreeing value.
- No unrelated card is included merely because it shared an entity group.
- Conflict IDs are order independent.
- Multi-value disagreements are bounded and deterministic.

---

# Workstream G — Native forge smoke evidence and release-proof protocol

## G.1 Problem

Native smoke infrastructure exists, but it has not produced release evidence. The current test helper calls `std::process::exit(0)` when no tokens are configured, which can terminate the entire test binary and prevent independently configured provider tests from running. The suite covers GitHub, GitLab, and Codeberg but does not provide a distinct public Gitea target. The current release record references the implementation commit rather than the later verification head, and no durable CI/native run identifiers are recorded.

## G.2 Fix test skipping

Remove process-wide successful exit from test helpers.

Each test must independently:

- inspect only its own prerequisites;
- return early with a clear skip message when unavailable;
- never suppress other provider tests;
- report whether it executed or skipped in a machine-readable summary when run through the release workflow.

A missing token for GitHub must not prevent GitLab or Codeberg tests from executing.

## G.3 Test adapters directly

Where possible, native smoke tests should call `forge_adapter::fetch_tree` or a narrowly exposed test API directly rather than depend on provider-selection configuration.

Benefits:

- proves the native adapter path without fallback ambiguity;
- allows unauthenticated public endpoints where supported;
- separates adapter correctness from provider-registration policy;
- makes provider ID and byte-budget telemetry directly assertable.

End-to-end `repo_map` native-mode tests should remain as an additional layer, not the only proof.

## G.4 Provider matrix

Required native targets:

1. GitHub public repository;
2. GitHub slash-containing ref using a stable controlled test branch;
3. GitLab public repository;
4. Codeberg or another Forgejo public repository;
5. a distinct Gitea public instance;
6. optional self-hosted Forgejo target when operator credentials are available.

Use controlled canary repositories/branches where external branch names or repository structures are otherwise unstable. Document ownership and expected immutable refs.

For the slash-ref case, use an actual slash-containing branch such as `smoke/slash-ref`; `v0.7.x` is not sufficient.

## G.5 Native assertions

Each native smoke must assert:

- native adapter function/provider ID was used;
- no generic fallback mode was accepted;
- resolved commit SHA is structurally valid;
- requested ref and resolved ref are preserved distinctly;
- tree entries are non-empty;
- entry URLs use resolved commit SHA where supported;
- commit, tree, and object identities are not conflated;
- byte telemetry is present;
- `aggregate_observed <= aggregate_limit`;
- request count is nonzero;
- redirects were not followed;
- no token or credential appears in output/logs.

## G.6 Scheduled/manual workflow

Add a dedicated workflow, for example `native-forge-smoke.yml`, with:

- `workflow_dispatch`;
- optional scheduled execution;
- per-provider jobs so one provider failure does not hide others;
- repository/environment secrets scoped per job;
- explicit timeout;
- concurrency control;
- no pull-request exposure of secrets;
- artifact upload of a sanitized evidence manifest;
- nonzero exit if a configured provider test fails;
- explicit executed/skipped status per target.

The evidence manifest should include:

```json
{
  "release_subject": "<sha>",
  "workflow_run_id": 123,
  "provider": "github",
  "target": "owner/repo",
  "requested_ref": "smoke/slash-ref",
  "resolved_commit_sha": "...",
  "entry_count": 123,
  "aggregate_observed": 4567,
  "aggregate_limit": 10485760,
  "result": "pass",
  "executed_at": "..."
}
```

Do not include API tokens, raw authorization headers, or full response bodies.

## G.7 Release-subject protocol

A commit cannot contain a file that truthfully names its own final SHA because modifying the file changes the SHA. Use an explicit two-commit protocol.

1. **Release subject commit (`R`)**
   - final code-bearing commit;
   - no known implementation changes remain;
   - full deterministic CI and native smoke run against `R`.
2. **Evidence commit (`E`)**
   - updates only `docs/release-verification.md` and permitted evidence manifests/pointers;
   - names `R` as the verified runtime subject;
   - records exact CI/native workflow run IDs for `R`;
   - contains no production code changes.
3. Run docs/schema/format checks on `E`.
4. Verify `git diff --name-only R..E` contains only approved evidence files.
5. Create the release-candidate tag at `E`.
6. The verification record must state both:
   - `release_subject_commit: R`;
   - `evidence_commit: E` or the tag that binds the evidence.

Do not claim that `E` itself was the code-bearing subject unless full CI/native tests were rerun on identical code and the distinction is documented.

## G.8 Remote CI proof

For `R`, record durable identifiers for:

- Linux feature matrix;
- macOS feature matrix;
- clippy;
- formatting;
- documentation;
- release build;
- publish dry run;
- schema/corpus tests;
- hardening tests;
- fuzz smoke;
- native forge smoke.

If the connector or status API cannot expose runs, use GitHub Actions run URLs/IDs captured by the workflow itself. Lack of externally visible proof keeps the classification provisional.

## G.9 Performance and bounded-memory evidence

Add affected-path benchmarks/tests:

- cold local inventory construction at representative file counts;
- warm inventory search;
- tracked plus untracked inventory near configured caps;
- bounded Git command under stdout saturation;
- bounded Git command under stderr saturation;
- multi-page GitLab/Forgejo parsing under aggregate budget;
- forge response retained-byte assertion relative to configured limit;
- attempt-ledger construction for representative job counts;
- retrieval-summary postprocessing for representative attempts/roles;
- conflict grouping for representative source-card counts.

Record configured caps and peak retained structures. Avoid claiming “no unbounded memory growth” from latency-only Criterion results.

A deterministic allocation-bound test or heap profiler artifact may be used. If exact peak memory cannot be measured portably, state the structural bound and test retained vector/body lengths directly.

## G.10 Fuzz/property targets

Add missing release-blocking targets or equivalent property suites for:

- aggregate forge-budget state transitions;
- termination-controller trigger races;
- retrieval-attempt to failure expansion;
- attempt-derived summary generation;
- workflow-resolution precedence;
- semantic research-role mapping;
- native security attempt outcome mapping;
- sourced conflict attribution;
- symlink-follow policy decisions.

Every declared fuzz target must appear in the CI matrix or be explicitly classified as scheduled/manual with a documented reason.

## G.11 Acceptance criteria

- No native smoke helper calls `std::process::exit(0)` for missing prerequisites.
- Each provider test executes or skips independently.
- GitHub, GitLab, Codeberg/Forgejo, and a distinct Gitea target have executed native evidence.
- An actual slash-containing ref is tested.
- Native evidence manifest records adapter identity, immutable SHA, entry count, and byte telemetry.
- Remote CI run IDs exist for the release subject.
- Release documentation names `R` and `E` using the two-commit protocol.
- `R..E` contains only approved evidence files.
- Performance claims refer to affected bounded paths.
- Classification remains provisional until all native and CI evidence is present.

---

# Workstream H — codegg contract and documentation closure

## H.1 codegg integration contract

Update end-to-end fixtures to prove codegg can consume:

- semantic research attempt roles;
- multi-role failures;
- native security attempts;
- enriched retrieval dimensions;
- explicit zero-result versus failure outcomes;
- rate-limit and deadline distinctions;
- conflict records with exact source IDs;
- workflow-resolution source;
- gap-driven next actions.

Fixtures must not require codegg to understand every additive field. Existing parsing should remain valid while new fields are available to newer clients.

## H.2 Schema compatibility

Regenerate or update schema snapshots.

Required checks:

- old fixture without new optional fields deserializes;
- new fixture deserializes with old optional fields ignored by a compatibility harness;
- enum serialization names remain stable;
- no required field is added to existing public response types;
- bounded collection limits remain documented;
- tool descriptions accurately explain retrieval-summary semantics.

## H.3 Documentation updates

Audit:

- `AGENTS.md`
- `docs/architecture/meta.md`
- `docs/architecture/overview.md`
- `docs/safety.md`
- `docs/config.md`
- tool schema descriptions
- `docs/release-verification.md`

Required documentation statements:

- research roles are planner-derived, not identifier-derived;
- failures expand across all intended roles;
- attempt outcomes and absence kinds are related but distinct;
- native advisory calls participate in coverage;
- conflict source IDs identify actual disagreeing cards;
- native smoke tests are distinct from fallback live smoke;
- DNS validation remains preflight-only;
- Windows remains unsupported unless separately established;
- release evidence uses the `R`/`E` protocol.

## H.4 Static guards

Use static guards only for architectural regressions that cannot be expressed better at runtime.

Useful guards:

- no `std::process::exit(0)` in native smoke tests;
- no `.first()`-only conversion of `intended_roles` in retrieval failure/summary code;
- no research dispatch role derivation from `rq_` labels;
- no silent `if let Ok` around native advisory operations that affect coverage;
- no fallback-mode acceptance in native smoke assertions;
- no release classification of stable RC without native evidence manifest references.

Runtime and integration tests remain the primary proof.

## H.5 Acceptance criteria

- codegg fixtures consume all additive output successfully.
- Existing response fixtures remain compatible.
- Documentation matches actual production behavior.
- Static guards protect only high-value architectural constraints.

---

## 4. Recommended implementation sequence

Use small, reviewable commits. Recommended sequence:

### Commit 1 — semantic subquery intent

- add typed semantic intent/intended roles to planned subqueries;
- update research planner mappings;
- pass roles directly into dispatch;
- add mapping tests.

Suggested message:

```text
fix(research): carry semantic evidence roles through dispatch
```

### Commit 2 — multi-role failure conversion

- pluralize retrieval failure conversion;
- deduplicate per provider/subquery/role/kind;
- update coverage tests.

Suggested message:

```text
fix(evidence): preserve all roles affected by retrieval failures
```

### Commit 3 — complete selection and dispatch ledger

- add structured selection exclusions;
- emit missing attempt outcomes;
- add truncation and query-fingerprint handling;
- add scheduler/property tests.

Suggested message:

```text
fix(retrieval): complete provider-subquery attempt accounting
```

### Commit 4 — native security attempt integration

- instrument direct advisory/package/KEV lookups;
- merge native and generic attempts;
- eliminate silent error swallowing;
- add security coverage tests.

Suggested message:

```text
fix(security): include native advisory retrievals in evidence ledger
```

### Commit 5 — enriched summaries and next actions

- add optional summary fields;
- produce one role dimension per attempt role;
- update aggregate counts and action generation;
- update schemas and codegg fixtures.

Suggested message:

```text
fix(evidence): expose complete attempt-derived retrieval summaries
```

### Commit 6 — exact conflict provenance

- group normalized values with source IDs;
- emit only disagreeing sources;
- add property tests.

Suggested message:

```text
fix(conflicts): attribute disagreements to exact source cards
```

### Commit 7 — verification infrastructure

- fix per-test native smoke skips;
- add direct-adapter native tests;
- add Gitea and real slash-ref targets;
- add scheduled/manual workflow and sanitized artifacts;
- add affected-path benchmarks/fuzz targets.

Suggested message:

```text
test(release): add reproducible native forge and bounded-path evidence
```

### Commit 8 — release subject

- documentation implementation audit;
- full deterministic gates;
- freeze production code as release subject `R`.

Suggested message:

```text
chore(release): freeze retrieval-ledger closure subject
```

### Commit 9 — evidence-only closure

After CI and native smoke complete for `R`:

- regenerate release verification;
- record run IDs/artifact references;
- document exact residual limitations;
- verify only evidence files changed;
- create evidence commit `E` and tag it.

Suggested message:

```text
docs(release): record verified retrieval-ledger closure evidence
```

Do not combine implementation and claimed verification evidence in one unverified commit.

---

## 5. Required command matrix

Run from a clean checkout of release subject `R`:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo test --locked --no-default-features
cargo test --locked --features mock
cargo test --locked --features pdf
cargo test --features mock --test evidence_integration
cargo test --features mock --test property_retrieval
cargo test --features mock --test property_conflict
cargo test --features mock --test integration
cargo test --features mock --test security_applicability_corpus
cargo test --features mock --test research_evidence_corpus
cargo test --features mock --test evidence_bundle_handoff
cargo test --features mock --test static_guards
cargo build --release
cargo publish --dry-run --locked
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
```

Add targeted suites introduced by this plan, for example:

```bash
cargo test --features mock --test retrieval_attempt_ledger
cargo test --features mock --test native_security_attempts
cargo test --features mock --test research_semantic_roles
cargo test --features mock --test conflict_source_attribution
cargo test --features mock --test codegg_evidence_contract
```

Run native evidence through the dedicated workflow or equivalent controlled environment. Do not substitute fallback live-smoke output.

Run Criterion or targeted benches for affected paths and retain raw output as an artifact.

---

## 6. Global acceptance criteria

The implementation is complete only when every item below is true.

### Retrieval intent

- [ ] Every research subquery carries typed semantic intent and deterministic intended roles.
- [ ] Opaque `rq_*` IDs are never used as the sole role source.
- [ ] Role mapping is stable under subquery reordering.

### Failure expansion

- [ ] Multi-role failed attempts produce one failure per distinct role.
- [ ] Coverage indeterminacy is limited to affected missing roles.
- [ ] No `.first()`-only failure conversion remains.

### Attempt ledger

- [ ] Every selected provider/subquery pair has exactly one terminal attempt.
- [ ] Policy, capability, not-applicable, deadline, rate-limit, zero-result, and truncation outcomes have real production paths.
- [ ] Attempt ordering is deterministic.
- [ ] Query telemetry cannot expose raw exact-error content.

### Native security

- [ ] CVE/GHSA/OSV/RustSec/package/KEV operations emit attempts.
- [ ] Native zero results and native failures remain distinguishable.
- [ ] Native failures influence security coverage.
- [ ] No authoritative lookup error is silently discarded.

### Retrieval summary

- [ ] Summary dimensions preserve provider, subquery, role, outcome, result count, error class, duration, and truncation where available.
- [ ] Multi-role attempts preserve all roles.
- [ ] Attempt-derived summaries replace provider/card inference on all instrumented paths.
- [ ] Aggregate counts reconcile with dimensions.

### Conflict provenance

- [ ] Every emitted conflict references only cards supplying disagreeing values.
- [ ] Conflict IDs are order independent.
- [ ] Same-card, cross-package, and cross-host false positives remain closed.

### Native forge evidence

- [ ] Native smoke tests skip independently.
- [ ] No process-wide success exit remains in the test harness.
- [ ] GitHub native adapter passes.
- [ ] Actual GitHub slash-ref native adapter passes.
- [ ] GitLab native adapter passes.
- [ ] Codeberg/Forgejo native adapter passes.
- [ ] Distinct Gitea native adapter passes.
- [ ] Sanitized evidence artifacts are retained.

### CI and release proof

- [ ] Full Linux and macOS deterministic matrices pass on release subject `R`.
- [ ] Required remote workflow run IDs are recorded.
- [ ] Affected-path benchmarks and bounded-memory assertions are retained.
- [ ] Fuzz/property coverage includes all new semantic state transitions.
- [ ] Evidence commit `E` changes only approved evidence files relative to `R`.
- [ ] Release record names both `R` and `E`/tag.
- [ ] Release classification is not promoted without native evidence.

### codegg compatibility

- [ ] codegg fixtures consume enriched retrieval summaries.
- [ ] Existing clients can ignore additive fields.
- [ ] Tool schema snapshots and documentation are current.

---

## 7. Explicit closure tests

The final handoff review should be able to answer “yes” to each question with a cited test or artifact:

1. Can a benchmark research subquery at `rq_7` still be recognized as benchmark evidence?
2. Can one failed attempt mark two required roles indeterminate?
3. Can a successful zero-result advisory lookup be distinguished from an advisory provider failure?
4. Can a rate-limited provider be distinguished from a policy-excluded provider?
5. Can a pending global-deadline cancellation be distinguished from provider-local timeout?
6. Can a partial response be distinguished from complete success?
7. Does every selected job have exactly one terminal attempt?
8. Are direct OSV/CVE/GHSA/RustSec and KEV calls visible in the security retrieval summary?
9. Does the retrieval summary expose the actual subquery and role rather than infer implementation evidence?
10. Does a conflict list only the cards that supplied conflicting values?
11. Does a missing GitHub token leave GitLab native smoke able to execute?
12. Does the slash-ref test use an actual slash-containing ref?
13. Is there native evidence for a distinct Gitea instance?
14. Are native smoke artifacts tied to the release subject SHA?
15. Does the verification record distinguish code subject `R` from evidence commit `E`?
16. Do performance claims cover inventory, subprocess, forge budget, and evidence paths?
17. Can current codegg fixtures consume the response without special-case parsing failures?

Any “no” answer leaves this plan open.

---

## 8. Rollback and failure handling

- Keep public fields additive so individual workstreams can be reverted independently.
- If typed semantic labels require excessive public-schema churn, keep them private to planning/dispatch and serialize only existing public concepts.
- If selection-stage skip instrumentation destabilizes provider routing, land execution attempts first but do not claim complete ledger closure until skip records are added.
- If a native public provider is operationally unreliable, retain deterministic mock contract tests and classify live smoke as scheduled/manual; do not weaken native assertions to accept fallback.
- If a Gitea public instance cannot be made stable, create or designate a controlled canary instance/repository rather than removing the target.
- If benchmark tooling cannot provide portable peak RSS, test structural retained-byte limits and document the measurement boundary.
- If native security APIs lack structured error classes, add a bounded coarse classification adapter rather than serializing raw provider errors.
- Do not promote release status to compensate for unavailable evidence.

---

## 9. Definition of done

This line of work is closed only when:

- the semantic retrieval ledger is complete for planned generic and native operations;
- every intended role survives planning, dispatch, failure conversion, summary construction, and coverage evaluation;
- native security failures and zero results are explicit;
- retrieval summaries expose structured attempt identity and outcome;
- conflict source attribution is exact;
- native forge adapters have executed non-fallback evidence across the required provider families;
- remote CI evidence exists for a frozen code-bearing release subject;
- an evidence-only commit records those runs without changing production code;
- codegg integration fixtures pass;
- no known remaining defect can materially misstate retrieval completeness, evidence coverage, conflict provenance, native-adapter execution, or release readiness.

Until all conditions hold, retain the classification:

> **Provisional release candidate — core safety guarantees closed; retrieval-ledger and native release evidence pending.**
