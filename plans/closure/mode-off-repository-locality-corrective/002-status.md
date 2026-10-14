# Mode-Off Repository Locality M002 Status

Status: closed
Recommendation: closed
Prepared candidate: `5e7b853e7eebf29c58f84959805c8774279d7ea4`
Branch: `agent/mode-off-local-repo-zero-dispatch-20261013` (pushed)
Closure evidence commit: the commit that updates this record
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

## Exact-candidate hosted qualification

Hosted run [38476463235](https://github.com/eggstack/eggsearch/actions/runs/38476463235)
qualified exact candidate `5e7b853e7eebf29c58f84959805c8774279d7ea4` before
publication. All seven target builds, target qualification jobs, consumer
validators, `required_gate`, and `aggregate` passed. The run manifest SHA-256
is `b9cf637793a6bf08176d229ea4c6e4bd280326e3f48f39b9abfe52f4cd9fb6e5`.
The full M003 record is
`plans/closure/mode-off-repository-locality-corrective/003-status.md`.

## Immutable publication and provenance

- Crate: `eggsearch 0.4.3` is visible on crates.io; `cargo info --registry
  crates-io eggsearch@0.4.3` confirmed the published package.
- Source and tag: annotated tag `v0.4.3` peels to exact qualified SHA
  `5e7b853e7eebf29c58f84959805c8774279d7ea4`; the tag is pushed to origin.
- Public release:
  [eggsearch v0.4.3](https://github.com/eggstack/eggsearch/releases/tag/v0.4.3)
  was published 2026-10-13 after draft review. Producer run
  [38480449328](https://github.com/eggstack/eggsearch/actions/runs/38480449328)
  passed all required jobs and staged the current 19-asset inventory: seven
  target binaries, seven checksum sidecars, four installer wrappers, and
  `release-manifest.json`.
- The published manifest identifies source revision
  `5e7b853e7eebf29c58f84959805c8774279d7ea4`. Its SHA-256 is
  `52653ca6b881c68a26099d71538d472cc2b199442c154b5fd6602e9a48b6e2e0`.
  The seven binary SHA-256 values are recorded in that manifest and were
  checked against the release asset digests:

| Target | Artifact | SHA-256 |
|---|---|---|
| `aarch64-apple-darwin` | `eggsearch-aarch64-apple-darwin` | `67838b7c1161aaeb4e549c9ecb3d459c2b8527d740b3fb6556e3203acafbce8c` |
| `aarch64-pc-windows-msvc` | `eggsearch-aarch64-pc-windows-msvc.exe` | `d557710bf22fb14d02032d4530c3484d03f7a3290f6a10f5bb9bf0e949d1854d` |
| `aarch64-unknown-linux-gnu` | `eggsearch-aarch64-unknown-linux-gnu` | `c35e5d8bf97e848d38e7c675835bbb106f5d97c520a1b61ff135da9534e9ab53` |
| `armv7-unknown-linux-gnueabihf` | `eggsearch-armv7-unknown-linux-gnueabihf` | `efe1ab2430c24f271fbfcfb3764a2845d72f5d5ae7fce7dae681b854c5ab95b4` |
| `x86_64-apple-darwin` | `eggsearch-x86_64-apple-darwin` | `40dd08b6825be779cb87122b8ea2f6f87547b7ecbad72c32aa544d7a8b4e2442` |
| `x86_64-pc-windows-msvc` | `eggsearch-x86_64-pc-windows-msvc.exe` | `780dbd861ed92aa599082343901ce8a758c1df3071887224bf687221625c1af8` |
| `x86_64-unknown-linux-gnu` | `eggsearch-x86_64-unknown-linux-gnu` | `23f67a17bc5d85e4fa1685257d9368dad83a19514ee1edd35ef734196e179d08` |

- Provenance run
  [38483437078](https://github.com/eggstack/eggsearch/actions/runs/38483437078)
  passed `verify-staged-release` for all 19 assets, verified the seven target
  binaries, public wrappers, generated exact installers, and their
  attestations, and completed attestation verification. After that review the
  draft was published. Local `gh attestation verify` for the downloaded public
  `aarch64-apple-darwin` binary also exited successfully with repository and
  signer workflow constrained to this repository's provenance workflow.
- Public binary smoke: `packaging/release-smoke.sh` passed on the downloaded
  `aarch64-apple-darwin` release asset at version `0.4.3`. Hosted native
  qualification and consumer validation passed for both Windows targets.
  A local PowerShell smoke was unavailable because this host has no
  PowerShell executable.

## Release-backed zero-call and handoff evidence

The release is built from the exact M001 implementation candidate whose
counting fixtures were executed at that SHA. M001's direct MCP fixture uses
two counting engines and a separately injected package resolver; both
provider counters, the adapter engine counter, and resolver counter remain
zero. It also covers a local positive match, empty-workspace exact error,
explicit provider hints, package/security context, and local opt-out. The
release manifest and SLSA provenance bind those tests' exact source revision
to the published binaries; no live provider, registry, forge, or fetch request
was used. The detailed test matrix and execution evidence remain in
`plans/closure/mode-off-repository-locality-corrective/001-status.md`.

The consumer contract is published in `docs/codegg-integration.md` and
`architecture/codegg-contract.md`: upgrade all version owners and the
immutable binary/source pointer, use a private Mode-Off config plus local
fixture, run native `repo_search` and Mode-Off `web_search`/`security_search`
denials, and run `scripts/verify.sh quick` on the same CodeGG SHA. CodeGG C002
paired-process qualification and legacy backend retirement remain consumer-
owned and are not claimed complete.

## Compatibility and security

The published patch preserves the ten-tool MCP surface, live-mode provider
semantics, and local-only Mode-Off authority. No schema, config, storage, or
migration change was made. Existing tags and releases were not mutated. No
unresolved M002 correctness, provenance, or release findings remain. CodeGG
C002 is the sole remaining downstream gate.
