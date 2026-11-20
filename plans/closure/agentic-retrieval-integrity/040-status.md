# M040 Local Content-Aware Candidate Generation and Symbol Recall — Closure Evidence

Status: closed

Source plan: `plans/implementation/agentic-retrieval-integrity/040-local-content-aware-candidate-generation.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Decision authority: M037 (local relevance and scope, closed), ADR-0006 (keyless
retrieval), M006 routing, and the local workspace boundary in
`architecture/local-workspace.md`. No new ADR.

## Scope

M040 makes a content or symbol query search the workspace instead of a role-ranked
sample:

1. When the path-ranked first pass admits fewer than `max_results`, one
   deterministic second pass scans the remaining inventory in relative-path order.
2. The pass is bounded by 512 files read, 8 MiB of contents, 1.5 seconds, the
   request deadline, and `max_indexed_files`.
3. A bounded `git grep -l -z -i -F` literal hint skips candidates that cannot
   match; every candidate is still validated and read locally, and any git failure
   falls back to the walk.
4. Symbol hints participate in candidate selection and can find a definition with
   no path hint; there is no persistent symbol index.
5. `local_search_truncated` means a bound cut the scan short or returned matches
   were dropped; `local_search_completed_no_matches` requires an exhausted scan.

The paper-first contract is in `architecture/local-workspace.md` § Search flow.

## Exact code candidate

Implementation commit `10ed7ed08ac47d80e39ca676ea157ecf0728518c` (parent
`6cd6c9a`). Every command and probe below ran against that tree. This closure
commit carries this record, the plan status, the roadmap status, and the registry
rows together.

## Requirement-to-evidence mapping

| # | Acceptance criterion | Evidence |
|---|---|---|
| 1 | A content-only term in a file outside the first ten candidates is found | `tests/local_workspace_integration.rs::local_content_query_finds_a_file_beyond_the_first_candidate_window` asserts the late file is returned and `files_scanned > 10`. Live `query: "has_relevance"` returned 4 cards including `src/meta/local_inventory_cache.rs` (previously 0). |
| 2 | A symbol query finds a definition without a path or file hint, with provenance | `::local_symbol_only_match_preserves_structured_provenance` asserts the late definition and provenance. Live `query: "zzz_no_match_zzz"` + `symbol: "run_bounded_command"` returned `src/process.rs:12` with `matched_symbol: run_bounded_command`, `evidence_confidence: exact`, and `rank_reasons: ["hint_match"]`. |
| 3 | A true miss reports no-match only after an exhausted scan | `::non_empty_workspace_without_query_match_returns_no_cards_without_false_truncation` and the live miss probe: 0 cards, `local_search_completed_no_matches`, `has_truncation: false`. |
| 4 | A scan cut by a bound reports truncation and not no-match | `::second_pass_byte_bound_warns_truncated_without_claiming_no_match` asserts the truncation warning and the absence of the no-match warning. |
| 5 | Git-accelerated and walk paths return the same deterministic result | `::git_hint_and_walk_second_pass_return_the_same_late_candidate` compares both paths on a git fixture and skips gracefully when git is unavailable. |
| 6 | A path-rich query keeps its first-pass result and ordering | `::path_rich_local_query_keeps_first_pass_result_order`. Live `query: "local_backend"` returned 5 cards with `src/meta/local_backend.rs` first and no truncation. |
| 7 | No new tool, argument, schema property, configuration key, or shipped default; broad gate green | No MCP tool/argument/schema change; `make check` exit 0 with 85 suites, 6,453 passed, 0 failed, 28 ignored; `make docs-check` exit 0. |

## Live before and after

Probes used the all-features debug binary, MCP `stdio`, and the local-only
configuration rooted at this checkout (983 tracked files, 388 Rust files).

| Probe | Before (M037 evidence) | After (candidate `10ed7ed`) |
|---|---|---|
| `query: "has_relevance"` (defined in `src/meta/local_inventory_cache.rs`) | 0 cards + `local_search_completed_no_matches` | 4 cards: `src/meta/local_backend.rs`, `src/meta/local_inventory_cache.rs`, and both closure/plan documents; no truncation |
| `symbol: "run_bounded_command"` (definition at `src/process.rs:12`) | 0 cards | With a non-matching query and no path hint: 2 cards, including `src/process.rs:12` with exact symbol provenance |
| True miss (`zzz_definitely_absent_symbol_zzz`) | no-match warning on a ten-file window | 0 cards + `local_search_completed_no_matches`, `has_truncation: false` |
| `query: "local_backend"` (path-rich) | first-pass result | 5 cards, `src/meta/local_backend.rs` first, no truncation, ordering retained |

`repo_search` validation requires a non-empty query or a repository locator, so the
symbol path is exercised as a symbol hint alongside a non-matching query rather
than an empty query. That is the plan's "without a path or file hint" case.

## Scan bounds as implemented

- Second pass: 512 files read, 8 MiB of file contents, 1.5 seconds per pass, the
  request deadline, and `max_indexed_files`.
- Git hint: `git grep -l -z -i -F --untracked` with the configured
  `--exclude-standard`/`--no-exclude-standard`, up to eight `-e` patterns, a 4 MiB
  stdout cap, and a 2-second cap further limited to half the pass budget.
  `git grep` exit code 1 (no matches) is an empty hint set, not a failure; any
  spawn, status, timeout, or cap failure falls back to walking every candidate.

## Verification

Using the pinned Rust 1.89 toolchain on the candidate:

- `make check`: exit 0 — **85 suites, 6,453 passed, 0 failed, 28 ignored**.
- `make docs-check`: exit 0.
- Focused: `local_workspace_integration` 22, `repo_workflow` 100,
  `structured_local_code_intelligence` 11, `static_guards` 67, `lib` 3,937.
- `local_backend.rs` is 2,229 lines / 84,020 bytes against its 2,700 / 100,000
  ratchet; the inline test module moved byte-identically to
  `src/meta/local_backend_tests.rs` (819 lines).
- `cargo fmt --all -- --check` and `git diff --check`: clean.

## Known limitations and residual risk

1. The second pass is bounded at 512 files / 8 MiB / 1.5 seconds, so a very large
   workspace can still cut a recall scan short; the response reports truncation.
2. There is no persistent symbol index. Symbol recall depends on the structured
   parser or regex fallback over the files the bounded pass reads.
3. The git hint searches tracked and untracked files with the configured ignore
   semantics; a git failure or an unsupported flag silently degrades to the walk,
   which preserves results but not acceleration.
4. Second-pass matches are appended before the existing stable score sort, so ties
   keep first-pass results ahead; ordering is deterministic but not a global
   path-tie order.
5. No new tool, argument, schema property, configuration key, or shipped default;
   no new dependency.

## Compatibility and migration

No tool, argument, schema, or configuration change. Behaviour change: a content or
symbol query can now return files the ten-file window missed, and no-match now
means the bounded scan completed. Both are documented in the changelog and
`architecture/local-workspace.md`. No migration.

## Recommendation

Accept M040 on `10ed7ed08ac47d80e39ca676ea157ecf0728518c`. Acceptance criteria have
deterministic test evidence, the live probes show the recall gain, and the residual
bounds are explicit.
