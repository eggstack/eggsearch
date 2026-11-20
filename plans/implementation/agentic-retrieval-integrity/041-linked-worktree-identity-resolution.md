# M041 — Linked Worktree Identity Resolution and Prefer-Local Honesty

Status: ready
Date: 2026-11-20
Subsystem: agentic retrieval integrity
Milestone class: capability (corrective to M037 and M040, local workspace scope)
Baseline: `16b4c2888206caeb6654ffebaa039862fc143712` on `feat/auto-20261117-044c4885`
Decision authority: existing contracts — accepted ADR-0006 (keyless deterministic retrieval),
accepted ADR-0016 (unified fetch and legacy aliases), and the local workspace boundary in
`architecture/local-workspace.md`. No new ADR.

References: `plans/000-long-term-specification.md` §2 and §4; `plans/001-terminology-and-domain-model.md` §3;
`plans/003-planning-process.md` §7;
`plans/subsystems/agentic-retrieval-integrity-roadmap.md` M037, M040;
original plan `plans/implementation/agentic-retrieval-integrity/037-local-repo-search-relevance-and-scope.md`;
original closure `plans/closure/agentic-retrieval-integrity/037-status.md`;
`architecture/local-workspace.md`.

## Objective

Make the local checkout identity resolve in a linked git worktree, and make `repo_fetch`
honest when it does not.

Today a `repo_fetch` that asks for the local copy of a repository checked out as a linked git
worktree silently performs a remote fetch instead, and can then return a 404 for a repository
that exists on the operator's own disk. The same request against a normal clone resolves to the
local copy.

Two defects sit under that symptom: the shared git configuration is not found in a linked
worktree, so the checkout has no remote identity and can never match a requested owner and repo;
and the `prefer_local` branch has no defined behaviour when the local copy is not used.

## Evidence

Measured on 2026-11-20 against baseline `16b4c2888206caeb6654ffebaa039862fc143712`, driving
`repo_fetch` through the MCP server over stdio with a temporary configuration whose only
difference between the two runs was the `[local].roots` entry.

1. `resolve_git_dir` (`src/meta/local_inventory.rs:334`) handles a gitfile checkout by
   returning the gitdir it names. `read_remotes_from_config` (`src/meta/local_inventory.rs:362`)
   then reads `<gitdir>/config` (line 367).
2. A linked worktree's gitdir does not contain `config`. On this checkout `.git` is a file
   naming `/Users/davidbowman/projects/eggsearch/.git/worktrees/feat-auto-20261117-044c4885`.
   That directory contains `HEAD`, `commondir`, `index`, `logs`, and `refs`, and no
   `config`. The shared configuration lives in the common directory, which `commondir`
   (`../..`) names.
3. The read therefore yields no remotes, so `build_repo_identity`
   (`src/meta/local_inventory.rs:690`) leaves `matched_owner` and `matched_repo` as `None`.
   `LocalRepoIdentity::matches` (line 170) then returns false for every request, so
   `match_local_repo` (line 741) never matches.
4. `repo_fetch` therefore never enters its local branch. `prefer_local` is handled at
   `src/mcp/tools/repo_fetch.rs:157`; the local match at line 170 fails, and the call falls
   through to the remote path at line 214.
5. Live control, identical request `{owner: "eggstack", repo: "eggsearch", path: "AGENTS.md",
   prefer_local: true}`, differing only in `[local].roots`:
   - roots = the main repository: `locator: {"kind": "workspace", "path": "AGENTS.md",
     "workspace_root": "eggsearch"}`;
   - roots = this linked worktree: `locator: {"kind": "remote", ...}`, `fetched_url:
     https://raw.githubusercontent.com/eggstack/eggsearch/main/AGENTS.md`.
   A request naming a repository that exists only on the operator's disk resolves remotely, and
   a worktree-only name 404s.
6. The existing regression test `read_remotes_from_config_gitfile`
   (`src/meta/local_inventory.rs:1202`) writes the configuration *inside* the directory named by
   the gitfile. That is the submodule layout. A linked worktree puts the configuration in the
   common directory instead, so no test exercised the failing layout and the suite passed.
7. The remote fallback is also silent. The `no matching repository` error in the `prefer_local`
   branch fires only when `host_local_scope.is_some()` (an authorized host scope). In the
   ordinary configured-roots case the caller is never told that the local copy was skipped.

## Why the earlier verification missed this

M037 and M040 established relevance admission, scan bounds, and candidate generation for local
search. Their fixtures and live probes used roots whose `.git` is a directory, so the remote
identity was always readable, and `prefer_local` was never asserted to resolve to the local
copy. `read_remotes_from_config_gitfile` appears to cover the gitfile case, but its fixture
models a submodule rather than a linked worktree. The gap is in fixture layout and in the
absence of any `prefer_local` resolution assertion, not in the admission or ranking rules that
M037 and M040 closed.

## Scope

In scope:

- resolving the shared git configuration through `commondir` so a linked worktree has a remote
  identity;
- a `prefer_local` precedence that is defined and observable in every case;
- regression coverage for the linked-worktree layout.

Out of scope:

- the `include_local` default and the local search admission, ranking, and scan rules closed in
  M037 and M040;
- trust labels, root-relative paths, stable IDs, and deterministic ordering;
- the host-scope authorization model for private workspace roots;
- submodule semantics beyond what is needed to keep the existing gitfile fixture passing;
- a new tool, argument, schema property, configuration key, dependency, or git execution
  capability; the resolution stays a bounded file read.

## Invariants

- A checkout's remote identity comes from the operator's own git configuration and from nowhere
  else. No network, no `git remote` invocation, no credential access.
- The resolution order is deterministic: `.git` directory, then the gitfile gitdir, then that
  gitdir's `commondir`, then the per-worktree gitdir.
- `workspace_id`, trust labels, root-relative paths, and stable IDs are unchanged for a checkout
  whose configuration was already readable. A checkout that gains a remote identity gains a
  `matched_owner`/`matched_repo`; it must not change its `workspace_id` derivation inputs.
- A local result is still admitted only on a query, path/file, or symbol match.
- No workspace code is executed and no native plugin is loaded.
- Bounded I/O: the resolution reads a small, fixed set of files and stays inside the existing
  per-root inventory budget.

## Expected production changes

- `src/meta/local_inventory.rs`: common-directory resolution for the shared git configuration;
  a helper that resolves the configuration path through `commondir`; the worktree fixture.
- `src/mcp/tools/repo_fetch.rs`: the `prefer_local` branch reports the local resolution
  outcome — resolved locally, or not resolved locally — without changing the remote fallback
  itself.
- `tests/local_workspace_integration.rs`, `tests/repo_workflow.rs`,
  `tests/structured_local_code_intelligence.rs`: regression coverage.
- `architecture/local-workspace.md`, `docs/features.md`, `docs/tool-matrix.md`,
  `docs/test-inventory.md`, `CHANGELOG.md`.

## Compatibility and migration

- No tool, argument, schema property, or configuration change.
- Behaviour change: a checkout in a linked worktree now resolves `prefer_local` to the local
  copy instead of the remote. This is the intended correction.
- A checkout whose git configuration is genuinely unreadable still falls back to the remote
  path; the response now states that the local copy was skipped.
- No index, storage, or dependency change. No migration.

## Dependencies

- Hard: M037 (closed) for the local scope and admission rules this milestone must not reopen;
  M040 (closed) for the candidate-generation and scan semantics that must stay unchanged.
- Interface: `architecture/local-workspace.md` for the ownership boundary.
- Soft: none.

## Work packages

1. **Paper first.** Update `architecture/local-workspace.md` to state the git-layout resolution
   order, that the shared configuration is found through `commondir`, and the `prefer_local`
   precedence and its reporting. This lands before code.
2. **Common-directory resolution.** Resolve the git configuration through the worktree gitdir's
   `commondir` when that file is present and the per-worktree gitdir has no `config` of its
   own. Fall back to the existing per-worktree path, then to the current behaviour. Keep the
   read bounded and symlink-safe.
3. **Identity correctness.** A linked worktree must expose `matched_owner`, `matched_repo`,
   and `matched_host` exactly as the normal clone does, and `LocalRepoIdentity::matches` must
   accept it. Do not change the `workspace_id` derivation inputs.
4. **Prefer-local reporting.** Define and implement the `prefer_local` precedence: a matching
   local checkout is fetched locally; otherwise the remote path is used and the response states
   that the local copy was skipped and why. Preserve the existing authorized-host-scope error
   unchanged.
5. **Worktree fixture.** Add a fixture that builds a real linked-worktree layout — a `.git`
   file naming a per-worktree gitdir, a `commondir` pointer, and a `config` in the common
   directory — and assert the remote identity is found. Keep the existing submodule-style
   fixture passing.
6. **Tests.** Extend the suites with:
   - a linked-worktree layout resolves `matched_owner` and `matched_repo`;
   - `prefer_local` against a linked worktree returns `locator.kind == "workspace"` and does
     not perform a remote fetch;
   - a `prefer_local` request that matches no local checkout states that the local copy was
     skipped and still returns the remote result;
   - the normal-clone and submodule-style fixtures still resolve identically;
   - `workspace_id`, trust labels, and root-relative paths are unchanged for an already
     readable checkout;
   - no git process is spawned by the resolution.
7. **Documentation.** `architecture/local-workspace.md`, `docs/features.md`,
   `docs/tool-matrix.md` (the `repo_fetch` local note), `docs/test-inventory.md`,
   `CHANGELOG.md`.

## Verification

```bash
cargo test --locked --all-features --test local_workspace_integration
cargo test --locked --all-features --test repo_workflow
cargo test --locked --all-features --test structured_local_code_intelligence
cargo test --locked --all-features --test mcp_tools
cargo test --locked --all-features --test static_guards
make check
make docs-check
```

A live acceptance probe is required: the same `repo_fetch` request run against a linked
worktree root and against the normal clone root, with both responses recorded. The defect is
only observable with a real worktree layout, so the fixture evidence and the live probe are
both part of the record.

## Acceptance criteria

- A checkout in a linked worktree exposes the same `matched_owner`, `matched_repo`, and
  `matched_host` as the equivalent normal clone.
- `prefer_local` against a linked worktree returns `locator.kind == "workspace"` and performs
  no remote fetch.
- A `prefer_local` request that matches no local checkout states that the local copy was
  skipped, and the remote result is still returned.
- The existing normal-clone and submodule-style fixtures keep their behaviour.
- `workspace_id`, trust labels, root-relative paths, stable IDs, and deterministic ordering are
  unchanged.
- No git process is spawned; the resolution is a bounded file read.
- No new tool, argument, schema property, configuration key, or dependency.
- The broad gate passes with 0 failures.

## Stop conditions

Stop and record rather than working around any of these:

- resolving the shared configuration would require running a `git` command — the resolution
  stays a bounded file read, and spawning git for this is a separate decision;
- honouring `commondir` would change the `workspace_id` derivation — identity semantics are
  fixed, and a stable-id change needs its own decision;
- the prefer-local reporting would need a new public argument — keep it in the existing
  response fields and record the constraint;
- the local precedence would conflict with the authorized host-scope model — the host-scope error
  path stays exactly as it is;
- a worktree layout appears that needs `.git` indirection beyond the stated resolution order —
  record it rather than growing the resolution into a general git-layout parser.

## Required closure evidence

`plans/closure/agentic-retrieval-integrity/041-status.md` with the exact candidate SHA, the
M037 and M040 closure references, a requirement-to-evidence matrix, the before/after
`repo_fetch` responses for the worktree root and the normal clone root, the worktree fixture,
the suite and broad-gate results, and residual risk — including that the remote fallback remains
the default when no local checkout matches.
