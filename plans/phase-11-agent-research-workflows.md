# Phase 11: Agent Research Workflow Support

## Purpose

Add workflow-level support for codegg's deeper research use cases without turning eggsearch into an autonomous agent. Eggsearch should help a coding agent scope, diversify, and audit a research query by returning structured research plans, source buckets, coverage gaps, and next-fetch recommendations.

This phase builds on the existing `research_search` tool and the new quality/uncertainty metadata from Phase 10. The intent is to support difficult architectural questions, API adoption research, library comparisons, and design-tradeoff investigations.

## Non-goals

Do not implement autonomous multi-step browsing, long-running background research, recursive crawling, summarization with model calls, or hidden stateful agent memory. Eggsearch remains a deterministic retrieval and structuring service.

## Current baseline

Eggsearch already has:

- `research_search` as a dedicated MCP tool.
- Research domains and desired source types.
- Grouped results and source-card metadata.
- Generic search providers and native code/repo providers.
- Suggested fetches.

This phase should make `research_search` more agent-useful by adding planning metadata and coverage reporting.

## Request additions

Extend `ResearchSearchArgs` / `ResearchSearchRequest` with:

```rust
pub struct ResearchSearchRequest {
    pub query: String,
    pub research_domain: Option<ResearchDomain>,
    pub desired_source_types: Vec<ResearchSourceType>,
    pub include_counterpoints: Option<bool>,
    pub include_primary_sources: Option<bool>,
    pub include_recent_discussion: Option<bool>,
    pub include_security_considerations: Option<bool>,
    pub max_results: Option<usize>,
    pub max_groups: Option<usize>,
    pub max_per_group: Option<usize>,
    pub freshness: Freshness,
    pub timeout_ms: Option<u64>,
    pub providers: Vec<String>,

    // New fields
    pub workflow: Option<ResearchWorkflow>,
    pub depth: Option<ResearchDepth>,
    pub compare_targets: Vec<String>,
    pub constraints: Vec<String>,
    pub known_context: Option<String>,
}
```

Recommended workflow enum:

```rust
pub enum ResearchWorkflow {
    ArchitectureDecision,
    ApiEvaluation,
    LibraryComparison,
    MigrationPlanning,
    SecurityReview,
    PerformanceInvestigation,
    EcosystemSurvey,
    General,
}

pub enum ResearchDepth {
    Quick,
    Standard,
    Deep,
}
```

Depth controls source diversity and subquery breadth, not autonomous looping.

## Response additions

Add a workflow context block:

```rust
pub struct ResearchWorkflowContext {
    pub workflow: ResearchWorkflow,
    pub interpreted_question: String,
    pub dimensions: Vec<ResearchDimension>,
    pub coverage: ResearchCoverage,
    pub gaps: Vec<ResearchGap>,
    pub recommended_next_fetches: Vec<RepoSuggestedFetch>,
    pub warnings: Vec<String>,
}

pub struct ResearchDimension {
    pub name: String,
    pub purpose: String,
    pub source_types: Vec<ResearchSourceType>,
    pub subqueries: Vec<String>,
}

pub struct ResearchCoverage {
    pub primary_sources_found: usize,
    pub official_docs_found: usize,
    pub implementation_sources_found: usize,
    pub benchmark_sources_found: usize,
    pub security_sources_found: usize,
    pub counterpoints_found: usize,
    pub recent_sources_found: usize,
}

pub struct ResearchGap {
    pub kind: ResearchGapKind,
    pub message: String,
    pub suggested_query: Option<String>,
}
```

Gap kinds:

- `NoPrimarySources`
- `NoRecentSources`
- `NoCounterpoints`
- `NoImplementationEvidence`
- `NoBenchmarks`
- `NoSecurityDiscussion`
- `NoMigrationDocs`
- `ProviderCoverageLimited`
- `AmbiguousQuestion`

## Query planning

For each workflow, generate deterministic dimensions.

### ArchitectureDecision

Dimensions:

- Official docs/specs.
- Reference implementations.
- Design discussions/RFCs.
- Benchmarks/performance notes.
- Failure modes/security considerations.
- Migration/adoption stories.
- Counterpoints/tradeoffs.

### ApiEvaluation

Dimensions:

- Official API docs.
- Examples/tutorials.
- Source implementation.
- Issues/known pitfalls.
- Version/release notes.
- Security/compatibility concerns.

### LibraryComparison

Dimensions:

- Official docs for each target.
- Benchmarks.
- Maintenance/release cadence.
- Issue volume / common bugs.
- Security advisories.
- Migration/interoperability.

### MigrationPlanning

Dimensions:

- Migration guides.
- Changelogs/release notes.
- Breaking-change issues.
- Examples before/after.
- Security changes.

Keep max subqueries bounded by depth.

## Source diversity

Do not allow one provider or one source type to dominate. Add deterministic diversity caps:

- Per domain cap.
- Per provider cap.
- Per source type cap.
- Per compare target cap where relevant.

Expose diversity warnings when caps remove otherwise high-ranked results.

## Coverage and gaps

After results are grouped, compute coverage from source-card metadata and quality fields.

Examples:

- If no primary/official docs found, add `NoPrimarySources`.
- If `include_counterpoints = true` and no counterpoint/community discussion found, add `NoCounterpoints`.
- If workflow is performance investigation and no benchmark sources found, add `NoBenchmarks`.
- If security review and no advisory/security docs found, add `NoSecurityDiscussion`.

These gaps should not be fatal. They are guidance for the calling agent.

## Suggested fetches

Generate recommended next fetches from:

- Highest-quality primary/official source.
- Most relevant implementation/source result.
- Best maintainer issue or design discussion.
- Best counterpoint if requested.
- Best security/performance result when applicable.

Keep suggestions bounded and explain reason strings:

- `primary_source`
- `official_api_docs`
- `reference_implementation`
- `design_discussion`
- `counterpoint`
- `benchmark`
- `security_consideration`
- `migration_guide`

## Interaction with batch fetch

If Phase 6 is implemented, document that codegg can feed `recommended_next_fetches` into `batch_fetch`. Do not call batch fetch automatically.

## Telemetry

Add research telemetry:

```rust
pub struct ResearchTelemetry {
    pub workflow: Option<ResearchWorkflow>,
    pub depth: ResearchDepth,
    pub dimensions_generated: usize,
    pub subqueries_generated: usize,
    pub source_diversity_caps_applied: Vec<String>,
    pub coverage_gaps: Vec<ResearchGapKind>,
}
```

## Tests

Add tests for:

- Workflow parsing.
- Architecture decision dimensions.
- API evaluation dimensions.
- Library comparison target handling.
- Depth controls subquery count.
- Coverage detects missing primary sources.
- Coverage detects missing counterpoints.
- Suggested fetches include diverse source types.
- Diversity cap prevents one domain from dominating.
- Response serializes workflow context and telemetry.
- Existing `research_search` calls without workflow remain backward-compatible.

Use mocked providers and deterministic result fixtures.

## Documentation

Update README and AGENTS.md:

- Explain workflow mode as deterministic research scaffolding, not autonomous research.
- Add examples for architecture decision, library comparison, migration planning, and security review.
- Explain coverage gaps and how agents should use them.
- Recommend `batch_fetch` for selected next evidence, if Phase 6 is present.

## Acceptance criteria

Phase 11 is complete when:

- `research_search` accepts workflow/depth/compare-target fields.
- Responses include workflow context, dimensions, coverage, gaps, and next-fetch recommendations.
- Source diversity and coverage gaps are deterministic and tested.
- Existing basic research search remains compatible.
- Docs clearly distinguish workflow scaffolding from autonomous browsing.
- `cargo fmt`, clippy, and tests pass.
