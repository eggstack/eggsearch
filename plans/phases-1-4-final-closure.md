# Phases 1-4 Final Closure Plan

## Objective

Close the remaining narrow gaps from the codegg-oriented `repo_search`, `security_search`, and `research_search` implementation line. The previous corrective pass fixed the main structural issues: request-level deadlines are mostly in place, security capability warnings use `supports_security_search`, repo explicit fields now override query hints, research `max_groups` is wired, KEV warnings are no longer stale, and security grouping/suggested-fetch logic moved out of MCP glue.

This final closure pass should avoid adding new scope. Its purpose is to make the current surfaces semantically correct, consistently bounded, and ready for codegg integration.

## Remaining gaps to close

1. `security_search` still does not appear to call the new OSV `query_package` helper for package/ecosystem/version requests.
2. The old OSV `SearchEngine::search` path still treats arbitrary free-text query strings as `package.name` without ecosystem.
3. Security orchestration remains too large inside `src/mcp/tools.rs` even after grouping/suggested-fetch extraction.
4. Request-deadline warnings for `repo_search` and `research_search` can underreport interrupted in-flight subqueries.
5. Unknown `repo_search.host` strings are still accepted as `CodeHost::Unknown` instead of rejected.
6. Warning strings are improved but not yet fully normalized or de-duplicated.
7. OSV CVSS/severity metadata remains weak and should be either improved or explicitly documented/tested as partial.
8. Validation evidence from CI/local commands is still unavailable from GitHub status checks.

## Workstream 1: Wire OSV package/ecosystem/version lookup into `security_search`

### Problem

`src/meta/engines/osv.rs` now contains `query_package(client, ecosystem, package, version, max_results, timeout)`, and it builds the correct OSV `/v1/query` body. However, the `security_search` tool path still primarily performs generic `web_search` plus ID-based `lookup_advisory` calls. Package-oriented requests such as `ecosystem=crates.io package=serde version=1.0.0` should use the native OSV package query path.

### Desired behavior

When `security_search` receives a resolved `package + ecosystem`, it should query OSV natively if OSV is enabled/configured. If `version` is present, pass it to OSV. The returned OSV vulnerabilities should populate `SecuritySearchResponse.vulnerabilities`, independent of generic web results.

### Implementation guidance

Add adapter-level helper methods so MCP code does not need to know OSV internals:

```rust
impl MetadataSearchAdapter {
    pub async fn query_advisories_by_package(
        &self,
        ecosystem: &str,
        package: &str,
        version: Option<&str>,
        max_results: usize,
    ) -> Result<Vec<VulnerabilityMetadata>, EngineErrorLike>
}
```

or, if the existing adapter error taxonomy makes that awkward, implement a security orchestration module that can call the OSV helper through a small internal API.

`security_search` should:

1. Resolve identifiers with `SecurityIdentifiers::parse`.
2. If `resolved_ids.package` and `resolved_ids.ecosystem` are both present, call the native OSV package query when OSV is available.
3. Pass `resolved_ids.version.as_deref()`.
4. Merge/deduplicate vulnerabilities from package query and ID lookups.
5. Emit a warning if package/ecosystem was supplied but no native advisory provider is available.
6. Preserve generic fallback results in groups, but do not depend on them for advisory facts.

### Deduplication

Deduplicate vulnerabilities by a canonical key:

1. Prefer OSV ID if present.
2. Else GHSA ID.
3. Else CVE ID.
4. Else package/ecosystem plus source-specific fallback.

If two records share aliases, merge rather than duplicate.

### Tests

Add mocked tests for:

- `security_search` with `package + ecosystem` calls native OSV package query.
- `security_search` with `package + ecosystem + version` includes version in the OSV body.
- Native vulnerabilities appear in `response.vulnerabilities` even when generic web results are empty.
- Package query vulnerabilities and ID lookup vulnerabilities dedupe by alias/ID.
- If OSV is not enabled, response includes `native_advisory_search_unavailable` or equivalent warning.

## Workstream 2: Guard or remove OSV free-text `SearchEngine::search`

### Problem

The OSV `search` function still posts:

```json
{"package": {"name": query}}
```

This means arbitrary prose sent through generic provider fan-out can be interpreted as a package name. That is semantically wrong and will produce misleading misses or noisy requests.

### Desired behavior

OSV should not treat unstructured prose as a package name.

Recommended direction: remove OSV from generic search fan-out and use it only as a native advisory provider inside `security_search`. If that is too invasive, make `OsvEngine::search` parse only structured tokens and return an empty result set for unstructured queries with a warning/failure that does not look like provider outage.

### Option A: Native-only OSV provider

Use OSV only through explicit adapter/security methods:

- `lookup_advisory(id)`
- `query_advisories_by_package(ecosystem, package, version)`

In this model, provider status still reports OSV as available for `security_search`, but generic `web_search` should not send OSV arbitrary query text.

This likely requires separating “engines used by generic metasearch” from “native advisory providers.” If that is too large, defer full separation but implement Option B now.

### Option B: Structured-only SearchEngine fallback

Modify `OsvEngine::search` to parse the incoming query for:

- CVE/GHSA/OSV/RustSec ID
- `package:`/`crate:`/`npm:`/`pypi:` hints
- `ecosystem:` hint
- `version:` hint

Rules:

- If a strong ID exists, call `lookup_by_id` and return one source-card result if found.
- If `package + ecosystem` exists, call `query_package`.
- If package exists through aliases such as `crate:serde`, infer ecosystem where unambiguous.
- If no structured signal exists, return `Ok(Vec::new())` rather than sending prose as `package.name`.

### Tests

Add tests for:

- Unstructured prose does not produce an OSV HTTP request as package name.
- `package:serde ecosystem:crates.io` builds a package query.
- `crate:serde` infers `crates.io` if supported.
- `CVE-...` routes to ID lookup.
- The behavior is documented in provider/tool docs.

## Workstream 3: Move security orchestration out of MCP glue

### Problem

Security grouping and suggested fetches moved to `src/meta`, but `run_security_search` still contains the full orchestration: generic fallback search, native ID lookups, KEV enrichment, warning construction, grouping, and response assembly. MCP tool code should mainly validate/convert args and call the adapter/meta layer.

### Desired behavior

Create a meta-level or adapter-level security search orchestration boundary.

Recommended module:

- `src/meta/security_search.rs`

Recommended public function:

```rust
pub async fn run_security_search_plan(
    adapter: &MetadataSearchAdapter,
    kev_client: &KevClient,
    req: &SecuritySearchRequest,
    effective_max: usize,
    max_results_cap: usize,
) -> SecuritySearchResponse
```

Alternative: implement `MetadataSearchAdapter::security_search(...)` directly, mirroring `repo_search` and `research_search`.

### Required behavior

The new security orchestration boundary should own:

- Identifier parsing output supplied by request or recomputed.
- Native OSV ID lookups.
- Native OSV package/ecosystem/version query.
- Generic `web_search intent=security` fallback.
- KEV enrichment.
- Security grouping and suggested fetches.
- Warning construction.
- Vulnerability deduplication.

`src/mcp/tools.rs::run_security_search` should own only:

- Mode/policy check.
- String enum conversion for args.
- Request validation.
- Provider resolution/unknown-provider rejection.
- Effective max computation.
- Calling the security search adapter/orchestrator.
- JSON serialization.

### Tests

Add direct unit tests for the new security orchestrator with mocked adapter/provider clients where possible. MCP tests should only verify request conversion and response serialization.

## Workstream 4: Fix request-deadline warning accuracy

### Problem

`repo_search` and `research_search` now use an overall deadline, but `subqueries_skipped` only increments when a subquery is skipped before launch. If the deadline expires during an in-flight subquery, the response can be partial without a `request_deadline_exceeded` warning if no later subqueries are skipped.

### Desired behavior

Any request-level deadline exhaustion should produce a warning, whether it occurs before launching a subquery or during one.

### Implementation guidance

Track a boolean:

```rust
let mut request_deadline_exceeded = false;
let mut subqueries_skipped = 0usize;
let mut subqueries_interrupted = 0usize;
```

Set `request_deadline_exceeded = true` when:

- `remaining.is_zero()` before launching a subquery.
- `remaining.is_zero()` while awaiting a subquery join set.
- `tokio::time::timeout(remaining, join_set.join_next()).await` returns timeout.

Increment `subqueries_interrupted` when the current subquery was launched but not all providers completed before the deadline.

Warning example:

```text
request_deadline_exceeded: repo_search returned partial results (1 interrupted, 2 skipped)
```

Provider failures should include attempted providers that timed out during a launched subquery if that is already consistent with current behavior. Providers for skipped subqueries should not be reported as provider failures.

### Tests

Add tests for:

- Deadline expires during first/only subquery and warning is emitted.
- Deadline expires before later subqueries and skipped count is included.
- Provider failures do not include providers for never-launched skipped subqueries.

## Workstream 5: Strict `repo_search.host` validation

### Problem

MCP argument conversion maps unknown host strings to `CodeHost::Unknown`. For an explicit structured field, silent Unknown is less useful than a validation error.

### Desired behavior

Unknown explicit host values should return `ToolError::Validation` with accepted values.

Accepted strings:

- `github`, `gh`
- `gitlab`, `gl`
- `codeberg`, `cb`

Optional: support `gitea`, `forgejo` if `CodeHost` already supports them and URL classification understands them.

### Implementation guidance

Replace:

```rust
_ => CodeHost::Unknown
```

with conversion returning `Result<Option<CodeHost>, ToolError>`.

If host is omitted, keep `None`. If host is provided but unknown, return validation error.

### Tests

Add tests:

- `host=github` accepted.
- `host=gh` accepted.
- `host=unknownhost` rejected.
- Query-parsed host hints still behave according to existing `RepoQueryHints` behavior.

## Workstream 6: Warning taxonomy and duplicate warning cleanup

### Problem

Warnings still mix long prose and snake_case prefixes. Some wrappers add generic untrusted warnings on top of adapter warnings. This is tolerable, but codegg will benefit from stable prefixes and reduced duplication.

### Desired behavior

Adopt stable prefix convention for new/specialized warnings:

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

Do not require a large response schema migration. It is acceptable to keep `SearchWarning { provider_id, message }` and make the message prefix stable.

### Implementation guidance

Normalize the main specialized warnings first:

- Repo native provider warnings.
- Research timeout/freshness/subquery warnings.
- Security native advisory/KEV/version/generic context warnings.
- Generic `web_search intent=security` advisory warning.

Then address duplicate untrusted warnings:

- Decide whether `web_search` adapter or MCP wrapper owns the standard untrusted warning.
- Prefer one standard top-level warning in the serialized response.
- Preserve existing behavior if removing duplication would be breaking, but document the policy.

### Tests

Add tests that assert warning prefixes, not entire long prose, for:

- no native advisory provider
- request deadline exceeded
- version match unavailable
- KEV skipped/miss/match/failure
- security generic context untrusted

## Workstream 7: OSV CVSS/severity precision or explicit limitation

### Problem

OSV metadata conversion still leaves `cvss_score` and `cvss_vector` empty and maps CVSS-vector-looking strings to `SeverityLevel::Unknown`. This is acceptable only if clearly documented as a limitation. Better is to parse at least the vector string and possibly score when present.

### Desired behavior

Minimum acceptable closure:

- Preserve CVSS vector text in `cvss_vector` when OSV exposes a CVSS vector.
- Leave `cvss_score = None` if no numeric score is exposed or computed.
- Severity remains `Unknown` unless a textual severity is actually provided or numeric scoring is implemented.

Better closure:

- Parse numeric CVSS score if OSV provides a score string that contains one.
- Map score to severity using standard bands:
  - 9.0-10.0 Critical
  - 7.0-8.9 High
  - 4.0-6.9 Medium
  - 0.1-3.9 Low
  - 0.0 None/Unknown depending enum support

Do not implement a full CVSS vector calculator unless a small, trusted crate is already acceptable. Avoid adding heavy dependencies for this closure pass.

### Tests

Add tests for:

- CVSS vector is preserved.
- Numeric score string maps to score and severity if supported.
- Unknown/unsupported severity does not fabricate data.

## Workstream 8: Documentation and integration contract cleanup

Update docs to reflect final behavior.

Required docs:

- README `security_search` section: OSV supports ID lookup and package/ecosystem/version lookup when native OSV provider is enabled.
- README provider section: OSV is advisory-native, not generic prose search.
- README `repo_search` section: explicit fields override query hints; unknown explicit host values are rejected.
- README `research_search` section: one total request deadline bounds all subqueries; `max_groups` is enforced.
- AGENTS.md: tool-selection guidance for codegg agents.
- CHANGELOG.md: final closure note.

If there is a generated or hand-written MCP schema/example section, update examples for package-based `security_search`.

## Workstream 9: Validation and CI evidence

### Required commands

Run locally or in CI:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test
```

If no GitHub Actions workflow exists, either add one or document that validation was run locally. Prefer adding a lightweight CI workflow if repo conventions allow it.

### Minimal CI workflow, if absent

Add `.github/workflows/ci.yml` with:

- Rust stable toolchain.
- Cargo cache.
- `cargo fmt --check`.
- `cargo clippy --all-targets --all-features -- -D warnings`.
- `cargo test --all-features`.

Do not include live-network tests in CI unless they are explicitly gated and mocked.

## Suggested implementation order

1. Wire OSV package/ecosystem/version query into `security_search` response vulnerabilities.
2. Guard/remove OSV free-text `SearchEngine::search` behavior.
3. Move security orchestration out of `src/mcp/tools.rs` into adapter/meta module.
4. Fix deadline warning accuracy for in-flight interrupted subqueries.
5. Reject unknown explicit `repo_search.host` values.
6. Normalize warning prefixes and de-duplicate obvious untrusted warning duplication.
7. Preserve/parse OSV CVSS vector/score where feasible.
8. Update README/AGENTS/CHANGELOG.
9. Run validation commands and add CI if absent.

## Acceptance criteria

This line of work is closed when:

- Package/ecosystem/version `security_search` requests produce native OSV vulnerabilities when OSV is enabled.
- OSV no longer sends arbitrary prose as `package.name`.
- Security orchestration is no longer embedded as a large workflow inside MCP tool glue.
- `repo_search` and `research_search` emit `request_deadline_exceeded` for both skipped and interrupted subqueries.
- Unknown explicit repo host strings are rejected with validation errors.
- Specialized warning prefixes are stable and documented.
- OSV CVSS/severity behavior is either improved or explicitly documented/tested as partial.
- README/AGENTS/CHANGELOG match actual behavior.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`, and `cargo test` pass.
- GitHub or local validation evidence is available for the final commit.

## Handoff note

Keep this pass narrow. The goal is not to add NVD, GHSA GraphQL, RustSec native indexing, or deep source-quality taxonomy work. The current implementation is already broadly useful; it needs final semantic closure so codegg can rely on it without compensating for misleading advisory behavior or unbounded/ambiguous responses.
