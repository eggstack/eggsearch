# Phase 6: Bounded Batch Fetch

## Purpose

Add a bounded batch-fetch capability that lets coding agents fetch several known documents or source spans in one MCP call without losing the strict budget, trust, and provenance guarantees established by `web_fetch` and `repo_fetch`.

This phase is intended for codegg workflows where `repo_search` returns multiple suggested fetches and the agent needs to inspect a small set of source files, docs, release notes, or issue pages before deciding what to edit. The goal is not crawling. The goal is controlled fan-out over explicit URLs or structured repo locators.

## Non-goals

Do not implement recursive crawling, automatic link following, repository cloning, workspace indexing changes, or background prefetching. Do not let a batch fetch silently exceed configured fetch caps. Do not merge fetched content into one ambiguous blob.

## Proposed tool surface

Add a new MCP tool:

```text
batch_fetch
```

The tool accepts a list of explicit items. Each item is either a web URL fetch or a structured repo fetch request.

Recommended request shape:

```rust
pub struct BatchFetchArgs {
    pub items: Vec<BatchFetchItem>,
    pub max_items: Option<usize>,
    pub max_chars_per_item: Option<usize>,
    pub max_total_chars: Option<usize>,
    pub timeout_ms: Option<u64>,
    pub continue_on_error: Option<bool>,
}

#[serde(tag = "type", rename_all = "snake_case")]
pub enum BatchFetchItem {
    Web {
        url: String,
        extract_mode: Option<ExtractMode>,
        include_links: Option<bool>,
        max_chars: Option<usize>,
    },
    Repo {
        host: Option<String>,
        owner: String,
        repo: String,
        ref_name: Option<String>,
        commit_sha: Option<String>,
        path: String,
        line_start: Option<u32>,
        line_end: Option<u32>,
        context_before: Option<u32>,
        context_after: Option<u32>,
        max_chars: Option<usize>,
    },
}
```

Recommended response shape:

```rust
pub struct BatchFetchResponse {
    pub fetched: usize,
    pub failed: usize,
    pub truncated: bool,
    pub total_chars_returned: usize,
    pub results: Vec<BatchFetchResult>,
    pub warnings: Vec<String>,
}

pub struct BatchFetchResult {
    pub index: usize,
    pub item_type: BatchFetchItemType,
    pub ok: bool,
    pub response: Option<serde_json::Value>,
    pub error: Option<String>,
    pub chars_returned: usize,
    pub truncated: bool,
}
```

Keep the individual payloads as serialized `web_fetch` / `repo_fetch` responses if that minimizes duplication. The important property is that each result remains individually attributable, with its own URL/locator/trust markers/warnings.

## Budget policy

Add config defaults under `[fetch]`:

```toml
batch_max_items = 8
batch_max_items_cap = 20
batch_max_chars_per_item_default = 12000
batch_max_total_chars_default = 50000
batch_max_total_chars_cap = 120000
batch_concurrency = 4
```

Rules:

- Reject empty `items`.
- Reject item count above `batch_max_items_cap`.
- Clamp default requested item count to `batch_max_items` unless caller provides lower.
- Reject `max_total_chars` above cap.
- Enforce `max_chars_per_item` per item.
- Enforce `max_total_chars` across the serialized returned text payloads.
- If the total budget is exhausted, stop launching further fetches and return a warning.
- Do not allow one item to consume the entire batch unless caller explicitly requests high per-item budget within cap.

## Execution model

Use bounded concurrency. A small `FuturesUnordered` plus semaphore is sufficient.

Important sequencing constraints:

- Validate every item before launching any network/local fetch.
- Preserve input order in `results` even if execution is concurrent.
- If `continue_on_error = false`, cancel or stop scheduling remaining items after first failure.
- If `continue_on_error = true` or omitted, return partial results with per-item errors.
- Reuse existing `run_web_fetch` / `run_repo_fetch` internals if possible, but avoid recursive MCP serialization overhead if a lower-level helper is cleaner.

Do not share mutable fetch-client state in a way that bypasses timeout or budget controls.

## Trust and sanitization

Each item retains its own trust markers and trust label. The batch response itself should not invent a single trust label for all items.

Rules:

- External web and remote repo content remains `external_untrusted`.
- Workspace repo fetch remains `local_trusted`, with local marker scanning.
- Batch-level warnings should summarize only aggregate conditions such as total budget truncation, item count truncation, or partial failure.
- Do not concatenate all item text into one unframed field.

## Suggested fetch integration

Update `repo_search` response docs to say that agents can pass `suggested_fetches` into `batch_fetch`, but do not automatically call `batch_fetch` from `repo_search`.

Optionally add a helper field to `RepoSuggestedFetch` later if needed, but this phase should work with existing `url` and `structured_repo_fetch` fields.

## MCP registration

Register `batch_fetch` in:

- `src/mcp/server.rs`
- `src/mcp/tools.rs`
- provider/tool status output
- README tool list
- AGENTS.md operational guidance

Provider status should expose:

```json
"batch_fetch": {
  "enabled": true,
  "max_items_cap": 20,
  "max_total_chars_cap": 120000,
  "supports_web": true,
  "supports_repo": true,
  "preserves_item_trust": true
}
```

## Tests

Add unit and integration tests for:

- Empty batch rejected.
- Batch over item cap rejected.
- Per-item `max_chars` enforced.
- Total `max_total_chars` enforced.
- Result order matches input order under concurrent execution.
- Mixed web and repo items return separate responses.
- Workspace item retains `local_trusted` and marker scan behavior.
- Remote item retains `external_untrusted`.
- `continue_on_error = true` returns partial success.
- `continue_on_error = false` stops after first failure.
- `provider_status` advertises batch capability.

Use mock HTTP servers and local temp workspaces. Do not add tests that depend on live internet.

## Acceptance criteria

Phase 6 is complete when:

- `batch_fetch` is registered as an MCP tool.
- Batch fetch can fetch explicit web URLs and structured repo locators.
- All per-item and total budget limits are enforced.
- Results retain input order and per-item attribution.
- Trust markers and warnings remain item-specific.
- Partial failure semantics are deterministic.
- README and AGENTS document the non-crawling nature of the tool.
- `cargo fmt`, clippy, and tests pass.
