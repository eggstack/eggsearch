# Phase 1–5 Corrective Closure Plan

## Purpose

This plan closes the remaining gaps found after the provider truthfulness, MCP contract, structured warning, bounded dispatch, and deterministic identity passes. The repository is in substantially better shape, but a few details still matter for coding-agent correctness: API-only validation must reflect configured providers rather than merely enabled providers, docs examples must match real schemas, warning conversion should avoid misleading fallback classifications, dispatch must preserve scheduling priority while remaining bounded, and deterministic identity should use a durable hash/canonicalization contract.

The intent is a narrow corrective pass. Do not expand this into phase 6 feature work. This pass should leave phases 1–5 genuinely closed and safe to build on.

## Current state summary

The implemented work since the roadmap is broadly aligned with the first five plans:

- Provider state was centralized enough to remove duplicate API descriptors and expose stable routing skip reason codes.
- Module docs, tool descriptions, a tool matrix, and workflow docs were added for the ten stable MCP tools.
- Structured warnings were added with stable codes, severity, deduplication, legacy string compatibility, and follow-up coverage for fetch, batch fetch, repo fetch, repo map, and evidence bundle responses.
- Multiquery dispatch was refactored from spawn-all semaphore gating to a queue-based executor with global and per-provider caps.
- Deterministic identity was added for source cards, fetches, suggested fetches, batch fetches, repo locators, documents, chunks, and evidence bundle linkage.

The remaining issues are concentrated and should be corrected before starting the next feature phase.

## Workstream 1: API provider validation must require configured API providers

### Problem

The provider truthfulness pass appears to allow live mode when all scrape providers are disabled and at least one API provider is merely enabled. That is weaker than the intended acceptance criterion. API-only live mode should be valid only when at least one API provider is actually configured enough to build or route: enabled, known, has a non-empty `api_key_env`, and the referenced environment variable is present unless the provider explicitly supports anonymous operation.

### Required behavior

In `Mode::Live`:

- A deployment with at least one enabled traditional provider remains valid.
- A deployment with no enabled traditional providers but at least one enabled-and-configured API provider is valid.
- A deployment with no enabled traditional providers and only API providers missing required env vars is invalid.
- A deployment with only unknown API provider IDs should not count as configured live capability.
- The validation error should mention that enabled API providers require resolvable credentials.

### Implementation guidance

Create a helper instead of duplicating conditions inline:

```rust
fn api_provider_is_configured(id: &str, cfg: &ApiProviderConfig) -> bool {
    if !cfg.enabled {
        return false;
    }
    if !API_PROVIDER_IDS.contains(&id) {
        return false;
    }
    match cfg.api_key_env.as_deref() {
        Some(env) if !env.is_empty() => std::env::var(env).is_ok(),
        _ => false,
    }
}
```

If the code already has runtime `api_configured` logic in adapter construction, consider sharing the same helper or moving it into a core provider/config utility. Do not let config validation and runtime adapter construction diverge.

### Required tests

- Live mode with all scrape providers disabled and `brave_api` enabled with present env var validates.
- Live mode with all scrape providers disabled and `brave_api` enabled with missing env var fails.
- Live mode with all scrape providers disabled and `brave_api` enabled with empty `api_key_env` fails.
- Live mode with all scrape providers disabled and only unknown API provider ID enabled fails or does not count toward live capability.
- Live mode with one enabled scrape provider and no API providers still validates.
- Provider status for missing-key API provider remains enabled but not configured.

### Acceptance criteria

- API-only live mode means configured API-only, not merely enabled API-only.
- Tests clean up any environment variables they set.
- Error messages do not leak secret values.

## Workstream 2: Validate documentation examples against real schemas

### Problem

The new workflow docs are directionally useful, but at least two example fields appear likely schema-invalid: `repo_search` examples use `intent`, and a `research_search` example uses `include_benchmarks`. These fields are not part of the earlier argument shapes. Agent-facing examples must be exact; otherwise agents copy invalid payloads.

### Required behavior

Every JSON or JSONC example in `docs/agent-workflows.md`, `docs/tool-matrix.md`, README excerpts, and AGENTS guidance must match the actual MCP argument structs. Where a concept is desired but no field exists, express it using existing fields such as `profile`, `mode`, `desired_source_types`, `include_primary_sources`, `compare_targets`, `constraints`, or `known_context`.

### Implementation guidance

Add a test that extracts fenced `json`/`jsonc` objects from docs where feasible and validates them against typed argument deserialization. If full extraction is too costly for this pass, at minimum add explicit tests containing the examples from `docs/agent-workflows.md` and deserialize them into the corresponding argument structs.

Likely replacements:

- Remove `intent` from `repo_search`; use `profile: "coding"`, `include_issues`, `include_releases`, `symbol`, `language`, and repo locator fields.
- Replace `include_benchmarks` in `research_search` with `desired_source_types` or `known_context` / `constraints` describing benchmarks.
- Ensure enum values match actual parsers. If `ResearchDomain` expects snake_case/lowercase rather than `SoftwareArchitecture`, use the accepted value.
- Ensure ecosystem values match `PackageEcosystem::parse` accepted inputs.
- Ensure evidence bundle examples use actual shapes, not placeholder arrays of strings where structs are expected. If placeholders are necessary, label them prose-only outside code blocks or use comments that make them non-runnable.

### Required tests

- Deserialize repo map example into `RepoMapArgs`.
- Deserialize repo search example into `RepoSearchArgs`.
- Deserialize repo fetch example into `RepoFetchArgs`.
- Deserialize exact error example into `RepoSearchArgs`.
- Deserialize security example into `SecuritySearchArgs`.
- Deserialize research example into `ResearchSearchArgs`.
- Either deserialize evidence bundle example into `EvidenceBundleArgs` or remove runnable-looking JSON if the example requires large nested objects.

### Acceptance criteria

- No documented runnable example includes fields absent from its tool schema.
- Docs still explain the intended workflow clearly.
- The repo has a test preventing schema drift in future docs examples.

## Workstream 3: Add neutral fallback warning codes

### Problem

`convert_fetch_warnings()` currently maps unrecognized fetch warning strings to `ProviderFailed`. That is semantically misleading. An unrecognized warning may be a truncation note, extraction limitation, parse note, or generic advisory; it should not be treated as provider failure unless it is actually provider failure.

### Required behavior

Unrecognized fetch-layer warnings should map to a neutral code such as `fetch_warning` or `unknown_warning`, not `provider_failed`. The structured warning system should preserve the original message and use a warning-level severity unless a stronger code is known.

### Implementation guidance

Add one or both of these warning codes:

- `FetchWarning`: generic fetch-layer warning.
- `UnknownWarning`: generic unclassified warning.

Recommended mapping:

- `convert_fetch_warnings()` unrecognized fallback -> `FetchWarning`.
- `search_warning_to_agent_warning()` unrecognized fallback -> `UnknownWarning` or existing generic warning if one exists.
- Provider failure strings should still map to provider-specific failure codes when recognized by provider failure format or explicit prefixes.

Update `WarningCode::as_str`, `default_severity`, `default_recommended_action`, tests enumerating variants, and AGENTS/docs counts if they list the number of codes.

### Required tests

- Unknown fetch warning maps to `fetch_warning`, not `provider_failed`.
- Recognized provider failure still maps to `provider_failed`/`provider_timeout`/`provider_rate_limited` as appropriate.
- Unknown generic search warning maps to neutral `unknown_warning` or equivalent.
- Legacy warning strings still preserve the original text.

### Acceptance criteria

- No generic fallback path incorrectly emits provider failure semantics.
- Agent warning severity remains conservative.
- Existing recognized warning conversions continue to pass.

## Workstream 4: Preserve scheduling priority in bounded dispatch

### Problem

The queue-based dispatcher uses `swap_remove(i)` when starting a pending job. This keeps removal O(1), but it mutates the pending queue order by moving the final pending element into the current slot. Because the dispatcher scans forward repeatedly, this can start lower-priority jobs before earlier eligible jobs in later iterations. Final result sorting is deterministic, but scheduling priority semantics are weakened.

### Required behavior

Dispatch should preserve the sorted pending order while still allowing scan-forward around provider-capacity blocks. The executor may start a later job when earlier jobs are blocked by per-provider capacity, but it should not reorder the remaining queue incidentally.

### Implementation guidance

Prefer correctness over O(1) removal. Job counts are bounded and small enough that `Vec::remove(i)` is acceptable. If performance is a concern, maintain a `VecDeque` plus a `started` bitmap or a binary heap / stable ready queue. The simple fix is:

```rust
pending_queue.remove(i);
```

and then do not increment `i`, preserving the next element at the same index.

Also remove or use the currently inert `pending_pos` variable if it remains permanently zero. It adds confusion and can hide future bugs.

Improve deadline accounting if feasible:

- Track `started_subquery_ids` explicitly.
- On deadline, queued-not-started subqueries should contribute to `subqueries_skipped`.
- Running-not-completed subqueries should contribute to `subqueries_interrupted`.
- Avoid counting the same subquery as both skipped and interrupted unless the semantics explicitly allow a subquery with multiple jobs to have both states.

### Required tests

- Construct jobs with mixed providers and priorities where provider A is saturated and provider B can run; verify scan-forward starts B without permuting later priority order.
- Verify two eligible jobs with different priorities start in priority order after earlier blocked jobs clear.
- Verify pending order is stable after multiple removals.
- Verify skipped vs interrupted subquery counts do not double-count the same subquery in simple cases.
- Existing global/per-provider concurrency tests still pass.

### Acceptance criteria

- The executor remains true bounded dispatch.
- Scheduling order respects priority except for intentional provider-capacity bypass.
- Final output ordering remains deterministic.
- Deadline telemetry is more precise or at least not regressed.

## Workstream 5: Replace `DefaultHasher` with an explicit stable identity hash

### Problem

The identity system uses `DefaultHasher` for deterministic IDs. This may be deterministic within a given build/run today, but it is not a good external identity contract. Rust does not guarantee `DefaultHasher` as a stable algorithm for persisted IDs across compiler versions/platforms. Cross-tool and cross-version identity should use an explicit, documented hash algorithm.

### Required behavior

Stable IDs should be generated with a fixed algorithm and versioned canonical input. The output can remain compact, but the implementation should not depend on stdlib hasher internals.

### Recommended implementation

Use one of these options:

1. `blake3` truncated to 128 bits or 64 bits.
2. `sha2::Sha256` truncated to 128 bits or 64 bits.
3. A small explicit FNV-1a 64-bit implementation if avoiding dependencies is critical.
4. `twox-hash` with fixed seed if non-cryptographic speed is preferred.

For evidence/provenance IDs, prefer BLAKE3 or SHA-256 truncated to 16 hex chars minimum. If dependency sensitivity matters, use an internal FNV-1a implementation and document the collision tradeoff. The important point is explicit algorithm stability.

Add an identity input version prefix before hashing:

```text
eggsearch-id-v1\0source\0...
```

This allows future canonicalization changes without silently reusing the same ID namespace.

### Canonicalization correction

Review `canonicalize_url()` host behavior. Stripping `www.` globally may merge resources that are not equivalent. For evidence identity, conservative behavior is safer:

- Lowercase scheme and host.
- Remove default ports.
- Strip fragments.
- Normalize percent-encoding in path as currently implemented.
- Preserve query parameters.
- Consider preserving `www.` by default.

If `www.` stripping is retained, document it as a deliberate dedupe heuristic and add tests for potential host-distinction concerns. Prefer moving host alias rules behind an explicit equivalence function rather than applying them globally.

### Required tests

- Known fixture inputs produce exact expected stable IDs. These should be golden tests to detect accidental algorithm changes.
- Same input produces same ID across repeated calls.
- Different entity type prefixes do not collide for the same fields.
- URL canonicalization still normalizes percent encodings as intended.
- `www.example.com` and `example.com` behavior is explicitly tested according to the chosen policy.
- Query parameters remain identity-significant.
- Fragment differences do not affect identity.

### Acceptance criteria

- No identity path uses `DefaultHasher` for public stable IDs.
- Hash algorithm and ID version are documented in `identity.rs` and AGENTS/docs.
- Golden tests protect stable outputs.
- Canonicalization is conservative and documented.

## Workstream 6: Final verification pass

After all workstreams are complete, run or require the following locally:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --no-default-features
```

If the repo has feature combinations beyond those commands, include the relevant matrix from existing CI/docs.

Add a short changelog entry under Unreleased summarizing this as a phase 1–5 corrective closure.

## Suggested commit structure

Use small commits in this order:

1. `fix(config): require configured API providers for API-only live mode`
2. `docs/tests: validate agent workflow examples against MCP args`
3. `fix(warnings): add neutral fallback warning codes`
4. `fix(dispatch): preserve pending priority order in bounded executor`
5. `fix(identity): use explicit stable hash for public IDs`
6. `docs: record phase 1-5 corrective closure`

## Completion checklist

- [ ] API-only live mode requires at least one configured API provider.
- [ ] Provider status still reports missing-key providers accurately.
- [ ] All workflow examples deserialize against real argument structs or are clearly marked non-runnable.
- [ ] Unknown fetch warnings no longer map to provider failure.
- [ ] Dispatch pending queue removal preserves priority order.
- [ ] Deadline skipped/interrupted telemetry is tested.
- [x] Public stable IDs no longer use `DefaultHasher`.
- [x] Identity algorithm has versioned input and golden tests.
- [x] URL canonicalization policy around `www.` is documented and tested.
- [x] fmt, clippy, and tests pass.
