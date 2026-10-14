from __future__ import annotations

from typing import Any, Literal, Required, TypedDict


JSONValue = str | int | float | bool | None | list["JSONValue"] | dict[str, "JSONValue"]
ResponseDetail = Literal["compact", "standard", "diagnostic"]
TrustLabel = Literal["external_untrusted", "local_trusted"]


class TrustMarkers(TypedDict, total=False):
    stripped_control_chars: bool
    framed: bool
    injection_markers: list[str]


class AgentWarning(TypedDict, total=False):
    code: str
    message: str
    provider_id: str
    source_id: str
    repair: dict[str, JSONValue]


class RepairHint(TypedDict, total=False):
    code: str
    message: str
    arguments: dict[str, JSONValue]


class ProviderFailure(TypedDict, total=False):
    provider_id: str
    failure_class: str
    message: str
    retryable: bool
    http_status: int


class SourceCard(TypedDict, total=False):
    id: str
    title: str
    url: str
    snippet: str
    provider_id: str
    score: float
    rank: int
    trust: TrustLabel
    trust_markers: TrustMarkers
    evidence_role: str
    metadata: dict[str, JSONValue]
    excerpts: list[dict[str, JSONValue]]
    next_actions: list[dict[str, JSONValue]]


class RetrievalSummary(TypedDict, total=False):
    status: str
    providers_failed: list[ProviderFailure]
    has_failures: bool
    has_absences: bool
    has_truncation: bool
    attempts: list[dict[str, JSONValue]]


class FetchDocument(TypedDict, total=False):
    title: str
    text: str
    content_type: str
    truncated: bool
    total_chars: int
    trust: TrustLabel
    trust_markers: TrustMarkers


class FetchResult(TypedDict, total=False):
    fetch_id: str
    source_id: str
    url: str
    fetched: bool
    document: FetchDocument
    error: dict[str, JSONValue]


class EvidenceBundle(TypedDict, total=False):
    bundle_id: str
    goal: str
    sources: list[dict[str, JSONValue]]
    fetches: list[dict[str, JSONValue]]
    gaps: list[dict[str, JSONValue]]
    warnings: list[AgentWarning]
    trust: TrustLabel


class EvidenceSourceInput(TypedDict, total=False):
    id: Any
    url: Any
    title: Any
    snippet: Any
    providers: Any
    score: Any
    trust: Any
    trust_markers: Any
    metadata: Any
    quality: Any


class EvidenceFetchInput(TypedDict, total=False):
    source_id: Any
    url: Any
    locator: Any
    fetched: Any
    content_type: Any
    language: Any
    selected_span: Any
    code_span_id: Any
    line_start: Any
    line_end: Any
    text: Any
    truncated: Any
    trust: Any
    trust_markers: Any
    warnings: Any


class WebSearchRequest(TypedDict, total=False):
    query: Required[str]
    max_results: int | None
    providers: list[str]
    safe_search: str | None
    timeout_ms: int | None
    intent: str | None
    freshness: str | None
    date_range: dict[str, str] | None
    include_domains: list[str]
    exclude_domains: list[str]
    language: str | None
    region: str | None
    excerpt_count: int | None
    response_detail: ResponseDetail | None


class WebFetchRequest(TypedDict, total=False):
    url: Required[str]
    max_chars: int | None
    timeout_ms: int | None
    extract_mode: str | None
    include_links: bool | None
    pdf: dict[str, JSONValue] | None
    cache_policy: str | None
    max_cache_age_seconds: int | None
    focus: str | None
    focus_max_chunks: int | None
    focus_max_chars: int | None
    render: str | None
    browser_profile: str | None
    response_detail: ResponseDetail | None


class BatchFetchRequest(TypedDict, total=False):
    items: Required[list[dict[str, JSONValue]]]
    max_items: int | None
    max_chars_per_item: int | None
    max_total_chars: int | None
    timeout_ms: int | None
    continue_on_error: bool | None
    response_detail: ResponseDetail | None


class ProviderStatusRequest(TypedDict, total=False):
    probe: bool
    recipe_detail: Literal["none", "summary", "full"] | None


class RepoSearchRequest(TypedDict, total=False):
    query: str
    host: str | None
    owner: str | None
    repo: str | None
    org: str | None
    path: str | None
    file: str | None
    language: str | None
    symbol: str | None
    goal: str | None
    sources: list[str]
    include_docs: bool | None
    include_registry: bool | None
    include_issues: bool | None
    include_releases: bool | None
    include_examples: bool | None
    include_pull_requests: bool | None
    max_results: int | None
    max_per_group: int | None
    freshness: str | None
    timeout_ms: int | None
    providers: list[str]
    profile: str | None
    ecosystem: str | None
    package: str | None
    version: str | None
    version_requirement: str | None
    package_namespace: str | None
    compare_version: str | None
    include_security_context: bool | None
    include_changelog: bool | None
    include_migration_guides: bool | None
    include_local: bool | None
    mode: str | None
    workflow: str | None
    response_detail: ResponseDetail | None


class RepoFetchRequest(TypedDict, total=False):
    owner: Required[str]
    repo: Required[str]
    path: Required[str]
    host: str | None
    ref_name: str | None
    commit_sha: str | None
    line_start: int | None
    line_end: int | None
    context_before: int | None
    context_after: int | None
    max_chars: int | None
    timeout_ms: int | None
    test_fetch_url: str | None
    symbol: str | None
    symbol_kind: str | None
    match_text: str | None
    expand_to_block: bool | None
    max_block_lines: int | None
    prefer_local: bool | None
    response_detail: ResponseDetail | None


class RepoMapRequest(TypedDict, total=False):
    owner: Required[str]
    repo: Required[str]
    host: str | None
    ref_name: str | None
    commit_sha: str | None
    max_entries: int | None
    max_depth: int | None
    include_files: bool | None
    include_directories: bool | None
    include_ci: bool | None
    include_security: bool | None
    timeout_ms: int | None
    providers: list[str]
    response_detail: ResponseDetail | None


class SecuritySearchRequest(TypedDict, total=False):
    query: str | None
    ecosystem: str | None
    package: str | None
    version: str | None
    cve_id: str | None
    ghsa_id: str | None
    osv_id: str | None
    rustsec_id: str | None
    severity_min: str | None
    include_kev: bool | None
    include_exploit_context: bool | None
    include_defensive_guidance: bool | None
    include_vendor_advisories: bool | None
    include: list[str]
    max_results: int | None
    max_per_group: int | None
    freshness: str | None
    timeout_ms: int | None
    providers: list[str]
    assess_applicability: bool | None
    dependency_files: list[str]
    goal: str | None
    workflow: str | None
    response_detail: ResponseDetail | None


class ResearchSearchRequest(TypedDict, total=False):
    query: Required[str]
    research_domain: str | None
    desired_source_types: list[str]
    include_counterpoints: bool | None
    include_primary_sources: bool | None
    include_recent_discussion: bool | None
    include_security_considerations: bool | None
    include: list[str]
    max_results: int | None
    max_groups: int | None
    max_per_group: int | None
    freshness: str | None
    timeout_ms: int | None
    providers: list[str]
    goal: str | None
    workflow: str | None
    depth: str | None
    compare_targets: list[str]
    constraints: list[str]
    known_context: str | None
    response_detail: ResponseDetail | None


class EvidenceBundleRequest(TypedDict, total=False):
    goal: str | None
    sources: list[EvidenceSourceInput]
    fetches: list[EvidenceFetchInput]
    include_unfetched_sources: bool | None
    max_sources: int | None
    max_fetched_items: int | None
    max_total_chars: int | None
    response_detail: ResponseDetail | None


class ToolResponse(TypedDict, total=False):
    results: list[SourceCard]
    sources: list[SourceCard]
    fetches: list[FetchResult]
    items: list[FetchResult]
    document: FetchDocument
    next_actions: list[dict[str, JSONValue]]
    structured_warnings: list[AgentWarning]
    providers_failed: list[ProviderFailure]
    retrieval_summary: RetrievalSummary
    trust: TrustLabel
    trust_markers: TrustMarkers
    workflow_coverage: dict[str, JSONValue]
    conflict_metadata: list[dict[str, JSONValue]]
    quality_metadata: dict[str, JSONValue]
    errors: list[dict[str, JSONValue]]
    warnings: list[AgentWarning]
    bundle: EvidenceBundle
    bundle_id: str
    gaps: list[dict[str, JSONValue]]


class ToolContract(TypedDict):
    name: str
    input_schema: dict[str, JSONValue]
    output_schema: dict[str, JSONValue] | None


class ToolContractSet(TypedDict):
    fingerprint: str
    tools: list[ToolContract]


WebSearchResponse = ToolResponse
WebFetchResponse = ToolResponse
BatchFetchResponse = ToolResponse
ProviderStatusResponse = ToolResponse
RepoSearchResponse = ToolResponse
RepoFetchResponse = ToolResponse
RepoMapResponse = ToolResponse
SecuritySearchResponse = ToolResponse
ResearchSearchResponse = ToolResponse
EvidenceBundleResponse = ToolResponse


__all__ = [
    "AgentWarning",
    "BatchFetchRequest",
    "BatchFetchResponse",
    "EvidenceBundle",
    "EvidenceFetchInput",
    "EvidenceBundleRequest",
    "EvidenceBundleResponse",
    "EvidenceSourceInput",
    "FetchDocument",
    "FetchResult",
    "JSONValue",
    "ProviderFailure",
    "ProviderStatusRequest",
    "ProviderStatusResponse",
    "RepairHint",
    "RepoFetchRequest",
    "RepoFetchResponse",
    "RepoMapRequest",
    "RepoMapResponse",
    "RepoSearchRequest",
    "RepoSearchResponse",
    "ResearchSearchRequest",
    "ResearchSearchResponse",
    "RetrievalSummary",
    "ResponseDetail",
    "SecuritySearchRequest",
    "SecuritySearchResponse",
    "SourceCard",
    "ToolResponse",
    "ToolContract",
    "ToolContractSet",
    "TrustLabel",
    "TrustMarkers",
    "WebFetchRequest",
    "WebFetchResponse",
    "WebSearchRequest",
    "WebSearchResponse",
]
