# Phase 28 — Eggress documentation and test-hygiene cleanup

Status: planned / ready for handoff

Planning baseline: `307036c2799d619dbb572ed8322021e4197facb8` (`eggsearch` 0.3.9 on `main`)

Depends on:

- Phase 25–27 implemented and closed;
- Phase 27 implementation candidate
  `6414a72ead3d059e53205892f8200afacee2f5e4`;
- hardened egress qualification run `35810222447`;
- Phase 26 default release qualification run `35806121182`.

## Context

The Eggress integration itself is closed. Phase 27 supplied the final
clean-tree release-gate evidence and hardened the qualification matrix/trigger
contract.

Two small hygiene items remain:

1. `plans/registry.md` still labels the Phase 27 section
   `Active maintenance workstream — Eggress closure evidence and qualification contract`
   even though Phase 27 is implemented and `AGENTS.md` correctly says
   phases 1–27 are implemented;
2. the targeted command

       cargo test --locked --features mock --test egress_routing \
         egress_host_validation_accepts_dns_ipv4_and_ipv6_literals

   fails even though the same test passes under `--all-features`.

The targeted-test behavior is not an IPv6 or runtime defect. The test currently
sets `egress.enabled = true`, so `AppConfig::validate()` correctly reaches the
compile-feature gate and rejects a route when the binary was built without the
`egress` feature. That feature-gating contract is separately intentional and
already covered.

The host grammar can be tested independently without changing production code:
an `EgressSection` with `enabled = false` and non-empty `hops` still runs
`validate_egress_hop()`, including `is_valid_egress_host()`, but deliberately
does not require the Cargo `egress` feature.

Phase 28 is therefore a documentation/test-hygiene pass only.

## Objective

Finish the Eggress workstream with internally consistent planning status and
feature-independent host-validation regressions, without changing any runtime
behavior or qualification contract.

The desired end state is:

- Phase 27 is presented as completed everywhere current-state planning status is
  summarized;
- hostname/IPv4/IPv6 syntax tests exercise host validation rather than the
  feature-enabled route gate;
- those host grammar tests pass under both:
  - `--features mock`; and
  - `--all-features`;
- feature-gating behavior remains separately tested;
- no production Rust logic changes;
- no new release qualification is needed unless implementation exceeds this
  intended scope.

## Non-negotiable invariants

Phase 28 must preserve:

1. all Phase 25 Outcome B routing/security decisions;
2. all Phase 26 runtime behavior;
3. all Phase 27 CI/qualification-contract behavior;
4. `EgressSection::validate()` production semantics;
5. `is_valid_egress_host()` production semantics;
6. failure when `egress.enabled = true` in a binary built without the
   `egress` feature;
7. acceptance of bare IPv4/IPv6/DNS proxy hosts when hop syntax is otherwise
   valid;
8. rejection of schemes, userinfo, paths, bracketed IPv6, and embedded ports in
   the `host` field;
9. no dependency, feature, packaging, installer, updater, or workflow-matrix
   change;
10. no crate or GitHub Release publication.

## Work item 1 — Freeze the hygiene baseline

Record:

- starting SHA;
- current Phase 27 registry section heading/status;
- current `AGENTS.md` plan summary;
- exact targeted host-validation command and failure mode under
  `--features mock`;
- the corresponding passing `--all-features` result;
- existing feature-gate regression coverage.

Do not classify the targeted-test failure as a runtime failure.

## Work item 2 — Correct the Phase 27 registry heading

Change the current-state registry heading from:

    ## Active maintenance workstream — Eggress closure evidence and qualification contract

to a completed-state heading, preferably:

    ## Completed maintenance workstream — Eggress closure evidence and qualification contract

Do not alter Phase 27's implementation status, candidate SHA, qualification run
IDs, or historical evidence.

Check for any other current-state documentation that still calls Phase 27
active after it was closed. Correct only genuine stale status language.

Historical prose describing Phase 27 as active before implementation should not
be rewritten unless it is presented as current state.

## Work item 3 — Decouple host-grammar tests from the Cargo feature gate

Update the two host-syntax tests in `tests/egress_routing.rs`:

- `egress_host_validation_accepts_dns_ipv4_and_ipv6_literals`;
- `egress_host_validation_rejects_scheme_userinfo_path_and_port_suffix`.

Preferred change:

- construct the test `EgressSection` with `enabled = false`;
- retain the non-empty configured hop;
- call the same public `AppConfig::validate()` path.

This preserves realistic config validation while isolating
`validate_egress_hop()` from the unrelated compile-feature requirement.

Do not:

- expose `is_valid_egress_host()` publicly solely for testing;
- add a production testing hook;
- weaken the feature gate;
- conditionally skip the test under `not(feature = "egress")` if the host
  grammar can be tested cleanly without doing so.

## Work item 4 — Preserve explicit feature-gate coverage

Confirm that a separate regression still proves:

- a configured and enabled egress route fails closed when compiled without
  `--features egress`; and
- the same route construction is allowed when the feature is present.

The existing
`egress_explicit_route_without_feature_fails_closed` test may remain the owner
if it accurately covers that behavior.

The hygiene change must not make enabled routing silently validate in
feature-disabled builds.

## Work item 5 — Add targeted test-matrix evidence

Run and record at minimum:

    cargo test --locked --features mock --test egress_routing       egress_host_validation_accepts_dns_ipv4_and_ipv6_literals

    cargo test --locked --features mock --test egress_routing       egress_host_validation_rejects_scheme_userinfo_path_and_port_suffix

    cargo test --locked --all-features --test egress_routing       egress_host_validation_accepts_dns_ipv4_and_ipv6_literals

    cargo test --locked --all-features --test egress_routing       egress_host_validation_rejects_scheme_userinfo_path_and_port_suffix

Also run the feature-gate regression in both relevant feature configurations
where its conditional assertions are meaningful.

The point is to prove the test now measures host syntax independently of the
optional transport feature.

## Work item 6 — Reconcile test inventory only if counts/semantics change

If no tests are added or removed, do not churn test-count documentation merely
because implementation details changed.

If a test is renamed, split, added, or removed, reconcile:

- `docs/test-inventory.md`;
- `architecture/testing.md`;
- `skills/eggsearch-dev/SKILL.md`;

according to existing repository conventions.

Prefer keeping the existing test names and count if the simple
`enabled = false` correction is sufficient.

## Work item 7 — Run normal repository gates

On the exact committed hygiene candidate run:

    cargo fmt --check
    cargo clippy --locked --all-targets --all-features -- -D warnings
    cargo check --locked --no-default-features
    cargo test --locked --all-features
    make hygiene
    make packaging-check

Also run the targeted `--features mock` host-validation commands from Work
Item 5.

A full `make release-check` is optional if and only if the final diff is
strictly tests/planning/docs and does not touch production source, Cargo
metadata, packaging behavior, workflows, release contracts, or dependencies.
Record that decision explicitly.

## Work item 8 — Qualification decision

No new seven-target egress qualification or default release qualification is
required if the final implementation changes only:

- `tests/egress_routing.rs`;
- `plans/registry.md`;
- Phase 28 planning/implementation records;
- `AGENTS.md` current-state planning text;
- directly related documentation with no build/package semantics.

If production code, Cargo metadata, workflow logic, release-target contracts,
or packaging scripts change unexpectedly, stop and reassess qualification
requirements instead of silently reusing Phase 26/27 evidence.

## Work item 9 — Close Phase 28 cleanly

Append an implementation record containing:

- starting SHA;
- implementation SHA;
- exact files changed;
- before/after targeted test behavior;
- confirmation that `EgressSection::validate()` production logic was not
  changed;
- feature-gate regression result;
- normal repository gate results;
- release/qualification reuse rationale;
- final registry/AGENTS reconciliation;
- deviations/blockers.

At closure:

- mark Phase 28 `implemented`;
- mark the Phase 28 registry workstream completed;
- ensure `AGENTS.md` states phases 1–28 are implemented;
- leave no Eggress-specific workstream labeled active unless new work has
  actually been registered.

## Non-goals

Phase 28 does not:

- alter proxy routing;
- alter IPv6 parsing;
- alter config schema;
- change feature behavior;
- change Eggress/Eggfetch versions;
- change cross-target CI;
- change release assets;
- add dependencies;
- refactor the config module;
- publish anything.

## Suggested execution order

    1. reproduce targeted --features mock failure
    2. change host-grammar test fixtures to enabled = false
    3. run targeted mock + all-features tests
    4. verify enabled-route feature-gate regression remains intact
    5. correct stale Phase 27 registry/current-state wording
    6. run normal hygiene gates
    7. record qualification reuse decision
    8. append Phase 28 implementation record + close registry/AGENTS

## Acceptance criteria

Phase 28 is complete only when all applicable items are true:

1. The Phase 27 registry section is labeled completed, not active.
2. No other current-state documentation incorrectly presents Phase 27 as active.
3. The IPv4/IPv6/DNS host-acceptance test passes under `--features mock`.
4. The invalid-host rejection test passes under `--features mock`.
5. Both host-validation tests continue to pass under `--all-features`.
6. Host grammar is tested through the public config validation path without
   exposing private production helpers solely for tests.
7. The enabled-route/no-`egress` feature gate remains fail-closed and has
   explicit regression coverage.
8. No production config/transport behavior changes.
9. No Cargo dependency/feature changes.
10. No packaging/workflow/release-contract changes.
11. Normal fmt/clippy/check/all-features test/hygiene/packaging gates pass.
12. Qualification reuse is explicitly justified if no new cross-target run is
    performed.
13. Phase 28 implementation evidence, registry status, and `AGENTS.md` are
    reconciled together.
14. No crate or GitHub Release is published.

## Handoff notes

This should be a very small diff.

The preferred test fix is not a new helper and not a feature-conditioned skip.
Use the existing behavior of disabled `[egress]` sections: configured hops are
still syntax-validated, while the compile-feature gate applies only when the
route is enabled. That gives the host grammar tests exactly the isolation they
need without changing production code.
