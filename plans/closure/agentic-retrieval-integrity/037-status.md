# M037 Local Repo-Search Relevance and Scope — Closure Evidence

Status: closed

Source plan: `plans/implementation/agentic-retrieval-integrity/037-local-repo-search-relevance-and-scope.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Decision authority: existing contracts — M006 routing, ADR-0006 keyless retrieval, and the local
workspace boundary in `architecture/local-workspace.md`. No new ADR.

## Scope

M037 makes a local result mean "this file matched" and makes local inclusion scope-honest:

1. `score_inventory_entry` added a role bonus (Readme +12, Implementation +10, Documentation +8,
   Test/Example +5) before the `score > 0.0` admission test, so any README or implementation file
   was admitted with zero query relevance.
2. `local_search_completed_no_matches` was therefore unreachable in a workspace containing any
   README or implementation file; a nonsense query returned five README cards.
3. `include_local` defaulted to true even when the request named a remote repository, so a scoped
   query could return unrelated workspace files.
4. `local_search_truncated` fired on a five-card answer; candidate pruning and the result cap were
   not distinguished.

The paper-first contract is in `architecture/local-workspace.md` (admission, scope, no-match, and
truncation).

## Exact code candidate

Implementation commit `76c1156a2c5418d4b9dab4493eddd9232bdd7b8a` (parent `614634198bfb6f145f7c223d5a330cc8924f677d`). The
pre-commit implementation fingerprint was
`103dbdf22a28f3bed06ff0209fc8fbabacf691be13d76e5bce072bd90fc89c7d` (SHA-256 of
`git diff -- src tests docs architecture CHANGELOG.md`, 2026-11-19T21:21:54Z). This closure
commit carries this record, the plan status, the roadmap status, and the registry rows together.

## Requirement-to-evidence mapping

| # | Acceptance criterion | Evidence |
|---|---|---|
| 1 | Admission requires a query/path/file/symbol match; role is ranking only | `score_inventory_entry` keeps its role weights unchanged. `has_relevance` in `src/meta/local_inventory_cache.rs` requires a symbol match, a path/file hint, an exact file-name or token path match, or a content match. It gates the inventory path, the cold-inventory path, and the fallback walker. |
| 2 | A nonsense query in a non-empty workspace returns zero cards and the no-match warning | `tests/local_workspace_integration.rs` adds a five-file workspace with language/repository hints and a nonsense query: zero cards, `local_search_completed_no_matches`, and no truncation warning. The live probe below repeats it on this checkout. |
| 3 | Remote-scoped requests include local results only with explicit `include_local: true` | `RepoSearchRequest::include_local_enabled()` returns explicit `true`/`false` unchanged and defaults to "no remote hints" using `resolved_hints()` (`host`, `owner`, `repo`, `org`). `tests/repo_workflow.rs` covers omitted/true/false with an empty mock remote engine. The live probe repeats the omitted case. |
| 4 | `local_search_truncated` means the result was truncated | Inventory-root truncation now propagates, the fallback walker admits one match beyond the cap before flagging, and candidate pruning alone does not set the flag. The truncation regression uses a one-card local budget with more admitted matches. |
| 5 | Existing empty-workspace no-match, symbol provenance, trust, and Mode-Off behaviour hold | The original empty-`TempDir` test still passes; the new structured-symbol case asserts `symbol_provenance`; Mode-Off local-only execution and `local_trusted` are unchanged. |
| 6 | No new tool, argument, schema property, or unscoped default | `include_local` keeps its name and meaning; `src/core/repo_search.rs` changes only the doc comment and the default resolver. |
| 7 | Broad gate | `make check` exit 0: 85 suites, 6,445 passed, 0 failed, 28 ignored. `make docs-check` exit 0. |

## Live before and after

Local-only probes used `[search].mode = "off"` with the workspace root at this checkout; the
remote-scoped probe used the live config. All ran against the candidate.

| Probe | Before (review evidence) | After |
|---|---|---|
| `repo_search` `query: "zzzzqqqxx"`, `sources: ["code"]`, `include_local: true` | 5 workspace README cards in a `readme` group, snippet `Local file: README.md`, `local_search_truncated`, no no-match warning | 0 groups, 0 results, `local_search_completed_no_matches`, no `local_search_truncated`; 875 ms |
| `repo_search` `query: "VecDeque"`, `owner: "rust-lang"`, `repo: "rust"`, `include_local` omitted | 0 remote results and 5 unrelated workspace README cards | 9 remote results, **0** `local_trusted` cards, no local no-match warning (local search was not dispatched); resolved hints show `owner: rust-lang`, `repo: rust` |
| `repo_search` `query: "has_relevance"`, `path: "src/meta"`, `include_local: true` | Not probed | 5 local cards, `local_search_truncated` (candidate window), no no-match warning |

## Verification

Using the pinned Rust 1.89 toolchain on the candidate:

- `make check`: exit 0 — **85 suites, 6,445 passed, 0 failed, 28 ignored**.
- `make docs-check`: exit 0.
- `cargo test --locked --all-features --test local_workspace_integration`: 18 passed, 0 failed.
- `cargo test --locked --all-features --test repo_workflow`: 100 passed, 0 failed.
- `cargo test --locked --all-features --test structured_local_code_intelligence`: 11 passed.
- `cargo test --locked --all-features --test mcp_tools`: 28 passed.
- `cargo test --locked --all-features --test static_guards`: 67 passed.
- Module sizes: `src/meta/local_backend.rs` 2,674 / 2,700 lines; `src/meta/local_inventory_cache.rs`
  1,730 / 1,800 lines.
- `cargo fmt --all -- --check` and `git diff --check`: clean.

## Known limitations and residual risk

1. **Content matches are bounded by path/role candidate generation.** `select_inventory_candidates`
   ranks the inventory by path/file tokens and role, truncates to `max_results * 2`, and only then
   reads content. A content-only query can therefore miss a file that genuinely contains the term:
   the live probe `query: "has_relevance"` with no path hint returned zero cards even though
   `src/meta/local_inventory_cache.rs` defines it. This is a pre-existing backend design (the local
   index is out of M037's scope), and the admission fix makes it visible rather than masking it
   with role-only cards. A future plan would need content-aware candidate generation or a bounded
   full-text index.
2. **A path hint admits every file under that path.** `has_relevance` treats a non-empty path or
   file hint as the relevance signal, so a path-scoped query returns the candidate window ordered
   by role/path even when the query text does not appear in those files. This follows the stated
   "path/file match admits" rule, but it is weaker than content search and should be understood as
   scoping, not matching.
3. **A timed-out local search does not claim completion.** The no-match warning is suppressed when
   `timed_out` is set, so a timeout reports `local_search_timeout` rather than a contradictory
   "completed without matching files". Truncation still does not suppress the warning.
4. Repository identity and language remain boosters/filters, never admission signals, and unscoped
   requests keep the previous default (local included).
5. No change to the local index, symbol parser, `repo_map`, root authorization, or trust labels;
   no new dependency.

## Compatibility and migration

No tool, argument, or schema change. One behaviour change: a request that names a remote repository
and omits `include_local` no longer receives local cards; an explicit `true` restores them and
`false` still suppresses them everywhere. Unscoped requests are unchanged. The feature and tool
matrix pages document the scope rule.

## Recommendation

Accept M037 on the closure commit. The corrective criteria are met with deterministic tests and the
live probes; the content-recall limit above is a pre-existing backend property that M037 makes
honest, and it is recorded as the next capability gap rather than hidden.
