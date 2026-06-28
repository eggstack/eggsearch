# Phase 4 Plan: Package and Version-Aware Repository Retrieval

## Objective

Add package coordinate and version-aware retrieval to `repo_search` so coding agents can ask about a package/API in a concrete ecosystem and receive registry, documentation, source repository, release/changelog, migration, and advisory evidence without manually composing several searches.

The target use cases are:

- Understand an API for a specific package version.
- Find migration and breaking-change context between versions.
- Locate the source repository and docs for a package.
- Connect package/version context to security advisories and patched versions.

Start with Rust crates, Python packages, and npm packages because they are high-value for codegg and have public registry APIs or predictable metadata pages.

## Current baseline

`repo_search` already groups docs, registry, source files, issues, releases, examples, PRs, and related categories. `security_search` already supports package/ecosystem/version-oriented advisory lookup through OSV. The missing bridge is a typed package coordinate model and deterministic resolution layer that can enrich repo searches before provider fan-out.

## Non-goals

Do not vendor full package registry indexes. Do not add a long-running package metadata cache in the first pass. Do not implement package installation or dependency solving. Do not claim exact source commit mapping unless registry metadata provides it. Do not merge `security_search` into `repo_search`; instead, allow repo search to attach or suggest security context.

## Request model additions

Extend `RepoSearchRequest` and MCP args with optional package fields:

```rust
pub ecosystem: Option<PackageEcosystem>,
pub package: Option<String>,
pub version: Option<String>,
pub version_requirement: Option<String>,
pub compare_version: Option<String>,
pub include_security_context: Option<bool>,
pub include_changelog: Option<bool>,
pub include_migration_guides: Option<bool>,
```

`PackageEcosystem` should support at least:

- `crates_io` with aliases `crates.io`, `cargo`, `rust`.
- `pypi` with aliases `python`.
- `npm` with aliases `javascript`, `node`.
- `unknown` only if needed for deserialization fallback; prefer validation errors for explicit bad values.

Explicit fields should override query-parsed hints. Add query parsing for common tokens later if simple, for example `crate:axum`, `package:requests`, `ecosystem:pypi`, `version:2.31.0`. Do not overcomplicate the parser in the first pass.

## Package metadata model

Add a module such as `src/core/package.rs`:

```rust
pub struct PackageCoordinate {
    pub ecosystem: PackageEcosystem,
    pub name: String,
    pub version: Option<String>,
    pub version_requirement: Option<String>,
}

pub struct PackageResolution {
    pub coordinate: PackageCoordinate,
    pub registry_url: Option<String>,
    pub docs_url: Option<String>,
    pub source_repository_url: Option<String>,
    pub homepage_url: Option<String>,
    pub changelog_url: Option<String>,
    pub license: Option<String>,
    pub latest_version: Option<String>,
    pub resolved_version: Option<String>,
    pub published_at: Option<String>,
    pub warnings: Vec<String>,
}
```

Keep resolution metadata separate from `SourceCard` at first. Add it to `RepoSearchResponse` as optional `package_resolution` or inside a new `context` object.

## Resolver implementation

Create a `PackageResolver` abstraction with bounded HTTP calls, shared fetch client settings, and no heavyweight dependencies. It should expose `resolve_package(coordinate, timeout)`.

Initial resolvers:

### crates.io / docs.rs

Use the crates.io API or registry metadata endpoint if available through simple HTTP JSON. Resolve:

- Registry URL: `https://crates.io/crates/{name}`.
- Docs URL: `https://docs.rs/{name}/{version?}` or latest docs URL if version absent.
- Repository/homepage/changelog if present in metadata.
- Latest version and requested version if available.

If API lookup fails, produce deterministic fallback URLs but warn that metadata was not verified.

### PyPI

Use PyPI JSON endpoint for package metadata. Resolve:

- Registry URL.
- Project URLs such as homepage, source, changelog, documentation.
- Latest version and requested version existence when available.
- Published timestamp if available for chosen release.

### npm

Use npm registry package metadata. Resolve:

- Registry URL.
- Repository URL.
- Homepage/docs/changelog if available.
- Latest version and requested version existence.

Normalize repository URLs where possible into code-host metadata so existing repo search hints can be derived.

## Integration with repo planner

When package fields are supplied and resolution succeeds, repo search should generate subqueries that use the resolved context:

- Registry result query.
- Official docs query.
- Source repository query if repository URL is known.
- Release/changelog/migration query when requested.
- Issue query scoped to repository if known.
- Source-file query scoped to repository if known and residual query exists.

If the package resolver finds a source repository, merge owner/repo/host into `resolved_hints` unless explicit owner/repo was already supplied. Explicit repo hints should win over package-derived hints, but the response should mention both.

Add telemetry indicating package-derived subqueries and whether package resolution was verified or fallback-only.

## Security integration

If `include_security_context` is true, invoke the same native advisory lookup path used by `security_search` for package/ecosystem/version. Keep this bounded and optional. The response should include either:

- A compact `security_context` object with vulnerability metadata summaries, or
- Suggested `security_search` calls if directly invoking advisory lookup would create too much coupling.

Recommended first pass: attach compact `security_context` containing OSV-derived vulnerability metadata when available, capped by `max_security_results` or `max_per_group`. Also add source-card groups for authoritative advisories when they are retrieved through existing security search machinery.

Do not use advisory absence as proof of safety. Emit an explicit warning when no advisory is found.

## Version and migration behavior

If `version` is supplied, prefer docs and registry URLs for that version. If `compare_version` is supplied, generate release/changelog subqueries containing both versions. Do not perform semantic version solving beyond simple string matching in the first pass.

Later phases can add semver parsing for crates/npm and packaging-version handling for Python. For now, keep version behavior transparent and conservative.

## Tests

Add unit tests for package coordinate validation and alias parsing:

- `crates.io`, `cargo`, and `rust` map to `crates_io`.
- `pypi` and `python` map to `pypi`.
- `npm`, `node`, and `javascript` map to `npm`.
- Empty package names are rejected.

Add resolver tests with `httpmock`:

- crates.io metadata resolves registry/docs/repository/latest version.
- PyPI metadata resolves project URLs and requested version.
- npm metadata resolves repository/homepage/latest version.
- Resolver timeout or bad JSON returns fallback URLs plus warnings, not a panic.

Add planner integration tests:

- Package-derived repo hints are used when no explicit repo is supplied.
- Explicit repo hints override package-derived repository URL.
- Changelog/migration subqueries are generated when requested.
- Security context is included only when requested.

Add response serialization tests for `package_resolution` and optional security context.

## Documentation

Update README `repo_search` docs with package/version examples:

```json
{
  "query": "Router::layer middleware behavior",
  "ecosystem": "crates.io",
  "package": "axum",
  "version": "0.7.0",
  "profile": "coding",
  "include_changelog": true,
  "include_security_context": true
}
```

Document that package resolution is metadata retrieval, not dependency solving. Explain fallback behavior when registry APIs fail.

## Acceptance criteria

- `repo_search` accepts package ecosystem/name/version fields.
- Rust/PyPI/npm package metadata can be resolved through bounded HTTP using tests with mocked responses.
- Package resolution can seed docs, registry, source repository, release, changelog, and security subqueries.
- Response includes package resolution metadata and transparent warnings.
- Existing repo-only searches continue to work unchanged.

## Suggested implementation order

1. Add package coordinate types and validation.
2. Add resolver abstraction and mocked resolver tests.
3. Implement crates.io/docs.rs resolver path.
4. Implement PyPI resolver path.
5. Implement npm resolver path.
6. Wire package resolution into repo planner and response.
7. Add optional security context using existing advisory lookup path.
8. Update README and examples.
