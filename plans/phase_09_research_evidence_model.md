# Phase 9: Research Evidence Model

## Objective

Make `research_search` output more useful for deep technical research agents by structuring results around claims, evidence, conflicts, gaps, source quality, and explicit next fetches. The tool should remain a bounded retrieval and evidence-discovery primitive; it should not become an autonomous summarizer or long-running browser.

This phase is aimed at codegg's deep research agent use case: difficult architecture, dependency, performance, security-design, and ecosystem questions where a flat list of links is insufficient.

## Current context

Eggsearch already supports `research_search`, source grouping, suggested fetches, workflow/depth hints, primary source preference, counterpoints, and evidence bundles. Phases 1–8 provide reliable provider status, stable warnings, deterministic IDs, richer code evidence, workflow recipes, and stronger security verdicts.

The remaining problem is research result shape. Agents need to reason over what evidence supports, what conflicts, what is missing, and what should be fetched next.

## Non-goals

- Do not generate final narrative research reports inside eggsearch.
- Do not summarize full articles beyond short source-card snippets and structured metadata.
- Do not automatically fetch all recommended sources.
- Do not crawl links from fetched pages.
- Do not present weak evidence as settled conclusions.

## Workstream 1: Research claim model

### Required concept

Introduce a compact claim/evidence model. Claims should be retrieval-derived and conservative. They are not final conclusions; they are structured hypotheses or evidence group labels that help agents decide what to inspect.

### Proposed shape

```rust
pub struct ResearchClaim {
    pub id: String,
    pub text: String,
    pub claim_type: ResearchClaimType,
    pub confidence: EvidenceConfidence,
    pub supporting_source_ids: Vec<String>,
    pub conflicting_source_ids: Vec<String>,
    pub missing_evidence: Vec<String>,
    pub source_quality_notes: Vec<String>,
}
```

Claim types:

- `performance`
- `security`
- `maintenance`
- `compatibility`
- `architecture`
- `api_design`
- `operational`
- `ecosystem`
- `cost`
- `unknown`

### Constraints

- Claim text must be brief and evidence-linked.
- If only one weak source supports a claim, confidence should be low.
- Conflicting evidence should be explicitly linked.

## Workstream 2: Source quality model

### Required source classes

Classify sources into:

- `official_docs`
- `reference_docs`
- `repository_source`
- `maintainer_issue`
- `release_notes`
- `benchmark`
- `paper`
- `standard_spec`
- `security_advisory`
- `vendor_blog`
- `engineering_blog`
- `forum_thread`
- `news_article`
- `unknown`

### Quality signals

Add or normalize source quality signals:

- primary source
- maintained/current
- version-specific
- commit-pinned
- reproducible benchmark
- peer-reviewed
- standard/spec source
- maintainer-authored
- stale source
- secondary source
- anecdotal source
- marketing source
- conflict source

These should be metadata, not prose-heavy summaries.

## Workstream 3: Conflict and counterpoint representation

### Required behavior

When `include_counterpoints` is enabled or when sources disagree:

- Group conflicting sources under a conflict object.
- Link the conflict to relevant claims.
- Preserve source IDs and quality reasons.
- Avoid choosing a winner inside eggsearch unless source-quality rules are mechanical and transparent.

### Proposed shape

```rust
pub struct ResearchConflict {
    pub id: String,
    pub topic: String,
    pub claim_ids: Vec<String>,
    pub side_a_source_ids: Vec<String>,
    pub side_b_source_ids: Vec<String>,
    pub notes: Vec<String>,
}
```

### Tests

- Library comparison query with two ecosystems returns conflict/counterpoint structure.
- Source quality distinguishes official docs from forum posts.
- Conflicts do not remove or hide original source cards.

## Workstream 4: Evidence gap analysis

### Required gaps

Research responses should identify gaps such as:

- no primary source found
- no recent source found
- no benchmark source found
- no security source found
- no migration/changelog source found
- only secondary sources found
- conflicting evidence not resolved
- source needs fetch before reliance
- version/context missing

Each gap should include recommended next fetches or search refinement when possible.

### Proposed shape

```rust
pub struct ResearchEvidenceGap {
    pub kind: ResearchEvidenceGapKind,
    pub message: String,
    pub affected_claim_ids: Vec<String>,
    pub affected_source_ids: Vec<String>,
    pub recommended_actions: Vec<AgentNextAction>,
}
```

## Workstream 5: Research suggested fetch ranking

### Required behavior

Suggested fetches should be ranked by information gain, not just source rank.

High priority:

- official docs/specs
- repository source/examples
- release notes/changelog/migration guides
- benchmark methodology/result pages
- security advisories
- direct counterpoint sources

Lower priority:

- duplicate secondary sources
- broad tutorials
- shallow news/articles
- forum posts unless uniquely relevant

### Required fields

- stable ID
- source ID
- reason code
- information gain score or ordinal priority
- source quality class
- expected evidence type
- explicit URL or repo locator

## Workstream 6: Workflow-specific output shaping

### Required workflows

Support at least:

- `architecture_decision`
- `library_comparison`
- `performance_investigation`
- `security_review`
- `migration_planning`
- `ecosystem_survey`
- `api_design_review`
- `operational_runbook_research`

Each workflow should influence:

- subquery generation;
- desired source types;
- grouping labels;
- gap kinds;
- suggested fetch ranking;
- next-action hints.

## Workstream 7: Evidence bundle integration

### Required behavior

`build_evidence_bundle` should preserve research claims/conflicts/gaps when the input includes them, or at least preserve enough metadata for the receiving agent to reconstruct the research state.

If evidence bundle input types do not currently accept research metadata, add optional fields or a compact `research_context` section.

### Tests

- Research response with claims can be bundled without losing source/fetch links.
- Bundle gap analysis preserves unfetched primary-source gaps.
- Bundle remains compatible when research metadata is absent.

## Tests

Add tests for:

- Claim IDs are stable and linked to source IDs.
- Source quality classification for official docs, repo source, benchmark, paper/spec, forum, and blog.
- Conflicts/counterpoints are represented when requested.
- Evidence gaps appear for missing primary sources and missing recent evidence.
- Suggested fetch ranking prioritizes primary/counterpoint sources.
- Workflow-specific output differs appropriately between library comparison and performance investigation.
- Research metadata is preserved in evidence bundles.
- All new structured fields serialize with stable snake_case enum names.

## Acceptance criteria

- `research_search` returns structured claims, conflicts, evidence gaps, source quality, and ranked suggested fetches.
- The retrieval layer remains bounded and non-summarizing.
- Agents can use the response to decide what to fetch next and what uncertainty remains.
- Evidence bundles can carry research context forward.
- Workflow-specific behavior is tested with offline/mocked fixtures.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` pass.
