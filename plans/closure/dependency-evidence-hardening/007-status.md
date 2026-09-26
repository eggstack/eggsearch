# Dependency Evidence Hardening M007 — Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/dependency-evidence-hardening/007-budgets-diagnostics-and-adversarial-qualification.md`

Source subsystem roadmap:

- `plans/subsystems/dependency-evidence-hardening-roadmap.md`

Repository baseline reviewed: `561be4a4` (M006 closure).

Hard dependency: M001-M006 closed (`bce32e7`, `e1464f6`, `0d934f5`,
`fff47bf`, `be389a0`, `f0eb6a1`).

Implementation commit:

- `18b4c03` M007: Dependency parser budgets, diagnostics, and adversarial qualification

## 1. Executive finding

M007 is complete. Every parser stage is budget-bounded, truncation and
weak-evidence states are machine-readable through stable warning codes,
and the completed multi-ecosystem surface is pinned by property tests, a
libfuzzer target (641k runs, zero crashes), realistic per-format
fixtures, and corpus regressions proving weak evidence cannot drive
exact applicability. No new production dependency; release binary
+0.4% over the M003 baseline (M004-M007 code only).

## 2. Requirement-to-evidence matrix (M001-M007 invariants)

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| Typed resolved vs requirement/reference/integrity (M001) | `DependencyFinding` fields, `DependencyParseReport` seam, `compose_confidence` | pass | Preserved; budgets truncate lists, never retype fields |
| Requirement-shaped versions never resolve (M001) | Regression suite | pass | Untouched, green |
| Cargo source/alias/workspace, Go go.sum demotion, PEP 508, dispatch parity (M002) | Ecosystem suites + `dependency_fixtures` | pass | go.sum empty-hash fix (no finding from empty digest) |
| NuGet target graph, Maven exclusion separation, quick-xml footprint (M003) | Fixtures + tree/size comparison | pass | Sole added crate still `quick-xml 0.38.4` |
| Bundler section machine, Composer source/dist (M004) | Fixtures | pass | Untouched, green |
| npm v1-v3, Yarn Classic/Berry 5-8, pnpm v6/v9, explicit unsupported (M005) | Fixtures + `max_nesting_depth` enforcement | pass | v1 recursion now budget-capped |
| Action SHA/tag/branch/expression/local, OCI tag/digest/stage/port (M006) | Suites + fixtures | pass | Untouched, green |
| File/finding budgets with boundary tests (M007) | Contract boundary suite, below/exact/above | pass | `DependencyParserBudget::standard()` constants |
| Diagnostics as stable warning codes (M007) | `parse_diagnostic_warning` mapping + contract | pass | Legacy `kind_*`/`partial` prefixes mapped |
| Property suite over parser boundary (M007) | `property_dependency_parse`, 24 passed | pass | Determinism, budgets, identity, lines, canonicalization |
| Fuzz target + campaign (M007) | `dependency_parse`, 233,055 + 408,200 runs | pass | Zero panics, zero hangs |
| Realistic multi-ecosystem corpus (M007) | 22 fixtures + mixed-evidence corpus tests | pass | Truncation visible as partial, never absence |
| Dependency graph + binary size (M007) | `cargo tree`, release build | pass | No new deps; +82,992 bytes (+0.4%) |
| `make check` on exact candidate (M007) | `make check` at `18b4c03` | pass | Exit 0; intermittent local_backend flakes noted below |

## 3. Parser warning/budget table

Budgets (`DependencyParserBudget::standard`, `core/security_applicability.rs`):

| Budget | Value | Enforced by |
|---|---|---|
| `max_files_per_request` | 32 | `cap_file_list` in request order |
| `max_findings_per_file` | 10,000 | `truncate_findings` per parser report |
| `max_findings_per_request` | 20,000 | `truncate_aggregate` in `assemble_dependency_evidence` |
| `max_diagnostics` | 32 | `truncate_report` |
| `max_diagnostic_chars` | 300 | `truncate_report` |
| `max_nesting_depth` | 64 | npm v1 recursion, YAML narrow parsers |

Warning codes (`core/warning.rs`, mapped in `parse_diagnostic_warning`):

| Code | Meaning |
|---|---|
| `dependency_parse_malformed` | Per-format malformed input; file attempted, partial evidence kept |
| `dependency_format_unsupported` | Unknown filename or future lock version; never silent |
| `dependency_parse_partial` | Truncated/partial parse; also the fallback for unknown diagnostic codes |
| `dependency_finding_budget_exceeded` | Per-file finding cap hit |
| `dependency_file_budget_exceeded` | File-list or aggregate cap hit (once per breach) |
| `weak_evidence_ignored_for_exact_applicability` | Unsupported (and malformed without native version data) excluded from exact pinning |

Fail-safe direction: truncation shortens evidence and lowers confidence;
`Unknown` is returned where comparison is impossible, never `NotAffected`.

## 4. Supported format/version matrix

| Ecosystem | Inputs | Versions / shapes |
|---|---|---|
| Cargo | `Cargo.toml`, `Cargo.lock` | Structured TOML; renames, workspace inheritance, git/path provenance |
| Go | `go.mod`, `go.sum`, `vendor/modules.txt` | mod requirements + indirect/replace; sum integrity-only; vendored explicit |
| Python | `requirements.txt`, `poetry.lock`, `uv.lock`, `Pipfile.lock` | PEP 508 subset; Poetry/uv/Pipfile exact locks |
| .NET | `packages.lock.json`, `.csproj` | Versioned target graph; structural csproj |
| JVM | `pom.xml`, `build.gradle(.kts)`, `gradle.lockfile` | POM/module separation; literal coordinates; exact lock coords |
| Ruby | `Gemfile.lock` | GEM/GIT/PATH section machine |
| PHP | `composer.lock` | packages + packages-dev with source/dist provenance |
| JavaScript | `package-lock.json`, `npm-shrinkwrap.json`, `yarn.lock`, `pnpm-lock.yaml` | npm v1-v3; Yarn Classic + Berry 5-8; pnpm v6/v9 |
| Actions | `.github/workflows/*.yml` | SHA/tag/branch/expression/local/docker typing |
| OCI | `Dockerfile*`, `docker-compose*.yml`/`compose*.yml` | FROM grammar + compose `image:` |

Each row has at least one realistic fixture under
`tests/fixtures/dependency_evidence/` (22 files) exercised by
`tests/dependency_fixtures.rs` (8 passed).

## 5. Production implementation evidence

`assemble_dependency_evidence()` collects at most five candidate
lockfiles per root (content-identity deduped), parses each under the
per-file budget, maps every report diagnostic to a stable warning code,
and truncates the aggregate — so one malformed file cannot erase valid
findings from another. Applicability matches over a per-file finding
index (`build_finding_index` + `matching_positions`) shared by grouped
advisories. The `dependency_parse` fuzz target routes arbitrary bytes
plus an ecosystem discriminant through `parse_dependency_file` and
asserts determinism, per-file budget constancy, identity/provenance
stability, and line bounds.

## 6. Verification executed

Against candidate `18b4c03`:

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --features mock --test security_applicability_contract --test security_applicability_regression --test security_workflow --test static_guards
cargo test --locked --features mock --lib dependency_parse
cargo test --locked --all-features --test property_dependency_parse --test dependency_fixtures
cargo test --locked --all-features
make bench-check
cargo build --locked --release
RUSTUP_TOOLCHAIN=nightly make fuzz-smoke
make check
```

Outcomes:

- `cargo fmt --check`: pass
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: pass
- `dependency_parse` lib filter: 89 passed, 0 failed
- `security_applicability_contract`: 55 passed, 0 failed
- `security_applicability_regression`: 24 passed, 0 failed
- `security_workflow`: 15 passed, 0 failed
- `static_guards`: 44 passed, 0 failed
- `property_dependency_parse`: 24 passed, 0 failed
- `dependency_fixtures`: 8 passed, 0 failed
- `cargo test --locked --all-features`: exit 0, 81 suites ok
- `make bench-check`: pass (compile gate)
- `make fuzz-smoke`: exit 0 — 708,838 + 602,274 + 587,195 + 408,200 runs, zero crashes
- Standalone `dependency_parse` campaign: 233,055 runs in 61 s, zero panics/hangs
- `make check`: exit 0 (fmt, clippy, feature-check, tests, hygiene, packaging-check)
- `cargo tree`: `quick-xml 0.38.4` -> `memchr` only; sole consumer is eggsearch; M007 adds zero dependencies
- Release binary: 19,621,744 bytes vs 19,538,752 at M003 (+82,992, +0.4% spanning M004-M007 code; no new crates)

Note: `make fuzz-smoke` requires a nightly toolchain (`RUSTUP_TOOLCHAIN=nightly`;
the Makefile assumes a nightly default). Nightly fuzz builds re-resolve
`fuzz/Cargo.lock` (577-line churn, no new crates); the lock was reverted
after each campaign — committed lock is unchanged.

## 7. Invariant review

No package-manager execution, no network lookup, no expression
evaluation. 1 MiB/root/safe-open boundary preserved. Deterministic
ordering at normal and truncated boundaries (stable source order kept).
Additive `DependencyFinding.version` compatibility preserved.
`ParseStatus` and warning codes are two views of one fact.

## 8. Failure and recovery review

Malformed files yield partial evidence plus a named warning, never a
wrong finding and never erased sibling evidence. Unknown codes fall
back to `dependency_parse_partial`, never to silence. Intermittent
`local_backend` cap/timeout assertions
(`small_index_cap_uses_nonzero_default_timeout`,
`multiple_roots_share_global_cap`) failed twice under full-suite load
and passed in isolation (33/33) and on retry (`make check` exit 0);
zero diff lines in `local_backend` — pre-existing load sensitivity,
unrelated to this workstream.

## 9. Migration and compatibility review

Additive fields and new warning codes only. Truncation and weak-evidence
warnings are new visible output on inputs that previously produced
unbounded or silent results; no previously exact assessment changes
value.

## 10. Documentation and operations

Updated `architecture/security.md` (budgets/diagnostics/assembly
section), `architecture/hardening.md` (23 targets, smoke set,
dependency-parse invariant), `architecture/testing.md` (78 suites, 17
property suites, 23 fuzz targets), `architecture/maintenance.md`
(budget/collection/index ownership), `docs/test-inventory.md` (new
suites), `skills/eggsearch-dev/SKILL.md` (78 suites, 23 targets, smoke
wording). Fuzz seed corpus (7 files) lives in the git-ignored live
corpus dir per existing target practice.

## 11. Residual unsupported cases

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| low | Yarn Berry outside versions 5-8 explicit unsupported | Visible, no false evidence | Revisit if upstream ships v9+ grammar |
| low | Future npm/pnpm lock versions explicit unsupported/partial | Visible, no false evidence | Per-format handling on arrival |
| low | Unknown diagnostic codes map to `dependency_parse_partial` | Conservative, never silent | None; mapping table grows with parsers |
| low | `local_backend` cap/timeout tests load-sensitive | Intermittent full-suite flakes, unrelated files | Separate hardening pass if it recurs |

## 12. Roadmap disposition

M001-M007 all closed. Workstream implementation complete; M008 handoff
checklist Gate C satisfied (budgets, diagnostics, property suite, fuzz
target, corpus, size comparison, fuzz-smoke, bench-check, check on the
exact candidate).

## 13. Registry updates

Covered in the same closure commit: M007 marked closed; workstream
marked implementation-complete.
