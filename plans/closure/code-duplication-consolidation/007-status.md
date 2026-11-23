# M007 One Repository-Target Field Set — Closure Evidence

Status: closed

Candidate: the working tree on `feat/auto-20261117-044c4885` above baseline
`fb9d6538f8ea2e21e08add43fb3f71ebe6ceaa64`, uncommitted because the request did not ask for a
commit. The canonical gate ran on exactly this tree; its output and the diff fingerprint are in
`plans/closure/code-duplication-consolidation/gate.md`.

Source plan: `plans/archive/implementation/code-duplication-consolidation/007-one-repository-target-field-set.md`

Roadmap: `plans/subsystems/code-duplication-consolidation-roadmap.md` M007

Decision authority: `plans/adrs/ADR-0023-one-repository-target-field-set.md`, accepted
2026-11-22. The acceptance is recorded in the ADR, the plan, and the registry.

## Change

- `core::repo_fetch::RepoTarget` owns the sixteen common locator fields. It lives in
  `src/core/repo_target.rs` (80 lines) and is re-exported from `core::repo_fetch` to keep the
  documented path; the module split keeps `src/core/repo_fetch.rs` at 1,623 lines, under its
  1,650-line ratchet.
- `RepoFetchArgs` is `host` + flattened `target` + `timeout_ms` + `response_detail` +
  `test_fetch_url`; `BatchFetchItem::Repo` is `host` + `target` + the focus trio;
  `RepoFetchRequest` is `host: Option<CodeHost>` + `target` + `timeout_ms`.
- `FetchRepoItem` is retired; `fetch`'s repo variant holds `RepoFetchArgs`.
- One `From<RepoTarget> for RepoFetchRequest` owns the locator-to-request conversion; the alias,
  batch, and fetch surfaces route through it.
- A parity test asserts each envelope's serialized keys equal `RepoTarget` keys plus its
  extras and that a full JSON object round-trips without a lossy drop.

## Schema measurements

| Full profile | Before | After |
|---|---:|---:|
| `fetch` definition | 9,013 B | 9,001 B |
| `fetch` input schema | 8,109 B | 8,097 B |
| `fetch` `$defs` | 12 | 11 |
| `tools/list` | 31,521 B | 31,509 B |
| Fingerprint | `82fb78440ab2053a` | `c98316a5061399d6` |

Compact `tools/list`: 18,686 → 19,282 B (fingerprint `a487e5c521541c84` →
`bcc94e8b6857744f`), below the existing 22,540 B cap. Compact `fetch`: 7,472 B definition,
7,223 B input schema, 11 `$defs`. The full `fetch` has a non-increasing guard at
`tests/mcp_schema_slimming.rs:306`; no budget was raised.

## Equivalence evidence

- Fixed alias fixture (mock local backend): text, lines 3–7, `fetch_fff990eb67a6ecac`, and
  `span_f9fbcbe39f0984eb` are unchanged; the regression is pinned in `tests/repo_workflow.rs`.
- `cargo test --locked --all-features --lib`: 3,865 passed.
- `mcp_schema_slimming` 27, `mcp_tool_contract` 22, `schema_identity_registry` 88,
  `batch_fetch_retrieval` 27, `mcp_tools` 29.
- Workflow/corpus suites: `corpus_runner` 55, `local_workspace_integration` 25,
  `repo_workflow` 103, `research_workflow` 141, `security_workflow` 20,
  `web_fetch_integration` 128 passed / 1 ignored — 372 passed across the six affected suites.
- `tool_surface_live`: deterministic report contract 1 passed; the ignored comparison passed via
  its explicit skip because `EGGSEARCH_EVAL_MODEL` is unset.

## Compatibility

No JSON property name, accepted value, default, tool name, or response field changed. The
advertised `fetch` repo-item descriptions are byte-identical to baseline. `repo_fetch` and
`batch_fetch` now share that wording instead of their previously drifted copies, which is the
documentation consolidation ADR-0023 decision 1 selects; names, values, and defaults are
unchanged. `repo_fetch` remains callable and unadvertised; the alias host diagnostic is
preserved because `host` keeps its wire type on every envelope.
