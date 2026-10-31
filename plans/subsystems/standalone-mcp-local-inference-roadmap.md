# Standalone MCP Local-Inference Efficiency Roadmap

Status: closed (M001-M008 closed at their recorded candidates)
Scope: small local-model tool-use and context qualification through an actual inference API; not CodeGG integration

Long-term references: `plans/000-long-term-specification.md` ``1-4, 7-9; `plans/001-terminology-and-domain-model.md`; `plans/002-long-term-roadmap.md`; `plans/003-planning-process.md`.
Controlling ADRs: `plans/adrs/ADR-0001-ten-tool-surface-and-adapter-ownership.md` and `plans/adrs/ADR-0006-keyless-deterministic-retrieval-and-optional-extraction.md`.
Closed predecessor: `plans/subsystems/tool-surface-consolidation-roadmap.md` (M001-M007); do not reopen its closure or claim CodeGG work here.

## Purpose and ownership

Qualify Eggsearch's compact schema and brief result modes for small locally hosted models that need to call tools while keeping prompt context small. LM Studio is the available API host in this environment, including for MLX support; the acceptance target is model tool-use and context cost, not LM Studio UI behavior. Eggsearch owns the MCP server's advertised schema, protocol responses, deterministic projection and operator configuration; the host owns the model, tokenizer, context window and invocation. CodeGG's native wrappers, progressive disclosure and diagnostic storage are intentionally unchanged.

Primary classes: M001 capability (compact advertised schema profile); M002 capability (brief bounded output); M003 capability (small local-model tool-use and context qualification). Infrastructure includes measurement and config; polish follows qualified behavior.

## Baseline and findings

Reviewed main `a666517c39adf28f32fad04198c74cb70d935c5b` on 2026-10-30. `src/mcp/server.rs` registers ten stable tools and `list_tools` returns all decorated definitions regardless of client; `src/mcp/output_schema.rs` adds output schemas. `src/mcp/projection.rs` supports compact/standard/diagnostic, with diagnostic as omitted-parameter default; `src/mcp/tools/common.rs` uses native structured results plus a text compatibility fallback. `src/core/config.rs` defaults to 10 search results, and configured fetch defaults can reach 12,000 characters. Closed consolidation work already shortened descriptions and some input schemas but is oriented around clients that own progressive disclosure.

Recorded offline baseline: `docs/retrieval-quality-context-efficiency.md` reports 72,939 serialized bytes for all ten definitions (~18,235 byte/4 estimated tokens) and 14,008 bytes for the three-tool core palette. Its short-card compact response corpus saved only 0.7%, while a 2,000-character web fetch fixture saved 46.9%. These are historical baselines, NOT current token counts measured inside LM Studio or proof that the client includes all definitions verbatim.

LM Studio documentation supports local/remote MCP in `mcp.json` and warns about MCP context overhead. Its API can use `allowed_tools` to restrict a configured MCP server; the API restriction is not proof that interactive app chats dynamically hydrate Eggsearch tools. Sources: https://lmstudio.ai/docs/app/mcp and https://lmstudio.ai/docs/developer/core/mcp . Measure the actual client path in M003.

## Non-negotiable invariants

- Retain exactly ten canonical registered MCP tools and all current backend capabilities, provider behavior, argument compatibility, response semantics in the default profile, stable IDs, CLI/Python interfaces and both MCP transports. No mega-tool, new tool name, second retrieval engine, or LLM/embedding inference in eggsearch.
- `full` remains the install/default presentation, returning the same ten names and current advertised schemas and default `diagnostic` responses unless a caller opts in. Profile selection is **presentation**, not an authorization, SSRF, local-filesystem, tool execution, or provider-routing control.
- The recommended `compact` profile still advertises all ten canonical names. Each remains normally callable by name; advanced/legacy parameters remain accepted by runtime validation even if not listed in compact schemas. Operators can switch to `full` for complete schema help.
- Do not claim that pruning `tools/list` is capability-preserving: reduced-name palettes may be documented as LM Studio/client `allowed_tools` choices, not silently imposed by eggsearch.
- External material remains untrusted; retain trust markers, source locators, stable identity, provider failure vs evidence absence, partial coverage, security applicability, conflicts, and truncation signals. Never invent synthesis.
- No cache of model responses, persisted result blobs, opaque result-handle store, automatic re-fetch/crawl, or network authorization change. Explicit retrieval budgets remain independent of display budgets.
- Do not promise immediate model-context savings from JSON bytes alone. Record actual advertised bytes, estimated and model-tokenized costs, and LM Studio host-injection behavior separately.

## Target architecture

A single `ServerState` / router owns the full tools; an opt-in server presentation profile selects which descriptions, JSON input properties, and optional output-schema metadata appear in `tools/list`, with deterministic fingerprinting of the **advertised** profile. Do not fork handlers or silently reinterpret omitted fields. An independent optional `brief` response projection offers terse, bounded evidence, preserving the existing `compact`/`standard`/`diagnostic` response contracts and making the default brief only when explicitly chosen by the local-inference operator profile (per-call overrides always win). Both stdio and loopback HTTP use the same configuration.

Avoid an unnecessary additional server-side dynamic discovery tool: direct MCP hosts may not support schema hydration after initialization. All ten tool names remain exposed in the compact profile so specialist research, repository, security, evidence and diagnostics are reachable without a separate host.

## Dependencies and sequence

```text
M001 compact ten-tool advertised schema profile (closed)
  |
  +-- hard --> M002 brief/budgeted agent responses (closed)
                  |
                  +-- operational --> M003 small local-model tool-use and context qualification (closed at `772a073`; mixed outcome, compact remains opt-in)
                  +-- corrective --> M005 brief document extraction and paired requalification (closed)
                  +-- corrective --> M006 compact direct-fetch tool-choice corrective (closed at `6dc63d5`; hosted CI run `41181749609`)
                  +-- corrective --> M007 compact schema reference corrective (closed at `772a073`; hosted CI run `41220450246`)
                  +-- corrective --> M008 compact browser-render reachability corrective (closed at `772a073`; hosted CI run `41220450246`)

M004 compact schema discoverability corrective (closed on hosted Linux/macOS candidate de69f1d)
```

No CodeGG dependency; it is a regression consumer, not the implementation target. M003's offline qualification can run in CI; live small-model API turns are the operational closure evidence. LM Studio is the available inference host here, not a product-specific acceptance target.

| Milestone | Primary class | Source plan | Exit |
|---|---|---|---|
| M001 | capability | `plans/implementation/standalone-mcp-local-inference/001-opt-in-compact-tool-advertisement.md` | Opt-in compact `tools/list` for all ten existing names; measurable schema reduction; parity across MCP transports; default unchanged |
| M002 | capability | `plans/implementation/standalone-mcp-local-inference/002-brief-evidence-and-output-budgets.md` | Explicit brief projection and bounded results with critical failure, identity, trust and security semantics intact; unchanged legacy defaults |
| M003 | capability | `plans/implementation/standalone-mcp-local-inference/003-lm-studio-consumer-qualification-and-docs.md` | Closed at `772a073`: actual context reduction is measured across paired local-model tasks; tool and evidence quality are mixed, so compact remains opt-in and the full default is unchanged |
| M004 | capability | `plans/implementation/standalone-mcp-local-inference/004-compact-schema-discoverability-corrective.md` | Advertise runtime-accepted research goal/domain values in compact schemas; test schema parity and size; focused live evidence is incorporated in the completed M003 matrix |
| M005 | corrective | `plans/implementation/standalone-mcp-local-inference/005-brief-document-extraction-and-paired-requalification.md` | Closed on `de69f1d`; brief preserves same-document extraction within budget; model evidence and unresolved M003 findings are recorded; hosted Linux/macOS/Windows gates pass |
| M006 | corrective | `plans/implementation/standalone-mcp-local-inference/006-compact-direct-fetch-tool-choice-corrective.md` | Resolve or bound the repeated compact `web_fetch` no-call failure; its requalification and M003's mixed final disposition are recorded in their closure records |
| M007 | corrective | `plans/implementation/standalone-mcp-local-inference/007-compact-schema-reference-corrective.md` | Closed at `772a073`; all compact local references resolve; hosted Linux/macOS/Windows CI run `41220450246` passed |
| M008 | corrective | `plans/implementation/standalone-mcp-local-inference/008-compact-browser-render-reachability-corrective.md` | Closed at `772a073`; compact browser call and extraction qualified on LFM3 2B; hosted CI run `41220450246` passed |

## Cross-cutting controls

Compatibility/protocol: verify current `rmcp` schema serialization and protocol negotiation before implementing profile-specific tool definitions. MCP `tools/list` caching or `listChanged` support MUST NOT be assumed by clients. Profiles are process/config scoped, never inferred from user agent or model size. Fingerprints MUST differ when serialized contracts differ and remain stable across equivalent process launches.

Safety: every tool uses existing policy and validation gates. Removing an advertised property MUST NOT remove runtime validation. Diagnostics must not disappear from the unmodified default. A brief response must signal incomplete retrieval; when a client ignores structured content, text fallback must still be useful.

Resources: local inference budget is an agent-context budget; do not reduce remote search recall, upstream concurrency or browser/PDF capabilities as a side effect. Keep memory bounded, deterministic order and fast serialization; avoid doubling large JSON payloads in memory merely to render brief.

Observability: keep per-tool profile-size fixtures, model-visible response samples, omission counts, source-card retention, constraint-enforcement caveats and error/repair traces. Test actual small-model API tool-use and evidence quality, not merely canned keyword matching.

Documentation: M001/M002 update `architecture/mcp.md`, `architecture/config.md`, `docs/config.md`, `docs/tool-matrix.md` and `docs/test-inventory.md` as applicable. M003 maintains optional LM Studio operator guidance and records the model qualification method/results. Never rewrite historical closure records.

## Known risks and decision boundaries

- Some MCP clients may reject incomplete or overly permissive input JSON schemas. If tests show this, retain required fields or fall back to the current schema instead of weakening validation.
- Omitting `outputSchema` may improve bytes but could reduce typed tool-result handling. Test real clients; keep `full` unchanged and omit only if measured safe.
- The brief view could collapse retrieval failures into apparent absence or hide applicability. Security/failure fixtures are hard gates, not optional benchmarks.
- Models may perform worse with less explicit tool parameter instruction. Do not land profile changes solely on size savings; M003 must measure call and repair quality.
- A host may include both MCP `structuredContent` and fallback `content` in context. Characterize duplication rather than assuming one is used.

## Definition of done

Every milestone gets an exact-SHA closure record in `plans/closure/standalone-mcp-local-inference/NNN-status.md`; update this roadmap and `plans/registry.md` in each closure commit. No full rollout or release, and no claim of small-model context or quality improvement without measured paired API turns.
