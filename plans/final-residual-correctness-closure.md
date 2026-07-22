# Final Residual Correctness Closure

**Status:** implementation handoff

**Baseline reviewed:** `07626669f2cf39bcc62d1259677038ed94d47d09`

**Purpose:** close the remaining correctness, containment, evidence-semantics, and verification gaps left after the post-closure corrective pass. This is a narrowly scoped closure plan. It must not introduce new search backends, new user-facing workflows, or unrelated architecture changes.

---

## 1. Outcome

At completion, Eggsearch must be able to make all of the following claims without qualification:

1. Git subprocess stdout and stderr are drained concurrently, bounded during read, and any timeout or byte-cap breach causes immediate process-group termination and reaping.
2. A forge-tree operation has one enforceable aggregate response-byte budget covering tree pages and all auxiliary metadata, commit-resolution, and fallback requests.
3. Local workspace reads cannot escape the configured root, including when symlink following is enabled.
4. Every card returned by `web_search`, `repo_search`, `research_search`, and `security_search` has a materialized evidence role.
5. Retrieval coverage and summaries are derived from actual provider/subquery attempts, including successful zero-result retrievals, rate limits, policy skips, deadlines, and truncation.
6. Evidence conflicts compare values from distinct sources for the same canonical entity and cannot create same-card or cross-entity false positives.
7. Explicit request workflow and profile selections drive the workflow coverage model.
8. The release verification record describes the actual final commit and only claims evidence that was executed and retained.
9. Native forge adapters and the supported platform matrix have reproducible verification evidence.

The final classification remains **provisional release candidate** until every release gate in this plan is satisfied.

---

## 2. Scope and non-goals

### In scope

- `src/meta/local_inventory_cache.rs`
- `src/meta/safe_open.rs`
- `src/meta/forge_adapter.rs`
- `src/meta/adapter.rs`
- `src/meta/security_search.rs`
- `src/meta/dispatch.rs` and related dispatch-result types
- `src/core/retrieval_status.rs`
- `src/core/evidence_postprocess.rs`
- `src/core/workflow_coverage.rs`
- `src/core/conflict.rs`
- MCP serialization and response fixtures
- static, property, adversarial, integration, live-smoke, and CI verification
- `docs/release-verification.md` and directly related safety/architecture documentation

### Non-goals

- adding search providers
- changing ranking algorithms unrelated to evidence roles
- redesigning public response structures incompatibly
- adding a database or persistent retrieval ledger
- broad provider-health redesign
- performance tuning outside the affected paths
- resolving DNS rebinding beyond the documented preflight policy unless a bounded, testable connection-pinning implementation is deliberately included

All response changes must remain additive and backward compatible unless a current field is demonstrably incorrect and the correction is documented.

---

## 3. Implementation invariants

These invariants apply to every workstream:

- Never infer success from source-code pattern checks alone.
- A timeout, truncation, cap breach, or unsupported safety mode must be explicit in structured output.
- Safety failures must fail closed; they must not silently downgrade to a less safe path.
- Tests must distinguish the specific failure mode being verified.
- Do not describe a request as queried when it was not dispatched.
- Do not describe zero results as a provider skip.
- Do not attribute a provider failure to an evidence role without request/subquery context.
- Do not compare evidence values unless they come from distinct source identities for the same canonical entity.
- Do not mark release criteria complete until the final commit itself has verification evidence.

---

# Workstream A — Immediate subprocess termination on output-cap breach

## A.1 Problem

The current bounded Git runner drains stdout and stderr concurrently, but reader threads only stop reading when a cap is exceeded. The process group is killed by the timeout watchdog, not directly by the cap breach. Closing a pipe may induce `SIGPIPE`, but that is incidental and does not guarantee prompt termination for children that ignore `SIGPIPE`, stop writing to that stream, retain other descriptors, or continue CPU work.

## A.2 Required design

Replace the current loosely coordinated reader/watchdog flow with a shared termination controller.

Recommended internal components:

```rust
enum TerminationTrigger {
    Timeout,
    StdoutLimitExceeded,
    StderrLimitExceeded,
}

struct ProcessTerminationController {
    child_pgid: i32,
    trigger: AtomicU8,
    kill_sent: AtomicBool,
}
```

The exact representation may differ, but it must provide:

- a single atomic first-writer-wins termination reason;
- an idempotent `terminate_process_group(trigger)` operation;
- immediate `SIGKILL` or a bounded TERM-then-KILL sequence;
- no PID-reuse race after the child has been reaped;
- deterministic mapping to `CommandTermination`;
- no detached watchdog thread after return.

## A.3 Execution sequence

1. Spawn the child in its own process group/session.
2. Capture stdout and stderr handles.
3. Start stdout and stderr readers concurrently.
4. Each reader:
   - reads at most the configured cap;
   - stores only bytes up to the cap;
   - when an additional byte is observed, atomically records its cap-breach reason;
   - immediately invokes process-group termination;
   - closes its pipe and exits.
5. Start a timeout watcher that invokes the same controller.
6. Wait for the child and reap it exactly once.
7. Signal all helper threads that the process is complete.
8. Join all helper threads.
9. Select the result reason from the controller, not from `SIGKILL` inference alone.

## A.4 Result semantics

`BoundedCommandResult` must expose:

- `termination: CommandTermination`
- `status: Option<ExitStatus>`
- `stdout_truncated`
- `stderr_truncated`
- `timed_out`
- bounded stdout/stderr bytes
- optional spawn/read/wait diagnostic class if existing APIs allow additive fields

Rules:

- `timed_out` is true only for a timeout trigger.
- `StdoutLimitExceeded` and `StderrLimitExceeded` remain distinguishable even though the child receives `SIGKILL`.
- If both streams breach, preserve the first trigger and mark both truncation booleans where observed.
- A normal external signal with no Eggsearch trigger maps to `Signaled`.
- Spawn failure maps to `SpawnFailed` with no watchdog or reader threads started.

## A.5 Apply everywhere

Use one implementation for:

- normal bounded Git commands;
- inventory-specific capped Git commands;
- HEAD/gitdir/worktree resolution;
- status hashing;
- ignore checks;
- tracked and untracked enumeration;
- any future local Git invocation.

Remove duplicated runner logic or make both wrappers delegate to a single configurable primitive.

## A.6 Tests

Add deterministic tests for:

1. stdout cap breach with a child that traps or ignores `SIGPIPE` and continues looping;
2. stderr cap breach with the same behavior;
3. simultaneous stdout/stderr saturation;
4. cap breach returns well before the configured timeout;
5. timeout returns `TimedOut`, not a cap reason;
6. a child spawning a grandchild is fully terminated through the process group;
7. no zombie remains after timeout or cap breach;
8. repeated rapid terminations do not kill an unrelated reused PID;
9. nonzero normal exit preserves diagnostics and maps to `Exited`;
10. spawn failure creates no helper-thread leak;
11. invalid UTF-8 remains byte-preserving;
12. inventory fallback occurs on either cap reason.

Linux-specific process inspection may use `/proc` in tests. macOS tests should use process-group signaling and bounded completion assertions without `/proc` assumptions.

## A.7 Acceptance criteria

- A cap-breach test configured with a 30-second timeout returns within a tight bounded interval, such as under 2 seconds.
- The recorded termination reason is the cap reason, not timeout.
- The child and descendants are reaped or demonstrably gone.
- No production Git command path bypasses the unified runner.

---

# Workstream B — True operation-wide forge byte budget

## B.1 Problem

Current pagination checks the aggregate counter only before a page and still reads each response with the full per-response limit. The operation may overshoot the aggregate budget. Commit resolution, default-branch lookup, and fallback requests also use independent counters, so the budget is not operation-wide.

## B.2 Budget contract

`ForgeReadBudget` must represent one complete `fetch_tree` operation and be passed to every body-reading helper invoked by that operation.

Required fields:

```rust
struct ForgeReadBudget {
    per_response_limit: usize,
    aggregate_limit: usize,
    aggregate_observed: usize,
    exhausted: bool,
}
```

Required behavior:

- `remaining()` returns the exact unread aggregate allowance.
- The cap for the next response is `min(per_response_limit, remaining())`.
- A response cannot increase `aggregate_observed` above `aggregate_limit`.
- Content-Length larger than the effective remaining cap is rejected before streaming.
- Chunked bodies stop as soon as the effective cap would be exceeded.
- The operation records which request class exhausted the budget.

## B.3 Shared-budget threading

Pass `&mut ForgeReadBudget` through:

- GitHub commit resolution;
- GitHub default-branch lookup;
- GitHub tree retrieval;
- GitHub Contents fallback;
- GitLab commit resolution;
- GitLab project/default-branch metadata;
- every GitLab tree page;
- Gitea/Forgejo/Codeberg commit resolution;
- repository metadata/default-branch requests;
- every forge tree page;
- bounded error-body previews where those bytes are part of the operation budget, or explicitly document and separately cap them if excluded.

Do not create local `total_bytes = 0` counters inside these helpers.

## B.4 Read API

Prefer a helper shaped like:

```rust
async fn read_with_budget(
    response: reqwest::Response,
    budget: &mut ForgeReadBudget,
    request_kind: ForgeRequestKind,
) -> Result<BoundedBody, ForgeReadError>
```

`ForgeReadError` should distinguish:

- per-response limit exceeded;
- aggregate budget exhausted;
- declared Content-Length too large;
- stream read failure;
- invalid UTF-8 where text is required.

Do not collapse these into the generic string `response_too_large` internally. Public-facing messages may remain stable while telemetry receives the structured reason.

## B.5 Pagination behavior

Before dispatching another page:

- stop if no budget remains;
- stop if entry or page caps have been reached;
- do not make a request that cannot return at least a minimal valid response under the remaining budget;
- produce `aggregate_budget_exhausted` warning and truncation telemetry;
- retain already parsed entries.

If an auxiliary request exhausts the budget before tree retrieval:

- do not silently reset the budget;
- return a structured bounded-operation error or a partial response only where the response contract explicitly supports it;
- never proceed with an unpinned mutable ref merely because commit resolution consumed the budget unless that downgrade is explicit in warnings and permitted by the request contract.

## B.6 Telemetry

Record:

- aggregate limit;
- aggregate observed bytes;
- remaining bytes;
- request count;
- request kind that exhausted the budget;
- whether truncation was provider-originated or Eggsearch-budget-originated;
- whether immutable commit resolution completed before exhaustion.

## B.7 Tests

Use mock servers to cover:

1. two pages whose combined sizes exactly equal the aggregate limit;
2. a second page larger than the remaining budget;
3. chunked response crossing the remaining budget by one byte;
4. Content-Length exceeding the remaining budget;
5. commit resolution consuming part of the budget before tree retrieval;
6. metadata plus multiple pages plus fallback sharing one budget;
7. fallback skipped when no budget remains;
8. error previews remain separately bounded and correctly accounted/documented;
9. telemetry never reports observed bytes above the aggregate limit;
10. every supported forge family uses the same budget semantics.

## B.8 Acceptance criteria

- No operation consumes or retains more body bytes than the configured aggregate limit.
- Supporting requests cannot reset or bypass the budget.
- All pagination paths stop before dispatch when exhausted.
- Tests intentionally use different response sizes and verify exact accounting.

---

# Workstream C — Contained symlink-following semantics

## C.1 Problem

The default `follow_symlinks=false` path is descriptor-relative and safe. When `follow_symlinks=true`, final-component containment is not guaranteed and non-Linux Unix platforms lack a reliable beneath-root check.

## C.2 Required policy

Define the supported semantics explicitly:

- `follow_symlinks=false`: reject all symlink components.
- `follow_symlinks=true`: permit symlinks only when the kernel primitive can prove the resolved target remains beneath the configured root.
- If the platform cannot prove containment race-safely, return an explicit unsupported-safety-mode error. Do not fall back to path canonicalization followed by pathname open.

## C.3 Linux implementation

For `follow_symlinks=true`, use `openat2` from the root descriptor with:

- `RESOLVE_BENEATH`
- `RESOLVE_NO_MAGICLINKS`
- omit `RESOLVE_NO_SYMLINKS`
- appropriate `O_RDONLY`, `O_CLOEXEC`, and final-type handling

Open the complete relative path in one descriptor-relative `openat2` operation or maintain an equally strong component walk. The kernel must enforce beneath-root resolution for the final target.

If `openat2` is unavailable:

- return `SafeOpenError::SafeSymlinkFollowingUnsupported` for `follow_symlinks=true`;
- keep no-follow mode available through `openat` plus `O_NOFOLLOW`.

## C.4 Other Unix platforms

Unless an equivalent race-safe primitive is implemented and tested, return the same unsupported error for `follow_symlinks=true`.

Do not claim contained symlink following on macOS based solely on `canonicalize`, `F_GETPATH`, or `/dev/fd` checks followed by a second pathname open.

## C.5 Windows

Document and test the Windows policy separately. Use handle-based open and reparse-point controls if implemented. Otherwise, reject `follow_symlinks=true` and retain the safest available no-follow behavior.

## C.6 Error and telemetry behavior

Add a distinct error variant for unsupported safe symlink following. Surface it as:

- a structured local retrieval warning;
- a failed read, not a skipped file silently treated as absent;
- documentation explaining the platform limitation.

## C.7 Tests

1. in-root symlink to in-root file succeeds when supported;
2. in-root symlink to outside file is rejected;
3. intermediate symlink escaping root is rejected;
4. chained symlinks that eventually escape are rejected;
5. symlink swap race cannot change the resolved target after open;
6. magic-link paths are rejected on Linux;
7. unsupported platforms return the explicit error;
8. no-follow mode remains unchanged;
9. final descriptor is a regular file and size limits are checked through `fstat`.

## C.8 Acceptance criteria

- No configuration permits workspace escape.
- There is no path-based fallback for a mode advertised as race-resistant.
- Platform support is explicit and truthful.

---

# Workstream D — Materialize evidence roles on security response cards

## D.1 Problem

`security_search` materializes evidence roles on a cloned flat vector used for summaries, then returns original groups. Serialized group cards may still have `evidence_role: null`.

## D.2 Implementation

Match the corrected repo and research paths:

1. make security groups mutable;
2. iterate through every group and call `materialize_evidence_roles(&mut group.results)`;
3. only then build the flattened `all_cards` view;
4. compute coverage, retrieval summaries, conflicts, source-quality summaries, and next actions from that same materialized card population;
5. serialize the mutated groups.

Audit every security response construction path, including:

- identifier-based advisory search;
- package-based search;
- generic security search;
- no-advisory and provider-failure paths;
- defensive-guidance and vendor-advisory groups.

## D.3 Tests

- end-to-end MCP fixture asserting every serialized security group card has an evidence role;
- authoritative advisory maps correctly;
- vendor guidance maps correctly;
- defensive configuration guidance maps correctly;
- community/news results do not become primary implementation by default;
- summaries equal the role counts observed in serialized groups.

## D.4 Acceptance criteria

- No returned `SourceCard` from supported search tools has a missing evidence role.
- Summary counts are derived from the exact serialized cards.

---

# Workstream E — Attempt-derived retrieval semantics

## E.1 Problem

Current coverage failures and retrieval summaries are reconstructed from provider IDs, provider failures, and card presence. This loses subquery intent and conflates zero-result success, policy skip, capability skip, and not-dispatched states. `build_retrieval_failures` currently maps all failures using the synthetic label `source`, which usually assigns `PrimaryImplementation` regardless of the failed retrieval’s real purpose.

## E.2 Canonical attempt record

Make `RetrievalAttempt` the source of truth for search execution semantics.

Each dispatched provider/subquery job must emit one record containing:

- provider ID;
- subquery ID/label;
- intended evidence roles;
- outcome;
- result count;
- optional error class;
- whether a global deadline interrupted it;
- whether result or response caps truncated it;
- optional query fingerprint or bounded query label;
- duration where telemetry already supports it.

## E.3 Outcome mapping

Use these outcomes precisely:

- `SuccessWithResults`
- `SuccessZeroResults`
- `Failed`
- `TimedOut`
- `RateLimited`
- `SkippedByPolicy`
- `SkippedCapabilityUnavailable`
- `NotApplicable`
- `InterruptedByDeadline`
- `TruncatedAfterPartialSuccess`

Rules:

- a job that was never selected is not `SuccessZeroResults`;
- a selected provider returning an empty list is `SuccessZeroResults`;
- a global deadline cancellation is not an ordinary provider timeout;
- rate limiting remains distinct through response serialization;
- partial results plus truncation remain distinct from complete success.

## E.4 Dispatch integration

Extend dispatch output to retain attempts alongside raw results and failures.

For every planned job:

1. derive intended roles from the actual subquery label and workflow context;
2. initialize an attempt identity;
3. update it from the provider result or error;
4. synthesize deadline-interrupted attempts for jobs still pending at global deadline;
5. retain zero-result successes;
6. retain policy and capability skips when selection logic makes those decisions.

Avoid reconstructing attempts later from aggregated cards because aggregation loses provider/subquery boundaries.

## E.5 Intended-role mapping

Replace calls that pass a universal `"source"` label.

The mapping function must receive:

- actual provider ID;
- actual planned subquery label;
- tool/workflow context where needed.

Examples:

- security advisory subquery → `AuthoritativeSecurityAdvisory`;
- vendor subquery → `VendorSecurityGuidance`;
- defensive subquery → `ConfigurationOrFeatureGate`;
- docs subquery → `OfficialDocumentation`;
- issues subquery → `IssueOrIncidentDiscussion`;
- release subquery → `ReleaseNoteOrChangelog`;
- source/code subquery → `PrimaryImplementation`;
- benchmark subquery → `BenchmarkOrPerformanceEvidence`.

A single attempt may intend multiple roles. Failure conversion must preserve all affected required roles rather than choosing only the first role when that would hide a coverage failure.

## E.6 Coverage integration

Convert attempt failures into `RetrievalFailure` records with exact roles and outcomes.

Coverage status rules:

- missing required role plus failed/timed-out/rate-limited/deadline-interrupted intended retrieval → `IndeterminateDueToFailures`;
- missing required role after completed zero-result retrieval → `Insufficient` with a no-matching-evidence reason;
- required roles found despite unrelated provider failures → do not downgrade coverage unnecessarily;
- recommended-role failures may reduce completion confidence and generate next actions without automatically making required coverage indeterminate.

## E.7 Retrieval summary generation

Replace `build_retrieval_summary_for_search(providers_failed, provider_ids, cards)` with an attempt-derived summary.

Each dimension should preserve:

- provider ID;
- subquery/role intent;
- evidence role;
- outcome/absence kind;
- result count;
- bounded message.

Do not label every successful provider as `PrimaryImplementation`. Derive the role from its attempts and/or returned cards for that attempt.

## E.8 Next actions

Generate actions from coverage gaps and attempt history:

- do not repeat a failed provider/query combination unchanged;
- suggest an alternate provider, narrower scope, or different query label where possible;
- use schema-valid templates;
- preserve gap-driven actions through the MCP layer;
- fall back to recipe actions only when no meaningful coverage-driven action exists.

## E.9 Tests

Create table-driven and end-to-end tests for:

1. provider success with results;
2. provider success with zero results;
3. timeout;
4. global deadline interruption;
5. rate limit;
6. policy skip;
7. capability skip;
8. partial success with truncation;
9. one provider serving multiple subqueries with different roles;
10. required advisory retrieval failure producing indeterminate security coverage;
11. unrelated docs failure not making implementation coverage indeterminate;
12. serialized retrieval summary matching attempt records;
13. next-action templates validating through the tool registry;
14. codegg fixture consuming the additive attempt-derived output.

## E.10 Acceptance criteria

- No production coverage or retrieval summary is inferred solely from provider/card presence.
- Zero-result success and policy skip are distinguishable.
- Failed required-role retrievals are assigned to their actual roles.

---

# Workstream F — Distinct-source conflict detection

## F.1 Problem

Vulnerability conflict detection pools values from all cards and compares the first two values. Multiple valid patched versions or dates from one card can be mistaken for disagreement between sources.

## F.2 Data model

Preserve provenance while collecting comparable values:

```rust
struct SourcedValue<'a> {
    source_id: &'a str,
    provider_ids: &'a [String],
    value: &'a str,
}
```

For set-valued fields such as patched versions, compare normalized sets per card rather than individual elements pooled globally.

## F.3 Comparison rules

A conflict requires:

- the same canonical entity key;
- at least two distinct source IDs;
- a field that is semantically comparable;
- normalized values or sets that materially differ.

Do not create a conflict when:

- two values originate from the same card;
- duplicate providers contributed to one aggregated card;
- one patched-version set is a reordered equivalent of another;
- values describe different packages under the same advisory without package scoping;
- dates represent different date fields;
- mutable and pinned repository cards refer to different repositories.

## F.4 Vulnerability scoping

Use a compound key where available:

- canonical advisory ID;
- ecosystem;
- package;
- compared field.

If package identity is absent, only compare advisory-level fields that are valid at advisory scope.

## F.5 Repository conflicts

Retain the repository-scoped mutable-versus-pinned fix and strengthen keys with normalized host/owner/repo identity where available. Do not merge same owner/repo names across different hosts.

## F.6 Benchmark conflicts

Only compare numbers when benchmark name, metric, unit, version/model identity, and evaluation setup are sufficiently aligned. Otherwise emit a non-comparable caveat, not a numeric conflict.

## F.7 Tests

- one card with two patched versions produces no conflict;
- two cards with the same patched-version set in different order produce no conflict;
- two cards with genuinely different sets produce a conflict;
- same CVE but different package produces no package-range conflict;
- same owner/repo on different hosts produces no repository conflict;
- duplicate aggregated provider contributions do not count as distinct sources;
- conflict IDs are stable and order-independent;
- property tests generate grouped cards and assert no cross-entity conflict.

## F.8 Acceptance criteria

- Every emitted conflict references at least two distinct source IDs.
- Every compared value retains source provenance.
- Same-card and cross-entity false positives are covered by regression tests.

---

# Workstream G — Honor explicit workflow and profile selection

## G.1 Problem

`repo_search` currently resolves workflow coverage with `profile=None`; `research_search` mainly derives its model from broad research domain and may ignore explicit workflow selection.

## G.2 Resolution precedence

Define and document deterministic precedence:

1. explicit request workflow;
2. explicit request profile;
3. exact-error mode;
4. research domain;
5. tool default.

Exact-error mode may override only where the public contract explicitly states it is a mode rather than a profile hint. Avoid ambiguous precedence by encoding this in one resolver.

## G.3 Resolver API

Replace loosely typed strings where feasible with a context object:

```rust
struct WorkflowResolutionContext<'a> {
    tool: ToolKind,
    workflow: Option<WorkflowKind>,
    profile: Option<ProfileKind>,
    research_domain: Option<ResearchDomain>,
    exact_error: bool,
}
```

If public request enums already exist, map them directly. Avoid duplicating string literals across adapters.

## G.4 Required mappings

Ensure explicit mappings for at least:

- API comprehension;
- repository architecture;
- error investigation;
- version migration;
- security review;
- dependency evaluation;
- performance investigation;
- comparative research;
- pre-change evidence;
- post-change review.

## G.5 Response telemetry

Return or retain:

- selected workflow ID;
- resolution source (`explicit_workflow`, `profile`, `mode`, `domain`, `default`);
- required/recommended/optional roles.

This may be included in existing coverage output rather than a new top-level field.

## G.6 Tests

- every explicit workflow maps to the expected model;
- explicit workflow wins over broad domain;
- profile is honored by repo search;
- exact-error behavior is deterministic;
- omitted fields preserve current defaults;
- serialization fixtures remain backward compatible;
- coverage expectations change appropriately between workflows for identical cards.

## G.7 Acceptance criteria

- No applicable request silently discards its explicit workflow or profile.
- Workflow selection is tested through MCP request handling, not only the resolver unit.

---

# Workstream H — Verification, CI, and release evidence

## H.1 Verification record reset

Do not incrementally edit stale numbers. Regenerate `docs/release-verification.md` after the final implementation commit.

The record must include:

- exact final commit SHA;
- exact timestamp;
- `rustc -Vv` output or precise host/toolchain triple;
- operating system and architecture without contradiction;
- commands executed;
- pass/fail result and test count captured from that run;
- CI run identifiers or durable links where available;
- explicit distinction between local evidence and remote CI evidence;
- native-provider versus fallback smoke mode;
- known residual limitations.

## H.2 Native forge smoke suite

Add direct native-adapter smoke tests that cannot pass through generic fallback mode.

Targets:

- GitHub public repository;
- GitHub non-default slash-containing ref where stable;
- GitLab public repository;
- Codeberg public repository;
- Gitea or Forgejo public instance;
- commit-pinned entry URL verification for each provider family.

Assertions:

- response mode/provider ID confirms native adapter;
- resolved commit SHA is immutable and structurally valid;
- tree entries are returned;
- URLs use the resolved commit where supported;
- byte telemetry is present and within limits;
- no credential is required for public cases where provider policy permits unauthenticated access.

If external rate limits make a test non-deterministic, classify it as a scheduled/manual smoke with recorded evidence rather than weakening assertions to accept fallback.

## H.3 Mock contract suite

Retain deterministic local mock-server tests for every native adapter. These are release-blocking and must cover:

- commit/tree/blob identity separation;
- slash ref encoding;
- aggregate budget across auxiliary and paginated requests;
- error and redirect handling;
- credential-origin policy;
- provider truncation and Eggsearch truncation.

## H.4 Platform CI

Minimum release matrix:

- Ubuntu Linux, stable pinned MSRV/toolchain policy;
- macOS;
- Windows for supported non-Unix paths or an explicit documented exclusion if the crate does not claim Windows support.

Linux CI must exercise `openat2` behavior where the runner kernel supports it. Add a test mode to force the `openat` fallback for no-follow semantics.

## H.5 Fuzz/property coverage

Add or update targets for:

- aggregate forge budget state transitions;
- subprocess termination-controller races;
- sourced conflict grouping;
- retrieval-attempt summarization;
- workflow-resolution precedence;
- symlink path policy parsing.

The CI fuzz matrix must include every declared release-blocking target, including `bounded_response_reader` if retained.

## H.6 Performance and memory evidence

Replace unrelated serialization-only claims with affected-path measurements:

- warm and cold local inventory search;
- large tracked/untracked inventory within caps;
- bounded Git process under output saturation;
- multi-page forge parsing within aggregate budget;
- peak retained body bytes relative to configured budget;
- evidence postprocessing over representative card counts.

Performance evidence is not required to prove zero allocation, but it must demonstrate bounded behavior and identify configured caps.

## H.7 CI status verification

Before release classification:

- verify the final commit has completed remote CI runs;
- record each required check and conclusion;
- do not write “deterministic CI is green” when no run/status is available;
- if GitHub status APIs do not expose a run, record the limitation and keep the classification provisional.

## H.8 Release classification

Promotion from provisional RC requires:

- all deterministic tests passing on the final commit;
- native forge smoke evidence;
- supported cross-platform CI evidence;
- regenerated verification record;
- no unresolved safety or evidence-semantics blocker from this plan.

---

# Workstream I — Documentation and enforceable guards

## I.1 Documentation audit

Update claims in:

- `AGENTS.md`
- `docs/architecture/meta.md`
- `docs/architecture/overview.md`
- `docs/config.md`
- `docs/safety.md`
- `docs/release.md`
- `docs/release-verification.md`

Use precise language for:

- immediate cap-triggered process termination;
- operation-wide byte budget scope;
- platform-specific symlink-follow support;
- attempt-derived retrieval semantics;
- workflow-selection precedence;
- distinct-source conflict rules;
- native versus fallback smoke evidence.

## I.2 Static guards

Static guards are supplemental. Add checks for obvious regressions, such as:

- no independent forge body counters inside auxiliary fetch helpers;
- no universal `map_provider_to_intended_roles(_, "source")` in failure reconstruction;
- security groups are materialized before flattening/serialization;
- no path-based safe-open fallback advertised as race-resistant;
- no retrieval summary built solely from provider IDs/cards in production paths.

Do not treat these guards as substitutes for runtime tests.

## I.3 Schema and compatibility

- regenerate schemas if additive fields change;
- run schema identity/corpus tests;
- confirm codegg fixtures deserialize new output;
- retain deprecated fields if needed for compatibility;
- document semantic corrections where an existing field’s values become more precise.

---

## 4. Recommended implementation sequence

### Commit 1 — Contract tests and truthful documentation baseline

- add failing regression tests for all remaining defects;
- downgrade any still-overstated release-verification claims;
- do not change runtime behavior beyond test scaffolding.

### Commit 2 — Unified subprocess termination controller

- consolidate runners;
- implement immediate cap/timeout termination;
- add process-group and descendant tests.

### Commit 3 — Shared forge operation budget

- thread one budget through all provider helpers;
- enforce remaining-byte caps;
- add exact accounting tests and telemetry.

### Commit 4 — Safe symlink-follow policy

- implement Linux contained-follow mode through `openat2`;
- reject unsupported safe-follow mode elsewhere;
- add escape and race tests.

### Commit 5 — Attempt-derived retrieval ledger

- emit attempts from dispatch;
- preserve subquery intended roles and exact outcomes;
- derive failures and retrieval summaries from attempts.

### Commit 6 — Evidence materialization and workflow selection

- fix security serialized cards;
- honor explicit workflows/profiles;
- preserve gap-driven actions through MCP serialization.

### Commit 7 — Distinct-source conflict correction

- retain sourced values and set semantics;
- add entity/package/host scoping tests.

### Commit 8 — Verification and release evidence

- complete mock and native smoke suites;
- complete platform CI;
- run release commands on final runtime commit;
- regenerate the verification record.

### Commit 9 — Documentation closure

- final documentation audit;
- remove obsolete caveats only when evidence supports removal;
- ensure the verification record references this final commit or explicitly references the tested runtime parent plus a documentation-only final commit with a reproducible compare.

Keep commits individually buildable and testable. Do not combine all workstreams into one opaque commit.

---

## 5. Release test matrix

Run at minimum:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo test --locked --no-default-features
cargo test --locked --features mock
cargo test --locked --features pdf
make hardening
make schema-corpus
make docs-tests
cargo build --release
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
cargo publish --dry-run --locked
```

Add targeted commands for:

```bash
cargo test --features mock --test bounded_command
cargo test --features mock --test forge_adapter
cargo test --features mock --test property_conflict
cargo test --features mock --test property_retrieval
cargo test --features mock --test integration
cargo test --features mock --test evidence_bundle_handoff
```

Run native forge smoke tests separately with output retained as release evidence.

---

## 6. Final acceptance checklist

### Subprocess safety

- [ ] stdout and stderr drain concurrently;
- [ ] stdout cap triggers immediate process-group termination;
- [ ] stderr cap triggers immediate process-group termination;
- [ ] timeout is distinguishable from cap breach;
- [ ] descendants are terminated and child is reaped;
- [ ] no production Git command bypasses the unified runner.

### Forge transport bounds

- [ ] one budget covers all auxiliary and tree requests;
- [ ] each response cap is constrained by remaining aggregate budget;
- [ ] aggregate observed bytes never exceed the configured limit;
- [ ] pagination and fallback stop when exhausted;
- [ ] telemetry identifies exhaustion accurately.

### Workspace containment

- [ ] no-follow mode rejects all symlinks;
- [ ] follow mode permits only beneath-root targets on supported platforms;
- [ ] unsupported platforms fail explicitly;
- [ ] final-component and intermediate escapes are rejected;
- [ ] all reads remain descriptor/handle based.

### Evidence semantics

- [ ] security group cards serialize materialized roles;
- [ ] dispatch emits attempt records for every planned job;
- [ ] zero results, skip, timeout, deadline, rate limit, and truncation differ;
- [ ] failures map to actual intended roles;
- [ ] coverage uses attempt-derived failures;
- [ ] retrieval summaries use attempt records;
- [ ] next actions use actual gaps and failure history.

### Conflict correctness

- [ ] conflicts require distinct source IDs;
- [ ] compared values retain provenance;
- [ ] set-valued fields compare normalized per-source sets;
- [ ] package, repository host, and entity scopes prevent false positives.

### Workflow correctness

- [ ] explicit workflow is honored;
- [ ] explicit profile is honored;
- [ ] resolution precedence is deterministic;
- [ ] default behavior remains compatible.

### Verification

- [ ] final commit has remote CI evidence;
- [ ] Linux and other supported platforms pass;
- [ ] native forge adapters are exercised without fallback acceptance;
- [ ] release record references the tested final commit;
- [ ] test counts and platform/toolchain fields are current;
- [ ] no unsupported assertion remains in documentation.

---

## 7. Definition of done

This plan is complete only when:

1. all checklist items above are satisfied;
2. every identified defect has a regression test that fails on baseline `07626669f2cf39bcc62d1259677038ed94d47d09` and passes on the final implementation;
3. the complete release test matrix passes on the final runtime commit;
4. native forge and supported-platform evidence is captured;
5. `docs/release-verification.md` is regenerated and internally consistent;
6. codegg integration fixtures consume the final response structures;
7. a final review finds no remaining path that can cause workspace escape, uncontrolled child execution, aggregate body-budget bypass, provenance misrepresentation, or materially misleading evidence coverage.

Until these conditions are met, retain the repository classification as **provisional release candidate**.