# Plan 011 — Explicit Request Ecosystem Identity Consistency

Status: closed

Source roadmap:
`plans/subsystems/dependency-evidence-hardening-roadmap.md`

Milestone: M010 explicit-request identity corrective

Primary class: invariant + corrective capability

Baseline: `deb345b5b15f09043d3f3b6bf722e7b5ba72e363`

Applicable ADR:
`plans/adrs/ADR-0004-typed-dependency-evidence-semantics.md`

Prior closure evidence:

- M008 corrective closure:
  `plans/closure/dependency-evidence-hardening/008-status.md`
- M009 terminal closure:
  `plans/closure/dependency-evidence-hardening/009-status.md`

## Objective

Make the caller-supplied `package + version` applicability path obey the same
ecosystem-specific package identity contract that M008 established for
dependency-file findings.

The current dependency-finding path indexes and matches packages through
`canonical_package_name()` / `packages_match()`, but the explicit request
path in `src/meta/security_search.rs` still performs
`eq_ignore_ascii_case()` on package strings and falls back to
`PackageEcosystem::CratesIo` when the advisory ecosystem cannot be mapped.

This corrective milestone removes that bypass without changing parser behavior,
dependency evidence semantics, provider retrieval, or the public MCP request
shape.

## Discovered defect

At baseline `deb345b5`, explicit request applicability contains the following
independent identity path:

- `vuln_pkg.eq_ignore_ascii_case(pkg)`;
- optional ecosystem strings compared with `eq_ignore_ascii_case()`;
- advisory-range evaluation uses the first extracted range ecosystem or
  defaults to crates.io;
- the assessment ecosystem parses the advisory ecosystem and also defaults to
  crates.io.

That means a request such as a case-variant Go module path can match an
advisory even though M008 correctly made Go identity exact for dependency
findings. An unmapped advisory ecosystem can also inherit crates.io
identity/version semantics even when no Rust package is involved.

## Why prior verification did not catch it

M008's identity matrix and regression suite exercised the shared package
identity helpers and dependency-finding index. The explicit request-field
branch predates that index and was not routed through the helper, so tests
could prove the new matrix correct while leaving a second caller path on the
old case-insensitive rule.

M009 then correctly qualified the M008 candidate it was given; this defect is
a later audit finding and must remain separate historical corrective evidence.

## Invariants

- One ecosystem identity contract applies to both dependency findings and
  caller-supplied package/version requests.
- Go, crates.io, npm, Maven, RubyGems, Packagist, GitHub Actions, and OCI do not
  become case-insensitive merely because the package/version came from request
  fields.
- PyPI retains PEP 503/packaging name normalization implemented by
  `canonical_package_name()`.
- NuGet retains case-insensitive package identity.
- Unknown/unmapped ecosystems never silently inherit crates.io identity or
  version-comparison semantics.
- Existing request fields, tool names, serialized response fields, and
  dependency parser behavior remain unchanged.
- Direct request versions remain explicit caller evidence; this plan does not
  apply dependency-file provenance gating to request fields.
- Applicability remains conservative: ambiguity produces no false
  Affected/NotAffected claim.
- Deterministic assessment ordering/dedup remain unchanged unless required to
  remove an unsafe fallback.

## Non-goals

- Changing dependency-file provenance policy from M008.
- Adding or changing package ecosystems.
- Reworking advisory range syntax or ecosystem-specific version comparison.
- Changing package registry clients or provider queries.
- Adding package-manager/network resolution.
- Refactoring structured parser status handling.
- Broad cleanup of `security_search.rs`.

## Required production changes

### 1. Route explicit request identity through the shared helper

In the explicit request applicability branch:

1. Parse the advisory ecosystem with
   `PackageEcosystem::parse(vuln.ecosystem)`.
2. If the request supplies an ecosystem, parse it with the same enum parser and
   require enum equality with the advisory ecosystem.
3. Compare the requested package to the advisory package with
   `packages_match(&ecosystem, ...)`.
4. Preserve the original request package spelling in the emitted assessment.

When the request omits an ecosystem, the parsed advisory ecosystem supplies the
identity rule. Do not fall back to global case-insensitive package matching.

Prefer a small helper shared by this branch and tests if that makes the
identity decision auditable; do not introduce another normalization
implementation.

### 2. Remove the implicit crates.io fallback

Do not use `PackageEcosystem::CratesIo` as a generic fallback when:

- the advisory ecosystem is missing;
- the advisory ecosystem string is unrecognized;
- no affected range exists from which to borrow an ecosystem.

For explicit request applicability, a firm range assessment requires a mapped
advisory ecosystem. If the ecosystem cannot be mapped, preserve the
vulnerability result but do not produce a false exact Affected/NotAffected
assessment under Rust semantics.

Use the repository's existing structured warning mechanism to make the
unassessed condition visible if the current response would otherwise imply the
request was fully evaluated. Reuse an existing suitable warning code when
semantically accurate; add an additive code only if necessary.

Do not coerce an invalid caller ecosystem into another ecosystem.

### 3. Keep range assessment ecosystem-consistent

Pass the parsed advisory ecosystem to `assess_version_applicability()` and
`request_version_assessment()`.

If extracted advisory ranges contain ecosystem metadata inconsistent with the
advisory's parsed ecosystem, do not silently choose whichever happens to be
first. Either:

- treat the inconsistent range as non-authoritative/Unknown using the existing
  range extraction contract; or
- add a focused conservative guard at this boundary.

Do not broaden this milestone into a complete advisory metadata validator.
Add a regression only if such inconsistent metadata is representable by the
current model.

### 4. Add request-path regression coverage

Add focused tests that exercise the actual explicit request branch, not only
`packages_match()` in isolation.

Required cases:

- Go: advisory package `Example.com/Mod` does not match explicit request
  `example.com/mod`.
- Go: exact spelling still matches and evaluates using Go version semantics.
- PyPI: `my_package`, `my-package`, and case variants continue to match
  under the shared normalization rule.
- NuGet: case variants continue to match.
- crates.io/npm: case variants do not match under the M008 exact-identity
  policy.
- Request ecosystem supplied and different from advisory ecosystem: no exact
  applicability assessment is emitted for that advisory.
- Unknown/missing advisory ecosystem: no crates.io fallback assessment.
- Existing normal exact request path still produces the same request-field
  source and confidence behavior when identity is compatible.

Include at least one integration-level
`security_applicability_contract`/workflow fixture that would fail if
`eq_ignore_ascii_case(pkg)` is restored in the explicit branch.

### 5. Add a recurrence guard if low-maintenance

If a stable behavioral test is sufficient, prefer it over source-text
inspection. A static guard is justified only if it can narrowly prevent
reintroduction of an independent request-field package comparator without
creating brittle coupling to local variable names.

## Ordered work

1. Add failing explicit-request identity and unknown-ecosystem regressions.
2. Route advisory/request ecosystem parsing through `PackageEcosystem`.
3. Replace explicit package case folding with `packages_match()`.
4. Remove crates.io fallback behavior from this path.
5. Add/adjust structured warning behavior for unassessable ecosystem metadata
   if needed.
6. Run focused applicability/workflow tests.
7. Run canonical verification.
8. Write
   `plans/closure/dependency-evidence-hardening/010-status.md`, and in the
   same closure commit return the roadmap/registry to `closed` if all
   acceptance criteria pass.

## Compatibility and migration

No public request or response field is removed or renamed.

Behavior intentionally changes only where the previous direct-request path
contradicted the accepted ecosystem identity contract:

- case-variant explicit requests may stop matching for exact-identity
  ecosystems;
- PyPI and NuGet behavior remains normalized/case-insensitive as already
  documented;
- advisories with unmapped ecosystems no longer receive a fabricated crates.io
  assessment.

This is a correctness tightening, not an API break.

## Security effect

The change removes an identity-confusion path that can attach an advisory to a
different case-significant package identity and removes accidental Rust range
semantics from unmapped ecosystems. It cannot increase advisory certainty;
ambiguous cases become unmatched or Unknown rather than Affected/NotAffected.

No new I/O, network access, execution, or authorization surface is introduced.

## Required verification

Run at minimum:

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --features mock --test security_applicability_contract
cargo test --locked --features mock --test security_applicability_regression
cargo test --locked --features mock --test security_workflow
cargo test --locked --features mock --test static_guards
cargo test --locked --all-features
make bench-check
make check
```

The implementation does not touch dependency parsing, so a standalone parser
fuzz campaign is not required unless implementation unexpectedly changes
parser/shared evidence code. If shared parser/evidence code changes, rerun the
relevant M008 property/fuzz gates.

Hosted CI must pass on the exact closure candidate before the subsystem is
returned to closed status.

## Documentation updates

Update only where the contract changes or current docs falsely imply all
request paths already use the identity matrix:

- `architecture/security.md` — state that request-field and dependency-file
  package matching share one ecosystem identity policy;
- `architecture/testing.md` only if suite inventory/contract coverage text
  changes.

Do not churn parser, hardening, or maintenance docs without a real contract
change.

## Acceptance criteria

- No explicit request package comparison in the applicability path uses
  global `eq_ignore_ascii_case()`.
- Explicit request ecosystem matching is enum-based, not free-form
  case-insensitive string matching.
- Request package matching uses the same `packages_match()` semantics as
  dependency findings.
- Go case-distinct module paths do not match in the actual request workflow.
- PyPI and NuGet positive normalization cases remain correct.
- Exact-identity ecosystem case variants do not match.
- Missing/unmapped advisory ecosystems cannot default to crates.io for
  package identity, range evaluation, or emitted assessment ecosystem.
- Existing valid request-field applicability behavior remains otherwise
  unchanged.
- Focused tests, all-feature tests, `make bench-check`, `make check`, and
  hosted CI pass on the exact closure candidate.
- Closure record `010-status.md`, roadmap status, and registry status are
  updated together.

## Stop conditions

Create another corrective plan rather than broadening M010 if:

- fixing this requires changing `PackageEcosystem` public variants;
- an upstream provider emits materially ambiguous ecosystem metadata requiring
  provider-specific normalization;
- range evaluation itself has a separate ecosystem correctness defect beyond
  removing the crates.io fallback;
- a breaking request/response schema change is required.

## Closure evidence required

`plans/closure/dependency-evidence-hardening/010-status.md` must record:

- exact implementation and closure candidate SHAs;
- before/after explicit-request examples;
- Go negative case and PyPI/NuGet positive cases;
- unknown/mismatched ecosystem behavior;
- focused and canonical test outcomes;
- hosted CI run on the exact candidate;
- any warning-contract change;
- residual findings and final recommendation.
