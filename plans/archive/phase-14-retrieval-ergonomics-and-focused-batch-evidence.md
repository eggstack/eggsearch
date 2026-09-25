# Phase 14 — Retrieval Ergonomics and Focused Batch Evidence

Status: completed
Depends on: phase 11; phase 13 preferred for structured locator reuse
Baseline for planning: `4a713ff82cec701534e285bbe3d330ae121f352c`
Roadmap: `plans/maintenance-codegg-quality-roadmap.md`

## Objective

Reduce MCP round trips and redundant document transfer for agent workflows by extending bounded focused extraction to `batch_fetch`, consolidating common locator/fetch execution machinery, and making repo/web/local fetch behavior more uniform without collapsing their public tool semantics.

## Current problem

`web_fetch` already supports deterministic model-free focus selection over extracted chunks. `batch_fetch` deliberately omits per-item focus, forcing agents to either retrieve larger documents or issue follow-up `web_fetch` calls. Repo suggested-fetch paths also construct structured locators independently, increasing duplicated conversion logic across workflows.

For CodeGG, a common pattern is search -> select several candidate files/pages -> inspect only the most relevant chunks. This should be expressible in one bounded batch request.

## Non-goals

- No autonomous recursive fetch/crawl.
- No model-based reranking.
- No unbounded parallelism.
- No removal of `web_fetch` or `repo_fetch`.
- No implicit traversal of links returned by fetched documents.

## Invariants

1. Existing SSRF, redirect, origin-concurrency, cache, sanitization, trust, local-root, and repo-locator safety rules remain authoritative.
2. Focus remains deterministic and operates only on already retrieved/extracted content.
3. Batch requests have both per-item and aggregate byte/character/time budgets.
4. One failed item does not discard successful sibling results unless the existing contract requires whole-request failure for malformed input.
5. Stable identities do not incorporate transient focus projection unless explicitly documented as a separate identity domain.
6. Focus metadata clearly distinguishes selected chunks from complete-document content.

## Production changes

### 1. Add per-item focus to batch fetch

Extend batch item requests additively with the same conceptual focus controls already supported by `web_fetch`:

```text
focus
focus_max_chunks
focus_max_chars
```

Prefer reuse of the exact existing focus request type/validation rules. If repo items support focused structured fetches differently, normalize the projection after the underlying repo fetch has returned a bounded document/span.

### 2. Add an aggregate batch output budget

Prevent N individually valid focused fetches from producing an excessive combined MCP response. Add an explicit aggregate budget with deterministic allocation.

A reasonable policy is:

- validate all item-level maximums;
- apply a server-level total character/byte cap;
- allocate in request order or a documented fair-share policy;
- mark truncated items and aggregate truncation explicitly.

Do not silently omit successful items.

### 3. Consolidate locator resolution

Audit conversions between:

- URL web fetches;
- structured `RepoFetchRequest` locators;
- SourceCard/code-evidence locators;
- local-workspace paths.

Create common internal locator types/helpers where this removes duplicated host/ref/path/line/symbol resolution.

A useful internal form may distinguish:

```text
WebUrl
RepoLocator
LocalLocator
```

without exposing one generic public MCP fetch tool.

### 4. Consolidate fetch execution policy

Common policy such as timeout resolution, max-char clamping, trust propagation, cache controls, and focus projection should live in shared internal execution code where semantics are identical.

Keep repo-specific revision resolution and local safe-open logic separate.

### 5. Improve suggested-fetch handoff

When repo/research/security workflows emit suggested fetches, include enough typed information for callers to pass a candidate directly into batch retrieval without reconstructing locators from URLs or prose.

For repo evidence, preserve structured repo locators when available. For web/research/security evidence, preserve URL and recommended extract/focus metadata where useful.

Any new response fields must be additive and deterministic.

### 6. Add batch-focused next-action recipes

Where several high-value evidence candidates exist, allow next-action generation to recommend one `batch_fetch` call with item-specific focus rather than many serial `web_fetch` calls.

Keep recipe generation bounded and avoid recommending batch fetch when candidates require materially different safety/config prerequisites.

### 7. Telemetry

Add response telemetry sufficient to understand:

```text
items requested/completed/failed/truncated
bytes/chars fetched
focused chunks selected
aggregate budget exhausted
cache hit/revalidation counts where available
```

Do not expose sensitive URLs/credentials beyond existing response semantics.

## Testing

Add deterministic tests for:

- mixed focused/unfocused batch items;
- web and repo items in one batch where currently supported;
- aggregate truncation;
- UTF-8-safe boundaries;
- focus with cache hit/revalidation;
- focus rejection for metadata-only content;
- individual timeout/failure isolation;
- local/repo locator safety preservation;
- suggested-fetch -> batch-fetch round-trip fixtures.

Property/fuzz coverage should reuse existing focus/chunk/bounded-reader targets where possible rather than duplicating fuzz harnesses.

## Acceptance criteria

- `batch_fetch` supports per-item deterministic focus with validation aligned to `web_fetch`;
- aggregate output/resource limits are explicit, tested, and observable;
- common locator conversion reduces duplicated SourceCard -> repo/web/local fetch code;
- common fetch policy is shared where semantics are truly identical;
- suggested-fetch responses preserve typed locators/recommended retrieval metadata sufficient for direct batch handoff;
- next-action generation can recommend focused batch retrieval for multi-source evidence workflows;
- existing fetch safety and trust contracts remain unchanged;
- `make check` passes on the exact candidate.
