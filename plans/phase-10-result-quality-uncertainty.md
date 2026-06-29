# Phase 10: Result Quality and Uncertainty Metadata

## Purpose

Add explicit quality, confidence, and uncertainty metadata to search and fetch results so coding agents can reason about when to trust a result, when to fetch more evidence, and when to ask for clarification.

Eggsearch already returns structured metadata, rank reasons, trust labels, and source cards. This phase adds a coherent quality layer over those signals. The goal is not to create a magic truth score; it is to expose deterministic, inspectable indicators that help codegg choose better next actions.

## Non-goals

Do not add model-based result judging in this phase. Do not hide results because of low confidence unless they violate existing policy. Do not claim factual correctness. Do not overfit to one provider's ranking score.

## Concepts

Add a result quality block to `SourceCard` or a nested metadata field:

```rust
pub struct ResultQuality {
    pub confidence: ResultConfidence,
    pub relevance: RelevanceEstimate,
    pub authority: AuthorityEstimate,
    pub freshness: FreshnessEstimate,
    pub evidence_strength: EvidenceStrength,
    pub uncertainty_reasons: Vec<UncertaintyReason>,
    pub quality_reasons: Vec<QualityReason>,
}
```

Enums:

```rust
pub enum ResultConfidence {
    High,
    Medium,
    Low,
    Unknown,
}

pub enum RelevanceEstimate {
    Exact,
    Strong,
    Partial,
    Weak,
    Unknown,
}

pub enum AuthorityEstimate {
    Primary,
    Official,
    Maintainer,
    PackageRegistry,
    Community,
    NewsOrBlog,
    Unknown,
}

pub enum FreshnessEstimate {
    Current,
    Recent,
    Historical,
    Undated,
    Stale,
    Unknown,
}

pub enum EvidenceStrength {
    ExactCodeSpan,
    ExactIdentifier,
    StructuredMetadata,
    SnippetOnly,
    UrlOnly,
    Unknown,
}
```

Uncertainty reasons should be deterministic:

- `NoSnippet`
- `NoTimestamp`
- `GenericProviderOnly`
- `ProviderFailed`
- `FuzzyQueryMatch`
- `NoExactPhraseMatch`
- `AmbiguousRepository`
- `UnverifiedVersionMatch`
- `LowAuthoritySource`
- `ConflictingSources`
- `ResultTruncated`
- `FetchSuggested`

Quality reasons:

- `ExactRepoMatch`
- `ExactPathMatch`
- `ExactSymbolMatch`
- `ExactErrorPhraseMatch`
- `OfficialDocs`
- `MaintainerSource`
- `PrimaryAdvisory`
- `PackageRegistryMetadata`
- `FreshTimestamp`
- `CommitPinnedEvidence`
- `StructuredCodeEvidence`

## Where to attach quality

Attach `quality` to each `SourceCard`. Also add aggregate quality to grouped responses:

```rust
pub struct RepoResultGroup {
    pub kind: RepoResultGroupKind,
    pub label: String,
    pub results: Vec<SourceCard>,
    pub truncated: bool,
    pub quality_summary: Option<GroupQualitySummary>,
}
```

Group summary:

```rust
pub struct GroupQualitySummary {
    pub high_confidence_count: usize,
    pub low_confidence_count: usize,
    pub primary_source_count: usize,
    pub exact_evidence_count: usize,
    pub warnings: Vec<String>,
}
```

If modifying `RepoResultGroup` is too invasive, start with per-card quality and aggregate response warnings.

## Deterministic quality rules

### Code results

High confidence when:

- Host/owner/repo/path are parsed.
- Source role is known.
- Match line or symbol is known.
- Raw URL or raw permalink is present.

Medium confidence when:

- URL points to recognizable code host but line/symbol is missing.
- Result comes from generic web provider but code evidence was inferred from URL.

Low confidence when:

- Only title/snippet imply code relevance.
- Repo is ambiguous.
- No fetchable URL exists.

### Docs/results

Authority:

- Official docs domain or configured official docs mapping -> official.
- Project repo README/docs -> maintainer.
- Package registry page -> package registry.
- Forum/blog/news -> lower authority.

Relevance:

- Exact phrase in title/snippet -> exact/strong.
- All query tokens present -> strong.
- Some tokens present -> partial.
- Only provider rank suggests relevance -> weak/unknown.

Freshness:

- Timestamp within requested freshness window -> current/recent.
- Timestamp older than configured stale window -> stale/historical.
- No timestamp -> undated.

### Security results

Use phase 7 source tiers:

- Primary advisory -> high authority.
- Vendor/package registry advisory -> high/medium.
- Blog/forum -> lower authority unless query asks for field reports.

Add uncertainty if version matching was impossible.

### Error search results

Use phase 8 exact-error context:

- Exact phrase match -> high relevance.
- Error code only -> medium.
- Tool/language only -> low/partial.

## Provider failure and aggregate uncertainty

When providers fail, preserve result-level quality for returned results but add response-level uncertainty:

```rust
pub struct SearchUncertaintySummary {
    pub provider_failures: usize,
    pub degraded_provider_selection: bool,
    pub partial_provider_selection: bool,
    pub low_confidence_results: usize,
    pub warnings: Vec<String>,
}
```

Add to `RepoSearchTelemetry` or response metadata.

Warnings should be stable and machine-friendly, e.g.:

- `quality_warning: only generic providers returned results`
- `quality_warning: no exact phrase matches found`
- `quality_warning: primary advisory source unavailable`
- `quality_warning: result set truncated before all groups were filled`

## API compatibility

Adding `quality` fields should be backward-compatible. Existing clients can ignore them.

Avoid renaming existing fields. Avoid changing sort order solely based on quality in the first implementation unless the behavior is obviously beneficial and tested. Prefer adding quality metadata first, then small deterministic boosts in ranking where safe.

## Ranking integration

After quality fields exist, add modest ranking boosts:

- Exact code span/symbol match.
- Official docs for docs group.
- Primary advisory for security group.
- Exact error phrase for exact-error mode.
- Commit-pinned raw permalink for code fetch suggestions.

Do not let authority overwhelm exact relevance. A generic official homepage should not outrank a precise maintainer issue or exact source match.

## Provider status

Expose whether quality metadata is enabled:

```json
"quality_metadata": {
  "enabled": true,
  "per_result": true,
  "group_summary": true,
  "uses_model_judging": false
}
```

## Tests

Add tests for:

- Code result with host/path/raw permalink gets high confidence and exact/strong evidence.
- Generic web result with no structured metadata gets lower confidence.
- Official docs domain gets official authority.
- Package registry source gets package-registry authority.
- Security advisory gets primary/package advisory authority.
- Missing timestamp yields `Undated` and `NoTimestamp` uncertainty.
- Provider failure increments aggregate uncertainty but does not erase results.
- Partial provider selection is reflected in uncertainty summary.
- Exact phrase match raises relevance estimate.
- Quality metadata serializes in `SourceCard` without breaking existing fields.

Use deterministic fixtures and mocked providers.

## Documentation

Update README and AGENTS.md:

- Explain that quality fields are deterministic heuristics, not truth judgments.
- Document the main fields and how codegg should use them.
- Recommend that agents fetch high-value, low-certainty results before acting.
- Recommend treating low authority + no exact match as weak evidence.

Example guidance for codegg:

- Prefer `High` confidence code spans with raw permalinks.
- Prefer official/maintainer docs for API semantics.
- Prefer primary advisories for vulnerability facts.
- Fetch more evidence when `uncertainty_reasons` includes `NoSnippet`, `FuzzyQueryMatch`, or `GenericProviderOnly`.

## Acceptance criteria

Phase 10 is complete when:

- `SourceCard` or equivalent metadata includes result quality fields.
- Quality values are deterministic and tested.
- Search responses include aggregate uncertainty or group quality summary.
- Ranking uses quality only in small, documented ways.
- Provider status advertises quality metadata.
- Docs explain quality as heuristic metadata, not factual certainty.
- `cargo fmt`, clippy, and tests pass.
