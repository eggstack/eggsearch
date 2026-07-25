# Final Retrieval-Ledger Corrective and Release-Proof Pass

**Repository:** `eggstack/eggsearch`  
**Baseline:** `d08ca05922391cd89358dfe7c79e21424bd8885f`  
**Status:** Small-model implementation handoff  
**Scope:** Narrow corrective closure only  
**Primary consumer:** codegg and other agent hosts consuming retrieval telemetry  
**Target after completion:** immutable release subject `R`, followed by evidence-only commit `E`

---

## 1. Objective

The broad telemetry-accounting implementation has landed. Do not redesign it.

This pass corrects the remaining defects that prevent the current implementation from satisfying its own release invariants:

1. `RetrievalOperationIdentity` exists but is not attached to attempts or used by ledger validation;
2. identifier and provider-operation budget edge cases produce incorrect counts or warnings;
3. role-completion and absence helpers still infer success from the legacy `absence_kind` field instead of the new authoritative `state` field;
4. `not_applicable_count` is documented as a dimension count but overwritten with an attempt count;
5. the codegg contract document was structurally damaged by an insertion in the middle of an existing table;
6. final release subject `R` and evidence-only commit `E` have not been produced for the actual final code.

This is the final corrective pass. It must not add capabilities, providers, tools, ranking behavior, or unrelated cleanup.

---

## 2. Small-Model Execution Rules

Follow these rules exactly:

1. Work in the gate order given below.
2. Do not combine gates until the focused tests for the current gate pass.
3. Do not rename existing serialized fields or enum variants.
4. Public response changes must be additive and optional.
5. Do not change provider routing, result ranking, grouping, or source-card semantics.
6. Do not alter native forge workflow behavior unless a release-proof test demonstrates a concrete defect.
7. Do not update the release classification to `release candidate` during implementation.
8. Do not select release subject `R` until all code, tests, documentation, formatting, clippy, and feature matrices pass.
9. Any code, test, workflow, benchmark, schema, or contract correction after choosing `R` creates a new `R`.
10. Evidence commit `E` may contain only approved release documentation and evidence references.

When a suggested type or helper conflicts with existing repository style, preserve the required semantics and use the nearest existing style. Do not broaden the design.

---

## 3. Current Defects to Preserve as Regression Tests

Before changing production code, add or confirm tests that expose these exact defects.

### Defect A — legitimate advisory operations collide

Two native advisory lookups can share:

```text
provider_id = "osv"
subquery_id = "advisory_by_cve"
evidence_role = AuthoritativeSecurityAdvisory
```

while representing distinct identifiers:

```text
CVE-2025-0001
CVE-2025-0002
```

The current validator keys uniqueness by provider, subquery, and role. It therefore cannot distinguish the two legitimate operations.

### Defect B — duplicate identifiers consume identifier budget

The current loop reserves identifier budget before checking whether the identifier was already seen. Repeated input values can consume the 32-identifier budget without producing additional provider calls.

### Defect C — provider-cap warning false positive

When zero selected providers support native advisory lookup:

- `reservation.allowed` is empty;
- `reservation.skipped_by_budget` is also empty;
- the current code can set `provider_op_cap_reached = true`.

This produces a cap warning even though no provider operation was skipped by the cap.

### Defect D — provider-cap warning false negative

When a reservation is partial:

- at least one provider is allowed;
- at least one provider is skipped by budget.

The current code may not set `provider_op_cap_reached` because `allowed` is not empty. A real budget skip can occur without a warning.

### Defect E — success/non-applicability helpers remain ambiguous

The new `RetrievalDimensionState` distinguishes `Satisfied` from `NotApplicable`, but helpers still use:

```rust
absence_kind == EvidenceAbsenceKind::NotApplicable
```

This can cause:

- successful evidence to be considered “absence only”;
- truly non-applicable roles to be counted as completed evidence roles;
- new state semantics to be ignored by downstream logic.

### Defect F — `not_applicable_count` changes level

`summarize_retrieval()` calculates `not_applicable_count` from dimensions. Then `summarize_retrieval_with_attempts()` overwrites it from `AttemptSummaryCounts`, changing the field to an attempt-level count.

A two-role non-applicable attempt should demonstrate the mismatch:

```text
attempt-level not applicable = 1
dimension-level not applicable = 2
```

### Defect G — codegg contract structure is invalid

The retrieval-state section was inserted before the existing Dirty State table finished. The `unknown` and `not_git` rows now appear after the new section, and section numbering duplicates `## 9`.

---

# Gate A — Attach Real Operation Identity to Retrieval Attempts

## A.1 Required outcome

Every attempt-derived ledger record must be uniquely identified by:

```text
provider ID
operation ID
evidence role
```

`subquery_id` is a human/machine-readable operation class. It is not sufficient as a unique operation instance ID.

## A.2 Add an optional serialized operation identifier

Add this field to `RetrievalAttempt`:

```rust
/// Deterministic bounded identifier for this operation instance.
///
/// This distinguishes multiple operations sharing one subquery label.
/// It must not contain raw query text, tokens, file contents, or secrets.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub operation_id: Option<String>,
```

Requirements:

- additive and optional;
- old JSON without the field still deserializes;
- new attempt-derived paths populate it;
- value is deterministic for the same logical operation;
- value is bounded;
- value does not expose raw query text;
- provider ID is not included in the operation ID because provider is already a separate ledger key component.

Do not serialize the debug representation of `RetrievalOperationIdentity` directly.

## A.3 Provide one canonical operation-ID encoder

Add a method on `RetrievalOperationIdentity` or a nearby pure helper:

```rust
impl RetrievalOperationIdentity {
    pub fn stable_id(&self) -> String;
}
```

Required formats may follow this pattern:

```text
search:<bounded-subquery-id>
advisory-id:<fingerprint>
advisory-package:<normalized-ecosystem>:<package-fingerprint>:<version-fingerprint-or-none>
kev:<cve-fingerprint>
```

Rules:

- use existing bounded FNV fingerprint helper for identifier/package/version values;
- normalize ecosystem to lowercase ASCII before encoding;
- bound human-readable subquery labels to the repository’s existing identifier length policy;
- do not include raw package name or raw vulnerability ID;
- do not include provider ID;
- do not include duration, result count, completion order, or random values.

Expected examples:

```text
RetrievalOperationIdentity::from_advisory_id("CVE-2025-0001")
    -> advisory-id:fp_<16 hex>

RetrievalOperationIdentity::from_advisory_id("CVE-2025-0002")
    -> a different advisory-id value
```

## A.4 Populate `operation_id` in all authoritative constructors

Update the helper constructors in `src/meta/security_search.rs` so operation identity is passed explicitly.

Preferred signatures:

```rust
fn native_advisory_attempt(
    provider_id: &str,
    subquery_id: &str,
    operation: &RetrievalOperationIdentity,
    intended_roles: Vec<EvidenceRole>,
    outcome: RetrievalAttemptOutcome,
    result_count: usize,
    error_class: Option<String>,
    query_text: &str,
    start: Instant,
) -> RetrievalAttempt
```

```rust
fn native_advisory_attempt_with_duration(
    provider_id: &str,
    subquery_id: &str,
    operation: &RetrievalOperationIdentity,
    intended_roles: Vec<EvidenceRole>,
    outcome: RetrievalAttemptOutcome,
    result_count: usize,
    error_class: Option<String>,
    query_text: &str,
    duration_ms: u64,
) -> RetrievalAttempt
```

Populate operation identities as follows:

| Path | Operation identity |
|---|---|
| generic dispatch subquery | `SearchSubquery { subquery_id }` |
| advisory lookup by ID | `AdvisoryLookupById { vulnerability fingerprint }` |
| package advisory query | `AdvisoryQueryByPackage { ecosystem, package fingerprint, version fingerprint }` |
| KEV lookup by CVE | `KevLookup { CVE fingerprint }` |
| KEV not applicable because no CVE exists | a stable request-level identity such as `search:kev-not-applicable`, or an explicit new internal variant if simpler |

For package outcomes, both the advisory-role attempt and dependency-role attempt must share the same `operation_id`. They represent separate roles for one provider operation.

For budget-skipped package attempts, use the same package operation identity that would have been dispatched.

For incapable-provider package attempts, use the same package operation identity that would have been dispatched.

## A.5 Populate generic dispatch attempts

Update generic dispatch attempt creation so every attempt has an operation ID derived from its exact subquery instance.

If subquery IDs are already unique per planned operation, use:

```rust
RetrievalOperationIdentity::from_search_subquery(&subquery_id).stable_id()
```

All terminal paths must carry the same operation ID for the same job:

- success;
- zero results;
- failure;
- timeout;
- rate limit;
- panic conversion;
- deadline interruption;
- capability skip;
- policy skip.

Do not derive the operation ID after task completion from result contents.

## A.6 Update every struct literal safely

Because `RetrievalAttempt` has many test and production struct literals, add:

```rust
operation_id: None,
```

only to legacy/manual fixtures that do not model a concrete operation identity.

New behavior tests must use real operation IDs.

Use compiler errors to find all struct literals. Do not hide missed call sites with an unrelated `Default` implementation.

## A.7 Make ledger validation use `operation_id`

Replace the validator uniqueness key:

```rust
(provider_id, subquery_id, role)
```

with:

```rust
(provider_id, operation_id, role)
```

Recommended key:

```rust
HashSet<(String, String, EvidenceRole)>
```

Operation-ID fallback policy for old attempts:

```rust
let operation_id = attempt
    .operation_id
    .clone()
    .or_else(|| attempt.subquery_id.as_ref().map(|s| format!("legacy-subquery:{s}")))
    .unwrap_or_else(|| "legacy-unknown".to_string());
```

The fallback exists only for backward-compatible manually constructed attempts. All authoritative production attempt paths should populate `operation_id`.

Update `AttemptLedgerViolation::DuplicateProviderOperationRole` to report:

```rust
provider_id
operation_id
role
```

It may retain `subquery_id` as optional diagnostic context, but operation identity must be present in the violation.

## A.8 Validate production ledgers without breaking responses

Immediately before postprocessing the assembled attempt vector, add a debug assertion or non-fatal structured warning:

```rust
debug_assert!(
    validate_attempt_ledger(&all_attempts).is_ok(),
    "assembled retrieval attempt ledger must satisfy invariants"
);
```

Use this in the central paths where complete attempt vectors are assembled.

Do not return an MCP error solely because the validator fails in release builds during this pass. Tests and debug builds should fail loudly.

## A.9 Deterministic ordering

Ensure operation identity participates in deterministic sorting when attempts from multiple native operations share provider and subquery class.

Recommended ordering:

```text
subquery/operation class order
operation_id
provider order
role label
outcome
```

Do not sort by duration or completion time.

## A.10 Gate A tests

Add tests with explicit operation IDs:

1. same provider + same subquery + same role + different advisory IDs is valid;
2. same provider + same operation ID + same role twice is rejected;
3. same operation ID + same provider + different roles is valid;
4. same operation ID + different providers + same role is valid;
5. two package role attempts share one operation ID and validate;
6. package advisory and dependency attempts are not duplicates;
7. two CVE IDs produce different operation IDs;
8. same CVE ID produces the same operation ID across runs;
9. operation ID contains no raw CVE ID;
10. package operation ID contains no raw package/version values;
11. legacy attempt without operation ID uses deterministic fallback;
12. serialization round-trip preserves optional operation ID;
13. old JSON without operation ID still deserializes;
14. assembled security attempt ledger validates for multiple IDs and providers;
15. generic dispatch terminal paths preserve the planned operation ID.

### Gate A acceptance criteria

- [ ] `RetrievalAttempt` has an additive optional `operation_id` field.
- [ ] All authoritative production attempts populate `operation_id`.
- [ ] Two distinct advisory IDs under the same provider/subquery/role validate successfully.
- [ ] Duplicate provider/operation/role tuples are rejected.
- [ ] Package advisory and dependency attempts share operation identity but remain distinct by role.
- [ ] Operation IDs are deterministic, bounded, and do not expose raw query/package/identifier text.
- [ ] Production assembly paths run ledger validation in debug builds or equivalent non-fatal diagnostics.
- [ ] Attempt ordering is deterministic across multiple operations.

Do not continue to Gate B until these criteria and tests pass.

---

# Gate B — Correct Identifier and Provider-Budget Accounting

## B.1 Required outcome

Budget telemetry must answer these questions truthfully:

1. How many unique identifiers were supplied?
2. How many unique identifiers were scheduled under the identifier cap?
3. How many capable provider operations were planned for scheduled operations?
4. How many provider operations were actually dispatched?
5. How many capable provider operations were skipped specifically because the provider-operation cap was exhausted?

A provider-cap warning must exist if and only if at least one capable provider operation was skipped by the provider-operation cap.

## B.2 Build a unique identifier plan before reserving budget

Do not reserve identifier budget inside the raw input loop.

Build one stable, deduplicated operation list first.

Recommended helper:

```rust
#[derive(Clone)]
struct PlannedAdvisoryIdentifier {
    value: String,
    subquery_id: &'static str,
    operation: RetrievalOperationIdentity,
}
```

Build in this order:

1. CVE IDs;
2. GHSA IDs;
3. OSV IDs;
4. RustSec IDs.

For each candidate:

- use one `HashSet<String>` to suppress duplicate raw normalized identifiers;
- preserve first-seen order in a `Vec`;
- create operation identity once;
- do not consume budget for duplicates.

Then set:

```rust
budget_summary.identifiers_planned = unique_identifier_plan.len();
```

Apply the identifier cap to the unique plan:

```rust
let scheduled_identifiers = unique_identifier_plan
    .into_iter()
    .take(MAX_NATIVE_ADVISORY_IDENTIFIERS)
    .collect::<Vec<_>>();
```

Set:

```rust
budget_summary.identifiers_scheduled = scheduled_identifiers.len();
```

Do not use the number of `reserve_identifier()` calls as a proxy for unique scheduling.

`reserve_identifier()` may be retained if tests or public internal API expect it, but orchestration must deduplicate before calling it.

## B.3 Do not break after the first provider-budget skip

Process every scheduled identifier even after the provider-operation budget is exhausted.

Reason:

- later provider calls will be policy-skipped, not dispatched;
- recording those policy skips makes the ledger complete for every scheduled operation;
- the identifier cap bounds this work;
- provider list cardinality is already bounded by configured engines.

For each scheduled operation:

1. emit capability-skip attempts for selected incapable providers;
2. reserve capable providers;
3. emit policy-skip attempts for `reservation.skipped_by_budget`;
4. dispatch only `reservation.allowed`;
5. continue to the next scheduled identifier even when allowed is empty.

Remove control flow that exits the identifier loop solely because provider budget was reached.

## B.4 Derive the provider-cap warning from actual skips

After every reservation:

```rust
let had_budget_skip = !reservation.skipped_by_budget.is_empty();
```

Accumulate:

```rust
budget_summary.provider_operations_skipped_by_budget +=
    reservation.skipped_by_budget.len();
```

At the end:

```rust
let provider_op_cap_reached =
    budget_summary.provider_operations_skipped_by_budget > 0;
```

Do not set this flag from:

```rust
reservation.allowed.is_empty()
```

An empty allowed set can mean:

- zero capable providers;
- all capable providers exhausted by budget;
- explicit empty routing.

Only a nonempty `skipped_by_budget` list proves budget exhaustion affected execution.

## B.5 Correct incapable-provider behavior

Incapable providers:

- do not consume provider-operation budget;
- receive `SkippedCapabilityUnavailable` attempts for each scheduled applicable operation;
- never contribute to `provider_operations_planned` if that field is defined as capable provider calls;
- never trigger provider-cap warnings.

Document `provider_operations_planned` precisely as:

> Number of capable provider operations planned for identifier/package operations that passed the identifier/request planning gates.

Do not count incapable providers in this field.

## B.6 Correct partial reservation behavior

Example:

```text
remaining provider budget = 1
capable providers = [osv, ghsa, vendor]
```

Expected:

```text
allowed = [osv]
skipped_by_budget = [ghsa, vendor]
provider_operations_planned += 3
provider_operations_dispatched += 1
provider_operations_skipped_by_budget += 2
provider cap warning = present
```

The allowed provider call must still execute.

## B.7 Correct zero-capable-provider behavior

Example:

```text
selected providers = [duckduckgo, startpage]
lookup-capable providers = []
```

Expected:

- capability-skip attempts for applicable native advisory role operations, if current contract models selected incapable providers per operation;
- zero planned capable provider operations;
- zero dispatched provider operations;
- zero budget-skipped provider operations;
- no provider-operation-cap warning;
- native advisory unavailable warning may remain.

## B.8 Package query uses the same corrected logic

For a package query:

1. create one package operation identity before reservation;
2. identify capable and incapable package providers;
3. reserve capable providers;
4. emit policy skips for budget-excluded capable providers;
5. emit capability skips for incapable providers;
6. dispatch allowed providers;
7. do not set cap warning merely because allowed is empty;
8. provider cap warning derives from accumulated skipped-by-budget count.

The package query must still be represented when identifier provider budget is exhausted. Its capable providers become policy-skipped, preserving complete ledger evidence.

## B.9 Identifier-cap warning

Emit the identifier-cap warning if and only if:

```rust
identifiers_planned > identifiers_scheduled
```

The warning must report unique counts:

```text
native_advisory_identifier_cap_reached:
processed <scheduled unique identifiers>; <planned - scheduled> additional unique identifiers were not scheduled
```

Do not claim raw duplicate identifiers were omitted by the cap.

No per-provider attempts are required for identifiers excluded by the identifier cap because those operation instances were not admitted into the scheduled plan. The warning is the truthful request-level record.

## B.10 Provider-cap warning

Emit if and only if:

```rust
provider_operations_skipped_by_budget > 0
```

Preferred wording:

```text
native_advisory_provider_operation_cap_reached:
dispatched <n> provider operations; <m> capable provider operations were skipped by policy after the provider-operation cap was reached
```

Do not say “executed or reserved.” Use the actual dispatched count.

## B.11 Gate B tests

Add tests for:

1. repeated identical CVE consumes one unique identifier slot;
2. same identifier repeated across input fields is deduplicated according to existing normalization policy;
3. 32 unique IDs are scheduled without identifier warning;
4. 33 unique IDs schedule 32 and warn about one;
5. duplicate IDs do not cause early cap exhaustion;
6. zero capable providers produces no provider-cap warning;
7. zero capable providers produces capability skips, not policy skips;
8. partial provider reservation produces a cap warning;
9. partial reservation dispatches allowed providers;
10. full provider exhaustion produces policy skips for all capable providers;
11. scheduled identifiers after exhaustion still receive policy-skip attempts;
12. package operation after exhaustion receives policy-skip attempts;
13. incapable providers do not consume operation budget;
14. `provider_operations_planned == dispatched + skipped_by_budget` for capable operations;
15. dispatched provider operations never exceed 64;
16. provider-cap warning count matches policy-skip attempts with `native_operation_budget_exhausted`;
17. no warning occurs when skipped-by-budget count is zero;
18. deterministic provider order is preserved in allowed and skipped lists;
19. mixed capable/incapable provider set has exact counts;
20. property test over identifier/provider cardinalities proves cap and accounting invariants.

### Gate B acceptance criteria

- [ ] Identifier deduplication occurs before budget reservation.
- [ ] `identifiers_planned` and `identifiers_scheduled` are unique counts.
- [ ] Duplicate identifiers cannot consume additional identifier capacity.
- [ ] Provider cap warning exists exactly when `skipped_by_budget > 0`.
- [ ] Empty capable-provider set does not trigger a cap warning.
- [ ] Partial reservations trigger a warning and still dispatch allowed providers.
- [ ] All scheduled operations remain represented after provider budget exhaustion.
- [ ] Package queries consume and report the same provider-operation budget.
- [ ] `planned == dispatched + skipped_by_budget` for capable operations.
- [ ] Actual provider calls never exceed the configured cap.

Do not continue to Gate C until these criteria and tests pass.

---

# Gate C — Make State Authoritative in Retrieval Helpers

## C.1 Required outcome

When `RetrievalDimensionStatus.state` is present, helper behavior must use it as the authoritative coarse terminal state.

Legacy `absence_kind` fallback is used only when `state` is absent.

## C.2 Add small pure state predicates

Add private or public helpers in `retrieval_status.rs`:

```rust
fn dimension_is_satisfied(d: &RetrievalDimensionStatus) -> bool;
fn dimension_is_completed_no_match(d: &RetrievalDimensionStatus) -> bool;
fn dimension_is_not_applicable(d: &RetrievalDimensionStatus) -> bool;
fn dimension_is_failed_or_interrupted(d: &RetrievalDimensionStatus) -> bool;
fn dimension_is_indeterminate(d: &RetrievalDimensionStatus) -> bool;
```

Preferred mappings when `state` exists:

| State | satisfied | no match | not applicable | failed/interrupted | indeterminate |
|---|---:|---:|---:|---:|---:|
| `Satisfied` | yes | no | no | no | no |
| `CompletedNoMatch` | no | yes | no | no | no |
| `Failed` | no | no | no | yes | yes |
| `SkippedByPolicy` | no | no | no | no | yes |
| `CapabilityUnavailable` | no | no | no | no | yes |
| `Interrupted` | no | no | no | yes | yes |
| `Partial` | no | no | no | no | yes |
| `NotApplicable` | no | no | yes | no | no |

Legacy fallback when `state` is absent:

- preserve prior interpretation as closely as possible;
- `absence_kind == NotApplicable` may be treated as legacy success because old responses cannot distinguish success from non-applicability;
- document that this ambiguity applies only to legacy dimensions without `state`.

## C.3 Correct role aggregate accounting

In `summarize_retrieval()`:

### Roles attempted

When state exists, include roles for all states except:

```text
NotApplicable
```

A genuinely non-applicable role was not attempted and should not inflate `roles_attempted`.

For legacy dimensions without state, preserve current fallback behavior.

### Roles complete

Count a role complete when at least one dimension for that role is:

```text
Satisfied
CompletedNoMatch
```

Do not mark `NotApplicable` as evidence-role completion.

Do not mark `Partial` complete.

### Roles indeterminate

Count a role indeterminate when at least one dimension is:

```text
Failed
SkippedByPolicy
CapabilityUnavailable
Interrupted
Partial
```

If the same role also has a `Satisfied` dimension from another provider, completion wins for the aggregate complete count, but the response may still retain failure dimensions. Do not erase the failures.

Recommended final set handling:

```rust
roles_complete.remove from roles_indeterminate only for aggregate unresolved-role count
```

or calculate:

```text
roles_indeterminate = indeterminate_roles - complete_roles
```

Document this choice and test it.

## C.4 Correct `is_absence_only()`

When state is present, `is_absence_only()` should return true only if every dimension is one of:

```text
CompletedNoMatch
NotApplicable
```

It must return false for:

```text
Satisfied
Partial
Failed
SkippedByPolicy
CapabilityUnavailable
Interrupted
```

A summary containing only successful evidence must not be classified absence-only.

For state-less legacy dimensions, preserve the old fallback.

## C.5 Correct `is_failure_only()` without broad rename

Do not rename the function in this pass.

Define its existing expected behavior explicitly. Preferred behavior based on current use:

```text
true when at least one dimension is Failed or Interrupted
```

If callers rely on “any failure” semantics, retain that behavior and update implementation to use state first.

Do not silently change it to “all dimensions failed” unless repository call sites and tests prove that is intended.

## C.6 Correct `has_indeterminate()`

When state exists, return true if any dimension is:

```text
Failed
SkippedByPolicy
CapabilityUnavailable
Interrupted
Partial
```

Legacy fallback remains based on old absence kinds.

## C.7 Correct `absent_roles()`

When state exists, return roles whose dimensions are:

```text
CompletedNoMatch
```

Do not include:

- successful evidence;
- genuine non-applicability;
- capability/policy skip;
- failure;
- partial results.

Deduplicate role output while preserving deterministic role order.

## C.8 Correct `failed_providers()`

When state exists, include providers with:

```text
Failed
Interrupted
```

Do not include:

- policy skips;
- capability skips;
- no-match;
- not-applicable;
- satisfied;
- partial, unless existing contract explicitly classifies confirmed truncation as provider failure.

Preserve deduplication and deterministic first-seen order.

## C.9 Keep legacy absence mapping stable

Do not change:

```rust
SuccessWithResults -> EvidenceAbsenceKind::NotApplicable
```

in this pass. The new `state` field is the compatibility-safe correction.

Update comments so future maintainers do not use `absence_kind` as the sole terminal-state predicate.

## C.10 Gate C tests

Add direct tests for every helper with state present:

1. satisfied-only summary is not absence-only;
2. completed-no-match-only summary is absence-only;
3. not-applicable-only summary is absence-only but has zero attempted roles;
4. satisfied role is complete;
5. completed-no-match role is complete;
6. not-applicable role is not complete;
7. failed role is indeterminate;
8. policy-skipped role is indeterminate;
9. capability-unavailable role is indeterminate;
10. interrupted role is indeterminate;
11. partial role is indeterminate and not complete;
12. one satisfied provider plus one failed provider for the same role yields complete aggregate role and preserves failure dimension;
13. `absent_roles()` returns completed-no-match only;
14. `failed_providers()` returns failed/interrupted only;
15. successful evidence with legacy `absence_kind = NotApplicable` and `state = Satisfied` is not treated as absent;
16. true non-applicability with `state = NotApplicable` is not treated as successful evidence;
17. state-less legacy fixture retains compatible behavior;
18. property test verifies every state maps to exactly the documented predicate set.

### Gate C acceptance criteria

- [ ] Every helper prefers `state` when present.
- [ ] Successful evidence is never classified as absence-only when state is present.
- [ ] Genuine non-applicability is never counted as completed evidence when state is present.
- [ ] Completed-no-match is treated as completed retrieval with absent evidence.
- [ ] Failure, interruption, skips, capability gaps, and partial results remain indeterminate.
- [ ] Legacy state-less JSON remains compatible.
- [ ] Aggregate role counts are deterministic and documented.

Do not continue to Gate D until these criteria and tests pass.

---

# Gate D — Restore Count-Level Consistency

## D.1 Required outcome

The public fields must have one stable meaning:

```text
attempted_job_count             attempt level
completed_job_count             attempt level
failed_job_count                attempt level
policy_skipped_count            attempt level
capability_skipped_count        attempt level
not_applicable_job_count        attempt level

attempted_dimension_count       dimension level
completed_dimension_count       dimension level
failed_dimension_count          dimension level
not_applicable_count            dimension level
```

## D.2 Add `not_applicable_job_count`

Add to `ResponseRetrievalSummary`:

```rust
/// Attempts whose outcome was `NotApplicable`.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub not_applicable_job_count: Option<usize>,
```

This field is additive and optional.

## D.3 Do not overwrite the dimension count

In `summarize_retrieval_with_attempts()`:

Replace:

```rust
summary.not_applicable_count = Some(attempt_counts.not_applicable);
```

with:

```rust
summary.not_applicable_job_count = Some(attempt_counts.not_applicable);
```

Leave `summary.not_applicable_count` as calculated from dimensions.

## D.4 Make dimension calculations state-aware

Calculate dimension counters from `state` when present.

### Completed dimension

Count:

```text
Satisfied
CompletedNoMatch
Partial
NotApplicable
```

The provider operation reached a terminal completed/partial state.

### Failed dimension

Count:

```text
Failed
Interrupted
```

Do not include policy/capability skips in failed dimensions.

### Not-applicable dimension

Count:

```text
NotApplicable
```

Legacy fallback may use absence/outcome when state is absent.

## D.5 Legacy `summarize_retrieval()` must not invent attempt counts

`summarize_retrieval(dimensions)` does not receive attempt records. It must not claim authoritative job-level counts based on dimension cardinality.

Preferred correction:

- calculate dimension-level fields;
- leave these attempt-level fields as `None`:
  - `attempted_job_count`;
  - `completed_job_count`;
  - `failed_job_count`;
  - attempt-level subtype counts where they cannot be derived without double-counting multi-role attempts.

However, some legacy callers may expect subtype counts from dimensions. To minimize risk:

- keep dimension-derived legacy subtype fields only if existing tests/contracts require them;
- add comments that attempt-derived summaries overwrite them authoritatively;
- at minimum, set the three `*_job_count` fields to `None` in the no-attempt path.

The authoritative attempt path remains:

```rust
summarize_retrieval_with_attempts(attempts, dimensions)
```

## D.6 Preserve the attempt partition invariant

For attempt-derived summaries:

```text
attempted_job_count
  == completed_job_count
   + failed_job_count
   + policy_skipped_count
   + capability_skipped_count
```

`not_applicable_job_count` is a subset of `completed_job_count`; do not add it again to the partition equation.

Similarly:

- `timed_out_count` is a subset of `failed_job_count`;
- `rate_limited_count` is a subset of `failed_job_count`;
- `deadline_interrupted_count` is a subset of `failed_job_count`;
- `zero_result_count` is a subset of `completed_job_count`;
- `truncated_count` is a subset of `completed_job_count`.

## D.7 Two-role example

Given one attempt:

```text
outcome = NotApplicable
roles = [OfficialDocumentation, UsageExample]
```

Expected:

```text
attempted_job_count = 1
completed_job_count = 1
not_applicable_job_count = 1
attempted_dimension_count = 2
completed_dimension_count = 2
not_applicable_count = 2
```

This exact fixture is mandatory.

## D.8 Gate D tests

Add tests for:

1. one-role non-applicable attempt;
2. two-role non-applicable attempt with exact counts above;
3. two-role successful attempt has one job and two dimensions;
4. two-role failed attempt has one failed job and two failed dimensions;
5. mixed attempts satisfy attempt partition invariant;
6. subtype counts are subsets, not additive partitions;
7. legacy `summarize_retrieval(dimensions)` leaves job counts unknown/None;
8. attempt-derived path fills job counts;
9. old JSON without `not_applicable_job_count` deserializes;
10. new JSON serializes both count levels distinctly;
11. codegg contract fixture matches implementation;
12. property test over role-vector lengths verifies attempt counts do not scale with role count.

### Gate D acceptance criteria

- [ ] `not_applicable_count` is always dimension-level.
- [ ] `not_applicable_job_count` is always attempt-level.
- [ ] Attempt-derived summaries never overwrite dimension counts with attempt counts.
- [ ] No-attempt summaries do not invent authoritative job counts from dimensions.
- [ ] Multi-role attempts count as one job and multiple dimensions.
- [ ] Attempt partition invariant holds for every outcome combination.
- [ ] Existing clients remain compatible because all new fields are optional/additive.

Do not continue to Gate E until these criteria and tests pass.

---

# Gate E — Repair the codegg Contract Document

## E.1 Required outcome

`docs/architecture/codegg-contract.md` must have valid table structure, unique logical section numbering, and examples matching serialized snake_case values.

## E.2 Repair the Dirty State table

Move these rows back into the Dirty State table immediately after `clean` and `dirty`:

```markdown
| `unknown` | Could not determine dirty state | Treat as dirty (conservative) |
| `not_git` | Not a git repository | Ignore dirty state |
```

The table must be complete before the next horizontal rule or heading.

## E.3 Fix section numbering

Renumber the inserted retrieval section so later headings remain monotonic.

Choose one consistent sequence. Example:

```text
8. Local Repository Metadata
  8.1 Local Repo Match
  8.2 Match Confidence
  8.3 Dirty State
  8.4 File Classification Flags
  8.5 Workspace ID
9. Retrieval Dimension State
  9.1 ...
10. Capability Discovery
  10.1 ...
```

Update all later duplicate numbering as required. Do not rewrite unrelated prose.

## E.4 Use serialized values in protocol tables

The Rust enum serializes snake_case. Contract tables should use serialized wire values:

```text
satisfied
completed_no_match
failed
skipped_by_policy
capability_unavailable
interrupted
partial
not_applicable
```

Rust variant names may be shown parenthetically, but the primary contract value must be the actual JSON value.

## E.5 Correct count-level documentation

Document:

```text
not_applicable_job_count = attempt level
not_applicable_count = dimension level
```

State that attempt-level subtype counts are subsets of completed or failed counts.

## E.6 Correct operation identity documentation

Add a brief compatibility note:

- `operation_id` distinguishes operation instances sharing one subquery class;
- consumers should group dimensions by `(provider_id, operation_id)` when reconstructing one provider operation;
- role dimensions for one operation may share operation ID;
- absence of operation ID indicates a legacy record and consumers may fall back to subquery ID.

Do not expose internal fingerprint inputs.

## E.7 Add document-contract tests

Add focused tests that verify:

1. Dirty State table contains all four rows before the next heading;
2. no duplicate top-level section number remains in the affected area;
3. serialized state values appear in lowercase snake_case;
4. both not-applicable count fields are documented at the correct level;
5. operation ID fallback behavior is documented;
6. JSON fixture parses into current response structs;
7. fixture contains distinct satisfied and not-applicable states.

### Gate E acceptance criteria

- [ ] No table rows appear outside their intended table.
- [ ] Affected section numbering is monotonic and non-duplicated.
- [ ] Wire-format values use snake_case.
- [ ] Count-level semantics match implementation.
- [ ] `operation_id` semantics and legacy fallback are documented.
- [ ] Contract JSON fixture deserializes in tests.

Do not select release subject `R` until Gate E passes.

---

# Gate F — Focused Regression and Compatibility Matrix

## F.1 Required test suites

Run after Gates A–E:

```bash
cargo test --locked --all-features --test retrieval_attempt_ledger
cargo test --locked --all-features --test property_retrieval
cargo test --locked --all-features --test codegg_evidence_contract
cargo test --locked --all-features --test static_guards
cargo test --locked --all-features security_search
```

Then run full feature coverage:

```bash
cargo test --locked --no-default-features
cargo test --locked --features mock
cargo test --locked --features pdf
cargo test --locked --all-features
```

Use exact feature names from the repository if they differ. Do not omit a repository-supported matrix entry that the release gate already requires.

## F.2 Formatting and lint

Run:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
```

## F.3 Schema and docs

Run the repository’s existing schema, documentation-contract, and rustdoc checks. At minimum:

```bash
cargo test --locked --doc --all-features
```

Also run the canonical `make check` if it exists and includes more gates.

## F.4 Compatibility fixtures

Mandatory compatibility cases:

1. old `RetrievalAttempt` JSON without `operation_id`;
2. new attempt JSON with `operation_id`;
3. old dimension JSON without `state`;
4. new dimension JSON with `state`;
5. old summary JSON without new count fields;
6. new summary JSON with both not-applicable count levels;
7. multi-operation advisory fixture;
8. multi-role package fixture;
9. codegg contract fixture round-trip.

## F.5 Static guards

Add or update static guards against these regressions:

- validator key omits `operation_id`;
- identifier reservation occurs before deduplication;
- provider cap flag derives from `allowed.is_empty()`;
- provider-cap warning is emitted without skipped-by-budget operations;
- state-aware helper regresses to sole `absence_kind == NotApplicable` predicate;
- `summarize_retrieval_with_attempts()` assigns attempt count to `not_applicable_count`;
- codegg contract has duplicate affected section numbering;
- Dirty State rows appear after Retrieval Dimension State heading.

Prefer behavioral tests over brittle source-string guards. Static guards are supplemental.

### Gate F acceptance criteria

- [ ] Focused suites pass.
- [ ] All supported feature combinations pass.
- [ ] Formatting and clippy pass with warnings denied.
- [ ] Docs/schema/rustdoc checks pass.
- [ ] Legacy and enriched fixtures both deserialize.
- [ ] No static regression pattern remains.

---

# Gate G — Select and Verify Final Release Subject `R`

## G.1 Select `R`

After all code, tests, docs, schema, and workflow corrections are committed and the working tree is clean:

1. run the full local deterministic gate;
2. ensure no pending source/test/doc fixes remain;
3. record the current full 40-character SHA as candidate release subject `R`;
4. do not amend, rebase, or add code-bearing commits after selecting it.

The current baseline `d08ca059...` cannot be final `R` because this corrective pass changes runtime and contract behavior.

## G.2 Local deterministic evidence

Record:

- full `R` SHA;
- date;
- operating system;
- architecture;
- Rust toolchain version;
- exact commands run;
- exact pass/fail status;
- test counts where available;
- benchmark compile and execution status.

Do not write final claims before commands finish successfully.

## G.3 Linux and macOS CI

Required:

- Linux CI on exact `R`;
- macOS CI on exact `R`;
- formatting/clippy checks;
- all feature/test matrices configured by the repository;
- documentation/schema/static guards;
- release build/package gate.

A later test-only or documentation-contract correction invalidates `R` because it changes the verified release contract.

## G.4 Affected-path benchmarks

Execute and retain evidence for benchmarks covering:

- ledger validation with multiple operation IDs;
- summary construction with multi-role attempts;
- state-aware helper aggregation;
- 32 scheduled unique identifiers;
- 64 dispatched provider operations;
- provider-budget exhaustion with policy-skip ledger emission;
- package operation after budget exhaustion.

Accurately distinguish:

- benchmark compiled;
- benchmark executed;
- artifact retained.

Do not claim heap stability unless measured.

## G.5 Native forge evidence

Manually run `.github/workflows/native-forge-smoke.yml` against exact `R`.

All required provider jobs must execute and pass:

- GitHub;
- GitLab;
- Codeberg/Forgejo;
- configured distinct Gitea instance;
- summary/manifest job.

Required evidence:

- workflow run ID;
- each provider job result;
- each artifact name/ID;
- combined manifest artifact;
- release subject inside every evidence file equals `R`;
- evidence file hashes;
- no missing-token, missing-fixture, fallback, skipped, or not-run result accepted as pass.

### Gate G acceptance criteria

- [ ] `R` is the final code-bearing commit.
- [ ] Local deterministic gate passes on exact `R`.
- [ ] Linux CI passes on exact `R`.
- [ ] macOS CI passes on exact `R`.
- [ ] Affected-path benchmarks execute and evidence is retained.
- [ ] All four native forge providers execute and pass on exact `R`.
- [ ] Combined evidence manifest is present and valid.
- [ ] No code-bearing commit exists after `R` before evidence commit `E`.

---

# Gate H — Create Evidence-Only Commit `E`

## H.1 Update the release record only after evidence exists

Update `docs/release-verification.md` with:

- classification: `release candidate`;
- exact full `R` SHA;
- exact full `E` SHA after commit creation, using the repository’s established two-step evidence-record method if necessary;
- local gate details;
- Linux run ID/result;
- macOS run ID/result;
- benchmark artifact identity;
- native forge workflow run ID;
- provider job results;
- provider artifact names/IDs;
- combined manifest identity;
- SHA-256 evidence hashes;
- exact statement that `R..E` changes only approved evidence/documentation paths.

If self-referencing `E` cannot be recorded in the same commit, use the repository’s accepted evidence protocol. Do not create an endless chain of evidence commits.

## H.2 Allowed evidence paths

Preferred allowlist:

```text
docs/release-verification.md
docs/release-checklist.md
evidence/**
```

No changes allowed in `E` to:

```text
src/**
tests/**
benches/**
.github/workflows/**
Cargo.toml
Cargo.lock
Makefile
public schemas
codegg contract
```

If any forbidden path needs correction, abandon the current `R`, make the correction, and select a new `R`.

## H.3 Verify `R..E`

Run a path-level diff check and record the result.

Expected:

```text
only allowlisted release documentation/evidence paths changed
```

## H.4 No pending-as-complete claims

The final record must not contain:

- `pending` for a required release gate;
- placeholder run IDs;
- benchmark “compiled” presented as “executed”;
- scheduled diagnostic runs presented as release evidence;
- skipped native provider jobs presented as pass;
- old provisional subject presented as final `R`.

### Gate H acceptance criteria

- [ ] `E` contains only allowlisted evidence/documentation changes.
- [ ] Release record names exact `R` and evidence protocol’s exact `E` identity.
- [ ] All required run IDs, artifacts, and hashes are recorded.
- [ ] No required item remains pending.
- [ ] Final classification is supported by evidence.
- [ ] `R..E` contains no code, tests, workflows, benchmarks, dependency, schema, or contract changes.

---

## 4. Expected Files to Modify

Primary runtime files:

```text
src/core/retrieval_status.rs
src/core/evidence_postprocess.rs
src/meta/dispatch.rs
src/meta/security_search.rs
```

Possible central assembly/call-site files:

```text
src/meta/adapter.rs
src/core/mod.rs
```

Primary tests:

```text
tests/retrieval_attempt_ledger.rs
tests/property_retrieval.rs
tests/codegg_evidence_contract.rs
tests/static_guards.rs
```

Documentation:

```text
docs/architecture/codegg-contract.md
docs/release-verification.md
```

Benchmark file only if existing benchmark coverage is extended:

```text
benches/perf.rs
```

Do not modify unrelated files merely to improve style.

---

## 5. Recommended Commit Sequence

### Commit 1 — Operation identity closure

```text
fix: bind retrieval ledger entries to operation identity
```

Contents:

- optional `operation_id` field;
- stable identity encoding;
- production population;
- validator key correction;
- focused Gate A tests.

### Commit 2 — Advisory budget correction

```text
fix: correct native advisory budget accounting
```

Contents:

- pre-budget identifier deduplication;
- complete scheduled-operation processing;
- exact cap warning condition;
- package path correction;
- Gate B tests.

### Commit 3 — State-aware helpers and count levels

```text
fix: make retrieval state and count levels authoritative
```

Contents:

- state-aware predicates/helpers;
- aggregate role corrections;
- `not_applicable_job_count`;
- no attempt/dimension overwrite;
- Gate C/D tests.

### Commit 4 — codegg contract repair

```text
docs: repair retrieval telemetry contract structure
```

Contents:

- table repair;
- section numbering;
- snake_case values;
- operation/count documentation;
- document-contract tests.

### Commit 5 — Verification-only implementation adjustments

Use only if full matrix exposes a real issue:

```text
test: close final retrieval ledger verification gaps
```

After this commit passes all gates, it becomes candidate `R`.

### Commit 6 — Evidence-only `E`

```text
docs(release): record final eggsearch release evidence
```

No code/test/workflow changes.

---

## 6. Mandatory Acceptance Matrix

| Area | Required evidence |
|---|---|
| Operation identity | distinct advisory IDs under one provider/subquery/role validate |
| Duplicate detection | same provider/operation/role duplicate is rejected |
| Privacy | operation IDs expose no raw identifier/package/version/query text |
| Legacy compatibility | missing operation ID deserializes and uses deterministic fallback |
| Identifier budget | duplicates consume no additional capacity |
| Identifier warning | emitted only when unique planned IDs exceed scheduled IDs |
| Provider warning | emitted exactly when capable operations are budget-skipped |
| Zero capable providers | capability skips only; no budget warning |
| Partial reservation | allowed providers dispatch; skipped providers are policy skips; warning present |
| Budget invariant | planned capable operations equal dispatched plus budget-skipped |
| State authority | helpers use state when present |
| Success semantics | satisfied evidence is not absence-only |
| Non-applicability | not-applicable is not evidence completion |
| Role aggregates | complete/indeterminate sets follow documented state table |
| Attempt counts | independent of role-vector length |
| Dimension counts | scale with role expansion |
| Not-applicable counts | job and dimension levels are distinct |
| Legacy summary | no invented authoritative job counts from dimensions |
| Contract document | valid table structure and monotonic numbering |
| Wire values | documented state values are snake_case |
| Focused tests | all pass |
| Feature matrix | all pass |
| Lint/format/docs | all pass |
| Linux CI | exact `R` passes |
| macOS CI | exact `R` passes |
| Benchmarks | executed and retained for `R` |
| Native forge | all four providers exact-pass on `R` |
| Evidence commit | only allowlisted evidence paths differ from `R` |

Any failed row keeps classification at **provisional release candidate**.

---

## 7. Final Definition of Done

This corrective pass is complete only when:

1. every authoritative retrieval attempt has a deterministic operation ID;
2. ledger uniqueness is enforced by provider, operation, and role;
3. legitimate multiple advisory IDs no longer collide;
4. duplicate identifiers do not consume identifier budget;
5. provider-cap warnings have no false positives or false negatives;
6. every scheduled operation remains represented after provider budget exhaustion;
7. state-aware helpers distinguish success, no-match, failure, skip, partial, and non-applicability;
8. attempt-level and dimension-level not-applicable counts are separate and documented;
9. codegg contract structure and serialized examples are correct;
10. all focused, feature-matrix, property, static, schema, docs, lint, and formatting checks pass;
11. a final immutable code-bearing subject `R` is selected;
12. Linux, macOS, benchmarks, and all four native forge jobs verify exact `R`;
13. evidence-only commit `E` records exact runs, artifacts, hashes, and final classification;
14. `R..E` contains only approved evidence/documentation paths.

Stop after these conditions are met. Any new provider, feature, ranking, or architectural improvement belongs in a separate post-release plan.
