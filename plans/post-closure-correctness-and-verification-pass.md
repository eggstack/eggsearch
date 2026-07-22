# Post-Closure Correctness and Verification Pass

**Status:** Ready for implementation handoff  
**Repository:** `eggstack/eggsearch`  
**Baseline reviewed:** `b9715808f4fd805b479670749117dcd9c9913226`  
**Purpose:** Close the remaining correctness, safety, evidence-semantics, and release-verification gaps discovered after the production-readiness closure pass.

---

## 1. Executive Summary

The production-readiness closure pass materially improved Eggsearch. The repository now has strong type separation for repository identity, bounded per-response forge reads, local inventory freshness signals, evidence workflow models, extensive property tests, and a broad deterministic CI matrix.

The remaining defects are concentrated rather than architectural, but they affect guarantees that the repository currently describes as complete:

1. local file opening is still pathname-based and does not close the time-of-check/time-of-use race it claims to prevent;
2. Git stdout and stderr are still drained sequentially, and cap breaches do not terminate the child immediately;
3. the forge aggregate byte budget is reported but not enforced across paginated operations;
4. custom endpoint policy is not wired from configuration and DNS validation is preflight-only;
5. GitLab commit-resolution refs containing slashes are not encoded;
6. evidence roles are materialized on detached clones for grouped tool responses, not on the cards actually returned;
7. retrieval-attempt outcomes are not propagated into workflow coverage, and failure states are misclassified;
8. retrieval and conflict summaries retain global or generic assumptions that can mislead agents;
9. the release verification record overstates native forge, subprocess, safe-open, fuzz, performance, and CI evidence.

This pass is intentionally corrective. It must not expand Eggsearch into a daemon, persistent index service, semantic database, repository clone manager, or generalized sandbox. The goal is to make the existing feature set truthful, bounded, race-resistant where claimed, and suitable for codegg integration testing.

---

## 2. Primary Goal

After this pass, Eggsearch must satisfy all of the following:

- every local file read used by workspace search/fetch is performed from a trusted root handle using descriptor-relative, no-follow semantics on supported platforms;
- file content is bounded while reading, not after allocation;
- Git subprocess stdout and stderr are drained concurrently with independent hard caps;
- timeout and cap breaches terminate the entire subprocess group promptly and reap it;
- forge operations enforce both per-response and aggregate byte budgets;
- configured endpoint policy is explicit and consistently applied;
- repository ref path components are encoded for every supported provider;
- evidence roles are present on the exact cards serialized to clients;
- workflow coverage reflects actual provider/subquery outcomes;
- failure, zero-result, policy skip, capability absence, deadline, and truncation states remain distinct;
- conflicts are scoped to the same canonical entity and distinct sources;
- next actions are generated from actual workflow gaps and valid tool inputs;
- native forge behavior and local safety properties are verified against the final commit;
- release documentation contains no claim stronger than the implementation and evidence.

---

## 3. Explicit Non-Goals

Do not add any of the following during this pass:

- repository cloning as a fallback for remote tree retrieval;
- a persistent filesystem watcher or indexing daemon;
- SQLite, vector storage, or semantic embeddings;
- a new MCP tool family;
- broad response-schema redesigns;
- automatic credential forwarding across redirects;
- generalized operating-system sandboxing;
- arbitrary symlink traversal outside configured workspace roots;
- new provider families unrelated to the existing closure defects;
- performance work not tied to a measured closure regression;
- compatibility-breaking removal of existing additive response fields.

---

## 4. Current Findings to Treat as Failing Contracts

The implementation team must begin by converting each finding below into a failing regression test or source-contract assertion before changing production code.

### 4.1 Local safe-open contract is not satisfied

Current behavior:

- the root directory is opened and the handle is discarded;
- intermediate components are checked by pathname;
- the final component is checked with `symlink_metadata` and then opened by pathname;
- a component can be replaced between check and open;
- `safe_read_file` uses `read_to_end` and truncates after allocation.

Required contract:

- intermediate and final traversal must be descriptor-relative on supported Unix platforms;
- every opened component must use no-follow semantics;
- final metadata must be obtained from the opened descriptor;
- no pathname re-resolution may occur after the trusted root descriptor is established;
- reads must stop once the configured cap is reached;
- file replacement races must either return the originally opened inode or fail safely, never escape the root.

### 4.2 Git command runner contract is not satisfied

Current behavior:

- stdout is read before stderr;
- a child filling both pipes can block until timeout;
- exceeding a capture cap causes the reader to stop but does not immediately kill the process group;
- timeout state is inferred from SIGKILL rather than recorded as an explicit supervisor outcome.

Required contract:

- stdout and stderr must be drained concurrently;
- each stream must have an independent capture cap;
- readers may continue draining/discarding after the capture cap only when needed to avoid blocking, or the supervisor may terminate immediately;
- cap breach must produce an explicit outcome and trigger prompt process-group termination;
- timeout, stdout-cap, stderr-cap, spawn failure, signal termination, and nonzero exit must be distinguishable;
- all child and descendant processes must be reaped or killed.

### 4.3 Forge aggregate byte budget is not enforced

Current behavior:

- each page is independently allowed up to the per-response cap;
- `total_bytes` is updated after a page is read;
- no pre-read or per-chunk check compares observed bytes against the operation budget;
- telemetry reports aggregate-cap reach after the fact.

Required contract:

- every forge operation owns a mutable aggregate budget;
- every chunk consumes both the per-response and aggregate budget;
- the reader rejects before storing bytes that would exceed either budget;
- pagination stops immediately on aggregate exhaustion;
- metadata, commit-resolution, fallback, and tree requests all count toward one operation budget unless explicitly documented otherwise;
- telemetry reports exact observed bytes and the limiting budget that terminated retrieval.

### 4.4 Endpoint policy is incomplete

Current behavior:

- `fetch_tree` always applies the default endpoint policy;
- private self-hosted forges cannot be explicitly enabled through this path;
- DNS is resolved during validation but the subsequent HTTP connection is not pinned to validated addresses;
- documentation acknowledges DNS rebinding remains possible.

Required contract:

- endpoint policy must be represented in configuration and passed into forge retrieval;
- private/loopback allowances must be explicit and disabled by default;
- authenticated HTTP must remain prohibited;
- redirects remain disabled;
- the implementation must either pin validated DNS addresses for the request or explicitly downgrade the security claim and document residual rebinding risk;
- tests must cover all policy combinations without requiring real private infrastructure.

### 4.5 Provider ref encoding is incomplete

Current behavior:

- GitHub and shared Forge paths encode refs;
- GitLab commit resolution interpolates the raw ref into a path component.

Required contract:

- owner, repository/project, ref, branch, tag, tree, path, and commit path components must be encoded according to each provider API;
- slash-containing refs must resolve successfully in unit fixtures;
- query parameters must not be double-encoded;
- response permalink paths must retain semantic path separators where required.

### 4.6 Returned grouped cards do not contain materialized roles

Current behavior:

- grouped cards are cloned into a temporary vector;
- roles are materialized on the clones;
- postprocessing uses the clones;
- original groups are serialized with `evidence_role: null`.

Required contract:

- role inference must run before grouping or mutate grouped cards in place;
- postprocessing and serialized cards must share the same role values;
- no response may report a role count for roles absent from its returned cards unless explicitly documented as derived-only metadata;
- stable card ordering and IDs must remain unchanged.

### 4.7 Retrieval outcomes are not part of coverage

Current behavior:

- `RetrievalAttempt` types exist;
- grouped tool postprocessing still receives an empty retrieval-failure slice;
- coverage becomes indeterminate only for a failure kind that current conversion does not emit;
- zero-result and skipped states are often inferred from provider-level output rather than tracked per subquery.

Required contract:

- dispatch must produce explicit attempt records for every provider/subquery pair considered;
- successful zero-result attempts must remain distinct from skips and failures;
- attempt records must map to intended evidence roles;
- workflow coverage must consume these records;
- a failed retrieval for a missing required role must yield `IndeterminateDueToFailures`, not a definitive absence;
- a completed zero-result retrieval for a required role may yield `Insufficient` with `NoMatchingEvidenceFound`;
- truncation must reduce completion confidence and identify affected roles.

### 4.8 Retrieval and conflict summaries remain overbroad

Current behavior:

- any provider with a result is summarized as primary implementation evidence;
- provider absence can be reported as skipped or not queried even when it returned zero results;
- mutable-versus-pinned conflicts are generated globally;
- vulnerability patched-version values may be compared within one source rather than across distinct sources.

Required contract:

- retrieval summaries derive evidence roles from actual attempt intent and returned cards;
- every dimension identifies provider, subquery, intended role, outcome, result count, and truncation/failure context;
- conflicts compare distinct source cards for the same canonical entity and field;
- one source containing multiple valid values must not conflict with itself;
- unrelated repositories, packages, advisories, benchmarks, or documentation entities must never be grouped into one conflict.

### 4.9 Release verification record is not authoritative

Current behavior:

- the record references a commit that is not the final head;
- architecture and platform text are internally inconsistent;
- native forge smoke tests ran in fallback mode;
- Gitea/Forgejo coverage is claimed without a listed native test;
- performance evidence does not cover the high-risk operations;
- fuzz target counts do not match CI;
- no status/check records were independently visible for the final head.

Required contract:

- generate a new record only after the implementation is complete;
- record the exact final commit and dirty-state status;
- separate local verification from GitHub Actions verification;
- list native and fallback smoke tests separately;
- do not classify the release as RC until native provider, local safety, and CI gates are satisfied;
- every checkbox must point to a command, test, workflow job, or artifact.

---

## 5. Execution Strategy

Implement in the following order:

1. freeze failing contracts and correct documentation overclaims;
2. implement descriptor-relative local open and bounded reads;
3. replace the Git command supervisor;
4. enforce forge aggregate budgets and finish endpoint/ref handling;
5. integrate evidence roles and retrieval attempts into returned responses;
6. correct entity-scoped conflict and retrieval semantics;
7. add native/integration verification and performance evidence;
8. regenerate the release record for the final commit.

Workstreams 2 and 3 may proceed in parallel after contract tests land. Workstreams 5 and 6 may proceed in parallel once shared retrieval-attempt contracts are stable. The release record must be the final commit of the pass or must be immediately followed only by a metadata correction that reruns all gates.

---

# Workstream A: Contract Freeze and Documentation Reset

## A.1 Add explicit failing regression tests

Add tests that fail on the current baseline for:

- final-component symlink replacement between validation and open;
- intermediate-directory symlink replacement;
- file growth beyond read cap after open;
- subprocess writing enough output to fill stdout and stderr simultaneously;
- stdout cap breach requiring termination before timeout;
- stderr cap breach requiring termination before timeout;
- aggregate forge budget exceeded across two individually valid pages;
- metadata plus tree requests exceeding one operation budget;
- GitLab slash-containing ref resolution;
- grouped response cards retaining `evidence_role: null` while summaries report roles;
- provider failure for a required role being reported as definitive absence;
- successful zero-result attempt being reported as policy skip;
- mutable/pinned conflict across unrelated repositories;
- multiple patched versions in one advisory creating a self-conflict;
- release verification claiming native provider coverage when mode is fallback.

## A.2 Correct current overclaims before implementation

Update documentation immediately so the baseline is truthful during the pass:

- describe `safe_open` as best-effort pathname validation until descriptor-relative implementation lands;
- describe Git output as capped but sequentially drained;
- describe forge limits as per-response with page-count bounding, not aggregate enforcement;
- state that grouped responses may expose derived role summaries before card role materialization is fixed;
- mark the current release classification as provisional, not release candidate;
- preserve the old verification record as historical evidence only, clearly superseded.

## A.3 Acceptance criteria

- every listed current defect has at least one failing test or source-contract test;
- documentation contains no statement known to be false at the baseline;
- test names clearly encode the invariant, not an implementation detail;
- tests remain deterministic and do not depend on scheduler luck without a bounded synchronization harness.

---

# Workstream B: Descriptor-Relative Local File Opening

## B.1 Unix implementation model

Implement a Unix-specific descriptor-relative traversal layer.

Preferred primitives:

- open configured root with directory-only and close-on-exec flags;
- walk each normal component relative to the current directory descriptor;
- use `openat2` with `RESOLVE_BENEATH`, `RESOLVE_NO_MAGICLINKS`, and `RESOLVE_NO_SYMLINKS` where available;
- provide an `openat` fallback using `O_NOFOLLOW`, directory descriptors, and `fstat` validation;
- never concatenate the trusted root with an attacker-controlled relative path after root opening;
- reject empty, absolute, parent-directory, prefix, NUL-containing, and non-normal components;
- reject intermediate non-directories;
- final target must be a regular file unless a future explicit policy permits otherwise;
- use descriptor metadata for size and identity checks.

Do not silently use the pathname implementation on Unix when the descriptor-relative implementation fails. Return a structured safe-open error.

## B.2 Platform abstraction

Create an internal abstraction such as:

```rust
trait SafeWorkspaceOpen {
    fn open_relative(
        &self,
        root: &Path,
        relative: &Path,
        policy: &LocalOpenPolicy,
    ) -> Result<OpenedWorkspaceFile, SafeOpenError>;
}
```

Requirements:

- Unix backend provides descriptor-relative guarantees;
- Windows backend uses handle-based no-reparse-point semantics where practical;
- unsupported guarantees must be explicit in telemetry/documentation;
- tests may inject a deterministic backend for race simulation;
- public MCP schema need not expose the backend type.

## B.3 Hard-capped content reads

Replace `read_to_end` with a bounded reader:

- read at most `max_size + 1` bytes;
- return a structured `FileContentLimitExceeded` result when the file exceeds the hard limit;
- do not allocate based solely on untrusted metadata;
- cap initial capacity at a small bounded value;
- distinguish search-snippet truncation from safety-limit rejection;
- do not return partially read source content as complete unless the response explicitly marks it truncated.

If repo search intentionally reads a bounded prefix, expose that as prefix/truncation metadata rather than silently truncating.

## B.4 Integrate every local read path

Audit and route through the safe-open abstraction:

- local inventory search content reads;
- local repo fetch;
- local repo map important-file probes;
- symbol extraction;
- exact-error local search;
- dependency-file reads performed on behalf of local repository workflows;
- any fallback text-match helper that currently reopens by absolute path.

Remove secondary path-based reopens after a safe descriptor has already been obtained. Pass content or the opened handle to downstream logic.

## B.5 Race tests

Add deterministic race tests using barriers or hooks:

1. replace final regular file with symlink after parent traversal but before final open;
2. replace intermediate directory with symlink;
3. rename the root directory while a trusted root descriptor remains open;
4. replace a file after open but before read;
5. grow a file while it is being read;
6. swap a file with a FIFO, socket, or directory;
7. test symlink-to-inside-root with `follow_symlinks=false`;
8. test the documented behavior when `follow_symlinks=true`;
9. test non-UTF-8 components on Unix;
10. test deeply nested paths near component limits.

## B.6 Static guards

Static guards must check meaningful invariants, not string absence alone:

- production local read paths cannot call `std::fs::read`, `read_to_string`, or `File::open` on reconstructed workspace paths;
- Unix safe-open backend must contain descriptor-relative operations;
- root descriptor must be retained through traversal;
- bounded read helper must not call `read_to_end`.

## B.7 Acceptance criteria

- root escape is not possible through intermediate or final symlink substitution on Unix;
- the final file descriptor is validated as a regular file;
- no content read allocates beyond the configured hard cap plus fixed overhead;
- every local retrieval path uses the same safe-open policy;
- Windows behavior is tested or explicitly documented as a release limitation;
- local integration tests pass for normal files, linked worktrees, ignored files, large files, and concurrent changes.

---

# Workstream C: Concurrent and Hard-Bounded Git Subprocess Supervision

## C.1 Replace the current runner

Implement one command supervisor rather than duplicated inventory-specific runners.

Suggested internal result:

```rust
enum CommandTermination {
    Exited,
    TimedOut,
    StdoutLimitExceeded,
    StderrLimitExceeded,
    SpawnFailed,
    WaitFailed,
    Signaled,
}

struct BoundedCommandResult {
    status: Option<ExitStatus>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    termination: CommandTermination,
    elapsed: Duration,
    stdout_observed: u64,
    stderr_observed: u64,
    stdout_captured: usize,
    stderr_captured: usize,
}
```

## C.2 Concurrent drainage

Requirements:

- take stdout and stderr handles immediately after spawn;
- drain them on independent threads or async tasks;
- enforce independent observed/captured byte counters;
- report read failures explicitly;
- do not wait for one stream before reading the other;
- join both reader tasks after process termination;
- never hold a lock while blocking on pipe reads.

## C.3 Cap behavior

Choose and document one of two valid models:

**Terminate-on-cap model:**

- once either observed stream exceeds its cap, signal the supervisor;
- kill the process group immediately;
- continue draining until EOF after kill;
- reap the child and descendants;
- return the relevant cap termination outcome.

**Discard-after-cap model:**

- retain only capped bytes;
- continue draining and discarding to avoid pipe blockage;
- optionally terminate when aggregate output exceeds an operation cap;
- never report a successful complete command if semantically required output was truncated.

For Git inventory commands, prefer terminate-on-cap because truncated `ls-files` or `status` output cannot safely define a complete inventory/change token.

## C.4 Process-group lifecycle

Unix requirements:

- create a dedicated process group/session and check `setsid` failure;
- use TERM followed by a short grace period, then KILL, or document immediate KILL;
- target the process group, not only the direct child;
- avoid PID reuse races by coordinating supervisor completion with child wait;
- explicitly record whether the supervisor initiated termination.

Windows requirements:

- use job objects or a documented equivalent to terminate descendants;
- add platform-gated tests where CI permits;
- do not infer timeout from a particular exit code alone.

## C.5 Remove redundant Git executions

The inventory currently runs a combined tracked/untracked `ls-files` and a second untracked-only invocation for counting.

Replace this with one of:

- one command using distinct stages whose output can be classified;
- derive `untracked_count` from a bounded status command already required for the change token;
- accept `untracked_count=None` rather than launching redundant work.

Do not add unbounded helper commands for diagnostics.

## C.6 Test matrix

Add deterministic tests for:

- small stdout and stderr;
- stdout-only cap;
- stderr-only cap;
- both pipes producing beyond OS pipe capacity;
- child process spawning a grandchild that keeps a pipe open;
- timeout with no output;
- timeout while both streams are active;
- cap breach completes before the nominal timeout;
- nonzero exit with bounded diagnostics;
- invalid UTF-8 output;
- process-group kill leaves no descendant alive;
- repeated timeout/cap runs without PID race or leaked threads;
- command spawn failure;
- reader failure injection;
- linked-worktree Git commands;
- huge repository status output;
- truncated inventory output causing native-walker fallback with a structured warning.

Assertions must include elapsed upper bounds, explicit termination reason, and captured/observed counters. Merely asserting `status.is_some()` is insufficient.

## C.7 Acceptance criteria

- stdout and stderr are provably drained concurrently;
- a cap breach terminates or safely drains without waiting for timeout;
- no production Git command uses `.output()` or an equivalent unbounded capture;
- descendants cannot survive timeout/cap termination;
- inventory never treats truncated command output as complete;
- telemetry distinguishes timeout, cap, nonzero exit, and spawn/read failures.

---

# Workstream D: Forge Operation Budgets, Endpoint Policy, and Ref Encoding

## D.1 Introduce an operation budget object

Create a shared mutable budget used by every request in one forge-tree operation.

Suggested shape:

```rust
struct ForgeReadBudget {
    per_response_limit: usize,
    aggregate_limit: usize,
    aggregate_observed: usize,
    responses_started: usize,
    responses_completed: usize,
}
```

The bounded reader must:

- reject early when `Content-Length` exceeds the remaining per-response or aggregate budget;
- on every chunk, check `response_observed + chunk.len()` and `aggregate_observed + chunk.len()` before storing;
- consume aggregate bytes for successful, metadata, commit-resolution, fallback, and bounded error bodies;
- report whether termination was per-response or aggregate;
- never wrap on arithmetic overflow;
- retain no more bytes than the configured limit.

## D.2 Decide operation scope

One `repo_map` native forge attempt should normally share one budget across:

- ref/commit resolution;
- default-branch metadata;
- tree pages;
- GitHub Contents fallback;
- bounded error previews.

If error previews use a separate diagnostic allowance, document and cap that allowance separately. Do not let diagnostics become an unbounded bypass.

## D.3 Pagination behavior

- stop before issuing another page when no aggregate budget remains;
- if a page causes aggregate exhaustion, return partial entries only when the response explicitly marks truncation and includes a warning;
- never report `aggregate_byte_cap_reached=false` after an aggregate-budget rejection;
- record pages attempted/completed;
- ensure `response_cap_applied` describes actual cap enforcement rather than equality with an observed total.

## D.4 Endpoint policy wiring

Extend forge configuration with explicit policy fields, for example:

```toml
[forge]
allow_loopback = false
allow_private_network = false
require_https = true
dns_mode = "preflight_validate"
```

Requirements:

- defaults preserve public HTTPS-only behavior;
- private and loopback use requires explicit operator configuration;
- API-key-bearing HTTP is always rejected;
- policy is passed from application configuration to every `fetch_tree` invocation;
- telemetry records the effective policy class without exposing secrets;
- unknown or invalid policy values fail configuration validation.

## D.5 DNS security decision

Choose and document one strategy:

### Option 1: Connection pinning

- resolve hostname once;
- validate every resolved address;
- configure the HTTP client/request to connect only to the validated address set while retaining the original hostname for TLS/SNI;
- reject if the connection target is not in the validated set;
- define TTL/re-resolution behavior for paginated operations.

### Option 2: Explicit preflight-only model

- retain preflight validation;
- document that it does not prevent DNS rebinding;
- do not claim credentials cannot reach a rebinding target;
- consider prohibiting credentials on custom hostnames unless pinning is enabled;
- mark this as a known limitation in release notes.

The implementation may choose Option 2 for scope control, but the documentation and release classification must be accurate.

## D.6 Encode provider path components

Audit every provider endpoint:

- GitHub owner, repo, ref/tree SHA, and contents path;
- GitLab project path, commit ref, repository path, branch/tag refs;
- Gitea/Forgejo/Codeberg owner, repo, commit ref, tree ref, and file path;
- browser/raw URL builders.

Add a provider-specific encoding helper rather than assuming one encoding function fits both API path components and browser paths.

Required fixtures:

- `feature/foo`;
- `release/2026.07`;
- tags containing `+`, `%`, spaces where provider permits, Unicode, and `#`;
- owner/repository names with allowed punctuation;
- nested file paths that must preserve `/` separators;
- already encoded input that must not be double-encoded.

## D.7 Native forge integration tests

Use deterministic mock servers for all providers and optional live tests for public hosts.

Mock tests must cover:

- exact endpoint paths and query parameters;
- commit/tree/blob IDs intentionally different;
- paginated aggregate exhaustion;
- oversized metadata response;
- oversized error response;
- fallback preserving resolved commit/ref;
- redirect rejection;
- DNS/private policy validation;
- slash refs;
- authentication header redaction.

Live tests must distinguish:

- native GitHub tree mode;
- native GitLab tree mode;
- native Codeberg/Gitea-compatible mode;
- fallback mode.

A fallback result does not satisfy a native provider smoke gate.

## D.8 Acceptance criteria

- no forge operation exceeds its aggregate storage budget;
- telemetry accurately identifies the limiting condition;
- configured endpoint policy reaches runtime retrieval;
- credential-bearing HTTP is impossible;
- DNS guarantees are accurately implemented and documented;
- slash refs resolve on GitHub, GitLab, and Gitea/Forgejo fixtures;
- native provider smoke tests exercise provenance and pagination paths.

---

# Workstream E: Evidence Role Materialization and Workflow Selection

## E.1 Materialize roles on returned cards

Refactor response construction so role assignment occurs before serialization.

Preferred flow:

1. aggregate source cards;
2. infer/materialize evidence roles on the owned card collection;
3. group the same cards without losing metadata;
4. derive summaries from the grouped cards or a read-only flattened view;
5. serialize those exact groups.

Alternative:

- add a group-level mutation function that walks every card and assigns missing roles in place before postprocessing.

Do not clone solely for materialization unless the clone replaces the returned collection.

## E.2 Response consistency invariants

Add invariant checks/tests:

- every role counted in `evidence_role_summary` appears on at least one returned card;
- `found_roles` equals the set of roles on returned cards;
- stable IDs and ordering are unchanged by role materialization;
- explicit provider/native roles are not overwritten by heuristic assignment;
- cards classified as weak/unknown remain explicitly weak/unknown rather than null.

Apply to:

- `web_search`;
- `repo_search`;
- `research_search`;
- `security_search`;
- any response wrapping the common postprocess result.

## E.3 Select workflow from actual request intent

`resolve_workflow_model` must accept enough request context to distinguish:

- repository architecture comprehension;
- API comprehension;
- exact-error investigation;
- security review;
- dependency evaluation;
- version migration;
- performance investigation;
- comparative research;
- pre-change evidence;
- post-change review.

Do not pass `profile=None` when the request contains a profile. Do not infer migration or API workflows only from a broad domain when an explicit workflow is present.

Suggested API:

```rust
fn resolve_workflow_model(ctx: &WorkflowSelectionContext) -> Option<WorkflowCoverageModel>
```

Where context contains tool, explicit workflow, profile, research domain, search mode, intent, and relevant flags.

## E.4 Compatibility behavior

- new fields remain additive;
- existing workflow IDs remain stable;
- absent workflow context may use a documented generic model or omit coverage;
- do not fabricate a precise workflow when the request is neutral;
- schema corpus tests must pin all workflow IDs and enum values.

## E.5 Acceptance criteria

- returned cards in every grouped response contain materialized roles;
- summaries are computed from those same cards;
- explicit workflow/profile settings select the intended model;
- neutral web search does not claim workflow completeness without an explicit model;
- response ordering and stable IDs do not regress.

---

# Workstream F: Retrieval Attempts and Failure-Aware Coverage

## F.1 Capture every provider/subquery attempt

Extend dispatch output with a bounded list of `RetrievalAttempt` records.

Each attempt must include:

- provider ID;
- subquery ID/label;
- intended evidence roles;
- outcome;
- result count before and after aggregation filters where useful;
- elapsed time or timeout class;
- truncation flag;
- failure class/message code;
- whether the attempt was launched, skipped, interrupted, or completed.

Create attempt records for:

- success with results;
- success with zero results;
- provider error;
- timeout;
- rate limit;
- policy skip;
- capability-unavailable skip;
- not applicable;
- interrupted by global deadline;
- partial success followed by truncation.

## F.2 Preserve subquery identity through dispatch

Current provider-level aggregation can lose which subquery produced a result or failure.

Requirements:

- dispatch job identifiers include provider and subquery;
- raw results retain subquery provenance long enough to build attempts;
- duplicate provider results across subqueries may still aggregate into one card, but retrieval telemetry remains per attempt;
- attempt list is bounded by configured provider/subquery caps.

## F.3 Map attempts to evidence roles

Improve `map_provider_to_intended_roles`:

- prioritize explicit subquery purpose;
- use provider capabilities only as a fallback;
- allow more than one intended role where appropriate;
- avoid mapping every repository-related provider to primary implementation;
- security advisory, vendor guidance, defensive configuration, official docs, tests, examples, releases, issues, benchmarks, manifests, and independent sources must remain distinct.

## F.4 Convert attempts into coverage failures/absence states

Define a single conversion table:

| Attempt outcome | Coverage meaning |
|---|---|
| success with results | role may be found |
| success zero results | `NoMatchingEvidenceFound` or requested-but-not-found |
| failed | indeterminate for intended roles |
| timed out | indeterminate/deadline for intended roles |
| rate limited | indeterminate/provider failed for intended roles |
| skipped by policy | provider skipped by policy |
| capability unavailable | provider capability unavailable |
| not applicable | evidence role not requested/not applicable |
| interrupted by deadline | indeterminate/deadline |
| truncated partial success | result truncated by cap; confidence reduced |

A missing required role with any unresolved failed attempt targeting that role must not be reported as definitively absent.

## F.5 Correct coverage status logic

Update `coverage_status` so indeterminate status is triggered by relevant unresolved failures, not only one rarely emitted enum value.

Rules:

- all required roles found: sufficient or usable-with-gaps regardless of unrelated provider failures;
- required role missing and all targeted attempts completed with zero results: insufficient;
- required role missing and at least one targeted attempt failed/timed out/rate-limited/interrupted: indeterminate;
- required role missing and no capable provider existed: insufficient with capability-unavailable reason, unless policy requires indeterminate;
- truncation affecting a required role lowers confidence and may be indeterminate when completeness cannot be determined;
- recommended-role failures do not override complete required coverage, but reduce confidence and explain gaps.

## F.6 Wire attempts into every tool

- `web_search`: include provider-level attempts and no workflow model unless requested;
- `repo_search`: pass repository subquery attempts to coverage;
- `research_search`: pass planned source-type attempts;
- `security_search`: include generic search, native advisory lookup, package query, and KEV lookup attempts where relevant;
- local workspace retrieval should contribute a local attempt when queried.

Do not continue passing `&[]` where attempts or failures exist.

## F.7 Generate next actions from actual gaps

Use `generate_gap_driven_next_actions` after coverage is computed.

Requirements:

- actions appear in the response field intended for client consumption, not only nested in a derived structure if the schema provides both;
- templates match the actual MCP tool schema;
- repository actions include owner/repo only when known;
- source IDs contain only relevant supporting cards;
- after failure, recommend a changed provider, query, scope, or tool rather than an identical retry;
- avoid generic placeholder actions when concrete ref/path/package/advisory identifiers exist;
- cap and deterministically sort actions.

## F.8 Acceptance criteria

- every dispatched provider/subquery has one terminal attempt record;
- zero results, failure, skip, and timeout are distinct in serialized output;
- required-role failures produce indeterminate coverage;
- completed no-match retrievals produce definitive gaps;
- coverage confidence changes when truncation/failure affects required evidence;
- next actions are valid, bounded, and derived from real gaps.

---

# Workstream G: Entity-Scoped Retrieval and Conflict Semantics

## G.1 Replace generic provider summaries

Build retrieval summaries from attempt records rather than inferring from provider IDs and final cards.

Each dimension should include:

- intended role;
- provider ID;
- subquery ID;
- outcome/absence kind;
- result count;
- message code;
- query or normalized query identifier when safe;
- truncation/deadline context.

A provider returning documentation must not be labeled primary implementation merely because it returned a card.

## G.2 Canonical entity keys

Define canonical keys per conflict class:

- vulnerability: normalized CVE/GHSA/OSV/RustSec identifier set;
- package: normalized ecosystem plus package name and optionally version target;
- repository: normalized host, owner, repo, and logical ref scope;
- benchmark: benchmark suite, task/version, metric, and model/project identity;
- documentation: canonical product/API/version topic where reliable;
- release: project plus release/tag identity.

Do not create a conflict when canonical identity is unavailable or ambiguous. Prefer no conflict over a cross-entity false positive.

## G.3 Compare distinct sources only

- deduplicate by stable source ID before comparison;
- require at least two distinct source cards;
- compare values grouped by source, not a flattened value vector;
- multiple valid values from one source are a set, not an internal conflict;
- compare normalized sets across sources;
- preserve all source IDs contributing to the disagreement.

## G.4 Scope mutable-versus-pinned conflicts

- group code evidence by canonical repository identity;
- only compare mutable and pinned evidence within that group;
- optionally include path/symbol scope when repository-wide comparison is too broad;
- do not produce one global conflict across unrelated repositories;
- exclude weak generic web cards with no repository identity.

## G.5 Vulnerability conflict rules

- normalize advisory IDs and alias sets;
- compare patched/affected ranges across distinct advisory sources for the same vulnerability/package;
- distinguish complementing values from contradictory values;
- different fixed versions are not automatically conflicts when both are valid for different branches;
- date conflicts should use source-specific dates and a documented tolerance;
- severity conflicts should preserve scoring system/version.

## G.6 Benchmark conflict rules

Only implement benchmark conflicts when metadata is sufficient to compare:

- same benchmark/task/version;
- same metric and direction;
- same model/project identity;
- comparable hardware/settings where required.

Otherwise return an incomparability reason rather than a conflict.

## G.7 Acceptance criteria

- unrelated entities never share a conflict;
- one source cannot conflict with itself;
- mutable/pinned conflicts are repository-scoped;
- vulnerability conflicts require distinct sources and comparable fields;
- conflict IDs are deterministic and order-independent;
- property tests cover permutations, duplicate sources, alias identifiers, and multi-valued fields.

---

# Workstream H: Verification, CI, and Release Evidence

## H.1 Correct static guards

Replace weak string-presence checks with targeted contract tests where possible.

Examples:

- instantiate the subprocess runner and prove concurrent drainage behavior;
- race-test descriptor-relative opening;
- test aggregate forge budget across multiple responses;
- serialize grouped responses and assert role consistency;
- run coverage with failed required-role attempts;
- inspect every configured forge path with slash refs.

Static guards remain useful for preventing obvious regressions, but they must not be used as the sole proof of runtime properties.

## H.2 CI matrix

Required checks on the final head:

- `cargo fmt --check`;
- `cargo clippy --all-targets --all-features -- -D warnings`;
- tests with all features;
- tests with no default features;
- mock feature tests;
- PDF feature tests;
- docs with warnings denied;
- schema/corpus contracts;
- hardening suites;
- release build;
- publish dry-run;
- Linux platform tests;
- macOS platform tests for descriptor-relative and workspace behavior;
- Windows tests where platform-specific local-open/process behavior is claimed.

If platform CI is unavailable, release documentation must list the unverified platform as a limitation.

## H.3 Fuzz and property coverage

Add or activate fuzz targets for:

- forge bounded body with aggregate budgets;
- URL/ref encoding;
- retrieval-attempt conversion;
- conflict entity-key normalization;
- safe relative path parsing;
- local bounded read limits.

Ensure every listed fuzz target exists and appears in CI. Do not claim property tests are equivalent to ASan/libFuzzer; describe them as complementary.

## H.4 Native provider smoke tests

Run native provider smoke tests only when credentials/config permit native API use.

For each provider, assert:

- response mode is native, not fallback;
- resolved commit SHA is present or an explicit warning explains absence;
- tree SHA/object SHA remain distinct;
- entry URLs use the commit SHA;
- nested entries are present;
- slash-containing ref test passes where a stable public fixture exists;
- pagination or bounded response behavior is exercised by mock tests even if live repos are small.

Minimum provider evidence:

- GitHub native;
- GitLab native;
- Codeberg or a Gitea/Forgejo-compatible native API;
- explicit Gitea/Forgejo fixture if Codeberg behavior is not identical enough to cover it.

Fallback smoke tests should remain, but in a separate section.

## H.5 Local safety and subprocess integration matrix

Record tests for:

- descriptor-relative final-component race;
- intermediate-component race;
- hard-capped growing file;
- concurrent stdout/stderr saturation;
- cap-triggered termination latency;
- descendant process termination;
- linked worktrees;
- new untracked file detection after probe interval;
- ignored file changes;
- concurrent cold inventory publication;
- failed rebuild preserving prior valid cache where policy allows.

## H.6 Performance and memory evidence

Benchmark the operations affected by this pass:

- cold inventory build on small, medium, and large fixtures;
- warm local search;
- status-hash freshness probe;
- bounded Git command with large output;
- native forge tree parsing with multiple pages;
- aggregate-budget rejection;
- evidence postprocessing at representative card/attempt counts;
- safe-open/read path overhead.

Memory evidence must use an actual measurement method. Serialization microbenchmarks do not establish bounded process memory.

At minimum record:

- fixture dimensions;
- wall-clock distribution;
- peak RSS or allocator metric where practical;
- captured/observed byte limits;
- before/after comparison against the baseline.

## H.7 Regenerate release verification record

The new record must include:

- exact final commit SHA;
- branch and clean working-tree statement;
- UTC timestamp;
- toolchain host triple and actual hardware architecture without contradiction;
- local verification commands and results;
- GitHub Actions run/check URLs or IDs;
- native provider smoke results;
- fallback smoke results;
- platform matrix;
- fuzz targets actually run;
- performance and memory evidence;
- known residual limitations;
- release classification based on completed gates.

The record must not say “deterministic CI is green” unless CI status/check evidence exists for the exact final commit.

## H.8 Acceptance criteria

- all required CI jobs pass on the final commit;
- native and fallback smoke evidence are clearly separated;
- every release checkbox has verifiable evidence;
- the verification record references the true final head;
- no documented guarantee exceeds the code;
- unresolved limitations are explicit and reflected in release classification.

---

## 6. Recommended Implementation Slices and Commit Order

Use small, reviewable commits. Suggested sequence:

1. `test: freeze post-closure failing contracts`
   - add failing regression tests;
   - downgrade inaccurate documentation claims;
   - mark old verification record superseded.

2. `fix(local): implement descriptor-relative safe workspace open`
   - Unix backend;
   - root-handle traversal;
   - final descriptor validation;
   - structured errors.

3. `fix(local): enforce hard content caps and route all reads through safe open`
   - bounded reader;
   - remove path reopens;
   - local search/fetch/map integration.

4. `fix(git): concurrently drain bounded subprocess output`
   - unified supervisor;
   - termination outcomes;
   - process-group lifecycle.

5. `test(git): add saturation, cap, timeout, and descendant tests`
   - elapsed assertions;
   - leak checks;
   - linked-worktree coverage.

6. `fix(forge): enforce operation-wide response budgets`
   - budget object;
   - chunk accounting;
   - pagination stop and telemetry.

7. `fix(forge): wire endpoint policy and complete ref encoding`
   - config schema;
   - DNS decision;
   - GitLab slash refs;
   - provider fixtures.

8. `fix(evidence): materialize roles on returned grouped cards`
   - in-place role assignment;
   - response consistency invariants;
   - workflow selection context.

9. `fix(evidence): propagate retrieval attempts into workflow coverage`
   - dispatch attempt records;
   - failure conversion;
   - status/confidence correction;
   - gap-driven next actions.

10. `fix(evidence): scope retrieval summaries and conflicts by entity`
    - attempt-based summaries;
    - distinct-source comparisons;
    - repository/vulnerability normalization.

11. `test: add end-to-end codegg evidence and native forge fixtures`
    - MCP JSON fixtures;
    - native/fallback mode assertions;
    - schema stability.

12. `ci: complete platform, fuzz, and closure verification gates`
    - add missing fuzz target(s);
    - platform jobs;
    - exact release gates.

13. `docs: publish final post-closure verification record`
    - final SHA;
    - CI evidence;
    - native smoke evidence;
    - performance/memory data;
    - truthful release classification.

Do not combine all production changes into one commit. The local-open and subprocess changes require isolated review because both are security-critical and platform-sensitive.

---

## 7. Detailed Test Inventory

The final pass should add or strengthen at least the following test groups.

### 7.1 Local safe-open tests

- normal nested file;
- empty path;
- absolute path;
- parent traversal;
- root/prefix component;
- hidden component policy;
- final symlink;
- intermediate symlink;
- symlink substitution race;
- directory substitution race;
- FIFO/socket/device rejection;
- root rename while descriptor remains valid;
- file growth past hard cap;
- sparse oversized file;
- non-UTF-8 path;
- maximum nesting;
- concurrent rename/delete;
- Windows reparse-point behavior where supported.

### 7.2 Subprocess tests

- small successful command;
- nonzero command;
- invalid UTF-8;
- stdout saturation;
- stderr saturation;
- simultaneous saturation;
- timeout;
- cap termination before timeout;
- grandchild holds pipe;
- grandchild ignores TERM;
- repeated kills;
- spawn failure;
- read failure injection;
- no leaked child/process group;
- observed versus captured byte accounting;
- truncated Git output rejected as inventory source.

### 7.3 Forge tests

- per-response cap;
- aggregate cap across pages;
- aggregate cap across metadata plus tree;
- content-length early rejection;
- chunked overflow;
- multibyte UTF-8 split across chunks;
- bounded error preview;
- redirect disabled;
- public DNS accepted;
- private DNS rejected/default;
- private DNS allowed/explicit policy;
- authenticated HTTP rejected;
- endpoint policy config serialization;
- GitHub slash ref;
- GitLab slash ref;
- Gitea/Forgejo slash ref;
- fallback ref consistency;
- different commit/tree/blob IDs;
- operation telemetry accuracy.

### 7.4 Evidence tests

- roles present on returned web cards;
- roles present on returned repo groups;
- roles present on returned research groups;
- roles present on returned security groups;
- summary roles equal returned-card roles;
- explicit role preserved;
- workflow selected from explicit request;
- zero-result required role -> insufficient;
- failed required role -> indeterminate;
- timed-out required role -> indeterminate;
- failed recommended role -> usable with lower confidence when required roles exist;
- truncation lowers confidence;
- local provider attempt included;
- next action changes scope after failure;
- next action templates deserialize into tool input schema.

### 7.5 Conflict tests

- unrelated repositories do not conflict;
- same repository mutable/pinned evidence conflicts;
- duplicate same source does not conflict;
- one advisory with two patched branches does not self-conflict;
- two advisories for same CVE with incompatible fixed versions conflict only when comparable;
- CVE/GHSA aliases normalize to one entity;
- benchmark values with different benchmark versions do not conflict;
- deterministic ordering and IDs across input permutations;
- conflict cap and truncation behavior.

### 7.6 Verification tests

- release record final SHA equals repository head at generation time;
- every claimed fuzz target exists;
- every required workflow job is present;
- native smoke section cannot accept `fallback_search` mode;
- platform/toolchain architecture fields are internally consistent;
- no unchecked “all gates pass” statement when status data is missing.

---

## 8. Codegg End-to-End Acceptance Scenarios

Add fixture-driven MCP scenarios that resemble codegg consumption.

### Scenario 1: Repository architecture investigation

Input:

- `repo_search` for a known repository architecture question;
- local and remote evidence enabled.

Assertions:

- returned cards have explicit roles;
- architecture workflow is selected;
- implementation and design-document roles are distinguished;
- local attempt outcome is represented;
- missing evidence yields concrete next actions.

### Scenario 2: Exact-error investigation with provider timeout

Assertions:

- exact-error workflow is selected;
- failed issue provider makes relevant coverage indeterminate;
- successful implementation result remains visible;
- next action suggests a changed provider/scope, not identical retry.

### Scenario 3: Security review with native advisory failure

Assertions:

- advisory role is required;
- generic web discussion does not satisfy authoritative advisory role;
- native advisory timeout produces indeterminate status;
- remediation is not overstated;
- retrieval summary distinguishes generic success from advisory failure.

### Scenario 4: Local dirty worktree

Assertions:

- newly created untracked source becomes discoverable after freshness probe;
- ignored files stay excluded;
- safe-open rejects a raced symlink;
- local content caps are enforced;
- no workspace escape occurs.

### Scenario 5: Remote slash-containing branch

Assertions:

- native provider commit resolution succeeds;
- resolved commit, tree, and object IDs are distinct;
- immutable URLs use the commit;
- fallback request preserves the same ref/commit state.

### Scenario 6: Comparative research with conflicting evidence

Assertions:

- comparative workflow selected;
- conflicts only join sources for the same canonical entity;
- unrelated mutable/pinned repository evidence does not conflict;
- counterpoint gap generates a research next action.

---

## 9. Rollback and Compatibility Requirements

### 9.1 Safe-open rollback

- keep the previous implementation only behind a test-only or explicitly unsafe compatibility flag if absolutely necessary;
- production defaults must use the hardened backend;
- do not silently fall back from descriptor-relative to pathname access on Unix;
- failure should produce a structured warning and skip the file.

### 9.2 Subprocess rollback

- retain the old runner only in tests long enough to compare behavior;
- all production Git invocations must move atomically to the new supervisor;
- command output schema is internal and may change, but warnings/telemetry should remain additive.

### 9.3 Forge compatibility

- response fields remain additive;
- existing per-response defaults may remain, but aggregate default must be documented;
- endpoint policy defaults must not unexpectedly enable private access;
- custom self-hosted users need migration documentation for explicit policy flags.

### 9.4 Evidence compatibility

- adding materialized `evidence_role` values is additive;
- preserve existing enum strings;
- preserve existing card IDs/order;
- if `next_actions` are populated where previously empty, cap output and retain deterministic ordering;
- schema corpus fixtures must be updated intentionally.

---

## 10. Observability Requirements

Add or correct telemetry so operators and agents can understand bounded failures.

### 10.1 Local access telemetry

- safe-open backend used;
- rejection reason code;
- file content cap reached;
- symlink/reparse rejection count;
- content bytes observed/captured;
- platform guarantee level.

Do not expose absolute sensitive paths unless current policy already permits them.

### 10.2 Git telemetry

- command class, not full secret-bearing arguments;
- termination reason;
- elapsed time;
- stdout/stderr observed and captured bytes;
- fallback-to-native-walker reason;
- process-group termination success.

### 10.3 Forge telemetry

- endpoint origin without credentials;
- effective policy class;
- per-response and aggregate limits;
- bytes observed;
- pages attempted/completed;
- limit that caused truncation;
- provenance pinned/unpinned state.

### 10.4 Evidence telemetry

- attempts by outcome;
- attempts by intended role;
- missing required/recommended roles;
- failures affecting coverage;
- next-action count;
- conflict count and truncation.

All telemetry must be bounded and deterministic where serialized.

---

## 11. Security Review Checklist

Before closure, perform a targeted review of:

- descriptor lifetime and ownership;
- `openat`/`openat2` flags and fallback semantics;
- Unix `unsafe` blocks and error handling;
- Windows reparse-point handling if implemented;
- process-group creation failure;
- PID reuse and kill races;
- pipe reader thread lifecycle;
- cap arithmetic overflow;
- DNS validation and credential handling;
- URL encoding and injection;
- error-body redaction;
- absolute path leakage in warnings;
- evidence metadata derived from untrusted content;
- next-action tool inputs containing untrusted text;
- conflict grouping denial-of-service via high-cardinality identities.

Every `unsafe` block added in this pass must have a nearby safety comment and a focused test where practical.

---

## 12. Performance Budgets

Establish baselines before enforcing thresholds.

Initial guardrails:

- safe-open overhead should remain compatible with interactive local search;
- status-hash probe must remain bounded by the Git timeout and output caps;
- concurrent pipe draining must not leak one thread per command after completion;
- aggregate forge enforcement must not retain duplicate page buffers;
- evidence postprocessing must remain bounded by result/attempt caps and avoid quadratic cross-entity comparisons.

Potential severe-regression alerts:

- >2x median regression in warm local search on the same fixture;
- >2x peak RSS for the same large forge fixture;
- unbounded thread growth across repeated Git commands;
- conflict processing exceeding quadratic behavior within one canonical group without explicit caps;
- next-action generation scaling with raw unbounded provider output.

Do not adopt exact release-blocking latency values until representative baseline data is recorded.

---

## 13. Definition of Done

This pass is complete only when every item below is true.

### Local file safety

- [ ] Unix local reads are descriptor-relative from a trusted root handle.
- [ ] Intermediate and final symlink substitution cannot escape the root.
- [ ] Final file type and size are checked from the opened descriptor.
- [ ] File reads stop at the hard cap without `read_to_end` over-allocation.
- [ ] Every local search/fetch/map read path uses the hardened abstraction.
- [ ] Platform limitations are explicit.

### Git execution

- [ ] Stdout and stderr are drained concurrently.
- [ ] Independent stream caps are enforced while reading.
- [ ] Cap breach triggers prompt termination or safe discard behavior.
- [ ] Timeouts/caps terminate descendants and reap the child.
- [ ] Explicit termination reasons are recorded.
- [ ] No production Git `.output()` or unbounded capture remains.
- [ ] Truncated Git output is never treated as complete inventory/status data.

### Forge safety and provenance

- [ ] Per-response and operation-wide aggregate byte budgets are enforced.
- [ ] Metadata, commit, tree, fallback, and diagnostic requests are budgeted.
- [ ] Pagination stops on aggregate exhaustion.
- [ ] Effective endpoint policy is configurable and reaches runtime.
- [ ] Credential-bearing HTTP remains impossible.
- [ ] DNS guarantees and residual risk are truthful.
- [ ] GitLab and other provider slash refs are correctly encoded.
- [ ] Native provider tests verify commit/tree/blob separation and immutable URLs.

### Evidence workflows

- [ ] Returned grouped cards carry materialized evidence roles.
- [ ] Role summaries match the exact returned cards.
- [ ] Explicit request workflow/profile selects the correct coverage model.
- [ ] Every provider/subquery attempt has a terminal outcome.
- [ ] Zero results, failures, skips, deadlines, and truncation remain distinct.
- [ ] Missing required evidence with retrieval failure is indeterminate.
- [ ] Completed no-match retrieval produces a definitive gap.
- [ ] Next actions are valid, bounded, concrete, and gap-driven.

### Retrieval and conflicts

- [ ] Retrieval summaries derive from actual attempts and roles.
- [ ] Successful providers are not generically labeled implementation evidence.
- [ ] Conflicts require the same canonical entity and distinct sources.
- [ ] Mutable/pinned conflicts are repository-scoped.
- [ ] One source cannot conflict with itself.
- [ ] Multi-valued advisory fields are compared as source-level sets.

### Release evidence

- [ ] CI is green for the exact final commit and evidence is recorded.
- [ ] Native forge smoke tests run in native mode.
- [ ] Fallback smoke tests are recorded separately.
- [ ] Linux and macOS closure tests pass; Windows claims match evidence.
- [ ] Fuzz target list matches the targets actually run.
- [ ] Performance and memory evidence covers affected high-risk operations.
- [ ] The final verification record references the true head and clean state.
- [ ] Documentation contains no stronger guarantee than implementation/evidence.

---

## 14. Final Release Classification Rule

Use the following classification after implementation:

- **Late beta:** any local root-escape/TOCTOU issue, subprocess deadlock/unbounded behavior, forge aggregate-budget defect, or materially misleading coverage semantics remains.
- **Provisional release candidate:** implementation defects are closed, deterministic tests pass, but native provider or cross-platform CI evidence is incomplete.
- **Release candidate:** all correctness gates pass on the final commit, native provider smoke evidence exists, platform limitations are documented, and no release record claim is unsupported.
- **Stable release ready:** release candidate evidence is complete and codegg integration fixtures pass without material response-contract defects.

Do not promote based solely on test count. Promotion requires the specific runtime invariants and evidence described in this plan.

---

## 15. Handoff Notes

Implementation should begin with the failing contracts in Workstream A. The most security-sensitive work is Workstream B and Workstream C; these should receive isolated review and should not be hidden inside broad refactors. Workstream D should retain the current strong provenance separation while correcting operation-budget enforcement and ref encoding. Workstreams E through G must be validated through serialized end-to-end responses, not only helper-unit tests.

The final documentation/verification commit should be produced only after the exact final implementation SHA has passed the required deterministic and native checks. If any code changes after that record is generated, rerun the affected matrix and update the record to the new final SHA.
