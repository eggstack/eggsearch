# Mode-Off Repository Locality M002 Status

Status: blocked
Recommendation: blocked
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

M002 WP3 requires exact-candidate hosted qualification before WP4 publishes the
crate. The current generated Eggpack workflow only accepts an exact existing
`release_tag`, checks out that tag, builds and validates its seven targets,
then stages the 19-asset draft. Current release documentation requires the
matching crate version to be visible before that workflow is dispatched. It
does not expose the historical non-publishing `mode=qualify` path.

The two requirements cannot both be met in their stated order with the current
workflow. No crates.io publication, Git tag, release workflow dispatch, draft
staging, or GitHub publication was attempted. No artifact digests, hosted
workflow run IDs, or release provenance are claimed for candidate `5e7b853`.

M002 remains blocked pending the maintainer's choice between a separately
registered prepublication exact-candidate qualifier and a plan-approved
sequence that publishes the crate before Eggpack builds/stages the draft,
while retaining manual final GitHub Release publication. CodeGG C002 remains
blocked and must not pin this candidate until the immutable release and
consumer-owned paired requalification exist.

## Compatibility and security

The prepared candidate preserves the ten-tool MCP surface, live-mode provider
semantics, and local-only Mode-Off authority. The M001 direct MCP fixture and
counting resolver/engine evidence remain in
`plans/closure/mode-off-repository-locality-corrective/001-status.md`.
No schema, config, storage, or migration change was made. No release state or
existing tag was mutated.
