# Phase 9 Plan: Security Reasoning and Dependency Applicability

## Objective

Deepen `security_search` from advisory discovery into traceable package/version applicability reasoning. Agents should be able to ask whether a dependency coordinate or local dependency file appears affected by a known advisory and receive an `affected`, `not_affected`, or `unknown` answer with evidence, reasons, confidence, and limitations.

This phase must preserve a strict boundary: eggsearch can reason about advisory/package/version applicability from metadata and dependency files, but it must not claim deployment exploitability. Applicability to a package version is not the same as exploitability in a specific runtime, configuration, or threat model.

## Current baseline

The repo already has:

- `security_search` for advisory/vulnerability retrieval.
- Structured advisory identifiers such as CVE, GHSA, OSV, and RustSec.
- Package/ecosystem/version request fields.
- Package-aware search integration.
- Local workspace search and repository identity metadata.
- Explicit trust markers and untrusted external content framing.

The missing capability is deterministic applicability analysis:

- Parse local dependency/lock files.
- Extract dependency coordinates.
- Parse advisory affected/fixed ranges when available.
- Compare the requested/current version against affected/fixed ranges.
- Return applicability status with clear confidence and evidence.

## Non-goals

Do not run package managers. Do not execute project code. Do not resolve full transitive dependency graphs unless the lock file already contains resolved dependency entries. Do not download artifacts. Do not produce exploit instructions. Do not claim runtime exploitability.

Do not implement a general SAT solver for all ecosystem version constraints. Start with practical range parsers for advisory formats and exact resolved versions in lock files.

## User-facing behavior

Security applicability should answer questions like:

- “Is `openssl` crate version `0.10.64` affected by `RUSTSEC-...`?”
- “Does this `package-lock.json` include a version affected by `GHSA-...`?”
- “Search advisories for `jackson-databind` and tell me if version `2.13.4` appears affected.”
- “Given this local repo, identify dependency entries that match known advisory affected ranges.”

Responses should include:

- Applicability status: `affected`, `not_affected`, `unknown`.
- Confidence: `high`, `medium`, `low`.
- Matched package coordinate.
- Requested/current version.
- Advisory ID and source.
- Affected/fixed ranges used.
- Reason text.
- Evidence source IDs / URLs.
- Limitations and unresolved gaps.

## Proposed core types

Add or extend types in `src/core/security.rs` or a dedicated `src/core/security_applicability.rs`.

```rust
pub enum ApplicabilityStatus {
    Affected,
    NotAffected,
    Unknown,
}

pub enum ApplicabilityConfidence {
    High,
    Medium,
    Low,
}

pub struct AdvisoryRange {
    pub ecosystem: PackageEcosystem,
    pub package: String,
    pub affected_range: Option<String>,
    pub fixed_versions: Vec<String>,
    pub introduced_versions: Vec<String>,
    pub source: String,
}

pub struct DependencyFinding {
    pub ecosystem: PackageEcosystem,
    pub package: String,
    pub version: Option<String>,
    pub source_file: Option<String>,
    pub source_line: Option<u32>,
    pub manifest_kind: ManifestKind,
}

pub struct ApplicabilityAssessment {
    pub status: ApplicabilityStatus,
    pub confidence: ApplicabilityConfidence,
    pub ecosystem: PackageEcosystem,
    pub package: String,
    pub version: Option<String>,
    pub advisory_ids: Vec<String>,
    pub matched_ranges: Vec<AdvisoryRange>,
    pub reasons: Vec<String>,
    pub evidence_urls: Vec<String>,
    pub warnings: Vec<String>,
}
```

Use serde defaults and skip-empty fields for public response additions.

## Workstream 1: Advisory range extraction

### Sources

Prioritize structured sources:

1. OSV JSON.
2. GitHub Security Advisory / GHSA metadata when available.
3. RustSec advisory metadata.
4. NVD CVE data as weaker fallback because package/version mapping may be less ecosystem-specific.
5. Vendor advisories as unstructured evidence only unless range metadata is parseable.

### OSV range support

Implement OSV affected package extraction:

- `package.ecosystem`.
- `package.name`.
- `ranges` with `SEMVER`, `ECOSYSTEM`, or `GIT` types.
- events: `introduced`, `fixed`, `last_affected`, `limit`.
- explicit affected versions.

Return `Unknown` when range type cannot be compared reliably.

### RustSec support

Parse RustSec fields if already fetched as structured metadata or fixture:

- `package.name`.
- `patched` ranges.
- `unaffected` ranges.
- affected versions when represented.

### Tests

- OSV semver introduced/fixed range extraction.
- OSV multiple affected packages.
- OSV explicit version list.
- RustSec patched/unaffected range extraction.
- Unsupported range type returns unknown with warning.

## Workstream 2: Version comparison and range evaluation

### Supported version schemes

Implement practical comparators by ecosystem:

- SemVer-like: crates.io, npm, Go modules, Maven when versions are semver-ish, NuGet, RubyGems, Packagist.
- Maven non-semver versions: best-effort lexical/qualifier support only when obvious; otherwise unknown.
- Python PEP 440: use existing parser if dependency is already present; otherwise implement minimal conservative parser or defer.
- OCI image tags: exact match only unless digest/fixed metadata is exact.
- GitHub Actions refs: exact tag/ref comparison only.

If a version cannot be compared safely, return `Unknown`, not `NotAffected`.

### Range evaluator

Support common expressions:

- Introduced/fixed event intervals.
- `<`, `<=`, `>`, `>=`, `=`, exact versions.
- SemVer caret/tilde only if already needed for manifest parsing; advisory ranges usually can be normalized from source metadata.

### Tests

- Version below introduced is not affected.
- Version between introduced and fixed is affected.
- Version equal to fixed is not affected unless source semantics say otherwise.
- Multiple intervals.
- Explicit affected versions.
- Unsupported/ambiguous version string returns unknown.

## Workstream 3: Dependency and lock file parsing

### Supported files

Start with resolved lock files first:

- Rust: `Cargo.lock`, `Cargo.toml` for direct dependency names when lock unavailable.
- npm: `package-lock.json`, `npm-shrinkwrap.json`, `yarn.lock`, `pnpm-lock.yaml` where feasible.
- Python: `poetry.lock`, `requirements.txt`, `Pipfile.lock`, `uv.lock` if common format is simple.
- Go: `go.mod`, `go.sum`.
- Maven/Gradle: `pom.xml`, `gradle.lockfile`, `build.gradle` best-effort.
- NuGet: `packages.lock.json`, `.csproj` PackageReference best-effort.
- Ruby: `Gemfile.lock`.
- Composer: `composer.lock`.
- Docker/OCI: Dockerfile `FROM` lines, docker-compose image refs.
- GitHub Actions: `.github/workflows/*.yml` `uses:` entries.

### Parsing discipline

Use direct parsers for structured files where dependencies already exist. If adding dependencies, keep them small and safe. Do not shell out to package managers.

Return parser confidence:

- `high`: lock file with exact version.
- `medium`: manifest exact/pinned version.
- `low`: manifest range or best-effort text extraction.

### Tests

Use small fixtures for each ecosystem. Tests should not require network access.

Required fixture tests:

- Extract exact package and version from Cargo.lock.
- Extract npm nested dependency from package-lock.
- Extract Go module requirement from go.mod.
- Extract Maven group/artifact/version from POM fixture.
- Extract NuGet PackageReference from csproj fixture.
- Extract GitHub Actions `uses: owner/repo@ref`.
- Reject malformed files with warnings, not panics.

## Workstream 4: Security search response integration

Extend `SecuritySearchRequest` with optional applicability inputs if not already present:

```rust
pub struct SecuritySearchRequest {
    pub query: Option<String>,
    pub ecosystem: Option<PackageEcosystem>,
    pub package: Option<String>,
    pub version: Option<String>,
    pub dependency_files: Vec<DependencyFileInput>,
    pub assess_applicability: Option<bool>,
}
```

If local workspace integration is preferred, use existing local roots rather than embedding file contents into requests. Keep MCP request shape minimal and backward-compatible.

Extend response:

```rust
pub struct SecuritySearchResponse {
    ...
    pub applicability: Vec<ApplicabilityAssessment>,
    pub dependency_findings: Vec<DependencyFinding>,
}
```

Applicability should be computed only when enough structured advisory data and package/version information are available.

## Workstream 5: Suggested fetch and evidence integration

Suggested fetches should prioritize:

1. Structured advisory source used for range comparison.
2. Package registry page for the exact version.
3. Release/fixed-version notes.
4. Vendor advisory.
5. Local dependency file span, if applicable.

If local dependency file parsing identifies a match, generate structured `repo_fetch`/workspace fetch locators for the manifest/lock-file lines.

## Workstream 6: Safety and wording

Every applicability response must distinguish:

- Package/version appears affected by advisory metadata.
- Package/version appears not affected by advisory metadata.
- Could not determine applicability.
- Deployment exploitability was not assessed.

Add a standard warning when applicability is assessed:

`applicability_not_exploitability: Advisory range matching does not determine runtime exploitability or reachability.`

Do not include exploit instructions or weaponized PoCs. Existing search may surface exploit context only when explicitly requested, but applicability output should remain defensive and evidence-focused.

## Workstream 7: Documentation

Update README and MCP docs with:

- Applicability status semantics.
- Supported ecosystems/files.
- Confidence model.
- Difference between advisory applicability and exploitability.
- Examples for direct coordinate and local lock-file flows.

## Acceptance criteria

- Structured advisory ranges can be extracted from OSV and at least one ecosystem-specific source such as RustSec.
- Version comparison returns affected/not_affected/unknown conservatively.
- Dependency parsers extract exact versions from major lock files without executing package managers.
- `security_search` can return applicability assessments for direct package/version requests.
- Local dependency findings can be linked to source file spans when available.
- Unsupported/ambiguous cases return `unknown` with warnings rather than false negatives.
- Tests cover advisory ranges, version comparison, dependency parsing, and response integration.
- Documentation clearly separates applicability from exploitability.
- Formatting, clippy, and tests pass.
