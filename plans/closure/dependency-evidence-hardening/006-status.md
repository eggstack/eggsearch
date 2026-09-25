# Dependency Evidence Hardening M006 — Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/dependency-evidence-hardening/006-actions-oci-reference-semantics.md`

Source subsystem roadmap:

- `plans/subsystems/dependency-evidence-hardening-roadmap.md`

Repository baseline reviewed: `e0dab301cc04bf5e1e232177e2e2647262a1fdd7`

Hard dependency: M001 closed (`bce32e7`).

Implementation commit:

- `f0eb6a1` M006: GitHub Actions and OCI reference semantics

## 1. Executive finding

M006 is complete. Action and container references are typed by
mutability instead of collapsed into version strings. Docker `FROM`
options, stages, tags, digests, ports, and variables parse without
false exact versions, and container findings use the correct source
kind. No expression evaluation, no network lookup, no new dependency
(M005 narrow-parser decision reused for workflow YAML).

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| Action ref classification | SHA/tag/branch/expression buckets with confidence | pass | SHA immutable; tags/branches mutable |
| Reusable workflows | Full path kept + workflow provenance | pass | Still reference evidence only |
| Local/same-repo | `./`, `../`, `$/` as local findings | pass | Never external packages |
| Docker actions | `docker://` routed to OCI parser | pass | Not GitHub package evidence |
| Bounded YAML seam | Quote-aware `uses:` extraction; no expression eval | pass | M005 decision reused |
| FROM grammar | Flags, `AS` stages, tag/digest/latest/variable | pass | Stage aliases + scratch skipped |
| Dockerfile source kind | All container findings use `Dockerfile` | pass | Prior `LockFile` fixed |
| Compose parity | Shared `parse_image_reference` grammar | pass | Ports, tag+digest, quotes |
| Bounded routing | Explicit Dockerfile/compose basename rules | pass | Substring routing removed |
| Regressions | platform/digest/checkout cases | pass | All three plan cases covered |

## 3. Production implementation evidence

`FROM --platform=$BUILDPLATFORM rust:1.89 AS builder` reports package
`rust` (not the flag); `ubuntu@sha256:...` reports package `ubuntu`
with digest reference and integrity hash (not `ubuntu@sha256` plus a
version tail); `uses: actions/checkout@main` is a branch reference
with no resolved version, so it can never drive exact applicability.

## 4. Verification executed

Against candidate `f0eb6a1`:

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
- `dependency_parse` lib filter: 89 passed, 0 failed
- `security_applicability_contract`: 50 passed, 0 failed
- `security_applicability_regression`: 24 passed, 0 failed
- `security_workflow`: 15 passed, 0 failed
- `static_guards`: 44 passed, 0 failed
- `cargo test --locked --all-features`: EXIT 0, 0 failures

## 5. Invariant review

No ref/image/expression resolution over the network. Unknown and
dynamic references stay explicit unknown evidence. Deterministic
ordering preserved.

## 6. Failure and recovery review

Malformed image strings yield no finding (not a wrong finding).
Expression refs are visible unknowns, never silent skips.

## 7. Migration and compatibility review

Additive fields only. Container findings change source kind from
`LockFile` to `Dockerfile` (correction per plan) and untagged/variable
images become visible reference findings instead of silent skips.

## 8. Security review

Mutable tags/branches cannot become exact resolved-version evidence.
Stage-alias confusion eliminated. Short SHAs conservatively bucket as
mutable (only full SHAs claim immutability).

## 9. Documentation and operations

Updated `architecture/security.md` (action/OCI semantics + YAML reuse
note).

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| low | Short-SHA refs bucket as branch | Conservative under-claim, no false precision | Accepted |

## 11. Roadmap disposition

M006 closed. M001-M006 all closed; M007 unblocked.

## 12. Registry updates

Covered in the same closure commit: M006 marked closed; M007 moved to
ready.
