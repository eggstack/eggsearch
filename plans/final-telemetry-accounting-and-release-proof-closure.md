# Final Telemetry Accounting and Release-Proof Closure Plan

**Repository:** `eggstack/eggsearch`  
**Baseline:** `e2244ac7097612e5d7dc152c75583d593a158ac3`  
**Status:** Implementation handoff  
**Scope:** Narrow final closure  
**Primary consumer:** codegg and other agent hosts that depend on truthful retrieval telemetry  
**Target classification:** release candidate after all gates pass

---

## 1. Purpose

The provider-attribution and release-evidence corrective pass landed the major architectural changes correctly. Eggsearch now has:

- typed role-capability partitioning;
- provider-scoped native advisory outcomes;
- request-scoped advisory routing;
- separate uncertain and confirmed truncation signals;
- fail-closed native forge workflows;
- additive evidence metadata suitable for codegg.

The remaining work is small but release-material because it affects the truthfulness of the retrieval ledger and the validity of release evidence.

This plan closes exactly four residual areas:

1. duplicate package-advisory dependency-role attempt records;
2. an advisory operation cap that counts identifiers instead of actual provider operations;
3. summary fields named as job counts but calculated from role dimensions;
4. success and genuine non-applicability sharing the same legacy absence sentinel, followed by a fresh immutable `R`/`E` verification cycle.

This is a closure pass, not another architecture phase. Do not add providers, tools, ranking behavior, broad schema redesign, or unrelated performance work.

---

## 2. Completion Standard

This line of work is complete only when all of the following are true:

- one logical provider advisory operation produces one advisory-role attempt and, when required, one independently justified dependency-role attempt;
- no provider/operation/role tuple is duplicated in the retrieval ledger;
- native advisory resource caps bound actual provider calls, not only input identifier groups;
- cap warnings and telemetry state exactly what was bounded and how many provider operations were attempted or skipped;
- `attempted_job_count`, `completed_job_count`, and `failed_job_count` are derived from retrieval attempts rather than expanded role dimensions;
- role-dimension counts remain separately available and correctly named;
- a consumer can distinguish success, absence, failure, skip, partial result, and true non-applicability without interpreting `absence_kind` heuristically;
- all response changes are additive and preserve codegg compatibility;
- all deterministic tests pass on the final code-bearing release subject `R`;
- Linux and macOS CI pass for the exact `R` SHA;
- affected-path benchmarks are captured against `R`;
- all four native forge jobs execute without skipping and produce validated evidence for `R`;
- an evidence-only commit `E` records exact run IDs, artifact identities, hashes, and final classification;
- `R..E` contains no runtime-code, test, workflow, dependency, or build-system change.

Until every item above is complete, the repository remains a **provisional release candidate**.

---

## 3. Explicit Non-Goals

Do not expand this pass into any of the following:

- new search providers or advisory sources;
- new MCP tools or public request parameters;
- ranking, grouping, reranking, or diversity changes;
- source-card identity redesign;
- broad replacement of `EvidenceAbsenceKind` across the repository;
- removal of existing public fields;
- changes to Git process termination, forge aggregate budgeting, safe-open containment, or local inventory architecture;
- Windows enablement;
- unrelated test-flake cleanup;
- general performance optimization;
- release publication before evidence commit `E` exists.

Refactoring is permitted only where needed to establish one authoritative accounting model and make it directly testable.

---

## 4. Current Residual Defects

### 4.1 Duplicate dependency-role record in package advisory outcomes

`record_package_outcomes()` currently handles `ProviderAdvisoryStatus::CapabilityUnavailable` by creating a first attempt whose role set contains both:

- `EvidenceRole::AuthoritativeSecurityAdvisory`;
- `EvidenceRole::ManifestOrDependencyMetadata`.

The function then unconditionally creates a second dependency-metadata attempt for the same provider and operation. This causes the dependency role to appear twice for one provider-scoped package query.

Consequences include:

- duplicate retrieval dimensions;
- inflated capability-skip counts;
- inflated role-attempt counts;
- duplicate `RetrievalFailure` records after role expansion;
- unstable codegg reasoning when it assumes one dimension per provider/operation/role tuple.

### 4.2 Native advisory cap does not bound provider calls

`MAX_NATIVE_ADVISORY_OPERATIONS` is incremented once per unique advisory identifier. Each identifier then fans out over every selected advisory provider.

For example:

- identifier cap: 32;
- selected providers: 4;
- possible provider calls: 128, plus a package query fan-out.

The current warning says at most 32 native advisory operations were attempted, which is false under provider fan-out.

The path remains deadline-bounded, but the named cap and telemetry do not represent the resource being consumed.

### 4.3 Job counters are calculated from dimensions

`build_attempt_derived_summary()` expands one `RetrievalAttempt` into one `RetrievalDimensionStatus` for each intended role. `summarize_retrieval()` then calculates:

- `attempted_job_count` from `dimensions.len()`;
- `completed_job_count` from successful dimensions;
- `failed_job_count` from failure dimensions.

A single two-role attempt therefore counts as two jobs. The fields are semantically inaccurate.

### 4.4 Success and non-applicability share `NotApplicable`

`SuccessWithResults` currently maps to `EvidenceAbsenceKind::NotApplicable`, as does `RetrievalAttemptOutcome::NotApplicable`.

The full dimension contains `attempt_outcome`, so informed consumers can disambiguate the two. However, consumers inspecting only `absence_kind` cannot distinguish:

- retrieval succeeded and evidence was found;
- the operation genuinely did not apply.

This should be corrected additively, without removing or repurposing existing public fields in a breaking way.

### 4.5 Release subject is stale

The provisional verification document names `97ebae60df6f7b367f9152b32c848a9af0ed8721` as `R`, but later commits changed test fixtures and timeout configuration. The eventual release subject must include this closure work and all subsequent deterministic fixes.

The old subject may remain documented as historical provisional evidence, but it cannot be the final release subject.

---

# Gate A — Canonical Provider/Operation/Role Attempt Identity

## A.1 Define the ledger invariant

The canonical invariant is:

> For a given request, there may be at most one terminal retrieval attempt for each distinct `(provider_id, operation identity, evidence role)` tuple.

A multi-role provider call is allowed to materialize multiple role dimensions, but each role dimension must originate from one and only one terminal attempt-role membership.

The implementation must distinguish:

- one physical provider call that serves the advisory role;
- a separate synthetic dependency-metadata outcome when the native advisory provider does not supply manifest/dependency evidence;
- provider capability absence before dispatch;
- operation failure after dispatch;
- request-level non-applicability.

Do not infer uniqueness from messages or vector position.

## A.2 Correct `record_package_outcomes()`

Refactor the function so the primary provider-scoped package advisory attempt always carries only:

```rust
vec![EvidenceRole::AuthoritativeSecurityAdvisory]
```

This applies to all provider statuses:

- `CapabilityUnavailable`;
- `InterruptedByDeadline`;
- `Completed(Ok(results))`;
- `Completed(Err(error))`.

Then emit exactly one dependency-metadata attempt for the same provider operation when dependency metadata was requested as a distinct evidence dimension.

The dependency attempt must use:

```rust
vec![EvidenceRole::ManifestOrDependencyMetadata]
```

Expected dependency outcome mapping:

| Primary provider status | Dependency attempt outcome | Reason |
|---|---|---|
| advisory capability unavailable | `SkippedCapabilityUnavailable` | provider cannot perform package advisory operation and supplies no dependency metadata |
| advisory interrupted by deadline | `InterruptedByDeadline` | operation could not complete before deadline |
| advisory success with results | `SkippedCapabilityUnavailable` | native advisory response is advisory evidence, not manifest evidence |
| advisory success with zero results | `SkippedCapabilityUnavailable` | zero advisories is not dependency metadata |
| advisory provider failure | `SkippedCapabilityUnavailable` or `Failed` only if a real dependency operation was dispatched | do not claim a dependency call failed when no dependency call existed |

For the current architecture, no separate native manifest operation is dispatched. Therefore the default dependency outcome after an advisory provider operation should remain `SkippedCapabilityUnavailable`, except when the global deadline prevented even classification/execution and the current contract intentionally reports the dependency dimension as deadline-interrupted.

Choose one deterministic deadline policy and document it. Preferred policy:

- if the advisory provider was selected and the global deadline prevented the provider operation, both role dimensions are `InterruptedByDeadline` because neither evidence dimension could be resolved;
- otherwise dependency metadata is `SkippedCapabilityUnavailable`.

## A.3 Add an internal operation discriminator

The plan previously permitted role-set identity in internal keys. This pass should make operation identity explicit enough to prevent future collisions.

Preferred additive internal type:

```rust
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum RetrievalOperationIdentity {
    SearchSubquery {
        subquery_id: String,
    },
    AdvisoryLookupById {
        vulnerability_id_fingerprint: String,
    },
    AdvisoryQueryByPackage {
        ecosystem: String,
        package_fingerprint: String,
        version_fingerprint: Option<String>,
    },
    KevLookup {
        cve_id_fingerprint: String,
    },
}
```

A smaller equivalent representation is acceptable. Requirements:

- no raw proprietary query text;
- deterministic for the request;
- provider-independent;
- sufficient to distinguish multiple package or identifier operations sharing a subquery label;
- available to internal deduplication and tests;
- public serialization is optional in this pass.

If no new field is added to `RetrievalAttempt`, introduce an internal ledger key constructed at emission time.

## A.4 Add a terminal-attempt ledger validator

Add a pure validation helper used by tests and optionally by debug assertions:

```rust
fn validate_attempt_ledger(attempts: &[RetrievalAttempt]) -> Result<(), AttemptLedgerViolation>
```

It must detect:

- duplicate provider/operation/role tuples;
- empty provider IDs;
- attempts with duplicate roles inside one role vector;
- `result_count > 0` paired with a failure or skip outcome;
- `SuccessZeroResults` with nonzero result count;
- `SuccessWithResults` with zero result count;
- deadline outcome without `deadline_interrupted = true`;
- confirmed truncation without a success/partial-success outcome;
- capability skip with an empty role set.

Do not make production responses fail because of a debug-only invariant until all call sites are covered. The final state should at minimum enforce the validator in unit/integration tests and use `debug_assert!` or structured logging in production construction paths.

## A.5 Required tests

Add focused tests covering:

1. capability-unavailable package provider emits exactly two attempts, one per role;
2. success-with-results package provider emits exactly two attempts, one advisory success and one dependency capability skip;
3. zero-result package provider emits exactly two attempts;
4. failed package provider emits exactly two attempts with no fabricated dependency provider failure;
5. deadline interruption emits no duplicate role tuple;
6. two providers produce four unique provider/operation/role tuples;
7. duplicate advisory metadata across providers does not collapse attempts;
8. attempt-to-failure expansion produces one failure per unique role tuple;
9. summary dimensions contain no duplicate provider/subquery/role/outcome record for the same operation;
10. property test over provider status combinations preserves ledger uniqueness.

### Gate A acceptance criteria

- [ ] `record_package_outcomes()` never creates one attempt containing both advisory and dependency roles.
- [ ] Exactly one advisory-role attempt exists per provider package operation.
- [ ] Exactly one dependency-role attempt exists per provider package operation when that dimension is modeled.
- [ ] No provider/operation/role tuple appears twice.
- [ ] Dependency evidence is never reported as a provider failure unless a real dependency operation was dispatched and failed.
- [ ] Ledger validation tests cover every terminal outcome.
- [ ] codegg fixtures prove no duplicate dimensions or failures.

---

# Gate B — Truthful Native Advisory Provider-Operation Budgets

## B.1 Separate identifier and provider-operation budgets

Replace the ambiguous single constant with two explicit bounded resources:

```rust
const MAX_NATIVE_ADVISORY_IDENTIFIERS: usize = 32;
const MAX_NATIVE_ADVISORY_PROVIDER_OPERATIONS: usize = 64;
```

Exact values may be adjusted after affected-path benchmarks, but both limits must exist and their semantics must be documented.

Definitions:

- **identifier budget:** maximum unique advisory identifiers accepted for native lookup;
- **provider-operation budget:** maximum selected-provider calls across identifier and package advisory operations.

The provider-operation budget is the release-material bound.

## B.2 Reserve operations before dispatch

Do not increment the provider-operation count after a fan-out is already started.

Before each adapter call:

1. determine the selected advisory providers for that operation;
2. determine which selected providers advertise the required capability;
3. calculate the number of provider operations that would be dispatched;
4. reserve only the remaining budget;
5. pass the allowed provider subset to the scoped adapter call;
6. emit explicit attempts for providers excluded by the operation budget.

Recommended helper:

```rust
struct NativeOperationBudget {
    max_identifiers: usize,
    max_provider_operations: usize,
    identifiers_seen: usize,
    provider_operations_reserved: usize,
}

impl NativeOperationBudget {
    fn reserve_identifier(&mut self) -> bool;
    fn reserve_providers(
        &mut self,
        provider_ids: &[String],
    ) -> ProviderReservation;
}
```

`ProviderReservation` should preserve input ordering and return:

- allowed providers;
- budget-skipped providers;
- remaining provider-operation capacity.

## B.3 Model budget exhaustion honestly

Do not use `SkippedCapabilityUnavailable` for budget exhaustion. The provider may have the capability.

Preferred outcome:

- `SkippedByPolicy`, with an error/reason class such as `native_operation_budget_exhausted`.

This treats the configured hard bound as execution policy.

If a dedicated future outcome is desired, it must be additive. Do not introduce a breaking enum rename in this closure pass.

For every provider excluded by the provider-operation cap, emit a terminal attempt with:

- real provider ID;
- real subquery/operation identity;
- intended role set;
- `SkippedByPolicy`;
- zero result count;
- bounded reason class;
- no raw query text.

## B.4 Preserve deadline semantics

The budget and deadline are independent:

- provider excluded before dispatch because the operation budget is exhausted: `SkippedByPolicy`;
- provider reserved and dispatched but global deadline expires: `InterruptedByDeadline`;
- provider returns engine timeout: `TimedOut`;
- identifier omitted because identifier cap is exhausted: produce a request-level warning and, when a concrete provider plan existed, policy-skip attempts for the omitted provider operations.

Avoid counting an operation both as budget-skipped and deadline-interrupted.

## B.5 Correct warnings and counters

Replace the current ambiguous warning with separate warnings as applicable:

```text
native_advisory_identifier_cap_reached:
processed <n> unique identifiers; <m> additional identifiers were not scheduled
```

```text
native_advisory_provider_operation_cap_reached:
executed or reserved <n> provider operations; <m> provider operations were skipped by policy
```

Warnings must not claim that an operation was attempted when it was only planned or skipped.

Add internal telemetry or summary fields as additive metadata:

```rust
pub struct NativeAdvisoryBudgetSummary {
    pub identifiers_planned: usize,
    pub identifiers_scheduled: usize,
    pub provider_operations_planned: usize,
    pub provider_operations_dispatched: usize,
    pub provider_operations_skipped_by_budget: usize,
}
```

Public exposure is optional if the retrieval attempts and structured warnings already provide the necessary agent-facing truth. Tests must still verify these counts internally.

## B.6 Bound package advisory fan-out

Package advisory queries consume one provider operation per selected provider. They must use the same provider-operation budget as identifier lookups.

Required ordering policy:

1. explicit advisory identifiers in stable input order;
2. identifier provider order in request-selected provider order;
3. package query after explicit identifiers unless the request contains only a package coordinate;
4. KEV lookup remains a separate bounded subsystem and is not charged to the advisory-provider budget unless explicitly unified and documented.

Do not allow package advisory fan-out to bypass a provider-operation budget exhausted by identifier lookups.

## B.7 Required tests

Add deterministic tests for:

1. one identifier and one provider;
2. one identifier and four providers;
3. two identifiers and four providers;
4. provider-operation budget smaller than provider fan-out;
5. provider-operation budget exhausted between identifiers;
6. package query consumes remaining budget;
7. package query skipped when no operation budget remains;
8. incapable providers do not consume dispatched-operation budget;
9. budget-skipped providers produce `SkippedByPolicy` attempts;
10. deadline interruption after reservation is not mislabeled as budget skip;
11. duplicate identifiers consume one identifier slot but each actual provider call consumes an operation slot;
12. unknown/unselected providers never receive attempts;
13. zero selected capable providers produces capability skips, not budget skips;
14. warning counts match ledger counts exactly;
15. property test proving dispatched provider operations never exceed the configured cap.

## B.8 Performance and memory checks

Extend affected-path benchmarks to cover:

- 32 unique identifiers × 1 provider;
- 32 unique identifiers × 4 providers with provider cap 64;
- provider cap exhaustion early in the identifier list;
- mixed identifier and package query workload;
- attempt-ledger construction at the maximum allowed operation count.

Record:

- elapsed time;
- number of provider futures created;
- number of attempts emitted;
- retained response/ledger bytes where practical;
- evidence that no vector grows beyond a function of the configured caps.

### Gate B acceptance criteria

- [ ] Identifier and provider-operation limits are separate and explicitly named.
- [ ] Actual provider dispatch count never exceeds the configured provider-operation cap.
- [ ] Package advisory calls consume the same operation budget.
- [ ] Capability-unavailable providers are not charged as dispatched calls.
- [ ] Budget exclusions are represented as policy skips, not capability skips or failures.
- [ ] Warnings distinguish planned, dispatched, and skipped operations.
- [ ] Tests prove exact behavior at cap boundaries and fan-out cross-products.
- [ ] Benchmarks demonstrate bounded attempt and future cardinality.

---

# Gate C — Separate Attempt Counts from Role-Dimension Counts

## C.1 Establish two accounting levels

Eggsearch has two legitimate counting levels:

1. **attempt level:** one terminal provider operation or synthetic terminal decision;
2. **dimension level:** one attempt expanded across one intended evidence role.

Do not use one level to populate fields named for the other.

## C.2 Calculate job/attempt counters before role expansion

Refactor summary generation so attempt-level counters are calculated directly from `&[RetrievalAttempt]`.

Recommended internal accumulator:

```rust
#[derive(Default)]
struct AttemptSummaryCounts {
    attempted: usize,
    completed: usize,
    failed: usize,
    zero_result: usize,
    timed_out: usize,
    rate_limited: usize,
    policy_skipped: usize,
    capability_skipped: usize,
    deadline_interrupted: usize,
    confirmed_truncated: usize,
    limit_reached_unknown: usize,
}
```

Terminal classification:

| Attempt outcome | attempted | completed | failed |
|---|---:|---:|---:|
| `SuccessWithResults` | 1 | 1 | 0 |
| `SuccessZeroResults` | 1 | 1 | 0 |
| `Failed` | 1 | 0 | 1 |
| `TimedOut` | 1 | 0 | 1 |
| `RateLimited` | 1 | 0 | 1 |
| `InterruptedByDeadline` | 1 | 0 | 1 |
| `SkippedByPolicy` | 1 | 0 | 0 |
| `SkippedCapabilityUnavailable` | 1 | 0 | 0 |
| `NotApplicable` | 1 | 1 or excluded from attempted totals, choose and document | 0 |
| `TruncatedAfterPartialSuccess` | 1 | 1 partial | 0 |

Preferred `NotApplicable` policy:

- count it as a terminal planned attempt in `attempted_job_count`;
- count it in `completed_job_count` because no further execution is required;
- expose a separate `not_applicable_count` field.

This keeps `attempted = completed + failed + skipped` easier to reason about when skipped categories are separately counted.

## C.3 Preserve public fields with corrected semantics

Keep existing public fields:

- `attempted_job_count`;
- `completed_job_count`;
- `failed_job_count`.

Correct their implementation to use attempt-level counts.

Add explicitly named dimension fields:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub attempted_dimension_count: Option<usize>,

#[serde(default, skip_serializing_if = "Option::is_none")]
pub completed_dimension_count: Option<usize>,

#[serde(default, skip_serializing_if = "Option::is_none")]
pub failed_dimension_count: Option<usize>,

#[serde(default, skip_serializing_if = "Option::is_none")]
pub not_applicable_count: Option<usize>,
```

These fields are additive. Do not rename or remove existing fields.

## C.4 Define exact invariants

At attempt level:

```text
attempted_job_count == attempts.len()
```

At dimension level:

```text
attempted_dimension_count == dimensions.len()
```

For every response with attempts:

```text
attempted_dimension_count >= attempted_job_count
```

Equality holds only when every attempt has exactly one materialized role.

Define and test the partition:

```text
attempted_job_count
  == completed_job_count
   + failed_job_count
   + policy_skipped_count
   + capability_skipped_count
```

This equation assumes `NotApplicable` is included in completed and deadline/timeouts/rate-limits are included in failed. If another definition is selected, document the exact equation and enforce it in tests.

Avoid double-counting timed-out and rate-limited attempts as both subtype counts and additional failed jobs. Subtype counts are subsets of `failed_job_count`.

## C.5 Do not derive role completeness from `NotApplicable`

Current role-completion logic marks roles complete when `absence_kind == NotApplicable`, which combines successful and truly non-applicable dimensions.

Refactor role completion to use `attempt_outcome` or the additive dimension state introduced in Gate D.

Required behavior:

- `SuccessWithResults`: role retrieval completed with evidence;
- `SuccessZeroResults`: role retrieval completed but evidence absent;
- `NotApplicable`: role excluded from required completion accounting;
- capability/policy skip: role unresolved;
- provider failure/deadline: role indeterminate;
- uncertain limit reached: role completed with possible incompleteness;
- confirmed truncation: role partial, not complete for completeness-sensitive workflows.

## C.6 Required tests

Add direct summary tests for:

1. one one-role successful attempt;
2. one two-role successful attempt;
3. one two-role failed attempt;
4. one supported-role success plus one unsupported-role capability skip;
5. one package provider operation producing advisory and dependency attempts;
6. mixed success, zero-result, failure, timeout, policy skip, capability skip, and non-applicable attempts;
7. confirmed truncation;
8. limit-reached-unknown;
9. no attempts fallback path;
10. serialization compatibility when new fields are absent;
11. codegg fixture verifying old fields remain parseable;
12. property test for count partition invariants.

### Gate C acceptance criteria

- [ ] Existing job-count fields are calculated from attempts, not dimensions.
- [ ] New dimension-count fields are additive and correctly named.
- [ ] Multi-role attempts no longer inflate job counts.
- [ ] Failure subtype counts remain subsets of failed-job count.
- [ ] Count partition invariants are enforced by tests.
- [ ] Role completeness no longer depends solely on `absence_kind == NotApplicable`.
- [ ] codegg fixtures parse both legacy and enriched summaries.

---

# Gate D — Additive Dimension State and Final Release Evidence

## D.1 Add an explicit dimension state

Preserve `EvidenceAbsenceKind` for compatibility, but stop requiring consumers to infer success from an absence enum.

Add an additive field:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalDimensionState {
    Satisfied,
    CompletedNoMatch,
    Failed,
    SkippedByPolicy,
    CapabilityUnavailable,
    Interrupted,
    Partial,
    NotApplicable,
}
```

Add it to `RetrievalDimensionStatus`:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub state: Option<RetrievalDimensionState>,
```

The field must be optional for backward-compatible deserialization.

## D.2 Define authoritative state mapping

Map attempt outcomes as follows:

| Attempt outcome / truncation evidence | Dimension state |
|---|---|
| `SuccessWithResults`, no confirmed truncation | `Satisfied` |
| `SuccessZeroResults` | `CompletedNoMatch` |
| `Failed`, `TimedOut`, `RateLimited` | `Failed` |
| `SkippedByPolicy` | `SkippedByPolicy` |
| `SkippedCapabilityUnavailable` | `CapabilityUnavailable` |
| `InterruptedByDeadline` | `Interrupted` |
| confirmed truncation | `Partial` |
| `TruncatedAfterPartialSuccess` | `Partial` |
| `NotApplicable` | `NotApplicable` |
| limit reached, additional results unknown | `Satisfied` plus `truncation_evidence = LimitReachedUnknown`, or `Partial` only if policy explicitly treats uncertainty as partial; choose once and document |

Preferred uncertain-limit policy:

- state remains `Satisfied` because the provider call completed and returned evidence;
- `truncation_evidence = LimitReachedUnknown` signals possible incompleteness;
- `has_truncation` remains false because truncation is unconfirmed.

## D.3 Keep legacy absence mapping stable

Do not break existing clients by changing all historical `absence_kind` values in this pass.

For newly constructed dimensions:

- retain current mappings where needed for compatibility;
- document that `state` is authoritative for terminal status;
- document that `absence_kind` describes absence/failure context and is not a complete success-state enum;
- add a deprecation note for consumers that use `absence_kind == NotApplicable` as a success test.

Do not remove `attempt_outcome`; it remains useful detailed provenance.

## D.4 Update codegg contract fixtures

Update `docs/architecture/codegg-contract.md` and `tests/codegg_evidence_contract.rs` with fixtures demonstrating:

1. successful evidence:
   - `state = satisfied`;
   - `attempt_outcome = success_with_results`;
2. true non-applicability:
   - `state = not_applicable`;
   - `attempt_outcome = not_applicable`;
3. capability skip:
   - `state = capability_unavailable`;
4. provider failure:
   - `state = failed`;
5. uncertain limit reached:
   - `state = satisfied`;
   - `truncation_evidence = limit_reached_unknown`;
6. confirmed truncation:
   - `state = partial`;
   - confirmed truncation evidence.

The contract must state that agent hosts should prefer:

1. `state` for coarse terminal interpretation;
2. `attempt_outcome` for exact operation outcome;
3. `absence_kind` for absence/failure reason compatibility;
4. `truncation_evidence` for completeness qualification.

## D.5 Static and schema guards

Add tests that ensure:

- `RetrievalDimensionState` remains snake_case;
- the `state` field is optional/additive;
- existing JSON fixtures without `state` still deserialize;
- new JSON fixtures serialize `state` deterministically;
- public schema includes all state variants;
- no production summary logic treats every `NotApplicable` absence kind as success without checking state/outcome;
- release documentation does not claim final release evidence while any required field is pending.

## D.6 Establish a fresh immutable release subject `R`

After Gates A–D implementation and all deterministic fixes:

1. ensure the working tree is clean;
2. run the full local deterministic gate;
3. commit all runtime, test, workflow, benchmark, schema, and documentation changes;
4. record the resulting full 40-character SHA as the new release subject `R`;
5. do not amend, rebase, or add code-bearing commits after `R` is selected;
6. any subsequent code/test/workflow correction invalidates `R` and requires a new subject.

The previous provisional subject remains historical only.

## D.7 Deterministic local gate on `R`

Run at minimum:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --no-default-features
cargo test --locked --features mock
cargo test --locked --all-features
cargo test --locked --doc --all-features
cargo bench --locked --all-features --bench perf --no-run
cargo build --locked --release --all-features
cargo publish --locked --dry-run
```

Use the repository's canonical `make check` if it includes every required command. The release record must list the exact commands actually run, toolchain version, operating system, architecture, date, and result.

A command not executed must not be marked passed.

## D.8 Linux and macOS CI on exact `R`

Required CI evidence:

- Linux test matrix on `R`;
- macOS test matrix on `R`;
- formatting;
- clippy;
- docs/schema/static guards;
- benchmark compilation;
- release build or equivalent package gate.

The current connector may not expose every workflow run. The implementation handoff must nevertheless record canonical GitHub run URLs/IDs in the evidence record.

No later fixture-only commit may be treated as harmless to `R`. Test changes create a new code-bearing subject because they alter the verified release contract.

## D.9 Capture affected-path benchmark evidence

Run the relevant benchmarks on `R` and retain machine-readable or text artifacts covering:

- role capability partitioning;
- package advisory attempt construction;
- maximum provider-operation budget;
- mixed retrieval summary construction;
- maximum dimension expansion;
- near-cap local inventory paths already included in the release suite;
- forge budget paths already included in the release suite.

The release record must distinguish:

- benchmark compiled;
- benchmark executed;
- benchmark artifact captured.

Do not claim memory stability unless an actual retained-memory measurement was performed. Bounded cardinality evidence is acceptable when described accurately.

## D.10 Run non-skipping native forge evidence on `R`

Manually dispatch `.github/workflows/native-forge-smoke.yml` with:

```text
release_subject = <full R SHA>
```

Required jobs:

- GitHub;
- GitLab;
- Codeberg/Forgejo;
- distinct configured Gitea instance;
- summary/evidence-manifest job.

Required preconditions:

- all provider credentials present;
- GitHub slash-containing ref explicitly provisioned and configured;
- Gitea HTTPS instance configured;
- each job checks out exact `R`;
- no job returns success through a missing-token or missing-fixture branch;
- each provider emits at least one validated JSON evidence file;
- each provider artifact includes bounded logs;
- summary job requires exact pass from every provider;
- combined manifest contains hashes for every provider evidence file.

## D.11 Create evidence-only commit `E`

After all required CI, benchmark, and native-smoke evidence passes:

1. update `docs/release-verification.md` with exact evidence;
2. add generated evidence manifests or stable references under an approved evidence path;
3. commit only release documentation and evidence material;
4. record the resulting full SHA as `E`;
5. verify the diff from `R` to `E`.

Allowed `R..E` paths should be limited to an explicit allowlist such as:

```text
docs/release-verification.md
docs/release-checklist.md
evidence/**
```

Forbidden changes between `R` and `E`:

- `src/**`;
- `tests/**`;
- `benches/**`;
- `.github/workflows/**`;
- `Cargo.toml`;
- `Cargo.lock`;
- `Makefile`;
- schemas or generated public API files, unless the evidence protocol explicitly stores immutable generated evidence under a separate approved path.

Add a release-document contract test or verification script that checks the allowlist.

## D.12 Final release record contents

The final record must include:

- classification: `release candidate`;
- full `R` SHA;
- full `E` SHA;
- local deterministic gate date, host OS, architecture, Rust version, and commands;
- Linux CI run ID and result;
- macOS CI run ID and result;
- benchmark run ID/artifact and scope;
- native forge workflow run ID;
- each provider job ID/result;
- each provider artifact ID/name;
- combined manifest artifact ID/name;
- SHA-256 hashes for retained evidence files;
- explicit statement that `R..E` contains only allowlisted evidence/documentation paths;
- any skipped optional check clearly labeled as skipped and excluded from release claims.

### Gate D acceptance criteria

- [ ] `RetrievalDimensionState` is additive and authoritative for coarse terminal state.
- [ ] Success and true non-applicability serialize to distinct states.
- [ ] Legacy fixtures without `state` remain compatible.
- [ ] codegg contract fixtures document interpretation order.
- [ ] Final code-bearing subject `R` includes all closure corrections.
- [ ] Linux and macOS CI pass on exact `R`.
- [ ] Affected-path benchmarks execute and produce retained evidence for `R`.
- [ ] All four native forge jobs execute and pass against exact `R`.
- [ ] Evidence-only commit `E` records exact run IDs, artifacts, and hashes.
- [ ] `R..E` changes only allowlisted evidence/documentation paths.
- [ ] Release documentation contains no pending value presented as completed evidence.

---

## 5. Cross-Cutting Implementation Requirements

### 5.1 Deterministic ordering

All attempt and dimension output must be deterministic regardless of provider completion order.

Recommended ordering key:

```text
operation order
provider order
subquery/operation identity
evidence role label
terminal outcome
```

Do not sort by duration, completion time, or provider response arrival.

### 5.2 Sensitive-data handling

No new identity or telemetry field may contain:

- raw query text;
- tokens;
- dependency-file content;
- private repository paths;
- proprietary package coordinates in unhashed diagnostic IDs unless those values are already public response fields by contract.

Continue using bounded non-recoverable fingerprints where diagnostic identity is required.

### 5.3 Additive public compatibility

Public response/schema changes must be additive:

- optional fields with serde defaults;
- no enum variant rename;
- no field removal;
- no semantic repurposing of existing serialized strings without compatibility tests;
- old JSON fixtures must still deserialize;
- codegg must not require an atomic upgrade to continue parsing responses.

### 5.4 One authoritative summary path

There should be one authoritative attempt-derived summary implementation. Avoid parallel calculations in:

- evidence postprocessing;
- security orchestration;
- workflow coverage;
- codegg fixture helpers.

Fallback summary construction for legacy no-attempt paths may remain, but it must be clearly isolated and tested.

### 5.5 No false evidence

Tests, CI, docs, and manifests must distinguish:

- configured but not executed;
- executed and failed;
- executed and passed;
- skipped;
- artifact missing;
- result unknown.

Only executed-and-passed work may support a release claim.

---

## 6. Likely Files to Inspect or Modify

Primary implementation paths:

```text
src/meta/security_search.rs
src/meta/adapter.rs
src/meta/dispatch.rs
src/core/retrieval_status.rs
src/core/evidence_postprocess.rs
src/core/workflow_coverage.rs
src/meta/engines/mod.rs
```

Primary test paths:

```text
tests/native_security_attempts.rs
tests/retrieval_attempt_ledger.rs
tests/property_retrieval.rs
tests/codegg_evidence_contract.rs
tests/evidence_integration.rs
tests/static_guards.rs
tests/release_document_contract.rs
```

Release and benchmark paths:

```text
benches/perf.rs
.github/workflows/ci.yml
.github/workflows/native-forge-smoke.yml
docs/architecture/codegg-contract.md
docs/architecture/meta.md
docs/architecture/testing.md
docs/release.md
docs/release-checklist.md
docs/release-verification.md
```

This list is guidance, not permission for unrelated edits.

---

## 7. Required Test Matrix

### 7.1 Unit tests

- provider/operation/role ledger uniqueness;
- package outcome mapping for every provider status;
- identifier budget accounting;
- provider-operation reservation;
- capability providers excluded from dispatch count;
- exact attempt-level summary counts;
- exact dimension-level summary counts;
- dimension state mapping;
- legacy absence compatibility;
- deterministic ordering;
- serialization round trips.

### 7.2 Integration tests

- security search with one advisory provider;
- security search with multiple advisory providers;
- explicit provider routing excludes unselected advisory providers;
- provider-operation budget truncates fan-out deterministically;
- mixed advisory and package lookup;
- duplicate identifier normalization;
- mixed success/failure/capability outcomes;
- codegg evidence response fixture;
- release document remains provisional before evidence completion.

### 7.3 Property tests

Properties to enforce:

```text
unique(provider, operation, role) for all terminal dimensions
```

```text
dispatched_provider_operations <= configured_provider_operation_cap
```

```text
attempted_job_count == attempts.len()
```

```text
attempted_dimension_count == dimensions.len()
```

```text
attempted_dimension_count >= attempted_job_count
```

```text
no raw query appears in attempt identity or summary diagnostics
```

```text
state mapping is total for every RetrievalAttemptOutcome × TruncationEvidence combination
```

### 7.4 Static guards

Guard against regression patterns such as:

- package advisory attempts containing both advisory and dependency roles;
- operation count incremented only once before multi-provider fan-out;
- assigning job count from `dimensions.len()`;
- using `absence_kind == NotApplicable` as the sole success predicate;
- native forge tests returning success on missing credentials;
- final release classification with pending required evidence;
- evidence commit modifying runtime paths.

Static source-string guards are acceptable only as a supplement to behavioral tests.

---

## 8. Recommended Commit Sequence

Keep commits small enough for review and rollback.

### Commit 1 — Package attempt ledger de-duplication

Suggested message:

```text
fix: deduplicate package advisory role attempts
```

Contents:

- canonical advisory/dependency attempt split;
- internal operation identity or ledger key;
- ledger validator;
- focused tests.

### Commit 2 — Provider-operation budget accounting

Suggested message:

```text
fix: bound native advisory provider operations
```

Contents:

- separate identifier/provider-operation budgets;
- reservation helper;
- policy-skip attempts for budget exclusions;
- warnings and tests;
- benchmark cases.

### Commit 3 — Attempt and dimension summary separation

Suggested message:

```text
fix: separate retrieval attempt and dimension counts
```

Contents:

- attempt-level accumulator;
- corrected existing job fields;
- additive dimension fields;
- invariant tests.

### Commit 4 — Additive dimension state and codegg contract

Suggested message:

```text
feat: add explicit retrieval dimension state
```

Contents:

- optional `RetrievalDimensionState`;
- mapping logic;
- compatibility fixtures;
- codegg documentation;
- schema/static guards.

### Commit 5 — Deterministic verification corrections

Suggested message:

```text
test: close final retrieval accounting verification
```

Contents only if required:

- test corrections;
- CI gate inclusion;
- benchmark compilation/execution plumbing;
- release-document contract updates.

The resulting commit after this step becomes candidate `R` only after all deterministic gates pass.

### Commit 6 — Evidence-only commit `E`

Suggested message:

```text
docs(release): record final release evidence
```

Contents:

- exact release verification record;
- evidence manifests/references;
- hashes and run IDs;
- final classification.

No runtime, test, workflow, benchmark, or dependency changes are allowed here.

---

## 9. Rollback Boundaries

### Gate A rollback

Safe rollback unit:

- package outcome mapping and ledger validation.

Do not roll back provider-scoped adapter outcomes from the previous pass.

### Gate B rollback

Safe rollback unit:

- operation reservation and cap telemetry.

Preserve the existing global deadline even if budget code must be reverted.

### Gate C rollback

Safe rollback unit:

- additive dimension counters.

Corrected existing job-count semantics should not be reverted once consumers rely on truthful counts.

### Gate D rollback

The optional `state` field can be omitted from serialization if a severe compatibility defect is found, because it is additive. Do not change old enum strings as an emergency workaround.

Any code-bearing rollback after selecting `R` invalidates all evidence and requires a new `R`.

---

## 10. Reviewer Checklist

### Ledger correctness

- [ ] Does every provider package operation produce exactly one advisory attempt?
- [ ] Is dependency metadata represented exactly once and independently?
- [ ] Can a provider/operation/role tuple be duplicated through any status branch?
- [ ] Are fabricated failures avoided for operations never dispatched?

### Resource bounds

- [ ] Does the cap count provider calls rather than identifier groups?
- [ ] Are incapable providers excluded from dispatched-call count?
- [ ] Are package queries charged to the same budget?
- [ ] Are budget exclusions visible as policy skips?
- [ ] Can concurrency or retries exceed the reserved budget?

### Summary semantics

- [ ] Are job counts derived from attempts?
- [ ] Are dimension counts derived from expanded dimensions?
- [ ] Do multi-role attempts preserve one job and multiple dimensions?
- [ ] Are subtype failure counts subsets rather than additions?
- [ ] Is role completion based on authoritative outcome/state?

### Compatibility

- [ ] Do old JSON fixtures deserialize without the new state field?
- [ ] Are existing serialized field names and enum strings unchanged?
- [ ] Can codegg prefer the new state while continuing to support old responses?
- [ ] Are query fingerprints bounded and non-recoverable?

### Release proof

- [ ] Is `R` the final code-bearing commit?
- [ ] Did Linux and macOS CI run on exact `R`?
- [ ] Were benchmarks executed and retained for `R`?
- [ ] Did every native forge provider execute without skipping?
- [ ] Does each artifact identify `R` and a pinned provider result?
- [ ] Does `E` contain only allowlisted evidence/documentation changes?
- [ ] Are all claims backed by exact run IDs and artifacts?

---

## 11. Final Acceptance Matrix

| Area | Release-blocking criterion |
|---|---|
| Package ledger | no duplicate provider/operation/role tuples |
| Dependency role | one independently justified dimension per provider package operation |
| Identifier cap | unique input identifiers bounded and counted accurately |
| Provider-operation cap | actual provider calls never exceed configured maximum |
| Budget outcome | excluded capable providers reported as policy skips |
| Job counts | derived from terminal attempts |
| Dimension counts | derived from role-expanded dimensions |
| Success semantics | explicit `satisfied` state distinct from true non-applicability |
| Compatibility | existing fixtures deserialize and codegg contract remains additive |
| Determinism | stable attempt/dimension ordering independent of completion order |
| Privacy | no raw query or secret material in identities/telemetry |
| Tests | all feature combinations, docs, schemas, properties, and static guards pass |
| Linux CI | exact `R` passes |
| macOS CI | exact `R` passes |
| Benchmarks | affected-path suite executed and artifact retained for `R` |
| Native forge | GitHub, GitLab, Codeberg, and Gitea exact-pass evidence for `R` |
| Evidence commit | `E` records exact runs/artifacts/hashes and changes only allowlisted paths |
| Documentation | no pending item presented as completed evidence |

Any failed row keeps the repository at **provisional release candidate**.

---

## 12. Definition of Done

The work is done when:

1. package advisory attempt construction is duplicate-free;
2. native advisory provider calls are bounded by an explicit provider-operation budget;
3. every budget skip, capability skip, failure, timeout, and deadline interruption is truthfully distinguished;
4. job counters describe attempts and dimension counters describe role expansion;
5. response dimensions expose an additive authoritative terminal state;
6. codegg compatibility fixtures pass with both legacy and enriched responses;
7. all deterministic local and CI gates pass on immutable subject `R`;
8. affected-path benchmarks and all four native forge jobs produce evidence tied to `R`;
9. evidence-only commit `E` records exact run IDs, artifact identifiers, and hashes;
10. the `R..E` diff contains no code-bearing or verification-contract changes;
11. the release record changes classification from provisional to release candidate only after every required proof exists.

No additional feature work should be started inside this closure pass. Any new capability request belongs in a separate post-release plan.
