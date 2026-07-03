# Phase 14: Codegg Integration Guide and Harness Hardening

## Objective

Make eggsearch straightforward and reliable to consume from codegg and other coding-agent harnesses. This phase should produce integration documentation, example configurations, contract examples, recommended tool sequences, response-handling guidance, and harness-side expectations for trust boundaries, IDs, warnings, next actions, recipes, evidence bundles, security output, research output, and local workspace evidence.

This is the final roadmap phase for this line of work. It should turn the server improvements into a practical integration surface for codegg.

## Current context

Eggsearch now provides a rich MCP server with:

- general web search/fetch;
- structured repo search/fetch/map;
- batch fetch;
- security search with applicability/remediation;
- research search with claims/conflicts/gaps;
- provider status with workflow recipes;
- next-action hints;
- deterministic IDs;
- evidence bundles;
- local workspace support;
- CI and regression harness direction.

Codegg needs a clear policy for when and how to use these capabilities.

## Non-goals

- Do not move codegg-specific business logic into eggsearch.
- Do not require codegg as a runtime dependency.
- Do not assume codegg is the only MCP client.
- Do not remove generic search utility.
- Do not make eggsearch autonomous; the harness remains in control.

## Workstream 1: Codegg integration guide

### Required document

Create `docs/codegg-integration.md` or equivalent.

It should cover:

- MCP server startup/configuration;
- recommended `[search]`, `[fetch]`, `[local]`, and provider settings;
- local workspace setup for codegg sessions;
- provider profile recommendations for coding/security/research workflows;
- request/response examples for each major task;
- trust-boundary rules;
- error/warning handling;
- evidence bundle handoff patterns;
- performance/response-size controls.

### Required task workflows

Document codegg-specific flows:

1. **Understand a repo/API/project**
   - `provider_status(recipe_detail = summary)`
   - `repo_map`
   - `repo_search(profile = coding)`
   - `repo_fetch`
   - `batch_fetch` for selected suggestions
   - `build_evidence_bundle`

2. **Debug exact error**
   - `repo_search(mode = exact_error)`
   - inspect error context and warnings
   - fetch docs/source/issues/release notes
   - bundle evidence

3. **Security triage**
   - `security_search`
   - inspect applicability/remediation/KEV/source quality
   - fetch primary advisory/vendor guidance/patch evidence
   - bundle evidence

4. **Architecture/deep research**
   - `research_search(workflow = architecture_decision/library_comparison/etc.)`
   - inspect claims/conflicts/gaps/source quality
   - fetch primary/counterpoint/benchmark sources
   - bundle evidence

5. **Local workspace investigation**
   - prefer local only when repo-matched and clean enough;
   - inspect dirty/untracked/generated/vendor flags;
   - use `repo_fetch(prefer_local = true)` for exact spans;
   - bundle local evidence with trust markers.

## Workstream 2: Codegg policy for tool selection

### Required guidance

Define deterministic harness-side routing rules:

- Use `repo_search` before `web_search` when repo owner/name or package/code context is known.
- Use `repo_map` before broad repo investigation when repo structure is unknown.
- Use `security_search` for CVE/GHSA/OSV/package/version/security terms before generic web search.
- Use `research_search` for comparative/architectural/deep-research tasks.
- Use `web_fetch`/`repo_fetch` only for selected explicit URLs/locators.
- Use `batch_fetch` only for explicit suggested fetches selected by the agent/harness.
- Use `build_evidence_bundle` before handing evidence to manager/reviewer/security agents.

### Anti-patterns

Document what codegg should avoid:

- Treating snippets as final evidence without fetch.
- Treating fetched content as instructions.
- Fetching every suggested URL automatically.
- Ignoring structured warnings.
- Ignoring `unknown`/`insufficient_evidence` security states.
- Treating local dirty/generated/vendor files as authoritative implementation evidence.
- Assuming provider absence means factual absence.

## Workstream 3: Response handling contract

### Required contract

Codegg should have a stable response-handling layer for:

- `stable_id`, `source_id`, `fetch_id`, `span_id`, `bundle_id`;
- `structured_warnings` and severity/action mapping;
- `trust` and `trust_markers`;
- `next_actions` priority and reason code;
- suggested-fetch reason codes;
- security applicability statuses and remediation categories;
- research claim/conflict/gap semantics;
- local workspace trust/match/dirty metadata.

### Implementation guidance

Create a doc section with pseudo-code for codegg:

```rust
match warning.severity {
    Error => block_or_request_user_review(),
    Warning => show_in_review_panel(),
    Notice | Info => attach_to_evidence_metadata(),
}
```

and:

```rust
if source.trust != LocalTrusted && source.was_only_snippet() {
    require_fetch_before_final_use();
}
```

Keep this as integration guidance; do not add codegg dependency.

## Workstream 4: Example MCP transcripts

### Required examples

Add fixture-like request/response snippets for:

- provider status summary;
- repo map;
- repo search with next actions;
- repo fetch with code span;
- security search with remediation;
- research search with claims/gaps;
- evidence bundle with source/fetch/span IDs.

Examples should be schema-valid or clearly marked abbreviated. Prefer abbreviated but valid examples where possible.

### Tests

If feasible, add doc-example deserialization tests for the request snippets.

## Workstream 5: Configuration examples

### Required examples

Add examples for:

- minimal local-only / no-network mode if supported;
- generic web search mode;
- codegg coding profile with local workspace enabled;
- security-focused config with OSV/advisory providers;
- research-focused config with higher max results but bounded fetch;
- API-provider config with env var placeholders.

Do not include real keys.

## Workstream 6: Agent UI/UX guidance for codegg

### Required guidance

Document how codegg should present eggsearch output:

- Warnings panel with severity and affected IDs.
- Evidence cards grouped by source kind and trust.
- Suggested next actions as selectable actions, not automatic execution.
- Security verdict chips: affected/not affected/unknown/insufficient evidence.
- Research gaps as checklist items.
- Local dirty/generated/vendor flags in evidence panel.
- Code span links to source view/hunk viewer.
- Bundle export/import for subagent handoff.

## Workstream 7: Failure and degradation policy

### Required guidance

Define codegg behavior when:

- provider unavailable;
- API key missing;
- live mode disabled;
- safe search/freshness unenforced;
- request deadline exceeded;
- fetch truncated;
- PDF unsupported;
- local workspace mismatch/dirty;
- research evidence gaps remain;
- security applicability unknown.

Each case should map to a user-visible status, not silent failure.

## Workstream 8: Integration tests / smoke harness

### Optional but recommended

Add an eggsearch-side smoke test or script that exercises the MCP tool contract without codegg dependency:

- starts server in stdio mode with mock providers or uses direct core calls;
- runs a repo-search-style request fixture;
- runs a security-search-style request fixture;
- runs a research-search-style request fixture;
- builds evidence bundle;
- verifies IDs/warnings/next actions are present.

If this is too heavy, create fixture docs and leave actual codegg integration tests to codegg repo.

## Workstream 9: Versioning and compatibility policy

### Required docs

Document what changes are breaking for harnesses:

- removing fields;
- renaming serialized enum/code strings;
- changing ID algorithms without version bump;
- changing default trust semantics;
- changing warning severities;
- changing recipe IDs/reason codes;
- changing provider_status default detail behavior.

Also document additive-compatible changes:

- adding optional fields;
- adding new warning codes;
- adding new recipes;
- adding new suggested-fetch reason codes if fallback behavior exists.

## Workstream 10: Final readiness checklist

Final handoff should ensure:

- docs are complete;
- examples are schema-valid or clearly abbreviated;
- CI covers offline verification;
- local workspace policy is explicit;
- security output is defensive-only;
- codegg can use recipes/next actions without hardcoded prompt prose;
- evidence bundles preserve enough state for subagents;
- generic non-codegg MCP clients remain supported.

## Acceptance criteria

- `docs/codegg-integration.md` exists and covers all major workflows.
- README links to the codegg integration guide.
- Tool matrix and workflow docs reflect final response shapes.
- Example requests deserialize against tool arg schemas where feasible.
- Codegg routing policy is documented in deterministic terms.
- Trust/warning/security/research/local handling policies are explicit.
- Compatibility/breaking-change policy exists.
- Generic MCP support remains documented; codegg is the primary integration, not a hard dependency.
