# Phase 5: Agent Workflow Optimization

Status: ready after repository intelligence phases
Depends on: Phase 1; Phase 2; Phase 3; Phase 4
Primary goal: make eggsearch responses maximally useful to coding, security, and architectural research agents without introducing model-dependent ranking or summarization.

## 1. Problem Statement

Eggsearch already has unusually strong machine-readable contracts: deterministic IDs, structured warnings, quality metadata, suggested fetches, next actions, and evidence bundles. The next improvement is to make these outputs more task-aware and evidence-complete.

A coding agent should not receive only a list of relevant URLs. It should receive enough structured information to answer:

- What evidence has been found?
- Which source is authoritative for each claim?
- Which repository locations are likely implementation, tests, documentation, or configuration?
- What remains unknown?
- Did any provider or subquery fail?
- What is the most productive safe next tool call?
- Is the current evidence sufficient for planning, editing, review, or security assessment?

This phase improves deterministic workflow orchestration while keeping final interpretation with the host agent.

## 2. Required Outcomes

- Search groups align with coding-agent evidence roles.
- Suggested fetches prioritize stable, authoritative, task-relevant evidence.
- Next actions are concrete and executable where response data already contains values.
- Responses distinguish evidence absence from retrieval failure.
- Contradictions, coverage gaps, and partial workflow completion are represented explicitly.
- Codegg contract fixtures demonstrate correct consumption and fallback behavior.
- No embedded model judgment is required.

## 3. Core Workflow Set

Optimize the following workflows first:

1. API and library comprehension.
2. Repository architecture review.
3. Exact error and debugging investigation.
4. Version migration and changelog analysis.
5. Security vulnerability and applicability review.
6. Dependency and package evaluation.
7. Performance investigation.
8. Comparative technical research.
9. Implementation evidence gathering before code changes.
10. Review evidence gathering after code changes.

Each workflow should have explicit desired evidence categories, completion signals, gap signals, and next-action rules.

## 4. Workstream A: Evidence Role Taxonomy

Unify or map existing source kinds, source roles, document kinds, and research source types into a stable workflow-facing taxonomy.

Recommended evidence roles:

- primary implementation;
- interface or API definition;
- usage example;
- test or behavioral specification;
- configuration or feature gate;
- manifest or dependency metadata;
- official documentation;
- architecture or design document;
- release note or changelog;
- migration guidance;
- benchmark or performance evidence;
- issue or incident discussion;
- pull request or design review;
- authoritative security advisory;
- vendor security guidance;
- independent corroboration;
- counterpoint or conflicting evidence;
- community discussion;
- unknown or weak-context evidence.

### Tasks

1. Define stable serialized identifiers.
2. Map current source metadata deterministically.
3. Preserve more specific existing fields rather than replacing them.
4. Add role confidence and reasons where classification is heuristic.
5. Ensure repository map, search, fetch, security, and research tools use compatible role semantics.

## 5. Workstream B: Workflow Coverage Model

Define deterministic coverage structures for each workflow.

Example for API comprehension:

```text
required:
  - interface_definition
  - primary_implementation
recommended:
  - official_documentation
  - usage_example
  - behavioral_test
optional:
  - migration_guidance
  - issue_discussion
```

Required fields:

- workflow ID;
- expected evidence roles;
- found roles;
- missing roles;
- failed retrieval dimensions;
- coverage status;
- completion confidence;
- reasons;
- recommended next actions.

Coverage status should distinguish:

- sufficient;
- usable_with_gaps;
- insufficient;
- indeterminate_due_to_failures.

Do not infer that evidence does not exist merely because it was not retrieved.

## 6. Workstream C: Grouping Improvements

### Repository search

Group results by evidence role rather than only broad source type when enough metadata exists:

- implementation;
- interface/API;
- tests;
- docs;
- examples;
- configuration;
- manifests;
- issues/PRs;
- releases/migrations;
- security;
- benchmarks.

### Security search

Separate:

- authoritative advisory;
- affected-version metadata;
- known exploitation evidence;
- vendor remediation;
- patch or fixing commit;
- dependency applicability evidence;
- defensive guidance;
- secondary reporting.

### Research search

Separate:

- primary source;
- specification;
- reference implementation;
- benchmark;
- design discussion;
- counterpoint;
- recent development;
- community experience.

### Acceptance

Group limits and global result limits remain deterministic, and grouping does not duplicate source cards unless the contract explicitly supports cross-role references.

## 7. Workstream D: Suggested Fetch Ranking

Refine the deterministic ranking pipeline using explicit factors.

Recommended order of influence:

1. Commit-pinned provenance.
2. Exact repository and path match.
3. Evidence role required by the active workflow.
4. Primary or official authority.
5. Exact symbol or identifier match.
6. Structured metadata over snippet-only evidence.
7. Source diversity.
8. Freshness when relevant to the workflow.
9. Mutable versus immutable URL.
10. Provider health and retrieval likelihood.

### Constraints

- Do not hide low-ranked candidates when they are the only evidence for a missing role.
- Do not allow many similar sources from one domain to crowd out role coverage.
- Preserve deterministic tie-breaking.
- Expose rank reasons and score components sufficiently for debugging.
- Penalize mutable branch URLs when commit-pinned alternatives exist.

## 8. Workstream E: Concrete Next Actions

Replace generic or placeholder-heavy actions with concrete tool calls whenever response data permits.

Examples:

- `repo_fetch` with resolved host, owner, repo, commit SHA, path, and line span.
- `repo_search` constrained to a concrete test directory and symbol.
- `web_fetch` for an authoritative advisory URL.
- `security_search` with resolved ecosystem, package, version, and CVE.
- `research_search` requesting the specific missing evidence role.
- `build_evidence_bundle` with concrete source IDs and fetched-item IDs.

### Rules

- Actions must declare why they are productive.
- Actions must identify the gap or evidence item they address.
- Unsafe or unavailable actions must not be emitted.
- Capability and policy checks should occur before action generation.
- Maximum action count remains bounded.
- Action ordering remains deterministic.

## 9. Workstream F: Contradiction and Conflict Metadata

Improve deterministic conflict detection without claiming semantic certainty.

### Detectable conflict classes

- differing version ranges for the same advisory;
- conflicting release dates or version identifiers;
- mutually exclusive structured status fields;
- divergent benchmark numbers with comparable labels;
- documentation versus implementation version mismatch;
- mutable branch content versus commit-pinned content;
- different provider metadata for the same canonical entity.

### Output

- conflict ID;
- involved source IDs;
- conflict class;
- compared fields;
- values;
- whether sources are directly comparable;
- recommended resolution action.

Do not label ordinary differences in wording as contradictions.

## 10. Workstream G: Failure and Absence Semantics

Introduce or refine response-level distinctions:

- no matching evidence found;
- provider capability unavailable;
- provider skipped by policy;
- provider failed;
- deadline prevented completion;
- result truncated by cap;
- evidence role not requested;
- evidence role requested but not found;
- evidence role indeterminate because retrieval failed.

This distinction should feed coverage status and next actions.

A host agent must never interpret an empty group as proof of absence when the corresponding retrieval dimension failed.

## 11. Workstream H: Security Workflow Refinement

### Required improvements

- Prefer native advisory providers over generic web results.
- Link vulnerability records to exact package ecosystem and version evidence.
- Distinguish affected, unaffected, fixed, unknown, and indeterminate applicability.
- Surface KEV evidence as exploitation context, not as generic severity.
- Identify fixing commits, patched versions, and vendor guidance where available.
- Keep defensive guidance separate from exploit context.
- Scope warnings to the affected package, advisory, or source IDs.
- Emit a next action to inspect the exact dependency declaration or lockfile entry when local workspace data is available.

### Acceptance

A security response explains whether applicability was evaluated, what evidence supported it, and what missing evidence prevents a conclusion.

## 12. Workstream I: Codegg Contract Fixtures

Create end-to-end offline fixtures demonstrating how codegg should consume each optimized workflow.

Required scenarios:

- Understand a Rust API before modification.
- Locate implementation and tests for a symbol.
- Investigate an exact compiler error.
- Compare two library versions for migration.
- Assess a dependency vulnerability against a local lockfile.
- Review repository architecture from remote map plus selected fetches.
- Gather performance evidence with conflicting benchmarks.
- Build and verify an evidence bundle for subagent handoff.
- Handle degraded provider capability.
- Handle partial deadline completion.

Each fixture should assert:

- deterministic IDs;
- group roles;
- coverage state;
- warnings;
- suggested fetches;
- next actions;
- evidence-bundle linkage.

## 13. Workstream J: Documentation and Recipes

Update:

- agent workflows;
- tool matrix;
- codegg response contract;
- provider diagnostics;
- example MCP transcripts;
- workflow recipe catalog.

Recipes should state:

- objective;
- required evidence roles;
- tool sequence;
- capability prerequisites;
- degradation path;
- stopping criteria;
- trust notes;
- bundle handoff criteria.

Documentation examples should be contract-tested where practical.

## 14. Testing Strategy

### Unit tests

- Role classification.
- Coverage computation.
- Fetch ranking components.
- Next-action generation.
- Conflict classification.
- absence-versus-failure state transitions.

### Integration tests

- Complete workflow fixtures.
- Mixed local and remote evidence.
- Missing providers.
- Partial failures.
- deterministic ordering under randomized input.
- cap and truncation behavior.

### Contract tests

- Stable serialized role identifiers.
- Additive schema compatibility.
- action input templates match target tool schemas.
- bundle IDs link to originating evidence.
- structured warnings use documented codes and scopes.

## 15. Definition of Done

- The core workflow set has explicit deterministic coverage models.
- Search groups align with evidence roles.
- Suggested fetch ranking prioritizes authoritative, stable, workflow-required evidence.
- Next actions are concrete when values are known.
- Empty evidence and failed retrieval are never conflated.
- Conflict metadata is precise and conservative.
- Security applicability communicates evidence and uncertainty correctly.
- Codegg end-to-end fixtures cover success, degradation, and partial completion.
- Public schemas remain compatible or have an explicit migration.
- Full release gate passes.

## 16. Handoff Notes

Implement the shared evidence-role and coverage model before changing per-tool ranking. Avoid hand-tuning individual workflows through scattered conditionals. Centralize workflow definitions and deterministic scoring inputs so codegg behavior remains auditable and regression-testable.