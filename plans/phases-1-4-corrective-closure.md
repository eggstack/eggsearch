# Phases 1-4 Corrective Closure Plan

## Objective

Close the remaining correctness, safety, and interface gaps from the first implementation pass of the codegg-oriented eggsearch roadmap. The repo now has the intended major surfaces: `repo_search`, `security_search`, `research_search`, OSV/KEV-related support, grouped result models, planners, suggested fetches, and MCP wiring. This corrective pass should not add another broad feature layer. It should make the implemented surfaces precise, bounded, and reliable enough for codegg to consume.

The highest priorities are:

1. Enforce a single request-level deadline for multi-subquery tools.
2. Correct security capability semantics and warnings.
3. Make OSV advisory lookup/query behavior explicit and structurally correct.
4. Resolve KEV warning/state inconsistency.
5. Clean up request semantics, especially repo explicit fields vs query hints.
6. Ensure `max_groups` and response limits are actually enforced.
7. Move security grouping/suggested-fetch logic out of MCP tool glue.
8. Update stale docs/comments and add regression tests.

## Non-goals

Do not add new major tools.

Do not add broad NVD/GHSA/RustSec native support in this pass unless it falls out naturally from cleanup. OSV + KEV correctness is enough for closure.

Do not turn `research_search` into a synthesizer.

Do not make `repo_search`, `security_search`, or `research_search` fetch pages automatically.

Do not relax `web_fetch` SSRF/private-network/explicit-URL boundaries.

Do not replace the existing generic `web_search` path.

## Workstream 1: Single request-level timeout for multi-subquery tools

### Problem

`repo_search` and `research_search` currently allocate a fresh `effective_timeout` deadline inside each subquery loop. This means total runtime can become roughly `subquery_count * timeout`, which violates the intended bounded-agent behavior.

### Desired behavior

A single timeout should bound the whole tool call. Each subquery should consume from a shared remaining budget. If the budget is exhausted, stop launching additional subqueries and return partial results with warnings.

### Implementation guidance

Refactor `MetadataSearchAdapter::repo_search` and `MetadataSearchAdapter::research_search`.

Current anti-pattern:

```rust
for subquery in &plan.subqueries {
    let deadline = tokio::time::Instant::now() + effective_timeout;
    ...
}
```

Target pattern:

```rust
let overall_deadline = tokio::time::Instant::now() + effective_timeout;
for subquery in &plan.subqueries {
    let remaining = overall_deadline.saturating_duration_since(tokio::time::Instant::now());
    if remaining.is_zero() {
        warnings.push(...);
        break;
    }
    run_subquery_with_timeout(remaining).await;
}
```

Consider extracting a helper for multi-subquery execution:

```rust
async fn run_bounded_subqueries(
    engines: &[Arc<dyn SearchEngine>],
    subqueries: &[PlannedSubquery],
    candidate_limit: usize,
    deadline: Instant,
) -> MultiSearchOutcome
```

This helper can be introduced only if it reduces duplication without destabilizing the code. A smaller direct fix in both methods is acceptable.

### Warning behavior

Add stable warnings:

- `request_deadline_exceeded: repo_search stopped before all subqueries completed`
- `request_deadline_exceeded: research_search stopped before all subqueries completed`
- Optional: include `completed_subqueries` and `total_subqueries` if warnings remain string-only.

Do not emit provider timeout failures for providers that were never launched because the request-level deadline was already exhausted. Provider failures should represent attempted provider calls.

### Tests

Add deterministic mocked tests where:

- A multi-subquery request with a short timeout returns partial results instead of multiplying timeout per subquery.
- The warning indicates request-level deadline exhaustion.
- Providers not launched due to exhausted request budget are not reported as provider failures.
- Existing per-provider timeout behavior still works for a launched subquery.

## Workstream 2: Correct security capability semantics

### Problem

Generic `web_search intent=security` currently suppresses the “no native security advisory search” warning based on code/issue/release/timestamp capabilities. Those are not advisory-native capabilities. The new `ProviderCapabilities::supports_security_search` flag should be authoritative.

### Desired behavior

For `SearchIntent::Security`, warning logic should look at `supports_security_search`. Providers like GitHub issues/releases may provide useful context, but they are not advisory databases unless explicitly modeled that way.

### Implementation guidance

Update the security warning block in `MetadataSearchAdapter::web_search`.

Current logic should be replaced with something equivalent to:

```rust
if req.intent == SearchIntent::Security
    && !any_engine_supports(&engines, |c| c.supports_security_search)
{
    capability_warnings.push(SearchWarning::new(
        "_system",
        "intent=security requested but no provider has native security advisory search; results are from generic/contextual search",
    ));
}
```

If retaining contextual provider recognition, use a second, lower-severity warning or rank reason. Do not let contextual issue/release support imply advisory-native support.

### Tests

Add tests with mock provider configurations or descriptors showing:

- `intent=security` + only generic providers emits warning.
- `intent=security` + GitHub issue/release providers but no OSV still emits warning.
- `intent=security` + OSV does not emit `no native advisory` warning.
- Provider capability summary includes `security_search` only for OSV or future true advisory providers.

## Workstream 3: OSV query and lookup correctness

### Problem

The OSV engine currently sends `{"package": {"name": query}}` to `/query`, using the entire query string as a package name and omitting ecosystem/version. That is not semantically correct for arbitrary `security_search` queries. ID lookups exist, but package/ecosystem/version queries should be explicit and not routed through generic free text.

### Desired behavior

OSV support should expose clear native operations:

- Lookup by vulnerability ID: CVE, GHSA, OSV ID, RustSec alias when OSV supports it.
- Query by package + ecosystem.
- Query by package + ecosystem + version when a version is supplied.

Generic free-text security search should remain a fallback over web providers. OSV should not receive arbitrary prose as a package name.

### Implementation guidance

Refactor `src/meta/engines/osv.rs` around explicit functions:

```rust
pub async fn lookup_by_id(
    client: &Client,
    vuln_id: &str,
    timeout: Duration,
) -> Result<Option<VulnerabilityMetadata>, EngineError>
```

Already present; harden it.

Add:

```rust
pub async fn query_package(
    client: &Client,
    ecosystem: &str,
    package: &str,
    version: Option<&str>,
    max_results: usize,
    timeout: Duration,
) -> Result<Vec<VulnerabilityMetadata>, EngineError>
```

Request shape should include:

```json
{
  "package": {"ecosystem": "crates.io", "name": "..."},
  "version": "..." // only when present
}
```

Keep OSV's `SearchEngine` implementation only if it has a defensible generic interpretation. Recommended options:

1. Make `OsvEngine::search` recognize structured query tokens (`package:`, `ecosystem:`, `version:`, CVE/GHSA/OSV/RustSec IDs) and return no results with a warning/error for unstructured prose.
2. Remove OSV from normal generic provider fan-out and use it only inside `security_search` native paths.

Option 2 is cleaner semantically, but may require capability/provider-status decisions. If OSV remains a provider, ensure it does not silently treat prose as package names.

### Integrate with `security_search`

Update `run_security_search` or, preferably, a new security orchestration module so that:

- Resolved IDs call `lookup_by_id`.
- Resolved `package + ecosystem` calls `query_package`.
- `version` is included when present.
- Native vulnerabilities are deduplicated/merged by ID aliases.
- Native provider errors are surfaced as provider failures or structured warnings.

Do not depend on generic `web_search` to obtain vulnerability metadata.

### Metadata correctness

Improve OSV conversion:

- Preserve CVSS vector string when OSV provides one.
- If practical, parse CVSS numeric score from OSV severity records.
- If not parsing score, leave `cvss_score = None` and add a warning only when the user requested severity-sensitive behavior.
- Do not derive `patched_ranges` as `"<fixed"`; that reads like vulnerable range rather than patched range. Prefer `fixed: <version>` in a string field or a clearer format such as `fixed >= <version>` only if that is semantically correct for the ecosystem/range type.
- Keep provider-native affected ranges as faithful strings if exact conversion is non-trivial.

### Tests

Add mocked HTTP tests for OSV:

- ID lookup hit.
- ID lookup 404 returns `Ok(None)`.
- Package/ecosystem query builds expected request body.
- Package/ecosystem/version query includes version.
- Unstructured prose is not sent as OSV package name.
- Vulnerability metadata includes aliases, package, ecosystem, references, timestamps, affected/fixed information.
- Severity/CVSS behavior is tested and documented.

## Workstream 4: KEV warning and state consistency

### Problem

`security_search` emits `kev_unavailable: CISA KEV catalog is not yet implemented` whenever KEV is requested, then later tries `state.kev_client.lookup`. If lookup succeeds, it removes the warning. This is internally inconsistent and misleading.

### Desired behavior

Warnings should reflect actual KEV client state and lookup outcome:

- If KEV support is configured and lookup succeeds with match: `kev_match`.
- If KEV support is configured and lookup succeeds with no match: `kev_absent_not_proof`.
- If KEV support is unavailable/disabled/client construction failed: `kev_unavailable`.
- If KEV lookup fails due to network/parse/timeout: `kev_lookup_failed`.

Do not emit “not implemented” if there is a KEV client.

### Implementation guidance

Inspect `ServerState` and `kev_client` initialization. Decide whether KEV is always present or optional.

Refactor KEV warning emission so it happens after attempted lookup, not before. Pseudocode:

```rust
if req.include_kev == Some(true) {
    match lookup_kev_for_vulnerabilities(...).await {
        KevOutcome::Matches(ids) => warn kev_match,
        KevOutcome::NoMatches => warn kev_absent_not_proof,
        KevOutcome::Unavailable => warn kev_unavailable,
        KevOutcome::Failed(err) => warn kev_lookup_failed,
    }
}
```

If no CVE IDs are available, emit:

- `kev_lookup_skipped: KEV lookup requires CVE identifiers`

### Tests

Add tests for:

- KEV requested with CVE and mocked hit.
- KEV requested with CVE and mocked miss.
- KEV requested without CVE identifiers.
- KEV lookup failure warning.
- No stale “not implemented” warning when KEV client is available.

## Workstream 5: Repo explicit-field precedence

### Problem

`RepoSearchRequest::resolved_hints` currently prefers query-parsed hints over explicit JSON fields. The handoff plan recommended explicit fields override or supplement query tokens. For MCP/tool calls, explicit fields are usually the more reliable structured user intent.

### Desired behavior

Explicit fields should win over parsed query hints unless there is a strong reason otherwise. Query hints should fill missing fields. The residual query should remain derived from the original query, but explicit fields should be reflected in the resolved hints.

### Implementation guidance

Change `resolved_hints` from:

```rust
if self.owner.is_some() && hints.owner.is_none() { ... }
```

to:

```rust
if self.owner.is_some() { hints.owner = self.owner.clone(); }
```

Apply similarly for host, repo, org, path, file, language, and symbol.

If host is explicitly invalid/unknown at MCP conversion time, decide whether to reject or set `CodeHost::Unknown`. Prefer validation error for unknown host strings rather than silently setting Unknown.

### Tests

Update tests:

- Explicit fields override `repo:owner/repo` in query.
- Query hints fill absent explicit fields.
- Explicit language is normalized/lowercased.
- Unknown host string in MCP args returns validation error if adopting strict behavior.

Update docs to state precedence clearly.

## Workstream 6: Enforce `max_groups` and response bounds

### Problem

`ResearchSearchRequest` exposes and validates `max_groups`, but adapter code visibly applies only `max_per_group`. If grouping modules enforce `max_groups`, verify with tests. If not, implement it.

### Desired behavior

For `research_search`:

- `max_results` bounds total returned cards across groups.
- `max_groups` bounds number of returned groups.
- `max_per_group` bounds cards per group.
- Truncation flags accurately indicate group-level truncation.

For `repo_search` and `security_search`:

- `max_results` and `max_per_group` should be consistently enforced.
- If a tool lacks `max_groups`, that is acceptable, but total result count must still be bounded.

### Implementation guidance

Audit:

- `src/meta/research_grouping.rs`
- `src/meta/repo_grouping.rs`
- security grouping currently in `src/mcp/tools.rs`
- suggested fetch generation modules

Add helper functions if needed:

```rust
fn enforce_group_limits<T>(groups: Vec<Group<T>>, max_groups: usize, max_results: usize) -> Vec<Group<T>>
```

Ensure suggested fetches are generated after final group limiting, or are filtered to URLs present in the final response.

### Tests

Add tests for:

- `research_search` with `max_groups = 1` returns exactly one group.
- `research_search` with small `max_results` does not exceed total card count.
- `max_per_group` truncates group results and marks `truncated = true`.
- Suggested fetches correspond only to visible/final result cards, except deterministic advisory ID URLs if deliberately allowed and documented.

## Workstream 7: Move security grouping and suggested fetches out of MCP glue

### Problem

Security grouping and suggested-fetch logic currently lives inside `src/mcp/tools.rs`. Repo/research logic has proper meta modules. Keeping security classification in MCP glue makes the file large, mixes concerns, and makes unit testing harder.

### Desired behavior

MCP tool functions should validate/convert args, call core/adapter/orchestration logic, serialize response, and map errors. Security classification, grouping, suggested fetches, and native advisory orchestration should live under `src/meta` or a dedicated security module.

### Implementation guidance

Create modules:

- `src/meta/security_grouping.rs`
- `src/meta/security_suggested_fetches.rs`
- optionally `src/meta/security_planner.rs` or `src/meta/security_orchestrator.rs`

Move these functions:

- `classify_security_result`
- `group_security_results`
- `security_group_label`
- `generate_security_suggested_fetches`

Consider moving most of `run_security_search` native lookup orchestration out of MCP tools into `MetadataSearchAdapter::security_search` or a meta-level service. The MCP function should become analogous to `run_repo_search` and `run_research_search`.

### Tests

Add direct unit tests for the new grouping/suggested-fetch modules.

Ensure no behavioral regressions in MCP integration tests.

## Workstream 8: Warning taxonomy and consistency cleanup

### Problem

Warnings currently mix human prose, snake_case prefixes, and duplicated generic untrusted-content warnings across layers. Some warnings are stale or inaccurate.

### Desired behavior

Warnings should be stable enough for codegg to show in the TUI and optionally classify. They can remain `SearchWarning { provider_id, message }`, but message prefixes should be consistent.

### Recommended warning prefixes

Use snake_case prefixes followed by concise detail:

- `safe_search_unenforced`
- `freshness_unenforced`
- `native_code_search_unavailable`
- `native_issue_search_unavailable`
- `native_release_search_unavailable`
- `native_advisory_search_unavailable`
- `generic_context_untrusted`
- `request_deadline_exceeded`
- `subquery_cap_applied`
- `version_match_unavailable`
- `kev_match`
- `kev_absent_not_proof`
- `kev_lookup_failed`
- `kev_lookup_skipped`
- `source_quality_heuristic`

Avoid warnings that say “not implemented” when a partial implementation exists.

Avoid adding “Live web results are untrusted external content” twice. Decide whether that belongs in adapter responses, MCP serialized responses, or both. Prefer one consistent top-level warning per response.

### Tests

Add tests that assert warning prefixes for critical cases rather than long exact prose, unless exact prose is intended stable API.

## Workstream 9: Documentation/comment cleanup

### Known stale docs/comments

- `src/mcp/tools.rs` still says “Three tools are exposed,” but six tools are now exposed.
- Tool descriptions and README should clarify actual OSV/KEV behavior after this corrective pass.
- Security docs should state which facts are native advisory metadata and which are generic context.
- Repo docs should state explicit-field vs query-hint precedence.
- Research docs should state `max_groups`, `max_results`, and deadline semantics.

### Required updates

Update:

- `src/mcp/tools.rs` module docs.
- `README.md` tool sections.
- `CHANGELOG.md` under Unreleased.
- `AGENTS.md` if it documents tool-selection behavior.
- Any provider configuration examples involving `osv` or KEV.

## Workstream 10: Regression and validation suite

### Required commands

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test
```

If `cargo clippy --all-targets --all-features -- -D warnings` is too strict due to existing known warnings, fix the warnings rather than weakening the command unless unavoidable.

### Required test categories

Add or update tests for:

- Request-level timeout budget across repo/research multi-subquery tools.
- Security warning uses `supports_security_search`.
- OSV ID lookup and package/ecosystem/version query body formation.
- KEV hit/miss/failure/no-CVE outcomes.
- Repo explicit field precedence.
- Research `max_groups` enforcement.
- Security grouping module after extraction from MCP glue.
- Warning prefix stability.
- MCP tool list includes all six tools.
- Generic `web_search` remains simple and unaffected.

Live-network tests must remain opt-in. Use mocked engines or mocked HTTP clients where possible.

## Suggested implementation order

1. Fix timeout budgeting in `repo_search` and `research_search`.
2. Correct `intent=security` capability warning to use `supports_security_search`.
3. Refactor OSV into explicit native operations and wire package/ecosystem/version query into `security_search`.
4. Fix KEV warning/outcome logic.
5. Flip repo explicit-field precedence and update tests/docs.
6. Enforce `max_groups` and total result bounds.
7. Extract security grouping/suggested-fetch code out of MCP tools.
8. Normalize warning prefixes and remove duplicate/stale warnings.
9. Update docs/comments/CHANGELOG.
10. Run full validation commands and address failures.

## Acceptance criteria

This corrective pass is complete when:

- `repo_search` and `research_search` are bounded by one total request deadline.
- `web_search intent=security` warning behavior reflects true advisory-native provider support.
- `security_search` uses OSV through explicit ID/package/ecosystem/version operations, not arbitrary prose-as-package queries.
- KEV warnings accurately reflect lookup status.
- Repo explicit fields override query hints, or the opposite behavior is explicitly justified and documented with tests.
- `max_groups`, `max_results`, and `max_per_group` limits are enforced consistently.
- Security grouping and suggested-fetch logic is no longer embedded in MCP tool glue.
- Warning messages are stable and not misleading.
- Stale comments/docs are updated.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`, and `cargo test` pass locally.

## Handoff notes

Prioritize correctness over adding more provider breadth. OSV + KEV done precisely is more valuable than adding NVD/GHSA/RustSec in a shallow way. The codegg-facing value comes from bounded behavior, trustworthy warnings, and clear separation between advisory facts and generic external discussion.
