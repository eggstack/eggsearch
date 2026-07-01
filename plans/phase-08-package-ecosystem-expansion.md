# Phase 8 Plan: Package and Ecosystem Expansion

## Objective

Expand package-aware retrieval beyond crates.io, PyPI, and npm so coding agents can start from common package coordinates and receive registry, docs, source, release, and advisory evidence across the major ecosystems used in real projects.

This phase is metadata-first. eggsearch should not become a dependency solver, package manager, artifact downloader, container scanner, or SBOM generator in this phase. The goal is to resolve package coordinates into high-quality evidence seeds and structured source metadata that downstream tools can explicitly fetch.

## Current baseline

The repo already supports package-aware retrieval for a smaller set of ecosystems, likely crates.io, PyPI, and npm. The current search stack can:

- Accept ecosystem/package/version fields in `repo_search` and `security_search`.
- Resolve some package registry metadata.
- Use package source repository URLs to improve repo-scoped search.
- Generate suggested fetches for registry/docs/source/release/security evidence.
- Surface package-resolution warnings when registry API lookup fails.

Phase 8 expands this model to more ecosystems while preserving the same bounded, transparent behavior.

## Target ecosystems

Implement in priority order. Stop at a clean boundary if the phase needs to be split.

1. Go modules.
2. Maven/Gradle JVM packages.
3. NuGet.
4. RubyGems.
5. Packagist/Composer.
6. Docker/OCI images.
7. GitHub Actions.

Each ecosystem should have enough support to resolve package identity and produce evidence links. Do not require full feature parity across all ecosystems in one commit; prefer incremental modules and tests.

## Non-goals

Do not download package artifacts by default. Do not execute package-manager commands. Do not resolve full dependency graphs. Do not implement vulnerability applicability beyond the metadata hooks needed for Phase 9. Do not require a network call for every fallback if deterministic registry URLs can be generated safely.

## Core model changes

### Package ecosystem enum

Extend the package ecosystem enum with stable serialized names:

- `go` or `go_modules`.
- `maven`.
- `nuget`.
- `rubygems`.
- `packagist`.
- `oci` or `docker`.
- `github_actions`.

Prefer backward-compatible aliases in parsing, but serialize with one canonical snake-case value.

### Package coordinate model

The existing coordinate model may not fit every ecosystem. Extend without breaking existing fields.

Suggested shape:

```rust
pub struct PackageCoordinate {
    pub ecosystem: PackageEcosystem,
    pub name: String,
    pub version: Option<String>,
    pub namespace: Option<String>,
    pub qualifier: Option<String>,
}
```

Where applicable:

- Maven: `namespace = group_id`, `name = artifact_id`.
- NuGet: `name = package id`.
- Go: `name = module path`.
- OCI: `namespace = registry/repository namespace`, `name = image`, `qualifier = tag or digest kind` if needed.
- GitHub Actions: `name = owner/repo`, `version = tag/ref`.

If the existing model already supports these, keep changes minimal.

### Package resolution response

Ensure resolution returns a common shape:

```rust
pub struct PackageResolution {
    pub coordinate: PackageCoordinate,
    pub verified: bool,
    pub registry_url: Option<String>,
    pub documentation_url: Option<String>,
    pub source_repository_url: Option<String>,
    pub homepage_url: Option<String>,
    pub latest_version: Option<String>,
    pub requested_version: Option<String>,
    pub license: Option<String>,
    pub release_url: Option<String>,
    pub advisory_urls: Vec<String>,
    pub warnings: Vec<String>,
}
```

Use serde defaults and skip-empty fields for any new public fields.

## Workstream 1: Go modules

### Resolution strategy

Use metadata-only HTTP calls where practical:

- `https://proxy.golang.org/<module>/@latest` for latest version.
- `https://proxy.golang.org/<module>/@v/<version>.info` for specific version.
- `https://pkg.go.dev/<module>` for docs.
- `https://sum.golang.org/lookup/<module>@<version>` only if useful and safe.

Source repository inference can use:

- Module path host patterns such as `github.com/owner/repo`.
- `go-import` meta tags only if existing fetch infrastructure can retrieve and parse bounded HTML safely.

### Tests

- Parse Go module coordinates such as `github.com/gin-gonic/gin` and `golang.org/x/net`.
- Deterministic fallback URLs when HTTP lookup is unavailable.
- Specific version creates pkg.go.dev and proxy URLs.
- Source repo inferred for GitHub module paths.

## Workstream 2: Maven/Gradle

### Coordinate parsing

Support conventional `group_id:artifact_id` and optional version.

Examples:

- `org.springframework:spring-core`.
- `com.fasterxml.jackson.core:jackson-databind`.

### Resolution strategy

Metadata-only:

- Maven Central search API or Solr endpoint if already acceptable.
- Deterministic fallback to Maven Central artifact path.
- Docs fallback to javadoc.io where possible.
- Source repository from POM SCM metadata if bounded XML fetch/parser is implemented.

### Tests

- Group/artifact parsing.
- Maven Central URL construction.
- javadoc.io URL construction.
- POM SCM extraction with small fixture.
- Fallback warnings when metadata lookup fails.

## Workstream 3: NuGet

### Resolution strategy

Use NuGet registration metadata endpoints:

- Package registration index.
- Version-specific catalog entry where needed.

Evidence URLs:

- nuget.org package page.
- project URL/homepage when available.
- repository URL when available.
- release notes when present.

### Tests

- Case-insensitive package IDs normalize predictably.
- Latest and specific version resolution.
- Repository/project URL extraction from fixture.
- Fallback URL creation.

## Workstream 4: RubyGems

### Resolution strategy

Use RubyGems API metadata endpoints:

- Package info.
- Version info if needed.

Evidence URLs:

- rubygems.org gem page.
- homepage/project URL.
- source code URL when provided.
- changelog URL when provided.

### Tests

- Gem name validation.
- Metadata extraction from fixture.
- Source repo URL extraction.
- Fallback warnings.

## Workstream 5: Packagist/Composer

### Resolution strategy

Use Packagist package metadata endpoint.

Coordinate form is usually `vendor/package`.

Evidence URLs:

- packagist.org package page.
- repository/source URL.
- homepage.
- release/version metadata.

### Tests

- Vendor/package validation.
- Metadata extraction from fixture.
- GitHub/GitLab repository inference.
- Version-specific resolution when available.

## Workstream 6: Docker/OCI images

### Scope limitation

This phase should only resolve image metadata and evidence links. Do not pull manifests by default unless using a bounded registry API call explicitly implemented for metadata only.

Support common forms:

- `nginx`.
- `library/nginx`.
- `docker.io/library/nginx`.
- `ghcr.io/owner/image`.
- `quay.io/org/image`.
- Tag or digest as `version`/qualifier.

Evidence URLs:

- Docker Hub page when registry is Docker Hub.
- GHCR package page when inferable.
- Quay page when inferable.
- Source repository only when metadata clearly provides it.

### Tests

- Image name normalization.
- Docker Hub official image fallback.
- GHCR/Quay URL construction.
- Tag/digest handling.
- No artifact/blob downloads.

## Workstream 7: GitHub Actions

### Coordinate form

Support action coordinates such as:

- `actions/checkout@v4`.
- `docker/login-action@v3`.
- `owner/repo/path?` only if path-based actions are explicitly supported.

Treat the action repo as the source repository. Registry page may be GitHub Marketplace when inferable.

Evidence URLs:

- GitHub repo.
- Marketplace page if known.
- README/action.yml via repo fetch suggestions.
- Releases/tags.
- Security advisories for the repo.

### Tests

- Parse `owner/repo@ref`.
- Generate GitHub repo locator.
- Suggested fetches include `action.yml`, README, releases.
- No GitHub API requirement for basic deterministic fallback.

## Workstream 8: Planner and suggested-fetch integration

Extend package resolution into:

- `repo_search` package-aware planning.
- `security_search` package-aware advisory planning.
- Suggested fetch ranking.
- Provider warnings/telemetry.

For each resolved package, preferred suggested fetches should include, when available:

1. Registry metadata page.
2. Official docs page.
3. Source repository map/search.
4. Release notes/changelog.
5. Security advisory sources.

Package resolution should not hide failure. If native lookup fails, return deterministic fallback URLs and warnings.

## Workstream 9: Validation and safety

Add ecosystem-specific validation:

- Reject empty package names.
- Reject obvious URL/script injection characters in package names.
- Preserve case where ecosystem requires it, normalize where ecosystem convention is case-insensitive.
- Cap lengths.
- Never treat package metadata text as instructions.
- Do not follow arbitrary registry-provided links during search; emit them as evidence candidates or fetch suggestions for explicit fetch.

## Documentation

Update README and MCP tool docs with:

- Supported package ecosystems.
- Coordinate examples.
- Metadata-only guarantee.
- Fallback behavior.
- Warning semantics.
- How package-aware `repo_search` and `security_search` differ.

## Acceptance criteria

- Package ecosystem enum supports the target ecosystems with stable parsing and serialization.
- At least Go, Maven, NuGet, RubyGems, Packagist, Docker/OCI, and GitHub Actions have deterministic fallback URL generation.
- Network-backed metadata resolution exists where practical and is bounded.
- Registry/docs/source/release/security evidence is exposed consistently.
- Package lookup failures return warnings, not silent empty responses.
- Suggested fetches rank registry/docs/source/release/security evidence sensibly.
- Tests use fixtures/mocks and do not require live registry availability for core correctness.
- `cargo fmt --check`, clippy, and relevant tests pass.
