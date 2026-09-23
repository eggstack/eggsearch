# Phase 27 — Eggress maintenance closure and qualification-contract hardening

Status: planned / ready for handoff

Planning baseline: `88110778a5f92350f7da41dc9729dd68ab41bcc9` (`eggsearch` 0.3.9 on `main`)

Depends on:

- Phase 25 implementation and Outcome B route scope;
- Phase 26 implementation candidate
  `8cfe9d873b55c1cda176c9c8f2af0182714ae431`;
- Phase 26 closure commit
  `88110778a5f92350f7da41dc9729dd68ab41bcc9`;
- successful Phase 26 egress feature qualification run `35806105807`;
- successful Phase 26 default release qualification run `35806121182`.

## Context

Phase 26 corrected and qualified the Eggress integration itself. The remaining
work is maintenance-only closure cleanup around evidence precision and CI
contract drift.

Two low-severity issues remain:

1. the Phase 26 implementation record says the canonical `make release-check`
   was run, but the same record also states that the strict
   `cargo publish --dry-run --locked` step refused the dirty working tree and
   only the `--allow-dirty` variant completed before commit. The later
   seven-target release qualification proves packaging/build integrity, but the
   record does not contain a clean-tree direct `make release-check` result;
2. `.github/workflows/egress-feature-qualify.yml` checks that its hard-coded
   seven targets exist in `packaging/release-targets.txt`, but it does not
   prove exact set equality. If a new maintained release target is added, the
   egress qualification matrix could silently omit it. Its path filters also do
   not cover every source file that can change the provider-side route
   construction seam.

These are not Eggress runtime defects. Phase 27 must not reopen the routing,
security, dependency, or packaging decisions already qualified by Phase 26.

## Objective

Close the Eggress workstream's final maintenance debt by:

- recording an unambiguous clean-tree canonical `make release-check` result;
- making the egress qualification target contract fail on both missing and
  extra targets;
- ensuring changes to the provider-route construction seam trigger the egress
  cross-target qualification workflow;
- adding deterministic workflow-contract tests/checks where practical so the
  CI maintenance guarantees do not rely on prose;
- reconciling Phase 26's evidence wording without falsifying historical
  execution;
- closing Phase 27 without changing Eggress runtime behavior.

## Non-negotiable invariants

Phase 27 must preserve all of the following:

1. no production Eggress routing changes;
2. no change to Phase 25 Outcome B traffic scope;
3. dynamic `FetchClient` targets remain direct and resolved-address pinned;
4. `egress` remains non-default;
5. no second binary SKU;
6. Eggress 1.0.8 base feature/dependency budget remains unchanged;
7. no protocol expansion;
8. no changes to retry, timeout, TLS, redirect, decompression, or SSRF
   ownership;
9. no publication of a crate, GitHub Release, or egress-enabled binary;
10. Phase 26's exact candidate qualification remains historically intact.

If a clean release gate reveals a real runtime or packaging defect, stop and
classify it separately rather than folding an unrelated implementation change
into this maintenance phase.

## Work item 1 — Freeze and verify the maintenance baseline

Record:

- exact starting SHA;
- `git status --porcelain --untracked-files=all` on the execution checkout;
- current `packaging/release-targets.txt`;
- current egress qualification workflow target matrix;
- current egress qualification workflow path filters;
- Phase 26 qualification run IDs and qualified SHA.

Confirm that the planning-only Phase 27 setup itself does not change production
code or dependency resolution.

## Work item 2 — Run the canonical release gate on a clean exact candidate

The central closure item is a direct clean-tree invocation of:

    make release-check

Requirements:

- run from a committed checkout with no tracked or untracked modifications;
- record the exact SHA and Rust toolchain;
- record that `release-candidate-check`, docs, release build, and
  `cargo publish --dry-run --locked` all completed successfully;
- do not use `--allow-dirty`;
- do not substitute the seven-target release qualification for this local
  canonical gate;
- do not edit the tree between the successful gate and recording its evidence.

If `make release-check` fails only because Phase 27's workflow/documentation
changes are not yet committed, commit the implementation candidate first, then
run the gate on that clean candidate and use a documentation-only closure commit
afterward.

If it exposes a package-content or release-validation defect, Phase 27 is
blocked until that defect is separately understood.

## Work item 3 — Make egress target-matrix validation exact

Harden `.github/workflows/egress-feature-qualify.yml` so its maintained target
set must equal the target set in `packaging/release-targets.txt`.

The contract must detect both:

- a workflow target missing from `release-targets.txt`; and
- a release target present in `release-targets.txt` but absent from the
  egress qualification matrix.

Preferred design:

- keep one canonical explicit list for the egress jobs if GitHub Actions matrix
  ergonomics require it;
- parse the first pipe-delimited column of `packaging/release-targets.txt`;
- normalize/sort both sets;
- compare exact equality;
- fail with a diagnostic listing missing and/or extra targets.

Do not merely compare counts.

Avoid introducing a second release target definition file.

If a cleaner reusable repository script is warranted, it may live under
`packaging/` and be consumed by both workflow preflight and a deterministic
test/packaging check, but keep the change proportional to this phase.

## Work item 4 — Harden workflow trigger coverage

Expand egress qualification workflow path filters so changes to the actual
provider-route construction seam cannot bypass the cross-target lane.

At minimum include the current files that can alter whether/how provider
traffic receives the Eggress Dialer:

- `src/fetch/egress.rs`;
- `src/core/config.rs`;
- the provider HTTP client construction module containing
  `build_http_client_with_egress`;
- the engine/default-provider builder containing
  `build_default_engines_with_egress`;
- the MetadataSearchAdapter construction path containing
  `new_with_egress`;
- the server-state wiring that passes `config.egress` into the adapter;
- `tests/egress_routing.rs`;
- relevant static-guard tests;
- `Cargo.toml` / `Cargo.lock`;
- `packaging/release-targets.txt`;
- the qualification workflow itself.

Prefer precise file paths over overly broad `src/**` triggering.

Because module locations may change in later maintenance, add a small
contract test or source check that verifies the known route-construction
symbols remain covered by one of the workflow paths, or document why such a
test would be more brittle than useful.

## Work item 5 — Add deterministic qualification-contract coverage

Add repository-local coverage for the CI contract where feasible.

Required proof:

1. an exact-set validation fixture or script fails when:
   - one maintained target is omitted from the egress set;
   - one extra target is added to the egress set;
2. the real current target sets compare equal;
3. the egress workflow remains non-publishing and runs
   `cargo check --locked --features egress` for each maintained target;
4. the workflow retains the Rust 1.89 all-features job;
5. no egress-enabled release artifact/upload job is introduced.

This may be implemented as:

- a focused shell/Python-free packaging check integrated with
  `make packaging-check`; or
- a Rust/static-guard test that parses the relevant text contracts.

Favor a small shell implementation if that naturally matches the existing
`packaging/check-contract.sh` conventions.

Do not add YAML parser dependencies solely for this phase unless they clearly
reduce maintenance burden.

## Work item 6 — Re-run the hardened egress qualification lane

After hardening the workflow, run it against the exact committed implementation
candidate.

Record:

- workflow run ID/URL;
- exact qualified SHA;
- preflight exact-set result;
- all seven maintained target results;
- MSRV all-features result.

All current maintained targets must pass.

This is compile qualification only. Do not create release artifacts or another
binary SKU.

## Work item 7 — Decide whether default release qualification must be rerun

The intended Phase 27 implementation changes CI/packaging validation and
documentation only.

If no production source, Cargo dependency/feature, release-target asset
contract, installer, updater, or release build logic changes, the successful
Phase 26 default qualification run `35806121182` remains valid for production
runtime contents.

If implementation touches any canonical release behavior beyond validation
preflight, rerun:

    release-binaries.yml
    mode = qualify
    ref = <exact implementation SHA>

Record the decision explicitly.

Do not reflexively rerun the expensive release matrix for a workflow path-filter
change alone; do rerun it if the release contract itself changes.

## Work item 8 — Correct Phase 26 evidence wording without rewriting history

Preserve Phase 26's implementation record as historical evidence, but append a
maintenance clarification stating:

- the dirty-tree strict publish dry-run was not complete evidence for the
  canonical clean-tree `make release-check`;
- Phase 27 exists to supply that exact clean-tree proof;
- all Phase 26 runtime and cross-target qualification claims remain valid;
- Phase 27 does not change the Phase 26 implementation candidate's runtime
  semantics.

Do not silently edit the original bullet into claiming a command ran differently
than it did.

## Work item 9 — Run normal repository gates

On the Phase 27 implementation candidate, run as applicable:

    cargo fmt --check
    cargo clippy --locked --all-targets --all-features -- -D warnings
    cargo check --locked --no-default-features
    cargo test --locked --all-features
    RUSTDOCFLAGS="-D warnings" cargo doc --locked --all-features --no-deps
    make hygiene
    make packaging-check
    make bench-check
    make release-check

The clean-tree `make release-check` is mandatory and must be the exact
candidate recorded in the implementation record.

If workflow-only changes do not exercise Rust code, do not omit the canonical
gate; this phase exists specifically to remove ambiguity about that evidence.

## Work item 10 — Close with exact evidence

Append a Phase 27 implementation record containing:

- starting SHA;
- implementation SHA;
- exact files changed;
- statement that runtime Eggress behavior did not change;
- before/after workflow trigger list;
- exact-set validation implementation and negative-test evidence;
- clean-tree `make release-check` result including strict
  `cargo publish --dry-run --locked`;
- hardened egress qualification workflow run ID and per-target result;
- Rust 1.89 result;
- whether default release qualification was rerun and why;
- Phase 26 maintenance clarification;
- final planning/AGENTS registry reconciliation;
- deviations/blockers.

Only then mark Phase 27 `implemented`.

## Non-goals

Phase 27 does not:

- change `src/fetch/egress.rs` runtime behavior;
- expand routed traffic classes;
- alter provider semantics;
- change Eggfetch/Eggress versions;
- change release binary features;
- publish anything;
- add a release target;
- remove a release target;
- create feature-specific artifacts;
- reopen the SSRF design;
- refactor the broader CI/release architecture.

## Suggested execution order

    1. freeze baseline + inspect target/trigger contracts
    2. implement exact target-set comparison
    3. extend egress workflow route-seam path triggers
    4. add deterministic contract tests/checks
    5. commit implementation candidate
    6. run clean-tree make release-check on that exact SHA
    7. run egress-feature-qualify on that exact SHA
    8. decide whether default release qualification remains reusable
    9. append Phase 26 maintenance clarification
    10. append Phase 27 implementation record + close registry/AGENTS

## Acceptance criteria

Phase 27 is complete only when all applicable items are true:

1. No Eggress runtime, route-scope, security, retry, TLS, decompression, or
   dependency behavior changes.
2. `make release-check` passes on a clean committed exact candidate.
3. The recorded canonical gate includes successful strict
   `cargo publish --dry-run --locked` without `--allow-dirty`.
4. Egress qualification preflight compares exact target-set equality against
   `packaging/release-targets.txt`.
5. A missing egress matrix target fails the contract check.
6. An extra egress matrix target fails the contract check.
7. The current seven-target sets compare equal.
8. Workflow path filters cover every current provider-route construction seam
   plus config, adapter, server wiring, tests, Cargo files, release-target
   contract, and workflow self-change.
9. Static/packaging coverage protects the target/trigger contract from silent
   drift where practical.
10. The hardened egress qualification workflow passes all seven maintained
    targets on the exact candidate.
11. The Rust 1.89 all-features qualification job passes on the exact candidate.
12. The workflow remains non-publishing and creates no egress release SKU.
13. Phase 26 receives an append-only maintenance clarification rather than
    falsified historical evidence.
14. The decision to reuse or rerun default release qualification is recorded
    with a concrete rationale.
15. Normal repository correctness/docs/packaging gates pass.
16. Phase 27 implementation evidence, registry status, and `AGENTS.md` are
    reconciled together.
17. No crate or GitHub Release is published.

## Handoff notes

This is intended to be the terminal housekeeping pass for the Eggress
integration workstream.

Keep the implementation small. The desired diff is CI/packaging-contract tests,
workflow path coverage, and evidence documentation. If runtime Rust networking
code starts changing, the pass has exceeded its scope.

The clean-tree release gate is the most important evidence correction. The
exact target-set comparison is the most important forward-maintenance
correction.
