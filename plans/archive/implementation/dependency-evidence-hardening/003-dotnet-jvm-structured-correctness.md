# Plan 003 — .NET, Maven, and Gradle Structured Correctness

Status: implementation plan

Source roadmap:
`plans/subsystems/dependency-evidence-hardening-roadmap.md`

Milestone: M003

Primary class: capability

Baseline for planning: `e0dab301cc04bf5e1e232177e2e2647262a1fdd7`

Hard dependency: M001 closed.

## Objective

Make NuGet lock evidence reflect the actual target-framework resolution graph
and replace unsafe line-oriented XML extraction for project/POM files with a
bounded structural parser. Keep Gradle extraction conservative where arbitrary
build language evaluation would otherwise be required.

## Current implementation evidence

- `parse_packages_lock_json()` expects a top-level `libraries` object with
  keys shaped `Package/Version`, which does not match ordinary NuGet
  `packages.lock.json`.
- NuGet lockfiles contain versioned `dependencies` graphs by TFM and
  optionally TFM/RID, with Direct/Transitive/Project and newer dependency
  classifications plus `requested` and `resolved` fields.
- `.csproj` scanning only recognizes same-line `PackageReference`
  attributes.
- Maven POM parsing tracks `groupId`, `artifactId`, and `version` while
  inside any `<dependencies>` block; nested `<exclusions>` can overwrite
  the parent dependency state and `dependencyManagement` is not
  distinguished.
- Gradle parsing accepts only a small quoted `group:artifact:version` subset.

## Parser dependency decision

Qualify `quick-xml` as the preferred streaming XML implementation with
default features disabled and only the minimum features needed. It is a
current, maintained pull parser with a small core dependency surface. Do not
enable async/encoding/serde features unless a concrete fixture requires them.

Record `cargo tree`, release-binary size before/after, and compile/test
results. If the measured footprint is disproportionate, stop and write a
corrective design rather than returning to fragile substring XML parsing.

## Required production changes

### 1. NuGet packages.lock.json

Parse the versioned `dependencies` object:

- retain target framework and optional runtime identifier as target context;
- package key is the package identity;
- `resolved` is exact resolved version evidence for package types that
  represent packages;
- `requested` is a requirement, not the resolved version;
- map Direct/Transitive/CentralTransitive-like types into relation without
  rejecting future/unknown types;
- treat Project entries as project/workspace provenance rather than NuGet
  registry package versions;
- retain content hash only as integrity metadata if represented by the core
  contract.

Unknown lockfile versions/types produce partial/unsupported diagnostics, not
panic or silent empty output.

### 2. .csproj PackageReference

Use streaming XML events to recognize PackageReference regardless of line
layout. Support both attribute and child-element version forms. Preserve
Condition/target context when cheaply available; do not evaluate MSBuild
expressions.

PackageReference versions are requirements. Central package version
management or property expressions that cannot be resolved from the current
file remain requirements/unknown, not exact selections.

### 3. Maven POM

Use XML structure/depth to identify actual project dependencies separately
from:

- `dependencyManagement`;
- nested `exclusions`;
- plugin dependencies;
- parent/project coordinates.

Capture scope and optionality as context/relation metadata where the core
contract supports it. Property/interpolation versions such as
`${revision}` or `${foo.version}` remain requirements/unknown. Do not
evaluate profiles or parent POMs.

### 4. Gradle lockfile

Keep exact `gradle.lockfile` coordinates as resolved lock evidence, validate
non-empty group/artifact/version tokens, and preserve deterministic
deduplication.

### 5. build.gradle / build.gradle.kts

Keep a conservative parser for literal dependency coordinates and supported
configuration names. Do not parse arbitrary Groovy/Kotlin. Dynamic/property
versions are requirements or unknown. Recognize enough common call syntax to
avoid false negatives without pretending to evaluate variables, version
catalogs, platforms/BOMs, or functions.

If version catalogs need support, defer to a separate future plan rather than
smuggling TOML catalog traversal into this milestone.

## Required tests

NuGet:

- versioned dependencies graph with two TFMs;
- TFM/RID graph;
- Direct, Transitive, CentralTransitive-like, Project, and unknown type;
- requested differs from resolved;
- no `libraries` key;
- malformed/unsupported version diagnostics.

XML:

- multiline PackageReference;
- child `<Version>`;
- conditioned reference;
- Maven exclusion nested inside dependency;
- dependencyManagement entry not emitted as direct runtime dependency;
- Maven scope/test/optional;
- property-valued version preserved as requirement;
- XML namespaces and ordinary whitespace.

Gradle:

- exact lock entries;
- literal Groovy/Kotlin dependency;
- property/dynamic version not treated as resolved;
- malformed coordinate skipped with diagnostic where applicable.

## Verification

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

Record before/after release binary bytes if `quick-xml` is added.

## Documentation updates

Document NuGet target-context semantics and Maven/.NET structural parsing in
`architecture/security.md`; record any new production dependency in the
appropriate architecture/build documentation.

## Acceptance criteria

- Real NuGet `packages.lock.json` target graphs yield exact resolved
  findings.
- Project references do not masquerade as NuGet registry packages.
- Maven exclusions/dependencyManagement cannot overwrite or become direct
  dependency findings.
- PackageReference and POM multiline XML is handled structurally.
- Unknown interpolation degrades to requirement/unknown evidence.
- Any new XML dependency passes the footprint and canonical gates.

## Stop conditions

Do not implement MSBuild, Groovy, Kotlin, Maven parent/profile resolution, or
network lookups. If correct extraction requires those, emit partial evidence
and document the limitation.
