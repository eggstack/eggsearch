# Agent Context Efficiency Measurements

M005 measures current Eggsearch schemas and projections with serialized JSON bytes. Token figures are estimates at four bytes per token; no model tokenizer or live model was used. The task corpus and hydration mechanics are deterministic synthetic host simulations.

## Advertised tool surface and discovery

| Surface | Serialized bytes | Approx. tokens | Relative to full ten-tool definitions |
|---|---:|---:|---:|
| Ten MCP tool definitions | 72,939 | 18,235 | 100% |
| Core palette (`web_search`, `web_fetch`, `repo_search`) | 14,008 | 3,502 | 19.2% (80.8% smaller) |
| Compact discovery, max observed result | 769 total; 260 max per query | 193 total; 65 max per query | discovery response |

The 43-case tool-choice corpus scored top-1 43/43, recall@3 43/43 and MRR 1.000. The synthetic hydration run selected follow-up tools only when requested; one fixture execution hydrated six tools. This is tool-choice and schema-byte evidence, not end-to-end model completion quality or measured host round-trip latency.

The M001 15-task host trace serializes 21,221 diagnostic bytes, 21,539 standard bytes and 21,068 compact bytes (5,306 / 5,385 / 5,267 estimated tokens). All projections retain the same 14 relevant result documents present in the fixture result lists; one of 15 judged relevant documents was not retrieved by the fixture, matching M001 recall of 14/15 for the 13 ranking tasks. Failure and absence states remain distinct. The fixture implies 42 MCP tool calls across the 15 tasks when each fixture-retrieved relevant source is fetched once and each non-empty task is bundled once. This is a deterministic call-count simulation, not measured model turns; it does not include deferred-tool discovery or schema hydration calls. Per-task serialized byte counts and retained result counts are emitted by `retrieval_quality_host_trace_records_per_case_projection_cost`.

## Projection payload comparison

| Representative payload | Diagnostic bytes | Standard bytes | Compact bytes | Compact reduction |
|---|---:|---:|---:|---:|
| `web_search` with two cards and a failure | 1,642 | 1,634 | 1,478 | 10.0% |
| `web_fetch` with 2,000-character text and a link | 4,292 | not measured by this fixture | 2,278 | 46.9% |

The MCP projection suite confirms compact output retains partial evidence, failure-versus-absence, trust markers, stable IDs, truncation/focus, security applicability and evidence-bundle identity. Corpus-level compact output is only 0.7% smaller than diagnostic because these synthetic result cards are already short, and standard output is 1.5% larger than diagnostic for the typed repository projections. On a representative 2,000-character `web_fetch` payload, compact output falls from 4,292 to 2,278 bytes (46.9%). `response_detail` continues to default to diagnostic; the benchmark does not authorize changing that compatibility boundary. Callers needing smaller context can opt into `compact` and retain the unprojected MCP `structuredContent` outside the model window.

This harness measures serialized bytes and rough token estimates only. It does not measure provider latency, model turns, host storage behavior, or evidence sufficiency after real model reasoning. No global clipping or batch orchestration changes are justified from these fixtures.
