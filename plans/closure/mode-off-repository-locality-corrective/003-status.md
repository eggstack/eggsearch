# Mode-Off Repository Locality M003 — Non-Publishing Exact-SHA Eggpack Qualifier

Status: closed
Recommendation: closed
Implementation: `fcb1fb52a72af7d03e69d5c9069d97ca939f302d`
Planning registration: `36954e5cf9b32aa82fc3979371541d4bf1217515`
Prior status reconciliation: `5bf66ad1e38902d8f7599480a57b7e1aa96212bc`
Closure evidence commit: recorded in the commit that updates this record
Plan: `plans/implementation/mode-off-repository-locality-corrective/003-prepublication-exact-sha-qualifier.md`
Related release: M002, `plans/implementation/mode-off-repository-locality-corrective/002-immutable-corrected-release-and-handoff.md`

## Result

The deterministic prepublication qualifier is available on the default branch
and completed a hosted exact-SHA qualification of the M002 candidate. The
workflow used read-only repository permissions and produced only run-scoped
artifacts. It did not publish a crate, create or move a tag, or create or
mutate a GitHub Release.

## Requirement-to-evidence matrix

| Requirement | Evidence | Result |
|---|---|---|
| Deterministic generated workflow and producer drift | `python3 packaging/gen-eggpack-qualify-workflow.py --check`; `make producer-drift` | Pass; the existing generated producer remained unchanged |
| Producer job, matrix, action pin, and qualification contracts preserved | `make packaging-check`; `packaging/check-eggpack-qualifier.py` | Pass; 25 non-staging jobs across all seven targets preserved, staging omitted |
| Workflow input, exact checkout, and source/package identity | Hosted run `38476463235`; source `5e7b853e7eebf29c58f84959805c8774279d7ea4`; package version `0.4.3` | Pass; `preflight` and `resolve` succeeded |
| Seven native target builds | Hosted run `38476463235` | Pass: `build_armv7_unknown_linux_gnueabihf`, `build_aarch64_unknown_linux_gnu`, `build_x86_64_pc_windows_msvc`, `build_x86_64_unknown_linux_gnu`, `build_aarch64_pc_windows_msvc`, `build_aarch64_apple_darwin`, and `build_x86_64_apple_darwin` |
| Target qualification and consumer validation | Hosted run `38476463235` | Pass for all seven target qualification jobs and all seven consumer validation jobs |
| Required gate and final aggregate | Hosted run `38476463235` | Pass: `required_gate` and `aggregate`; aggregate outcome `complete` |
| Candidate output identity and inventory | Run artifact `eggpack-candidate-qualified/release-manifest.json` | Pass; manifest identifies product `eggsearch`, release identity `v0.4.3`, exact source revision above, and all seven target binaries and checksums |
| Offline repository gates on the exact candidate | M002 exact-candidate record `plans/closure/mode-off-repository-locality-corrective/002-status.md` | Pass; `make check`, docs, release-candidate, release, and focused zero-remote regression gates passed at the same candidate SHA |
| No publication or release-state mutation | `git ls-remote --tags origin refs/tags/v0.4.3`; `gh release view v0.4.3`; `cargo info --registry crates-io eggsearch@0.4.3` | Pass; no tag, GitHub Release, or crates.io package exists for `0.4.3` |

## Hosted run details

- Run: [38476463235](https://github.com/eggstack/eggsearch/actions/runs/38476463235)
- Event/ref: `workflow_dispatch` from `main`
- Workflow definition SHA: `f386b07d7adaa06bbb4a51a9d7e2eecf971c238b`
- Candidate SHA: `5e7b853e7eebf29c58f84959805c8774279d7ea4`
- Conclusion: `success`
- Run artifacts: retained by the workflow for seven days; downloaded during closure review to `/tmp/eggsearch-qualify-38476463235`
- Candidate manifest SHA-256: `b9cf637793a6bf08176d229ea4c6e4bd280326e3f48f39b9abfe52f4cd9fb6e5`

GitHub emitted Node.js 20 deprecation notices for pinned actions and an
unexpected `if-no-files-found` input warning from the producer-derived
download-artifact steps. These warnings did not fail any target, validator,
required-gate, or aggregate job. The workflow's existing static and producer
drift contracts passed.

## Compatibility, security, and limitations

The qualifier adds release-qualification infrastructure only. The seven
target inventory, installed package, runtime behavior, MCP surface, and
generated release producer are unchanged. The qualification verifies source
and target artifacts; it does not publish immutable release bytes or attest a
GitHub Release. M002 must still complete its publication and release-provenance
work package. CodeGG C002 remains consumer-owned and blocked until the
immutable corrected eggsearch release exists and the paired process test is
run against it.

No unresolved M003 correctness or security findings remain. M003 is closed;
M002 is unblocked for its remaining release work.
