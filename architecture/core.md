# Core Types Deep Dive

**Location:** `src/core/` (35 files)
**Purpose:** Pure types, configuration, error types, and source card model. Intentionally independent of any MCP, HTTP, or search-engine implementation.

---

## Module Map

| File | Responsibility |
|------|---------------|
| `mod.rs` | Module declarations + ~300 re-exports |
| `config.rs` | `AppConfig`, `SearchSection`, `FetchSection`, `Mode`, `ProviderConfig`, validation, provider resolution |
| `error.rs` | `CoreError`/`CoreResult<T>` via `thiserror` |
| `source_card.rs` | `SourceCard` (canonical output), `SourceKind`, `SourceMetadata`, `RankReason`, `IssueMetadata`, `ReleaseMetadata` |
| `identity.rs` | Deterministic FNV-1a ID system: `source_id`, `fetch_id`, `suggested_fetch_id`, `batch_fetch_id`, `locator_id`, `doc_id`, `chunk_id`, `code_span_id` |
| `provider.rs` | `ProviderKind`, `ProviderCapabilities` (24 flags), `ProviderDescriptor`, `KNOWN_PROVIDER_IDS` (34 ids) |
| `query.rs` | `WebSearchRequest`, `Freshness`, `SafeSearch`, `SearchIntent`, `MaxResultsResolution` |
| `sanitize.rs` | `strip_control_chars`, `bound_text`, `frame`, `scan_injection_markers` (3-tier sanitization) |
| `fetch.rs` | `WebFetchRequest`/`WebFetchResponse`, `ExtractMode`, `FetchTransform`, `FetchTrust` |
| `batch_fetch.rs` | `BatchFetchItem`, `BatchFetchResponse`, `BatchFetchResult` |
| `document.rs` | `FetchDocument`, `DocumentChunk`, `RenderFormat`, `RenderedBlock` |
| `result.rs` | `SearchWarning`, `TrustLevel` |
| `warning.rs` | `AgentWarning`, `WarningCode`, `WarningSeverity`, `WarningAccumulator` |
| `quality.rs` | `ResultQuality`, `ResultConfidence`, `SearchUncertaintySummary`, evidence strength/freshness/relevance estimates |
| `evidence_role.rs` | `EvidenceRole` enum (19 roles): `PrimaryImplementation`, `OfficialDocumentation`, etc. |
| `evidence_bundle.rs` | `EvidenceBundle`, `EvidenceBundleRequest`, `EvidenceBundleSource`, `EvidenceGap`, `EvidenceTrustSummary` |
| `evidence_postprocess.rs` | `postprocess()`, `detect_structured_conflicts()`, `materialize_evidence_roles()`, `resolve_workflow_model()` |
| `conflict.rs` | `EvidenceConflict`, `ConflictDetector`, `ConflictSeverity`, `ConflictClass`, `ConflictResolution` |
| `retrieval_status.rs` | `RetrievalAttempt`, `RetrievalAttemptOutcome`, `TruncationEvidence`, `classify_absence()` |
| `security.rs` | `SecuritySearchRequest`/`Response`, `VulnerabilityMetadata`, `SecurityIdentifier`, `SeverityLevel`, `DefensiveGuidance` |
| `security_applicability.rs` | `ApplicabilityAssessment`, `DependencyFinding`, `AdvisoryRange` |
| `repo_search.rs` | `RepoSearchRequest`/`Response`, `RepoResultGroup`, `SearchProfile`, `RepoSuggestedFetch` |
| `repo_fetch.rs` | `RepoFetchRequest`/`Response`, `RepoLocator`, `RepoFetchedLine`, GitHub/GitLab URL builders |
| `repo_map.rs` | `RepoMapRequest`/`Response`, `RepoMapEntry`, `ImportantFileKind`, `ImportantDirKind` |
| `repo_query.rs` | `RepoQueryHints` — structured repo search hints (owner, repo, path, language, symbol) |
| `research.rs` | `ResearchSearchRequest`/`Response`, `ResearchWorkflow`, `ResearchClaim`, `ResearchGap`, `ResearchSourceClass` |
| `local.rs` | `LocalConfig`, `LocalSearchRequest`/`Result`, `LocalFileEntry`, `LocalMatch` |
| `package.rs` | `PackageCoordinate`, `PackageEcosystem`, `PackageResolution` |
| `code_context.rs` | `CodeContext`, `ExtractionLanguage`, `detect_language()`, `extract_code_context()` |
| `code_evidence.rs` | `CodeEvidence`, `SourceRole` (16 variants), `EvidenceConfidence`, `infer_source_role()` |
| `code_host_fetch.rs` | `CodeHostFetchTarget`, `resolve_code_host_fetch_target()` |
| `code_metadata.rs` | `CodeHost`, `CodeMetadata`, URL classification for GitHub/GitLab/Codeberg |
| `error_query.rs` | `ErrorSearchContext`, `ErrorSubquery`, `ExactErrorConfig`, compiler/runtime error search mode |
| `workflow.rs` | `AgentWorkflowRecipe`, `AgentWorkflowStep`, `AgentWorkflowFallback`, `AgentNextAction` |
| `workflow_coverage.rs` | `WorkflowCoverageModel`, `WorkflowCoverageResult`, `RetrievalFailure`, `CoverageStatus` |

---

## Key Types

### SourceCard (`source_card.rs`)
The canonical output type for all search results. Contains:
- `id` — deterministic FNV-1a hash
- `url`, `title`, `snippet` — basic metadata
- `kind` — URL classification (16 variants: `Documentation`, `Code`, `Issue`, `Release`, `Package`, etc.)
- `score`, `rank_reason` — quality/ranking metadata
- `freshness`, `trust_level` — temporal and trust signals

### ProviderCapabilities (`provider.rs`)
24 boolean capability flags per provider:
- `web_search`, `code_search`, `issue_search`, `release_search`
- `advisory_search`, `package_lookup`, `scholarly_search`
- `local_search`, `repo_fetch`, `repo_map`
- etc.

### Identity System (`identity.rs`)
Deterministic FNV-1a 64-bit hashes for:
- `source_id(url, title, snippet)` — deduplication across tools
- `fetch_id(url)` — cache keys
- `suggested_fetch_id(url, label)` — suggested fetch deduplication
- `batch_fetch_id(urls)` — batch fetch grouping
- `locator_id(owner, repo, path, ref)` — repo locator stable IDs
- `doc_id(url, content)` — document chunk IDs
- `code_span_id(url, symbol, start_line, end_line)` — code span IDs

### Sanitization (`sanitize.rs`)
3-tier sanitization for all untrusted text:
1. **Control char strip** — remove non-printable characters
2. **Framing** — bound text length, truncate intelligently
3. **Injection scan** — detect prompt injection markers

### AppConfig (`config.rs`)
Root configuration type:
```toml
[search]
providers = ["duckduckgo", "brave"]
profile = "generic"  # generic | coding | security | research

[fetch]
max_chars = 50000
timeout_ms = 30000

[local]
enabled = false
```

---

## Error Handling

- `CoreError` enum via `thiserror`
- `CoreResult<T> = Result<T, CoreError>`
- Adapters return `WebSearchResponse` (never errors; partial failures are soft)
- MCP tools return `Result<serde_json::Value, ToolError>`

---

## Design Principles

- **No HTTP or engine dependencies** — pure types only
- **Deterministic IDs** via FNV-1a (never random UUIDs for stable types)
- **All untrusted text** flows through `sanitize.rs`
- **Serializable** — all types derive `serde::Serialize/Deserialize` + `schemars::JsonSchema`

---

[← Back to Overview](overview.md)
