# Retrieval Quality CodeGG Consumer Handoff

This is an Eggsearch-side handoff from the deterministic context experiment. CodeGG consumer implementation and acceptance remain owned by the CodeGG repository.

## Contract identity

- Eggsearch package version: `0.4.4`.
- Ten canonical MCP tools remain unchanged.
- Tool-surface fingerprint: `eggsearch-0.4.4|tools=batch_fetch:150,build_evidence_bundle:144,provider_status:148,repo_fetch:150,repo_map:146,repo_search:186,research_search:160,security_search:159,web_fetch:159,web_search:160|bytes=72939`.
- Full serialized tool definitions: 72,939 bytes. Core palette definitions: 14,008 bytes. Compact discovery: at most 260 bytes per observed query in the 43-case corpus.
- Projection modes: `compact`, `standard`, `diagnostic`. Omitted `response_detail` remains `diagnostic`.

## Recommended consumer behavior

1. Advertise the core palette initially: `web_search`, `web_fetch`, `repo_search`.
2. Use the existing tool discovery contract to find deferred tools, then hydrate only tools needed for the current task. Treat `next_actions` as advisory known-tool hints.
3. Request `response_detail=compact` only when a smaller model-visible payload is useful. Keep full returned `structuredContent` in host memory and attach it to later evidence-bundle construction as required.
4. On an unsupported response mode or a malformed/uncertain compact response, retry with the existing `standard` or `diagnostic` mode and preserve current tool-error repair behavior. Use `diagnostic` for failure investigation.
5. Key hydrated-schema caches by the full tool-surface fingerprint. Invalidate on any fingerprint change; do not key by package version alone.
6. Preserve source IDs, provenance/trust, truncation, warnings, partial-failure and absence indicators when selecting evidence. Never treat compact prose as the authoritative result when full structured content is available.

## Consumer acceptance tests

Register these as CodeGG-owned tests: initial core palette is callable; one deferred-tool lookup can hydrate a requested tool; hydration cache invalidates after fingerprint change; compact mode lowers serialized bytes while stable IDs, trust, truncation and failure/absence survive; full structured results remain available for evidence-bundle construction; unsupported compact mode falls back to standard/diagnostic; search-to-fetch-to-bundle trace does not fetch a source twice solely because of disclosure. Record serialized bytes and host round trips in CodeGG; the Eggsearch tests do not claim those consumer measurements.
