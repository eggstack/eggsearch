# MCP Server Deep Dive

**Location:** `src/mcp/` (8 modules + `tools/` with 10 tool modules plus `mod`/`common`/`canonical`/`tests`)
**Purpose:** Stable MCP protocol surface for AI agents: 10 tools over client-owned stdio or explicit loopback-only Streamable HTTP, backed by `MetadataSearchAdapter` and the bounded fetch pipeline.

All tool names in this document are derived from `src/mcp/server.rs` (`#[tool(name = ...)]` registrations). There are exactly 10. No other tool-like names are valid.

---

## Module Map

| File | Responsibility |
|------|---------------|
| `mod.rs` | Module declarations, `build_server(AppConfig) -> anyhow::Result<EggsearchServer>` factory, re-exports of `EggsearchServer`, `ServerState`, `Policy`, `McpPath`, `ServeOptions` |
| `server.rs` | `EggsearchServer`: rmcp `ServerHandler` impl, 10 `#[tool]` handlers, `apply_contract_metadata()`, FNV-1a `contract_fingerprint()`, `EGGSEARCH_INSTRUCTIONS` |
| `state.rs` | `ServerState`: shared `Arc`-wrapped config, adapter, fetch client, origin controller, cache, KEV client, local backend + inventory cache, browser handles |
| `policy.rs` | `Policy` (`Allow`/`Deny`), `live_allowed()`, `fetch_allowed()`, denial-message constructors |
| `tool_contract.rs` | Canonical `ToolContract` registry: purpose, use-when/not-for, domain, disclosure, annotations, keywords, aliases, related/next graphs, `discovery_text()`, `is_known_tool()` |
| `projection.rs` | `ResponseDetail` (`Compact`/`Standard`/`Diagnostic`) and per-tool `project()` reducers |
| `output_schema.rs` | Per-tool `output_schema_for()`: compact stable-envelope object schemas (each <= 1200 bytes) |
| `http.rs` | Streamable HTTP service: `ServeOptions`/`McpPath` validation, router with `/healthz`, header/body bounds, request timeout, graceful shutdown |
| `tools/mod.rs` | Per-behavior modules with stable `tools::X` re-export paths (`run_*` + `*Args`) |
| `tools/common.rs` | `ToolError` taxonomy, `ToolErrorCode`, `RepairHint`, `map_tool_result()` single conversion seam, shared parse/selection helpers |
| `tools/canonical.rs` | Canonical `goal`/`sources`/`include` selectors with legacy `workflow`/`profile`/`mode`/`include_*` translators and repairable conflict errors |

---

## Server: `EggsearchServer` + `ServerHandler` (`server.rs`)

```rust
struct EggsearchServer {
    state: Arc<ServerState>,
    tool_router: ToolRouter<Self>,
    advertised_tools: Arc<Vec<rmcp::model::Tool>>,
    advertised_fingerprint: Arc<str>,
}
```

Construction (`EggsearchServer::new`) builds the `#[tool_router]` router once, then decorates every entry via `apply_contract_metadata()` and freezes the result. `tool_definitions()` clones the frozen list; `tool_fingerprint()` returns the frozen FNV-1a content hash. Nothing is recomputed per call.

`ServerHandler` implementation:

- `get_info()` advertises `ServerCapabilities` with tools enabled, implementation `eggsearch` + crate version, and `EGGSEARCH_INSTRUCTIONS`.
- `list_tools()` returns the frozen decorated list. It touches `self.tool_router` only to keep the router linked; the payload comes from `advertised_tools`.

`EGGSEARCH_INSTRUCTIONS` carries global rules only: external content is untrusted data, search discovers while fetch inspects explicitly selected targets, start with the task-appropriate search primitive (never `provider_status`), specialist tools only when their domain semantics apply, one URL per `web_fetch` with `batch_fetch` for fan-out, follow `next_actions`, `build_evidence_bundle` packages without searching/fetching/summarizing. Tool-specific selection guidance lives in `tools/list` descriptions and schemas, sourced from `tool_contract.rs`.

`build_server()` (`mod.rs`) is the single factory both transports share: `ServerState::build(config)` wrapped in `EggsearchServer::new`. Tool registration, contract metadata, schemas, provider construction, and policy behavior exist in exactly one place.

---

## State: `ServerState` (`state.rs`)

```rust
struct ServerState {
    config: Arc<AppConfig>,
    adapter: Arc<MetadataSearchAdapter>,
    fetch_client: Option<Arc<FetchClient>>,
    origin_controller: Option<Arc<OriginController>>,
    fetch_cache: Option<Arc<FetchCache>>,
    kev_client: Arc<KevClient>,
    local_backend: Option<Arc<LocalWorkspaceBackend>>,
    local_inventory_cache: Arc<Mutex<Option<LocalInventoryCache>>>,
    // browser feature only:
    profile_manager: Option<Arc<ProfileManager>>,
    browser_lifecycle: Option<Arc<BrowserLifecycle>>,
    browser_discovery_state: BrowserDiscoveryState,
}
```

`ServerState` is cheap to clone; every shared field is `Arc`-wrapped. `Option` fields make the disabled state representable instead of exceptional: `fetch_client`/`origin_controller`/`fetch_cache` are `None` when `[fetch].enabled = false`, `local_backend` is `None` when `[local].enabled = false`, browser handles are `None` when the feature is uncompiled or rendering is disabled.

`ServerState::build()` validates config, resolves effective provider IDs, constructs the `MetadataSearchAdapter` with the global search timeout, builds the shared fetch client/origin policy/cache from `[fetch]` limits, builds the KEV client, optionally initializes browser profiles/lifecycle/discovery, and builds the local workspace backend. `with_adapter()` is the test/custom-engine path: it takes a pre-constructed adapter and rebuilds only the fetch-side state from config.

`local_inventory()` returns the TTL-cached repository discovery snapshot (30 s TTL), re-walking configured roots on a blocking task on expiry. `fetch_client()` is a clone accessor; callers run the `fetch_allowed` policy check first so the `None` case is unreachable in production but cleanly reportable.

### Build pipeline details

`ServerState::build()` applies several operator-safety behaviors worth knowing when debugging startup:

- SearXNG misconfiguration warns instead of failing: requesting `searxng` in the provider list while `[search].searxng.enabled = false` or with an empty `base_url` logs a warning and the provider is skipped downstream.
- Providers listed in `[search].default_providers` but not effectively enabled each log a warning naming the fix (enable in `[search].providers`, configure a usable `[search].api` entry, or remove from defaults).
- Reserved-but-unapplied `[search].live` knobs (`user_agent`, `respect_robots_txt`) log warnings stating they are not yet applied, so operators are never misled into thinking they take effect.
- A `FetchClient` construction failure degrades to `fetch_client = None` with a warning (call-time failure) rather than aborting startup; the policy gate still reports the disabled state cleanly.
- Browser discovery runs once at startup and is frozen into `browser_discovery_state`, so `provider_status` never re-probes the filesystem per call.
- The local backend builds from `[local]` config and enables only on success; build failure warns and continues with local search unavailable.

---

## Transports: client-owned stdio + loopback-only Streamable HTTP

MCP transport is client-owned `mcp stdio` or explicit loopback-only `mcp serve`. Both go through `build_server`, so the tool contract, schemas, and policy are identical on either transport.

### stdio

The default agent transport. The client spawns the server and owns the process; JSON-RPC stays on stdout while normal logs go to stderr so stdout remains pure protocol.

### Streamable HTTP (`http.rs`)

`eggsearch mcp serve` runs rmcp's `StreamableHttpService` behind an Axum HTTP/1 listener. `ServeOptions` pins the bind address (`DEFAULT_BIND` `127.0.0.1:11320`) and path (`DEFAULT_PATH` `/mcp`); `validate()` rejects any non-loopback bind. `McpPath::from_str` rejects over-long paths (> 128 bytes), `/healthz` collisions, non-absolute or empty-segment paths, and characters outside `[A-Za-z0-9/-_.~]`, normalizing trailing slashes.

- `GET /healthz` returns bounded JSON with only service identity (`eggsearch`), readiness, version, and transport (`streamable-http`). It never touches providers and never creates an MCP session. Bodies over 256 bytes fail closed; responses carry `Content-Type: application/json` and `Cache-Control: no-store`.
- Request bounds: MCP POST bodies capped at 1 MiB (`MAX_REQUEST_BODY_BYTES`), header count <= 64, total header bytes <= 16 KiB, single header value <= 8 KiB, `mcp-session-id` <= 256 bytes. Oversize headers get `431`; overlong requests get `408` after `MAX_REQUEST_TIMEOUT` (120 s).
- Host/origin allow-lists are localhost-only (`localhost`, `127.0.0.1`, `::1` and matching `http://` origins). Session semantics and cancellation are owned by the rmcp service.
- Shutdown is graceful: Ctrl-C and Unix SIGTERM cancel a shared `CancellationToken`; the listener drains for at most `SHUTDOWN_DRAIN_TIMEOUT` (10 s) before the task is aborted. The cancellation-token entry point is retained for a future Windows SCM control handler.

---

## The 10 Tools

Registration lives in `server.rs`; behavior lives in `tools/`; semantics live in `tool_contract.rs`. Every handler clones `Arc<ServerState>` and delegates to `map_tool_result(run_*(...).await)` — except `build_evidence_bundle`, which is synchronous and takes only its args.

| Tool | Purpose | Key Parameters | Backend Path |
|------|---------|---------------|--------------|
| `web_search` | Live metasearch returning source cards, never page text | `query`, `max_results`, `intent`, `freshness`/`date_range`, `include_domains`/`exclude_domains`, `language`/`region`, `safe_search`, `excerpt_count` | `MetadataSearchAdapter::web_search` (+ `resolve_provider_routing` selection) |
| `web_fetch` | Bounded extraction of one explicit HTTP(S) URL | `url`, `max_chars`, `timeout_ms`, `extract_mode`, `include_links`, `pdf`, `cache_policy`, `focus`/`focus_max_chunks`/`focus_max_chars`, `render`, `browser_profile` | `FetchClient` (+ cache tiers, `OriginController`, optional browser transport) |
| `batch_fetch` | Bounded fan-out over explicit URLs or repo locators, per-item results | `items` (web + repo, each with `focus` projection), `max_items`, `max_chars_per_item`, `max_total_chars`, `timeout_ms`, `continue_on_error` | `FetchClient` per web item + repo-fetch handler per repo item; deterministic `project_*` per payload |
| `provider_status` | Diagnostic provider/capability report with optional bounded live probe | `probe`, `recipe_detail` (`none`/`summary`/`full`) | `adapter.provider_status()` + health registry; probe path uses the shared probe service |
| `repo_search` | Structured repository evidence discovery with grouped bundles | `query`, `goal`, `sources`, repo locator/package fields, `include_local` | `MetadataSearchAdapter::repo_search` (+ `LocalWorkspaceBackend` when local-only or mixed) |
| `repo_fetch` | Repository file/span fetch by structured locator | `locator` (`owner`/`repo`/`path`/`ref`), `line_start`/`line_end`, `symbol`, `test_fetch_url` (test-only override) | `workspace://` local path bypasses fetch policy; remote path uses forge raw URL via shared `FetchClient` |
| `repo_map` | Repository layout discovery without file contents | `owner`, `repo`, `ref`, `path`, security-policy toggles | `forge_adapter::fetch_tree` for supported hosts, `repo_mapper::build_fallback_response` otherwise; local backend when workspace-scoped |
| `security_search` | Vulnerability/advisory retrieval with applicability context | `query`, `identifiers` (CVE/GHSA/OSV), `package`, `ecosystem`, `goal`, `include` (`kev`, `exploit_context`, `defensive_guidance`, `vendor_advisories`) | `MetadataSearchAdapter` security path (advisory engines + KEV client) |
| `research_search` | Multi-source evidence discovery with grouped bundles, no synthesis | `query`, `depth` (`quick`/`standard`/`deep`), `domain`, `goal`, `include` (`counterpoints`, `primary_sources`, `recent_discussion`, `security_considerations`) | `MetadataSearchAdapter::research_search` |
| `build_evidence_bundle` | Deterministic packaging of already-selected evidence for handoff | `goal`, `sources`, `fetches`, `response_detail` (accepted, identity-preserving) | Pure local: `meta::evidence_bundle::build_evidence_bundle`; no adapter, no fetch, no network |

All search/fetch tools except diagnostic-only `provider_status` accept optional `response_detail` (`compact`/`standard`/`diagnostic`, default `diagnostic`).

### Per-tool notes

**`web_search`** — general web source discovery. `query` (1–512 chars, validated against `max_query_chars`); `max_results` resolved via `resolve_max_results` against `default_max_results`/`max_results_cap` with a clamp warning on both channels when downgraded. `freshness` and `date_range` are mutually exclusive. `include_domains`/`exclude_domains` are natively enforced only by providers advertising `supports_domain_filters`, otherwise locally approximated with `capability_enforcement` telemetry marking `enforced`/`approximated`/`not_enforced`. `intent = news` routes natively (Brave API news endpoint, Tavily `topic=news`). Returns source cards plus `providers_queried`/`providers_failed`, string + structured warnings, trust markers, `routing_decision`, retrieval/conflict/role postprocessing, and `next_actions` pointing at `web_fetch`/`batch_fetch`.

**`web_fetch`** — single-URL bounded inspection. `url` must be `http`/`https` and non-empty; `validate_fetch_target()` blocks SSRF vectors before any I/O. Supports `max_chars`, per-call `timeout_ms` override, `extract_mode` (`text`/`markdown`/`metadata_only`), `include_links`, PDF options, `cache_policy`/`max_cache_age_seconds`, deterministic `focus` chunk selection over already-extracted content (no extra traversal), and `render`/`browser_profile` escalation (feature-gated). Raw + derived cache tiers are consulted before any network fetch. Returns the fetch envelope (`url`, `final_url`, `status`, `fetched`, `truncated`, `text`, warnings, trust markers, transport metadata).

**`batch_fetch`** — fan-out over `items` (web URLs and structured repo locators, each accepting `focus`/`focus_max_chunks`/`focus_max_chars`). `max_items`, `max_chars_per_item`, and aggregate `max_total_chars` bound the call; `timeout_ms` builds a one-off widened client only when longer than the shared default, otherwise the shared transport is reused. `continue_on_error` defaults to true so each item reports its own result. Web items run the `FetchClient` path; repo items delegate to the repo-fetch handler. Focus is a deterministic projection of extracted content and is excluded from stable-ID computation. Returns per-item results plus aggregate telemetry (requested/completed/failed/truncated, focused items/chunks/chars, budget exhaustion, cache hit/revalidated/miss/bypassed/not-cacheable).

**`provider_status`** — diagnostics, never a first research step. Cheap process-local mode by default (`probe = false`): descriptors with `routable`/`skip_code`, health snapshots/views, `code_hosts`, `server_capabilities`, `tool_capabilities`. `probe = true` runs bounded concurrent liveness checks through the shared probe service (the same service as CLI `doctor --probe`); non-routable providers report `skipped` with a stable skip code rather than a network failure. `recipe_detail` controls workflow-recipe verbosity (`none`/`summary`/`full`). Patches the `local_workspace` descriptor from `local_backend` presence. The only tool without `response_detail`.

**`repo_search`** — codebase investigation across source, docs, issues, releases, and package metadata. `goal` selects the workflow (with `debug` enabling exact-error behavior); `sources` overrides the goal defaults; repo locator/package fields scope the search; `include_local` mixes in the workspace backend. When `[search].mode = "off"` only the local-only path is allowed. Returns grouped bundles (docs, code, issues, releases) with resolved hints, telemetry, package resolution, and security context. Legacy `profile`/`mode`/`workflow`/`include_*`/`providers`/`timeout_ms` are accepted but hidden (see canonical section below).

**`repo_fetch`** — known-file inspection by structured `locator` (`owner`/`repo`/`path`/`ref`) with `line_start`/`line_end` spans and `symbol` expansion. `workspace://` locators resolve through the local backend and bypass the fetch policy (no network I/O). Remote locators resolve to a canonical forge raw URL (Gitea/Forgejo permalink preferred) fetched through the shared `FetchClient` under fetch policy. Control chars are stripped per line, injection markers scanned when `sanitize_output` is set, and trust markers recorded.

**`repo_map`** — orientation before search: repository tree without file contents. `owner`/`repo` plus optional `ref`/`path`. Supported forge hosts go through `forge_adapter::fetch_tree` under a bounded endpoint policy with timeout fallback; anything else (or timeout) returns `repo_mapper::build_fallback_response` with explicit `mode_reason` telemetry. Workspace-scoped requests use the local backend under the local-only policy path. Never a substitute for search or fetch.

**`security_search`** — advisory retrieval with applicability context. `query` plus optional `identifiers` (CVE/GHSA/OSV), `package`/`ecosystem` scoping, `goal` (defaults to `security_review`), and `include` toggles for KEV, exploit context, defensive guidance, and vendor advisories. Runs the adapter security path over advisory engines (OSV, GitHub Advisory, NVD, CISA KEV via `kev_client`, RustSec). Returns normalized vulnerability metadata (severity, affected versions, fixes) grouped with applicability context. `goal` uses the same vocabulary as `repo_search` via `parse_security_goal`.

**`research_search`** — grouped multi-source evidence for complex architectural/comparative questions. `query` plus `depth` (`quick`/`standard`/`deep`), `domain`, `goal` (mapped through `parse_research_goal` onto `ResearchWorkflow`), and `include` toggles. Runs `MetadataSearchAdapter::research_search` with the research planner (claims/gaps/conflicts, depth control, semantic roles). Returns evidence grouped by quality and class with subqueries, source-quality, and workflow-context metadata. Does not synthesize answers and does not fetch pages.

**`build_evidence_bundle`** — pure local packaging of already-selected `sources` and `fetches` into a portable `EvidenceBundle` with trust summary and gaps. Synchronous (`run_build_evidence_bundle` takes only `EvidenceBundleArgs`, no `ServerState`). No adapter, no fetch, no network, no summarization. `response_detail` is accepted but identity-preserving: all three modes return the identical canonical bundle.

---

## Per-Tool Module Pattern and the `map_tool_result` Seam

Every tool module follows one pipeline:

1. **Validate** — parse and bound args (`WebSearchRequest::validate`, URL scheme checks, `max_chars`/`timeout_ms` > 0, locator shape, canonical enum parsing). Uninterpretable invocation shapes become `ToolError::InvalidRequest`.
2. **Policy** — `live_allowed()` for search-path tools, `fetch_allowed()` for fetch-path tools. Denials name the tool and the config knob to flip.
3. **Adapter/fetch** — call `MetadataSearchAdapter` (`web_search`, `repo_search`, `research_search`, security path) or the shared `FetchClient`/forge/local path. Provider failures stay soft (warnings + `providers_failed`); total failure of every selected provider is an internal error, not a semantic one.
4. **Sanitize** — untrusted text flows through `core::sanitize` tiers; trust markers, injection-hit warnings, and the generic-context-untrusted warning are attached here.
5. **JSON via one seam** — return `Result<serde_json::Value, ToolError>`; `server.rs` maps it with `map_tool_result()` (`tools/common.rs`). No handler repeats the conversion.

`map_tool_result` is the single result/error boundary:

- `Ok(value)` → structured success (`CallToolResult::structured`: native `structuredContent` plus a text JSON fallback for older clients).
- `ToolError::Validation` (legacy) and `ToolError::Execution { code, message, data, repair }` → repairable `isError: true` tool result with a stable `code` and bounded `RepairHint { field, accepted[<=20], suggested_value }`. Stable codes: `invalid_semantic_value`, `conflicting_arguments`, `capability_unavailable`, `provider_unavailable`, `policy_denied`, `budget_invalid`, `locator_invalid`, `manual_interaction_required`, `upstream_failed`. Legacy `Validation` strings are code-inferred so old call sites stay repairable without a flag day.
- `ToolError::InvalidRequest` → JSON-RPC `invalid_params`. Reserved for shapes that cannot be interpreted at all.
- `ToolError::Internal` → JSON-RPC `internal_error` without stack traces. Reserved for server faults.

rmcp input-schema decode failures surface as `isError` tool errors in the pinned release; only true protocol failures (malformed JSON-RPC, missing negotiation headers) remain JSON-RPC errors on the wire.

### Shared helpers (`tools/common.rs`)

Beyond the error seam, `common.rs` owns the helpers every tool module reuses:

- Strict enum parsing (`parse_strict_enum_arg`, `parse_strict_freshness`, `parse_code_host_arg`, `parse_symbol_kind_arg`) — unknown values become legacy `Validation` errors carrying the accepted list, which the seam code-infers to `invalid_semantic_value`.
- Selection-stage ledger merging (`skip_reason_to_attempt`, `merge_selection_stage_attempts`) — provider-selection skips from `resolve_provider_routing` are converted into `RetrievalAttempt` entries and folded into the response `retrieval_summary`, so capability/policy skips are explicit attempts rather than silent omissions.
- Fetch-side constructors (`cached_document_response`, `derived_cache_entry`) — rebuild typed `WebFetchResponse` values from raw/derived cache entries with correct trust markers, truncation state, and transport metadata.
- Browser helpers (feature-gated: browser fetch helper, `browser_manual_interaction_error`, `browser_profile_requires_attention_error`, `browser_unavailable_error`, `browser_deadline_exceeded_error`) — profile-scoped lifecycle handling and the manual-interaction error vocabulary, reusing the stable `ToolErrorCode` set with preserved `data` payloads.

---

## Canonical Selectors vs Hidden Advanced Fields (`tools/canonical.rs`)

Ordinary agent schemas stay slim. The canonical vocabulary is:

- `goal`: `understand`, `architecture`, `debug`, `migration`, `security`, `dependency`, `performance`, `compare`, `pre_change`, `post_change` (see `REPO_GOAL_VALUES`).
- `sources` (`repo_search`): `code`, `docs`, `registry`, `issues`, `pull_requests`, `releases`, `examples`, `changelog`, `migration_guides`, `security` (see `REPO_SOURCE_VALUES`).
- `include` (`research_search`): `counterpoints`, `primary_sources`, `recent_discussion`, `security_considerations`; (`security_search`): `kev`, `exploit_context`, `defensive_guidance`, `vendor_advisories`.

`canonical.rs` resolves these into typed planner inputs: `resolve_repo_semantics()` (goal → `WorkflowKind` + implied `SearchProfile`/`RepoSearchMode`, with `debug` implying `ExactError`), `resolve_repo_sources()` (tokens → `include_*` booleans), `resolve_research_workflow()` / `resolve_research_includes()`, `resolve_security_workflow()` / `resolve_security_includes()`.

Advanced fields (`providers`, `timeout_ms`, `profile`, `mode`, `workflow`, legacy `include_*`) remain accepted at runtime but are hidden from `tools/list` input schemas (`#[schemars(skip)]`). When both forms are present they must agree; disagreement is a repairable `ConflictingArguments`/`InvalidSemanticValue` error with a bounded repair hint, never a silent override. Unknown tokens are `InvalidSemanticValue` with the accepted list echoed.

---

## Semantic Contract (`tool_contract.rs`)

One `ToolContract` per stable tool is the authoritative source for selection semantics:

- `name` — canonical `tools/list` name.
- `description` — concise selection-oriented text (budget: `MAX_TOOL_DESCRIPTION_LEN` = 300 bytes), applied verbatim to `tools/list` by `apply_contract_metadata()`.
- `purpose` / `use_when` / `not_for` — one-phrase capability, selection trigger, explicit non-goal.
- `domain` — `web`, `fetch`, `repository`, `security`, `research`, `evidence`, `diagnostic`.
- `disclosure` — advisory host hint only, never policy-enforcing: `core` (`web_search`, `web_fetch`, `repo_search`), `deferred` (specialists), `diagnostic` (`provider_status`).
- `read_only` / `open_world` — static MCP annotation hints. All 10 tools are read-only; all except `provider_status` and `build_evidence_bundle` are open-world. `provider_status` reports `open_world_hint=false` even though `probe=true` performs bounded live checks, because annotations never vary by arguments.
- `keywords` — discovery terms (e.g. `cve`, `ghsa`, `kev` for `security_search`; `batch`, `fan-out` for `batch_fetch`).
- `aliases` — discovery-only index names (e.g. `websearch`, code-search alias, `deep_research`). Never wire names; never valid in `next_actions`.
- `related_tools` / `next_tools` — semantic neighborhood and likely follow-ups (e.g. `web_search` → `web_fetch`, `batch_fetch`; `repo_search` → `repo_fetch`, `repo_map`).
- `discovery_text()` — concatenates name, domain, disclosure, purpose, guidance, keywords, aliases, and both graphs into one indexable string for host BM25/keyword catalogs. Hosts index this text without returning full schemas in ordinary discovery results.
- `is_known_tool()` — the harness filter for `next_actions` targets. Unknown or malicious names never widen execution authority.

The registry is tested sorted, unique, and exactly 10 entries.

---

## `tools/list`: Name-Sorted Advertisement + FNV Fingerprint

`apply_contract_metadata()` runs once at construction: for each router-generated tool it overwrites `description` and `annotations` from the contract registry, attaches the compact `outputSchema` from `output_schema.rs`, then sorts by name so `tools/list` is deterministic and cacheable across transports and protocol eras.

`contract_fingerprint()` hashes canonical names, descriptions, annotations, serialized input/output schemas, and canonical discovery metadata (domain, disclosure, purpose, use-when/not-for, keywords, aliases, related/next) with FNV-1a, formatted as 16 hex chars. It changes only when the public contract changes. Clients cache by fingerprint plus server version, never by tool count; hydration state is applied after retrieving a cached base surface.

Output schemas (`output_schema.rs`) are permissive stable envelopes (`type: object`, `additionalProperties: true`, minimal `required` sets, named top-level keys per tool), each under the 1200-byte budget. Full `$defs`-expanded typed schemas were measured at ~285 KiB for four search definitions alone and rejected as a context-budget regression; the compact envelopes preserve native `structuredContent` plus advertised `outputSchema` without that cost. Every stable tool has one (enforced by test).

| Tool | Envelope keys advertised |
|------|-------------------------|
| `batch_fetch` | `results` (required), `fetched`, `failed`, `warnings`, `structured_warnings`, `telemetry` |
| `build_evidence_bundle` | `sources`, `fetches`, `warnings` |
| `provider_status` | `providers` (required), `mode`, `server_capabilities`, `probe` |
| `repo_fetch` | `text`, `warnings`, `structured_warnings`, `trust_markers` |
| `repo_map` | `warnings`, `structured_warnings`, `structure_truncated` |
| `repo_search` | `results`, `warnings`, `structured_warnings`, `next_actions`, `retrieval_summary` |
| `research_search` | `results`, `warnings`, `structured_warnings`, `next_actions` |
| `security_search` | `results`, `warnings`, `structured_warnings`, `next_actions` |
| `web_fetch` | `url`, `final_url`, `status`, `fetched`, `truncated` (required), `text`, `warnings`, `structured_warnings` |
| `web_search` | `query`, `results` (required), `providers_queried`, `warnings`, `structured_warnings`, `next_actions` |

Tool behavior modules carry unit coverage in-file plus the shared `tools/tests.rs` (bundle validation, locator edge cases); protocol-level conformance lives in the behavioral suites (`mcp_tools`, `mcp_2026_protocol`, `mcp_projection`, `mcp_http`).

---

## Projection: `ResponseDetail` and Identity-Preserving Bundles (`projection.rs`)

`ResponseDetail` (`Compact` / `Standard` / `Diagnostic`, default `Diagnostic`) is parsed case-insensitively (`parse_raw`) with a stable error for unknown values. Canonical typed responses are built first, then projected to JSON at the MCP boundary via the central `project(tool, value, detail)` function — structured values are never truncated before host capture.

- `diagnostic` — full observability payload, passthrough with no added keys.
- `standard` — specialist research/security default surface. Keeps full `retrieval_summary`, `conflict_metadata`, `workflow_coverage`, `evidence_role_summary`, `capability_enforcement`, and fetch/cache metadata; summarizes only `routing_decision` (→ `routing_summary`) and `telemetry.routing_decision`.
- `compact` — ordinary agent operation. Trims excerpts to 1 per card, replaces `routing_decision` with `routing_summary`, replaces `retrieval_summary`/`conflict_metadata` with `retrieval_status`/`conflict_indicator`, drops `workflow_coverage`/`telemetry`/`document`/`links`, strips per-vulnerability `references`/`details`, caps `repo_map` entries at 50. Every projection stamps `response_detail` plus an explicit truncation marker (`projection_excerpts_trimmed`, `projection_entries_truncated`, `links_omitted_in_compact`).

The suite asserts `compact < standard < diagnostic` and that compaction never erases failure-vs-absence state, trust/injection markers, conflict/applicability semantics, truncation evidence, or stable IDs.

Per-tool reducers (`projection.rs`):

| Tool | `compact` drops/summarizes | `standard` keeps over `compact` |
|------|---------------------------|---------------------------------|
| `web_search` | excerpts → 1/card; `routing_decision` → `routing_summary`; `retrieval_summary` → `retrieval_status`; `conflict_metadata` → indicator; drops `workflow_coverage`, `evidence_role_summary` | full `retrieval_summary`, `conflict_metadata`, `workflow_coverage`, `evidence_role_summary`, `capability_enforcement`; summarizes only `routing_decision` |
| `repo_search` | group excerpts → 1/card; drops `resolved_hints`, `telemetry`, `package_resolution`, `security_context`, `error_context`, `workflow_coverage`, `evidence_role_summary` | full telemetry (minus `routing_decision`), `retrieval_summary`, `conflict_metadata`, coverage/role summaries |
| `research_search` | group excerpts → 1/card; drops `subqueries`, `telemetry`, `workflow_context`, `source_quality`, coverage/role summaries; merges both conflict channels into the indicator | same full set as `repo_search` |
| `security_search` | strips per-vulnerability `references`/`details`; routing/capability/retrieval/conflict summarized as in `web_search`; drops `workflow_coverage`, `evidence_role_summary`, `security_evidence_summary` | full `retrieval_summary`, `conflict_metadata`, coverage/role summaries, `capability_enforcement` |
| `web_fetch` | drops `raw_text*`, `raw_body`, `response_headers`, `fetch_transform`, `document`, `description`, `links` (keeping `links_seen` + `links_omitted_in_compact`), `browser_profile`, backoff fields | full document, links, fetch/cache metadata |
| `repo_fetch` | drops `document`, `lines`, `code_context`, `fetched_url`, `raw_url` | full lines, code context, document |
| `repo_map` | entries/`root_entries` capped at 50 (`projection_entries_truncated`); drops `telemetry`, `language_distribution`, `test_relationships`, `build_configs` | full entries; drops only `telemetry` |
| `batch_fetch` | applies the web/repo item reducers per payload `item_type` | same, with `standard`-level item payloads |
| `build_evidence_bundle` | identity-preserving: unchanged in all modes | unchanged |
| `provider_status` | no projection (diagnostic-only tool) | n/a |

`build_evidence_bundle` accepts `response_detail` but returns the canonical bundle unchanged in all modes, so handoff identity never diverges. Hosts store full `structuredContent` internally and inject only the selected projection into model-visible context.

---

## Policy (`policy.rs`)

`Policy` is a two-variant allow/deny switch; the multi-state concept lives one level up in `core::config::Mode` (`Live` / `Off`, default `Live`):

- `live_allowed(mode)` — gates the search path (`web_search`, `repo_search`, `security_search`, `research_search`, `repo_map` unless workspace-local). `Mode::Off` denies; the tool returns a repairable `policy_denied`-class message naming `[search].mode = "live"`.
- `fetch_allowed(fetch_enabled)` — gates the fetch path (`web_fetch`, `batch_fetch`, remote `repo_fetch`). Disabled when `[fetch].enabled = false`; the tool returns a message naming `[fetch].enabled = true`.
Policy checks run before any adapter, cache, or network work, so denied calls are cheap and side-effect-free.

`workspace://` local `repo_fetch` bypasses the fetch policy because it performs no network I/O. `repo_map` and `repo_search` take the local-only path under `Mode::Off` when a workspace backend is present, so on-disk investigation keeps working with live search disabled. Denial constructors (`policy_message()`, `live_search_denied_message()`, `web_search_denied_message()`, `web_fetch_denied_message()`) produce stable human-readable strings that tests pin.

---

## Further Reading

- [Machine-Readable Response Contract →](codegg-contract.md) — trust markers, warnings taxonomy, deterministic IDs, `next_actions`/`suggested_fetches` discipline, schema evolution rules.
- [Evidence & Workflow →](evidence-workflow.md) — evidence bundles, role taxonomy, workflow coverage, conflict detection, retrieval ledger behind the search responses.
- [Overview →](overview.md) | [Metasearch Adapter →](meta.md) | [HTTP Fetch →](fetch.md) | [CLI Commands →](commands.md)

---

## Conformance Gates

The surface described here is pinned by behavioral suites, not prose:

- `tests/docs_tool_names.rs` derives the documented tool inventory from `src/mcp/server.rs` — prose inventing a tool-like name fails the gate.
- `tool_contract.rs` unit tests pin alphabetical order, uniqueness, exactly 10 entries, and the 300-byte description budget.
- `output_schema.rs` unit tests pin one schema per stable tool, object type, and the 1200-byte budget.
- `tests/mcp_projection.rs` pins `compact < standard < diagnostic` and the identity/trust/conflict preservation rules on fixtures.
- `tests/mcp_http.rs` covers legacy initialize/tools-list/call plus modern discover/list/call, structured success, repairable errors, decode failures, and deterministic-list fixtures over the HTTP transport.
- `tests/mcp_2026_protocol.rs` pins native `structuredContent` + advertised `outputSchema` + representative-payload conformance.
- The tool-surface evaluation gate caps `EGGSEARCH_INSTRUCTIONS` (current 1614 bytes, max 2000); see `tests/fixtures/tool_surface/README.md` for the definition-byte baseline and fingerprint.
