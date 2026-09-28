# Repository Hardening M005 — Forge Safety Ownership Decomposition

Status: closed

Recommendation: closed

Implementation commits: `cb4048180e29cdeaffb77f2cc86eaec2a8e33b7b`, `85322ac127033bdc4058177ea09cff67417308d0`

## Requirement evidence

| Requirement | Evidence | Result |
|---|---|---|
| Preserve internal module path and host coverage | Directory-backed `src/meta/forge_adapter/` keeps `crate::meta::forge_adapter` and separates GitHub, GitLab, and Gitea/Forgejo/Codeberg execution | pass |
| Give shared policy one owner | `policy.rs` owns endpoint, DNS, address, and redirect policy; host modules cannot introduce duplicate address or redirect logic | pass |
| Give budgets and URL construction one owner | `budget.rs` owns aggregate/per-response reads; `urls.rs` owns resolved-ref permalink construction | pass |
| Prevent response/client bypass | Static guards cover every module's body-reader ownership and reject provider-local clients/policy | pass |
| Replace the monolith ceiling with per-owner ratchets | Facade 1,200 lines, policy 400, budget 300, URLs 180, GitHub 600, GitLab 500, Gitea 550; each enforced by static guards | pass |

## Verification

- `cargo test --locked --all-features --test forge_adapter --test property_forge_url --test static_guards --test provider_workstream_regression`: included in all-features `make check`; pass.
- Native forge live smoke is ignored by default and was not run; deterministic policy/property and host behavior coverage passed without credentials/network.
- `make check`, `make docs-check`, `make bench-check`: pass.
- `make release-check`: pass, including release build and publish dry-run.
- Exact-candidate GitHub CI passed: run [`36467672672`](https://github.com/eggstack/eggsearch/actions/runs/36467672672).

The split preserves provider IDs, transport ownership, budgets, entry ordering,
and resolved-commit permalink semantics. No safety finding remains open.
