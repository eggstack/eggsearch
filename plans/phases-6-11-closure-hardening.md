# Phases 6–11 Closure Hardening Plan

## Purpose

The follow-on implementation pass landed substantial code for phases 6–11: `batch_fetch`, security-context enrichment, exact-error mode, GitLab/Gitea providers, result quality metadata, and research workflows. This plan is a closure pass for those implementations. It should harden semantics, remove inconsistencies, and add targeted verification. It should not add another feature tranche.

The current repo is in a strong expansion state, but not yet in closure shape. The modules and tool surfaces exist. The remaining work is to make the behavior match the handoff plans precisely enough for codegg to consume without compensating for edge cases.

## Scope

This pass covers:

1. Batch-fetch execution semantics and validation.
2. GitLab/Gitea native-provider correctness and capability reporting.
3. Exact-error configuration and redaction integration.
4. Provider-selection telemetry preservation.
5. Quality metadata population across result paths.
6. Security-context safety and source-quality tests.
7. Research workflow backward compatibility, gaps, and telemetry.
8. Documentation alignment and full verification.

Do not add Sourcegraph, new package ecosystems, autonomous crawling, background research, model judging, active security scanning, or new top-level tools beyond the surfaces already present.

## Current state summary

Implemented surfaces now present:

- `batch_fetch` core request/response types and MCP tool registration.
- Batch config fields under `[fetch]`.
- `security_search` richer arguments and a larger normalized security model.
- `repo_search.mode = exact_error` plus `error_query` parser/planner.
- GitLab and Gitea code/issues/releases engine modules.
- Provider descriptors for GitLab/Gitea providers.
- `ResultQuality` model and `SourceCard.quality` field.
- Research workflow arguments and workflow context/telemetry plumbing.

Known closure gaps:

- `batch_fetch` currently executes sequentially despite a concurrency config/semaphore.
- Batch web item prevalidation only checks empty URL; lower-level fetch likely handles scheme/policy, but the batch tool does not satisfy its prevalidation contract.
- GitLab is omitted from repo-search native-provider capability checks and warnings in several places.
- GitLab code results currently emit `ResultMetadata::None`, weakening host-native `CodeEvidence` generation.
- Exact-error validation uses `max_query_chars.max(8000)` instead of the configured `search.exact_error.max_error_chars`.
- Provider-selection telemetry may be reset/defaulted in adapter response and must be verified through the MCP wrapper.
- Quality metadata exists structurally, but population must be proven across web, repo, local, security, and research paths.
- Security enrichment needs tests that keep exploit context defensive and advisory/source-quality oriented.
- Research workflow behavior needs backward-compatibility and deterministic gap tests.

## Task 1: Batch-fetch closure

### 1.1 Decide and enforce execution semantics

The phase plan requested bounded concurrency. The current implementation creates a semaphore but awaits each fetch inside a serial loop. Choose one of two options:

Preferred option: implement actual bounded concurrency.

- Validate all effective items first.
- Spawn tasks for items up to `batch_concurrency`.
- Preserve input order in the final `results` vector.
- Apply `continue_on_error` deterministically. For `continue_on_error = false`, avoid launching later items once an earlier item has failed. If concurrency makes this ambiguous, use ordered waves: launch up to concurrency, wait for the wave, stop scheduling later waves after the first ordered failure.
- Keep total-budget enforcement deterministic. The simplest safe approach is ordered waves with a shared remaining budget computed before each item is scheduled. Avoid races where two concurrent items both assume the same remaining budget.

Acceptable option: document and rename the behavior as sequential bounded batch.

- Remove or de-emphasize `batch_concurrency` until true concurrency is implemented.
- Change provider/tool status wording from concurrency claims to sequential bounded fan-out.
- This is less desirable because config already exposes `batch_concurrency`.

Recommendation: implement ordered bounded waves. It gives meaningful fan-out while preserving deterministic budgets and ordered abort behavior.

### 1.2 Strengthen prevalidation

Prevalidate all items before I/O:

- Web URL must be non-empty.
- Web URL must parse successfully.
- Web URL scheme must be `http` or `https`.
- Per-item `max_chars`, if present, must be > 0.
- Repo owner/repo/path validation should mirror `RepoFetchRequest.validate` as much as possible.
- Repo path must reject absolute paths and traversal, not just substring `..`.
- Host aliases should normalize before validation and dispatch.

Do not duplicate network/private-address policy if `FetchClient` owns it, but make batch validation errors stable and early for malformed URLs and impossible repo locators.

### 1.3 Budget correctness

Add tests proving:

- `max_total_chars` above cap is rejected, not silently clamped, if that is the documented policy.
- `max_items` above cap is rejected or clamped consistently with docs.
- Per-item `max_chars` cannot exceed remaining total budget.
- `total_chars_returned` never exceeds `max_total_chars` except for unavoidable framing/metadata overhead if documented.
- Budget exhaustion warnings are emitted once and are machine-readable.

If current behavior intentionally clamps caller-supplied limits, update docs to say that. The earlier plan preferred rejecting over-cap total budgets; align code and docs.

### 1.4 Tests

Add or update tests for:

- Malformed URL rejected before fetch.
- Unsupported scheme rejected before fetch.
- Absolute repo paths rejected.
- Traversal variants rejected: `../x`, `a/../b`, `%2e%2e` where applicable.
- Bounded concurrency/wave ordering.
- `continue_on_error = false` behavior.
- Total budget exhaustion with mixed web/repo items.
- Result order preserved under delayed mock responses.

## Task 2: GitLab/Gitea provider hardening

### 2.1 Fix native-provider detection

Update repo-search native capability checks to include GitLab everywhere GitHub/Gitea are counted:

- `has_native_code`: `github_code`, `gitlab_code`, `gitea_code`.
- `has_native_issues`: `github_issues`, `gitlab_issues`, `gitea_issues`.
- `has_native_releases`: `github_releases`, `gitlab_releases`, `gitea_releases`.

Fix all warnings that currently mention only GitHub or only GitHub/Gitea. Warnings should say “native code-host provider” or include the actual selected provider IDs.

### 2.2 Provider-status accuracy

`provider_status.code_hosts` currently aggregates by host kind. Verify it includes:

- GitHub.
- GitLab.
- Gitea.
- Accurate `enabled` and `configured` flags.
- Accurate `code_search`, `issue_search`, and `release_search` flags.
- No token or sensitive base URL leakage unless explicitly intended.

If self-hosted instance names are not implemented yet, document that only built-in provider IDs are supported in this pass.

### 2.3 GitLab result metadata

GitLab code results currently convert to `SearchResult` with `ResultMetadata::None`. Strengthen this so host-native code results carry enough structured metadata for downstream `CodeEvidence`:

- host = GitLab.
- owner/namespace if derivable.
- repo if derivable.
- project id if namespace/repo cannot be derived.
- path.
- ref if returned by API or defaulted.
- browser URL.
- language when derivable from path.
- source role from path.

If the GitLab API response URL is project-id based and namespace/repo are not available, add a warning or metadata variant rather than inventing owner/repo. Do not fabricate structured repo-fetch locators when the locator is not valid.

### 2.4 GitLab/Gitea URL and namespace tests

Add tests for:

- GitLab nested namespace URL encoding for API calls.
- GitLab code result conversion with path/ref/project URL.
- GitLab issues and releases result metadata.
- Gitea base URL handling.
- Gitea code/issues/releases capability descriptors.
- GitLab providers counted in native-provider warnings.
- `host:gitlab` does not emit false native-provider-unavailable warnings when GitLab providers are selected.

## Task 3: Exact-error mode closure

### 3.1 Use configured exact-error limits

`RepoSearchRequest.validate` currently allows exact-error queries up to `max_query_chars.max(8000)`. That bypasses `SearchSection.exact_error.max_error_chars`.

Change validation so the MCP layer supplies the correct cap:

- For normal mode, use `search.max_query_chars`.
- For exact-error mode, use `search.exact_error.max_error_chars`.
- If `search.exact_error.enabled = false`, reject `mode = exact_error` with a clear validation error.

Recommended approach:

- Add `RepoSearchRequest::validate_with_config(&SearchSection)` or pass both normal and exact-error caps into validation.
- Avoid hardcoded `8000` except as the default config value.

### 3.2 Planner uses config

Ensure exact-error planner uses the configured values:

- `enabled`.
- `max_subqueries`.
- `max_error_chars`.
- `redact_sensitive_tokens`.
- `prefer_official_docs`.

The adapter currently creates `ExactErrorConfig::default()` inside repo search. Replace that with request/server config propagated from the MCP layer or adapter state. If adapter does not currently store config, pass an exact-error config reference through the call chain.

### 3.3 Redaction completeness

Audit redaction behavior:

- Redact home directories.
- Redact local absolute paths.
- Redact obvious API tokens/secrets.
- Redact UUIDs and memory addresses.
- Avoid leaking sensitive original text into provider-dispatched subqueries.
- It is acceptable for response `original_error` to include original text only if documented as local response content. Consider adding `redacted_error` if codegg needs to inspect what was sent externally.

### 3.4 Fix suspicious code patterns

In `generate_error_subqueries`, remove odd patterns such as converting `code.tool.as_str()` into an `Option` when the value is always present. Keep the implementation simple and explicit.

### 3.5 Tests

Add tests for:

- Exact-error disabled rejects mode.
- Exact-error max chars uses config, not hardcoded 8000.
- `max_subqueries` uses config.
- Redacted provider subqueries do not contain home path, token, UUID, or memory address.
- `original_error` and dispatched queries are distinguishable in response context.
- Rust, TypeScript, Python, npm/pnpm/yarn parsing still pass.

## Task 4: Provider-selection telemetry preservation

### Problem

The adapter constructs `RepoSearchTelemetry` with `ProviderSelectionTelemetry::default()`. Earlier corrective work made profile telemetry meaningful: requested/applied profile, degraded, partial, skipped providers, and reason. The MCP wrapper may overwrite this later, but the boundary is fragile.

### Required behavior

For `repo_search`, final responses must preserve:

- `profile_requested`.
- `profile_applied`.
- `degraded` only for full fallback.
- `partial` for partial profile provider availability.
- `skipped_providers`.
- stable `reason`.

### Implementation options

Preferred:

- Compute provider-selection telemetry once in `run_repo_search` after config/provider resolution.
- Pass it into `adapter.repo_search` or overwrite exactly once after adapter response.
- Add tests against the public MCP/tool output, not only adapter internals.

Avoid:

- Adapter defaulting telemetry and relying on a later undocumented overwrite.
- Duplicating profile resolution in multiple places.

### Tests

Add tests for:

- Coding profile all providers available.
- Coding profile some providers unavailable.
- Coding profile all profile providers unavailable and defaults used.
- Explicit unavailable provider remains validation error.
- GitLab native provider available is reflected in provider attempts/capabilities.

## Task 5: Quality metadata population

### Problem

`ResultQuality` exists and `SourceCard` has a `quality` field, but closure requires proof that it is populated consistently and not just structurally present.

### Required behavior

Populate `quality` for all `SourceCard` values after aggregation and grouping:

- Generic web results.
- Repo/source-code results.
- Local workspace results.
- Security advisory results.
- Research results.
- Native code-host results.

Rules should remain deterministic and cheap. No model calls.

### Suggested implementation

Add a single quality enrichment pass:

```rust
pub fn enrich_quality(card: &mut SourceCard, query_context: &QualityContext)
```

Run it in:

- generic search aggregation,
- repo grouping or immediately before response,
- security grouping,
- research grouping,
- local source-card conversion.

Quality should consider:

- `SourceKind`.
- rank reasons.
- code evidence.
- vulnerability metadata.
- timestamps.
- provider IDs.
- exact-error context when applicable.

### Aggregate uncertainty

Ensure `SearchUncertaintySummary` reflects real response state:

- provider failures count.
- degraded/partial provider selection from profile telemetry.
- low-confidence result count.
- useful warnings such as generic-only, no exact matches, no timestamps.

Do not leave `degraded_provider_selection` and `partial_provider_selection` hardcoded false if provider telemetry says otherwise.

### Tests

Add tests for:

- Code result with raw permalink -> high confidence / commit-pinned quality reason.
- Generic snippet-only result -> lower confidence and uncertainty reason.
- Official docs -> official authority.
- Package registry -> package-registry authority.
- Security advisory -> primary/package advisory authority.
- Local workspace source card gets quality.
- Research results get quality.
- Aggregate uncertainty counts low-confidence results correctly.

## Task 6: Security-context closure

### 6.1 Safety and semantics audit

Verify `security_search` remains retrieval/context only:

- No exploit execution.
- No payload generation.
- No active target validation.
- `include_exploit_context` returns source-card context and warnings, not procedural exploit instructions.
- Defensive guidance categories remain mitigation-oriented.

### 6.2 Advisory source quality

Ensure `security_context.source_quality` is derived from actual returned source cards/advisory metadata, not only from optimistic assumptions.

For package advisory lookups:

- If advisory provider returns vulnerabilities, source tier can be package/advisory tier.
- If only generic web results exist, source quality should reflect the actual source cards.
- If version matching is not possible, emit warning rather than implying vulnerability.

### 6.3 Tests

Add tests for:

- CVE query produces exact identifier context.
- GHSA query produces exact identifier context.
- CWE query produces weakness-class context.
- Package+version query with no advisory match emits no false vulnerability claim.
- Version comparison unknown emits warning.
- Exploit-context flag does not produce executable/procedural exploit payload fields.
- Defensive guidance categories are mitigation-oriented.
- Source quality matches advisory/blog/forum inputs.

## Task 7: Research workflow closure

### 7.1 Backward compatibility

Existing `research_search` calls without workflow/depth/compare-targets must behave as before:

- No required new fields.
- Default workflow absent or `general` does not force extra gaps unexpectedly.
- Response remains stable for old clients that ignore new fields.

### 7.2 Deterministic workflow behavior

Verify workflow helper behavior:

- Architecture decision dimensions are deterministic.
- API evaluation dimensions are deterministic.
- Library comparison handles multiple compare targets.
- Depth controls subquery breadth.
- Diversity caps are deterministic and documented.

### 7.3 Coverage gaps

Coverage gaps should be helpful but not noisy:

- `NoPrimarySources` only when primary/official sources are actually absent.
- `NoCounterpoints` only when counterpoints requested or workflow implies them.
- `NoBenchmarks` only for performance/library comparison workflows where benchmarks matter.
- `NoSecurityDiscussion` only for security/performance/architecture workflows when requested or relevant.

### 7.4 Tests

Add tests for:

- Research search without workflow remains compatible.
- Architecture decision dimensions and gaps.
- Library comparison compare-target handling.
- Depth affects subquery count.
- Diversity cap warning is stable.
- Suggested next fetches are diverse and bounded.
- Telemetry reports workflow/depth/dimensions/gaps.

## Task 8: Documentation alignment

Update README and AGENTS.md to match final behavior, not aspirational behavior.

Required updates:

- `batch_fetch`: state whether it is truly concurrent or sequential/wave-bounded. Document item caps, total caps, and error behavior.
- GitLab/Gitea providers: document provider IDs, token env config, base URL behavior, and capability limits.
- Exact-error mode: document config fields and redaction behavior.
- Quality metadata: document heuristic nature and how codegg should consume it.
- Security context: document defensive/retrieval-only boundaries.
- Research workflows: document that they are deterministic scaffolding, not autonomous browsing.

Avoid large prose rewrites. Keep docs tied to tested behavior.

## Task 9: Verification

Run:

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Targeted test commands to add/run:

```bash
cargo test batch_fetch
cargo test gitlab
cargo test gitea
cargo test exact_error
cargo test error_query
cargo test quality
cargo test security_context
cargo test research_workflow
cargo test provider_selection
```

If GitHub status checks are not available, record local command output in the implementation summary.

## Acceptance criteria

This closure pass is complete when:

- `batch_fetch` semantics match docs and tests, including concurrency or clearly documented sequential behavior.
- Batch item validation is early and deterministic.
- GitLab providers are included in all native-provider capability checks and warnings.
- GitLab/Gitea result metadata is sufficient for correct source-card/code-evidence behavior or explicitly warns when structured locators cannot be derived.
- Exact-error mode uses configured enablement, max chars, max subqueries, and redaction settings.
- Provider-selection telemetry remains meaningful in final public `repo_search` output.
- Quality metadata is populated across major result paths and aggregate uncertainty is not hardcoded.
- Security context remains defensive/retrieval-only and has identifier/source-quality tests.
- Research workflows are backward-compatible and deterministic.
- README/AGENTS describe actual behavior.
- `cargo fmt`, clippy, and full tests pass.

## Suggested implementation order

1. Fix GitLab native-provider checks and warnings.
2. Fix exact-error config propagation and validation.
3. Fix/decide batch-fetch execution semantics and validation.
4. Preserve provider-selection telemetry cleanly.
5. Add quality enrichment pass and aggregate uncertainty wiring.
6. Harden GitLab/Gitea metadata conversion.
7. Add security-context defensive/source-quality tests.
8. Add research workflow compatibility/gap tests.
9. Update docs.
10. Run full verification.

Keep changes focused. If a task exposes a larger architectural problem, document it separately rather than expanding this closure pass indefinitely.
