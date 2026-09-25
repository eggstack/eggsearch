# Dependency Evidence Hardening M002 — Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/dependency-evidence-hardening/002-core-ecosystem-and-dispatch-correctness.md`

Source subsystem roadmap:

- `plans/subsystems/dependency-evidence-hardening-roadmap.md`

Repository baseline reviewed: `e0dab301cc04bf5e1e232177e2e2647262a1fdd7`

Hard dependency: M001 closed (`bce32e7`, closure
`plans/closure/dependency-evidence-hardening/001-status.md`).

Implementation commit:

- `e1464f6` M002: Cargo/Go/Python structured correctness and cross-platform dispatch

## 1. Executive finding

M002 is complete. Cargo, Go, and Python parsing now use the typed M001
contract with structured grammars instead of line heuristics: Cargo via
the existing `toml` crate, Go with minimum-requirement/indirect/replace
semantics plus vendored-manifest support, Python with a PEP 508 subset
and PyPI canonicalization. Dispatch is path-safe on Windows and Unix,
and `vendor/modules.txt` is recognized only under a `vendor` parent.
`go.sum` is integrity-only evidence. No package manager is executed and
no new runtime dependency was added (bounded local parsers only).

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| Cross-platform dispatch | `dispatch_basename()` + `is_go_vendor_manifest()`; Unix/Windows parity table test | pass | Backslash fallback; bare `modules.txt` stays `Unsupported` |
| Cargo.toml structured | `cargo.rs` via `toml`; deps/dev-build/target tables; rename/workspace/git/path handling | pass | Exact-looking strings stay requirements |
| Cargo.lock structured | `cargo.rs` via `toml`; `source` preserved as provenance | pass | `source_line` now `None` (prefer none over wrong) |
| Go mod requirements | `go.rs` two-pass; `// indirect` relation; `replace` provenance | pass | `Manifest`/Medium; replace-to-path typed |
| go.sum demotion | Integrity observation, no resolved evidence | pass | Regression held from M001 |
| vendor/modules.txt | `parse_vendor_modules()`; `## explicit` direct marking | pass | Exact resolved evidence when present |
| Python requirements | PEP 508 subset: exact/wildcard/arbitrary equality, ranges, `~=`, direct URL, extras, markers | pass | Markers/extras as target context, never evaluated |
| PyPI canonicalization | Comparison-time canonicalization; display preserved | pass | Extends M001 contract tests |
| Poetry/uv/Pipfile | `python_locks.rs` via `toml`/`serde_json`; source/path/git/index provenance | pass | New module split under existing ceilings |
| No new runtime deps | Bounded local parsers; `toml`/`serde_json` already in graph | pass | No `cargo tree`/footprint campaign needed |

## 3. Production implementation evidence

Cargo rename: `[dependencies] foo = { package = "real-crate", version = "1.2.3" }`
yields `package: real-crate`, `version_requirement: 1.2.3`,
`provenance: alias: foo`, no resolved evidence.

Go replace: `replace github.com/b/mod => ../local/b` yields the require
finding with `provenance: replace => ../local/b` and a path reference
instead of silent public-module attribution.

Python: `requests[security]>=2.28.0; python_version > "2.7"` yields
`package: requests`, `version_requirement: >=2.28.0`,
`target_context: extras: security,socks; marker: ...`, no resolved
evidence. `flask==2.3.2` keeps legacy `version: 2.3.2` with
`version_requirement: ==2.3.2` (full specifier text; M001 regression
expectation updated accordingly).

Python was split into `python.rs` (requirements) and `python_locks.rs`
(poetry/uv/pipfile) to hold the 400-line per-file ratchet; ownership and
ceilings updated in `architecture/maintenance.md` and
`tests/static_guards.rs`.

## 4. Verification executed

Against candidate `e1464f6`:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked --features mock --lib dependency_parse
cargo test --locked --features mock --test security_applicability_contract
cargo test --locked --features mock --test security_applicability_regression
cargo test --locked --features mock --test security_workflow
cargo test --locked --all-features --test static_guards
cargo test --locked --all-features
```

Outcomes:

- `cargo fmt --check`: pass
- `cargo clippy --all-targets --all-features -- -D warnings`: pass
- `dependency_parse` lib filter: 59 passed, 0 failed
- `security_applicability_contract`: 50 passed, 0 failed
- `security_applicability_regression`: 24 passed, 0 failed
- `security_workflow`: 15 passed, 0 failed
- `static_guards`: 44 passed, 0 failed
- `cargo test --locked --all-features`: pass, 0 failures

No Python parser dependency was added, so no `cargo tree`/release-size
campaign was required.

## 5. Invariant review

No package-manager execution. File byte/root safety unchanged. No second
TOML implementation (`toml 0.8` reused). Unevaluable syntax preserved as
requirement/context, never guessed. Finding order deterministic (sorted
table iteration; stable source order elsewhere).

## 6. Failure and recovery review

Malformed TOML/JSON yields empty findings without panic. Unknown
`modules.txt` locations yield `Unsupported`, not empty-complete.
Workspace inheritance yields Low-confidence unknown requirements.

## 7. Migration and compatibility review

Additive fields only. Two intentional refinements of M001-new fields:
`version_requirement` for `==` pins now carries full specifier text
(`==2.3.2`); structured TOML locks report `source_line: None`.
Both are covered by updated tests; legacy `version` projection unchanged.

## 8. Security review

`go.sum` stale versions cannot produce `Affected`/`NotAffected`
(parser-level regression plus applicability gating on `exact_version()`).
Replaced/vendored provenance prevents public-registry misattribution.
MVS is not computed; markers/expressions are not evaluated.

## 9. Documentation and operations

Updated `architecture/security.md` (Go/Cargo/Python evidence semantics)
and `architecture/maintenance.md` (Python split ownership + ceilings).

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| low | Cargo `[patch]`/version-catalogs not modeled | Rare override syntax stays unparsed | Future plan if needed |
| low | Go pseudo-version/MVS not computed | Vendored/requirement evidence only | By design; documented |

## 11. Roadmap disposition

M002 closed. M003-M006 remain ready; M007 remains blocked on M003-M006
closure.

## 12. Registry updates

Covered in the same closure commit: M002 marked closed.
