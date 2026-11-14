# Plan 005 — CodeGG Progressive Disclosure Integration

Status: implementation handoff plan
Primary implementation repo: `dbowm91/codegg`
Contract owner: `eggstack/eggsearch`
Depends on: eggsearch Plans 001–004

## Objective

Make CodeGG consume eggsearch's full capability set with minimal initial tool-definition context. Reuse CodeGG's existing `ToolCatalog`, deferred loading, `tool_search`, role-specific disclosure, `ResolvedToolSurface`, and structured search backend instead of introducing a second discovery subsystem.

The desired runtime shape is:

```text
small immediate palette
  -> compact tool discovery when needed
  -> hydrate 1–few full definitions
  -> call tool
  -> use eggsearch next_actions to hydrate likely follow-up tools
```

All eggsearch capabilities remain registered and callable.

## Current-state observations

CodeGG already has the necessary architectural primitives:

- `ToolCatalog` with keyword/BM25 search;
- `ToolDisclosure::{Core, Deferred, ProfileSpecific, Hidden}`;
- model-specific core/curated/minimal palettes;
- `ResolvedToolSurface` and parent capability ceilings;
- a `tool_search` tool constrained to the already-allowed policy set;
- raw eggsearch MCP tools hidden by default behind stable native wrappers;
- structured MCP result retention through `call_tool_structured`.

The main inefficiency is that `tool_search` currently returns the full `parameters` JSON schema for every match, up to ten matches. That can move context inflation from initial tool definitions into the discovery result rather than eliminating it.

## Non-goals

- Do not expose raw `mcp__eggsearch__*` tools by default.
- Do not create an eggsearch-specific discovery protocol inside CodeGG when the generic tool catalog can represent the metadata.
- Do not bypass existing permission, broker, capability-ceiling, or plan-mode filtering.
- Do not make deferred tools undiscoverable.
- Do not require all MCP servers to adopt eggsearch-specific metadata.

## Workstream A — Compact `tool_search`

### 1. Remove full schemas from ordinary search results

Change `ToolSearchTool` so default search results include only compact selection metadata, for example:

```json
{
  "name": "repo_fetch",
  "purpose": "Inspect a known repository file or source span",
  "use_when": "repo_search identified a concrete file/path/symbol",
  "not_for": "repository discovery or arbitrary URLs",
  "category": "ReadOnly",
  "disclosure": "deferred",
  "domain": "repository",
  "keywords": ["source", "file", "symbol", "line range"]
}
```

Do not include `parameters` by default.

If an explicit diagnostic mode is useful, allow one selected tool schema to be inspected, but normal agent discovery should hydrate the actual deferred definition through CodeGG's provider-definition machinery rather than serialize it into a text result.

### 2. Lower default result count

Return the best 3–5 matches by default. Keep a bounded larger maximum for explicit diagnostic/search cases.

Expose `total_matches` so truncation remains transparent.

### 3. Improve catalog indexing

Extend `ToolMetadata` with compact hidden indexing metadata where available:

- aliases;
- domain;
- keywords/capabilities;
- `use_when`;
- `not_for`;
- related/follow-up tools.

For eggsearch wrappers, source this metadata from the canonical eggsearch contract or a CodeGG translation table validated against it.

Search ranking should index these fields without necessarily returning all of them.

### 4. Make BM25 the preferred discovery mode

Evaluate BM25 against keyword mode on the new agentic selection corpus. Unless regressions appear, make BM25 the default for minimal-with-discovery profiles because semantic multi-word tool queries are the intended use case.

Do not add embeddings until BM25 fails measured cases; the catalog is small enough that lexical ranking is cheap and deterministic.

## Workstream B — Definition hydration

### 1. Hydrate selected tools, not schemas in prose

When `tool_search` selects a deferred tool, CodeGG should add the corresponding complete `ToolDefinition` to the next provider-visible tool surface.

Use the existing `deferred_tool_definitions` store and immutable per-turn surface model. Avoid creating a second mutable registry.

The implementation must decide and document whether hydration persists for:

- the remainder of the turn;
- the current agent run;
- or only the next provider request.

Preferred default: persist for the current run once selected, bounded by an LRU/relevance cap, because repeatedly re-discovering the same specialist tool wastes calls.

### 2. Keep policy monotonic

Hydration can only expose a tool already present in the current policy-allowed discoverable set. It must never bypass:

- denied tools;
- model-disabled tools;
- plan mode;
- missing backend state;
- parent capability ceiling;
- hidden disclosure.

Add tests for every omission reason.

### 3. Bound hydrated definitions

Keep a small maximum number of simultaneously hydrated deferred definitions, e.g. 3–5 beyond the core palette. Evict least-recently-used or no-longer-relevant definitions only between provider requests, never while a tool call is in flight.

## Workstream C — Eggsearch `next_actions` as guided disclosure

### 1. Parse canonical next actions

Eggsearch already returns validated `next_actions`. Teach the search backend to retain their target tool names and argument templates in structured results.

### 2. Hydrate justified follow-up tools

After a successful eggsearch call, mark high-priority next-action target tools as eligible for immediate hydration on the next model request.

Examples:

- `websearch` -> `webfetch`, `batch_fetch`;
- `repo_search` -> `repo_fetch`, `repo_map`, `batch_fetch`;
- `research_search` -> `webfetch`, `repo_fetch`, `batch_fetch`, `evidence_bundle`;
- `security_search` -> relevant fetch/evidence tools.

Use next actions as hints, not forced execution. The model still chooses whether to call the tool.

### 3. Do not broaden authority

A next action cannot hydrate a tool outside the resolved capability set. Filter it through the same policy used by `tool_search`.

### 4. Prefer graph-guided discovery over repeated search

If the current result explicitly recommends a valid follow-up capability, do not require another `tool_search` call merely to discover that capability. This reduces latency and redundant reasoning.

## Workstream D — Role-specific eggsearch palettes

Keep ordinary coding small:

- `websearch`
- `repo_search`
- `tool_search`
- optionally `webfetch` depending on measured frequency/context cost

Research role may receive immediately:

- `research_search`
- `repo_search`
- selected fetch/evidence tools

Security-review role may receive immediately:

- `security_search`
- only the fetch/evidence tools required by the workflow

Do not automatically expose every research/evidence wrapper to specialist roles if evaluation shows some are better hydrated from next actions.

## Workstream E — MCP structured result modernization

When eggsearch Plan 003 lands:

- prefer `structuredContent` over parsing JSON text/content blocks;
- retain backward compatibility with older eggsearch versions;
- validate against `outputSchema` when practical;
- distinguish MCP tool-level `isError=true` from transport/protocol failure;
- retain stable structured data before display clamping.

Update `architecture/mcp.md` and `architecture/search_backend.md` accordingly.

## Workstream F — Cache correctness

The current tool-definition cache should key on content identity, not count. Ensure the cache fingerprint covers:

- canonical tool name;
- description;
- full input schema;
- annotations/disclosure metadata that affects provider behavior;
- wire aliases;
- MCP tool-surface revision.

Hydration state must either be part of the cache key or applied after retrieving a cached base surface.

## Testing

Add focused tests covering:

- `tool_search` returns no full schema by default;
- BM25 finds `security_search` for vulnerability/CVE queries;
- BM25 finds `repo_fetch` for "read known source file/span";
- BM25 finds `batch_fetch` for "fetch these several URLs/files";
- top-k cap and `total_matches`;
- tool hydration adds the complete definition on the next request;
- denied/deferred-but-not-allowed tools never hydrate;
- `next_actions` can hydrate an allowed follow-up without a second search call;
- malicious/unknown next-action tool names are ignored;
- hydration cap/eviction behavior is deterministic;
- raw eggsearch MCP tools remain hidden;
- legacy eggsearch structured-content fallback still works.

## Metrics to collect

For representative coding tasks record:

- initial serialized tool-definition bytes/tokens;
- bytes/tokens after discovery hydration;
- number of model-visible tool definitions;
- number of `tool_search` calls;
- first-tool selection accuracy;
- redundant tool calls;
- malformed argument rate;
- completion success.

The implementation should demonstrate that context savings are real rather than merely relocated into `tool_search` output.

## Acceptance criteria

- Ordinary CodeGG runs retain access to every eggsearch capability.
- `tool_search` no longer emits full schemas for multiple matches by default.
- Discovery hydrates only the selected full definitions.
- High-confidence eggsearch `next_actions` can expose valid follow-up tools without another discovery round trip.
- Hydration never widens execution authority.
- Initial eggsearch-related tool-definition context is materially smaller than the current surface.
- Existing `websearch`, `webfetch`, repo, research, security, batch, and evidence integration tests continue to pass.

## Suggested CodeGG files

Likely touch points:

- `src/tool/catalog.rs`
- `src/tool/tool_search.rs`
- `src/tool/disclosure.rs`
- `src/agent/tool_surface.rs`
- `src/agent/request_preparation.rs`
- `src/search_backend/eggsearch.rs`
- `src/search_backend/mod.rs`
- `src/mcp/mod.rs`
- relevant architecture and integration tests

Keep ownership boundaries intact; do not move search policy into generic MCP transport code.