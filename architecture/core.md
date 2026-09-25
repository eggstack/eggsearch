# Core Types Deep Dive

**Location:** `src/core/` (38 files)
**Role:** Pure domain model. Foundation of the dependency flow `core <- meta <- mcp <- commands`; `fetch` is independent of `meta`.
**Invariant:** No HTTP, no engines, no MCP. Only `serde`, `schemars`, `thiserror`, `regex` beyond std. Every type derives `Serialize`/`Deserialize` + `JsonSchema` where it crosses a tool boundary.

---

## 1. Purpose and invariants

`core` is the language every other layer speaks. `meta` orchestrates these types, `fetch` fills them, `mcp` serializes them, `commands` configures them.

Rules that hold across all 38 files:

- **Pure types only.** `mod.rs` states it explicitly: independent of any MCP, HTTP, or search-engine implementation. Network, dispatch, rendering, and transport live in `meta`, `fetch`, `mcp`.
- **Deterministic identity.** Every stable output ID is an FNV-1a 64-bit content hash with versioned prefix `eggsearch-id-v1\0` (see §5). Never random UUIDs in stable fields.
- **Sanitize untrusted text.** All provider/fetch text flows through `sanitize.rs` (`strip_control_chars`, `bound_text`, `frame`, `scan_injection_markers`) before it reaches a card or response.
- **Soft failure.** Adapters return responses, not errors; failures become `SearchWarning` / `AgentWarning` / `RetrievalAttempt` entries. `CoreError`/`CoreResult` is for validation and config, not per-provider outage.
- **Bounded everything.** Titles, snippets, excerpts, focus chunks, bundle sizes, domain filters, and cache ages all carry constants and clamps.
- **Additive schema evolution.** Responses grow via new optional fields only. Removing or renaming a field breaks corpus regression and downstream agents.
- **Bounded `git` execution.** Repo paths shell out only through bounded process-group helpers; timeout or byte-cap breach kills the group and records truncation evidence instead of hanging the adapter.

---

## 2. Config type model (`config.rs`)

Root type is `AppConfig` with four sections:

| Section | Type | Notes |
|---------|------|-------|
| `search` | `SearchSection` | `mode` (`Mode::Live` / `Mode::Off`, parsed by `Mode::parse`), `default_max_results`, `max_results_cap`, `max_query_chars`, `timeout_ms`, `default_providers`, `providers` map, `searxng: SearxngConfig`, `api: BTreeMap<String, ApiProviderConfig>`, profile/browser subsections |
| `fetch` | `FetchSection` | `enabled`, `timeout_ms`, `max_bytes`, `max_chars_default`, `max_chars_cap`, `redirect_limit`, `allow_private_network`, `allow_localhost`, `include_links_default`, `user_agent`, plus `FetchCacheSection` / `FetchBrowserSection` |
| `local` | `LocalConfig` (in `local.rs`) | `enabled`, roots, ignore rules; backend availability gates `local_workspace` routing |
| `egress` | `EgressSection` (`EgressHopConfig`) | Opt-in listener-free proxy-chain route for provider upstreams only |

Supporting types: `LiveConfig` (parsed but currently no-op `user_agent` / `respect_robots_txt` with startup warning), `SearxngConfig` (`enabled` + `base_url`), `ApiProviderConfig` (`enabled`, `api_key_env`, `base_url`), `ProfileConfig`, `PersistentBrowserProfilesConfig`. Helpers `optional_api_key` / `optional_api_key_misconfigured` implement keyless fallback for `OPTIONAL_API_PROVIDER_IDS` (`firecrawl_developer`); `AppConfig::load` falls back to defaults when the file is absent and `AppConfig::validate` rejects bad values. `EgressSection` / `EgressHopConfig` stay listener-free and provider-upstream-only; browser profile scoping keys `CacheScope::Profile` off the opaque profile ID. Operator reference lives in `docs/config.md`; the type-level companion dive is [config.md](config.md).

---

## 3. Request validation rules

Validation lives next to each request type and returns `CoreError::InvalidQuery` / `InvalidUrl` / `Config`, never a provider failure.

- `query.rs` — `WebSearchRequest::validate(max_query_chars)`: non-empty after trim, char-count cap, `max_results > 0`, `timeout_ms > 0`, `SearchDateRange` start `<=` end with strict `YYYY-MM-DD` parsing, `date_range` mutually exclusive with non-`Any` `Freshness`, `include_domains`/`exclude_domains` each `<= MAX_DOMAIN_FILTERS` (32) with `normalize_domain` + `domain_matches_filter` / `hostname_from_url` checks, `validate_language` (`MAX_LANGUAGE_LEN` 32) / `validate_region` (`MAX_REGION_LEN` 32), hostname/label length caps (`MAX_HOSTNAME_LEN` 253, `MAX_LABEL_LEN` 63). `resolve_max_results` applies default/cap; result is `MaxResultsResolution`.
- `repo_search.rs` — `RepoSearchRequest` (`RepoSearchMode`, `SearchProfile`, repo locator/package fields, `include_local`): planner-level checks for owner/repo/path/language/symbol coherence; telemetry via `RepoSearchTelemetry`, `ProviderSelectionTelemetry`, `RepoSearchSubqueryTelemetry`.
- `repo_fetch.rs` — `RepoFetchRequest` with `RepoLocator` / `RepoLocatorKind`: line-range clamping via `apply_line_range`; URL builders `github_browser_url`, `github_permalink_url`, `github_raw_url`, `gitlab_browser_url`, `gitlab_raw_url` (+ Codeberg/Gitea variants). `commit_sha` resolution comes from `resolved_ref`, not the entry SHA.
- `repo_map.rs` — `RepoMapRequest` (owner/repo/ref/path, `RepoMapMode`): path scoping; response `RepoMapResponse` with `RepoMapEntry` / `RepoMapEntryKind`, `RepoImportantFile` / `ImportantFileKind`, `RepoImportantDirectory` / `ImportantDirKind` via `classify_important_file` / `classify_important_directory`, plus `RepoPathSummary`, `RepoMapSuggestedFetch`, package/language/module/entrypoint/test-relationship summaries.
- `security.rs` / `security_applicability.rs` — `SecuritySearchRequest`: `SecurityIdentifiers` (`SecurityIdentifier`, `SecurityIdentifierKind`), `classify_query_kind`, `build_identifier_list`; `AdvisoryRange` + `DependencyFinding` (`DependencyRelation`, `DependencySource`) feed `ApplicabilityAssessment` (`ApplicabilityStatus`, `ApplicabilityConfidence`).
- `research.rs` — `ResearchSearchRequest` (`ResearchDomain`, `ResearchDepth`, `ResearchWorkflow`, `ResearchWorkflowContext`, `ResearchDimension`): depth-bounded subquery fan-out (`ResearchSubquery`), grouped response (`ResearchResultGroup`, `ResearchResultGroupKind`, `ResearchSuggestedFetch`, `ResearchTelemetry`).
- `local.rs` — `LocalSearchRequest` / `LocalConfig`: root scoping, `SKIP_DIRS` / `BINARY_EXTENSIONS` eligibility (`is_eligible_for_indexing`, `should_skip_component`), `validate_local_fetch_path` with `LocalFetchPathError`.
- `error_query.rs` — `validate_error_query`, `parse_error_query` / `redact_error_query` / `generate_error_subqueries` over `ErrorQueryParts` (`ErrorCode`, `StackFrameHint`), `ErrorSearchContext` / `ErrorSubquery` / `ExactErrorConfig`.
- `fetch.rs` / `fetch_policy.rs` / `fetch_locator.rs` — `WebFetchRequest::validate`-adjacent clamps: `resolve_effective_max_chars`, `validate_focus_query` / `validate_focus_max_chunks` / `validate_focus_max_chars` / `validate_focus_for_extract_mode`, `validate_cache_age` (`MAX_CACHE_AGE_SECONDS`), `validate_web_url`, `validate_repo_path`, `parse_batch_repo_host` (`BatchRepoHost`).
- `evidence_bundle.rs` — `EvidenceBundleRequest` (`EvidenceSourceInput`, `EvidenceFetchInput`) clamped by `EvidenceBundleLimits` against `DEFAULT_MAX_SOURCES` / `DEFAULT_MAX_FETCHED_ITEMS` / `DEFAULT_MAX_TOTAL_CHARS` and hard caps (`MAX_SOURCES_CAP`, `MAX_FETCHED_ITEMS_CAP`, `MAX_TOTAL_CHARS_CAP`).

---

## 4. Module map (all 38 files)

Grouped by responsibility; `mod.rs` declares modules and re-exports the public surface.

**Canonical output and metadata**

| File | Key types / functions |
|------|-----------------------|
| `source_card.rs` | `SourceCard`, `SourceKind` (17 kinds), `RankReason`, `SourceMetadata`, `IssueMetadata`, `ReleaseMetadata`, `SourceExcerpt` / `ExcerptProvenance`, `LocalRepoMatch`, `classify_source_kind`, `excerpt_normalized_key`, `parse_result_timestamp`, excerpt bounds `MAX_EXCERPTS_PER_CARD` (3), `MAX_EXCERPT_CHARS` (500), `MAX_EXCERPT_TOTAL_CHARS` (1200), `MAX_EXCERPT_REQUEST_COUNT` (3) |
| `document.rs` | `FetchDocument`, `DocumentChunk`, `DocumentKind`, `DocumentOutlineEntry`, `RenderFormat`, `RenderedBlock`, `FetchRenderMetadata`, `PdfPageMetadata` / `PdfDocumentMetadata`, `build_document_chunks` |
| `result.rs` | `TrustLevel`, `SearchWarning` |
| `quality.rs` | `ResultQuality`, `ResultConfidence`, `RelevanceEstimate`, `AuthorityEstimate`, `FreshnessEstimate`, `EvidenceStrength`, `QualityReason`, `GroupQualitySummary`, `SearchUncertaintySummary` / `UncertaintyReason`, `compute_card_quality`, `compute_card_quality_with_now`, `compute_group_quality` |
| `warning.rs` | `AgentWarning`, `WarningCode`, `WarningSeverity`, `WarningAccumulator`, `convert_warnings`, `convert_fetch_warnings`, `search_warning_to_agent_warning` |
| `error.rs` | `CoreError` (`InvalidUrl`, `InvalidQuery`, `Config`, `Provider`), `CoreResult<T>` |

**Identity and sanitization**

| File | Key types / functions |
|------|-----------------------|
| `identity.rs` | `FnvHasher`, `entity_prefix`, `write_entity_prefix`, `write_str`, `canonicalize_url`, `source_id` / `fetch_id` / `suggested_fetch_id` / `batch_fetch_id` / `locator_id` / `doc_id` / `chunk_id` / `code_span_id` + `compute_*` variants and `SourceKey` / `FetchKey` / `SuggestedFetchKey` / `BatchFetchKey` / `RepoLocatorKey` / `DocKey` / `DocChunkKey` / `CodeSpanKey` (see §5) |
| `sanitize.rs` | `strip_control_chars`, `bound_text`, `truncate_at_word`, `scan_injection_markers`, `frame`, `TrustMarkers` (+ `merge`), `MarkerHit`, `TITLE_MAX_CHARS` (200), `SNIPPET_MAX_CHARS` (500) (see §6) |

**Providers**

| File | Key types / functions |
|------|-----------------------|
| `provider.rs` | `ProviderKind` (`HtmlScrape`, `JsonApi`, `ApiKey`, `Local`), `ProviderCapabilities` (24 flags), `CapabilityOption`, `ProviderDescriptor`, `ProviderSkipCode`, `KNOWN_PROVIDER_IDS` (37), `API_PROVIDER_IDS`, `OPTIONAL_API_PROVIDER_IDS`, `CredentialRequirement`, `credential_requirement`, `is_api_provider`, `is_optional_api_provider`, `provider_configured_state`, `built_in_provider_descriptor`, `provider_skip_code` (see §7) |

**Per-workflow query/response types**

| File | Key types |
|------|-----------|
| `query.rs` | `WebSearchRequest`, `SearchDateRange`, `Freshness`, `SafeSearch`, `SearchIntent`, `MaxResultsResolution`, domain/language/region helpers |
| `repo_query.rs` | `RepoQueryHints` (owner/repo/path/language/symbol) |
| `repo_search.rs` | `RepoSearchRequest` / `RepoSearchResponse`, `RepoResultGroup` / `RepoResultGroupKind`, `RepoSuggestedFetch`, `ResolvedRepoIdentity` / `RepoIdentitySource`, `RepoSearchMode`, `SearchProfile`, telemetry trio |
| `repo_fetch.rs` | `RepoFetchRequest` / `RepoFetchResponse`, `RepoLocator` / `RepoLocatorKind`, `RepoFetchedLine`, `CodeSpanEvidence`, `FetchTrust`, forge URL builders |
| `repo_map.rs` | `RepoMapRequest` / `RepoMapResponse`, entries/summaries/telemetry above |
| `security.rs` | `SecuritySearchRequest` / `SecuritySearchResponse`, `SecurityResultGroup` / `SecurityResultGroupKind`, `SecuritySuggestedFetch`, `VulnerabilityMetadata` / `VulnerabilitySummary` / `VulnerabilityReference` / `VulnerabilitySource`, `SecurityIdentifier(s)` / `SecurityIdentifierKind`, `SecurityQueryKind`, `SecuritySourceTier` / `SecuritySourceClass` / `SecuritySourceQuality`, `SecurityRankReason`, `SeverityLevel`, `SecurityContext` / `CompactSecurityContext`, `SecurityEvidenceSummary`, `SecurityRemediation` / `RemediationCategory`, `DefensiveGuidance` / `DefensiveGuidanceCategory`, `AffectedPackageSummary`, `KevMetadata`, `TextSafetyWarning` / `TextSafetyCategory`, `assess_source_quality`, `classify_source_tier`, `classify_query_kind`, `build_identifier_list` |
| `security_applicability.rs` | `ApplicabilityAssessment`, `ApplicabilityStatus`, `ApplicabilityConfidence`, `AdvisoryRange`, `RangeMatch`, `DependencyFinding`, `DependencyRelation`, `DependencySource` |
| `research.rs` | `ResearchSearchRequest` / `ResearchSearchResponse`, `ResearchWorkflow` / `ResearchWorkflowContext`, `ResearchDepth`, `ResearchDomain`, `ResearchDimension`, `ResearchCoverage`, `ResearchSubquery`, `ResearchResultGroup`, `ResearchSuggestedFetch`, `ResearchClaim` / `ResearchClaimType`, `ResearchGap` / `ResearchGapKind`, `ResearchConflict`, `ResearchEvidenceGap` / `ResearchEvidenceGapKind`, `ResearchSourceType` / `ResearchSourceClass` / `ResearchSourceQuality`, `EvidenceQuality`, `ResearchQualitySignal`, `ResearchTelemetry` |
| `local.rs` | `LocalConfig`, `LocalSearchRequest` / `LocalSearchResult`, `LocalFileEntry`, `LocalMatch`, `InventoryTelemetry`, `FreshnessConfidence`, skip/binary helpers |
| `package.rs` | `PackageCoordinate`, `PackageEcosystem`, `PackageResolution`, `ecosystem_to_osv`, `user_ecosystem_to_osv` |
| `error_query.rs` | `ErrorQueryParts`, `ErrorCode`, `StackFrameHint`, `ErrorSearchContext`, `ErrorSubquery`, `ExactErrorConfig` |
| `code_context.rs` | `CodeContext`, `ExtractionLanguage`, `detect_language`, `detect_language_str`, `extract_code_context`, `extract_imports`, `find_enclosing_symbol` |
| `code_evidence.rs` | `CodeEvidence`, `SourceRole`, `SymbolKind`, `EvidenceConfidence`, `CodeEvidenceReason`, `infer_source_role`, `build_code_evidence` |
| `code_host_fetch.rs` | `CodeHostFetchTarget`, `resolve_code_host_fetch_target` |
| `code_metadata.rs` | `CodeHost`, `CodeMetadata`, `classify_and_extract`, `parse_github_url`, `parse_gitlab_url`, `parse_codeberg_url`, `language_from_extension` |

**Fetch execution types**

| File | Key types / functions |
|------|-----------------------|
| `fetch.rs` | `WebFetchRequest`, `WebFetchResponse`, `ExtractMode`, `ExtractedLink` / `LinkKind`, `FetchTransform` / `FetchTransformKind`, `FetchTrust`, `FetchCachePolicy`, `FocusedFetchSelection`, `PdfFetchOptions` / `PdfOcrPolicy`, `RedactedString`, `MAX_FOCUS_CHUNKS` (5), `MAX_FOCUS_QUERY_CHARS` (512), `MAX_CACHE_AGE_SECONDS` |
| `fetch_policy.rs` | Focus/cache/char-budget validators above, `focus_max_chunks_or_default`, `focus_max_chars_or_default`, `cache_policy_or_default`, `apply_focus_to_document`, `truncate_utf8_safe` |
| `fetch_locator.rs` | `FetchLocator` (`WebUrl` / `Repo` / `Local`), `LocalLocator`, `BatchRepoHost`, URL/path/host validators, `batch_repo_item_to_locator`, `structured_repo_fetch_to_batch_item`, `url_to_batch_web_item` |
| `focus.rs` | `select_focus_chunks`, `select_focus_for_text`, `synthetic_document_for_text` (deterministic lexical ranking) |
| `batch_fetch.rs` | `BatchFetchItem` / `BatchFetchItemType`, `BatchFetchResult`, `BatchFetchResponse`, `BatchFetchTelemetry` |

**Evidence subsystem**

| File | Key types / functions |
|------|-----------------------|
| `evidence_bundle.rs` | `EvidenceBundle` (`bundle_id`), `EvidenceBundleRequest`, `EvidenceBundleSource`, `EvidenceBundleFetchedItem`, `EvidenceBundleLink` / `EvidenceBundleLinkReason`, `EvidenceBundleLimits`, `EvidenceGap` / `EvidenceGapKind`, `EvidenceTrustSummary`, `EvidenceProviderSummary` / `EvidenceProviderCount`, `EvidenceSourceInput`, `EvidenceFetchInput`, `compute_bundle_id`, default/cap constants |
| `evidence_role.rs` | `EvidenceRole` (19 roles; `from_source_kind` maps `SourceKind` to role) |
| `workflow_coverage.rs` | `WorkflowKind` (10 kinds), `WorkflowCoverageModel`, `WorkflowCoverageRequest` / `WorkflowCoverageResult`, `CoverageStatus`, `RetrievalFailure` / `RetrievalFailureKind`, `ResolutionSource`, `WorkflowResolutionContext`, `*_model()` constructors, `compute_coverage`, `coverage_status`, `generate_gap_driven_next_actions` |
| `conflict.rs` | `EvidenceConflict`, `ConflictClass`, `ConflictSeverity`, `ConflictResolution`, `ConflictDetector`, `ConflictEntityKey` / `ConflictEntityType`, `SourcedValue`, `detect_version_range_conflicts`, `detect_date_conflicts`, `detect_provider_metadata_conflicts`, `detect_mutable_vs_pinned`, `detect_entity_scoped_conflicts`, `detect_benchmark_conflicts`, `extract_entity_key` |
| `evidence_postprocess.rs` | `EvidencePostprocessResult`, `EvidenceRoleSummary` / `RoleCount`, `postprocess`, `assign_evidence_role`, `materialize_evidence_roles`, `compute_evidence_role_summary`, `detect_structured_conflicts`, `resolve_workflow_model`, `resolve_workflow_model_with_context`, retrieval-summary builders |
| `retrieval_status.rs` | `RetrievalAttempt` / `RetrievalAttemptOutcome`, `RetrievalDimensionStatus` / `RetrievalDimensionState`, `ResponseRetrievalSummary`, `EvidenceAbsenceKind`, `TruncationEvidence`, `RetrievalOperationIdentity`, `AttemptLedgerViolation`, `AttemptSummaryCounts`, `summarize_retrieval`, `summarize_retrieval_with_attempts`, `validate_attempt_ledger`, `attempts_to_failures`, `classify_absence`, `is_absence_only` / `is_failure_only` / `has_indeterminate`, `absent_roles`, `failed_providers`, `attempt_outcome_to_dimension_state`, `query_fingerprint_from_query` |
| `workflow.rs` | `AgentWorkflowRecipe`, `AgentWorkflowStep`, `AgentWorkflowFallback`, `AgentNextAction`, `RecipeDetail`, `RecipeSupport`, `STABLE_TOOL_NAMES` (10 tools), `is_stable_tool`, `sanitize_next_actions`, `MAX_NEXT_ACTIONS` (5) |

`CacheScope::Profile` keys off the opaque profile ID, never the display name; explicit invalid browser paths surface as `ExplicitPathInvalid` without auto-discovery fallback.

**Inventory note.** `ls src/core/` shows 38 files; `mod.rs` is the declaration point and `src/lib.rs` documents `core` as the intentionally reusable request/response/config/identity surface. The rest of the crate (`meta`, `fetch`, `mcp`, `commands`) may change without a major bump; `core` changes still respect additive evolution per §9.

---

## 5. Deterministic ID scheme

`identity.rs` implements FNV-1a 64-bit over length-prefixed fields, always preceded by the versioned prefix `eggsearch-id-v1\0` plus an entity sub-namespace (`entity_prefix` / `write_entity_prefix`). 16 lowercase hex chars follow a human-readable tag:

| Prefix | Builder | Content hashed |
|--------|---------|----------------|
| `src_` | `source_id` / `compute_source_id` (`SourceKey`) | Canonical URL + title + snippet |
| `fetch_` | `fetch_id` / `compute_fetch_id` (`FetchKey`) | Canonical URL |
| `suggested_` | `suggested_fetch_id` / `compute_suggested_fetch_id` (`SuggestedFetchKey`) | URL + group + priority |
| `batch_` | `batch_fetch_id` / `compute_batch_fetch_id` (`BatchFetchKey`) | Label + index |
| `loc_` | `locator_id` / `compute_locator_id` (`RepoLocatorKey`, `normalize_locator_key`) | Owner/repo/path/ref |
| `doc_` | `doc_id` / `compute_doc_id` (`DocKey`) | URL/title/kind |
| `chunk_` | `chunk_id` / `compute_chunk_id` (`DocChunkKey`) | Doc ID + index + heading path |
| `span_` | `code_span_id` / `compute_code_span_id` (`CodeSpanKey`) | URL + symbol + line range |
| `bundle_` | `compute_bundle_id` (in `evidence_bundle.rs`) | Goal + source/fetch inputs |

`canonicalize_url` normalizes before hashing: lowercase scheme/host, strip `www.`, drop default ports and fragments, normalize percent-encoding. Length-prefixing (`write_str`) prevents field-boundary ambiguity. Bumping `eggsearch-id-v1` migrates the algorithm without cross-version collisions. Changing any hashed field breaks corpus regression and cross-tool dedup by design, so ID semantics are frozen.

---

## 6. Sanitization pipeline

`sanitize.rs` exposes four primitives; callers compose them according to the `sanitize_output` flag (production default `true`, tests default `false`):

1. **Strip** — `strip_control_chars` removes NUL/CR, ASCII control ranges, bidi controls (U+200E-200F, U+202A-202E, U+2066-2069), zero-width chars (U+200B-200D, U+FEFF), and U+2028-2029 separators; preserves LF/TAB. Returns the cleaned string plus removal count.
2. **Bound** — `bound_text` clamps to `max_chars` (`TITLE_MAX_CHARS` 200, `SNIPPET_MAX_CHARS` 500, excerpt caps in `source_card.rs`) with word-safe `truncate_at_word` where applicable; truncation appends `…` and sets the truncated flag.
3. **Frame** — `frame` wraps output in `<<<EXTERNAL_UNTRUSTED field=... id=...>>>` … `<<<END>>>` delimiters when `sanitize_output` is on (Tier 2).
4. **Scan** — `scan_injection_markers` reports `MarkerHit` entries (`ignore_previous`, `disregard_all`, `system_colon`, `assistant_colon`, `im_start`, `im_end`, `chatml_tag` families) without mutating input (Tier 3).

`TrustMarkers` (`text_sanitized`, `text_truncated`, `text_framed`, `control_chars_removed`, `injection_hits`) records what happened per field and merges across title + snippet + excerpts via `merge`. Every `SourceCard` and fetch response carries the merged markers so agents can distinguish cleaned content from verbatim upstream text.

---

## 7. Provider model

`ProviderKind`: `HtmlScrape`, `JsonApi`, `ApiKey`, `Local`.

`ProviderCapabilities` carries 24 flags: `supports_safe_search`, `supports_freshness`, `supports_language`, `supports_region`, `supports_domain_filters`, `supports_news`, `supports_code_search`, `supports_repo_filter`, `supports_org_filter`, `supports_path_filter`, `supports_language_filter`, `supports_symbol_hint`, `supports_issue_search`, `supports_release_search`, `supports_result_timestamps`, `supports_security_search`, `supports_package_metadata`, `supports_advisory_lookup_by_id`, `supports_advisory_lookup_by_package`, `supports_exploit_kev_status`, `supports_scholarly_search`, `supports_doi_lookup`, `supports_repo_indexing`, `supports_structured_changelog`.

`KNOWN_PROVIDER_IDS` holds 37 ids (6 generic HTML/JSON, `brave_api`, 9 forge code/issues/releases, 5 advisory, `local_workspace`, 8 registries, 3 scholarly, `sourcegraph`, `firecrawl_developer`, `exa`, `tavily`). `API_PROVIDER_IDS` requires `api_key_env`; `OPTIONAL_API_PROVIDER_IDS` stays routable keyless. `CredentialRequirement` (`None` / `Optional` / `Required`) and `provider_configured_state` drive `ProviderDescriptor` (built by `built_in_provider_descriptor`) and skip reporting (`ProviderSkipCode`, `provider_skip_code`, `CapabilityOption`). Native enforcement matrix: `brave_api` natively enforces safe-search, freshness/date-range, language, region, news; `exa` freshness/date-range, domain filters, timestamps; `tavily` safe-search, freshness/date-range, language, region, domain filters, news. Domain filters are natively enforced only by `exa`/`tavily`; everything else is local approximation recorded in capability telemetry rather than silently dropped.

---

## 8. Quality, warnings, and workflow guidance

`quality.rs` scores what dispatch produced: `ResultQuality` per card (`ResultConfidence`, `RelevanceEstimate`, `AuthorityEstimate`, `FreshnessEstimate`, `EvidenceStrength`, `QualityReason`) via `compute_card_quality` / `compute_card_quality_with_now`, rolled up into `GroupQualitySummary` and `SearchUncertaintySummary` (`UncertaintyReason`) via `compute_group_quality`. Scores are advisory — they never change IDs or ordering guarantees, only annotate them.

`warning.rs` / `result.rs` separate transport from advice. `SearchWarning` is the adapter-level record; `convert_warnings` / `convert_fetch_warnings` project it into `AgentWarning` (`WarningCode`, `WarningSeverity`) accumulated by `WarningAccumulator` with dedup. Codes cover untrusted content, injection markers, unenforced safe-search/freshness, missing native code/issues/releases/advisory providers, budget caps, and offline skips. Warnings are additive: new codes appear as new enum variants, never as reworded strings.

`workflow.rs` / `workflow_coverage.rs` turn coverage into next steps. `WorkflowKind` has 10 variants (`ApiComprehension`, `RepositoryArchitecture`, `ErrorInvestigation`, `VersionMigration`, `SecurityReview`, `DependencyEvaluation`, `PerformanceInvestigation`, `ComparativeResearch`, `PreChangeEvidence`, `PostChangeReview`), each with a `*_model()` constructor (`api_comprehension_model`, `repo_architecture_model`, and so on) yielding a `WorkflowCoverageModel`. `compute_coverage` / `coverage_status` produce `WorkflowCoverageResult` (`CoverageStatus`, `RetrievalFailure` / `RetrievalFailureKind`, `ResolutionSource`) over a `WorkflowResolutionContext`; `generate_gap_driven_next_actions` proposes follow-ups. `AgentWorkflowRecipe` / `AgentWorkflowStep` / `AgentWorkflowFallback` / `AgentNextAction` carry the executable form, gated by `is_stable_tool` over `STABLE_TOOL_NAMES` (all 10 tools) and clamped by `sanitize_next_actions` to `MAX_NEXT_ACTIONS` (5).

---

## 9. Schema evolution

 Additive only: new optional fields with `#[serde(default)]` / `skip_serializing_if`. `SourceCard.excerpts` is the canonical example — opt-in bounded passages excluded from stable IDs so enabling them never changes `src_` hashes. Timestamps follow the same rule: `SourceMetadata.published_at` is set only from parseable provider evidence via `parse_result_timestamp` (never inferred from snippet text) and feeds freshness reranking without altering identity. `EvidenceBundle` preserves identity across `response_detail` modes. Any removal, rename, or ID-semantic change fails corpus regression (`corpus_runner`) and contract tests by design.

---

## Links

- [← Back to Overview](overview.md) — bird's-eye view, dependency flow, component index
- [Configuration model](config.md) — `AppConfig` field reference, provider resolution, validation rules
- [Evidence & workflow](evidence-workflow.md) — roles, bundles, coverage models, conflicts, retrieval ledger in motion
- [MCP server & tools](mcp.md) — how these types are validated, sanitized, and serialized across the 10 tools
- Operator docs in `docs/` (config reference, safety, threat model) build on this type model without restating it.
