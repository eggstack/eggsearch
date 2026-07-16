# Phase 3: Remote Repository Intelligence

Status: ready after Phase 2
Depends on: Phase 1; Phase 2 boundary tooling
Primary goal: make `repo_map` a real bounded remote repository structure primitive across supported code hosts.

## 1. Problem Statement

The current `repo_map` contract advertises repository structure discovery, but effective remote behavior is metadata-only unless a matching local checkout is available. Coding agents need a low-cost way to understand repository shape before issuing file-specific search and fetch requests.

This phase adds native remote tree retrieval without cloning repositories and without turning eggsearch into a persistent code-host mirror.

## 2. Required Outcomes

- Public remote repositories can be mapped on GitHub, GitLab, Gitea, Forgejo, and Codeberg.
- Authenticated private repositories work where credentials are configured.
- All tree operations enforce entry, depth, byte, pagination, concurrency, and timeout limits.
- Results use one provider-neutral deterministic schema.
- Repository maps classify important files and directories for agent workflows.
- Unsupported or degraded paths remain explicit and machine-readable.

## 3. Non-Goals

- Full repository cloning.
- Commit-history traversal.
- Blob content download for every tree entry.
- Hosted global code indexing.
- Model-generated repository summaries.
- Recursive submodule fetching.

## 4. Workstream A: Provider-Neutral Repository Tree Model

### Tasks

Define internal types distinct from public response types:

```text
RepositoryTreeProvider
RepositoryTreeRequest
RepositoryTreePage
RepositoryTreeEntry
RepositoryMetadata
RepositoryTreeFailure
```

Required entry fields:

- host;
- owner/namespace;
- repository;
- resolved ref;
- path;
- entry kind;
- size when available;
- object identifier when available;
- URL and raw URL when derivable;
- depth;
- provider provenance.

Required request controls:

- `max_entries`;
- `max_depth`;
- total response-byte cap;
- timeout;
- page cap;
- include files;
- include directories;
- optional ref or commit SHA.

Keep provider-specific response details behind adapters.

### Acceptance

All host adapters emit equivalent internal entries and use common truncation and error semantics.

## 5. Workstream B: GitHub Adapter

### Tasks

1. Support the Git Trees API for recursive or bounded traversal.
2. Fall back to the Contents API when tree responses are truncated or unsuitable.
3. Resolve default branch when no ref is supplied.
4. Prefer commit-pinned links when the resolved SHA is known.
5. Support configured tokens without requiring them for public repositories.
6. Handle GitHub API pagination and rate-limit metadata.
7. Detect API-level `truncated` responses and expose them honestly.

### Tests

Use recorded fixtures for:

- small repository;
- nested repository;
- truncated tree response;
- missing ref;
- empty repository;
- private/auth failure;
- rate limiting;
- submodule entries;
- symlink entries.

## 6. Workstream C: GitLab Adapter

### Tasks

1. Use the repository tree API with recursive pagination where supported.
2. Correctly encode nested namespaces.
3. Resolve default branch and commit SHA.
4. Build browser and raw URLs safely.
5. Support self-hosted base URLs from provider configuration.
6. Enforce page and response limits before aggregation.

### Tests

Cover nested groups, paginated trees, absent projects, authentication failures, and custom base URLs.

## 7. Workstream D: Gitea, Forgejo, and Codeberg

### Tasks

1. Implement a shared Forge-compatible adapter where API behavior is equivalent.
2. Keep host identity and capability reporting distinct even when implementation is shared.
3. Support custom Gitea/Forgejo base URLs and Codeberg defaults.
4. Validate base URLs using the same outbound safety expectations as configured provider endpoints.
5. Resolve branch, commit, and entry links deterministically.
6. Record host-specific deviations in fixtures and adapter tests.

### Acceptance

One shared implementation may serve multiple hosts, but runtime diagnostics must report the actual configured host and capabilities.

## 8. Workstream E: Classification and Repository Summary

Add deterministic classification for:

- manifests and lockfiles;
- source roots;
- tests;
- examples;
- benchmarks;
- documentation;
- CI configuration;
- security policy and advisories;
- contribution and governance files;
- release/changelog files;
- migrations;
- generated and vendored paths;
- build output and dependency directories;
- submodules.

Do not fetch file contents merely to classify ordinary entries. Use paths, names, extensions, object metadata, and a small bounded set of optional important-file fetches only when explicitly allowed by the request or workflow.

The map response should expose:

- resolved repository identity;
- resolved ref/commit;
- entries;
- important files grouped by role;
- top-level language hints;
- manifests;
- CI and security indicators;
- truncation reasons;
- next actions.

## 9. Workstream F: Routing and Capability Reporting

### Tasks

1. Extend provider capability descriptors with native tree support.
2. Route `repo_map` to the matching host adapter rather than generic search providers.
3. Preserve local checkout preference where appropriate, but make local versus remote provenance explicit.
4. Add skip codes for unavailable tree capability, missing credentials, rate limiting, and unsupported host versions.
5. Change `provider_status.tool_capabilities.repo_map.repo_map_remote` from `metadata_only` only when actual native capability is available.
6. Keep fallback responses honest when no adapter can execute.

## 10. Workstream G: Limits and Failure Semantics

### Required limits

- Maximum entries.
- Maximum depth.
- Maximum pages.
- Maximum encoded response bytes read from providers.
- Per-request timeout.
- Maximum important-file probes.
- Maximum concurrent provider/API requests.

### Required failure classes

- repository not found;
- ref not found;
- authentication required;
- permission denied;
- rate limited;
- provider unavailable;
- malformed response;
- response truncated by provider;
- response truncated by eggsearch;
- deadline exceeded;
- unsupported host/version.

Partial maps should be returned when safe and useful, with structured warnings rather than being discarded wholesale.

## 11. Workstream H: Agent Next Actions

Generate deterministic next actions based on map contents, such as:

- fetch README or architecture document;
- fetch primary manifest;
- inspect main library entry point;
- inspect CLI entry point;
- inspect security policy;
- inspect tests for a named component;
- run `repo_search` constrained to an important directory;
- build an evidence bundle after selected fetches.

Every action must include resolvable input fields rather than placeholder-only suggestions when the map already supplies the value.

## 12. Testing Strategy

- Provider adapters use offline recorded JSON fixtures.
- Contract tests cover equivalent schema output across hosts.
- Deterministic ordering tests randomize provider entry order.
- Pagination tests prove hard page and entry bounds.
- Failure tests preserve partial results.
- Public MCP schema tests ensure additive compatibility.
- Live smoke tests remain ignored/manual or scheduled.

## 13. Definition of Done

- All five supported host families have native bounded tree retrieval.
- `repo_map` returns useful remote structure without a local checkout.
- Provider status reports effective capability accurately.
- Entry and depth bounds are hard limits.
- Partial and truncated maps are explicit.
- Important file classification is deterministic and tested.
- Next actions point to concrete repository evidence.
- Full release gate passes.

## 14. Handoff Notes

Implement the provider-neutral model and GitHub adapter first. Do not copy provider-specific JSON types into public MCP responses. Avoid a single giant `repo_mapper.rs`; keep host adapters and classification independent so later host drift can be fixed without destabilizing the public map assembly path.