# Plan 001 — Dependency Security Remediation and Policy Gate

Status: closed

Closure record: `plans/closure/repository-hardening/001-status.md`.

Source roadmap:
`plans/subsystems/repository-hardening-roadmap.md`

Milestone: M001 dependency remediation and enforceable dependency policy

Primary class: infrastructure + security invariant

Planning baseline: `c49c600b76e690bb1bc52f641554cca6bf79f36f`

Hard dependencies: none.

Relevant long-term requirements:

- `plans/000-long-term-specification.md#4`
- `plans/000-long-term-specification.md#9`

Relevant ADR:

- `plans/adrs/ADR-0002-eggfetch-transport-ownership.md`

## Objective

Patch the currently demonstrated vulnerable dependency versions and establish a
repeatable repository policy that fails CI on newly introduced known
vulnerabilities or disallowed dependency sources/licenses, without changing
eggsearch's API surface, feature set, or runtime capability.

## Current evidence

At the planning baseline:

- `Cargo.toml` allows `quick-xml 0.38`; `Cargo.lock` resolves
  `quick-xml 0.38.4`.
- RUSTSEC-2026-0194 is fixed in `quick-xml >=0.41.0`.
- `src/meta/dependency_parse/dotnet.rs` calls
  `element.attributes().flatten()` in both PackageReference and Condition
  parsing, reaching the advisory's default duplicate-attribute check.
- The parser receives bounded input, but the advisory is CPU-quadratic inside
  parsing and therefore is not made safe by an I/O timeout.
- `Cargo.lock` resolves `rustls 0.23.43`.
- RUSTSEC-2026-0285 affects `rustls >=0.23.13,<0.23.45`.
- Production `eggfetch-core 0.2.0` depends on rustls through the selected
  `tls-rustls` graph.
- `Cargo.lock` also includes `ttf-parser 0.25.1` through optional lopdf;
  RUSTSEC-2026-0192 marks that crate unmaintained with no patched ttf-parser
  version.
- Routine CI has no RustSec/license/source-policy step.

Research references:

- https://rustsec.org/advisories/RUSTSEC-2026-0194.html
- https://rustsec.org/advisories/RUSTSEC-2026-0285.html
- https://rustsec.org/advisories/RUSTSEC-2026-0192
- https://rustsec.org/

## Invariants

- Dependency parsing output/provenance semantics do not change merely because
  quick-xml is upgraded.
- Duplicate XML attributes remain rejected/diagnosed according to the parser's
  existing correctness contract; do not disable duplicate checking as the
  remediation.
- TLS verification remains enabled with the same root/trust and timeout policy.
- Browser/PDF/egress feature availability is unchanged.
- A dependency-policy exception never converts a known vulnerable package into
  an undocumented pass.
- The application lockfile remains committed and authoritative for release
  builds.

## Non-goals

- Broad dependency modernization.
- Replacing quick-xml with a different parser.
- Removing lopdf/PDF support to avoid the ttf-parser maintenance notice.
- Upgrading eggfetch merely to obtain a newer transitive version if a compatible
  lock update resolves rustls cleanly.
- Duplicate-version cleanup with no security or footprint benefit.
- Dependabot/automerge policy changes.

## Required production/repository work

### 1. Patch quick-xml at the manifest boundary

Raise the direct quick-xml requirement so a vulnerable 0.38.x lock cannot
reappear. The minimum accepted version is 0.41.0; prefer the latest compatible
0.41+ release that preserves the used API on the implementation date.

Re-run the .NET and Maven parser suites and update only API-call syntax required
by quick-xml. Do not change dependency-evidence meaning, parse status, source
line semantics, or XML scope handling as part of the upgrade.

Add an adversarial fixture/test with a large count of distinct attributes on a
single relevant csproj element. The test need not benchmark wall-clock time;
its role is to lock parser correctness and ensure the upgraded checked-attribute
path is exercised.

### 2. Move rustls above the patched floor

Update `Cargo.lock` so every selected rustls 0.23 instance is
`>=0.23.45`. If Cargo selects a newer compatible 0.23 release, accept it only
after the existing local TLS/fetch tests pass.

Do not add a redundant direct production rustls dependency solely to constrain
a transitive crate. The application lockfile plus the dependency-policy gate is
the correct downstream enforcement mechanism unless eggfetch's published
constraint itself blocks a secure resolution.

### 3. Establish checked-in dependency policy

Add `deny.toml` (or the current cargo-deny configuration filename if the
tooling contract changed) with explicit policy for:

- RustSec vulnerabilities: deny;
- yanked crates: deny unless a documented exceptional compatibility case exists;
- unknown/unapproved registries and arbitrary git sources: deny;
- licenses: allow only licenses actually required by the resolved graph and
  acceptable to the project;
- duplicate versions: report/diagnose first, not a blanket failure condition.

Run the first full policy pass against `--all-features` resolution and
classify every finding.

If RUSTSEC-2026-0192 remains through lopdf/ttf-parser and no patched upstream
path exists, a temporary scoped waiver is permitted only when all of the
following are recorded in a companion dependency-security document:

- advisory ID and exact dependency path;
- why capability-preserving removal is not currently possible;
- whether affected code is optional/default;
- upstream tracking reference;
- review/expiry date no more than 60 days from the implementation commit.

Do not use wildcard advisory ignores.

### 4. Put the policy in pull-request CI

Add a lightweight dependency-security job or canonical `make` target that
runs the pinned cargo-deny policy on pull requests and main. Avoid compiling the
entire crate a second time merely to inspect metadata/advisories.

Pin any third-party action used to run cargo-deny to a full commit SHA, or pin
the cargo-deny binary version and install it with `--locked`. Record the
selected version/SHA in the workflow.

Add a scheduled security-policy run (weekly is sufficient) so a newly published
advisory can fail without a Cargo.lock change. A scheduled failure must be
visible in Actions; it does not silently mutate Cargo.lock.

### 5. Add regression guards

Add repository checks that fail if:

- quick-xml resolves below 0.41.0;
- rustls 0.23 resolves below 0.23.45;
- the dependency-security policy/config is removed from the canonical CI path;
- an advisory waiver lacks the required documented ID/reason/review date.

Prefer interrogating Cargo metadata/lock content through a small deterministic
script over fragile grep when version comparison is required.

### 6. Documentation

Update:

- `architecture/security.md` — dependency threat model and policy;
- `architecture/testing.md` / `docs/test-inventory.md` — dependency gate;
- `architecture/maintenance.md` — waiver lifecycle and ownership;
- contributor/release docs that enumerate mandatory preflight checks.

Do not rewrite dependency-evidence parser history; this milestone patches the
parser library, not its evidence model.

## Failure and recovery semantics

- Advisory database/tool failure is not a clean audit. CI should distinguish
  "policy failed to execute" from "dependency policy violation".
- If quick-xml 0.41+ causes parser semantic drift, stop rather than weakening
  existing parser assertions.
- If rustls >=0.23.45 cannot resolve with eggfetch 0.2.0, stop and create an
  explicit eggfetch-upgrade/upstream corrective plan; do not suppress
  RUSTSEC-2026-0285.
- An unavoidable informational/unmaintained warning may be time-bounded; a
  reachable High/Medium vulnerability may not be waived merely to make CI green.

## Focused verification

At minimum:

```bash
cargo tree --locked -i quick-xml
cargo tree --locked -i rustls
cargo deny check
cargo test --locked --all-features --test dependency_fixtures
cargo test --locked --all-features --test property_dependency_parse
cargo test --locked --all-features --test security_applicability_contract
cargo test --locked --all-features --test security_applicability_regression
cargo test --locked --all-features --test web_fetch_integration
cargo test --locked --all-features --test fetch_safety
make check
make docs-check
```

Also run the scheduled/PR dependency-security workflow on the exact candidate.

## Acceptance criteria

- No resolved quick-xml version is affected by RUSTSEC-2026-0194.
- No resolved rustls 0.23 version is affected by RUSTSEC-2026-0285.
- csproj/POM parser behavior remains contract-equivalent.
- A checked-in dependency policy covers advisories, sources, and licenses.
- PR/main CI and a scheduled run exercise that policy.
- Every advisory exception, if any, is scoped and time-bounded.
- Existing all-features and no-default-feature capability gates remain green.
- No public API, MCP schema, provider, browser/PDF/egress capability, or
  CodeGG contract changes.

## Stop conditions

Stop and create a corrective/upstream plan if:

- the secure quick-xml or rustls floor requires a semantically incompatible
  parser/TLS rewrite;
- cargo-deny exposes another reachable High/Medium advisory requiring more than
  a bounded version lift;
- the only way to obtain a green audit is to remove supported capability;
- the candidate cannot pass existing parser/fetch/TLS qualification.

## Closure evidence required

Create `plans/closure/repository-hardening/001-status.md` containing:

- exact implementation and closure candidate SHAs;
- before/after dependency versions and reverse-dependency paths;
- RustSec/policy output;
- waiver table with review dates, if non-empty;
- focused and canonical test results;
- hosted dependency-security + routine CI run IDs;
- explicit API/capability compatibility statement.
