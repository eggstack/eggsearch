# Dependency Evidence Hardening M005 — Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/dependency-evidence-hardening/005-javascript-lockfile-modernization.md`

Source subsystem roadmap:

- `plans/subsystems/dependency-evidence-hardening-roadmap.md`

Repository baseline reviewed: `e0dab301cc04bf5e1e232177e2e2647262a1fdd7`

Hard dependency: M001 closed (`bce32e7`).

Implementation commit:

- `be389a0` M005: npm/Yarn/pnpm lockfile modernization

## 1. Executive finding

M005 is complete. npm v1-v3, Yarn Classic plus the supported Berry
shape, and pnpm v6/v9 produce deterministic resolved evidence with
workspace/link/non-registry provenance preserved. Unsupported future
lock versions are explicit diagnostics. The YAML decision landed on
narrow bounded generated-format parsers (no new dependency).

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| npm lockfileVersion | `npm.rs`: 1/2/3 routed; future explicit unsupported | pass | Missing version falls back by shape |
| npm v2/v3 packages | Root excluded; scoped names; workspace/link provenance; direct marking | pass | git/tarball/file sources preserved |
| npm v1 recursion | Iterative walk, depth cap 64, sorted dedup | pass | No recursive-descent overflow |
| Yarn Classic | Grouped/scoped selectors; descriptor constraints as requirements | pass | integrity preserved |
| Berry modern | `__metadata` 5-8; `version:`+`resolution:` identity; protocol provenance | pass | Other metadata versions unsupported |
| pnpm v6/v9 | Key grammars; peer-suffix stripping; importer direct+specifier | pass | v9 snapshots isolated |
| Quote/peer hygiene | Cross-format assertions in tests | pass | No identity contains quotes/parens |
| YAML decision | Narrow parsers; no new crate | pass | Recorded in security.md + here |

## 3. Production implementation evidence

npm v1 nested `a -> b` fixture yields both resolved findings. Berry
`mypkg@workspace:.` yields workspace provenance with no resolved
version. pnpm v9 `lodash@4.17.21` resolves Direct via importers while
`snapshots:` entries never leak peer-suffixed identities.
`lockfileVersion: 99` (npm), metadata 99 (Berry), and `5.4` (pnpm) all
yield explicit `Unsupported`.

JS split into `npm.rs`/`yarn.rs`+`yarn_berry.rs`/`pnpm.rs`+`pnpm_v9.rs`
under the 400-line ratchet.

## 4. Verification executed

Against candidate `be389a0`:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked dependency_parse
cargo test --locked --features mock --test security_applicability_regression
cargo test --locked --features mock --test security_workflow
cargo tree --locked
cargo build --locked --release
make bench-check
cargo test --locked --all-features
```

Outcomes:

- `cargo fmt --check`: pass
- `cargo clippy --all-targets --all-features -- -D warnings`: pass
- `dependency_parse` lib filter: 84 passed, 0 failed
- `security_applicability_regression`: 24 passed, 0 failed
- `security_workflow`: 15 passed, 0 failed
- `static_guards`: 44 passed, 0 failed
- `cargo tree`: no new crates (quick-xml only from M003)
- Release binary: 19,588,208 bytes (+49,456 vs M003; no new deps)
- `make bench-check`: pass
- `cargo test --locked --all-features`: pass on re-run (lib 3286/0);
  one initial sweep showed 2 transient lib failures under load with no
  failing test names captured; two subsequent full runs green.

## 5. Invariant review

No package-manager execution. Deterministic ordering (sorted maps and
names; document order otherwise). Finding order stable at truncation
boundaries (no truncation in this milestone; budgets land in M007).

## 6. Failure and recovery review

Future lock versions are explicit `Unsupported`, never optimistic
parses. Malformed JSON is `Malformed`. Shape-less inputs are
`Unsupported`, never valid-empty.

## 7. Migration and compatibility review

Additive fields only. npm v1 transitive entries now appear (previously
missed) as resolved findings — a false-negative fix. Root project
entry no longer masquerades as an external dependency.

## 8. Security review

Non-registry sources (workspace/link/git/tarball) cannot inherit npm
registry certainty. Mutable Berry descriptors stay requirements.

## 9. Documentation and operations

Updated `architecture/security.md` (JS semantics + YAML decision) and
`architecture/maintenance.md` (split ownership + ceilings).

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| low | Transient lib-test flake (2 failures, one sweep) | No persistent failure; green on rerun | Monitor; corrective plan if CI reproduces |
| low | Berry metadata beyond 8 unsupported | Future Yarn versions need a follow-up | New plan when observed |

## 11. Roadmap disposition

M005 closed. M006 remains ready; M007 remains blocked on M006 closure.

## 12. Registry updates

Covered in the same closure commit: M005 marked closed.
