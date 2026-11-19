# M037 — Local Repo-Search Relevance and Scope

Status: closed
Date: 2026-11-19
Subsystem: agentic retrieval integrity
Milestone class: invariant (corrective)
Decision authority: existing contracts — M006 routing, ADR-0006 keyless retrieval, and the local
workspace boundary in `architecture/local-workspace.md`. No new ADR.

References: `plans/000-long-term-specification.md` §2 (stable MCP tool contract) and §4 (trust and
safety bounds); `architecture/local-workspace.md` and `architecture/meta.md`.

## Objective

Make a local result in `repo_search` mean "this file matched the query", and make local inclusion
scope-honest. A query that matches nothing in a non-empty workspace returns an explicit empty
state, not inventory cards; a request that names a remote repository does not silently receive
unrelated workspace files. The caller should not have to filter workspace noise out of a remote
result set.

## Evidence

Measured on 2026-11-19 against candidate `55180790c6dc5dbb2ce0d94c186ef88cd7584f48` with the local
workspace enabled and rooted at this checkout:

1. `repo_search` with `query: "zzzzqqqxx"`, `sources: ["code"]`, `include_local: true`, and
   remote execution off returned **5 local README cards** in a `readme` group, each with the
   snippet `Local file: README.md`, and no `local_search_completed_no_matches` warning. The same
   response carried `local_search_truncated`.
2. `repo_search` with `query: "VecDeque"`, `owner: "rust-lang"`, `repo: "rust"` returned
   **0 remote results** (DuckDuckGo blocked, Brave 429, Yahoo 500) and the same 5 workspace README
   cards. `include_local` was omitted; it defaults to true
   (`RepoSearchRequest::include_local_enabled`).
3. `score_inventory_entry` adds a role bonus regardless of query match: Readme +12,
   Implementation +10, Documentation +8, Test/Example +5. `search_sync` admits every candidate
   with `score > 0.0`, so the role bonus is an admission floor: any README or implementation file
   passes even with zero query relevance.
4. `local_search_completed_no_matches` is emitted only when `local_cards.is_empty()`. Because the
   role bonus admits files, the warning is unreachable in any workspace that contains a README or
   implementation file.
5. `tests/local_workspace_integration.rs` exercises the no-match warning only against an empty
   `TempDir`, so the defect is invisible to the existing suite.

## Scope

In scope:

- the local relevance/admission rule in `src/meta/local_backend.rs`;
- `include_local` scope semantics in `src/meta/adapter/repo.rs`;
- the no-match and truncation warnings;
- tests and documentation for those contracts.

Out of scope:

- remote provider selection, routing, and failure handling;
- the local index, symbol parser, `repo_map`, and workspace authorization/host-scope behavior;
- a new tool, argument, or default for unscoped queries;
- changing `local_trusted` trust semantics or root-relative paths.

## Invariants

- Local results remain `local_trusted` provenance, root-relative, sanitized, and bounded by the
  configured roots and index limits.
- Mode-Off local-only execution and the deny-remote policy are unchanged.
- `include_local: false` still suppresses local search entirely.
- A repository identity or language hint is a booster or filter, never an admission signal.
- No new tool, argument, or schema property; the change refines existing behavior.
- Deterministic ordering and stable IDs are unchanged.
- No new dependency; `cargo fmt` clean; ordinary files under 1,600 lines / 80 KB.

## Expected production changes

- `src/meta/local_inventory_cache.rs`: the role bonus stays a ranking input but no longer admits a
  file.
- `src/meta/local_backend.rs`: admission, no-match, and truncation signals.
- `src/meta/adapter/repo.rs`: remote-scope handling for `include_local`.
- `tests/local_workspace_integration.rs`, `tests/repo_workflow.rs`: regression coverage.

## Compatibility and migration

- No tool, argument, or schema change; `include_local` keeps its name and its explicit `false`
  meaning.
- One behavior change: a remote-scoped request with `include_local` omitted stops receiving local
  cards; unscoped requests keep today's default. It is documented in the feature and tool-matrix
  pages.
- No index, storage, configuration, or dependency change. No migration.

## Dependencies

- Hard: none (the local search path is shipped and tested).
- Interface: M006 routing (closed) for the hint/scope semantics; `architecture/local-workspace.md`
  for the boundary.
- Soft: none.

## Work packages

1. **Paper first.** Update `architecture/local-workspace.md` (search admission, scope, no-match,
   and truncation) and, if grouping is described there, `architecture/meta.md`. State that a
   local file is admitted only on a relevance signal and that a remote-scoped request includes
   local results only when asked. This lands before code.
2. **Separate relevance from role.** Keep the role bonus for ranking, but require a query,
   path/file, or symbol match before admission. A file with only a role, language, or repository
   match is not a result.
3. **No-match signaling.** Emit `local_search_completed_no_matches` whenever local search ran and
   admitted zero cards, including when candidates existed. Do not derive it from "no candidates".
4. **Scope semantics.** When the resolved hints name a remote repository or code host
   (`host`, `owner`, `repo`, `org`, or a resolved `repo:owner/name` token), run local search
   only when `include_local` is explicitly `true`; when it is `None`, do not dispatch local
   search and do not add local cards. Unscoped requests keep today's default (local included).
   An explicit `false` still suppresses everywhere.
5. **Truncation meaning.** Audit why `local_search_truncated` fired on a five-card answer. Emit it
   only when returned matches were truncated or the inventory truncation could have hidden a
   match; candidate pruning alone is not truncation.
6. **Tests.** Extend `tests/local_workspace_integration.rs` and `tests/repo_workflow.rs` with:
   - a non-empty workspace and a nonsense query: zero local cards and the no-match warning;
   - a matching query: cards, with `symbol_provenance`/`is_exact_definition` where the structured
     parser applies;
   - a remote-scoped request with `include_local` omitted: zero local cards;
   - a remote-scoped request with `include_local: true`: matching local cards only;
   - `include_local: false`: zero local cards;
   - a truncation case: the flag is true only when the result was actually truncated;
   - the existing empty-workspace no-match test still passes.
7. **Documentation.** `architecture/local-workspace.md`, `docs/features.md`,
   `docs/tool-matrix.md` (the `repo_search` local note), `docs/test-inventory.md`, and a
   `CHANGELOG.md` entry under the existing unreleased section.

## Verification

```bash
cargo test --locked --all-features --test local_workspace_integration
cargo test --locked --all-features --test repo_workflow
cargo test --locked --all-features --test structured_local_code_intelligence
cargo test --locked --all-features --test research_workflow
cargo test --locked --all-features --test mcp_tools
cargo test --locked --all-features --test static_guards
make check
make docs-check
./packaging/check-repo-hygiene.sh
```

## Acceptance criteria

- A query with no match in a non-empty workspace returns zero local cards and the
  `local_search_completed_no_matches` warning.
- Role, language, and repository identity never admit a file without a query, path/file, or
  symbol match.
- A request that names a remote repository returns no local cards unless `include_local: true`
  is explicit; with `true`, only matching local cards are added.
- `local_search_truncated` is reported only when the returned result was truncated.
- Mode-Off local-only search, `include_local: false`, trust labels, root-relative paths, and
  deterministic ordering are unchanged.
- No new tool, argument, or schema property is introduced.
- The broad gate passes with 0 failures.

## Stop conditions

Stop and record rather than working around any of these:

- the correct scope rule requires a new argument, or a default change for unscoped requests (that
  needs its own measured acceptance);
- the scoring function is shared with a surface whose contract would change (for example
  `repo_map`), so the fix would need a second plan;
- a zero-result local search would break a documented inventory/orientation behavior; record the
  tradeoff rather than hiding it;
- the no-match warning cannot be made reliable without changing the local result type.

## Required closure evidence

`plans/closure/agentic-retrieval-integrity/037-status.md` with the exact candidate SHA, a
requirement-to-evidence matrix, the before/after responses for the nonsense and remote-scoped
probes, the suite results, the broad-gate counts, and residual risk (including any surface that
still receives inventory cards by design).
