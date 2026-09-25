# Dependency Evidence Hardening M001 — Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/dependency-evidence-hardening/001-typed-evidence-and-applicability-boundary.md`

Source subsystem roadmap:

- `plans/subsystems/dependency-evidence-hardening-roadmap.md`

Applicable ADR:

- `plans/adrs/ADR-0004-typed-dependency-evidence-semantics.md`

Repository baseline reviewed: `e0dab301cc04bf5e1e232177e2e2647262a1fdd7`

Implementation commit:

- `bce32e7` M001: typed dependency evidence and applicability trust boundary

## 1. Executive finding

M001 is complete. Dependency findings state what their source proves via
typed resolved/requirement/reference/provenance/target/integrity fields,
`security_search` applicability consumes only exact resolved-version
evidence, assessment confidence is bounded by both sides, and provenance
survives into assessments. The legacy `version` field remains as a
compatibility projection and is no longer the applicability authority.

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| Typed finding contract | `src/core/security_applicability.rs` `resolved_version`, `version_requirement`, `reference_kind`/`reference_value`, `provenance`, `target_context`, `integrity_hash` | pass | Additive serde defaults, legacy `version` preserved |
| Parse-result seam | `DependencyParseReport`, `ParseStatus`, `ParseDiagnostic`, `parse_dependency_file_report()` | pass | Complete/Partial/Unsupported/Malformed distinguished |
| Gate applicability on resolved evidence | `src/meta/security_search.rs` uses `exact_version()`; weak evidence emits explicit `Unknown` | pass | No silent skip; stable `weak_evidence_ignored_for_exact_applicability` warning |
| Confidence composition | `compose_confidence()`, `advisory_confidence_for_ranges()` | pass | High requires High on both sides |
| Provenance propagation | `version_source`, `dependency_relation` populated; `RequestField` for caller version | pass | Assessment builders in `src/meta/advisory_range.rs` |
| Identity seam | `canonical_package_name()`, `packages_match()` | pass | PyPI canonicalization; conservative case-insensitive elsewhere |
| Parser migration | All ecosystem constructors populate typed fields; `go.sum` demoted to integrity observation | pass | Minimal migration only; format cleanup deferred to M002-M006 |

## 3. Production implementation evidence

Before (Cargo.toml `serde = "1.0.193"`):

```json
{"ecosystem": "crates_io", "package": "serde", "version": "1.0.193", "source_kind": "manifest", "confidence": "medium"}
```

After (same input):

```json
{"ecosystem": "crates_io", "package": "serde", "version": "1.0.193", "source_kind": "manifest", "confidence": "medium", "version_requirement": "1.0.193"}
```

`exact_version()` is `None`; applicability yields `Unknown` with reason
`has only a version requirement` and warning
`weak_evidence_ignored_for_exact_applicability`.

Before (Cargo.lock `serde 1.0.193`):

```json
{"ecosystem": "crates_io", "package": "serde", "version": "1.0.193", "source_kind": "lock_file", "confidence": "high"}
```

After (same input):

```json
{"ecosystem": "crates_io", "package": "serde", "version": "1.0.193", "source_kind": "lock_file", "confidence": "high", "resolved_version": "1.0.193"}
```

`exact_version()` is `Some("1.0.193")`; ordinary range-based
`Affected`/`NotAffected` assessment is eligible with composed confidence
and populated `version_source`/`dependency_relation`.

Assessment construction was extracted from `src/meta/security_search.rs`
into `src/meta/advisory_range.rs` (`primary_advisory_id`,
`request_version_assessment`, `resolved_finding_assessment`,
`weak_evidence_reason`, `weak_evidence_assessment`) to hold the
`orchestration_module_size_ratchet` ceiling (81,143 bytes vs 88,000).

## 4. Verification executed

Against candidate `bce32e7`:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked --features mock --test security_applicability_contract
cargo test --locked --features mock --test security_applicability_regression
cargo test --locked --features mock --test security_workflow
cargo test --locked --lib dependency_parse
cargo test --locked --all-features --test static_guards
cargo test --locked --all-features
```

Outcomes:

- `cargo fmt --check`: pass
- `cargo clippy --all-targets --all-features -- -D warnings`: pass
- `security_applicability_contract`: 50 passed, 0 failed
- `security_applicability_regression`: 24 passed, 0 failed
- `security_workflow`: 15 passed, 0 failed
- `dependency_parse` lib filter: 39 passed, 0 failed
- `static_guards`: 44 passed, 0 failed (includes `orchestration_module_size_ratchet`)
- `cargo test --locked --all-features`: pass (3241 lib + all suites, 0 failures)

Note: bare `cargo test --locked dependency_parse` without `--lib` builds
mock-gated integration targets and fails to compile by design; the
canonical evidence above uses `--lib` and `--features mock` forms.

## 5. Invariant review

Tool name and request shape unchanged. `DependencyFinding` fields remain
serializable with additive evolution. Unknown/unresolved evidence never
becomes `NotAffected`. Ambiguous evidence never becomes `Affected` from
string shape. Parser/applicability code remains deterministic and passive;
no package manager executed.

## 6. Failure and recovery review

Missing typed resolved evidence is weaker evidence, not parser failure.
Malformed/unsupported reports stay distinct from complete-empty reports.
No new panic paths introduced; assessment builders are pure functions.

## 7. Migration and compatibility review

Additive serialized response. Existing clients may keep reading `version`
with documentation stating it is a legacy projection. Internal security
logic switches to `resolved_version` with no fallback to legacy `version`.

## 8. Security review

Fail-safe direction: manifest constraints, checksum observations, and
mutable references lose applicability confidence instead of gaining false
precision. `go.sum` entries carry `integrity_hash` and Medium confidence
and are ineligible for range assessment. Weak evidence is explicit
`Unknown`, never silent absence.

## 9. Documentation and operations

Updated `architecture/security.md` (typed semantics, confidence
composition, identity rules, parse-report seam) and
`architecture/maintenance.md` (parse-report ownership wording).

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| low | Per-format malformed/partial detection still minimal | Format milestones must fill diagnostics | M002-M006 per-plan fixtures |
| low | NuGet/JS/Yarn-modern lock shapes still legacy | Covered by later milestones, not this boundary | M003/M005 |

## 11. Roadmap disposition

M001 closed. M002-M006 unblocked (hard dependency satisfied); M007
remains blocked on M002-M006 closure.

## 12. Registry updates

Covered in the same closure commit: M001 marked closed; M002-M006 moved
from blocked to ready; M007 stays blocked on M002-M006.
