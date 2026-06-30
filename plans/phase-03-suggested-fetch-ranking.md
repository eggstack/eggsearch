# Phase 3 Plan: Suggested-Fetch Ranking Overhaul

## Objective

Replace fixed-order suggested fetch selection with a deterministic ranking pipeline that chooses the highest-value evidence for coding agents. The goal is to reduce wasted fetches and make `repo_search`, `repo_map`, `security_search`, and `research_search` better at telling codegg what to read next.

This phase should not change the basic search/fetch split. Search tools still discover and rank candidate evidence; fetch tools still retrieve explicit URLs or structured repo locators.

## Current behavior

The current repo suggested-fetch generator uses a fixed rule order and selects the first card from each matching group. This is simple and deterministic, but it does not adapt to the user’s task. Docs currently outrank source files even when the query is symbol-oriented; release notes may be underweighted during migration questions; issue threads may be underweighted during exact-error searches.

The system already has several ingredients needed for better ranking:

- `SourceKind` and grouped repo result kinds.
- `CodeEvidence` and evidence confidence.
- Source roles such as implementation, test, example, documentation, changelog, migration, configuration, and unknown.
- Result quality metadata.
- Rank reasons.
- Suggested structured `repo_fetch` locators.
- Exact-error context with parsed error codes and generated subqueries.

This phase should connect those signals into the suggested-fetch layer.

## Scope

In scope:

- Add a scoring model for suggested fetch candidates.
- Return ranking reasons and information-gain hints.
- Prefer structured `repo_fetch` locators when available for source evidence.
- Make ranking mode-aware: normal repo search, exact-error search, repo map, package/migration search, security search, and research search.
- Preserve bounded output caps.
- Preserve deterministic ordering for equal scores.
- Add tests for ranking behavior.

Out of scope:

- ML ranking.
- Fetching content during ranking.
- Persistent user/session personalization.
- Provider scheduler changes.
- New search providers.

## Proposed model

Introduce a general fetch-candidate structure internal to ranking:

```rust
pub struct FetchCandidate {
    pub url: String,
    pub structured_repo_fetch: Option<RepoFetchRequest>,
    pub group: RepoResultGroupKind,
    pub expected_kind: SourceKind,
    pub recommended_extract_mode: Option<ExtractMode>,
    pub source_card_id: Option<String>,
    pub stable: bool,
    pub score: i32,
    pub reasons: Vec<FetchRankReason>,
}
```

Add public fields to `RepoSuggestedFetch` if compatibility permits:

```rust
pub score: Option<i32>,
pub rank_reasons: Vec<String>,
pub information_gain: Option<f32>,
pub stable: Option<bool>,
pub preferred_tool: Option<String>,
```

Use serde defaults and skip-empty serialization so older clients remain unaffected.

## Ranking signals

Suggested scoring should be deterministic and easy to test. Start with integer weights.

### Provenance and stability

- Commit-pinned raw permalink: high boost.
- Commit-pinned browser permalink: strong boost.
- Mutable raw URL: moderate boost.
- Mutable browser source URL: moderate boost.
- Generic web page URL: neutral.
- Ambiguous or sparse code evidence: penalty.

### Evidence confidence

- `EvidenceConfidence::Exact`: large boost.
- `Strong`: boost.
- `Weak`: small boost or neutral.
- `Unknown`: penalty.

### Source role

Normal coding/API investigation:

- Implementation/source: boost.
- Official docs: boost.
- README: boost.
- Examples: boost.
- Tests: moderate boost when query includes behavior/error/regression/test terms.
- Changelog/migration: boost when version or migration context exists.
- Issue thread: boost when query includes bug/error/panic/regression terms.

Exact-error mode:

- Issue threads with exact phrase/error-code match: very high boost.
- Pull requests/commits touching error text: high boost.
- Changelog/release notes: high boost.
- Source files with error symbol/path match: high boost.
- Generic docs: lower than exact issue/source evidence.

Package/migration mode:

- Registry page for exact package/version: high boost.
- Versioned docs: high boost.
- Release notes/changelog/migration guide: very high boost.
- Source files: boost when symbol/path context exists.

Security mode:

- Native advisory / OSV / NVD / RustSec / GHSA: very high boost.
- Vendor advisory: high boost.
- Release/changelog for fixed version: high boost.
- Exploit discussion: useful but below authoritative advisories unless requested.
- Community discussion: lower confidence.

Research mode:

- Specifications and official docs: high boost.
- Reference implementations: high boost for implementation questions.
- Benchmarks: high boost for performance investigations.
- Security considerations: high boost for security workflows.
- Discussions/counterpoints: include but cap by domain/source diversity.

### Query and context matching

Boost when candidate metadata or title/snippet matches:

- exact symbol;
- path hint;
- language hint;
- file hint;
- error code;
- exact quoted phrase;
- package name;
- version;
- compare version;
- requested source type.

Do not inspect fetched page bodies during ranking.

### Diversity and caps

Avoid returning eight fetches from the same domain or same group unless the request explicitly narrows to one group. Suggested initial caps:

- Max 2 per domain by default.
- Max 2 per group by default.
- Always allow at least one official docs/README candidate when available.
- Always allow at least one source candidate when source evidence exists.
- Preserve the global suggestion cap from config or current default.

## Module changes

Likely files:

- `src/meta/suggested_fetches.rs`
- `src/meta/research_suggested_fetches.rs`
- `src/meta/security_suggested_fetches.rs`
- `src/core/repo_search.rs`
- `src/core/research.rs`
- `src/core/security.rs`
- `src/core/code_evidence.rs`
- `README.md`

Consider extracting shared logic into:

- `src/meta/fetch_ranking.rs`

This module can expose reusable scoring functions for repo/security/research fetch candidates without forcing all tool response types to become identical.

## Implementation steps

1. Define rank reason names as stable snake-case strings or an enum with `as_str()`.
2. Build candidate extraction from repo result groups.
3. Score each candidate using the deterministic weights above.
4. Apply diversity caps.
5. Sort by score descending, then original group/card order for deterministic ties.
6. Convert ranked candidates back to `RepoSuggestedFetch`.
7. Add public optional score/reason fields if compatible.
8. Repeat or share the same pattern for research/security suggested fetches if feasible in this pass.
9. Update README examples to show rank reasons and preferred tool.

## Tests

Add unit tests for:

- Source file with strong code evidence outranks generic docs for symbol queries.
- Official docs outrank source for generic API overview when no symbol is present.
- Exact-error issue with exact phrase outranks generic docs.
- Changelog/release notes outrank source for compare-version/migration context.
- Native advisory outranks exploit discussion in security mode.
- Commit-pinned raw permalink outranks mutable browser URL.
- Structured `repo_fetch` candidate sets preferred tool to `repo_fetch`.
- Diversity caps prevent one domain/group from dominating.
- Equal scores retain deterministic original order.

Add integration tests using mock cards with realistic metadata.

## Acceptance criteria

- Suggested fetches are ranked by deterministic score, not fixed first-card group order.
- Ranking reasons are visible in the response or can be inspected in tests.
- Existing suggested-fetch fields remain backward-compatible.
- Source, docs, issues, releases, security, and research workflows have sensible mode-aware ranking.
- No fetch ranking path performs network I/O or content fetches.
- Tests cover the principal ranking modes and tie behavior.
- `cargo test` passes.

## Handoff notes

Keep the ranking model explicit and boring. Do not introduce ML, embeddings, or heuristic opacity. The value is not a perfect search-rank algorithm; it is giving coding agents a predictable “read this next” order with enough reasons to debug mistakes.
