# MCP Server Deep Dive

**Location:** `src/mcp/` (5 files)
**Purpose:** MCP (Model Context Protocol) server exposing 10 stable tools for AI agents. Transport is stdio.

---

## Module Map

| File | Responsibility |
|------|---------------|
| `mod.rs` | Module declarations and re-exports |
| `server.rs` | `EggsearchServer` — rmcp `ServerHandler` impl, 10 `#[tool]` handlers, `EGGSEARCH_INSTRUCTIONS` |
| `tools.rs` | Tool implementations: validation, adapter calls, response formatting (~5600 lines) |
| `state.rs` | `ServerState` — shared state: config, adapter, fetch client, cache, etc. |
| `policy.rs` | `Policy` enum, `live_allowed()`, `fetch_allowed()`, policy denial messages |

---

## MCP Server (`server.rs`)

### EggsearchServer

Implements `rmcp::ServerHandler`:

```rust
struct EggsearchServer {
    state: ServerState,
}
```

### EGGSEARCH_INSTRUCTIONS

Constant containing server instructions for AI agents:
- Tool descriptions
- Usage patterns
- Trust model
- Evidence bundle guidance

### Tool Registration

Uses `rmcp` proc macros:

```rust
#[tool_router]
impl EggsearchServer {
    #[tool]
    async fn web_search(&self, args: WebSearchArgs) -> Result<Value>;
    // ... 9 more tools
}
```

---

## The 10 MCP Tools

### 1. `web_search`
**Purpose:** Live metasearch over configured upstream providers.

| Parameter | Type | Description |
|-----------|------|-------------|
| `query` | String | Search query (1-512 chars) |
| `max_results` | Option<usize> | Max results (1-50, default 10) |
| `providers` | Option<Vec<String>> | Specific provider IDs |
| `freshness` | Option<String> | Time filter: day, week, month, year |
| `safe_search` | Option<String> | Safe search: off, moderate, strict |

**Returns:** Array of `SourceCard` objects.

### 2. `web_fetch`
**Purpose:** Bounded extraction of one explicit HTTP(S) URL.

| Parameter | Type | Description |
|-----------|------|-------------|
| `url` | String | URL to fetch (must be https) |
| `max_chars` | Option<usize> | Max extracted chars (default 50000) |
| `extract_mode` | Option<String> | text, markdown, or metadata_only |

**Returns:** Extracted content with metadata.

### 3. `batch_fetch`
**Purpose:** Bounded batch fetch over explicit URLs or structured repo locators.

| Parameter | Type | Description |
|-----------|------|-------------|
| `items` | Vec<BatchFetchItem> | URLs or repo locators to fetch |
| `max_chars_per_item` | Option<usize> | Per-item char limit |

**Returns:** Array of fetch results.

### 4. `provider_status`
**Purpose:** Diagnostic report of configured providers and server capabilities.

**Returns:** Provider health, capabilities, configuration.

### 5. `repo_search`
**Purpose:** Structured repository evidence discovery with grouped result bundles.

| Parameter | Type | Description |
|-----------|------|-------------|
| `query` | String | Search query |
| `max_results` | Option<usize> | Max results |
| `profile` | Option<String> | Search profile: generic, coding, security, research |

**Returns:** Grouped results by category (docs, code, issues, releases).

### 6. `repo_fetch`
**Purpose:** Structured repository file fetch by locator with line ranges and symbols.

| Parameter | Type | Description |
|-----------|------|-------------|
| `locator` | RepoLocator | Repository location (owner/repo/path/ref) |
| `line_start` | Option<usize> | Start line |
| `line_end` | Option<usize> | End line |
| `symbol` | Option<String> | Symbol name to expand |

**Returns:** File content with context.

### 7. `repo_map`
**Purpose:** Bounded repository-structure discovery for coding agents.

| Parameter | Type | Description |
|-----------|------|-------------|
| `owner` | String | Repository owner |
| `repo` | String | Repository name |
| `ref` | Option<String> | Branch/tag/commit |
| `path` | Option<String> | Subdirectory path |

**Returns:** Repository tree with important files/directories highlighted.

### 8. `security_search`
**Purpose:** Security-oriented retrieval with normalized vulnerability metadata.

| Parameter | Type | Description |
|-----------|------|-------------|
| `query` | String | Security query |
| `identifiers` | Option<Vec<String>> | CVE, GHSA, OSV IDs |
| `package` | Option<String> | Package name |
| `ecosystem` | Option<String> | Package ecosystem |

**Returns:** Vulnerability metadata with severity, affected versions, fixes.

### 9. `research_search`
**Purpose:** Research-oriented multi-source evidence discovery with grouped bundles.

| Parameter | Type | Description |
|-----------|------|-------------|
| `query` | String | Research query |
| `depth` | Option<String> | Search depth: quick, standard, deep |
| `domain` | Option<String> | Research domain |

**Returns:** Evidence sources grouped by quality and class.

### 10. `build_evidence_bundle`
**Purpose:** Packages already-selected evidence into a portable container.

| Parameter | Type | Description |
|-----------|------|-------------|
| `sources` | Vec<EvidenceSourceInput> | Evidence sources to bundle |
| `fetched` | Option<Vec<EvidenceFetchInput>> | Fetched content to include |

**Returns:** `EvidenceBundle` with trust summary and gaps.

---

## ServerState (`state.rs`)

Shared state across all tool handlers:

```rust
struct ServerState {
    config: AppConfig,
    adapter: MetadataSearchAdapter,
    fetch_client: FetchClient,
    origin_controller: OriginController,
    fetch_cache: FetchCache,
    kev_client: Option<KevClient>,
    local_backend: Option<LocalWorkspaceBackend>,
    profile_manager: Option<ProfileManager>,
    browser_lifecycle: Option<BrowserLifecycle>,
}
```

---

## Policy (`policy.rs`)

Controls what operations are allowed:

```rust
enum Policy {
    Live,      // All operations allowed
    DryRun,    // No network requests
    Offline,   // Only cached/local data
}
```

### Policy Checks

```rust
fn live_allowed(policy: &Policy) -> bool;
fn fetch_allowed(policy: &Policy) -> bool;
```

---

## Tool Implementation Pattern (`tools.rs`)

Each tool follows this pattern:

1. **Parse args** — Validate input from JSON
2. **Check policy** — Ensure operation is allowed
3. **Build request** — Construct core request type
4. **Call adapter** — Delegate to `MetadataSearchAdapter`
5. **Format response** — Convert to `serde_json::Value`
6. **Return** — `Result<Value, ToolError>`

### ToolError Enum

```rust
enum ToolError {
    Validation(String),
    Provider(String),
    Fetch(String),
    Internal(String),
    PolicyDenied(String),
}
```

---

## Integration with Other Modules

```
MCP Server
  ├→ core::config::AppConfig          (configuration)
  ├→ core::*::Request/Response        (type definitions)
  ├→ meta::MetadataSearchAdapter      (search operations)
  ├→ fetch::FetchClient               (URL fetching)
  ├→ fetch::cache::FetchCache         (caching)
  ├→ fetch::origin::OriginController  (per-origin limits)
  └→ fetch::browser::*                (optional browser rendering)
```

---

## Security Considerations

- **Input validation** — All tool inputs validated against schema
- **URL validation** — Only HTTPS allowed for `web_fetch`
- **Policy enforcement** — Dry-run/offline modes block network
- **Bounded responses** — All responses have size limits
- **No secrets in logs** — Sensitive data redacted

---

[← Back to Overview](overview.md)
