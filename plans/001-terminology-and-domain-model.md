# Eggsearch Terminology and Domain Model

Status: normative long-term terminology

This document defines the authoritative language for eggsearch planning and
implementation. Subsystem roadmaps and implementation plans MUST use these
terms consistently and MUST NOT redefine them locally. Amendments follow
`plans/003-planning-process.md` §2.1.

## 1. Core pipeline terms

| Term | Definition |
|---|---|
| Discover | Provider fan-out producing `SourceCard` candidates without fetching page content. |
| Rank | RRF aggregation plus intent/freshness reranking with deterministic tie ordering. |
| Select | Bounded candidate selection under per-tool budgets. |
| Bounded fetch | Fetch through `read_bounded_body()` / `ForgeReadBudget` with size, deadline, and redirect bounds. |
| Evidence bundle | Deterministic handoff packaging from `build_evidence_bundle`: dedup, linking, caps, trust/provider summaries, gaps. Never ranking math. |
| SourceCard | Compact discovery output, not fetched page content. |
| Excerpt | Bounded source-derived passage (at most 3 per card, 500 chars each, 1,200 total), merged deterministically and sanitized. |
| Focus read | Deterministic query-focused `focus` projection over fetched content (lexical chunk ranking, no extra traversal). |

## 2. Provider and engine terms

| Term | Definition |
|---|---|
| Provider | A named upstream source identified by an entry in `KNOWN_PROVIDER_IDS` (37 IDs). |
| Engine | A vendored `SearchEngine::search(&EngineSearchRequest)` implementation serving one or more providers. `local_workspace` has no engine; it is served by the local backend. |
| Native enforcement | The provider API itself constrains results (see `000-long-term-specification.md` §5). |
| Local approximation | Eggsearch constrains results after retrieval because the provider cannot. |
| Capability-skip attempt | An explicit dispatch record that a provider was skipped for an unsupported capability. Silent omission is forbidden. |
| Provider health | Process-local cooldown/diagnostics owned by the adapter and surfaced via `provider_status` and CLI diagnostics using the same core service. |
| Probe | Real, bounded provider liveness check using the same core service as CLI diagnostics. |

## 3. Workflow terms

| Term | Definition |
|---|---|
| `WorkflowExecution` | Shared execution primitive for domain workflows. |
| `RetrievalAttemptSet` | Shared record of retrieval attempts; domain adapters MUST record through it. |
| `FetchCandidateBuilder` | Shared suggested-fetch construction seam; domain builders MUST construct through it. |
| Typed planner | Domain-specific `repo`/`research`/`security` planning logic layered over shared primitives. Must stay typed per domain. |
| Result group | Typed per-domain grouping (`ResearchResultGroupKind` vs `SecurityResultGroupKind`). Over-deduplication across taxonomies is forbidden. |
| Canonical resolution | Goal/workflow/source resolution owned by `src/mcp/tools/canonical.rs`. Domain tools call it, never duplicate it. |

## 4. Fetch and transport terms

| Term | Definition |
|---|---|
| `FetchClient` | Shared fetch executor owning one shared eggfetch client. |
| Resolved-address pinning | Binding the physical route to the address snapshot authorized by eggsearch while preserving the logical hostname for HTTP/TLS. |
| `EggressDialer` | Optional physical-route layer beneath the eggfetch `Dialer` seam, provider-only, fail-closed, credential-indirected. |
| Outcome B | The selected egress architecture: SSRF-safe narrow route with source-build `egress` opt-in and no second SKU. |
| Direct route | Pinned direct connection without egress. REQUIRED for dynamic `FetchClient` targets, forge, updater, loopback, rmcp, and browser paths. |
| OriginController | Sole retry/circuit authority consuming typed transport evidence. |

## 5. Local workspace terms

| Term | Definition |
|---|---|
| Inventory | Git discovery/identity snapshot of the workspace. |
| SymbolBackend | Bounded structured-parsing seam; regex remains the fallback. |
| Repo map | Deterministic package/module/symbol/test/build structure from `repo_map`. |
| Forge adapter | Shared safety owner for forge access: base-URL validation, address classification, bounded reads, redirect rejection. |

## 6. Distribution terms

| Term | Definition |
|---|---|
| Seven-target matrix | Release qualification across seven targets with exact 16-file assembly. |
| Qualification | Non-publishing `mode=qualify` run on an exact SHA. Qualification is SHA-specific; a different release candidate MUST be re-qualified. |
| Closure candidate | The exact immutable commit a phase qualifies and closes against. |
| SKU | A published binary variant. Multiple feature-specific SKUs are forbidden. |

## 7. Planning terms

Defined normatively in `plans/003-planning-process.md`: invariant,
capability, infrastructure, polish; hard/interface/soft/operational
dependencies; proposed/ready/active/blocked/closing/closed/conditionally
closed/superseded/archived status vocabulary.
