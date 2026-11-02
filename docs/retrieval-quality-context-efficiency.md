# Agent Context Efficiency Measurements

This report measures serialized JSON bytes for current Eggsearch tool definitions and MCP projections. Token figures are estimates at four bytes per token; no model tokenizer was used. The 15-task corpus is a deterministic synthetic host simulation.

## Advertised tool surface and discovery

| Surface | Serialized bytes | Approx. tokens | Relative to full ten-tool definitions |
|---|---:|---:|---:|
| Ten MCP tool definitions from initialized stdio `tools/list` | 72,923 | 18,231 | 100% |
| Compact ten-tool profile from initialized stdio `tools/list` | 18,545 | 4,637 | 25.4% (74.6% smaller) |
| Core palette (`web_search`, `web_fetch`, `repo_search`) | 14,008 | 3,502 | 19.2% (80.8% smaller) |
| Compact discovery, max observed result | 769 total; 260 max per query | 193 total; 65 max per query | discovery response |

The 43-case tool-choice corpus scored top-1 43/43, recall@3 43/43 and MRR 1.000 in both full and compact description profiles. The synthetic hydration run selected follow-up tools only when requested; one fixture execution hydrated six tools. This is deterministic discovery and schema-byte evidence, not end-to-end model completion quality or measured host round-trip latency.

The 15-task host trace serializes 21,221 diagnostic bytes, 21,539 standard bytes, 21,068 compact bytes and 18,715 brief bytes (5,306 / 5,385 / 5,267 / 4,679 estimated tokens). Every projection retains the same 14 relevant documents present in fixture result lists; one judged document was not retrieved by the fixture. Failure and absence remain distinct. The trace implies 42 MCP calls if each retrieved relevant source is fetched once and each non-empty task is bundled once. This is a deterministic call-count simulation, not measured model turns; it excludes deferred-tool discovery and schema hydration. Brief is 11.8% smaller than diagnostic on this sparse corpus, below the threshold by design; per-task counts are emitted by `retrieval_quality_host_trace_records_per_case_projection_cost`.

## Projection payload comparison

| Representative payload | Diagnostic bytes | Brief bytes | Brief reduction vs diagnostic |
|---|---:|---:|---:|
| `web_search` with rich source metadata and a provider failure | 2,728 | 1,418 | 48.0% |
| `web_fetch` with 2,000-character text and a link | 4,292 | 1,090 | 74.6% |
| `repo_search` grouped stress fixture with redundant metadata | 10,846 | 669 | 93.8% |
| `research_search` grouped stress fixture with redundant metadata | 10,846 | 669 | 93.8% |

The brief projection tests preserve partial retrieval, failure-versus-absence, trust and injection markers, stable IDs, truncation and PDF page coordinates, security applicability, failed batch members, and evidence-bundle identity. Large applicability lists retain affected/not-affected/unknown totals and expose per-entry omissions while staying inside the configured budget. Brief serializes the same projected data into both structured content and its text fallback. Its 8,000-character default display budget is configurable from 512 to 32,000 characters; omitted result counts and truncation are reported. The web and fetch fixtures are nontrivial samples; the repository and research figures are synthetic stress fixtures with deliberately redundant metadata and are not model-prompt or production-average claims. Compact mode defaults to brief only when selected in server configuration; the full profile remains diagnostic by default.

This harness measures serialized characters/bytes and rough token estimates only. It does not measure provider latency, actual model tokens, host storage behavior, or evidence sufficiency after real model reasoning. LM Studio consumer qualification is a separate operational gate.
