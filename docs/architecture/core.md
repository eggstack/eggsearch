# core Module Deep Dive

**Path:** `src/core/`
**Purpose:** Pure domain types, config model, error types, identity system, sanitization, warnings, source cards, quality heuristics, and domain-specific types for security, research, repos, and local search.

The `core` module is intentionally independent of HTTP, MCP, or any search engine implementation. It defines the canonical data model used throughout the codebase.

---

## Submodule Inventory (30+ files)

### Configuration

| File | Key Types | Responsibility |
|------|-----------|----------------|
| `config.rs` | `AppConfig`, `SearchSection`, `FetchSection`, `Mode`, `ProfileConfig`, `ApiProviderConfig`, `SearxngConfig`, `LiveConfig` | TOML config model. Path defaults to `$XDG_CONFIG_HOME/eggsearch/config.toml`. Provider resolution with enabled/known validation. |
| `provider.rs` | `ProviderDescriptor`, `ProviderSkipCode`, `ProviderKind`, `ProviderCapabilities`, `CapabilityOption` | Provider identity, diagnostics, and capability flags. `ProviderSkipCode` is a stable snake_case enum for machine-readable skip reasons (13 variants). `ProviderDescriptor` includes `routable`, `skip_reason`, and `skip_code` fields. `CapabilityOption` is a 24-variant enum for querying specific provider capabilities. |

### Error Handling

| File | Key Types | Responsibility |
|------|-----------|----------------|
| `error.rs` | `CoreError`, `CoreResult<T>` | `thiserror`-based error enum covering URL, query, config, provider, IO, serde, TOML failures |

### Query Model

| File | Key Types | Responsibility |
|------|-----------|----------------|
| `query.rs` | `WebSearchRequest`, `SafeSearch`, `SearchIntent`, `Freshness`, `MaxResultsResolution` | Input shape for `web_search`. Supports intent hints (web/docs/code/issues/releases/security/news) and freshness hints (any/day/week/month/year). Both have alias parsing for weaker models |
| `repo_query.rs` | `RepoQueryHints` | Parses `repo:owner/name`, `path:src/`, `file:lib.rs`, `lang:rust`, `symbol:Router`, `host:github`, `org:`/`owner:` hints, and bare `owner/repo` patterns from query text |
| `error_query.rs` | `ExactErrorConfig`, `ErrorSubquery`, `ErrorCode`, `StackFrameHint` | Deterministic error-message parser for exact-error search mode |

### Source Cards (Core Output Model)

| File | Key Types | Responsibility |
|------|-----------|----------------|
| `source_card.rs` | `SourceCard`, `SourceKind`, `SourceMetadata`, `RankReason`, `IssueMetadata`, `ReleaseMetadata`, `LocalRepoMatch` | Canonical provider-agnostic output model. Deterministic `source_kind` classification from URL heuristics. 30+ `RankReason` variants. Code-host URLs delegate to `code_metadata::classify_and_extract` |

### Trust and Warnings

| File | Key Types | Responsibility |
|------|-----------|----------------|
| `result.rs` | `TrustLevel`, `SearchWarning` | Trust labels (`ExternalUntrusted`, `LocalTrusted`, `Unknown`) and per-provider warning struct. Used by `SourceCard.trust`, `WebFetchResponse.trust`, and `evidence_bundle` warnings |

### Identity System

| File | Key Types | Responsibility |
|------|-----------|----------------|
| `identity.rs` | `FnvHasher`, `SourceKey`, `FetchKey`, `SuggestedFetchKey`, `BatchFetchKey`, `CodeSpanKey`, `RepoLocatorKey`, `DocKey`, `DocChunkKey` | FNV-1a 64-bit deterministic IDs. Versioned prefix (`eggsearch-id-v1\0`) + entity namespace. URL canonicalization (strip `www.`, default ports, fragments, normalize percent-encoding). ID prefixes: `src_`, `fetch_`, `suggested_`, `batch_`, `span_`, `loc_`, `doc_`, `chunk_`, `bundle_` |

### Sanitization

| File | Key Types | Responsibility |
|------|-----------|----------------|
| `sanitize.rs` | `TrustMarkers`, `MarkerHit`, `strip_control_chars`, `bound_text`, `scan_injection_markers`, `frame`, `SNIPPET_MAX_CHARS`, `TITLE_MAX_CHARS` | 3-tier sanitization: Tier 1 (control-char strip + length bound, always on), Tier 2 (`<<<EXTERNAL_UNTRUSTED>>>` framing), Tier 3 (prompt-injection marker scanning). 7 regex patterns for injection detection. Public functions: `strip_control_chars`, `bound_text`, `scan_injection_markers`, `frame` |

### Warnings

| File | Key Types | Responsibility |
|------|-----------|----------------|
| `warning.rs` | `AgentWarning`, `WarningCode`, `WarningSeverity`, `WarningAccumulator` | 50+ machine-readable warning codes with stable `snake_case` strings. 4 severity levels (info/notice/warning/error). Deduplication by `(code, provider_ids, result_ids, source_ids)` |

### Quality Heuristics

| File | Key Types | Responsibility |
|------|-----------|----------------|
| `quality.rs` | `ResultQuality`, `GroupQualitySummary`, `SearchUncertaintySummary`, `ResultConfidence`, `RelevanceEstimate`, `AuthorityEstimate`, `FreshnessEstimate`, `EvidenceStrength`, `UncertaintyReason`, `QualityReason` | Heuristic quality metadata computed from URL/domain heuristics and structured result metadata. `compute_card_quality()` and `compute_group_quality()` are pure functions |

### Provider Diagnostics

| File | Key Types | Responsibility |
|------|-----------|----------------|
| `provider.rs` | `ProviderSkipCode` | 13-variant enum serialized as stable snake_case strings. Variants: `unknown_provider`, `disabled_by_user`, `missing_api_key`, `missing_searxng_config`, `missing_base_url`, `invalid_base_url`, `missing_local_backend`, `credential_not_configured`, `credential_env_missing`, `credential_invalid`, `cooldown_active`, `not_built`, `unknown`. Used by `ProviderDescriptor.skip_code` and `ProviderSkipReason.skip_code` for machine-readable diagnostics. |

### Workflow Guidance

| File | Key Types | Responsibility |
|------|-----------|----------------|
| `workflow.rs` | `AgentWorkflowRecipe`, `AgentWorkflowStep`, `AgentWorkflowFallback`, `AgentNextAction`, `RecipeDetail`, `RecipeSupport` | Machine-readable workflow recipes for agent guidance. `MAX_NEXT_ACTIONS = 5` |

### Document Model

| File | Key Types | Responsibility |
|------|-----------|----------------|
| `document.rs` | `FetchDocument`, `DocumentKind`, `RenderFormat`, `BlockKind`, `RenderedBlock`, `DocumentChunk`, `DocumentOutlineEntry`, `FetchRenderMetadata` | Structured document model for `web_fetch` responses. 16 document kinds (HTML, PlainText, Markdown, Code, JSON, TOML, YAML, PDF, etc.). Block-based rendering with outline/chunks |

### Fetch Types

| File | Key Types | Responsibility |
|------|-----------|----------------|
| `fetch.rs` | `WebFetchRequest`, `WebFetchResponse`, `ExtractMode`, `ExtractedLink`, `LinkKind`, `FetchTransform`, `FetchTransformKind`, `FetchTrust` | Fetch request/response types. 3 extraction modes (Text, Markdown, MetadataOnly). 15 link kinds. Code-host URL transforms (4 kinds: GithubRawFile, GitlabRawFile, CodebergRawFile, GiteaRawFile) |
| `batch_fetch.rs` | `BatchFetchItem`, `BatchFetchResult`, `BatchFetchResponse` | Tagged enum items (Web or Repo). Per-item results with stable IDs |

### Code Metadata

| File | Key Types | Responsibility |
|------|-----------|----------------|
| `code_metadata.rs` | `CodeHost`, `CodeMetadata` | Deterministic URL parsing for GitHub/GitLab/Codeberg. Extracts owner, repo, ref, path, language, line ranges from URL structure |
| `code_evidence.rs` | `CodeEvidence`, `SourceRole`, `SymbolKind`, `EvidenceConfidence`, `CodeEvidenceReason` | Enriched code-match evidence: raw URLs, browser URLs, source roles (17 kinds), symbol kinds, confidence levels |
| `code_context.rs` | `CodeContext`, `ExtractionLanguage`, `detect_language`, `detect_language_str`, `extract_code_context`, `extract_imports`, `find_enclosing_symbol` | Lightweight line-oriented code context extraction: imports, enclosing symbol. Supports Rust, Python, TypeScript, JavaScript, Go |
| `code_host_fetch.rs` | `CodeHostFetchTarget` | Rewrites GitHub/GitLab/Codeberg source-file browser URLs to raw content URLs for fetching |

### Repository Types

| File | Key Types | Responsibility |
|------|-----------|----------------|
| `repo_fetch.rs` | `RepoFetchRequest`, `RepoFetchResponse`, `RepoLocator`, `RepoLocatorKind`, `CodeSpanEvidence`, `RepoFetchedLine`, `apply_line_range`, `github_browser_url`, `github_permalink_url`, `github_raw_url`, `gitlab_browser_url`, `gitlab_raw_url` | Structured repository fetch by locator (host/owner/repo/path/ref). Line ranges, context lines, symbol expansion. URL helper functions for GitHub/GitLab browser/permalink/raw URLs |
| `repo_search.rs` | `RepoSearchRequest`, `RepoSearchResponse`, `RepoResultGroup`, `RepoResultGroupKind`, `RepoSearchMode`, `SearchProfile`, `RepoSuggestedFetch`, `ResolvedRepoIdentity`, `RepoIdentitySource`, `ProviderSelectionTelemetry`, `RepoSearchSubqueryTelemetry`, `RepoSearchTelemetry` | Structured repo evidence discovery. 4 profiles (generic/coding/security/research). `exact_error` mode for compiler errors |
| `repo_map.rs` | `RepoMapRequest`, `RepoMapResponse`, `RepoMapEntry`, `RepoMapEntryKind`, `RepoMapMode`, `RepoMapSuggestedFetch`, `RepoImportantFile`, `RepoImportantDirectory`, `RepoPathSummary`, `ImportantFileKind`, `ImportantDirKind`, `classify_important_file`, `classify_important_directory` | Repository structure discovery: important files (README, manifest, CI, security) and directories |

### Security Types

| File | Key Types | Responsibility |
|------|-----------|----------------|
| `security.rs` | `SecuritySearchRequest`, `SecuritySearchResponse`, `VulnerabilityMetadata`, `SeverityLevel`, `SecurityIdentifier`, `SecurityRemediation`, `DefensiveGuidance`, `CompactSecurityContext`, ~30 types | Security-oriented retrieval with normalized vulnerability metadata, advisories, remediation categories, defensive guidance |
| `security_applicability.rs` | `AdvisoryRange`, `ApplicabilityAssessment`, `ApplicabilityStatus`, `DependencyFinding` | Advisory affected/fixed range extraction and applicability assessment |

### Research Types

| File | Key Types | Responsibility |
|------|-----------|----------------|
| `research.rs` | `ResearchSearchRequest`, `ResearchSearchResponse`, `ResearchClaim`, `ResearchConflict`, `ResearchDomain`, `ResearchSourceType`, `ResearchDepth`, `ResearchWorkflow` | Research-oriented multi-source evidence discovery with claims, conflicts, gaps, and depth control |

### Package Registry Types

| File | Key Types | Responsibility |
|------|-----------|----------------|
| `package.rs` | `PackageCoordinate`, `PackageEcosystem`, `PackageResolution` | 10 ecosystems (crates.io, PyPI, npm, Go, Maven, NuGet, RubyGems, Packagist, OCI, GitHub Actions). Registry API URL construction |

### Evidence Bundle

| File | Key Types | Responsibility |
|------|-----------|----------------|
| `evidence_bundle.rs` | `EvidenceBundle`, `EvidenceBundleSource`, `EvidenceBundleFetchedItem`, `EvidenceBundleLink`, `EvidenceGap`, `EvidenceGapKind`, `EvidenceTrustSummary`, `EvidenceProviderSummary` | Deterministic non-summarizing evidence container for multi-agent handoff. 25+ gap kinds |

### Local Search

| File | Key Types | Responsibility |
|------|-----------|----------------|
| `local.rs` | `LocalConfig`, `LocalFileEntry`, `LocalSearchRequest`, `LocalSearchResult`, `LocalMatch`, `LocalFetchPathError` | Local workspace search types, path validation (traversal, binary, symlink, hidden, skip-dirs), language detection |

---

## Key Design Decisions

1. **Pure domain types** — `core` has no HTTP or MCP dependencies. This enables testing domain logic without network or transport concerns.

2. **Deterministic IDs** — All stable output types use FNV-1a content-derived hashes, never random UUIDs. This enables cross-tool deduplication and regression testing.

3. **Soft failures via warnings** — Domain operations return warnings alongside results rather than failing. This matches the agent-oriented use case where partial results are better than none.

4. **Three-tier sanitization** — Untrusted text flows through control-char stripping, external framing, and injection scanning before reaching agents.

5. **URL canonicalization** — Identity system normalizes URLs (strip `www.`, default ports, fragments, percent-encoding) to prevent trivial differences from producing different IDs.

---

**Back to:** [overview.md](overview.md)
