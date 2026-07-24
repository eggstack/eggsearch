# Final Provider Attribution and Release Evidence Closure Plan

**Repository:** `eggstack/eggsearch`  
**Baseline:** `dd181392267bd4d45fbadd055b60df85db51fc30`  
**Status:** Handoff plan  
**Scope:** Narrow corrective closure  
**Primary consumer:** codegg and other agent hosts that depend on truthful evidence and retrieval metadata

---

## 1. Purpose

Eggsearch is close to release-candidate quality, but the final review identified a small set of correctness and release-evidence defects that remain material for agent use:

1. provider capability mismatches are recorded as `not_applicable` rather than `skipped_capability_unavailable`;
2. partial provider capability is treated as an all-or-nothing job skip;
3. native advisory lookup APIs erase provider identity and suppress individual provider errors;
4. native advisory lookups are not reliably constrained to the request's routed provider set;
5. candidate-limit saturation is labeled as confirmed truncation without proof;
6. the native-forge workflow can report success when provider tests did not execute;
7. slash-ref testing depends on an unprovisioned default ref;
8. the release-verification record is stale, internally inconsistent, and not tied to the actual final runtime subject.

This plan closes only those items. It does not reopen already-closed forge budgeting, process-group termination, local path containment, workflow selection, evidence-role materialization, or conflict-scoping work.

---

## 2. Completion Standard

This line of work is complete only when all of the following are true:

- every retrieval attempt reports the real provider, operation, intended role set, and outcome;
- unsupported roles are distinguished from non-applicable work;
- partial capability never suppresses work a provider can perform;
- advisory provider failures cannot be converted into false zero-result success;
- explicit provider routing constrains native advisory execution;
- truncation is reported only when supported by evidence;
- native forge release jobs fail when credentials, fixtures, execution evidence, or provider coverage are missing;
- a new immutable release subject commit is verified by the full deterministic matrix and all required native forge jobs;
- a separate evidence-only commit records exact workflow run IDs and generated evidence for that release subject;
- the release documentation contains no pending measurement presented as completed evidence.

The target classification after completion is **release candidate**. Until every release-evidence gate passes, the repository remains a **provisional release candidate**.

---

## 3. Current Defects and Required Semantics

### 3.1 Capability mismatch is not “not applicable”

`dispatch_subqueries` currently detects unsupported roles through `SearchEngine::supports_role()` and sets `skip_not_applicable`. The dispatcher then emits `RetrievalAttemptOutcome::NotApplicable`.

These concepts are different:

- **Not applicable:** the operation does not apply to the request. Example: KEV lookup requested but no CVE identifier exists.
- **Capability unavailable:** the operation applies, but this provider cannot perform it.
- **Policy skipped:** the operation applies and the provider could perform it, but configuration or operator policy prevented execution.

Agent consumers rely on this distinction. A capability skip means the evidence dimension is still unresolved; `not_applicable` means no evidence was required from that operation.

### 3.2 Partial support must not skip supported work

The current `any(!supports_role)` check skips an entire provider/subquery job when one intended role is unsupported. Multi-role attempts can therefore lose valid work.

Required behavior:

- fully supported role set: dispatch normally;
- partially supported role set: dispatch the supported roles and separately record unsupported roles as capability skips;
- fully unsupported role set: do not dispatch and record capability skips for all intended roles;
- empty role set: preserve existing fallback role mapping, then apply the same partitioning rules.

### 3.3 Native advisory APIs erase provider outcomes

`MetadataSearchAdapter::lookup_advisory()` and `query_advisories_by_package()` currently iterate engines, continue on errors, and return only the first useful aggregate result. This prevents the security orchestrator from knowing:

- which provider actually executed;
- which provider returned a result;
- which providers returned zero results;
- which providers failed;
- whether all providers failed;
- whether the operation was unsupported rather than a real zero-result lookup.

Hardcoded provider labels based on identifier type do not solve this. Provider identity must come from the engine that executed the operation.

### 3.4 Native advisory execution must respect routing

An explicit provider list or routed provider decision must constrain native advisory operations. Native advisory lookup must not silently fan out across every enabled engine when the request selected a narrower set.

### 3.5 Candidate-limit saturation is not confirmed truncation

Returning exactly `candidate_limit` results only proves that the limit was reached. It does not prove that additional results existed. The response should distinguish:

- no truncation signal;
- limit reached, additional results unknown;
- confirmed truncation by Eggsearch;
- provider-reported truncation or additional-page availability.

### 3.6 Native forge release jobs can be false green

The current workflow does not inject the expected GitLab, Codeberg, and Gitea token variables into their jobs. Tests return successfully when a token is absent, and the summary job fails only on explicit `fail`. A skipped provider can therefore produce a green release workflow.

### 3.7 Slash-ref fixture is not deterministic

The GitHub slash-ref test defaults to `smoke/slash-ref`, but the target repository is not guaranteed to contain that ref. Release evidence must use a stable, provisioned fixture and must fail when the fixture is unavailable.

### 3.8 Release evidence is stale

The verification record names an older commit and still contains pending run IDs and pending benchmarks while also asserting completion. The final release protocol must use:

- `R`: immutable code-bearing release subject;
- `E`: evidence-only commit that verifies `R` and changes no runtime code.

---

## 4. Non-Goals

Do not expand this pass into:

- new search providers;
- new MCP tools;
- ranking changes;
- redesign of source-card grouping;
- Windows support;
- connection-time DNS pinning;
- broad refactoring of the metasearch adapter;
- changes to already-closed Git subprocess or local filesystem safety work;
- performance optimization unrelated to the affected paths.

Refactoring is allowed only when necessary to make provider outcomes explicit and testable.

---

# Workstream A — Correct Capability Outcome Semantics

## A.1 Replace the boolean skip model

Replace `DispatchJob.skip_not_applicable: bool` with a typed capability disposition.

Recommended shape:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CapabilityDisposition {
    FullySupported,
    PartiallySupported {
        supported_roles: Vec<EvidenceRole>,
        unsupported_roles: Vec<EvidenceRole>,
    },
    Unsupported {
        unsupported_roles: Vec<EvidenceRole>,
    },
    NotApplicable {
        roles: Vec<EvidenceRole>,
        reason: NotApplicableReason,
    },
}
```

A smaller equivalent design is acceptable, but it must preserve all four states and the exact role subsets.

Do not encode this through booleans or inferred string messages.

## A.2 Partition intended roles deterministically

Add a pure helper:

```rust
fn partition_roles_for_engine(
    engine: &dyn SearchEngine,
    intended_roles: &[EvidenceRole],
) -> RoleCapabilityPartition
```

Required properties:

- preserves input order for supported roles;
- preserves input order for unsupported roles;
- removes duplicate roles;
- does not mutate the original subquery;
- returns no role in both sets;
- union of the sets equals the deduplicated input set;
- an empty role list is resolved through existing deterministic role mapping before partitioning.

## A.3 Emit separate attempts for partial support

For a partially supported job:

1. dispatch one actual provider call with only `supported_roles` as its `intended_roles`;
2. emit one synthetic capability-skip attempt covering `unsupported_roles`;
3. do not mark the dispatched attempt as incomplete merely because a separate unsupported subset exists;
4. ensure retrieval summaries expose both dimensions.

Because two attempt records may share a provider and subquery, add a stable attempt identifier or operation discriminator.

Recommended additive fields:

```rust
pub struct RetrievalAttempt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_kind: Option<RetrievalOperationKind>,
    // existing fields
}
```

`attempt_id` should be deterministic from operation kind, provider ID, subquery ID, and a bounded role-set representation. It must not include raw query text.

If the implementation avoids adding `attempt_id`, it must still guarantee that attempts are not deduplicated solely by `(provider_id, subquery_id)`.

## A.4 Emit the correct outcome

Use:

```rust
RetrievalAttemptOutcome::SkippedCapabilityUnavailable
```

for unsupported roles.

Use `NotApplicable` only when request semantics make an operation unnecessary.

Use `SkippedByPolicy` only for explicit policy, configuration, health suppression, cooldown, or routing decisions that intentionally prevent an otherwise supported operation.

## A.5 Preserve role-level coverage behavior

Capability-skipped required roles must remain unresolved. They must not be counted as complete.

Expected mapping:

| Attempt outcome | Coverage interpretation |
|---|---|
| `SuccessWithResults` | attempted and evidence found |
| `SuccessZeroResults` | attempted, no matching evidence found |
| `SkippedCapabilityUnavailable` | indeterminate because capability was unavailable |
| `SkippedByPolicy` | indeterminate because retrieval was not performed |
| `NotApplicable` | does not count against coverage |
| `Failed` / `TimedOut` / `RateLimited` / deadline | indeterminate due to retrieval failure |
| confirmed truncation | partial evidence; not complete for completeness-sensitive roles |

## A.6 Tests

Add or extend tests for:

1. one supported role;
2. one unsupported role;
3. two supported roles;
4. two unsupported roles;
5. one supported and one unsupported role;
6. duplicate input roles;
7. empty roles using fallback mapping;
8. partial support with successful provider result;
9. partial support with provider failure;
10. partial support with zero results;
11. capability skip on a required workflow role;
12. capability skip on an optional workflow role;
13. `NotApplicable` remaining distinct from capability skip;
14. serialization and deserialization of all new fields;
15. deterministic attempt ordering under parallel completion.

### A acceptance criteria

- [ ] No production path uses `NotApplicable` to represent provider incapability.
- [ ] A partially capable provider still executes work for supported roles.
- [ ] Unsupported roles appear in a distinct capability-skip attempt.
- [ ] Retrieval summaries count capability skips correctly.
- [ ] Required capability-skipped roles are not reported complete.
- [ ] No role is silently dropped.
- [ ] Tests prove role partitioning is deterministic and duplicate-safe.
- [ ] Existing codegg response parsing remains compatible through additive fields only.

---

# Workstream B — Provider-Scoped Native Advisory Operations

## B.1 Add explicit advisory capabilities

The default trait methods currently return empty results, conflating “unsupported” with “supported but no match.” Add explicit capability declarations.

Recommended shape:

```rust
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AdvisoryCapabilities {
    pub lookup_by_id: bool,
    pub query_by_package: bool,
}

pub trait SearchEngine: Send + Sync {
    fn advisory_capabilities(&self) -> AdvisoryCapabilities {
        AdvisoryCapabilities::default()
    }

    // existing methods
}
```

Override capabilities only on engines that implement the operation.

At minimum, audit and explicitly classify:

- OSV;
- GitHub Advisory;
- RustSec;
- NVD, if it implements native lookup;
- any future advisory engine already present in the repository.

Do not infer advisory support from provider-name substrings.

## B.2 Introduce provider-scoped outcome types

Replace aggregate adapter methods with provider-scoped operations.

Recommended types:

```rust
#[derive(Clone, Debug)]
pub enum NativeAdvisoryOperation {
    LookupById { vulnerability_id: String },
    QueryByPackage {
        ecosystem: String,
        package: String,
        version: Option<String>,
    },
}

#[derive(Debug)]
pub struct ProviderAdvisoryOutcome<T> {
    pub provider_id: String,
    pub operation: NativeAdvisoryOperation,
    pub result: Result<T, EngineError>,
    pub duration_ms: u64,
}
```

For ID lookup, `T` may be `Option<VulnerabilityMetadata>`. For package query, `T` may be `Vec<VulnerabilityMetadata>`.

Equivalent names are acceptable. The essential requirement is one outcome per provider operation.

## B.3 Constrain provider selection

Add provider-scoped adapter entry points that take an explicit allowed provider set:

```rust
pub async fn lookup_advisory_scoped(
    &self,
    allowed_provider_ids: &[String],
    vulnerability_id: &str,
) -> Vec<ProviderAdvisoryOutcome<Option<VulnerabilityMetadata>>>;
```

```rust
pub async fn query_advisories_by_package_scoped(
    &self,
    allowed_provider_ids: &[String],
    ecosystem: &str,
    package: &str,
    version: Option<&str>,
    max_results: usize,
) -> Vec<ProviderAdvisoryOutcome<Vec<VulnerabilityMetadata>>>;
```

Rules:

- empty allowed set means the already-resolved enabled provider set, not every known provider;
- explicit request providers mean only those providers;
- routed provider decisions must be applied before native operations;
- unsupported engines produce capability-skip attempts without invoking default no-op methods;
- unknown providers remain a request validation concern and are not silently ignored;
- provider cooldown or operator suppression produces policy-skip attempts where applicable.

## B.4 Preserve every provider outcome

For every capable selected provider:

- `Ok(Some(metadata))` → success with one result;
- `Ok(None)` → successful zero-result lookup;
- `Ok(nonempty package list)` → success with result count;
- `Ok(empty package list)` → successful zero-result lookup;
- timeout → timed out;
- HTTP 429 → rate limited;
- other errors → failed with coarse error class;
- global deadline → interrupted by deadline.

Do not use `Err(_) => continue` in provider-scoped advisory code.

Do not convert all-provider failure into `Ok(None)` or an empty vector.

## B.5 Obtain provider identity from execution

Security orchestration must never hardcode provider identity based only on identifier format.

Examples of forbidden assumptions:

- CVE lookup always equals `osv`;
- GHSA lookup always equals `github_advisory`;
- RustSec ID lookup always equals `rustsec`.

The same identifier may be served by multiple advisory providers. Each actual provider attempt must be recorded with `engine.name()` or another canonical provider identifier supplied by the adapter.

## B.6 Separate result aggregation from attempt recording

The orchestration sequence should be:

1. resolve selected advisory-capable providers;
2. execute provider-scoped operations;
3. convert every outcome into a `RetrievalAttempt`;
4. collect successful metadata results;
5. deduplicate vulnerability records by normalized advisory identity;
6. preserve all attempts even when successful records deduplicate;
7. build coverage and retrieval summaries from attempts;
8. build vulnerability groups from deduplicated records.

Deduplication must never erase provider provenance from the retrieval ledger.

## B.7 ID lookup planning

Build explicit operations for each parsed identifier:

- CVE;
- GHSA;
- OSV;
- RustSec;
- other supported advisory identifiers.

A provider may support more than one identifier family. Capability and provider APIs, not identifier-prefix hardcoding, determine whether it is queried.

Add a bounded operation count. The existing request timeout and global deadline must remain authoritative.

## B.8 Package advisory planning

Package advisory queries target at least:

- `AuthoritativeSecurityAdvisory`;
- `ManifestOrDependencyMetadata` where applicable.

Apply the partial capability rules from Workstream A. If an engine can query advisories but cannot provide dependency metadata, execute the advisory role and record the unsupported metadata role separately.

## B.9 KEV attempt semantics

KEV is a separate provider operation and should retain its current explicit provider identity.

Add an explicit `NotApplicable` attempt when all of the following are true:

- KEV enrichment was requested;
- no CVE identifier is available after advisory resolution;
- the lookup therefore genuinely does not apply.

Do not emit `NotApplicable` when a CVE exists but KEV capability or policy prevents the lookup.

## B.10 Tests

Use mock engines with explicit advisory capabilities.

Required scenarios:

1. provider A success, provider B zero results;
2. provider A failure, provider B success;
3. provider A timeout, provider B success;
4. provider A rate limit, provider B zero results;
5. all capable providers fail;
6. all capable providers return zero results;
7. no selected provider supports advisory lookup;
8. explicit provider set excludes a capable enabled provider;
9. explicit provider set includes exactly one provider;
10. provider returns duplicate advisory already returned by another provider;
11. duplicate advisory result deduplicates while both attempts remain;
12. CVE served by non-OSV mock provider records the real provider ID;
13. GHSA served by non-GitHub mock provider records the real provider ID;
14. package query returns partial capability;
15. request deadline interrupts multiple provider operations;
16. unsupported provider method is never invoked;
17. KEV requested without CVE emits true not-applicable status;
18. capability and failure counters remain distinct;
19. codegg fixture can distinguish no match from all-provider failure;
20. raw query text does not appear in serialized attempts.

### B acceptance criteria

- [ ] No native advisory adapter function suppresses provider errors with `continue`.
- [ ] Every capable selected provider produces an observable terminal outcome.
- [ ] Provider IDs in attempts come from the actual engine.
- [ ] Explicit provider routing constrains native advisory operations.
- [ ] All-provider failure cannot serialize as successful zero results.
- [ ] Unsupported operations are capability skips, not zero-result successes.
- [ ] Vulnerability deduplication does not delete attempt records.
- [ ] Package queries preserve all intended role outcomes.
- [ ] KEV not-applicable behavior is semantically correct.

---

# Workstream C — Evidence-Based Truncation Semantics

## C.1 Add truncation evidence

Add an additive enum:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TruncationEvidence {
    None,
    LimitReachedUnknown,
    ConfirmedByEggsearch,
    ConfirmedByProvider,
}
```

Add it to `RetrievalAttempt` and `RetrievalDimensionStatus` with a default that preserves compatibility.

The existing `truncated: bool` may remain for compatibility, but it must be derived as:

```text
true only for ConfirmedByEggsearch or ConfirmedByProvider
```

`LimitReachedUnknown` must not set confirmed truncation.

## C.2 Reclassify candidate-limit saturation

When a provider returns exactly `candidate_limit` and does not expose additional-page or truncation metadata:

- outcome remains `SuccessWithResults`;
- `result_count` equals the returned count;
- truncation evidence equals `LimitReachedUnknown`;
- `truncated` remains false;
- retrieval summary exposes a separate limit-reached counter or dimension field.

Use `TruncatedAfterPartialSuccess` only when:

- Eggsearch had additional records and dropped them because of a hard cap;
- a provider explicitly reports additional pages or truncation;
- a bounded read or response transformation demonstrably removed data after partial success.

## C.3 Summary counters

Add an additive counter:

```rust
pub limit_reached_unknown_count: Option<usize>
```

Keep `truncated_count` limited to confirmed truncation.

## C.4 Tests

Required scenarios:

- below candidate limit;
- exactly candidate limit without provider metadata;
- above internal final cap after known extra results;
- provider explicitly reports next page;
- zero results;
- confirmed body truncation;
- summary containing both unknown limit reach and confirmed truncation;
- old payload without new enum deserializes with default behavior.

### C acceptance criteria

- [ ] Exact candidate-limit saturation is no longer labeled confirmed truncation.
- [ ] Confirmed truncation remains visible through the existing boolean.
- [ ] Possible truncation is represented separately.
- [ ] Coverage does not mark a role incomplete solely because the provider returned exactly the requested limit without proof of missing data.
- [ ] Compatibility fixtures cover old and new payload forms.

---

# Workstream D — Native Forge Workflow Must Fail Closed

## D.1 Separate diagnostic and release workflows

The scheduled workflow may remain a diagnostic signal, but release evidence must use an explicit manual workflow with a required release-subject SHA.

Recommended workflow input:

```yaml
on:
  workflow_dispatch:
    inputs:
      release_subject:
        description: Full commit SHA to verify
        required: true
        type: string
```

The workflow must:

1. validate that the input is a full 40-character hexadecimal commit SHA;
2. check out exactly that SHA;
3. verify `git rev-parse HEAD` equals the input;
4. include that SHA in every evidence artifact;
5. never silently substitute the triggering branch head.

## D.2 Inject every required credential and fixture variable

Configure provider jobs explicitly:

### GitHub

```yaml
env:
  GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
  GITHUB_SLASH_REF: ${{ vars.NATIVE_SMOKE_GITHUB_SLASH_REF }}
```

### GitLab

```yaml
env:
  GITLAB_TOKEN: ${{ secrets.GITLAB_TOKEN }}
```

### Codeberg

```yaml
env:
  CODEBERG_TOKEN: ${{ secrets.CODEBERG_TOKEN }}
```

### Gitea

```yaml
env:
  GITEA_TOKEN: ${{ secrets.GITEA_TOKEN }}
  GITEA_INSTANCE_URL: ${{ vars.GITEA_INSTANCE_URL }}
```

Exact secret names may differ, but they must be documented and consistently used by workflow and tests.

## D.3 Preflight required configuration

Before running provider tests, each release job must fail if required configuration is missing.

Do not treat a missing release credential or fixture as a skip.

Example:

```bash
test -n "$GITLAB_TOKEN" || {
  echo "GITLAB_TOKEN is required for release evidence" >&2
  exit 2
}
```

The diagnostic scheduled workflow may mark unavailable providers as skipped, but the release workflow must fail closed.

## D.4 Prove that tests executed

A zero exit code is insufficient because ignored tests or token-gated early returns can produce false success.

Implement one of these designs:

### Preferred design: structured evidence output

The native smoke test writes a provider evidence JSON file only after all provider assertions pass.

Environment variable:

```text
EGGSEARCH_NATIVE_SMOKE_EVIDENCE_DIR
```

Required fields:

```json
{
  "schema_version": 1,
  "release_subject": "full-sha",
  "provider": "gitlab",
  "target": "gitlab-org/gitlab-runner",
  "requested_ref": "main",
  "resolved_ref": "main",
  "resolved_commit_sha": "full-sha",
  "mode": "native",
  "entry_count": 123,
  "request_count": 4,
  "response_bytes_observed": 12345,
  "aggregate_limit": 1048576,
  "provenance_pinned": true,
  "result": "pass",
  "executed_at": "RFC3339"
}
```

The workflow must fail if the file is absent, malformed, has the wrong release subject, or does not contain `result=pass` and `mode=native`.

### Alternative design

Use a dedicated test harness binary that exits nonzero unless at least one named provider assertion executed and passed. It must produce equivalent structured output.

Do not parse human-readable `cargo test` text as the sole execution proof.

## D.5 Stable slash-ref fixture

Remove the implicit `smoke/slash-ref` default.

Use a deliberately provisioned immutable fixture ref. Preferred choices:

1. an immutable tag containing a slash in a dedicated public fixture repository;
2. a protected fixture branch in an Eggstack-owned fixture repository;
3. a documented existing slash-containing ref with an integrity check.

The workflow variable must name the exact fixture ref. The job must preflight it before the test.

The test must assert:

- requested ref contains `/`;
- requested ref is preserved in response identity;
- resolved commit SHA is a full commit SHA;
- native mode was used;
- provenance is pinned;
- response budget counters are valid.

Do not rely on an external project maintaining a branch solely for Eggsearch testing.

## D.6 Summary job semantics

The release summary job passes only if all required provider evidence results are exactly `pass`.

Fail on:

- `fail`;
- `skip`;
- missing output;
- cancelled job;
- provider job not run;
- malformed evidence;
- release-subject mismatch;
- fallback mode;
- zero entries;
- missing resolved commit;
- response bytes exceeding aggregate limit.

## D.7 Workflow artifacts

Upload:

- provider evidence JSON;
- bounded test log;
- combined manifest containing all provider evidence hashes;
- optional benchmark artifact for native forge request counts and bytes.

Set an explicit retention period appropriate for release evidence.

The combined manifest should include SHA-256 hashes of each provider evidence file.

## D.8 Workflow contract tests

Add repository tests that parse `.github/workflows/native-forge-smoke.yml` and verify:

- release subject input exists;
- exact-SHA checkout is used;
- all required secret variables are mapped;
- slash-ref variable has no unsafe default;
- summary rejects skip and missing outputs;
- evidence artifacts are required;
- provider jobs run the expected test filter;
- diagnostic and release semantics are not conflated.

Use a YAML parser or bounded structural checks. Avoid brittle line-number assertions.

### D acceptance criteria

- [ ] GitHub, GitLab, Codeberg, and Gitea jobs receive required configuration.
- [ ] Missing release configuration fails the workflow.
- [ ] Every provider job proves a native assertion executed.
- [ ] Skipped provider coverage cannot produce a green release workflow.
- [ ] Slash-ref coverage uses a stable provisioned fixture.
- [ ] Evidence files identify the exact release subject and resolved provider commit.
- [ ] The summary job requires exact pass results for all required providers.
- [ ] Workflow contract tests prevent regression to false-green behavior.

---

# Workstream E — Refresh Affected-Path Performance Evidence

## E.1 Remove unsupported claims

Do not claim “no unbounded memory growth” unless supported by a named measurement and result.

Do not call pending benchmarks completed.

The verification document must clearly distinguish:

- measured;
- compile-checked only;
- pending;
- not applicable.

## E.2 Required affected-path benchmarks

Add or complete benchmarks for:

1. capability partitioning with 1, 4, 16, and 64 roles;
2. retrieval-summary construction with mixed success, zero-result, capability-skip, policy-skip, failure, deadline, and truncation outcomes;
3. provider-scoped advisory outcome conversion for 1, 4, 8, and 16 providers;
4. vulnerability deduplication while preserving attempt records;
5. package advisory fanout with multi-role attempts;
6. forge response construction at representative entry counts;
7. local inventory search near configured cap if it remains part of the verification document.

## E.3 Memory-bound evidence

Use deterministic bounded-input checks rather than vague process-wide observations.

At minimum record:

- maximum number of provider operations per request;
- maximum number of retrieval attempts generated;
- maximum retained advisory records before and after deduplication;
- maximum serialized retrieval-summary size under configured caps;
- forge aggregate byte cap and observed bytes in native smoke evidence.

A heap profiler is optional. If no heap profiler is used, describe the evidence as **bounded-size analysis and stress-test evidence**, not a proof of zero memory growth.

## E.4 Tests

- property test that attempt count cannot exceed planned bounded operations plus synthetic capability attempts;
- property test that serialization size grows linearly under bounded role and provider caps;
- stress test with maximum configured providers and subqueries;
- benchmark compilation in CI;
- benchmark-result artifact generated for the release subject.

### E acceptance criteria

- [ ] No pending benchmark is described as completed.
- [ ] Every performance claim names a command or artifact.
- [ ] Affected capability and advisory paths have measured baselines.
- [ ] Memory language accurately reflects the evidence obtained.
- [ ] Release artifacts preserve benchmark results for the release subject.

---

# Workstream F — Establish a Truthful Release Subject and Evidence Commit

## F.1 Complete code changes before selecting `R`

Do not select a release subject until Workstreams A–E are complete and no code-bearing corrective changes remain.

The final code-bearing commit is `R`.

After selecting `R`:

- no production code changes are permitted before evidence is recorded;
- any production-code fix creates a new `R` and invalidates prior release runs;
- rerunning CI on the old `R` does not validate a later code commit.

## F.2 Required deterministic matrix on `R`

Run against exact `R`:

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo test --locked --no-default-features
cargo test --locked --features mock
cargo test --locked --features pdf
make hardening
make schema-corpus
make docs-tests
RUSTDOCFLAGS="-D warnings" cargo doc --locked --all-features --no-deps
cargo build --locked --release
cargo publish --dry-run --locked
cargo bench --locked --all-features --no-run
```

Also run the repository's fuzz-smoke/property matrix and any platform-specific local-workspace tests.

Linux and macOS runs must identify the same `R`.

## F.3 Required targeted suites

Run and record exact counts for:

- research semantic roles;
- retrieval attempt ledger;
- native security attempts;
- conflict source attribution;
- codegg evidence contract;
- capability partition tests;
- provider-scoped advisory tests;
- truncation semantics tests;
- native forge workflow contract tests;
- bounded command and local filesystem hardening tests.

## F.4 Native forge evidence on `R`

Run the fail-closed native forge release workflow against exact `R`.

Required providers:

- GitHub;
- GitLab;
- Codeberg/Forgejo implementation path;
- independent Gitea instance path.

All provider jobs must pass. A skipped provider does not satisfy the gate.

## F.5 Generate a machine-readable release manifest

Create a generated manifest, for example:

```text
docs/release-evidence/release-R.json
```

Required contents:

- full `R` SHA;
- Rust toolchain;
- operating systems;
- exact command list;
- test counts;
- workflow run IDs;
- native provider artifact names and hashes;
- benchmark artifact name and hash;
- known limitations;
- generation timestamp;
- schema version.

The human-readable release verification document should be generated from or checked against this manifest.

## F.6 Create evidence-only commit `E`

`E` may modify only approved evidence paths, such as:

- `docs/release-verification.md`;
- `docs/release-evidence/**`;
- optionally a release manifest pointer or checksum file.

It must not modify:

- `src/**`;
- `tests/**`;
- `benches/**`;
- `fuzz/**`;
- `Cargo.toml`;
- `Cargo.lock`;
- workflow logic;
- runtime documentation unrelated to evidence.

Verify:

```bash
git diff --name-only R..E
```

The output must contain only approved evidence files.

## F.7 Rewrite `docs/release-verification.md`

The refreshed document must include:

- verification date;
- full release subject `R`;
- full evidence commit `E`;
- exact toolchain;
- exact platform matrix;
- exact command results and test counts;
- exact workflow run IDs;
- native provider evidence artifact identifiers;
- benchmark results or explicit omission;
- known limitations;
- release classification;
- statement that runtime code equals `R`, while the tag may point to evidence-only `E` if that remains project policy.

Remove:

- stale commit IDs;
- contradictory test counts;
- pending rows presented as complete;
- claims not supported by an artifact;
- statements that token-gated tests passed when they actually skipped.

## F.8 Tagging rule

Tag only after `E` passes evidence-scope validation.

The tag annotation must name both:

- runtime subject `R`;
- evidence commit `E`.

If the tag points to `E`, documentation must clearly state that `E` differs from `R` only by evidence files.

### F acceptance criteria

- [ ] `R` is the final code-bearing commit.
- [ ] Full Linux and macOS deterministic matrices pass on exact `R`.
- [ ] Native forge release workflow passes all providers on exact `R`.
- [ ] Exact workflow run IDs and artifact hashes are recorded.
- [ ] `E` changes only approved evidence files.
- [ ] Test counts are internally consistent.
- [ ] No pending measurement is represented as complete.
- [ ] Release classification is supported by recorded evidence.

---

# Workstream G — Static Guards and Regression Prevention

## G.1 Provider error-erasure guards

Add source-contract tests preventing these patterns in provider-scoped advisory code:

- `Err(_) => continue`;
- `Err(_) => Ok(None)`;
- all-provider failure returning an empty successful result;
- hardcoded provider IDs in outcome construction when an engine ID is available.

Static guards are supplementary. Behavioral mock tests remain authoritative.

## G.2 Capability outcome guards

Add guards that fail if:

- `supports_role()` failure maps to `NotApplicable`;
- an `any(!supports_role)` all-or-nothing pattern is reintroduced;
- `SkippedCapabilityUnavailable` has no production constructor path;
- partial support is not covered by tests.

## G.3 Workflow guards

Add guards that fail if the release native-smoke workflow:

- lacks release-subject input;
- does not check out the exact subject;
- omits required provider environment variables;
- treats skip as pass;
- lacks evidence artifact validation;
- contains the unsafe slash-ref default;
- permits summary success with missing provider outputs.

## G.4 Release-document consistency checks

Add a docs test that compares the machine-readable release manifest with `docs/release-verification.md` for:

- `R` SHA;
- test counts;
- run IDs;
- classification;
- provider list;
- benchmark status.

Fail when `pending` appears in a section classified as completed release evidence.

### G acceptance criteria

- [ ] Regression guards cover each defect from this plan.
- [ ] Guards are structural enough to tolerate harmless formatting changes.
- [ ] Behavioral tests still prove runtime semantics.
- [ ] Release documentation cannot drift silently from the evidence manifest.

---

# Workstream H — codegg Compatibility Verification

## H.1 Additive schema policy

Changes to retrieval attempts and dimensions must be additive wherever possible.

Do not remove or rename existing fields during this pass.

Retain:

- `truncated: bool`;
- existing outcome strings;
- existing absence-kind strings;
- existing summary counters.

New fields such as `attempt_id`, `operation_kind`, `truncation_evidence`, and `limit_reached_unknown_count` should have serde defaults and omission rules.

## H.2 Required codegg fixtures

Add fixtures proving that a codegg-style consumer can distinguish:

1. no matching advisory;
2. all advisory providers failed;
3. one provider failed and another succeeded;
4. provider capability unavailable;
5. provider skipped by policy;
6. true not-applicable operation;
7. partial role support;
8. possible truncation versus confirmed truncation;
9. native provider identity;
10. exact per-role failure expansion.

## H.3 Consumer guidance

Update agent workflow documentation with concise rules:

- do not treat zero results as provider failure;
- do not treat capability skip as absence;
- do not treat `limit_reached_unknown` as confirmed truncation;
- use provider-scoped attempts for provenance and diagnostics;
- treat required-role policy or capability skips as indeterminate coverage;
- native security facts remain advisory evidence, not exploitability conclusions.

### H acceptance criteria

- [ ] Existing codegg fixtures continue to deserialize.
- [ ] New fixtures exercise every new semantic distinction.
- [ ] No breaking field removal or enum rename occurs.
- [ ] Documentation gives agents deterministic interpretation rules.

---

## 5. Recommended Implementation Sequence

### Commit 1 — Capability partition and outcomes

Implement Workstream A and associated tests.

Suggested message:

```text
fix(retrieval): distinguish capability skips and partial role support
```

Gate before proceeding:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features retrieval
cargo test --all-features capability
```

### Commit 2 — Provider-scoped advisory operations

Implement Workstream B and native security tests.

Suggested message:

```text
fix(security): preserve provider-scoped advisory outcomes
```

Gate:

```bash
cargo test --all-features --test native_security_attempts
cargo test --all-features --test retrieval_attempt_ledger
cargo test --all-features --test codegg_evidence_contract
```

### Commit 3 — Truncation semantics

Implement Workstream C and compatibility fixtures.

Suggested message:

```text
fix(retrieval): separate limit saturation from confirmed truncation
```

### Commit 4 — Native forge fail-closed workflow

Implement Workstream D and workflow contract tests.

Suggested message:

```text
ci: make native forge release evidence fail closed
```

### Commit 5 — Benchmarks, guards, and documentation behavior

Implement Workstreams E, G, and H, excluding final release evidence.

Suggested message:

```text
test: add final retrieval and release-evidence regression gates
```

### Commit 6 — Release subject `R`

Apply any final formatting-only or test-fix changes, run the full local deterministic matrix, and designate the resulting code-bearing commit as `R`.

Suggested message if a final consolidation commit is needed:

```text
chore(release): finalize provider attribution closure subject
```

Do not edit release evidence after this commit until CI and native smoke complete.

### Commit 7 — Evidence commit `E`

After all required runs pass on `R`, generate the evidence manifest and rewrite the verification record.

Suggested message:

```text
docs(release): record verified release subject and evidence
```

Verify the `R..E` diff scope before tagging.

---

## 6. Detailed Test Matrix

### Unit and property tests

| Area | Required proof |
|---|---|
| role partition | supported/unsupported sets are complete, disjoint, ordered, deduplicated |
| capability outcome | unsupported maps to capability skip, never not-applicable |
| partial support | supported call executes and unsupported roles remain visible |
| provider outcome | every selected capable provider produces one terminal result |
| error preservation | all-provider failure remains failure |
| provider routing | explicit provider list constrains execution |
| result dedup | attempts remain after advisory deduplication |
| truncation | exact limit is unknown saturation, not confirmed truncation |
| summary counters | job, role, failure, skip, and truncation counts remain consistent |
| query privacy | raw query and tokens absent from serialized attempts |
| ordering | output deterministic across completion order |

### Integration tests

| Scenario | Expected result |
|---|---|
| OSV fails, GitHub advisory succeeds | two attempts; one failure, one success; advisory returned |
| all advisory providers fail | no false zero-result success; required role indeterminate |
| no provider supports lookup | capability-skip dimensions; no provider call |
| package query partial support | advisory role executed; metadata role capability-skipped |
| explicit provider subset | excluded provider has no execution attempt |
| KEV requested without CVE | true not-applicable KEV attempt |
| candidate limit reached | limit-reached-unknown, `truncated=false` |
| known extra results dropped | confirmed truncation, `truncated=true` |
| codegg payload | all fields deserialize and preserve semantic distinctions |

### CI contract tests

| Contract | Expected result |
|---|---|
| missing GitLab token | release native-smoke job fails |
| provider test returns without evidence file | job fails |
| evidence subject differs from input `R` | job fails |
| provider result equals skip | summary fails |
| slash-ref variable missing | GitHub slash-ref job fails preflight |
| native response uses fallback mode | provider job fails |
| all provider evidence valid | summary passes |

### Platform matrix

- Ubuntu Linux stable toolchain;
- macOS stable toolchain;
- Linux nightly fuzz smoke where configured;
- no Windows claim or gate added by this pass.

---

## 7. Failure Injection Requirements

Add deterministic mocks for:

- advisory timeout;
- HTTP 429;
- HTTP 500;
- parse failure;
- network failure;
- task panic;
- global deadline before dispatch;
- global deadline during provider call;
- provider success with zero results;
- provider success with duplicate advisory;
- provider capability unavailable;
- provider policy suppression;
- partial role support;
- missing native-smoke evidence file;
- malformed native-smoke JSON;
- wrong release-subject SHA;
- fallback-mode native response;
- slash-ref fixture not found.

Tests must not depend on external network access except the explicitly ignored/manual native release-smoke suite.

---

## 8. Compatibility and Migration Rules

1. All response schema changes should be additive.
2. New enum fields require serde defaults.
3. Existing `truncated` remains supported.
4. Existing retrieval outcomes retain their serialized names.
5. Existing codegg fixtures remain valid.
6. Provider IDs remain stable canonical engine IDs.
7. No raw query text enters the attempt ledger.
8. Existing security warnings may be retained, but structured attempts are authoritative.
9. Release-document generation must not require consumers to read GitHub Actions APIs at runtime.

---

## 9. Observability Requirements

Add bounded tracing for:

- advisory operation count;
- provider ID;
- operation kind;
- outcome class;
- duration;
- result count;
- supported-role count;
- unsupported-role count;
- global deadline interruption;
- limit-reached-unknown;
- confirmed truncation.

Do not log:

- API keys;
- authorization headers;
- raw proprietary query text;
- full dependency file contents;
- unbounded provider error bodies.

Use query fingerprints and coarse error classes.

---

## 10. Documentation Updates

Update only relevant documents:

- `AGENTS.md` invariants;
- evidence/retrieval section of `docs/agent-workflows.md`;
- `docs/architecture/meta.md` for provider-scoped advisory design;
- `docs/safety.md` for failure visibility and query privacy;
- `docs/release-verification.md` only in evidence commit `E`;
- a generated machine-readable release evidence manifest.

Document these invariants explicitly:

- unsupported capability is not not-applicable;
- native advisory provider identity is execution-derived;
- advisory provider errors are never discarded;
- explicit provider routing constrains native operations;
- candidate-limit saturation is not proof of truncation;
- release-native-smoke skips do not satisfy release gates;
- release evidence names a code-bearing subject separate from its evidence-only commit.

---

## 11. Rollback Guidance

### Capability changes

If partial role dispatch introduces regressions, do not revert to all-or-nothing skipping. Temporarily dispatch only fully supported jobs and emit capability skips for unsupported or partial jobs while correcting partition logic.

### Advisory changes

If provider-scoped fanout introduces instability, retain the provider-outcome types and reduce concurrency. Do not restore aggregate APIs that erase errors.

### Truncation changes

If consumers require time to adopt new fields, keep `truncated=false` for unknown saturation and expose the new evidence field additively. Do not return to confirmed-truncation overstatement.

### Workflow changes

If provider secrets are unavailable, keep the repository provisional. Do not weaken release jobs to permit skips.

### Release evidence

If any post-`R` code fix is required, abandon the old evidence run, select a new `R`, and rerun all gates.

---

## 12. Definition of Done

### Runtime correctness

- [ ] Capability unavailable and not applicable are distinct in production.
- [ ] Partial role support executes supported work.
- [ ] Unsupported roles remain visible as capability skips.
- [ ] Native advisory provider identity is execution-derived.
- [ ] Every selected capable provider produces a terminal outcome.
- [ ] Provider errors are not swallowed.
- [ ] Explicit routing constrains native advisory providers.
- [ ] All-provider failure cannot appear as no-match success.
- [ ] Advisory deduplication preserves attempt provenance.
- [ ] Unknown limit saturation is not confirmed truncation.

### Tests

- [ ] Unit, property, integration, and codegg contract tests cover all new semantics.
- [ ] Failure injection covers provider and workflow failure modes.
- [ ] Static guards prevent reintroduction of error erasure and false capability semantics.
- [ ] Linux and macOS matrices pass.
- [ ] Fuzz targets or property equivalents cover role partition, provider outcomes, and summary construction.

### Native forge evidence

- [ ] Release workflow checks out exact `R`.
- [ ] All provider credentials and fixture variables are explicitly mapped.
- [ ] Missing configuration fails closed.
- [ ] Each provider produces structured execution evidence.
- [ ] GitHub slash-ref test uses a stable provisioned fixture.
- [ ] GitHub, GitLab, Codeberg, and Gitea all pass in native mode.
- [ ] Summary rejects skips, missing outputs, malformed evidence, and subject mismatch.

### Release proof

- [ ] A final code-bearing release subject `R` is selected.
- [ ] Full deterministic CI passes on exact `R`.
- [ ] Native forge release evidence passes on exact `R`.
- [ ] A machine-readable release manifest records exact runs and artifacts.
- [ ] Evidence-only commit `E` changes only approved evidence files.
- [ ] Verification documentation is internally consistent.
- [ ] No unsupported performance or memory claim remains.
- [ ] Release classification is upgraded only after all gates pass.

---

## 13. Handoff Checklist

Before implementation:

- [ ] Read this plan and the prior retrieval-ledger closure plan.
- [ ] Confirm baseline commit and inspect newer commits before editing.
- [ ] Inventory all advisory-capable engines and their actual operations.
- [ ] Decide the stable slash-ref fixture and provision it before workflow verification.
- [ ] Confirm repository secret and variable names.

During implementation:

- [ ] Keep commits scoped by workstream.
- [ ] Add tests before deleting aggregate advisory methods.
- [ ] Preserve additive schema compatibility.
- [ ] Avoid logging raw queries or credentials.
- [ ] Run targeted suites after each commit.

Before selecting `R`:

- [ ] Complete all code-bearing work.
- [ ] Run the full local deterministic matrix.
- [ ] Confirm no pending runtime TODO remains from this plan.
- [ ] Confirm workflow contract tests pass.
- [ ] Confirm benchmark rows have actual results or are explicitly omitted.

Before creating `E`:

- [ ] Run full CI on exact `R`.
- [ ] Run fail-closed native forge workflow on exact `R`.
- [ ] Download and hash all required artifacts.
- [ ] Generate the machine-readable release manifest.
- [ ] Verify every recorded run ID targets `R`.

Before tagging:

- [ ] Verify `git diff --name-only R..E` contains evidence files only.
- [ ] Verify release document counts match the manifest.
- [ ] Verify all native provider results are pass, not skip.
- [ ] Verify the tag annotation names both `R` and `E`.

---

## 14. Final Closure Decision

This pass is closed only when there are no remaining known cases where Eggsearch can:

- mislabel provider incapability as not-applicable;
- drop supported work because another role is unsupported;
- attribute an advisory result to a provider that did not produce it;
- erase provider failures behind a later success or empty aggregate result;
- query native advisory providers outside the routed request scope;
- claim confirmed truncation based only on reaching a requested limit;
- report native provider release coverage when the test skipped;
- publish release evidence for a commit other than the final code-bearing subject.

Once these conditions and the full Definition of Done are satisfied, the retrieval-ledger and release-proof hardening line can be considered complete.