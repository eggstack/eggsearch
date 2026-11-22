# Retrieval Quality CodeGG Consumer Handoff

This is an Eggsearch-side handoff from the deterministic context experiment. CodeGG consumer implementation and acceptance remain owned by the CodeGG repository.

## Contract identity

- Eggsearch package version: `0.4.4`.
- Nine canonical MCP tools are advertised.
- The live tool-surface fingerprint is reported by `provider_status` / `tools/list`; do not pin it in prose.
- Tool-definition and core-palette byte counts are not pinned here; use the live surface. Compact discovery was measured at at most 260 bytes per observed query in the 43-case corpus.
- Projection modes: `compact`, `standard`, `diagnostic`. Omitted `response_detail` remains `diagnostic`.

## Recommended consumer behavior

1. Advertise the core palette initially: `web_search`, `fetch`, `repo_search`.
2. Use the existing tool discovery contract to find deferred tools, then hydrate only tools needed for the current task. Treat `next_actions` as advisory known-tool hints.
3. Request `response_detail=compact` only when a smaller model-visible payload is useful. Keep full returned `structuredContent` in host memory and attach it to later evidence-bundle construction as required.
4. On an unsupported response mode or a malformed/uncertain compact response, retry with the existing `standard` or `diagnostic` mode and preserve current tool-error repair behavior. Use `diagnostic` for failure investigation.
5. Key hydrated-schema caches by the full tool-surface fingerprint. Invalidate on any fingerprint change; do not key by package version alone.
6. Preserve source IDs, provenance/trust, truncation, warnings, partial-failure and absence indicators when selecting evidence. Never treat compact prose as the authoritative result when full structured content is available.

## Consumer acceptance tests

Register these as CodeGG-owned tests: initial core palette is callable; one deferred-tool lookup can hydrate a requested tool; hydration cache invalidates after fingerprint change; compact mode lowers serialized bytes while stable IDs, trust, truncation and failure/absence survive; full structured results remain available for evidence-bundle construction; unsupported compact mode falls back to standard/diagnostic; search-to-fetch-to-bundle trace does not fetch a source twice solely because of disclosure. Record serialized bytes and host round trips in CodeGG; the Eggsearch tests do not claim those consumer measurements.

M007 adds a bounded CodeGG Harness Lab acceptance across the RFC 9110 and
FEVER claim tasks. The exact model route, usage, verified tool sequence, and
remaining measurement limits are in
`docs/retrieval-quality-consumer-acceptance.md`.
