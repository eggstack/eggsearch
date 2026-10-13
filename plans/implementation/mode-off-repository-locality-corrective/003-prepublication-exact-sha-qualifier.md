# Mode-Off Repository Locality M003 — Non-Publishing Exact-SHA Eggpack Qualifier

Status: ready
Primary class: release-qualification infrastructure
Planning baseline: eggsearch `main` at `90e996ef6c83c969e30c30a36bc9d968a85dc00f`.
Roadmap: `plans/subsystems/mode-off-repository-locality-corrective-roadmap.md`
Normative: `plans/000-long-term-specification.md`, `plans/002-long-term-roadmap.md`, `plans/003-planning-process.md`.
Producer authority: `release/eggpack/`; `.github/workflows/release-eggpack.yml` remains generated and must only change through the producer.
Blocked consumer milestone: M002, `plans/implementation/mode-off-repository-locality-corrective/002-immutable-corrected-release-and-handoff.md`.

## 1. Objective

Add a separately dispatched, hosted Eggpack qualification path that checks out
and qualifies an exact candidate commit without requiring a Git tag, publishing
a crate, creating a GitHub Release, or staging a draft. Reuse the current
seven-target build, runtime qualification, consumer validation, required gate,
and aggregate contracts. This supplies M002 WP3's missing prepublication
evidence while preserving the existing Eggpack release producer as the sole
authority for release builds and publication.

The qualification must bind every output to a user-supplied full 40-character
commit SHA and the package version read from that commit's `Cargo.toml`. The
workflow must not create or imply a release tag. A synthetic version identity
may be supplied internally to Eggpack only if its resolver accepts it without
requiring a Git tag and all output records retain the exact source SHA.

## 2. Dependencies and boundaries

- Hard: eggpack's existing generated workflow and Eggpack CLI provide the
  current target and qualification contract; inspect their implementation
  before adapting it.
- Interface: M002 has a prepared `0.4.3` candidate at
  `5e7b853e7eebf29c58f84959805c8774279d7ea4`; rerun its exact-SHA local gates
  if its source changes.
- Operational: a hosted run must be dispatchable from the repository's
  default branch before M002 can consume the qualifier. Landing this workflow
  on a feature branch alone does not satisfy hosted acceptance.
- No changes to CodeGG, crates.io, Git tags, GitHub Release state, or the
  generated release workflow by hand.

## 3. Invariants and non-goals

- The existing release producer continues to own release tags, seven-target
  package builds, draft staging, attestations, and publication.
- The new qualifier has read-only repository permissions and no secrets or
  publication credentials.
- No job may stage a draft, create a tag, publish to crates.io, or mutate a
  GitHub Release. No registry query is needed to qualify source/build output.
- Derive the target matrix and pinned actions/tool versions from the generated
  producer workflow; fail closed if the producer shape changes unexpectedly.
- Do not weaken, omit, rename, or silently reinterpret any target build,
  runtime qualification, consumer validator, required gate, or aggregate
  contract.
- Run-scoped artifacts may be retained for review, but must be clearly
  identified as candidate qualification outputs, not release assets.

## 4. Ordered work packages

### WP1 — Prove the Eggpack resolver contract

Inspect the pinned Eggpack CLI source and producer generator. Demonstrate
whether `_resolve-release` can operate on an exact source SHA plus a
Cargo-derived version without a tag or registry state. If it cannot, stop and
record the missing upstream interface; do not simulate or mislabel a release.

### WP2 — Generate a qualification-only workflow

Add a deterministic repository-owned generator and checked-in workflow for
`workflow_dispatch` with a full candidate SHA input. It must:

1. Check out and verify exactly the requested SHA.
2. Derive the package version from that checkout and resolve the Eggpack plan
   without a Git tag or publication state.
3. Reuse the current generated producer's seven target build and qualification
   jobs, consumer validators, required gate, and aggregate.
4. Omit draft staging and every release-writing step.
5. Use only read permissions and retain outputs only as workflow-run artifacts.
6. Fail closed when the producer workflow shape changes, including job IDs,
   matrices, pins, permissions, or qualification commands.

The generator must read the generated release workflow as its source rather
than maintain a second target matrix. Do not hand-edit
`.github/workflows/release-eggpack.yml`.

### WP3 — Add static and documentation contracts

Extend packaging guards to prove deterministic generation and reject release
mutation, publication commands, contents-write permissions, tag-based
checkout, incomplete SHA validation, or missing target/qualification jobs.
Document how maintainers dispatch a candidate and how to interpret the
run-scoped outputs. Keep release docs clear that this workflow does not publish
or stage a release.

### WP4 — Validate implementation and qualify M002 candidate

Run the relevant generator-drift, packaging, planning consistency, and
workflow static checks plus the repository's required offline gates. Dispatch
the M002 candidate by its exact SHA after this workflow is available on the
default branch. Record the hosted run ID, exact source SHA, all target and
consumer job outcomes, output identity, and confirmation that no release
state changed.

## 5. Acceptance and stop conditions

M003 closes only when deterministic generation and static guards pass, all
current producer qualification jobs are preserved, and a hosted exact-SHA run
completes green without creating a tag, crate publication, or GitHub Release
mutation. The closure record must include implementation commits, generator
drift and static-guard results, run ID and job evidence, exact candidate SHA,
and release-state inspection.

Stop if Eggpack cannot resolve a candidate without a tag, if target checks
cannot be safely reused, or if qualification requires release-write authority.
In that case M003 remains blocked pending an Eggpack interface change. Do not
substitute a local run for hosted evidence or mark M002 qualified.

Closure path: `plans/closure/mode-off-repository-locality-corrective/003-status.md`.
