# Final Keyless Proof and Release-Chain Corrective Closure

**Repository:** `eggstack/eggsearch`  
**Baseline:** `c1fdbffa8876f71797dbe05f3cb29217bbd18880`  
**Status:** Small-model implementation handoff  
**Scope:** Final narrow corrective closure  
**Primary consumer:** codegg and other MCP agent hosts  
**Release invariant:** useful core operation requires no API keys  
**Evidence invariant:** no code, test, workflow, schema, or contract change may follow the final release subject `R`

---

## 1. Objective

Most of the keyless-core and retrieval-accounting work has landed. This plan closes only the defects that still prevent the line of work from being considered complete:

1. the recorded release subject/evidence pair is invalid because production code and tests were committed after evidence commit `E`;
2. several keyless-core tests are tautological or do not exercise the condition named by the test;
3. the optional native-adapter summary workflow attempts to dynamically dereference GitHub `needs` data through invalid Bash variable indirection;
4. retrieval role and dimension accounting still treats authoritative `NotApplicable` dimensions as attempted or completed;
5. the release record lacks runtime benchmark evidence tied to the final code subject;
6. the final evidence sequence has not been rerun against a new immutable `R`.

The desired final state is:

```text
keyless core behavior          proven by deterministic behavioral tests
optional credentialed adapters isolated and claim-scoped
retrieval summary semantics    internally consistent and documented
adapter workflow               valid for one or many selected adapters
release subject R              final code-bearing commit
release evidence E             one terminal evidence-only commit after R
```

This is not another architecture phase. Do not expand provider inventory, ranking, fetch conversion, MCP tools, security analysis, or documentation scope beyond the exact closure items in this plan.

---

## 2. Immediate Release-State Correction

Before implementing any other gate, correct the repository's internal release status.

The currently recorded pair is invalid:

```text
recorded R = 2268971087beb5f54bf6244da159ff97a913a7bf
recorded E = 6fea3c2d41a74a90469a3a2260c816b962df0b45
current head = c1fdbffa8876f71797dbe05f3cb29217bbd18880
```

`c1fdbffa...` contains production code and test changes after `E`. Therefore:

- the old `R` does not represent current code;
- the old `E` is not terminal;
- the old evidence may remain historical context but must not be presented as current release proof;
- no current release subject or evidence commit exists until this plan is completed.

Update `docs/release-verification.md` in the first implementation commit to state:

```text
Classification: provisional release candidate
Current release subject R: not selected
Current evidence commit E: not created
Previous invalidated subject: 2268971087beb5f54bf6244da159ff97a913a7bf
Invalidation reason: code and tests changed after the recorded evidence commit
```

Do not delete historical run IDs. Move them to an explicitly labeled section such as:

```text
Historical superseded evidence — not valid for current head
```

### Immediate-state acceptance criteria

- [ ] The repository no longer presents `226897...` as the current `R`.
- [ ] The repository no longer presents `6fea3c2...` as a valid current `E`.
- [ ] Historical CI information is clearly marked superseded.
- [ ] Classification remains `provisional release candidate`.
- [ ] No replacement `R` is selected before all implementation gates pass.

---

## 3. Non-Goals

This plan does **not** authorize:

- new providers;
- removing optional providers;
- changing MCP tool names;
- new mandatory API keys;
- requiring GitHub, GitLab, Codeberg, Gitea, Forgejo, Sourcegraph, Brave API, or Semantic Scholar credentials;
- making SearXNG mandatory;
- changing the default keyless provider list unless a failing behavioral test proves it is necessary;
- new public schema fields unless needed to resolve an unavoidable accounting ambiguity;
- provider-trait redesign;
- broad test cleanup;
- broad documentation rewriting;
- dependency upgrades unrelated to deterministic testing;
- treating optional adapter conformance as a core release gate;
- accepting compile-only benchmarks as runtime benchmark evidence;
- creating evidence commit `E` before all exact-`R` evidence exists.

---

## 4. Small-Model Execution Rules

Follow these rules exactly:

1. Work in gate order.
2. Add or correct focused tests before changing implementation behavior.
3. A test must fail when the claimed behavior is broken.
4. Do not use assertions equivalent to `result.is_ok() || result.is_err()`.
5. Do not name a test “mixed providers” unless the actual request contains both a keyless and a credentialed provider.
6. Do not test missing credentials by deleting the credentialed provider configuration entirely.
7. Environment-mutating tests must be serialized or isolated in child processes.
8. Preserve provider-scoped skip telemetry.
9. Do not silently substitute a fallback provider for explicit unavailable-only selection.
10. Preserve the distinction between native forge evidence and generic web evidence.
11. Keep optional adapter workflows fail-closed for every selected adapter.
12. Select `R` only after code, tests, workflows, schemas, docs, and benchmark definitions are final.
13. Any change to code, tests, workflow, config, schema, benchmark definition, or contract after `R` invalidates `R`.
14. `E` may modify only approved evidence/documentation paths.
15. If a release-evidence defect is found after `E`, create a new evidence-only commit only if `R` remains unchanged; if code or workflow must change, select a new `R` and rerun everything.

---

# Gate A — Replace Tautological Keyless Tests with Behavioral Proof

## A.1 Required outcome

The keyless-core suite must prove real behavior under these conditions:

```text
no user config
no provider credential values
keyless defaults available
credentialed providers optional and unavailable
```

Tests must assert observable contract outcomes, not merely absence of panic.

## A.2 Add a reusable scrubbed-environment harness

Create a test-only harness that can run closures or child processes with recognized credential variables absent.

Credential variables to scrub at minimum:

```text
GITHUB_TOKEN
GH_TOKEN
GITLAB_TOKEN
GITEA_TOKEN
FORGEJO_TOKEN
CODEBERG_TOKEN
SOURCEGRAPH_API_KEY
BRAVE_API_KEY
SEMANTIC_SCHOLAR_API_KEY
NVD_API_KEY if recognized
```

Requirements:

- preserve original values;
- remove variables during the test;
- restore values after the test, including panic cleanup where possible;
- serialize tests that mutate process environment;
- prefer child-process isolation for end-to-end startup tests;
- never print credential values.

If the repository already has an environment lock/helper, reuse it. Do not add a second incompatible mechanism.

## A.3 Prove actual no-config loading

The existing direct `AppConfig::default()` path does not prove that a missing config file produces defaults.

Add a test that exercises the real configuration loader:

1. create a temporary empty config root;
2. set `XDG_CONFIG_HOME` or the platform-equivalent input used by eggsearch to that directory;
3. ensure `eggsearch/config.toml` does not exist;
4. scrub credential variables;
5. invoke the production configuration-loading path;
6. build server state;
7. assert success;
8. assert enabled defaults are keyless and routable.

Required test name or equivalent:

```text
keyless_missing_config_file_loads_healthy_defaults
```

Do not implement this test by constructing `AppConfig::default()` directly.

## A.4 Prove process-level startup

Add one end-to-end smoke test or release script that launches the built binary in a scrubbed environment using a temporary config root.

Preferred approaches, in order:

### Option 1 — deterministic CLI diagnostic

If `eggsearch doctor` or another noninteractive command builds the complete server state, run it as a child process and assert:

- exit status success;
- no fatal credential error;
- output identifies usable keyless providers;
- output does not contain secret values.

### Option 2 — MCP initialization smoke

Launch:

```bash
eggsearch mcp stdio
```

Send a bounded MCP initialize request over stdin, read the response, and terminate the process. Assert successful initialization.

Use a timeout and guaranteed child cleanup.

### Option 3 — direct production-loader/state integration

Use only if the CLI paths cannot be made deterministic. Document why process-level startup was not practical.

Required acceptance is production loader plus production state construction, not a synthetic default-only unit test.

## A.5 Replace the web-fetch tautology

Remove assertions equivalent to:

```rust
assert!(result.is_ok() || result.is_err());
```

Add a deterministic local HTTP fixture.

Recommended fixture:

1. bind a test server to `127.0.0.1` on an ephemeral port;
2. configure test-only fetch policy with `allow_localhost = true`;
3. serve a small fixed HTML or text response containing a unique marker;
4. call the actual `run_web_fetch` path;
5. assert `Ok`;
6. assert returned content contains the marker;
7. assert extraction mode and bounds remain correct;
8. assert no provider credentials are configured or used.

Example marker:

```text
eggsearch-keyless-fetch-fixture
```

If an existing HTTP fixture/helper exists, reuse it. Do not introduce a network dependency on `httpbin.org` or another public service.

Required tests:

```text
keyless_web_fetch_returns_fixture_content
keyless_batch_fetch_returns_fixture_content
```

The batch-fetch test may be omitted only if a preexisting deterministic batch-fetch integration test already proves the same no-credential path and is referenced in the plan completion notes.

## A.6 Prove enabled credentialed provider with missing environment variable

The current tests clear API configuration. That proves disabled providers do not break startup, not that configured optional providers with missing keys degrade safely.

For each credential configuration family, add a representative test that:

1. starts from the real default config;
2. enables/configures the credentialed provider;
3. sets `api_key_env` to a unique test variable;
4. ensures the variable is absent;
5. builds server state;
6. asserts startup succeeds;
7. inspects `provider_status`;
8. asserts provider is non-routable;
9. asserts `requires_api_key == true`;
10. asserts skip code is `credential_env_missing`, `missing_api_key`, or the exact canonical code documented by the implementation;
11. asserts keyless defaults remain routable.

At minimum cover:

```text
github_code
gitlab_code
gitea_code or forgejo equivalent
sourcegraph
brave_api
semantic_scholar
```

Provider-specific duplication may be reduced with a table-driven test.

Required test pattern:

```text
configured_optional_provider_missing_key_does_not_fail_core(provider_id)
```

## A.7 Prove actual mixed-provider routing

Add a request whose provider list actually contains:

```text
duckduckgo
github_code
```

Test configuration:

- DuckDuckGo uses a deterministic mock engine returning one result;
- GitHub code search is configured as enabled;
- its configured credential environment variable is absent;
- fallback semantics are not needed because an explicit keyless provider is present.

Assert:

- request succeeds;
- DuckDuckGo result is present;
- GitHub provider has a scoped missing-credential skip;
- GitHub is not listed as a provider execution failure;
- response is degraded/partial if the contract uses those flags;
- keyless results are not erased;
- no credential prompt or fatal error appears.

Required test name:

```text
mixed_keyless_and_missing_credential_provider_preserves_keyless_results
```

Do not satisfy this test by requesting only `duckduckgo` and inspecting global provider status afterward.

## A.8 Prove explicit unavailable-only selection

Request only:

```text
github_code
```

with the provider configured but its credential variable absent.

Assert:

- no server crash;
- no silent substitution of DuckDuckGo, Startpage, Yahoo, or other defaults;
- no generic result labeled as GitHub-native;
- structured response or typed error follows the existing MCP contract;
- provider skip code is canonical and machine-readable;
- the server remains usable for subsequent keyless requests.

Required test name:

```text
explicit_missing_credential_provider_is_truthful_and_does_not_fallback
```

If the current tool contract returns a typed error for zero routable explicitly selected providers, retain that behavior and assert the exact error code. Do not redesign the entire response schema.

## A.9 Strengthen profile tests

Current “non-empty provider list” assertions are insufficient.

For each profile:

```text
coding
security
research
```

assert that after full routability filtering with no credentials:

- at least one provider is routable;
- every selected execution provider either requires no credential or is separately skipped;
- the profile does not fail solely because optional credentials are absent;
- the resolved provider IDs correspond to the documented keyless path.

Specific expectations:

### Coding

At least one keyless web provider, and local workspace only when explicitly configured.

### Security

At least one applicable keyless advisory or web provider. Verify OSV/NVD/CISA KEV/RustSec behavior according to actual provider registration rather than hard-coding all four if feature/config rules differ.

### Research

At least one keyless web or scholarly provider. OpenAlex/Crossref may be checked when enabled by the actual profile.

## A.10 Prove server health vs adapter availability

Add a test that computes both:

```text
core_healthy = at least one required keyless workflow path is routable
optional_adapter_unavailable = one configured credentialed provider lacks its key
```

Assert both can be true simultaneously.

If `provider_status` does not expose a global health field, assert the underlying data needed by codegg:

- keyless provider routable;
- credentialed provider non-routable;
- scoped skip code;
- stable server/tool capabilities remain true.

Do not add a global public health field unless codegg cannot derive the state from existing fields.

## A.11 Gate A acceptance criteria

- [ ] Missing config is tested through the production loader.
- [ ] Process-level or equivalent production startup is tested with credentials scrubbed.
- [ ] `web_fetch` asserts successful deterministic content retrieval.
- [ ] No test accepts both `Ok` and `Err` as success.
- [ ] Configured enabled providers with missing keys are tested.
- [ ] Mixed-provider request includes both provider types.
- [ ] Explicit unavailable-only selection does not silently fall back.
- [ ] Profile tests assert routable keyless providers, not merely non-empty IDs.
- [ ] Server health remains independent of optional adapter availability.
- [ ] Credential values never appear in output.

Do not continue until all Gate A tests fail against the defective behavior and pass after the minimal correction.

---

# Gate B — Correct Retrieval Role and Dimension Semantics

## B.1 Required outcome

Authoritative `RetrievalDimensionState::NotApplicable` means:

```text
role not attempted
role not complete
not an indeterminate failure
not_applicable_count incremented
```

It must not enter `roles_attempted` or `roles_complete`.

## B.2 Use state before inserting role membership

Current logic inserts every dimension role into `roles_seen` before interpreting state.

Refactor `summarize_retrieval()` so role sets are updated inside the effective-state match.

Required state mapping:

| Effective state | Role attempted | Role complete | Role indeterminate | Completed dimension | Failed dimension | Not-applicable dimension |
|---|---:|---:|---:|---:|---:|---:|
| `Satisfied` | yes | yes | no | yes | no | no |
| `CompletedNoMatch` | yes | yes | no | yes | no | no |
| `Partial` | yes | no | yes | no | no | no |
| `Failed` | yes | no | yes | no | yes | no |
| `Interrupted` | yes | no | yes | no | yes | no |
| `SkippedByPolicy` | yes | no | yes | no | no | no |
| `CapabilityUnavailable` | yes | no | yes | no | no | no |
| `NotApplicable` | no | no | no | no | no | yes |
| legacy success-compatible state | preserve documented compatibility | preserve documented compatibility | no unless legacy failure | according to legacy mapping | according to legacy mapping | no unless `EvidenceRoleNotRequested` |

This table is the authoritative contract for this pass.

## B.3 Resolve `attempted_dimension_count`

Use the semantically correct definition:

```text
number of applicable role-expanded dimensions for which an operation was attempted or terminally skipped
```

Therefore authoritative `NotApplicable` dimensions do **not** count.

Implementation pattern:

```rust
let mut attempted_dimension_count = 0;

match effective_state {
    NotApplicable => { ... }
    _ => attempted_dimension_count += 1,
}
```

Do not set it to `dimensions.len()`.

The total number of serialized dimensions remains available as:

```rust
summary.dimensions.len()
```

Do not add `total_dimension_count` unless an existing external consumer demonstrably requires an explicit count.

## B.4 Resolve `completed_dimension_count`

Define completion as a role dimension whose retrieval operation conclusively completed with either evidence or a confirmed zero-result outcome:

```text
Satisfied
CompletedNoMatch
```

Do not count:

```text
Partial
NotApplicable
Failed
Interrupted
SkippedByPolicy
CapabilityUnavailable
```

This aligns the implementation with the codegg contract phrase “dimensions with evidence or no-match.”

Attempt-level `NotApplicable` may remain counted in `completed_job_count` if required for the attempt partition invariant. The attempt and dimension levels intentionally differ.

Document that distinction explicitly.

## B.5 Resolve role aggregation with multiple providers

Use deterministic role-level aggregation:

- `roles_attempted`: distinct role has at least one non-`NotApplicable` dimension;
- `roles_complete`: distinct role has at least one `Satisfied` or `CompletedNoMatch` dimension;
- `roles_indeterminate`: distinct role has at least one `Partial`, `Failed`, `Interrupted`, `SkippedByPolicy`, or `CapabilityUnavailable` dimension.

A role may be both complete and indeterminate when one provider succeeds and another fails. Preserve both signals rather than forcing exclusivity.

This matches provider-scoped evidence: evidence exists, but retrieval was not uniformly healthy.

## B.6 Correct helper semantics

Verify:

### `is_absence_only`

True only when every applicable dimension is `CompletedNoMatch`, with optional `NotApplicable` dimensions ignored.

Required examples:

```text
[CompletedNoMatch]                         -> true
[CompletedNoMatch, NotApplicable]          -> true
[NotApplicable]                            -> false, because no applicable retrieval occurred
[Satisfied, NotApplicable]                 -> false
[Failed, CompletedNoMatch]                 -> false
```

### `is_failure_only`

True only when at least one applicable dimension exists and every applicable dimension is `Failed` or `Interrupted`.

Capability and policy skips are indeterminate but not provider failures.

### `has_indeterminate`

True for any:

```text
Partial
Failed
Interrupted
SkippedByPolicy
CapabilityUnavailable
```

### `absent_roles`

Includes roles with `CompletedNoMatch`; excludes `NotApplicable`.

### `failed_providers`

Includes only `Failed` and `Interrupted` provider IDs.

## B.7 Required tests

Add or correct tests:

1. `not_applicable_role_is_not_attempted`;
2. `not_applicable_role_is_not_complete`;
3. `not_applicable_dimension_does_not_increment_attempted_dimension_count`;
4. `not_applicable_dimension_does_not_increment_completed_dimension_count`;
5. `partial_dimension_is_attempted_indeterminate_not_complete`;
6. `policy_skip_is_attempted_indeterminate_not_failed`;
7. `capability_skip_is_attempted_indeterminate_not_failed`;
8. `success_and_failure_same_role_is_complete_and_indeterminate`;
9. `zero_result_and_not_applicable_is_absence_only`;
10. `not_applicable_only_is_not_absence_only`;
11. `dimension_count_equations_hold_for_all_states`;
12. `attempt_level_not_applicable_partition_remains_valid`;
13. `state_overrides_contradictory_legacy_absence_kind`;
14. `legacy_state_none_behavior_is_preserved`.

Expected count fixture:

```text
states:
  Satisfied
  CompletedNoMatch
  Partial
  Failed
  Interrupted
  SkippedByPolicy
  CapabilityUnavailable
  NotApplicable

attempted_dimension_count = 7
completed_dimension_count = 2
failed_dimension_count = 2
not_applicable_count = 1
```

## B.8 Update codegg contract

Update the dimension count table to state:

```text
attempted_dimension_count: applicable dimensions with attempted or terminally skipped retrieval
completed_dimension_count: satisfied or completed-no-match dimensions
failed_dimension_count: failed or interrupted dimensions
not_applicable_count: dimensions for which the role did not apply
```

Add explicit note:

```text
NotApplicable may count as a completed attempt-level job for terminal accounting,
but it is not an attempted or completed evidence dimension.
```

### Gate B acceptance criteria

- [ ] `NotApplicable` does not enter attempted roles.
- [ ] `NotApplicable` does not enter complete roles.
- [ ] `NotApplicable` does not increment attempted dimensions.
- [ ] `NotApplicable` does not increment completed dimensions.
- [ ] Partial is attempted and indeterminate, not complete.
- [ ] Capability/policy skips are indeterminate, not failed.
- [ ] Mixed success/failure preserves both complete and indeterminate role signals.
- [ ] Code and codegg contract use identical count definitions.

---

# Gate C — Fix Selected-Adapter Workflow Summary Evaluation

## C.1 Required outcome

A manual native-forge workflow run with one or more selected adapters must:

- run only selected adapter jobs;
- require exact success and `pass` output for every selected adapter;
- ignore unselected jobs as `not_requested`;
- fail if any selected adapter is skipped, failed, cancelled, or missing output;
- build a manifest containing evidence only from selected adapters;
- never use invalid Bash variable indirection to access GitHub `needs` context.

## C.2 Remove dynamic `needs.*` Bash dereferencing

Do not use patterns such as:

```bash
result_var="needs.${adapter}.result"
result="${!result_var}"
```

GitHub expression context is resolved before the shell runs. Bash cannot dynamically dereference dotted `needs` paths.

## C.3 Pass explicit job results into the summary environment

Recommended implementation:

```yaml
- name: Require exact pass for selected adapters
  env:
    SELECTED_ADAPTERS: ${{ inputs.adapters }}
    GITHUB_JOB_RESULT: ${{ needs.github.result }}
    GITHUB_EVIDENCE_RESULT: ${{ needs.github.outputs.result }}
    GITLAB_JOB_RESULT: ${{ needs.gitlab.result }}
    GITLAB_EVIDENCE_RESULT: ${{ needs.gitlab.outputs.result }}
    CODEBERG_JOB_RESULT: ${{ needs.codeberg.result }}
    CODEBERG_EVIDENCE_RESULT: ${{ needs.codeberg.outputs.result }}
    GITEA_JOB_RESULT: ${{ needs.gitea.result }}
    GITEA_EVIDENCE_RESULT: ${{ needs.gitea.outputs.result }}
  run: |
    set -euo pipefail
    adapters="$SELECTED_ADAPTERS"
    if [[ -z "$adapters" || "$adapters" == "all" ]]; then
      adapters="github,gitlab,codeberg,gitea"
    fi

    IFS=',' read -ra selected <<< "$adapters"
    for raw in "${selected[@]}"; do
      adapter="$(printf '%s' "$raw" | xargs)"
      case "$adapter" in
        github)
          job="$GITHUB_JOB_RESULT"
          evidence="$GITHUB_EVIDENCE_RESULT"
          ;;
        gitlab)
          job="$GITLAB_JOB_RESULT"
          evidence="$GITLAB_EVIDENCE_RESULT"
          ;;
        codeberg)
          job="$CODEBERG_JOB_RESULT"
          evidence="$CODEBERG_EVIDENCE_RESULT"
          ;;
        gitea)
          job="$GITEA_JOB_RESULT"
          evidence="$GITEA_EVIDENCE_RESULT"
          ;;
        *)
          echo "unsupported adapter: $adapter" >&2
          exit 1
          ;;
      esac

      [[ "$job" == "success" ]]
      [[ "$evidence" == "pass" ]]
    done
```

Equivalent implementations are acceptable if they use valid GitHub expression interpolation and remain fail-closed.

## C.4 Validate adapter input exactly

The current `contains(inputs.adapters, 'github')` form may accept malformed substring values such as `notgithub`.

Prefer a validated input model.

Safe options:

### Option A — boolean inputs

```yaml
verify_github: true|false
verify_gitlab: true|false
verify_codeberg: true|false
verify_gitea: true|false
```

This is the most robust option but modifies the manual UI.

### Option B — normalized comma list

Keep `adapters`, but add an early validation job/step that:

- splits on commas;
- trims whitespace;
- rejects empty entries except whole empty input meaning all;
- accepts only exact tokens;
- rejects duplicates or normalizes them deterministically;
- emits normalized adapter selection.

Job `if:` expressions cannot consume a shell-produced output from a step in the same job. If normalization must control jobs, use a small `prepare` job with outputs, then depend on it.

### Option C — explicit exact-match expression

Use delimiters and exact JSON/contains logic only if maintainable and covered by static tests.

For a small-model corrective pass, Option A is preferred if existing contract tests can be updated safely. Option B is preferred if retaining the current CLI-like input is important.

## C.5 Manifest must match selected adapters

The combined manifest step must assert:

- at least one adapter selected;
- exactly one or more valid evidence files per selected adapter according to the provider's expected fixture count;
- no evidence from an unselected adapter is required;
- every evidence file has exact `R`;
- every evidence file has `mode = native` and `result = pass`;
- hashes are computed after validation;
- manifest records selected adapters explicitly.

Recommended manifest fields:

```json
{
  "schema_version": 1,
  "release_subject": "<R>",
  "selected_adapters": ["github", "gitlab"],
  "evidence": [
    {
      "provider": "github",
      "file": "...",
      "sha256": "..."
    }
  ]
}
```

Do not require unselected adapter artifacts.

## C.6 Workflow contract tests

Extend `tests/native_forge_workflow_contract.rs` or equivalent static workflow tests to prove:

1. no `${!result_var}` or dotted Bash `needs.*` indirection exists;
2. selected adapter results are passed through explicit GitHub expressions;
3. unknown adapter input is rejected;
4. one selected adapter can pass while unselected jobs are skipped;
5. a selected skipped job fails summary;
6. a selected job with missing output fails summary;
7. `all` selects all adapters;
8. manifest records exact release subject;
9. manifest contains only selected adapter evidence;
10. scheduled runs remain diagnostic and do not promote release.

If practical, factor the shell validation into a checked-in script and unit-test it with environment fixtures. This is preferable to testing complex shell only through substring guards.

### Gate C acceptance criteria

- [ ] Invalid Bash indirection is removed.
- [ ] Selected adapter results are evaluated through valid explicit values.
- [ ] Unknown/malformed selections fail before evidence promotion.
- [ ] Unselected adapters do not block selected-adapter verification.
- [ ] Selected skipped/failed/missing-output adapters fail closed.
- [ ] Combined manifest is exact-`R` and selection-aware.
- [ ] Workflow contract tests cover one, many, all, and invalid selections.

---

# Gate D — Strengthen the Core Keyless CI Gate

## D.1 Required outcome

The keyless CI job must run the behavioral tests from Gate A in an environment where credentials are genuinely unavailable to the test process.

## D.2 Do not rely on a prior `unset` step

Environment changes made in one GitHub Actions `run` step do not persist into a later step unless written to the workflow environment files.

The current job has both:

- a credential-scrubbing step;
- empty credential values on the test step.

The explicit empty environment on the test step is the effective mechanism. Simplify and make this intentional.

Recommended test step:

```yaml
- name: Run keyless-core tests with credentials absent
  env:
    GITHUB_TOKEN: ""
    GH_TOKEN: ""
    GITLAB_TOKEN: ""
    GITEA_TOKEN: ""
    FORGEJO_TOKEN: ""
    CODEBERG_TOKEN: ""
    SOURCEGRAPH_API_KEY: ""
    BRAVE_API_KEY: ""
    SEMANTIC_SCHOLAR_API_KEY: ""
    XDG_CONFIG_HOME: ${{ runner.temp }}/eggsearch-empty-config
  run: |
    set -euo pipefail
    rm -rf "$XDG_CONFIG_HOME"
    mkdir -p "$XDG_CONFIG_HOME"
    cargo test --locked --all-features --test keyless_core
```

If tests require variables to be truly absent rather than empty, invoke them through a checked-in wrapper using `env -u NAME` for every credential.

Preferred:

```bash
env \
  -u GITHUB_TOKEN \
  -u GH_TOKEN \
  -u GITLAB_TOKEN \
  ... \
  cargo test --locked --all-features --test keyless_core
```

GitHub may automatically expose `GITHUB_TOKEN`; explicitly remove it for the child process.

## D.3 Run keyless gate on Linux and macOS

Because keyless startup/config behavior and local paths are user-facing and platform-sensitive, use a two-OS matrix:

```yaml
strategy:
  matrix:
    os: [ubuntu-latest, macos-latest]
```

At minimum, the production config-loader/startup and provider-status tests must run on both.

Network-listener fixture tests must use portable Rust helpers and ephemeral ports.

## D.4 Add a release-keyless smoke command

Create one documented command that runs the complete deterministic keyless gate locally.

Preferred Make target:

```make
check-keyless:
	./scripts/check-keyless.sh
```

The script should:

- create an empty temporary config root;
- remove recognized credential variables from the child environment;
- run keyless integration tests;
- run process-level startup/doctor smoke;
- clean temporary files;
- return nonzero on any failure.

Do not embed real credentials or inspect user secret values.

## D.5 Gate D acceptance criteria

- [ ] Keyless tests run with credentials truly absent from child processes.
- [ ] Empty config root is used.
- [ ] Linux and macOS keyless jobs pass.
- [ ] A local `check-keyless` command reproduces the CI behavior.
- [ ] CI does not depend on optional provider secrets.
- [ ] No live third-party network dependency makes the keyless gate flaky.

---

# Gate E — Add Exact-Subject Runtime Benchmark Evidence

## E.1 Required outcome

The final release evidence must include at least one runtime benchmark artifact tied to exact `R`.

`cargo bench --no-run` proves compilation only and is not runtime evidence.

## E.2 Keep normal CI compile-only if needed

The existing normal CI benchmark job may remain compile-only to control runtime and cost.

Add a manual exact-subject release benchmark workflow or extend an existing release-evidence workflow.

Suggested workflow:

```text
.github/workflows/core-release-evidence.yml
```

Inputs:

```text
release_subject: required full 40-character SHA
```

Workflow requirements:

1. validate SHA format;
2. checkout exact SHA;
3. assert `git rev-parse HEAD` equals input;
4. use locked dependencies;
5. scrub optional provider credentials;
6. run keyless core gate;
7. run runtime benchmark command;
8. capture stdout/stderr to bounded files;
9. record environment metadata;
10. calculate SHA-256 hashes;
11. upload artifacts;
12. fail if expected artifact files are absent.

## E.3 Choose bounded benchmark subsets

Do not run an unbounded or excessively long suite on shared CI.

Run the benchmark groups affected by this line of work:

```text
retrieval summary construction
attempt ledger validation
identifier planning near cap
provider-operation budget fanout
provider-status generation
mixed routing diagnostic construction
```

If the Criterion bench supports filters, invoke explicit filters. Otherwise add a dedicated bounded release benchmark binary or bench target using the existing benchmark framework.

Do not change benchmark methodology after selecting `R`.

## E.4 Artifact contents

Required artifact directory:

```text
core-release-evidence/
  subject.txt
  environment.json
  keyless-check.log
  benchmark.log
  benchmark-summary.json
  publish-check.log
  sha256-manifest.json
```

Minimum `benchmark-summary.json` fields:

```json
{
  "schema_version": 1,
  "release_subject": "<R>",
  "rustc": "...",
  "target": "...",
  "runner_os": "...",
  "benchmarks_executed": ["..."],
  "result": "pass"
}
```

The artifact need not impose hard performance thresholds unless stable historical baselines exist. It must prove execution and provide measurements.

## E.5 Hash manifest

`sha256-manifest.json` must include every uploaded evidence file except itself or must define a deterministic self-exclusion rule.

Example:

```json
{
  "schema_version": 1,
  "release_subject": "<R>",
  "files": [
    {"path": "benchmark.log", "sha256": "..."}
  ]
}
```

Validate the manifest before upload.

## E.6 Gate E acceptance criteria

- [ ] Runtime benchmarks execute, not merely compile.
- [ ] Workflow checks out exact `R`.
- [ ] Benchmark definitions are final before `R`.
- [ ] Artifact includes subject and environment metadata.
- [ ] Artifact includes SHA-256 manifest.
- [ ] Optional credentials are absent.
- [ ] Missing benchmark output fails the workflow.

---

# Gate F — Final Release Subject and Evidence Sequence

## F.1 Pre-`R` closure

Before selecting `R`, complete all of the following:

- Gate A behavioral tests;
- Gate B retrieval semantics;
- Gate C workflow correction;
- Gate D keyless CI strengthening;
- Gate E benchmark workflow and definitions;
- codegg contract updates;
- release documentation protocol updates;
- formatting and lint;
- feature matrices;
- schema and documentation contracts;
- clean package verification.

The release verification document must still say:

```text
R not selected
E not created
```

throughout implementation.

## F.2 Full local gate before `R`

Run from a clean checkout:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo test --locked --no-default-features
cargo test --locked --features mock
cargo test --locked --features pdf
cargo doc --all-features --no-deps
cargo build --release
cargo publish --dry-run --locked
```

Also run repository-specific:

```text
schema corpus
documentation contracts
hardening tests
static guards
native workflow contracts
check-keyless
```

Do not use `--allow-dirty`.

## F.3 Select `R`

After all implementation and verification-definition changes are committed, select the final code-bearing commit as `R`.

`R` may contain:

- production code;
- tests;
- workflow definitions;
- benchmark definitions;
- schemas;
- contracts;
- non-evidence documentation.

After selection, do not commit changes to any of those categories.

Record `R` only after its SHA exists. A documentation change that merely writes the SHA after selection should preferably be deferred to `E` so that `R` remains the last code-bearing commit.

## F.4 Exact-`R` remote evidence

For exact `R`, obtain:

1. normal Linux CI run ID;
2. normal macOS CI run ID or matrix job evidence;
3. Linux keyless-core job success;
4. macOS keyless-core job success;
5. format success;
6. clippy success;
7. all feature test combinations;
8. schema/document contract success;
9. release build success;
10. clean publish dry-run success;
11. runtime benchmark workflow run ID;
12. runtime benchmark artifact ID;
13. runtime benchmark artifact SHA-256 manifest.

Optional adapter evidence may be collected for any selected adapters but is not required for core release promotion.

## F.5 Exact-`R` local evidence

Run `check-keyless` and the full local gate from a clean exact-`R` checkout.

Record:

- operating system;
- architecture;
- kernel version;
- Rust version;
- exact command;
- exit status;
- date;
- checkout cleanliness.

Do not claim the local package check passed cleanly if `--allow-dirty` was used.

## F.6 Create terminal evidence-only `E`

After all mandatory evidence exists, create one commit `E`.

Allowed paths:

```text
docs/release-verification.md
docs/release-evidence/**
release-evidence/**
```

`E` must record:

- full 40-character `R`;
- full 40-character `E` where practical, or a documented self-reference convention;
- final classification;
- Linux/macOS run IDs;
- keyless job IDs;
- runtime benchmark workflow/artifact IDs;
- artifact hashes;
- clean package evidence;
- local environment evidence;
- optional adapter table with verified/unverified states;
- invalidated historical `R`/`E` clearly separated.

`E` must not modify:

```text
src/**
tests/**
.github/workflows/**
Cargo.toml
Cargo.lock
Makefile
scripts used by gates
schemas
benchmark definitions
architecture contracts
```

## F.7 Self-reference handling for `E`

A commit cannot contain its own final SHA without a second commit. Use one of these explicit conventions:

### Preferred

The evidence document records:

```text
Evidence commit E: this commit
```

The release tag/notes or external verification record supplies the resolved full SHA.

### Alternative

Create `E1` with evidence, then `E2` changes only the `E` SHA field. In this case the final evidence commit is `E2`, and both commits must be evidence-only. Document the two-step convention in advance.

Do not create a production/test commit merely to fix the evidence SHA field.

## F.8 Post-`E` stop rule

After `E`:

- no further commits for this line of work;
- if only a typo in evidence documentation is found, use an evidence-only correction and update the final evidence commit designation;
- if any code, test, workflow, schema, contract, or benchmark-definition defect is found, invalidate `R` and restart Gate F.

### Gate F acceptance criteria

- [ ] `R` is selected after all implementation changes.
- [ ] All CI and benchmark evidence targets exact `R`.
- [ ] Clean publish dry-run passes without `--allow-dirty`.
- [ ] Runtime benchmark artifact and hashes exist.
- [ ] `E` is evidence-only.
- [ ] No code/test/workflow change follows `E`.
- [ ] Release record does not mix historical superseded evidence with current proof.

---

## 5. Required Focused Tests

The implementing model must add or correct tests matching these behaviors. Names may vary, behavior may not.

### Configuration and startup

```text
keyless_missing_config_file_loads_healthy_defaults
keyless_process_startup_succeeds_without_credentials
configured_github_missing_key_does_not_fail_core
configured_gitlab_missing_key_does_not_fail_core
configured_gitea_missing_key_does_not_fail_core
configured_sourcegraph_missing_key_does_not_fail_core
configured_brave_api_missing_key_does_not_fail_core
configured_semantic_scholar_missing_key_does_not_fail_core
```

### Search and fetch

```text
keyless_web_search_returns_mock_result
keyless_web_fetch_returns_fixture_content
keyless_batch_fetch_returns_fixture_content
mixed_keyless_and_missing_credential_provider_preserves_keyless_results
explicit_missing_credential_provider_is_truthful_and_does_not_fallback
```

### Profiles and diagnostics

```text
coding_profile_resolves_routable_keyless_provider
security_profile_resolves_routable_keyless_provider
research_profile_resolves_routable_keyless_provider
server_core_health_is_independent_of_optional_adapter_availability
provider_status_reports_canonical_missing_credential_code
provider_status_does_not_expose_credential_values
```

### Retrieval semantics

```text
not_applicable_role_is_not_attempted
not_applicable_role_is_not_complete
not_applicable_dimension_is_not_attempted_or_completed
partial_dimension_is_indeterminate_not_complete
success_and_failure_same_role_preserves_both_signals
state_first_dimension_counts_match_contract
```

### Workflow contract

```text
native_summary_uses_explicit_needs_values
native_summary_rejects_unknown_adapter
native_summary_one_selected_adapter_ignores_unselected_jobs
native_summary_selected_skipped_job_fails
native_manifest_records_selected_adapters
native_manifest_requires_exact_release_subject
```

---

## 6. Concrete Failure Fixtures

### Fixture 1 — configured missing GitHub credential

Configuration:

```toml
[search]
default_providers = ["duckduckgo"]

[search.providers]
duckduckgo = true

[search.api.github_code]
enabled = true
api_key_env = "EGGSEARCH_TEST_MISSING_GITHUB_TOKEN"
```

Environment:

```text
EGGSEARCH_TEST_MISSING_GITHUB_TOKEN absent
```

Expected:

```text
state build succeeds
duckduckgo routable
github_code non-routable
github_code skip_code canonical missing-credential value
server core remains usable
```

### Fixture 2 — actual mixed provider request

Request:

```json
{
  "query": "tokio timeout example",
  "providers": ["duckduckgo", "github_code"]
}
```

Fixture engines/status:

```text
duckduckgo -> one deterministic result
github_code -> configured, key absent, scoped skip
```

Expected:

```text
response success
keyless result present
github skip present
github not listed as network/provider execution failure
```

### Fixture 3 — explicit unavailable only

Request:

```json
{
  "query": "tokio timeout example",
  "providers": ["github_code"]
}
```

Expected:

```text
no keyless fallback silently added
no generic result labeled native
structured unavailability or canonical typed error
server remains healthy afterward
```

### Fixture 4 — deterministic fetch

Server response:

```http
HTTP/1.1 200 OK
Content-Type: text/html; charset=utf-8
Content-Length: ...

<html><body>eggsearch-keyless-fetch-fixture</body></html>
```

Expected:

```text
run_web_fetch returns Ok
content contains fixture marker
no external network access
```

### Fixture 5 — dimension accounting

Dimensions:

```text
Satisfied
CompletedNoMatch
Partial
Failed
Interrupted
SkippedByPolicy
CapabilityUnavailable
NotApplicable
```

Expected:

```text
serialized dimensions length = 8
attempted_dimension_count = 7
completed_dimension_count = 2
failed_dimension_count = 2
not_applicable_count = 1
```

### Fixture 6 — selected GitHub adapter only

Workflow input:

```text
release_subject = exact 40-char R
adapters = github
```

Job states:

```text
github = success/pass
gitlab = skipped
codeberg = skipped
gitea = skipped
```

Expected:

```text
summary succeeds
manifest contains github evidence only
unselected skipped jobs do not block
```

### Fixture 7 — selected GitHub adapter missing output

Workflow input:

```text
adapters = github
```

Job states:

```text
github job result = success
github output result = missing
```

Expected:

```text
summary fails
no release manifest promoted
```

---

## 7. Likely Files to Inspect

Modify only what is needed.

### Runtime and retrieval

- `src/core/retrieval_status.rs`
- `src/core/evidence_postprocess.rs`
- provider configuration/loading modules
- profile routing modules
- provider diagnostics/status modules
- `src/mcp/tools.rs`

### Tests

- `tests/keyless_core.rs`
- `tests/retrieval_attempt_ledger.rs`
- `tests/property_retrieval.rs`
- `tests/native_forge_workflow_contract.rs`
- existing config-loader and fetch fixture test modules

### Workflows and scripts

- `.github/workflows/ci.yml`
- `.github/workflows/native-forge-smoke.yml`
- optional new `.github/workflows/core-release-evidence.yml`
- `Makefile`
- optional `scripts/check-keyless.sh`

### Documentation

- `docs/architecture/codegg-contract.md`
- `docs/release-verification.md`
- `docs/release.md`
- `README.md` only if wording must be corrected; do not rewrite the already-correct keyless statement

### Benchmarks

- existing `benches/perf.rs` or equivalent
- `Cargo.toml` only if a dedicated bounded bench target is necessary

---

## 8. Recommended Commit Sequence

Use this sequence to keep ownership and rollback clear.

### Commit 1 — invalidate stale evidence record

Scope:

- release document only;
- mark old `R`/`E` superseded;
- no new `R`.

Suggested message:

```text
docs(release): invalidate superseded keyless evidence pair
```

### Commit 2 — correct retrieval dimension semantics

Scope:

- role/dimension counts;
- helper semantics;
- focused tests;
- codegg count contract.

Suggested message:

```text
fix: align retrieval dimension counts with state semantics
```

### Commit 3 — replace weak keyless tests and minimal runtime fixes

Scope:

- production config-loader test;
- deterministic fetch fixture;
- configured missing-key tests;
- actual mixed routing;
- explicit unavailable-only behavior;
- profile routability tests;
- minimal implementation corrections.

Suggested message:

```text
test: prove keyless core behavior under missing credentials
```

If implementation changes are required, use:

```text
fix: preserve keyless service with unavailable optional adapters
```

and keep test-only changes separate if practical.

### Commit 4 — repair native adapter summary workflow

Scope:

- valid selected-adapter evaluation;
- input validation;
- selection-aware manifest;
- workflow contract tests.

Suggested message:

```text
fix(ci): make selected adapter evidence fail closed
```

### Commit 5 — add exact-subject core evidence workflow

Scope:

- runtime benchmark evidence;
- keyless exact-subject smoke;
- artifact manifest;
- workflow contract/static tests.

Suggested message:

```text
ci: add exact-subject keyless release evidence workflow
```

### Commit 6 — final pre-release corrections

Scope:

- only issues found by full matrix;
- no broad cleanup.

Suggested message:

```text
test: close final release proof gaps
```

### Select `R`

The last code/test/workflow/schema/contract/benchmark-definition commit becomes `R` after all local gates pass.

### Create `E`

After exact-`R` CI, keyless, benchmark, package, and evidence checks pass:

```text
docs(release): record final keyless core evidence
```

No code-bearing commit may follow.

---

## 9. Stop Conditions

Stop implementation and do not select `R` if any condition is true:

- a keyless test still accepts both success and failure;
- the no-config test bypasses the production loader;
- configured missing-key tests remove the provider configuration;
- mixed-provider test does not include both provider types;
- explicit unavailable-only selection silently falls back;
- `NotApplicable` enters attempted or complete role counts;
- workflow summary uses dynamic dotted Bash indirection;
- selected adapter output can be missing while summary passes;
- runtime benchmark is compile-only;
- publish evidence requires `--allow-dirty`;
- any mandatory CI job is pending or failed;
- evidence artifact cannot be tied to exact `R`;
- release document names an `E` before evidence exists.

Stop and invalidate `R` after selection if:

- code changes;
- tests change;
- workflows change;
- schemas change;
- architecture contract changes;
- benchmark definitions change;
- package contents change.

---

## 10. Reviewer Checklist

### Keyless proof

- [ ] Missing config uses production loader.
- [ ] Process startup or equivalent production path succeeds without keys.
- [ ] Fetch returns deterministic fixture content.
- [ ] Configured optional provider missing key remains nonfatal.
- [ ] Mixed request preserves keyless result and scoped skip.
- [ ] Explicit unavailable-only request does not silently fall back.
- [ ] Profiles contain routable keyless execution paths.
- [ ] No credentials are exposed.

### Retrieval semantics

- [ ] NotApplicable role is not attempted.
- [ ] NotApplicable role is not complete.
- [ ] NotApplicable dimension is neither attempted nor complete.
- [ ] Partial dimension is indeterminate, not complete.
- [ ] Dimension counts match the contract table.
- [ ] Attempt-level terminal partition remains valid.

### Adapter workflow

- [ ] No invalid Bash `needs` dereferencing.
- [ ] Exact adapter selection is validated.
- [ ] One selected adapter can pass independently.
- [ ] Selected skipped or missing-output adapter fails.
- [ ] Manifest contains only selected evidence.
- [ ] Exact `R` is validated.

### Release proof

- [ ] Old `R`/`E` marked superseded.
- [ ] New `R` selected only after all changes.
- [ ] Linux and macOS exact-`R` CI pass.
- [ ] Keyless jobs pass on both operating systems.
- [ ] Runtime benchmark artifact exists.
- [ ] Artifact hashes exist.
- [ ] Clean publish dry-run passes.
- [ ] `E` is evidence-only and terminal.

---

## 11. Definition of Done

This line of work is complete only when all statements are true:

1. The stale `226897...` / `6fea3c2...` evidence pair is explicitly invalidated.
2. Missing configuration is tested through the production loader.
3. Keyless process startup is proven without credential variables.
4. Keyless fetch returns deterministic fixture content.
5. Enabled credentialed providers with absent keys degrade provider-locally.
6. A real mixed-provider request preserves keyless results.
7. Explicit unavailable-only provider selection is truthful and does not silently fall back.
8. Coding, security, and research profiles each have a proven routable keyless path.
9. `NotApplicable` roles and dimensions are neither attempted nor complete.
10. Dimension counters and codegg documentation use identical semantics.
11. The native adapter summary workflow correctly evaluates selected jobs through valid GitHub expression values.
12. Selected adapter evidence is fail-closed and selection-aware.
13. Keyless CI runs on Linux and macOS with credentials absent.
14. A runtime benchmark artifact is generated for exact final `R`.
15. A clean package/publish dry-run passes for exact `R` without `--allow-dirty`.
16. Final normal CI, keyless CI, benchmark evidence, and hashes all reference exact `R`.
17. Final `E` modifies only evidence/documentation paths.
18. No code, test, workflow, schema, contract, or benchmark-definition commit follows `E`.
19. Optional adapters remain optional for users and claim-scoped for maintainers.
20. The final release record can truthfully classify the core as **release-verified in keyless mode**.

Until every item is satisfied, retain **provisional release candidate** status.
