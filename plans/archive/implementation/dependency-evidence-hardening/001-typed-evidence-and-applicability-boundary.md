# Plan 001 — Typed Dependency Evidence and Applicability Trust Boundary

Status: implementation plan

Source roadmap:
`plans/subsystems/dependency-evidence-hardening-roadmap.md`

Milestone: M001

Applicable ADR:
`plans/adrs/ADR-0004-typed-dependency-evidence-semantics.md`

Primary class: invariant + infrastructure

Baseline: `e0dab301cc04bf5e1e232177e2e2647262a1fdd7`

## Objective

Make dependency findings state what their source actually proves and make
`security_search` applicability consume only exact resolved-version evidence.
Preserve the existing serialized finding fields additively while eliminating
the current confidence escalation from manifest/checksum evidence to
High-confidence applicability.

## Readiness

ADR-0004 is accepted. No hard implementation dependency remains.

## Current implementation evidence

- `src/core/security_applicability.rs` exposes
  `DependencyFinding { ecosystem, package, version, source_file, source_line,
  source_kind, confidence, relation }`.
- `src/meta/security_search.rs` iterates every dependency finding with a
  non-empty `version`, sends it to `assess_version_applicability()`, then
  derives assessment confidence from whether advisory ranges are present.
- The resulting `ApplicabilityAssessment` currently drops the finding's
  `source_kind` and `relation` by serializing `version_source: None` and
  `dependency_relation: None`.
- Unit tests recognize that Cargo manifest strings may be ranges, but the type
  cannot preserve that distinction.

## Invariants

- Existing `security_search` request shape and tool name do not change.
- Existing `DependencyFinding` fields remain serializable.
- Unknown/unresolved evidence never becomes `NotAffected`.
- Ambiguous evidence never becomes `Affected` solely because a string happens
  to parse as a version.
- Parser/applicability code remains deterministic and passive.

## Non-goals

- Format-specific correctness beyond the minimum fixtures needed to establish
  the contract.
- Rewriting advisory range evaluation.
- Adding new ecosystems.
- Removing the legacy `version` field.

## Required production changes

### 1. Extend the dependency finding contract

Add additive typed evidence capable of representing:

- exact resolved version;
- version requirement/request;
- source/reference kind and value;
- provenance/source locator or source class;
- target/environment context;
- integrity-only/checksum observation when relevant.

Use serde defaults and skip-empty serialization. Preserve `version` as a
legacy display/compatibility projection.

The implementation may use flat optional fields or a nested typed evidence
object, but the JSON contract must remain straightforward for agents and must
not require inspecting `source_file` to interpret version semantics.

### 2. Add a parse-result seam

Introduce an internal parse result/report around the existing dispatcher so
parsers can return findings plus completeness/diagnostic information without
panicking. Keep a compatibility helper if useful for existing unit tests.

The initial diagnostic vocabulary must distinguish at least:

- complete parse;
- partial parse;
- unsupported format/version;
- malformed input.

M007 will add aggregate budgets and full diagnostic surfacing.

### 3. Gate applicability on resolved evidence

Refactor the dependency-finding loop in `security_search` so only exact
resolved-version evidence can produce ordinary range-based
`Affected`/`NotAffected` assessments.

When a matching dependency has only a requirement/reference/integrity
observation, either emit an `Unknown`/`InsufficientEvidence` assessment
with an explicit reason or preserve it only in `dependency_findings`; choose
one deterministic contract and test it. Do not silently skip in a way that
looks like negative evidence.

Explicit caller-provided package+version remains eligible as exact request
evidence.

### 4. Compose confidence rather than replace it

Add a small deterministic helper that combines dependency-evidence confidence
with advisory/range confidence. High assessment confidence requires High
confidence on both sides. Medium or Low dependency evidence cannot be promoted
to High.

### 5. Preserve provenance in assessment output

Populate `version_source` and `dependency_relation` from the matching
finding. Add an optional assessment basis/provenance field only if needed to
make resolved-versus-requirement reasoning unambiguous to clients.

### 6. Add ecosystem-aware comparison seam

Introduce one dependency-package identity helper used at the applicability
boundary. M002-M006 will fill ecosystem-specific rules. The default must be
conservative; do not globally replace punctuation or path separators.

## Ordered work packages

1. Add typed core enums/fields and serialization tests.
2. Add parse-report/internal result type and adapt the dispatcher without
   changing format behavior yet.
3. Change security applicability to use typed resolved evidence.
4. Add confidence composition and provenance propagation.
5. Add conservative identity comparison seam.
6. Migrate existing parser constructors/fixtures to compile under the new
   contract without attempting format-specific cleanup.
7. Update architecture documentation.

## Failure/recovery semantics

Missing typed resolved evidence is not a parser failure. It is weaker evidence.
Malformed/unsupported reports must stay distinct from complete-empty reports.
No new panic paths are acceptable.

## Compatibility and migration

The change is additive at the serialized response level. Existing clients may
continue reading `version`, but documentation must state that it is a legacy
projection and not proof of resolution. Internal security logic switches
immediately to the typed exact field; do not retain a fallback from
`resolved_version` to legacy `version`.

## Required tests

Add focused tests covering:

- resolved lock finding -> eligible assessment;
- manifest requirement shaped like an exact number -> not treated as resolved;
- checksum/integrity observation -> not treated as resolved;
- High advisory + Medium dependency -> at most Medium assessment;
- source kind and relation propagate to assessment;
- package mismatch remains unmatched;
- legacy JSON still contains existing fields;
- typed fields round-trip through serde/schemars expectations;
- parse report distinguishes malformed, unsupported, partial, and complete
  empty cases.

Extend `security_applicability_contract` and
`security_applicability_regression` rather than creating redundant workflow
suites unless test ownership requires it.

## Verification

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked --features mock --test security_applicability_contract
cargo test --locked --features mock --test security_applicability_regression
cargo test --locked --features mock --test security_workflow
cargo test --locked --all-features
```

## Documentation updates

Update:

- `architecture/security.md` dependency finding and confidence semantics;
- `architecture/maintenance.md` parser ownership if the parse-report seam
  changes ownership wording;
- tool/schema documentation only if generated response documentation exposes
  the new fields.

## Acceptance criteria

- `DependencyFinding` can represent resolved, requirement, reference,
  provenance, and target context without filename inference.
- Applicability no longer consumes legacy `version` as proof of resolution.
- Assessment confidence cannot exceed dependency-evidence confidence.
- `version_source` and `dependency_relation` are populated for
  dependency-driven assessments.
- Existing tool/request compatibility remains intact.
- Focused and broad tests pass.

## Stop conditions

Stop and write a corrective/ADR follow-up if preserving existing serialized
fields requires keeping the unsafe applicability fallback, or if the typed
model cannot represent NuGet multi-target and source-replacement cases needed
by later milestones without another breaking redesign.

## Closure evidence required

Record the exact SHA, before/after JSON examples for one resolved and one
requirement finding, confidence-composition tests, legacy serialization
evidence, focused test output, and broad gate status.
