# Phase 5: Deterministic Cross-Tool Identity Model

## Objective

Make deterministic identity and provenance links consistent across eggsearch tools. Source cards, suggested fetches, repo locators, local workspace results, fetch responses, batch fetch items, and evidence bundle entries should share a stable identity model so agents can search, fetch, batch-fetch, and bundle evidence without losing provenance or duplicating sources.

This phase turns evidence handoff into a first-class contract. It is especially important for codegg, where manager, coder, reviewer, and security agents may pass evidence between each other.

## Current problem statement

Eggsearch already has many evidence-related objects: source cards, source metadata, suggested fetches, fetch responses, repo fetch responses, document blocks/chunks, quality metadata, trust markers, and evidence bundle IDs. These pieces are useful, but identity should be made uniform across the tool surface.

Agents need to answer:

- Which search result produced this fetch?
- Which provider/subquery/rank produced this source?
- Have I already fetched this URL or repo locator?
- Which evidence bundle item corresponds to which source card?
- Did this local result map to the same remote repository as this source card?
- Which warnings apply to which source/fetch/document?

## Scope

In scope:

- Define canonical ID generation rules.
- Add or normalize IDs for sources, suggested fetches, repo locators, fetches, documents, and evidence bundle links.
- Add parent-child provenance links between search results and fetches.
- Add stable dedupe keys for URLs and repo locators.
- Preserve compatibility with existing IDs where possible.
- Add deterministic tests.

Out of scope:

- Re-ranking results.
- Persisting IDs across a database.
- Adding a local cache unless a tiny in-memory helper is needed for tests.
- Summarizing evidence.

## Identity requirements

### Source identity

Each source card should have a deterministic `source_id` derived from normalized fields such as:

- canonical URL or repo locator;
- source kind/role where relevant;
- provider ID(s);
- query/subquery identity;
- result origin if needed to avoid collisions.

Existing `src_001`-style display IDs can remain for compact response readability, but a stable ID should be available for cross-tool linking.

### Fetch identity

Each fetch response should have a deterministic `fetch_id` derived from:

- final canonical URL or repo locator;
- selected line/span/symbol/match parameters;
- extract mode;
- content transform where applicable.

A fetch response should include `source_id` when it was created from a suggested fetch or source card.

### Repo locator identity

Structured repo locators should have deterministic IDs based on:

- host;
- owner/namespace;
- repo;
- ref/commit;
- path;
- line range or symbol/match fields;
- local-vs-remote source where relevant.

### Document identity

Fetched document IDs should be tied to fetch IDs, with chunk/block IDs derived from document ID plus block/chunk index and line span.

### Evidence bundle identity

Evidence bundle IDs should remain deterministic and should link sources and fetches by canonical IDs. Bundle gap analysis should report missing fetches by `source_id` and suggested `fetch_id` where possible.

## Canonicalization rules

Define explicit canonicalization helpers for:

- URL normalization: scheme/host case normalization, default port removal, fragment handling policy, trailing slash policy, percent-encoding normalization where safe.
- Repo locator normalization: host aliases, owner/repo casing policy, ref defaults, path normalization without allowing traversal.
- Local file normalization: absolute canonical path within an allowed root plus repo identity if available.

Be conservative. Avoid normalizing away semantically meaningful query parameters unless there is a well-defined provider-specific rule.

## Implementation steps

1. Inventory existing ID functions in evidence bundle and source/fetch code.
2. Define a central `identity` module or extend the existing evidence identity helpers.
3. Add canonical key structs for URL fetches, repo locators, local file spans, source cards, and document chunks.
4. Implement deterministic ID generation using a stable hash. Prefer a compact prefix plus hex/base64url digest, for example `src_`, `fetch_`, `repo_`, `doc_`, `chunk_`, `bundle_`.
5. Add stable IDs to source card metadata or top-level fields where appropriate.
6. Add `source_id`/`parent_source_id` fields to suggested fetches and fetch outputs where feasible.
7. Update `build_evidence_bundle` to prefer canonical IDs over positional matching.
8. Ensure batch fetch preserves input order while also returning deterministic fetch IDs.
9. Add compatibility tests so existing simple IDs are not accidentally removed from responses if agents may depend on them.

## Required tests

Add tests for:

- Same source card input produces same stable ID across repeated runs.
- URL case/default-port normalization behaves deterministically.
- Different meaningful query parameters produce different IDs.
- Same repo locator produces same ID independent of field ordering.
- Different line ranges produce different fetch IDs.
- Source-to-fetch-to-bundle links remain intact.
- Duplicate sources dedupe by canonical key but preserve provider provenance.
- Local file identity stays inside configured workspace roots.
- Evidence bundle gap analysis reports missing fetches by stable source ID.
- Legacy compact IDs remain present if currently part of public output.

## Acceptance criteria

- Source cards expose deterministic stable identity.
- Suggested fetches can be linked back to source cards.
- Fetch responses can be linked to source cards or explicit locator IDs.
- Evidence bundles use stable source/fetch links.
- Dedupe behavior is deterministic and tested.
- No trust boundary is weakened by canonicalization.

## Risks and mitigations

Risk: URL canonicalization accidentally merges distinct resources.

Mitigation: Use conservative canonicalization and test query/fragment edge cases. Do not strip query parameters globally.

Risk: Adding IDs expands response payloads.

Mitigation: Keep IDs compact and avoid duplicating large provenance objects when a link is sufficient.

Risk: Existing `src_001` display IDs are useful for humans.

Mitigation: Keep display IDs and add stable IDs separately, at least during transition.

## Handoff notes

This phase should avoid behavior changes outside identity/linking. Do not rewrite ranking, grouping, or fetch extraction. The main deliverable is a reliable provenance spine across tools.
