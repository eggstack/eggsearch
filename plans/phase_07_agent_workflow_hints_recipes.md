# Phase 7: Agent Workflow Hints and Task Recipes

## Objective

Expose machine-readable workflow hints so codegg and other agent harnesses can choose eggsearch tool sequences with less prompt-level reasoning. The output should teach agents when to use `provider_status`, `web_search`, `repo_search`, `repo_map`, `repo_fetch`, `web_fetch`, `batch_fetch`, `security_search`, `research_search`, and `build_evidence_bundle`, while preserving eggsearch's bounded retrieval model.

The deliverable is not a planner inside eggsearch. The deliverable is a compact, typed set of capabilities, recipes, fallback rules, and next-action suggestions that agents can inspect and apply.

## Current context

Phases 1–6 provide:

- truthful provider/capability status;
- stable tool docs;
- structured warnings;
- bounded dispatch;
- stable evidence identity;
- richer code evidence and suggested fetches.

Agents still need to decide what to do with these primitives. Hardcoding sequences in prompts is brittle. This phase gives agents a machine-readable retrieval playbook.

## Non-goals

- Do not introduce autonomous multi-step execution inside eggsearch.
- Do not let eggsearch fetch URLs that were not explicitly selected by the caller.
- Do not create a new MCP tool for every workflow.
- Do not make provider-specific recipes unless generic recipes cannot express the flow.
- Do not summarize search/fetch results.

## Workstream 1: Define the recipe model

### Proposed type

Create a small recipe model, likely in `src/core/workflow.rs` or near provider diagnostics:

```rust
pub struct AgentWorkflowRecipe {
    pub id: String,
    pub title: String,
    pub goal: String,
    pub suitable_when: Vec<String>,
    pub avoid_when: Vec<String>,
    pub required_capabilities: Vec<String>,
    pub optional_capabilities: Vec<String>,
    pub steps: Vec<AgentWorkflowStep>,
    pub fallbacks: Vec<AgentWorkflowFallback>,
    pub expected_outputs: Vec<String>,
    pub trust_notes: Vec<String>,
}

pub struct AgentWorkflowStep {
    pub order: u8,
    pub tool: String,
    pub purpose: String,
    pub input_hints: Vec<String>,
    pub inspect_fields: Vec<String>,
    pub next_action_rule: Option<String>,
}
```

The exact model can differ, but it must be serializable, schema-friendly, and compact.

### Location

Expose recipes through one of these approaches:

1. Add a `workflow_recipes` field to `provider_status` response.
2. Add an optional `include_recipes` argument to `provider_status` if output size is a concern.
3. Add a small `tool_capabilities` block to provider status and keep full recipe docs in `docs/agent-workflows.md`.

Prefer option 1 or 2 if the response remains compact.

## Workstream 2: Required recipes

Implement recipes for the following workflows.

### Generic web lookup

Purpose: discover and fetch evidence for ordinary web questions.

Sequence:

1. `provider_status`
2. `web_search`
3. inspect source cards, quality, warnings, suggested fetches
4. `web_fetch` selected URLs
5. optional `build_evidence_bundle`

Fallback: if live search disabled, return clear unavailable state to host; do not synthesize.

### Documentation/API lookup

Purpose: find authoritative docs and examples.

Sequence:

1. `provider_status`
2. `web_search(intent = docs)` or `repo_search` when package/repo is known
3. prefer official docs, registry docs, README, examples
4. fetch selected docs/examples
5. bundle evidence

Fallback: if no docs provider support, use generic web search with docs-oriented query terms and warning.

### Repository investigation

Purpose: understand a repo/API/project.

Sequence:

1. `provider_status`
2. `repo_map` if repo locator known
3. `repo_search(profile = coding)` with symbol/path/language/package hints
4. `repo_fetch` selected code spans
5. `batch_fetch` for explicit selected suggestions
6. `build_evidence_bundle`

Fallback: if native code providers unavailable, use generic search with repo qualifiers and route warnings.

### Exact error investigation

Purpose: debug compiler/runtime errors.

Sequence:

1. `repo_search(mode = exact_error, profile = coding)`
2. inspect error context, redactions, matched error codes, result groups
3. fetch official docs/issues/release notes/source files
4. bundle evidence

Fallback: if no repo/provider context, use `web_search` with exact phrase plus toolchain terms.

### Security package/version triage

Purpose: determine whether a package/version may be affected.

Sequence:

1. `security_search` with ecosystem/package/version/identifier
2. include applicability when package/version or dependency file text exists
3. inspect applicability, advisory ranges, KEV/exploit context, warnings
4. fetch primary advisory/vendor/OSV/GHSA/RustSec sources
5. bundle evidence

Fallback: if OSV/native providers unavailable, use generic security search with explicit unsupported-capability warning.

### Dependency upgrade/migration research

Purpose: understand safe upgrade path.

Sequence:

1. `repo_search` or `research_search` with package/version constraints
2. fetch changelog, migration guide, release notes, README examples, relevant source/tests
3. use `security_search` if upgrade is security motivated
4. bundle evidence

Fallback: generic docs/release search.

### Architecture/deep research

Purpose: compare libraries/patterns/architectures.

Sequence:

1. `research_search` with workflow/depth/domain/source-type hints
2. inspect grouped claims/evidence/source quality/conflicts
3. fetch primary and conflicting sources
4. bundle evidence

Fallback: use `web_search` with explicit source-type filters and warnings.

### Local workspace investigation

Purpose: investigate current local code when available.

Sequence:

1. `provider_status` to see local provider state
2. `repo_search(include_local = true)` or local-aware repo search
3. prefer clean local matches when task is about current checkout
4. `repo_fetch(prefer_local = true)` exact files/spans
5. bundle evidence with local trust/dirty state

Fallback: remote repo search when local unavailable.

## Workstream 3: Capability-to-recipe gating

Recipes should identify required and optional capabilities. Provider status should allow an agent to know whether a recipe is fully supported, partially supported, or unsupported.

Suggested support enum:

- `available`
- `partial`
- `unavailable`

Examples:

- Repository investigation is `available` when native code/repo providers or local workspace are configured.
- Security triage is `available` when OSV or advisory providers are configured; `partial` with generic web only.
- Local workspace investigation is `available` only when local workspace is enabled/configured.

## Workstream 4: Next-action hints in responses

Add lightweight next-action hints to relevant responses. Do not make them verbose.

Examples:

- `repo_search` response: top suggested next actions such as `repo_fetch` top source span, `batch_fetch` selected tests/examples, or `build_evidence_bundle`.
- `security_search` response: fetch primary advisory, inspect applicability, fetch vendor guidance.
- `research_search` response: fetch primary sources, fetch counterpoints, bundle evidence.
- `web_search` response: fetch selected URLs before relying on snippets.

Suggested type:

```rust
pub struct AgentNextAction {
    pub tool: String,
    pub reason_code: String,
    pub priority: u8,
    pub input_template: serde_json::Value,
    pub source_ids: Vec<String>,
}
```

Keep it optional and bounded.

## Workstream 5: Docs and examples

Update:

- `docs/agent-workflows.md`
- `docs/tool-matrix.md`
- `AGENTS.md`
- README examples if needed

Docs should clearly state that recipes are guidance only. Hosts remain in control of tool sequencing.

## Tests

Add tests for:

- Provider status includes recipe metadata or capability-to-recipe map.
- Recipes serialize with stable IDs and expected tools.
- Unsupported providers mark recipes partial/unavailable appropriately.
- Each recipe references only real registered tool names.
- Next-action hints use valid input templates for their target tools.
- Response next-action count is bounded.
- No recipe instructs autonomous crawling or automatic link following.

## Acceptance criteria

- Agents can inspect a machine-readable recipe/capability surface.
- Recipes cover generic lookup, docs/API lookup, repo investigation, exact error, security triage, upgrade/migration research, architecture research, and local workspace investigation.
- Recipes are gated by actual provider capabilities and local configuration.
- Response-level next-action hints are compact, explicit, and bounded.
- All recipe examples deserialize against real tool args or are clearly non-runnable.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` pass.
