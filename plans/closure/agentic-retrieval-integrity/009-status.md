# M009 Long-Running Progress and Pagination — Closure Status

Status: closed

Source plan: `plans/archive/implementation/agentic-retrieval-integrity/009-long-running-progress-and-pagination.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Planning baseline: `d751bd7471a647e651ee41b8e51199c66d04d01a`

Implementation candidate and closure commit: the commit that carries this record. Implementation,
closure evidence, roadmap status, and the registry row all moved in that single commit on top of the
planning baseline above.

## Work package 0 — capability confirmation (recorded finding)

Verified against the vendored crate at
`~/.cargo/registry/src/index.crates.io-*/rmcp-3.9.0/`, not assumed:

- `src/model/task.rs:14` — `TASKS_EXTENSION_ID = "io.modelcontextprotocol/tasks"`, with `Task`,
  `TaskStatus`, `TaskPayload`, `DetailedTask`, `CreateTaskResult`, `GetTaskResult`,
  `TaskAckResult`.
- `src/task_manager.rs:300` — `pub struct TaskManager` with `spawn`, `get_task`, `update_task`,
  `cancel_task`, `running_task_count`, and a `TaskContext` exposing cooperative cancellation.
- `src/handler/server.rs:666,678,687` — `ServerHandler::get_task`/`update_task`/`cancel_task`
  default to `method_not_found`, with **no bridge to `TaskManager`**; the server must write one.
- `src/model/mrtr.rs:105` — `CallToolResponse::Task(CreateTaskResult)`.
- `src/model/capabilities.rs:448` — `ServerCapabilitiesBuilder::enable_tasks`.
- `src/service/server.rs:947` — `notify_progress` on `Peer<RoleServer>`; `src/model.rs:1797`
  `ProgressNotificationParam`; `src/model/meta.rs:414` `with_progress_token`.
- `src/service/server.rs:241` — the server refuses to hand a task to a client that did not declare
  the extension.

So the roadmap's blocking question resolves to "supported", and the extension is a real gate rather
than a protocol label. Because Tasks requires client capability negotiation, the layered design the
plan described is what shipped: always-on cursor, client-requested progress, capability-gated Tasks.

## Cursor

`src/mcp/cursor.rs`. Canonical JSON, base64url without padding, `e1.` prefix, transported string
capped at **512 bytes**:

```json
{"v":1,"t":"web_fetch","k":"<16 hex FNV-1a plan key>","p":{"ord":3,"id":"..."},"d":{"timeout_ms":8000}}
```

`k` is built from `src/core/identity.rs` primitives (`FnvHasher`, `write_entity_prefix`, `write_str`,
`write_usize`, `canonicalize_url`) over a fixed field order, with `canonicalize_url(final_url)`
included for `web_fetch`.

Decode classification:

| Condition | Result |
|---|---|
| oversize, bad base64/padding, non-canonical JSON, unknown field, `v != 1`, malformed `k`/`id` | `budget_invalid` |
| wrong tool or plan key | `cursor_mismatch` |
| position no longer present | `cursor_status: "stale"`, first page, no `next_cursor` |

**Zero server-side cursor state.** The cursor is a replay token: the server re-runs the same bounded
pipeline and resumes after `p.id`. There is no cache to forge, evict, or saturate.

Deadline: `d.timeout_ms` records the first call's budget; a resume uses
`min(recorded, requested_or_config)` and installs it as both the client timeout and the call
deadline. A cursor never widens the original budget.

Wired at: `web_fetch` (chunk ordinal), `batch_fetch` (global item index), `research_search`
(canonical group order).

## Progress

`src/mcp/progress.rs`. Emitted only when `_meta.progressToken` is present. Fixed phases: `plan`,
`dispatch n/m`, `extract`, `merge`, `group`, `project`. At most one notification per 250 ms, at most
64 per call, at most 4 KiB total, each message bounded ASCII of 64 bytes carrying counts only. Sends
are spawned so progress can never delay the response, and progress never changes the final payload.

## Tasks

`src/mcp/tasks.rs` plus `ServerHandler::get_task`/`update_task`/`cancel_task` overrides forwarding to
`rmcp::task_manager::TaskManager`, returning `method_not_found` when absent, and advertising the
capability via `enable_tasks()` only when configured and the client declared it.

Eligible calls: `research_search` with `depth = "deep"`, `batch_fetch` with more than eight items.
At `max_concurrent` the call silently takes the synchronous path — never an error. Cancellation is
cooperative through the task context. Task ids are transport state and never enter evidence identity.

## Configuration

`[mcp.tasks]` in `src/core/config/mcp.rs`, validated through `AppConfig::validate`:

| Field | Default | Bounds |
|---|---:|---|
| `enabled` | `false` | — |
| `ttl_ms` | `300000` | `1000..=3600000` |
| `max_concurrent` | `4` | `1..=16` |
| `poll_interval_ms` | `1000` | `250..=10000` |

`McpSchemaProfile` and `McpSection` are byte-for-byte unchanged otherwise.

## Verification

- `cargo fmt --check`: clean.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: clean after the parent added
  `#[allow(clippy::too_many_arguments)]` to two Anubis functions this milestone did not touch.
- `cargo check --locked --no-default-features`: passed.
- `mcp_tools` 19, `mcp_projection` 31, `mcp_2026_protocol` 14, `config_validation` 91,
  `static_guards` 61, `mcp_schema_slimming` 20, `mcp_tool_contract` 19, `property_cursor` 5,
  `web_fetch_integration` 85, `research_workflow` 133, `batch_fetch_retrieval` 13, lib 3,750.
- Full `cargo test --locked --all-features`: **6,177 collected, 27 ignored, 6,150 passed, 0 failed**
  (exit 0).
- `./packaging/check-repo-hygiene.sh` and `python3 packaging/check-planning-consistency.py`: pass.

## Known limitations (recorded, not hidden)

- `src/fetch/**` was out of scope for this milestone, so extraction still stops at `max_chars`. A
  `web_fetch` page therefore delivers whole `document.chunks`, which paginate only when the chunk
  join exceeds `max_chars` — typically two pages. Exhaustiveness holds at chunk granularity, and
  `focus` is computed over the whole document rather than being page-sliced.
- `research_search` widens its internal `max_groups` to 14 only when the caller passed a smaller
  value; aggregate fields (`claims`, `conflicts`, `suggested_fetches`, `telemetry`) are computed
  over that wide set on paginated calls.
- Byte identity is exact for single-page responses. A paginated response is today's payload plus
  `next_cursor` (and `cursor_status` on resume), which is additive by design.

## Residual risk

- A client that caches a `next_cursor` across a config or provider change gets `cursor_mismatch`
  rather than a silently different page, which is the intended failure.
- Task records are process-local and die with the server; a restarted server cannot resume a task.
  The cursor path is the durable option.
- Progress is best-effort by construction: under load it is dropped rather than delaying a response,
  so a client must not treat a missing notification as completion.
