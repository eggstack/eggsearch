# mcp Module Deep Dive

**Path:** `src/mcp/`
**Purpose:** MCP server over stdio (rmcp), 10 tool definitions, shared server state, policy enforcement.

---

## Module Structure

| File | Responsibility |
|------|----------------|
| `server.rs` | `EggsearchServer` — `rmcp` server with `tool_router` proc macros. 10 `#[tool]` handlers. Exposes `tool_definitions()` for capability discovery |
| `state.rs` | `ServerState` — shared state (config, adapter, optional fetch_client, kev_client, local_backend, local_inventory_cache) |
| `tools.rs` | Tool argument structs and `run_*` implementations (~3700 lines). All 10 tools |
| `policy.rs` | `Policy` enum (Allow/Deny), mode-based gating. Helper functions: `live_allowed()`, `fetch_allowed()`, `policy_message()`, `live_search_denied_message()`, `web_fetch_denied_message()` |

---

## MCP Server Architecture

```
EggsearchServer
  ├── Arc<ServerState>
  │   ├── Arc<AppConfig>
  │   ├── Arc<MetadataSearchAdapter>
  │   ├── Option<Arc<FetchClient>>  (None when [fetch].enabled = false)
  │   ├── Arc<KevClient> (CISA KEV catalog)
  │   ├── Option<Arc<LocalWorkspaceBackend>>
  │   └── Arc<Mutex<Option<LocalInventoryCache>>>  (30s TTL)
  ├── tool_router (rmcp proc macros)
  └── 10 #[tool] handlers
```

### Transport

- **stdio only** — No HTTP, no WebSocket
- **rmcp crate** — MCP protocol implementation
- **Server instructions** — `EGGSEARCH_INSTRUCTIONS` constant in `server.rs`

---

## Tool Definitions (10)

### Search Tools

| Tool | Handler | Purpose |
|------|---------|---------|
| `web_search` | `run_web_search` | Live metasearch over configured providers |
| `repo_search` | `run_repo_search` | Structured repository evidence discovery |
| `security_search` | `run_security_search` | Security vulnerability and advisory search |
| `research_search` | `run_research_search` | Research-oriented multi-source evidence discovery |

### Fetch Tools

| Tool | Handler | Purpose |
|------|---------|---------|
| `web_fetch` | `run_web_fetch` | Bounded extraction of one HTTP(S) URL |
| `batch_fetch` | `run_batch_fetch` | Batch fetch over URLs or repo locators |
| `repo_fetch` | `run_repo_fetch` | Structured repository file fetch by locator |
| `repo_map` | `run_repo_map` | Repository structure discovery |

### Utility Tools

| Tool | Handler | Purpose |
|------|---------|---------|
| `provider_status` | `run_provider_status` | Diagnostic provider configuration report |
| `build_evidence_bundle` | `run_build_evidence_bundle` | Package selected evidence into a portable container |

---

## Tool Input/Output Patterns

### Search Tools

```
Input: { query, max_results?, profile?, freshness?, safe_search? }
Output: {
    results: SourceCard[],
    warnings: AgentWarning[],
    suggested_fetches: SuggestedFetch[],
    next_actions: AgentNextAction[],
    quality: SearchUncertaintySummary?,
    grouping: GroupQualitySummary?,
    ...
}
```

### Fetch Tools

```
Input: { url } or { locator } or { items[] }
Output: {
    document: FetchDocument,
    trust: FetchTrust,
    warnings: AgentWarning[],
    ...
}
```

---

## Policy Enforcement

The `Policy` enum gates tool execution:

```rust
enum Policy {
    Allow,
    Deny,
}
```

Mode-based rules (implemented in `src/mcp/policy.rs`):
- `Mode::Live` — Live metasearch tools allowed; fetch tools follow `[fetch].enabled`.
- `Mode::Off` — All live-search tools denied; fetch and local-workspace paths still follow their own gates.

### Per-Tool Policy Checks

| Tool | Policy Check |
|------|-------------|
| `web_search`, `repo_search`, `security_search`, `research_search` | `live_allowed()` |
| `web_fetch`, `repo_fetch`, `batch_fetch` | `fetch_allowed()` |
| `repo_map` | `live_allowed()` (with local-only path bypass) |
| `provider_status`, `build_evidence_bundle` | No policy check — always allowed |

Policy is checked before any adapter or fetch client call.

---

## Server State

`ServerState` holds all shared resources:

```rust
struct ServerState {
    config: Arc<AppConfig>,
    adapter: Arc<MetadataSearchAdapter>,
    fetch_client: Option<Arc<FetchClient>>,  // None when [fetch].enabled = false
    kev_client: Arc<KevClient>,
    local_backend: Option<Arc<LocalWorkspaceBackend>>,
    local_inventory_cache: Arc<Mutex<Option<LocalInventoryCache>>>,  // 30s TTL
}
```

Shared resource fields are `Arc`-wrapped (some optionally). `ServerState` is constructed via `ServerState::build()` (production, validates config) or `ServerState::with_adapter()` (tests/custom adapters). Helper methods: `fetch_client()`, `local_inventory()`, `invalidate_local_inventory_cache()`.

---

## Tool Implementation Pattern

Each tool follows the same pattern:

1. **Deserialize** tool arguments
2. **Policy check** — Is this tool allowed?
3. **Validate** input
4. **Call** adapter or fetch client
5. **Process** response (sanitize, group, rank)
6. **Build** structured output with deterministic IDs
7. **Return** `Result<serde_json::Value, ToolError>`

---

## Error Handling

Tool errors use `ToolError`:

```rust
enum ToolError {
    Validation(String),
    Internal(String),
}
```

Tools never panic. All errors are structured and machine-readable. Note: `build_evidence_bundle` is the only sync tool (no `state` parameter, pure logic).

---

**Back to:** [overview.md](overview.md)
