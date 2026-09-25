# Dependency Evidence Hardening M004 — Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/dependency-evidence-hardening/004-ruby-composer-lock-provenance.md`

Source subsystem roadmap:

- `plans/subsystems/dependency-evidence-hardening-roadmap.md`

Repository baseline reviewed: `e0dab301cc04bf5e1e232177e2e2647262a1fdd7`

Hard dependency: M001 closed (`bce32e7`).

Implementation commit:

- `fff47bf` M004: Ruby and Composer lock provenance

## 1. Executive finding

M004 is complete. Bundler lockfiles distinguish resolved specs from
child constraints with source provenance and direct marking; Composer
locks preserve source/dist provenance with dev-version distinction and
dev-package context. No Gemfile evaluation, no network access, no new
dependencies.

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| Bundler state machine | `ruby.rs`: GEM/GIT/PATH sections, 4-space spec rows, subkey metadata | pass | Bare + colon section headers |
| Child constraints excluded | 6-space rows never parsed; dedicated regression | pass | `benchmark (>= 0.3)` case covered |
| GIT/PATH provenance | Remote + revision/branch/ref preserved | pass | Registry certainty never inherited |
| Direct marking | Two-pass DEPENDENCIES collection; others Transitive | pass | Deterministic; no Gemfile eval |
| Same package two sources | Both findings kept with distinct provenance | pass | No collapsing |
| Composer resolved + provenance | `composer.rs`: source/dist type/URL/reference/shasum | pass | Custom sources visible |
| Dev versions distinct | `dev version:` provenance + commit reference | pass | Not conflated with releases |
| Dev context | `packages-dev` target context | pass | Runtime/dev distinguishable |
| Empty versions | `None`, never `Some("")` | pass | Prior serialization bug fixed |

## 3. Production implementation evidence

Bundler: `benchmark (>= 0.3)` nested under `activesupport` yields no
finding; `rails`/`foo!` mark Direct; `foo` carries
`git: https://example.com/foo.git; revision: abc123; branch: main`.

Composer: `acme/dev-pkg dev-main` yields resolved `dev-main` with
`dev version: dev-main` provenance and commit reference
`abc123def456`; `acme/empty` yields `version: None`.

## 4. Verification executed

Against candidate `fff47bf`:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked dependency_parse
cargo test --locked --features mock --test security_applicability_regression
cargo test --locked --features mock --test security_workflow
cargo test --locked --all-features
```

Outcomes:

- `cargo fmt --check`: pass
- `cargo clippy --all-targets --all-features -- -D warnings`: pass
- `dependency_parse` lib filter: 74 passed, 0 failed
- `security_applicability_contract`: 50 passed, 0 failed
- `security_applicability_regression`: 24 passed, 0 failed
- `security_workflow`: 15 passed, 0 failed
- `static_guards`: 44 passed, 0 failed
- `cargo test --locked --all-features`: pass, 0 failures

## 5. Invariant review

No Bundler/Composer execution. Deterministic file-order findings.
Unevaluable metadata preserved, never guessed. Existing request/tool
compatibility intact.

## 6. Failure and recovery review

Malformed sections yield empty findings without panic. Unknown sources
retain unknown/custom provenance rather than registry certainty.

## 7. Migration and compatibility review

Additive fields only. One fix: empty Composer versions no longer
serialize `Some("")`. Legacy `version` projection otherwise unchanged.

## 8. Security review

Child constraints cannot inflate into High-confidence resolved
findings. VCS/path sources cannot silently inherit Packagist certainty.

## 9. Documentation and operations

Updated `architecture/security.md` (Bundler/Composer provenance and
relation semantics).

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| low | `BUNDLED WITH`/`RUBY VERSION` parsed as context only | Version-manager pins not findings | Accepted; out of scope |

## 11. Roadmap disposition

M004 closed. M005-M006 remain ready; M007 remains blocked on M005-M006
closure.

## 12. Registry updates

Covered in the same closure commit: M004 marked closed.
