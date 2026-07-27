# Final Keyless-Core and Retrieval-Semantics Closure

**Repository:** `eggstack/eggsearch`  
**Baseline:** `d5eb3b130f7b4d97d8d5ec6483b18613add8213d`  
**Status:** Small-model implementation handoff  
**Scope:** Narrow final closure only  
**Primary consumer:** codegg and other MCP agent hosts  
**Product invariant:** a useful production baseline must require no API keys  
**Release model:** core keyless release evidence is mandatory; credentialed-adapter evidence is optional and claim-scoped

---

## 1. Objective

The repository is close to release, but two classes of work remain:

1. a small set of retrieval-summary and ledger correctness defects;
2. an explicit product and release invariant that protects eggsearch's central value proposition: **users must never need API keys for the core product to start and provide useful service**.

Credentialed GitHub, GitLab, Gitea/Forgejo, Sourcegraph, Brave API, Semantic Scholar, and similar adapters are optional enhancements. They may improve forge-native precision, provenance, structured metadata, rate limits, or coverage, but they must not become prerequisites for:

- installation;
- startup;
- generic web search;
- bounded fetch;
- baseline coding-oriented search;
- baseline security search;
- baseline research search;
- local workspace search when explicitly configured;
- release classification of the core artifact.

Maintainers may use credentials to prove that optional adapters work. That evidence verifies an adapter-specific claim. It must not imply that users need those credentials, and the absence of third-party credentials must not block release of the keyless core.

This pass closes the remaining correctness issues, codifies the keyless-core invariant in runtime behavior and tests, repairs the operator and codegg documentation, and establishes a two-layer evidence model:

```text
core keyless release evidence         required
optional adapter conformance evidence optional, per adapter, claim-scoped
```

Do not broaden the provider inventory, tool surface, ranking model, query planner, or security-analysis scope.

---

## 2. Non-Goals

This plan does **not** authorize:

- adding new providers;
- removing existing optional providers solely to simplify the release;
- changing MCP tool names or required request fields;
- creating a research agent inside eggsearch;
- turning eggsearch into a crawler;
- requiring a hosted aggregator;
- requiring SearXNG;
- requiring a local workspace;
- requiring GitHub, GitLab, Codeberg, Gitea, or Sourcegraph credentials;
- promising parity between keyless generic search and every forge-native API feature;
- silently treating generic web evidence as native forge evidence;
- weakening fail-closed adapter tests;
- weakening fetch SSRF protections, trust labels, sanitization, or bounded-resource behavior;
- redesigning the provider trait hierarchy;
- performing unrelated refactors or dependency upgrades;
- finalizing release evidence before all code and documentation corrections land.

---

## 3. Product Invariant

The following statement must be true in code, tests, documentation, and release evidence:

> A clean eggsearch installation with no configuration file and no provider credential environment variables starts successfully and provides a useful keyless MCP search/fetch service. Credentialed providers are optional enhancements. Missing optional credentials are reported as provider-scoped non-routability and never make the server globally unhealthy or fail an otherwise serviceable request.

### 3.1 Required keyless capabilities

With no API keys, eggsearch must support:

| Capability | Required keyless behavior |
|---|---|
| Server startup | starts successfully with defaults |
| `provider_status` | succeeds and identifies optional credentialed providers as non-routable without classifying the server as failed |
| `web_search` | queries configured keyless defaults |
| `web_fetch` | bounded explicit fetch works without provider credentials |
| `batch_fetch` | bounded explicit batch fetch works without provider credentials |
| `repo_search` | produces coding-oriented evidence from available keyless web/local sources or a truthful zero-result response; missing forge credentials alone must not fail the request |
| `repo_fetch` | supports keyless explicit public HTTP(S) or local workspace paths already supported by the repository; native credential-only routes remain optional |
| `repo_map` | supports keyless public/local paths already supported by the repository or returns a truthful scoped capability result; missing optional credentials must not fail unrelated providers |
| `security_search` | uses available keyless advisory and web providers such as OSV, NVD, CISA KEV, RustSec, and keyless generic search |
| `research_search` | uses available keyless web and scholarly sources |
| `build_evidence_bundle` | operates on supplied evidence without provider credentials |

This table does not require every tool to fabricate a result. It requires successful request handling, useful available behavior, and truthful provider-scoped telemetry.

### 3.2 Optional enhancement rule

A credentialed adapter may add:

- native repository tree traversal;
- exact code search;
- issue and release search;
- private repository access;
- higher rate limits;
- structured provider-specific advisory data;
- stronger immutable provenance;
- richer provider metadata.

The absence of that adapter may reduce quality or capability. It must not prevent the keyless baseline from operating.

### 3.3 Release-claim rule

The release may claim:

```text
eggsearch core is release-verified in keyless mode
```

when the core keyless release gate passes.

The release may claim:

```text
GitHub native adapter verified
```

only when GitHub adapter evidence exists for exact release subject `R`.

The absence of GitLab, Codeberg, or Gitea credentials must not block the core release. It only prevents claiming those individual adapters as release-verified.

---

## 4. Small-Model Execution Rules

Follow these rules exactly:

1. Work in gate order.
2. Add a failing focused regression test before correcting each defect.
3. Do not add providers or MCP tools.
4. Do not change `query` or `url` from being the primary required agent-facing inputs.
5. Public response changes must be additive and optional unless correcting an already-documented semantic bug.
6. Preserve existing provider IDs and serialized enum values.
7. Keep missing-credential behavior provider-scoped.
8. Do not silently downgrade native evidence into generic evidence.
9. Do not weaken adapter-specific fail-closed workflows.
10. Separate core release status from optional adapter verification status.
11. Do not select a new release subject `R` until all code, tests, docs, lint, and feature gates pass.
12. Any code, test, workflow, schema, or contract change after selecting `R` creates a new `R`.
13. Evidence commit `E` must contain only approved release evidence/documentation changes.
14. When uncertain, preserve the existing implementation and add the smallest change necessary to satisfy the acceptance criteria.

---

## 5. Current Defects to Preserve as Regression Tests

### Defect A — dimension non-applicability uses legacy absence kind

`not_applicable_count` is currently derived from:

```rust
d.absence_kind == EvidenceAbsenceKind::NotApplicable
```

Successful dimensions also retain the legacy `NotApplicable` absence value. A successful dimension is therefore incorrectly counted as non-applicable.

### Defect B — role accounting performs legacy and state classification

The summary loop first marks legacy `absence_kind == NotApplicable` roles complete, then separately evaluates `RetrievalDimensionState`. A genuinely non-applicable role can be inserted into `roles_with_success` before the state branch correctly says it is neither attempted nor complete.

### Defect C — dimension-only summaries invent job counts

`summarize_retrieval(dimensions)` currently derives job counters from dimension count. A role-expanded dimension list does not prove how many provider operations occurred.

### Defect D — identifier-cap omitted count is incomplete

The ID loop stops after the first identifier beyond the scheduling cap. `identifiers_planned` therefore does not include the unvisited remainder of the deduplicated input.

### Defect E — production ledger assembly does not validate the invariant

`validate_attempt_ledger()` exists and is tested, but the complete assembled attempt vector is not checked at central production assembly boundaries.

### Defect F — codegg contract remains structurally incorrect

The local file-classification section was replaced by retrieval-state content, workspace-ID documentation is missing, section hierarchy is inconsistent, and state tables use Rust variant spelling instead of JSON wire values.

### Defect G — release proof treats optional adapter evidence too centrally

The current release record and README describe all native forge provider evidence as required for release promotion. That conflates:

- core keyless release proof;
- optional provider adapter conformance.

### Defect H — keyless mode lacks a release-grade explicit test matrix

The repository documents keyless defaults, but it does not yet have one authoritative release gate proving that a scrubbed environment and absent config still provide the complete baseline service.

---

# Gate A — Make Retrieval Summary State-Authoritative

## A.1 Required outcome

When `RetrievalDimensionStatus.state` exists, every aggregate interpretation must use it as the authoritative terminal state.

Legacy `absence_kind` may be used only when `state` is `None`.

The summary must never classify one dimension twice through both legacy and state paths.

## A.2 Add pure state-first classification helpers

Add or consolidate nearby private helpers in `src/core/retrieval_status.rs`:

```rust
fn dimension_state_or_legacy(d: &RetrievalDimensionStatus) -> EffectiveDimensionState;
```

The exact internal type may differ. It must represent at least:

```text
Satisfied
CompletedNoMatch
Failed
SkippedByPolicy
CapabilityUnavailable
Interrupted
Partial
NotApplicable
LegacyOtherAbsence
```

Prefer a simple function returning `RetrievalDimensionState` when a safe mapping exists. Do not add a new public enum unless required.

Required mapping when `state` is present:

| State | Attempted role | Complete role | Indeterminate role | Non-applicable dimension |
|---|---:|---:|---:|---:|
| `Satisfied` | yes | yes | no | no |
| `CompletedNoMatch` | yes | yes | no | no |
| `Partial` | yes | no | yes | no |
| `Failed` | yes | no | yes | no |
| `Interrupted` | yes | no | yes | no |
| `SkippedByPolicy` | yes | no | yes | no |
| `CapabilityUnavailable` | yes | no | yes | no |
| `NotApplicable` | no | no | no | yes |

Legacy fallback must preserve existing compatibility as closely as possible.

## A.3 Rewrite the summary loop as one classification path

In `summarize_retrieval()`:

1. inspect `state` first;
2. if `state` is absent, apply legacy absence/outcome mapping;
3. update role sets and dimension counters exactly once;
4. do not run a second independent classification block.

Remove patterns equivalent to:

```rust
if d.absence_kind == EvidenceAbsenceKind::NotApplicable {
    roles_with_success.insert(...);
}

if let Some(state) = d.state {
    // classify again
}
```

## A.4 Correct dimension counts

Compute these from state when present:

```text
attempted_dimension_count
completed_dimension_count
failed_dimension_count
not_applicable_count
```

Required semantics:

- `attempted_dimension_count`: all dimensions except authoritative `NotApplicable` dimensions;
- `completed_dimension_count`: `Satisfied`, `CompletedNoMatch`, and `Partial` if the existing contract treats partial as completed provider execution; document the exact choice;
- `failed_dimension_count`: `Failed` and `Interrupted`; do not count capability/policy skips as provider failures;
- `not_applicable_count`: authoritative `NotApplicable` dimensions only.

If compatibility requires `attempted_dimension_count == dimensions.len()`, retain that field and add a separate optional applicable/attempted field only if necessary. Prefer correcting the documented meaning without broad schema expansion.

## A.5 Correct role counts

Required semantics:

```text
roles_attempted      distinct roles with any applicable operation
roles_complete       distinct roles with at least one Satisfied or CompletedNoMatch dimension and no stronger unresolved requirement under existing aggregation policy
roles_indeterminate  distinct roles with failures, interruption, policy skip, capability skip, or partial evidence where completion cannot be proven
```

Do not count a role as attempted solely because a `NotApplicable` dimension exists.

If one role has both `Satisfied` and `Failed` dimensions from different providers, preserve the existing workflow policy but make it deterministic and document it. Do not silently erase the failure dimension.

## A.6 Correct helper behavior

Verify and correct:

- `is_absence_only`;
- `is_failure_only`;
- `has_indeterminate`;
- `absent_roles`;
- `failed_providers`.

All must prefer `state` when present.

`is_failure_only` must mean all applicable dimensions are failure/interrupted, not merely that any failure exists. If changing this would break established callers, add a specifically named helper and migrate callers rather than preserving misleading semantics.

## A.7 Gate A tests

Add focused tests:

1. successful dimension is not counted as non-applicable;
2. one success plus one genuine non-applicable dimension gives `not_applicable_count == 1`;
3. multi-role non-applicable attempt gives dimension count equal to role expansion and job count equal to one;
4. non-applicable-only role is not complete;
5. non-applicable-only role is not attempted;
6. satisfied role is complete;
7. zero-result role is complete but absent;
8. partial role is indeterminate;
9. capability skip is indeterminate, not failed;
10. policy skip is indeterminate, not failed;
11. failed dimension is failed and indeterminate;
12. interrupted dimension is failed and indeterminate;
13. mixed satisfied and failed providers preserve both signals;
14. legacy dimension with `state = None` remains compatible;
15. state overrides contradictory legacy `absence_kind`.

### Gate A acceptance criteria

- [ ] `not_applicable_count` uses authoritative state when present.
- [ ] Genuine non-applicability is not marked complete.
- [ ] Genuine non-applicability is not marked attempted.
- [ ] Summary classification occurs once per dimension.
- [ ] State overrides contradictory legacy absence values.
- [ ] Legacy state-less fixtures retain compatible behavior.
- [ ] Helper names and semantics are no longer misleading.

Do not continue until focused tests pass.

---

# Gate B — Stop Dimension-Only Summaries from Inventing Job Counts

## B.1 Required outcome

Only a real `&[RetrievalAttempt]` can produce attempt/job counters.

Dimension-only fallback summaries must not infer provider job counts from role-expanded dimensions.

## B.2 Change `summarize_retrieval(dimensions)`

The following fields must be `None` when attempts are unavailable:

```text
attempted_job_count
completed_job_count
failed_job_count
zero_result_count            if defined as attempt-level
 timed_out_count             if defined as attempt-level
rate_limited_count           if defined as attempt-level
policy_skipped_count         if defined as attempt-level
capability_skipped_count     if defined as attempt-level
deadline_interrupted_count   if defined as attempt-level
truncated_count              if defined as attempt-level
limit_reached_unknown_count  if defined as attempt-level
not_applicable_job_count
```

Review the public contract before deciding which subtype counters are dimension-level versus attempt-level. The current documentation presents job-oriented subtype counts. Make the implementation and docs agree.

Dimension fields remain populated:

```text
attempted_dimension_count
completed_dimension_count
failed_dimension_count
not_applicable_count
roles_attempted
roles_complete
roles_indeterminate
has_failures
has_absences
has_truncation
```

## B.3 Keep attempt-derived summary authoritative

`summarize_retrieval_with_attempts()` must populate all job counters from `AttemptSummaryCounts` and must not overwrite dimension counters.

Required invariant:

```text
attempted_job_count
  == completed_job_count
   + failed_job_count
   + policy_skipped_count
   + capability_skipped_count
```

`not_applicable_job_count` is a subset of `completed_job_count`, not an additional partition term.

## B.4 Gate B tests

1. dimension-only summary has all job counters `None`;
2. dimension-only summary still has dimension counters and flags;
3. one multi-role attempt produces one attempted job and multiple dimensions;
4. two attempts sharing one role produce two jobs and two dimensions;
5. non-applicable multi-role attempt produces one `not_applicable_job_count` and N `not_applicable_count`;
6. attempt partition invariant holds across all outcomes;
7. serde omission preserves old consumers when job counters are `None`;
8. codegg contract fixture distinguishes job and dimension levels.

### Gate B acceptance criteria

- [ ] Dimension-only paths never manufacture job counts.
- [ ] Attempt-derived paths populate job counts from attempts only.
- [ ] Dimension counts are never overwritten by attempt counts.
- [ ] Count-level documentation matches serialization behavior.

---

# Gate C — Build the Complete Deduplicated Identifier Plan Before Budgeting

## C.1 Required outcome

Native advisory budget telemetry must know the complete unique input plan before applying scheduling caps.

For 40 unique identifiers with a 32-identifier cap:

```text
identifiers_planned   = 40
identifiers_scheduled = 32
identifiers_omitted   = 8
```

Duplicates must not consume slots.

## C.2 Introduce a pure planning helper

Add a bounded pure helper near native security orchestration:

```rust
struct PlannedAdvisoryIdentifier {
    identifier: String,
    subquery_id: &'static str,
    operation: RetrievalOperationIdentity,
}

fn plan_unique_advisory_identifiers(
    resolved: &SecurityIdentifiers,
) -> Vec<PlannedAdvisoryIdentifier>;
```

Requirements:

- stable family order: CVE, GHSA, OSV, RustSec unless existing policy differs;
- stable within-family order from parsed input;
- global deduplication across repeated values;
- case normalization only when valid for that identifier family;
- no random ordering;
- bounded by the existing request/query limits;
- planning itself performs no network calls.

## C.3 Apply the identifier cap after planning

Recommended flow:

```rust
let planned = plan_unique_advisory_identifiers(&resolved_ids);
budget_summary.identifiers_planned = planned.len();

for item in planned.iter().take(MAX_NATIVE_ADVISORY_IDENTIFIERS) {
    let reserved = budget.reserve_identifier();
    debug_assert!(reserved);
    // reserve providers and dispatch/skip
}

budget_summary.identifiers_scheduled = planned
    .len()
    .min(MAX_NATIVE_ADVISORY_IDENTIFIERS);
```

Do not stop planning when the cap is reached.

## C.4 Correct warning calculation

Identifier-cap warning exists if and only if:

```text
identifiers_planned > identifiers_scheduled
```

Warning must report exact omitted count:

```text
identifiers_planned - identifiers_scheduled
```

Provider-operation warning remains based on actual capable provider operations skipped by budget.

## C.5 Correct misleading tests

Replace any test that calls `reserve_identifier()` twice and labels this duplicate deduplication.

A duplicate test must exercise the planning helper or full orchestration:

```text
input IDs: CVE-X, CVE-X, CVE-Y
planned unique IDs: 2
scheduled slots consumed: 2
```

## C.6 Gate C tests

1. 40 unique IDs report 40 planned, 32 scheduled, eight omitted;
2. 40 repeated copies of one ID report one planned and one scheduled;
3. mixed duplicates across input fields are deduplicated according to identifier normalization policy;
4. stable family order is preserved;
5. provider-operation budget still processes all 32 scheduled operation identities, creating policy-skip attempts after exhaustion;
6. no-cap case emits no identifier-cap warning;
7. exact-cap case emits no warning;
8. cap-plus-one emits omitted count one;
9. planning is deterministic;
10. raw identifier values do not leak into `operation_id`.

### Gate C acceptance criteria

- [ ] Complete unique planning precedes budget reservation.
- [ ] Duplicate IDs consume one slot.
- [ ] Omitted count is exact.
- [ ] Scheduled operation identities remain deterministic.
- [ ] Provider-operation policy-skip telemetry remains complete.

---

# Gate D — Enforce Ledger Invariants at Production Assembly Boundaries

## D.1 Required outcome

Complete attempt ledgers are validated in debug/test builds immediately before postprocessing or response construction.

Validation must remain non-fatal in production release behavior unless a separate explicit strict mode already exists.

## D.2 Add one helper to avoid inconsistent assertions

Recommended helper:

```rust
#[inline]
fn debug_validate_attempt_ledger(context: &str, attempts: &[RetrievalAttempt]) {
    debug_assert!(
        validate_attempt_ledger(attempts).is_ok(),
        "{context}: assembled retrieval attempt ledger must satisfy invariants"
    );
}
```

If `debug_assert!` prevents useful violation detail, use:

```rust
if cfg!(debug_assertions) {
    if let Err(err) = validate_attempt_ledger(attempts) {
        panic!("... {err:?}");
    }
}
```

Do not log raw query content or credentials.

## D.3 Apply at authoritative assembly points

Inspect at least:

- generic web-search attempt assembly;
- repo-search attempt assembly;
- research-search attempt assembly;
- security-search `all_attempts` assembly;
- central postprocess entry if it always receives complete attempts.

Avoid validating a partial intermediate vector that legitimately lacks terminal attempts for pending jobs.

## D.4 Gate D tests

1. assembled multi-ID/multi-provider security ledger validates;
2. package advisory and dependency role attempts validate;
3. generic multi-role attempt ledger validates;
4. deliberate duplicate panics or fails under focused debug test;
5. release build behavior does not turn an invariant diagnostic into a user-facing MCP failure;
6. validation diagnostics contain provider/operation/role but no raw query text.

### Gate D acceptance criteria

- [ ] Every authoritative complete attempt vector is checked in debug/test builds.
- [ ] Validation is not applied to in-flight partial vectors.
- [ ] Release behavior does not fail requests solely due to debug invariant machinery.
- [ ] Diagnostics do not leak sensitive query content.

---

# Gate E — Codify the Keyless-Core Runtime Contract

## E.1 Required outcome

No configuration and no credential environment variables produce a healthy, useful server.

Missing optional credentials never become a global startup or request failure.

## E.2 Define credential categories without redesigning providers

Use existing provider metadata where possible:

```text
keyless built-in
optional endpoint/configuration
optional credentialed
local operator-configured
```

Do not add a new public enum unless the existing `requires_api_key`, provider kind, and routability metadata cannot express the required contract.

At minimum, central test helpers should be able to classify providers into:

- no credential required;
- credential required but optional;
- non-credential configuration required, such as SearXNG base URL;
- local backend required.

## E.3 Startup behavior

With all provider credential variables absent:

- config parsing succeeds;
- server state construction succeeds;
- default providers are routable if their non-credential requirements are available;
- credentialed providers are disabled/non-routable;
- missing credentials produce provider-scoped status entries;
- no global fatal error is returned;
- `doctor` may report optional provider issues but exits successfully unless a genuinely required core configuration is invalid.

Do not auto-enable credentialed providers merely because an environment variable exists unless current documented behavior already does so.

## E.4 Request routing behavior

For requests containing a mix of keyless and credentialed providers:

- keyless providers execute;
- missing-credential providers produce scoped skip telemetry;
- response remains successful if any applicable keyless route can execute;
- response may be marked degraded/partial;
- missing credentials must not erase keyless results;
- missing credentials must not be reported as provider failure;
- no-match from a keyless provider remains distinct from skipped credentialed providers.

For an explicit request containing only unavailable credentialed providers:

- return a truthful structured unavailable/degraded result according to existing tool contract;
- do not crash or fail server startup;
- do not silently substitute a provider when explicit provider selection forbids fallback;
- include a stable skip code such as `missing_api_key` or `credential_env_missing`.

## E.5 Profile behavior

Profiles are advisory routing preferences, not secret requirements.

Verify:

### Generic profile

Uses keyless default providers without credentials.

### Coding profile

Without credentials, uses available keyless web providers and local workspace when configured. Forge-native adapters are enhancements.

The coding profile must not require GitHub tokens to produce a response.

### Security profile

Without credentials, uses OSV/NVD/CISA KEV/RustSec and keyless web context as applicable. GitHub Advisory is optional.

### Research profile

Without credentials, uses keyless web and scholarly providers such as OpenAlex/Crossref as available. Brave API, Semantic Scholar key, or SearXNG are optional.

If the existing profile definitions prioritize unavailable credentialed providers, routing must skip them and continue deterministically with keyless providers.

## E.6 Specialized-tool behavior

Do not claim native behavior when only generic behavior is available.

Required distinctions:

```text
native forge adapter used
keyless public HTTP/local route used
generic web discovery used
provider capability unavailable
provider skipped because credential missing
```

`repo_search`, `repo_fetch`, and `repo_map` must remain truthful about provenance and mode.

A keyless result is acceptable. A falsely labeled native result is not.

## E.7 Provider status and health

`provider_status` should allow codegg to decide:

- server core is healthy;
- provider is optional;
- provider requires credentials;
- provider is currently routable;
- provider skip reason/code;
- keyless fallback is available for the workflow.

Prefer deriving workflow availability from existing server/tool capabilities and routable provider inventory. Add optional fields only if current data cannot distinguish core health from adapter availability.

Do not classify the entire server unhealthy because optional credentials are absent.

## E.8 Gate E deterministic tests

Use a scrubbed environment helper that removes every recognized credential variable for the duration of the test. Serialize environment-mutating tests to prevent races.

Test matrix:

1. no config file, no keys: state builds;
2. no config file, no keys: default provider list contains only keyless defaults;
3. no keys: `provider_status` succeeds;
4. no keys: credentialed providers report non-routable optional status;
5. no keys: keyless web search dispatch fixture succeeds;
6. no keys: bounded fetch fixture succeeds;
7. no keys: coding profile has a keyless execution path;
8. no keys: security profile has a keyless native advisory path;
9. no keys: research profile has a keyless path;
10. mixed providers: keyless result survives credentialed skip;
11. mixed providers: missing credential is a skip, not a provider failure;
12. explicit unavailable-only provider selection returns truthful scoped unavailability;
13. missing `GITHUB_TOKEN` does not fail startup;
14. missing `GITLAB_TOKEN` does not fail startup;
15. missing Gitea/Forgejo token does not fail startup;
16. missing `SOURCEGRAPH_API_KEY` does not fail startup;
17. missing `SEMANTIC_SCHOLAR_API_KEY` does not fail startup;
18. missing `BRAVE_API_KEY` does not fail startup;
19. optional SearXNG absence does not fail startup;
20. local workspace disabled does not fail startup;
21. no credential values appear in serialized status, warnings, or logs;
22. schema generation works in scrubbed environment;
23. codegg contract fixture can identify keyless-core readiness.

### Gate E acceptance criteria

- [ ] Clean no-config/no-key startup succeeds.
- [ ] Core keyless tools remain useful.
- [ ] Missing optional credentials are provider-scoped.
- [ ] Mixed routing preserves keyless results.
- [ ] Profiles have deterministic keyless paths.
- [ ] Explicit provider selection remains truthful and is not silently overridden.
- [ ] No credential value is serialized or logged.
- [ ] Server health is distinct from optional adapter availability.

---

# Gate F — Repair and Clarify User and codegg Documentation

## F.1 README

Add a prominent statement near the opening description:

```text
No API keys are required for the default installation. eggsearch ships with
keyless web, fetch, advisory, registry, and scholarly paths. Credentialed forge
and search adapters are optional enhancements.
```

Clarify the native forge workflow paragraph:

- credentials are maintainer test credentials;
- the workflow verifies optional adapters;
- users do not need these credentials;
- missing optional adapter evidence limits adapter-specific release claims but does not invalidate keyless-core release evidence.

## F.2 Provider setup guide

Reorganize or add a top-level matrix:

| Category | Examples | User requirement |
|---|---|---|
| Keyless defaults | DuckDuckGo, Startpage, Yahoo | none |
| Keyless specialist | OSV, NVD, CISA KEV, RustSec, registries, OpenAlex, Crossref | none |
| Optional configured endpoint | SearXNG, self-hosted forge base URL | operator configuration |
| Optional credentialed | GitHub/GitLab/Gitea code search, Sourcegraph, Brave API, Semantic Scholar | opt-in credential |
| Optional local | local workspace | configured local root |

State that all credentialed providers are disabled or non-routable unless explicitly configured.

## F.3 Configuration guide

Place **Keyless profiles first**.

Required examples:

### Keyless default

Existing shipped default.

### Keyless coding

```toml
[search]
default_providers = ["duckduckgo", "startpage", "yahoo"]

[local]
enabled = false
roots = []
```

Explain that local workspace may be enabled without credentials.

### Enhanced coding

Place GitHub/GitLab/Gitea/Sourcegraph examples after keyless coding and label them optional.

### Keyless security

Use OSV and other keyless advisory providers according to actual configuration model.

### Enhanced security

GitHub Advisory token is optional.

### Keyless research

Use keyless web plus OpenAlex/Crossref according to actual provider routing.

### Enhanced research

Brave API, SearXNG, and Semantic Scholar key are optional.

Do not present a token-heavy example as the default coding profile.

## F.4 codegg contract repair

Restore correct sections:

```text
8.3 Dirty State
8.4 File Classification Flags
8.5 Workspace ID
9. Retrieval Dimension State
10. Capability Discovery
11. Implementation Checklist
12. Schema Stability Rules
```

Restore file-classification and workspace-ID content that was removed.

Use JSON wire values in wire-format tables:

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

Add a keyless-core section instructing codegg:

1. do not require credentialed providers;
2. inspect provider status;
3. prefer native adapters when routable;
4. continue with keyless providers when optional adapters are unavailable;
5. preserve provenance/mode distinctions;
6. do not prompt the user for API keys merely to perform baseline search;
7. suggest optional credentials only when a user explicitly needs private/native provider capability.

## F.5 Tool matrix and agent workflows

Audit for statements that imply GitHub/GitLab/etc. credentials are required.

Each workflow should identify:

- keyless baseline path;
- optional enhanced path;
- truthful degradation behavior.

## F.6 Release documents

Split the release record into two sections:

### Core release verification

Required:

- exact `R`;
- clean no-secrets local gate;
- Linux CI with no optional secrets;
- macOS CI with no optional secrets;
- schema/docs/hardening/publish checks;
- benchmark artifact;
- evidence commit `E`.

### Optional adapter conformance

Per adapter table:

| Adapter | Status | Exact `R` | Run ID | Artifact/hash | Claim allowed |
|---|---|---|---|---|---|
| GitHub | unverified/verified | ... | ... | ... | yes/no |
| GitLab | unverified/verified | ... | ... | ... | yes/no |
| Codeberg | unverified/verified | ... | ... | ... | yes/no |
| Gitea/Forgejo | unverified/verified | ... | ... | ... | yes/no |

`unverified` must not mean broken. It means no release evidence was captured for that adapter.

## F.7 Documentation contract tests

Add static/document tests proving:

1. README says no API keys required for defaults;
2. README labels native adapter credentials maintainer-only/optional for users;
3. provider setup identifies keyless and optional credentialed categories;
4. config presents keyless coding/security/research examples before enhanced examples;
5. codegg contract contains restored section headings;
6. codegg state tables use snake_case wire values;
7. release record separates core and adapter evidence;
8. no docs state that every optional provider is required for core release;
9. no default install command includes credential setup;
10. tool matrix identifies a keyless path for each stable tool where applicable.

### Gate F acceptance criteria

- [ ] Keyless operation is prominent, not buried.
- [ ] Credentialed adapters are consistently labeled optional.
- [ ] codegg is instructed not to demand keys for baseline search.
- [ ] File classification and workspace-ID contract sections are restored.
- [ ] Wire-value tables use serialized values.
- [ ] Core and adapter release evidence are clearly separated.

---

# Gate G — Split Core Release Proof from Optional Adapter Conformance

## G.1 Required outcome

The core release can be promoted without third-party API keys, provided the mandatory keyless release matrix passes.

Optional adapters remain fail-closed when tested, but their absence does not block the core release.

## G.2 Preserve fail-closed adapter workflows

Do not weaken existing native adapter assertions:

- exact 40-character subject;
- exact checkout;
- required fixture for the adapter being tested;
- no generic fallback accepted as native;
- structured provider evidence;
- provenance pinning;
- bounded request/response evidence.

Change only the release-policy interpretation:

```text
adapter test failed or unavailable
  -> adapter-specific claim not verified
  -> core keyless release unaffected unless shared core code failed
```

## G.3 Prefer per-adapter manual inputs

If the current workflow requires all adapters in one invocation, choose the smallest safe change:

Option A, preferred if simple:

- add a workflow input selecting adapters;
- run only selected adapters;
- summary requires exact pass for every selected adapter;
- unselected adapters are explicitly `not_requested`, not pass or skip.

Option B, if workflow restructuring is risky:

- retain the all-adapter workflow as a comprehensive optional verification suite;
- add separate per-adapter workflows or documented commands;
- do not make the comprehensive suite a core promotion gate.

Do not mark absent credentials as passing evidence.

## G.4 Core no-secrets workflow

Add or extend CI to run a deterministic keyless-core job with all known credential variables unset.

Recommended shell preamble:

```bash
unset GITHUB_TOKEN || true
unset GH_TOKEN || true
unset GITLAB_TOKEN || true
unset GITEA_TOKEN || true
unset FORGEJO_TOKEN || true
unset SOURCEGRAPH_API_KEY || true
unset BRAVE_API_KEY || true
unset SEMANTIC_SCHOLAR_API_KEY || true
```

Account for CI-provided `GITHUB_TOKEN`: tests must use a child process with the variable removed or explicit test configuration that prevents accidental use.

Required keyless CI coverage:

- config/default tests;
- provider-status tests;
- mixed-routing tests;
- keyless profile tests;
- all stable tool contract fixtures;
- schema and documentation contracts;
- no secret leakage assertions.

## G.5 Core release evidence requirements

For exact final code subject `R`, capture:

1. clean source checkout identity;
2. Linux keyless CI run ID and job results;
3. macOS keyless CI run ID and job results;
4. local `make check` from clean checkout with credentials scrubbed;
5. standalone feature combinations required by the project, not merely implied by `--all-features`;
6. release build;
7. rustdoc;
8. package/publish dry-run from a clean exact-`R` checkout without `--allow-dirty`;
9. affected benchmark runtime artifact;
10. SHA-256 hashes for evidence artifacts.

## G.6 Optional adapter evidence requirements

For each adapter claimed verified:

- exact same `R`;
- adapter-specific run ID;
- exact provider result;
- evidence artifact ID;
- artifact hash;
- fixture identity;
- native mode proof;
- no fallback;
- credentials used only in CI secret storage;
- no credential value in artifacts.

Adapters with no evidence remain `unverified` and are omitted from verified-adapter claims.

## G.7 Evidence-only commit `E`

After all mandatory core evidence passes, create one evidence-only commit containing only approved paths such as:

```text
docs/release-verification.md
docs/release-evidence/**
release-evidence/**
```

`E` records:

- exact `R` and `E`;
- keyless Linux/macOS run IDs;
- local gate environment and command;
- benchmark artifact/hash;
- publish dry-run evidence;
- final core classification;
- optional adapter table with verified/unverified state;
- adapter artifacts only for adapters actually tested.

No code, tests, workflow, schema, config, benchmark definition, or contract changes may occur in `E`.

### Gate G acceptance criteria

- [ ] Core release proof runs with no optional credentials.
- [ ] Missing adapter credentials do not block core promotion.
- [ ] Adapter-specific claims require exact evidence.
- [ ] Unverified adapters are not called broken or verified.
- [ ] Fail-closed native tests remain strict.
- [ ] Clean publish dry-run succeeds without `--allow-dirty`.
- [ ] Final `E` is evidence-only.

---

# Gate H — Full Verification Matrix

## H.1 Focused unit and integration tests

Run all newly added tests for:

- state-authoritative summary classification;
- dimension-only job count omission;
- complete unique ID planning;
- ledger validation at assembly;
- scrubbed-environment startup;
- mixed provider routing;
- profile keyless fallbacks;
- codegg keyless contract;
- release-policy documentation.

## H.2 Feature matrix

Run standalone commands, not only aggregate coverage:

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features mock
cargo test --features pdf
```

If other feature combinations are release-required by current Makefile/CI, run them too.

## H.3 Static checks

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo doc --all-features --no-deps
cargo build --release --all-features
```

Run repository hardening, schema-corpus, documentation-contract, and publish-check targets.

## H.4 Clean package verification

From a clean exact-`R` checkout:

```bash
cargo package --allow-dirty=false
```

Use the repository's actual publish dry-run command if different.

Ignored local dependencies or generated directories must not force `--allow-dirty`. Use a clean worktree or clean checkout.

## H.5 Benchmarks

Run affected benchmarks for:

- attempt ledger validation;
- retrieval summary construction;
- native advisory identifier planning near cap;
- provider-operation budget fanout;
- mixed keyless/credentialed routing diagnostics;
- provider-status generation.

Benchmarks are regression indicators, not zero-allocation or zero-growth proofs.

## H.6 Privacy checks

Verify output contains no:

- API key values;
- environment variable values;
- authorization headers;
- raw private repository URLs with embedded credentials;
- raw security query content in operation IDs;
- provider secret configuration.

### Gate H acceptance criteria

- [ ] All focused tests pass.
- [ ] All standalone feature combinations pass.
- [ ] Formatting, clippy, docs, schema, hardening, release build, and clean package checks pass.
- [ ] Keyless CI passes with credentials scrubbed.
- [ ] Benchmarks are captured for exact `R`.
- [ ] No secrets appear in output or artifacts.

---

## 6. Likely Files to Inspect

This list is directional. Do not modify every file unless required.

### Retrieval semantics

- `src/core/retrieval_status.rs`
- `src/core/evidence_postprocess.rs`
- `src/meta/adapter.rs`
- `src/meta/dispatch.rs`
- `src/meta/security_search.rs`

### Provider configuration and routing

- provider inventory/config modules under `src/config/` or equivalent
- provider diagnostics/status modules
- profile/routing modules
- server state construction
- CLI doctor implementation

### MCP surfaces

- `src/mcp/tools.rs`
- response/schema types used by `provider_status`

### Tests

- `tests/retrieval_attempt_ledger.rs`
- `tests/property_retrieval.rs`
- `tests/codegg_evidence_contract.rs`
- `tests/evidence_integration.rs`
- `tests/integration.rs`
- `tests/static_guards.rs`
- provider/profile/config test modules
- native security attempt tests

### Documentation

- `README.md`
- `docs/provider-setup.md`
- `docs/config.md`
- `docs/tool-matrix.md`
- `docs/agent-workflows.md`
- `docs/architecture/codegg-contract.md`
- `docs/release-checklist.md`
- `docs/release-verification.md`
- `AGENTS.md`
- `skills/eggsearch-architecture/SKILL.md`

### CI and release evidence

- `.github/workflows/ci.yml` or current CI files
- `.github/workflows/native-forge-smoke.yml`
- Makefile/release scripts
- benchmark definitions

---

## 7. Required Test Scenarios in Concrete Form

### Scenario 1 — default keyless startup

Environment:

```text
no config file
all credential variables absent
```

Expected:

```text
server builds and starts
default providers are keyless
provider_status succeeds
no fatal credential error
```

### Scenario 2 — mixed coding providers without keys

Requested providers:

```text
github_code
duckduckgo
startpage
```

Environment:

```text
GITHUB_TOKEN absent
```

Expected:

```text
github_code -> scoped missing credential skip
duckduckgo/startpage -> execute
request -> successful or truthful zero-result response
response -> degraded/partial telemetry allowed
whole request -> not failed solely because GitHub token is absent
```

### Scenario 3 — explicit unavailable-only provider

Requested providers:

```text
github_code
```

Environment:

```text
GITHUB_TOKEN absent
```

Expected:

```text
no server crash
no silent provider substitution
structured non-routable result
skip_code = missing_api_key or credential_env_missing
```

### Scenario 4 — keyless security

Environment:

```text
all credentials absent
```

Expected providers as configured/applicable:

```text
OSV
NVD
CISA KEV
RustSec
keyless web context
```

Expected:

```text
security_search handles request
GitHub Advisory absence is optional
provider-scoped telemetry remains truthful
```

### Scenario 5 — identifier cap

Input:

```text
40 unique vulnerability IDs
```

Expected:

```text
planned = 40
scheduled = 32
omitted = 8
all 32 scheduled operations receive provider dispatch or policy-skip attempts
```

### Scenario 6 — state contradiction

Dimension:

```text
state = NotApplicable
absence_kind = NotApplicable
```

Expected:

```text
not attempted
not complete
not_applicable_count += 1
```

Dimension:

```text
state = Satisfied
absence_kind = NotApplicable
```

Expected:

```text
attempted
complete
not_applicable_count unchanged
```

### Scenario 7 — optional adapter evidence absent

Release state:

```text
keyless Linux/macOS gates pass
benchmark and package evidence pass
GitLab adapter credentials unavailable
```

Expected:

```text
core classification may be release verified
GitLab adapter status = unverified
no claim that GitLab adapter is release verified
core release is not blocked
```

---

## 8. Recommended Commit Sequence

Use small, reviewable commits.

### Commit 1 — retrieval state and count correctness

Scope:

- state-first summary classification;
- correct non-applicability counts;
- dimension-only job counters set to `None`;
- focused tests.

Suggested message:

```text
fix: make retrieval summaries state-authoritative
```

### Commit 2 — complete identifier planning and ledger checks

Scope:

- complete deduplicated ID plan;
- exact cap telemetry;
- central debug ledger validation;
- focused tests.

Suggested message:

```text
fix: close advisory planning and ledger invariants
```

### Commit 3 — keyless-core runtime contract

Scope:

- no-config/no-key behavior tests;
- mixed routing tests;
- profile keyless fallbacks;
- provider-status/core-health distinction;
- minimal runtime corrections.

Suggested message:

```text
fix: enforce keyless core operation across provider routing
```

### Commit 4 — documentation and codegg contract

Scope:

- README;
- provider/config guides;
- codegg contract repair;
- keyless/enhanced examples;
- documentation tests.

Suggested message:

```text
docs: define keyless core and optional adapter contract
```

### Commit 5 — release workflow policy split

Scope:

- core no-secrets CI gate;
- optional adapter conformance policy/workflow adjustments;
- release docs remain provisional.

Suggested message:

```text
ci: separate keyless core proof from adapter conformance
```

### Commit 6 — verification-only corrections before `R`

Scope:

- only defects found by full matrix;
- no unrelated cleanup.

Suggested message:

```text
test: close final keyless release verification gaps
```

### Select `R`

After all code, tests, docs, workflows, benchmarks, and release scripts are final, select the exact code-bearing commit as `R`.

### Commit `E`

After mandatory core evidence passes, create evidence-only `E`.

Suggested message:

```text
docs(release): record keyless core release evidence
```

---

## 9. Rollback Boundaries

Each gate must be independently revertible.

- Gate A/B rollback: retrieval summary semantics only.
- Gate C rollback: identifier planning only.
- Gate D rollback: debug validation only.
- Gate E rollback: keyless routing/status corrections only.
- Gate F rollback: documentation only.
- Gate G rollback: CI/release policy only.

Do not combine runtime routing changes with documentation or workflow changes in one commit.

---

## 10. Reviewer Checklist

### Retrieval correctness

- [ ] State is authoritative when present.
- [ ] Success is not counted as non-applicable.
- [ ] Non-applicable roles are neither attempted nor complete.
- [ ] Dimension-only summaries do not invent jobs.
- [ ] Attempt partition equations hold.
- [ ] Full unique ID plan precedes caps.
- [ ] Omitted ID count is exact.
- [ ] Complete ledgers are validated in debug/test builds.

### Keyless core

- [ ] No config and no keys starts successfully.
- [ ] Default provider list is keyless.
- [ ] Missing optional credentials are scoped skips.
- [ ] Mixed requests preserve keyless results.
- [ ] Coding profile has a keyless path.
- [ ] Security profile has a keyless path.
- [ ] Research profile has a keyless path.
- [ ] Provider status distinguishes server health from adapter availability.
- [ ] Codegg is not instructed to demand keys for baseline use.

### Documentation

- [ ] README prominently says no API keys are required.
- [ ] Token-based examples are labeled enhanced/optional.
- [ ] codegg contract sections are restored.
- [ ] State wire values are snake_case.
- [ ] Core release proof is separate from adapter evidence.

### Release evidence

- [ ] New `R` is selected after all corrections.
- [ ] Linux and macOS keyless CI run on exact `R`.
- [ ] Clean package/publish dry-run passes without `--allow-dirty`.
- [ ] Benchmark artifact is tied to exact `R`.
- [ ] Optional adapter claims match available evidence.
- [ ] `E` changes only evidence/documentation paths.

---

## 11. Final Acceptance Matrix

| Area | Required result |
|---|---|
| Installation | no API key setup required |
| Startup | succeeds with no config and no credential env vars |
| Default search | keyless providers execute |
| Fetch | explicit bounded fetch works without keys |
| Coding search | useful keyless path; native forge APIs optional |
| Security search | keyless advisory path; GitHub Advisory optional |
| Research search | keyless web/scholarly path; paid/keyed sources optional |
| Missing credentials | provider-scoped skip, never global unhealthy state |
| Explicit unavailable provider | truthful structured unavailability, no silent fallback |
| Retrieval state | state-authoritative aggregation |
| Job counts | attempts only |
| Dimension counts | dimensions only |
| Identifier cap | complete plan and exact omitted count |
| Ledger | provider/operation/role uniqueness checked |
| codegg contract | correct sections, wire values, keyless guidance |
| Core release proof | no-secrets Linux/macOS/local/package/benchmark evidence |
| Adapter proof | per-adapter, optional, fail-closed, claim-scoped |
| Evidence commit | documentation/evidence only |

---

## 12. Definition of Done

This line of work is complete only when all statements below are true:

1. A clean installation starts and operates usefully without API keys.
2. Missing optional credentials do not make the server globally unhealthy.
3. Every primary workflow has a documented and tested keyless path.
4. Credentialed providers are clearly optional enhancements.
5. codegg does not require or prompt for keys for baseline search.
6. Retrieval summary counts use authoritative state and correct accounting levels.
7. Complete identifier planning produces exact cap telemetry.
8. Production attempt ledgers are checked at complete assembly boundaries.
9. The codegg contract is structurally and semantically correct.
10. Core release evidence runs with credentials scrubbed.
11. Optional adapters are claimed verified only when exact-`R` evidence exists.
12. Missing optional adapter evidence does not block the keyless core release.
13. A fresh immutable code-bearing `R` is selected after all corrections.
14. Linux and macOS core gates pass on exact `R`.
15. Clean package and benchmark evidence is captured for exact `R`.
16. Evidence-only `E` records core evidence and optional adapter status without code changes.

Until all mandatory core items are complete, retain the classification **provisional release candidate**.
