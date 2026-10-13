# Mode-Off Repository Locality M002 Status

Status: blocked
Recommendation: blocked pending immutable publication and provenance
Prepared candidate: `5e7b853e7eebf29c58f84959805c8774279d7ea4`
Branch: `agent/mode-off-local-repo-zero-dispatch-20261013` (pushed)
Plan: `plans/implementation/mode-off-repository-locality-corrective/002-immutable-corrected-release-and-handoff.md`

## Completed preparation

- Audited Cargo, tag, and GitHub release history: `0.4.2` is the latest
  published crate/tag/release; `0.4.3` was unused when selected.
- Prepared candidate `5e7b853` with the M001 source, version bump, changelog,
  release documentation, installation version examples, and CodeGG handoff
  contract.
- `cargo run --locked -- --version` reports `eggsearch 0.4.3`.
- Candidate local regression and source gates passed:
  - Mode-Off direct MCP zero-remote regression: 1 passed.
  - adapter tests: 94 passed.
  - provider routing: 58 passed.
  - dispatch fault injection with required `mock` feature: 34 passed.
  - `make check`: passed, 3,474 tests.
  - `make docs-check`: passed within `make release-check`.
  - `make release-candidate-check`: passed within `make release-check`.
  - `make release-check`: passed, including optimized build and
    `cargo publish --dry-run --locked`.
  - planning consistency: passed; worktree was clean at the candidate gate.

One initial `make check` attempt had a timing-sensitive local inventory test
fail (`multiple_roots_share_global_cap`); the isolated test passed, and the
subsequent full canonical and release checks passed.

## Blocking requirement

M002 WP3 is complete through the M003 qualifier. Hosted run
`38476463235` completed successfully against exact candidate
`5e7b853e7eebf29c58f84959805c8774279d7ea4`. The prepublication candidate
manifest records `v0.4.3`, that exact source revision, all seven target
artifacts, and their checksums. All seven target builds, target qualification
jobs, consumer validators, `required_gate`, and `aggregate` passed. See
`plans/closure/mode-off-repository-locality-corrective/003-status.md` for the
full hosted evidence.

M002 remains blocked at WP4. No crates.io publication, Git tag, release
workflow dispatch, draft staging, or GitHub publication has occurred. The
exact candidate is prepared and locally qualified, but immutable crate/tag/
release provenance and the release-backed zero-call fixture evidence remain
outstanding. Crates.io has no `eggsearch@0.4.3`; origin has no `v0.4.3` tag;
GitHub has no `v0.4.3` release. CodeGG C002 remains blocked and must not pin
this candidate until immutable corrected bytes exist and the consumer-owned
paired requalification passes.

## Compatibility and security

The prepared candidate preserves the ten-tool MCP surface, live-mode provider
semantics, and local-only Mode-Off authority. The M001 direct MCP fixture and
counting resolver/engine evidence remain in
`plans/closure/mode-off-repository-locality-corrective/001-status.md`.
No schema, config, storage, or migration change was made. No release state or
existing tag was mutated.
