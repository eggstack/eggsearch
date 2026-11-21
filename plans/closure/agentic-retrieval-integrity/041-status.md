# M041 Linked Worktree Identity Resolution and Prefer-Local Honesty — Closure Evidence

Status: closed

Source plan: `plans/implementation/agentic-retrieval-integrity/041-linked-worktree-identity-resolution.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M041

Corrective to: M037, closure `plans/closure/agentic-retrieval-integrity/037-status.md`; and M040,
closure `plans/closure/agentic-retrieval-integrity/040-status.md`.

Decision authority: accepted ADR-0006 (keyless deterministic retrieval), accepted ADR-0016
(unified fetch and legacy aliases), and the local workspace boundary in
`architecture/local-workspace.md`. No new ADR.

## Scope

M041 corrects two defects under one symptom: a checkout that is a linked git worktree had no
remote identity, and `repo_fetch` with `prefer_local` therefore fell through to the remote
path without saying so.

1. The shared git configuration is resolved through the worktree gitdir’s `commondir`. The
   resolution order is deterministic: a `.git` directory uses its own `config`; a `.git` file
   resolves the named gitdir and uses its `config` when present; otherwise the gitdir’s
   `commondir` resolves the shared configuration directory. The reads are bounded and
   symlink-safe, and no Git process is spawned.
2. A linked worktree exposes the same `matched_owner`, `matched_repo`, and `matched_host` as
   the equivalent normal clone, and `LocalRepoIdentity::matches` accepts it.
3. `prefer_local` precedence is defined and observable in every case: a matching local checkout
   is fetched locally; otherwise the remote path is used and the response states that the local
   copy was skipped and why. The authorized-host-scope error path is unchanged.
4. `workspace_id` derivation inputs, trust labels, root-relative paths, stable IDs, and
   deterministic ordering are unchanged.

The paper-first contract is in `architecture/local-workspace.md` and `docs/features.md`.

## Exact code candidate

Implementation candidate `380a55cd7ea7c0b37dc6eff53851926e96e894d0` (parent
`688948b40394563315bbaea6ef9680cc94af3676`). Every command and probe below ran against
that tree. The candidate was rebased onto the concurrently drafted planning commit
`688948b` so that this milestone and the concurrently drafted knowledge-delta proposals share
one linear branch; the rebased candidate carries the same source, test, and documentation tree
as the tree that was qualified, and `688948b` touches planning documents only. This closure commit carries this
record, the plan status, the roadmap status, and the registry rows together.

## Requirement-to-evidence mapping

| # | Acceptance criterion | Evidence |
|---|---|---|
| 1 | A checkout in a linked worktree exposes the same `matched_owner`, `matched_repo`, and `matched_host` as the equivalent normal clone | `tests/local_workspace_integration.rs::linked_worktree_resolves_the_same_remote_identity_as_main_checkout` builds a real linked worktree with `git worktree add`, asserts the per-worktree gitdir has no `config` but has `commondir`, and compares host, owner, repo, remotes, and `matches()` against the main clone. |
| 2 | `prefer_local` against a linked worktree returns `locator.kind == "workspace"` and performs no remote fetch | `tests/repo_workflow.rs::repo_fetch_prefer_local_uses_linked_worktree_without_remote_fetch` asserts `locator.kind` is `workspace`, trust is `local_trusted`, the body is the local file, and the mock remote recorded zero hits. |
| 3 | A `prefer_local` request that matches no local checkout states that the local copy was skipped, and the remote result is still returned | `::repo_fetch_prefer_local_reports_skip_and_returns_remote_result` asserts the remote body is returned and the response warning contains `prefer_local_skipped`, `local copy was skipped`, and `no matching local repository`. `::repo_fetch_prefer_local_names_the_skip_reason_when_the_remote_fails` asserts the same reason survives a failing remote fetch, so a 404 cannot mask it. |
| 4 | The existing normal-clone and submodule-style fixtures keep their behaviour | The unchanged `read_remotes_from_config_gitfile` unit fixture (the submodule layout) passes; the normal-clone identity is compared in the new test and the live probe below shows the normal clone unchanged. |
| 5 | `workspace_id`, trust labels, root-relative paths, stable IDs, and deterministic ordering are unchanged | `workspace_id` is asserted to equal `compute_workspace_id(root_path, remotes, current_commit)` for both the main clone and the linked worktree; `tests/structured_local_code_intelligence.rs::linked_worktree_structured_search_keeps_repository_relative_path` asserts repository-relative paths are preserved; no stable-ID or ordering code was touched. |
| 6 | No Git process is spawned; the resolution is a bounded file read | The resolution uses `read_bounded_git_file` over the gitfile, `commondir`, and `config` only. `run_bounded_command` was not used for identity. The linked-worktree fixture uses Git only to construct a real layout. |
| 7 | No new tool, argument, schema property, configuration key, or dependency | `make check` static guards pass; no MCP surface changed. |
| 8 | The broad gate passes with 0 failures | `make check` exit 0 on the candidate; `make docs-check` exit 0. |

## Live before and after

Probes used the candidate debug binary and the baseline `16b4c288` debug binary, MCP
`stdio`, and a configuration whose only difference between the two runs was the `[local].roots`
entry. Request: `{owner: "eggstack", repo: "eggsearch", path: "AGENTS.md", prefer_local: true,
max_chars: 300}`.

| Root | Before (`16b4c288`) | After (`380a55c`) |
|---|---|---|
| Main clone `/Users/davidbowman/projects/eggsearch` | `locator: {"kind": "workspace", "path": "AGENTS.md", "workspace_root": "eggsearch"}` | `locator: {"kind": "workspace", "path": "AGENTS.md", "workspace_root": "eggsearch"}` — unchanged |
| Linked worktree `.../.worktrees/feat-auto-20261117-044c4885` | `locator: {"kind": "remote", "owner": "eggstack", "repo": "eggsearch"}`, `fetched_url: https://raw.githubusercontent.com/eggstack/eggsearch/main/AGENTS.md` | `locator: {"kind": "workspace", "path": "AGENTS.md", "workspace_root": "feat-auto-20261117-044c4885"}`, `raw_url: workspace://feat-auto-20261117-044c4885/AGENTS.md`, trust `local_trusted`, no remote URL fetched |
| Linked worktree, repository matching no local checkout | Remote fetch attempted; the response carried no statement about the skipped local copy | Remote fetch attempted; the response states `prefer_local_skipped: local copy was skipped because no matching local repository was found`, and a failing remote repeats the reason in the error |

The main-clone row is the control: the fix does not change an already-resolvable checkout.

## Verification

Using the pinned Rust 1.89 toolchain on the candidate `380a55c`:

- `make check`: exit 0 — 90 suites, 6,520 passed, 0 failed, 28 ignored.
- `make docs-check`: exit 0.
- Focused: `local_workspace_integration` 23, `repo_workflow` 103,
  `structured_local_code_intelligence` 12, `mcp_tools` 29, `static_guards` 67, `lib` 3,947.
- `src/meta/local_inventory.rs` is 1,532 lines / 51,920 bytes, under the 1,600-line /
  81,920-byte ordinary-file ceiling.
- `cargo fmt --all -- --check` and `git diff --check`: clean.

## Known limitations and residual risk

1. The remote fallback remains the default when no local checkout matches. That is the
   intended behaviour; the change is that the fallback now says so.
2. The gitfile, `commondir`, and configuration reads are bounded (4 KiB, 4 KiB, and 256 KiB)
   and reject symlinked files. A checkout whose `.git` is itself a symlink, or whose
   configuration exceeds the cap, therefore loses its remote identity and falls back to the
   remote path. No such layout exists in the supported checkout forms, but the behaviour is
   a deliberate change from the previous unbounded, symlink-following read.
3. The shared configuration is resolved through `commondir` only. A layout that needs `.git`
   indirection beyond the stated resolution order is not handled and would need a separate
   milestone rather than a wider resolver.
4. The local-skip reason is attached to the response warnings and to a failing remote error.
   It is not a new top-level response field, and no advertised schema changed.
5. The upstream provider availability of the remote fallback is unchanged; a failing remote
   still surfaces its own upstream error.

## Compatibility and migration

No tool, argument, schema, or configuration change. Behaviour change: a checkout in a linked
worktree now resolves `prefer_local` to the local copy instead of the remote. A checkout whose
git configuration is genuinely unreadable still falls back to the remote path, and the response
now states that the local copy was skipped. No index, storage, or dependency change; no
migration.

## Hosted qualification

The candidate is pushed to `origin/feat/auto-20261117-044c4885` for the hosted pipeline.
Hosted CI was not a requirement of this milestone plan, so the local gate above is the
closure evidence; the hosted run is recorded here when it completes.
## Recommendation

Accept M041 on `380a55cd7ea7c0b37dc6eff53851926e96e894d0`. Every acceptance criterion has
deterministic test evidence, the live probe shows the linked worktree resolving locally while
the normal clone is unchanged, and the residual bounds are explicit.
