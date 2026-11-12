# M009 — Long-Running Progress and Pagination

Status: ready
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M009
Primary class: infrastructure
Baseline: `d751bd7471a647e651ee41b8e51199c66d04d01a` (the worktree also carries uncommitted retrieval-default, scraper, and ADR-0009 changes from prior turns; re-read `git status` before starting and do not revert them)
References: ADR-0001; MCP SEP-2663 `io.modelcontextprotocol/tasks` as implemented by `rmcp` 3.9.0; `architecture/mcp.md`; `src/mcp/server.rs`; `src/mcp/tools/`; `src/mcp/projection.rs`; `src/mcp/output_schema.rs`; `src/fetch/client.rs`; `src/fetch/limits.rs`; `src/core/identity.rs`; `src/core/config.rs`
Dependencies: interface — the MCP 2026-07-28 Tasks extension (`io.modelcontextprotocol/tasks`, SEP-2663) through `rmcp` 3.9.0. Work package 0 confirms that contract from the vendored crate source and records the result before any other package is written. No hard dependency.

## Superseded in part

ADR-0011 and `020-offset-addressing-and-cursor-retirement.md` retire the opaque replay cursor for
`web_fetch`, `batch_fetch`, and `research_search`: reading is addressed by the original request
re-specified with `offset`/`limit`, guarded by a server-derived plan key, and `next_cursor` is
replaced by `next_offset`. The cursor was never released, so this is a replacement rather than a
deprecation.

What stands unchanged from this plan: token-gated bounded progress, the rule that a resumed call
never widens the original deadline, the capability-gated Tasks path, and the requirement that a
caller with no knowledge of the mechanism receives today's payload byte for byte. Its closure
record remains history and is not rewritten.

## Objective

Make the three long-running retrieval paths — `web_fetch` over a large document, `batch_fetch` over many items, and `research_search` over a deep plan — resumable and observable without adding a tool, renaming a field, changing a wire string, or widening any existing bound. A caller with no knowledge of this milestone receives today's complete-or-truncated response byte for byte.

## Non-goals

- No background daemon, no job queue across processes, no durable job store, no cancellation service, no inference.
- No persistence. Nothing about a cursor, a page position, or a task is written to disk, config, fixtures, or logs.
- No new MCP tool and no `tools/list` pagination. The inventory is ten tools and the frozen advertised list is already small; paging it would churn `contract_fingerprint()` for no benefit.
- No pagination for `repo_fetch` or `repo_search`. `repo_fetch` is already span-addressed by `line_start`/`line_end`; a second positional mechanism would be redundant and ambiguous.
- No `build_evidence_bundle` change. The bundle is identity-preserving in every projection mode and stays that way.
- No default-on Tasks extension. `[mcp.tasks].enabled` ships `false`.
- No resumption across server restarts. A cursor is replayable, but the underlying content may be gone; that is reported as stale, never guessed.
- No client-visible partial results in progress notifications.

## Invariants that cannot regress

- Ten MCP tools; stable FNV-1a IDs; sanitization tiers; trust framing; RRF tie ordering; capability-skip semantics; deadline bounds; Mode-Off remote denial.
- **Payload identity.** For any request, the response is byte-identical whether or not the caller supplied `_meta.progressToken`. Progress is side-channel only.
- **Legacy identity.** A request with no `cursor`, no progress token, and no declared tasks extension produces the current response, including the current `web_fetch` `truncated` shape and the current `batch_fetch` `max_total_chars` behaviour.
- **No new identity scheme.** Every content-derived value in the cursor is produced by existing `src/core/identity.rs` functions (`FnvHasher`, `write_str`, `write_usize`, `canonicalize_url`, `chunk_id`, `batch_fetch_id`, `source_id`).
- **The cursor is not a session id.** It carries no server-side handle, so there is no cursor table to saturate, evict, or poison.
- **The cursor cannot widen a bound.** A resumed call never exceeds the recorded timeout, never exceeds `[fetch].max_chars_cap`, never exceeds `max_total_chars` on that page, and never gains a provider the original request did not select. Resumption returns *more pages under the same per-page bounds*; it never returns a wider page.
- All untrusted I/O stays behind `read_bounded_body`/`read_bounded_response`/`read_with_budget`. No `reqwest`. No comments in shipped source.

## Expected production changes

1. **Cursor codec — `src/mcp/cursor.rs` (new).** A `RetrievalCursor` value, canonical JSON, base64url-unpadded, prefixed `e1.`:

   | Field | Type | Meaning |
   |---|---|---|
   | `v` | `u8` | Cursor schema version. `1`. Any other value is `cursor_invalid`. |
   | `t` | enum | `web_fetch` \| `batch_fetch` \| `research_search`. A cursor from another tool is `cursor_mismatch`. |
   | `k` | 16 hex | Plan key: FNV-1a 64 of the canonical request identity, hashed with `write_str`/`write_usize` in a fixed field order. For `web_fetch` this includes the canonicalised resolved `final_url` (`canonicalize_url`), so a redirect-target change is caught. |
   | `p` | object | Position, `{ "ord": u32, "id": string }`: the ordinal of the last delivered unit plus that unit's existing stable ID — `chunk_id` for `web_fetch`, `batch_fetch_id` for `batch_fetch`, `source_id` of the last delivered card for `research_search`. |
   | `d` | object | `{ "timeout_ms": u64 }`: the effective per-call budget of the first call, after clamping. |

   The transported string is capped at **512 bytes**. Decode rejects > 512 bytes, invalid base64, non-canonical JSON, unknown fields, and `v != 1` — all as `budget_invalid`, never a panic and never a silent default.
2. **Position resolution.** On resume the server re-runs the same bounded pipeline, locates `p.id` in the freshly recomputed ordered output, and continues after it. If `p.id` is absent the content changed and the cursor is **stale**: the call returns today's complete first page plus `cursor_status: "stale"` and no `next_cursor`. If `k` does not match the recomputed plan key the cursor is rejected as `cursor_mismatch`. A forged `p` can therefore only skip forward inside content the server itself just recomputed, and a forged `k` can only fail.
3. **Pagination wiring.**
   - `web_fetch` (`src/mcp/tools/web_fetch.rs`): slice the already-extracted content by chunk ordinal. Page size stays `max_chars`; `focus` remains a deterministic projection applied before slicing so the two features compose without a second traversal.
   - `batch_fetch` (`src/mcp/tools/batch_fetch.rs`): slice the caller's own `items` array by index. `max_total_chars` becomes per-page; the aggregate byte count of the full chain is still bounded because page count is bounded by `items.len()`.
   - `research_search` (`src/mcp/tools/research_search.rs`): group the **full** canonical order from `group_research_results` (`src/meta/research_grouping.rs:351`, `CANONICAL_GROUP_ORDER`), then slice by `(group, slot)` for this page. Per-page `max_groups`/`max_per_group` still apply inside the page; they must not truncate the set used to resolve `p.id`.
4. **Progress sink — `src/mcp/progress.rs` (new).** A `ProgressSink` built from `RequestContext<RoleServer>` that returns immediately when `context.meta.progress_token()` is `None`. Reports `notifications/progress` via the existing `Peer::notify_progress`. Phases are fixed vocabulary: `plan`, `dispatch <n>/<m>`, `extract`, `merge`, `group`, `project`. Rate limit: at most **one notification per 250 ms** per request, coalescing to the newest phase, and at most **64 notifications per call**. Each `message` is a bounded ASCII phase string (≤ 64 bytes) carrying counts only — never a URL, title, snippet, provider body, or untrusted text; total progress bytes ≤ 4 KiB per call.
5. **Task layer — `src/mcp/tasks.rs` (new wiring).** `ServerState` gains `task_manager: Option<Arc<rmcp::task_manager::TaskManager>>`, built from `[mcp.tasks]`. `EggsearchServer` overrides `get_task`, `update_task`, and `cancel_task` on `ServerHandler`, forwarding to the manager and returning `method_not_found` when it is `None`. `get_info()` adds `.enable_tasks()` only when enabled. Eligible calls — `research_search` with `depth = "deep"`, `batch_fetch` with more than 8 items — return `CallToolResponse::Task(CreateTaskResult::new(task))` gained from `TaskManager::spawn`; every other call returns synchronously. The spawned future polls `TaskContext::is_cancel_requested()` at each unit boundary so `tasks/cancel` is cooperative.
6. **Configuration.** New `[mcp.tasks]` table: `enabled` (default `false`), `ttl_ms` (default `300000`, validated `1000..=3600000`), `max_concurrent` (default `4`, validated `1..=16`), `poll_interval_ms` (default `1000`, validated `250..=10000`). Serde-defaulted so existing configs parse unchanged, with the same three-defaults-agree discipline used by `fetch.pdf_enabled`.
7. **Args and schemas.** Add an advertised optional `cursor: Option<String>` to `WebFetchArgs`, `BatchFetchArgs`, and `ResearchSearchArgs`, and `next_cursor`/`cursor_status` to the advertised output envelopes for those three tools in `src/mcp/output_schema.rs`. `cursor` is advertised rather than `#[schemars(skip)]`: a hidden field would strand the feature exactly as the roadmap warns for advertisement presets.
8. **Docs.** `architecture/mcp.md` (new pagination/progress subsection), `docs/config.md` (`[mcp.tasks]`), `docs/features.md`, `docs/test-inventory.md`, `architecture/testing.md` if a suite count changes, `CHANGELOG.md`.

## Storage, protocol, and compatibility effects

No schema-breaking change, no renamed field, no changed wire string, no new tool. `cursor` is an optional input, `next_cursor`/`cursor_status` are optional output keys; every existing required key and every existing `required` set in `output_schema.rs` is unchanged, and the per-schema 1,200-byte budget is re-asserted rather than raised. `notifications/progress` is core MCP gated on the client's `_meta.progressToken`, so progress needs no negotiation. The Tasks extension is negotiated by capability on both sides; when the client does not declare `io.modelcontextprotocol/tasks`, `rmcp` refuses to emit a task handle and the server returns the synchronous result, so the extension is invisible to every client that does not ask for it.

Server state is process-local and bounded:

- **Cursor state: none.** The cursor is a replay token; the server stores nothing keyed by it. This is the load-bearing property for "cannot be forged into an unbounded cache" — there is no cache to forge.
- **Task state:** bounded by `ttl_ms` (maximum 3,600,000 ms) and by `TaskManager`'s own opportunistic sweep on every `spawn`/`get_task`/`update_task`/`cancel_task`/`running_task_count`. `running_task_count() >= max_concurrent` does **not** produce an error: the call falls back to the synchronous path, so task saturation can never block retrieval.
- **Progress state:** one coalescing slot per in-flight request; dies with the request.

`TaskManager::spawn` mints a `uuid::Uuid::new_v4()` task id. That is a protocol session handle, not evidence identity: no task id appears in `results`, `source_id`, `fetch_id`, `chunk_id`, or `build_evidence_bundle`, and `src/core/identity.rs` semantics are untouched. A guard test pins that separation.

## Ordered work packages

1. **Work package 0 — capability confirmation (recorded finding, no production code).** Read the vendored crate at `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/rmcp-3.9.0/` and record what exists. The finding at baseline `d751bd7` is:

   | Capability | Evidence (relative to the crate root) |
   |---|---|
   | Extension id constant | `src/model/task.rs:14` — `TASKS_EXTENSION_ID = "io.modelcontextprotocol/tasks"` |
   | Task model | `src/model/task.rs` — `Task`, `TaskStatus`, `TaskPayload`, `DetailedTask`, `CreateTaskResult`, `GetTaskResult`, `TaskAckResult` |
   | Server runtime | `src/task_manager.rs` — `pub struct TaskManager` (`:300`), `spawn` (`:315`), `get_task` (`:402`), `update_task` (`:416`), `cancel_task` (`:453`), `running_task_count` (`:475`), `TaskContext` (`task_id`, `request_input`, `set_status_message`, `is_cancel_requested`, `cancelled`) |
   | Handler hooks | `src/handler/server.rs:666` `get_task`, `:678` `update_task`, `:687` `cancel_task`, all defaulting to `method_not_found` |
   | Task-bearing tool result | `src/model/mrtr.rs:105` `CallToolResponse::Task(CreateTaskResult)`; `src/handler/server/tool.rs:113` `IntoCallToolResult for CallToolResponse` |
   | Capability advertisement | `src/model/capabilities.rs:448` `ServerCapabilitiesBuilder::enable_tasks`; `:201`/`:249` `supports_tasks()` on client and server capabilities |
   | Progress send | `src/service/server.rs:947` `method!(peer_not notify_progress ProgressNotification(ProgressNotificationParam))` on `Peer<RoleServer>` |
   | Progress payload | `src/model.rs:1797` `ProgressNotificationParam { progress_token, progress, total, message, meta }` |
   | Progress token intake | `src/service.rs:1213` `RequestContext { meta: RequestMetaObject, peer, .. }`; `src/model/meta.rs:414` `with_progress_token`, `set_progress_token` |
   | Gate | `src/service/server.rs:241` refuses `CallToolResponse::Task` to a client that did not declare the extension; `src/handler/server.rs:31` `validate_tasks_capability` |

   **Outcome of this package is that table, appended to the closure record**, with the crate version and the exact files read. It is not a task-management proposal; it is evidence.

   Both branches are decided here, and only one is taken:
   - **If the Tasks APIs exist (the observed case):** use them, exactly as work package 5 specifies, gated on config plus client capability. Do not reimplement a task store.
   - **If they do not:** implement progress notifications only (which are core protocol and were verified independent of the extension) plus the cursor, and record in the closure record that `tasks/get` polling is unavailable through the pinned `rmcp`, that no parity is claimed, and that the cursor — not a task handle — is the resumption mechanism. Do not fake a task store, do not add a tool, and do not widen `ttl`-less state.

   The finding is re-run against the actually resolved crate at implementation time (`Cargo.lock` pins `rmcp` 3.9.0; `Cargo.toml` requests `3.2.0`). If the resolved version differs, the table is re-derived before work package 5 starts.
2. `src/mcp/cursor.rs`: encode/decode, 512-byte cap, plan-key derivation, position match, and stale/mismatch classification. Unit tests for each rejection path; `tests/property_cursor.rs` for round-trip, arbitrary-byte decode, oversize rejection, and the invariant that a cursor from one tool never decodes for another.
3. `web_fetch` pagination: chunk-ordinal slicing, `next_cursor`, stale handling, `max_chars`/`max_chars_cap`/`timeout_ms` interaction unchanged.
4. `batch_fetch` pagination: item-index slicing, per-page `max_total_chars`, `continue_on_error` unchanged, aggregate telemetry per page.
5. `research_search` pagination: full-canonical-order grouping then page slice; `max_groups`/`max_per_group` apply inside the page only.
6. `src/mcp/progress.rs` plus wiring the three handlers through `RequestContext<RoleServer>` (the `repo_search`/`repo_fetch` handlers at `src/mcp/server.rs:436`/`:464` already take a context, so this is an existing pattern). Prove payload identity: the same request with and without a progress token yields identical `structuredContent`.
7. Task layer: `[mcp.tasks]` config, `ServerState`/`EggsearchServer` wiring, the three `ServerHandler` overrides, capability advertisement, eligibility rule, cooperative cancel, saturation fallback.
8. Deadline composition: record `d.timeout_ms`, clamp on resume to `min(recorded, requested_or_config)`, install one deadline for the whole resumed call, and never issue a cursor for a page that delivered zero units.
9. Docs, output-schema keys, projection passthrough for `next_cursor`/`cursor_status` in `brief`/`compact` (these fields are punctuation-sized and must survive every projection, since dropping them would break resume for compact callers).
10. Full verification and the closure record.

## Verification

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features --test mcp_tools
cargo test --locked --all-features --test mcp_projection
cargo test --locked --all-features --test mcp_2026_protocol
cargo test --locked --all-features --test mcp_schema_slimming
cargo test --locked --all-features --test mcp_tool_contract
cargo test --locked --all-features --test web_fetch_integration
cargo test --locked --all-features --test research_workflow
cargo test --locked --all-features --test property_cursor
cargo test --locked --all-features --test static_guards
./packaging/check-repo-hygiene.sh
python3 packaging/check-planning-consistency.py
cargo test --locked --all-features
```

`tool_surface_live` is **not** a gate: it is `#[ignore]`d by default and its comparison entry point needs `EGGSEARCH_EVAL_MODEL`. Run it only as recorded operator evidence, never as an acceptance condition:

```bash
cargo test --locked --all-features --test tool_surface_live -- --ignored
```

Placement follows `architecture/testing.md`: cursor behaviour extends `mcp_tools` and `web_fetch_integration`, grouping behaviour extends `research_workflow`, wire shape extends `mcp_2026_protocol`, budget behaviour extends `mcp_projection`, and the pure codec gets the new `tests/property_cursor.rs` because pure functions get `proptest` files. `make check` is the canonical gate and must pass before closure. Because `cursor` is a new advertised property, `tests/mcp_schema_slimming.rs` property counts and the compact definition-byte baseline in `tests/fixtures/tool_surface/README.md` are re-recorded in the same change, not relaxed.

## Acceptance and stop conditions

- A client that never sends `cursor`, never sends a progress token, and never declares the tasks extension gets today's response, proven by a byte-identity test on a fixed fixture.
- `web_fetch` over content larger than `max_chars` returns a first page plus `next_cursor`; resuming with it returns the next page; iterating to exhaustion returns every chunk exactly once with no duplication and no gap, and the final page carries no `next_cursor`.
- The same holds for `batch_fetch` over more than one page of `items` and for `research_search` deep.
- A cursor from a different request, a different tool, or a different `v` is rejected with a stable repairable error code and no partial payload.
- A cursor whose `p.id` no longer exists returns the complete first page with `cursor_status: "stale"` — never a silently shifted page and never a partial page presented as complete.
- A resumed call never runs longer than the recorded `d.timeout_ms` and never extracts beyond `max_chars_cap`.
- With `[mcp.tasks].enabled = true` and a client declaring the extension, an eligible call returns a task handle; `tasks/get` reports `working` then `completed` with the same payload the synchronous path would return; `tasks/cancel` settles as `cancelled` and the spawned work stops at the next unit boundary. With the extension undeclared by the client, the same call returns synchronously.
- With `max_concurrent` tasks running, further eligible calls take the synchronous path; none error.
- No notification, log line, task record, or error message contains page text, a title, a snippet, a provider body, or a cookie.
- **Fail closed, return for a decision, and do not proceed if:** the cursor cannot be made content-derived and would require a server-side handle table that grows with request count; resumption requires exceeding `max_chars_cap`, `max_total_chars`, or the recorded timeout; the change would require an eleventh tool, a renamed field, or a new required output key; the resolved `rmcp` no longer exposes the APIs in the work package 0 table; or `TaskManager` would need per-call state that outlives `ttl_ms`. In the first three cases the correct return is today's complete-or-truncated response plus an explicit limitation note in the closure record — not a degraded page pretending to be complete.
- Stop and require an ADR if the final shape stops being additive: no new tool, no removed tool, no changed wire string, no `required`-set change. Anything else needs a decision, not a plan amendment.

## Closure evidence

`plans/closure/agentic-retrieval-integrity/009-status.md` must record: the implementation candidate SHA; the work package 0 capability table with the resolved `rmcp` version and the exact crate files read; the branch actually taken and, if the fallback branch was taken, the explicit statement of non-parity; the exact test commands and their results including `make check`; the byte-identity evidence for legacy callers; the pagination exhaustiveness evidence (every unit once, no gap, no duplication) for all three tools; the stale/mismatch/oversize rejection evidence; the measured progress rate-limit and byte-cap results; the task lifecycle transcript including cooperative cancel and saturation fallback; confirmation that no task id reached evidence identity; the re-recorded schema and compact byte baselines; and residual risk (content drift between pages, upstream result-set churn, extension availability in future `rmcp`, and the fact that a cursor is not portable across content changes).
