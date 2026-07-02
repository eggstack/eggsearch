# Phase 2: MCP Tool Contract and Documentation Consistency

## Objective

Align the public documentation, crate documentation, MCP tool descriptions, README examples, and CLI help with the actual stable eggsearch tool surface. Agents and harness authors should see one coherent contract: ten stable MCP tools with clear use cases, inputs, outputs, trust semantics, and recommended follow-up actions.

This is an agent-facing phase. It reduces planner mistakes caused by stale docs and makes codegg integration more deterministic.

## Current problem statement

The repository now exposes a rich tool surface, including web search/fetch, batch fetch, provider status, repo search/fetch/map, security search, research search, and evidence bundle construction. Some older module-level docs still describe only the original smaller tool surface. This mismatch can cause agents to call generic `web_search` for tasks that should use `repo_search`, `security_search`, `research_search`, or `repo_fetch`.

The README already explains many workflows in prose. This phase turns that into a precise, repeated, schema-aligned contract.

## Scope

In scope:

- Update crate/module docs to describe all stable tools.
- Add or update a docs page that acts as an agent workflow guide.
- Ensure README tool list, stable baseline, examples, and tool descriptions match implementation.
- Ensure MCP tool schema descriptions are clear and oriented toward agent use.
- Add tests or lightweight checks ensuring registered tool names match docs/tool matrix.
- Add example calls for the most important codegg workflows.

Out of scope:

- Changing tool behavior except where a docs mismatch reveals an obvious validation bug.
- Adding new MCP tools.
- Large README rewrite unrelated to tool contract clarity.

## Stable tool contract to document

Document the following tools as stable:

- `web_search`: generic web metasearch discovery over configured providers.
- `web_fetch`: explicit bounded fetch for one HTTP(S) URL.
- `batch_fetch`: bounded batch fetch over explicit URLs or structured locators.
- `provider_status`: non-probing provider/config/capability diagnostic surface.
- `repo_search`: structured repository evidence discovery.
- `repo_fetch`: precise repository file/span/symbol fetch.
- `repo_map`: bounded repository structure discovery.
- `security_search`: vulnerability/advisory/package security search.
- `research_search`: multi-source research evidence discovery.
- `build_evidence_bundle`: deterministic evidence packaging for handoff.

Each tool entry should include:

- Purpose.
- When to use.
- When not to use.
- Minimal input.
- Important optional inputs.
- Output shape summary.
- Trust semantics.
- Recommended next tool.
- Common fallback path.

## Agent workflow documentation

Add `docs/agent-workflows.md` or `plans/agent_workflows.md` depending on repo convention. Prefer `docs/` if it already exists or if the repo publishes docs; otherwise use `plans/` for handoff.

Required workflows:

### Generic lookup

Recommended sequence:

1. `web_search` with `intent = web`.
2. Inspect source cards and warnings.
3. `web_fetch` selected explicit URLs.
4. Optionally `build_evidence_bundle`.

### Documentation/API lookup

Recommended sequence:

1. `web_search` with `intent = docs` or `repo_search` with package/repo hints.
2. Prefer official docs, registry docs, README, examples.
3. Fetch selected docs or source examples.
4. Bundle evidence.

### Repository investigation

Recommended sequence:

1. `repo_map` when a repo locator is known.
2. `repo_search` with `profile = coding` and repo/package/symbol/path hints.
3. `repo_fetch` precise files/spans.
4. `batch_fetch` when multiple suggested fetches are selected.
5. `build_evidence_bundle` for handoff.

### Exact error investigation

Recommended sequence:

1. `repo_search` with `mode = exact_error` and the compiler/runtime error text.
2. Use parsed error codes, redacted query text, and suggested docs/issues/changelog fetches.
3. Fetch top evidence.
4. Bundle results with gap analysis.

### Security triage

Recommended sequence:

1. `security_search` with identifier or package/ecosystem/version.
2. Include applicability assessment when package/version or dependency files are available.
3. Fetch vendor/OSV/GHSA/RustSec references when needed.
4. Bundle advisory evidence.

### Research / architecture decision

Recommended sequence:

1. `research_search` with workflow/depth/domain/source type hints.
2. Prefer primary sources.
3. Fetch conflicting or high-authority sources.
4. Bundle evidence with known gaps.

## Implementation steps

1. Update `src/lib.rs` module docs so the MCP module description lists all stable tools or points to a canonical tool list.
2. Update `src/mcp/mod.rs` docs so the public API description no longer mentions only the old three-tool surface.
3. Review `src/mcp/tools.rs` argument descriptions. Improve fields whose descriptions are too terse for agents, especially `repo_search.mode`, `profile`, `include_local`, `security_search.assess_applicability`, `dependency_files`, `research_search.workflow`, and `research_search.depth`.
4. Update README stable baseline and examples only where needed. Avoid duplicating an enormous schema dump.
5. Add a compact tool matrix document. Include the tool selection guidance above.
6. Add a test or script that checks the documented stable tool names against registered MCP tool names. This can be a unit test with a constant list, a snapshot, or a small docs consistency test.
7. Add examples in JSON for the five most important codegg calls: repo map for `owner/repo`, repo search for symbol/API usage, exact error search, security package/version triage, and research architecture decision.

## Acceptance criteria

- No crate/module docs claim the MCP surface is only `web_search`, `web_fetch`, and `provider_status`.
- README and docs agree on stable tool names.
- Tool descriptions tell an agent when to use specialized tools instead of generic `web_search`.
- A docs/tool-name consistency check exists.
- Example JSON calls are valid according to current argument structs.
- Trust boundaries are documented for snippets, fetched content, local workspace results, and evidence bundles.

## Risks and mitigations

Risk: Documentation becomes duplicated and drifts again.

Mitigation: Keep one canonical tool matrix and have other docs link or summarize it. Add a consistency test for tool names.

Risk: The docs imply stronger provider enforcement than the implementation can guarantee.

Mitigation: Repeat the rule that intent/freshness/profile fields are retrieval hints unless a selected provider reports native support.

Risk: Agent workflow docs over-prescribe behavior.

Mitigation: Present workflows as recommended sequences and include fallback paths for unavailable providers.

## Handoff notes

This phase should be mostly docs and small schema-description edits. Do not refactor tool internals unless a documented contract is already implemented incorrectly. If implementation changes are needed, isolate them in a follow-up corrective plan.
