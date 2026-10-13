# Repository Hardening M001 — Dependency Security and Policy Gate

Status: closed

Recommendation: closed

Implementation commits: `cb4048180e29cdeaffb77f2cc86eaec2a8e33b7b`, `85322ac127033bdc4058177ea09cff67417308d0`

## Requirement evidence

| Requirement | Evidence | Result |
|---|---|---|
| Patch quick-xml checked-attribute handling | `Cargo.toml` requires quick-xml 0.41; `Cargo.lock` resolves 0.41.0; `.csproj` iteration uses checked attributes and reports malformed/duplicate attributes | pass |
| Lock the hostile many-attribute regression | `src/meta/dependency_parse/dotnet.rs` has large distinct-attribute and duplicate-attribute cases | pass |
| Patch rustls and yanked lock entries | rustls 0.23.45; chacha20 0.10.2 | pass |
| Enforce all-feature advisories, yanked versions, licenses, and sources | `deny.toml`, pinned cargo-deny 0.20.2; `make dependency-policy` in `make check` and weekly `.github/workflows/dependency-security.yml` | pass |
| Keep waivers exact, documented, and time-bounded | Three exact RustSec IDs documented with dependency paths and review date 2026-11-27; deterministic guard limits review dates to 60 days | pass |

## Verification

- `cargo test --locked --all-features --test dependency_fixtures`: included in the all-features `make check` test run; pass.
- `cargo test --locked --all-features --test property_dependency_parse`: included in the all-features `make check` test run; pass.
- `cargo test --locked --all-features --test security_applicability_contract --test security_applicability_regression --test web_fetch_integration --test fetch_safety`: included in the all-features `make check` test run; pass.
- `cargo deny check`: advisories, bans, licenses, and sources pass; duplicate-version diagnostics remain warnings by policy.
- `packaging/check-dependency-policy.py`: pass.
- `make check`, `make docs-check`, `make bench-check`: pass.
- `make release-check`: pass, including release build and publish dry-run.
- Exact-candidate GitHub CI passed: run [`36467672672`](https://github.com/eggstack/eggsearch/actions/runs/36467672672).

## Security evidence and limitations

No reachable vulnerable advisory is waived. The three unmaintained notices
have exact-ID exceptions in `deny.toml`, scoped rationale in
`docs/dependency-security.md`, and review date 2026-11-27. PDF support remains
enabled. No migration or compatibility change was introduced.

No unresolved High/Medium dependency finding remains. Review the scoped
unmaintained entries by 2026-11-27.

## Scheduled workflow operational evidence (M010)

The dependency-security workflow's first successful scheduled execution was
run `38290328560`, event `schedule`, on head
`90e996ef6c83c969e30c30a36bc9d968a85dc00f` (2026-10-12). Its `cargo-deny`
job completed successfully and reported advisories, bans, licenses, and
sources all okay. M001's scheduled operational condition is closed through
M010; see `plans/closure/repository-hardening/010-status.md`.
