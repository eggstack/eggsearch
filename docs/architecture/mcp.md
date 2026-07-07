# mcp Module Deep Dive

**Path:** `src/mcp/`
**Purpose:** MCP server over stdio (rmcp), 10 tool definitions, shared server state, policy enforcement.

---

## Module Structure

| File | Responsibility |
|------|----------------|
| `server.rs` | `EggsearchServer` — `rmcp` server with `tool_router` proc macros. 10 `#[tool]` handlers |
| `state.rs` | `ServerState` — shared state (config, adapter, fetch_client, kev_client, local_backend). All `Arc`-wrapped |
| `tools.rs` | Tool argument structs and `run_*` implementations (~3700 lines). All 10 tools |
| `policy.rs` | `Policy` enum (Allow/Deny), mode-based gating for search/fetch |

---

## MCP Server Architecture

```
EggsearchServer
  ├── ServerState (Arc-wrapped)
  │   ├── AppConfig
  │   ├── MetadataSearchAdapter
  │   ├── FetchClient
  │   ├── KevClient (CISA KEV catalog)
  │   └── LocalWorkspaceBackend (optional)
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
    Deny { reason: String },
}
```

Mode-based rules:
- `Mode::Live` — All tools allowed
- `Mode::DryRun` — Search tools allowed, fetch tools denied
- `Mode::Disabled` — All tools denied

Policy is checked before any adapter or fetch client call.

---

## Server State

`ServerState` holds all shared resources:

```rust
struct ServerState {
    config: Arc<AppConfig>,
    adapter: Arc<MetadataSearchAdapter>,
    fetch_client: Arc<FetchClient>,
    kev_client: Arc<KevClient>,
    local_backend: Option<Arc<LocalWorkspaceBackend>>,
}
```

All fields are `Arc`-wrapped for safe sharing across async tool handlers.

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
    InvalidInput(String),
    PolicyDenied(String),
    Internal(String),
}
```

Tools never panic. All errors are structured and machine-readable.

---

**Back to:** [overview.md](overview.md)
