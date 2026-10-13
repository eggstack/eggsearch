# Mode-Off Repository Locality M003 — Non-Publishing Exact-SHA Eggpack Qualifier

Status: blocked
Recommendation: blocked pending default-branch hosted qualification
Implementation: `fcb1fb52a72af7d03e69d5c9069d97ca939f302d`
Planning registration: `36954e5cf9b32aa82fc3979371541d4bf1217515`
Branch: `agent/mode-off-local-repo-zero-dispatch-20261013`
Plan: `plans/implementation/mode-off-repository-locality-corrective/003-prepublication-exact-sha-qualifier.md`
Roadmap: `plans/subsystems/mode-off-repository-locality-corrective-roadmap.md`
Related blocked release: M002, `plans/implementation/mode-off-repository-locality-corrective/002-immutable-corrected-release-and-handoff.md`

## Result

The non-publishing exact-SHA workflow, deterministic generator, fail-closed
packaging guard, and operator documentation are implemented and pushed. Local
contracts establish that it preserves the Eggpack producer's 25 non-staging
jobs across all seven targets, checks out only the requested SHA, compares
`HEAD` to that SHA, derives the internal Eggpack release identity from the
candidate `Cargo.toml`, and retains read-only repository permissions. The
release producer workflow remains generated and unchanged.

M003 is not closed. GitHub `workflow_dispatch` requires this workflow to be
available on the default branch. It is currently on the implementation branch,
so no hosted exact-SHA qualification run can yet be dispatched. The full
`make check` result is also incomplete because the bounded-command integration
suite did not return during the aggregate run, although that suite passed when
run independently. No crate, tag, GitHub draft, or publication was created.

## Requirement and evidence matrix

| Requirement | Evidence | Result |
|---|---|---|
| Deterministic qualifier derives from generated producer | `python3 packaging/gen-eggpack-qualify-workflow.py --check`; `make producer-drift` | Pass; generated release workflow and producer inputs unchanged |
| Keep all seven target build, qualify, consumer, gate, and aggregate jobs | `make packaging-check`; `packaging/check-eggpack-qualifier.py` | Pass; 25 producer jobs retained, only `stage` removed, seven targets checked |
| Workflow syntax | `python3 -c 'import yaml; from pathlib import Path; d=yaml.load(Path(".github/workflows/qualify-eggpack-candidate.yml").read_text(), Loader=yaml.BaseLoader); assert d["on"]["workflow_dispatch"]["inputs"]["candidate_sha"]["required"] == "true"; print("workflow YAML parsed; jobs:", len(d["jobs"]))'` | Pass; parsed 25 jobs |
| Exact full SHA checkout and identity | Candidate workflow inspection; YAML parse; local `eggpack ci _resolve-release` against `fcb1fb52a72af7d03e69d5c9069d97ca939f302d` using synthetic `v0.4.3` | Pass locally; resolver plan carried the exact source revision and seven targets |
| No release writes or publication | Packaging guard rejects `contents: write`, stage commands, secrets, token references, publication commands, tag refs, and implicit refs | Pass locally; all workflow/job permissions are read-only |
| Existing producer and action pin contracts | `make producer-drift`; `make packaging-check` | Pass; release workflow remained unchanged; all seven workflow files passed pin checks |
| Documentation and release candidate tree contract | `make docs-check`; `make release-candidate-check`; `git diff --check` | Pass |
| Full repository `make check` | First run interrupted after stalling in `bounded_command`; serial run passed formatting, clippy, no-default check, 3,474 library tests, 13 binary tests, adversarial corpus, and batch-fetch suite, then did not return from the bounded-command suite | Incomplete; no failure was reported before the stalled suite |
| Focused bounded-command suite | `RUST_TEST_THREADS=1 cargo test --locked --all-features --test bounded_command -- --test-threads=1 --nocapture` | Pass; 31 passed, 0 failed, 594.66 seconds |
| Hosted exact-SHA candidate run with no release state mutation | Not run; workflow is not yet available on default branch | Blocked |

## Compatibility, security, and migration

The workflow changes release qualification infrastructure and operator
sequence only. It does not change the crate's runtime behavior, MCP tools,
target inventory, release assets, or installer contract. Candidate outputs are
retained only as seven-day workflow-run artifacts. There are no secrets,
write-scoped GitHub permissions, tag checkouts, crate publication steps, or
draft-staging steps in the candidate workflow.

The local resolver check used the installed Eggpack CLI `0.1.0`; it proves the
resolver accepts a synthetic version identity bound to the exact source SHA,
but does not substitute for the pinned CLI's hosted workflow run. The full
repository test gate remains incomplete as recorded above.

## Blocker and next action

The implementation must be merged or otherwise made available on the
repository's default branch before dispatch. Then run the candidate qualifier
against an exact candidate SHA, record every target, consumer validator,
required-gate, and aggregate result, and confirm no tag, crate, or GitHub
Release state changed. Rerun the complete offline `make check` gate against
the final candidate SHA. Only then may M003 close and M002 WP3 proceed.

M002 remains blocked. No crates.io publication, Git tag, release draft, or
CodeGG handoff was made under this milestone.
