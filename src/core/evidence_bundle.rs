//! Evidence bundle types for multi-agent handoff.
//!
//! An evidence bundle is a deterministic, non-summarizing structured
//! evidence container that agents can pass across manager, coder,
//! reviewer, security, and research loops without repeating search
//! and fetch work. The bundle preserves source IDs, provenance, trust
//! markers, selected fetch spans, quality signals, unresolved gaps,
//! and provider diagnostics.
//!
//! The bundle is NOT a conclusion. It does NOT summarize or
//! reinterpret untrusted content. It packages already-selected
//! evidence and metadata for reuse by other agents.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::core::fetch::FetchTrust;
use crate::core::quality::ResultQuality;
use crate::core::repo_fetch::RepoLocator;
use crate::core::result::{SearchWarning, TrustLevel};
use crate::core::sanitize::TrustMarkers;
use crate::core::source_card::{SourceKind, SourceMetadata};
use crate::fetch::span::SelectedSpan;

/// Deterministic gap kind derived from response fields. Each variant
/// represents a specific evidence gap that can be programmatically
/// detected from existing search/fetch response metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceGapKind {
    /// No primary source found for the query.
    NoPrimarySourceFound,
    /// A provider was degraded during the search.
    ProviderDegraded,
    /// Native repo filter was not enforced.
    NativeRepoFilterNotEnforced,
    /// Security applicability could not be determined.
    SecurityApplicabilityUnknown,
    /// A fetch operation failed.
    FetchFailed,
    /// A source was included but not fetched.
    SourceUnfetched,
    /// All results are external untrusted content.
    AllResultsExternalUntrusted,
    /// A local checkout is dirty (uncommitted changes).
    LocalCheckoutDirty,
    /// Native advisory provider was unavailable.
    NativeAdvisoryUnavailable,
    /// Symbol hint had no native provider.
    SymbolHintNoNativeProvider,
    /// Issue search had no native provider.
    IssueSearchNoNativeProvider,
    /// Release search had no native provider.
    ReleaseSearchNoNativeProvider,
    /// Freshness was not enforced by any provider.
    FreshnessNotEnforced,
    /// Package resolution failed.
    PackageResolutionFailed,
    /// No fixed version found for a vulnerability.
    NoFixedVersionFound,
    /// No counterpoint found when requested.
    NoCounterpointFound,
    /// No benchmarks found when requested.
    NoBenchmarksFound,
}

/// A deterministic gap extracted from response metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceGap {
    /// The kind of gap.
    pub kind: EvidenceGapKind,
    /// Human-readable description of the gap.
    pub message: String,
    /// Optional source ID this gap relates to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    /// Optional provider ID that produced the gap signal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
}

/// A source entry in an evidence bundle, derived from a `SourceCard`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceBundleSource {
    /// Deterministic source ID: `src_<hash>`.
    pub source_id: String,
    /// Original source card ID, if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_id: Option<String>,
    /// Source URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Source title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Deterministic source-kind classification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<SourceKind>,
    /// Source role (implementation, test, documentation, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_role: Option<String>,
    /// Provider that contributed this source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    /// Rank position in the original response.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<usize>,
    /// Aggregate score (e.g. RRF).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    /// Deterministic rank-reason tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rank_reasons: Vec<String>,
    /// Trust label for this source.
    pub trust: TrustLevel,
    /// Trust markers from sanitization of title/snippet.
    #[serde(default)]
    pub trust_markers: TrustMarkers,
    /// Deterministic quality and uncertainty metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<ResultQuality>,
    /// Whether this source has stable content (e.g. pinned commit).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stable: Option<bool>,
    /// Structured repo-fetch locator, if this source can be fetched
    /// via `repo_fetch`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_repo_fetch: Option<RepoLocator>,
    /// Full source metadata from the original card.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<SourceMetadata>,
}

/// A fetched item in an evidence bundle, derived from a fetch response.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceBundleFetchedItem {
    /// Deterministic fetch ID: `fetch_<hash>`.
    pub fetch_id: String,
    /// Source ID this fetch is linked to, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    /// URL that was fetched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Structured repo locator, if fetched via `repo_fetch`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locator: Option<RepoLocator>,
    /// Whether the fetch succeeded.
    pub fetched: bool,
    /// Content-Type of the response.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Detected language of the content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Selected span metadata, if a symbol/line range was used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_span: Option<SelectedSpan>,
    /// Effective line start (1-indexed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_start: Option<u32>,
    /// Effective line end (1-indexed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_end: Option<u32>,
    /// Extracted text content (bounded).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Whether the text was truncated.
    pub truncated: bool,
    /// Trust label for this fetched content.
    pub trust: FetchTrust,
    /// Trust markers from sanitization.
    #[serde(default)]
    pub trust_markers: TrustMarkers,
    /// Warnings from the fetch operation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// A link between a source and a fetched item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceBundleLink {
    /// Source ID.
    pub source_id: String,
    /// Fetch ID.
    pub fetch_id: String,
    /// How the link was established.
    pub link_reason: EvidenceBundleLinkReason,
}

/// How a source-to-fetch link was established.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceBundleLinkReason {
    /// URL matched between source and fetch.
    UrlMatch,
    /// Structured locator matched.
    LocatorMatch,
    /// Explicitly linked by the caller.
    Explicit,
    /// Source ID was provided on the fetch input.
    SourceIdMatch,
}

/// Aggregated trust summary across all sources and fetched items.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceTrustSummary {
    /// Number of sources with `external_untrusted` trust.
    pub external_untrusted_count: usize,
    /// Number of sources with `local_trusted` trust.
    pub local_trusted_count: usize,
    /// Total number of injection markers detected across all sources.
    pub total_injection_hits: usize,
    /// Total number of control characters removed.
    pub total_control_chars_removed: usize,
    /// Whether any text was sanitized.
    pub any_text_sanitized: bool,
    /// Whether any text was truncated.
    pub any_text_truncated: bool,
    /// Whether any text was framed.
    pub any_text_framed: bool,
}

/// Aggregated provider summary across all sources.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceProviderSummary {
    /// Unique provider IDs that contributed sources.
    pub providers_used: Vec<String>,
    /// Number of sources per provider.
    pub per_provider_counts: Vec<EvidenceProviderCount>,
}

/// Source count for a single provider.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceProviderCount {
    /// Provider ID.
    pub provider_id: String,
    /// Number of sources from this provider.
    pub count: usize,
}

/// Bundle-level limits that were applied.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceBundleLimits {
    /// Maximum sources allowed.
    pub max_sources: usize,
    /// Maximum fetched items allowed.
    pub max_fetched_items: usize,
    /// Maximum total characters across all fetched text.
    pub max_total_chars: usize,
    /// Whether sources were truncated due to limits.
    pub sources_truncated: bool,
    /// Whether fetched items were truncated due to limits.
    pub fetched_items_truncated: bool,
    /// Whether total chars exceeded the budget.
    pub total_chars_exceeded: bool,
}

/// A deterministic, non-summarizing evidence bundle for multi-agent
/// handoff. Packages selected evidence and metadata for reuse without
/// losing source identity or trust context.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceBundle {
    /// Deterministic bundle ID: `bundle_<hash>`.
    pub bundle_id: String,
    /// Optional goal description for this bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    /// ISO 8601 timestamp of when the bundle was created.
    pub created_at: String,
    /// Sources included in the bundle.
    pub sources: Vec<EvidenceBundleSource>,
    /// Fetched items included in the bundle.
    pub fetched_items: Vec<EvidenceBundleFetchedItem>,
    /// Links between sources and fetched items.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_links: Vec<EvidenceBundleLink>,
    /// Aggregated trust summary.
    pub trust_summary: EvidenceTrustSummary,
    /// Aggregated provider summary.
    pub provider_summary: EvidenceProviderSummary,
    /// Deterministic gaps detected from the evidence.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gaps: Vec<EvidenceGap>,
    /// Warnings carried over from search/fetch responses.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<SearchWarning>,
    /// Limits that were applied to this bundle.
    pub limits: EvidenceBundleLimits,
}

/// Input for a source card from a search response.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceSourceInput {
    /// Source card ID (e.g. `src_<uuid>`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Source URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Source title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Snippet text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    /// Providers that contributed this source.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers: Vec<String>,
    /// Aggregate score.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    /// Trust label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust: Option<TrustLevel>,
    /// Trust markers from sanitization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust_markers: Option<TrustMarkers>,
    /// Full source metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<SourceMetadata>,
    /// Quality metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<ResultQuality>,
}

/// Input for a fetched item from a fetch response.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceFetchInput {
    /// Source ID this fetch is linked to (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    /// URL that was fetched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Structured repo locator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locator: Option<RepoLocator>,
    /// Whether the fetch succeeded.
    #[serde(default = "default_true")]
    pub fetched: bool,
    /// Content-Type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Detected language.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Selected span metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_span: Option<SelectedSpan>,
    /// Effective line start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_start: Option<u32>,
    /// Effective line end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_end: Option<u32>,
    /// Extracted text content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Whether text was truncated.
    #[serde(default)]
    pub truncated: bool,
    /// Trust label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust: Option<FetchTrust>,
    /// Trust markers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust_markers: Option<TrustMarkers>,
    /// Warnings from the fetch.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

fn default_true() -> bool {
    true
}

/// Request to build an evidence bundle.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceBundleRequest {
    /// Optional goal description for this bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    /// Source cards from search responses to include.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<EvidenceSourceInput>,
    /// Fetched items from fetch responses to include.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fetches: Vec<EvidenceFetchInput>,
    /// Whether to include unfetched sources (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_unfetched_sources: Option<bool>,
    /// Maximum number of sources (default 50).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_sources: Option<usize>,
    /// Maximum number of fetched items (default 20).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_fetched_items: Option<usize>,
    /// Maximum total characters across all fetched text (default 100000).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_total_chars: Option<usize>,
    /// Warnings to carry into the bundle from prior responses.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<SearchWarning>,
}

/// Default maximum sources in a bundle.
pub const DEFAULT_MAX_SOURCES: usize = 50;
/// Default maximum fetched items in a bundle.
pub const DEFAULT_MAX_FETCHED_ITEMS: usize = 20;
/// Default maximum total characters across all fetched text.
pub const DEFAULT_MAX_TOTAL_CHARS: usize = 100_000;

/// Server-enforced upper bound on sources.
pub const MAX_SOURCES_CAP: usize = 200;
/// Server-enforced upper bound on fetched items.
pub const MAX_FETCHED_ITEMS_CAP: usize = 100;
/// Server-enforced upper bound on total characters.
pub const MAX_TOTAL_CHARS_CAP: usize = 500_000;

/// Deterministic hash-based source ID.
///
/// `source_id = src_<short_hash(provider_id + url + title + source_kind)>`.
pub fn compute_source_id(
    provider_id: Option<&str>,
    url: Option<&str>,
    title: Option<&str>,
    source_kind: Option<SourceKind>,
) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    provider_id.unwrap_or("").hash(&mut hasher);
    url.unwrap_or("").hash(&mut hasher);
    title.unwrap_or("").hash(&mut hasher);
    format!("{:?}", source_kind).hash(&mut hasher);
    format!("src_{:016x}", hasher.finish())
}

/// Deterministic hash-based fetch ID.
///
/// `fetch_id = fetch_<short_hash(url_or_locator + line_range + text_hash_prefix)>`.
pub fn compute_fetch_id(
    url: Option<&str>,
    locator: Option<&RepoLocator>,
    line_start: Option<u32>,
    line_end: Option<u32>,
    text_prefix: Option<&str>,
) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    if let Some(loc) = locator {
        format!("{:?}", loc).hash(&mut hasher);
    } else {
        url.unwrap_or("").hash(&mut hasher);
    }
    line_start.hash(&mut hasher);
    line_end.hash(&mut hasher);
    // Hash first 64 chars of text for content stability
    let prefix = text_prefix.unwrap_or("");
    let prefix = if prefix.len() > 64 {
        &prefix[..64]
    } else {
        prefix
    };
    prefix.hash(&mut hasher);
    format!("fetch_{:016x}", hasher.finish())
}

/// Deterministic bundle ID from canonicalized content.
pub fn compute_bundle_id(
    goal: Option<&str>,
    source_ids: &[String],
    fetch_ids: &[String],
) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    goal.unwrap_or("").hash(&mut hasher);
    for id in source_ids {
        id.hash(&mut hasher);
    }
    for id in fetch_ids {
        id.hash(&mut hasher);
    }
    format!("bundle_{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_id_deterministic() {
        let id1 = compute_source_id(
            Some("duckduckgo"),
            Some("https://docs.rs/axum"),
            Some("axum - Rust"),
            Some(SourceKind::OfficialDocs),
        );
        let id2 = compute_source_id(
            Some("duckduckgo"),
            Some("https://docs.rs/axum"),
            Some("axum - Rust"),
            Some(SourceKind::OfficialDocs),
        );
        assert_eq!(id1, id2);
        assert!(id1.starts_with("src_"));
    }

    #[test]
    fn source_id_changes_with_url() {
        let id1 = compute_source_id(
            Some("duckduckgo"),
            Some("https://docs.rs/axum"),
            Some("axum - Rust"),
            Some(SourceKind::OfficialDocs),
        );
        let id2 = compute_source_id(
            Some("duckduckgo"),
            Some("https://crates.io/crates/axum"),
            Some("axum - Rust"),
            Some(SourceKind::OfficialDocs),
        );
        assert_ne!(id1, id2);
    }

    #[test]
    fn source_id_changes_with_title() {
        let id1 = compute_source_id(
            Some("duckduckgo"),
            Some("https://docs.rs/axum"),
            Some("axum - Rust"),
            Some(SourceKind::OfficialDocs),
        );
        let id2 = compute_source_id(
            Some("duckduckgo"),
            Some("https://docs.rs/axum"),
            Some("axum web framework"),
            Some(SourceKind::OfficialDocs),
        );
        assert_ne!(id1, id2);
    }

    #[test]
    fn source_id_changes_with_kind() {
        let id1 = compute_source_id(
            Some("duckduckgo"),
            Some("https://docs.rs/axum"),
            Some("axum - Rust"),
            Some(SourceKind::OfficialDocs),
        );
        let id2 = compute_source_id(
            Some("duckduckgo"),
            Some("https://docs.rs/axum"),
            Some("axum - Rust"),
            Some(SourceKind::PackageRegistry),
        );
        assert_ne!(id1, id2);
    }

    #[test]
    fn source_id_changes_with_provider() {
        let id1 = compute_source_id(
            Some("duckduckgo"),
            Some("https://docs.rs/axum"),
            Some("axum - Rust"),
            Some(SourceKind::OfficialDocs),
        );
        let id2 = compute_source_id(
            Some("brave"),
            Some("https://docs.rs/axum"),
            Some("axum - Rust"),
            Some(SourceKind::OfficialDocs),
        );
        assert_ne!(id1, id2);
    }

    #[test]
    fn fetch_id_deterministic() {
        let id1 = compute_fetch_id(
            Some("https://raw.githubusercontent.com/axum/axum/main/src/lib.rs"),
            None,
            Some(1),
            Some(50),
            Some("use tower::Service;"),
        );
        let id2 = compute_fetch_id(
            Some("https://raw.githubusercontent.com/axum/axum/main/src/lib.rs"),
            None,
            Some(1),
            Some(50),
            Some("use tower::Service;"),
        );
        assert_eq!(id1, id2);
        assert!(id1.starts_with("fetch_"));
    }

    #[test]
    fn fetch_id_changes_with_range() {
        let id1 = compute_fetch_id(
            Some("https://example.com/src.rs"),
            None,
            Some(1),
            Some(50),
            None,
        );
        let id2 = compute_fetch_id(
            Some("https://example.com/src.rs"),
            None,
            Some(1),
            Some(100),
            None,
        );
        assert_ne!(id1, id2);
    }

    #[test]
    fn fetch_id_changes_with_text() {
        let id1 = compute_fetch_id(
            Some("https://example.com/src.rs"),
            None,
            Some(1),
            Some(50),
            Some("hello"),
        );
        let id2 = compute_fetch_id(
            Some("https://example.com/src.rs"),
            None,
            Some(1),
            Some(50),
            Some("world"),
        );
        assert_ne!(id1, id2);
    }

    #[test]
    fn bundle_id_deterministic() {
        let sources = vec!["src_aaa".to_string(), "src_bbb".to_string()];
        let fetches = vec!["fetch_ccc".to_string()];
        let id1 = compute_bundle_id(Some("debug error"), &sources, &fetches);
        let id2 = compute_bundle_id(Some("debug error"), &sources, &fetches);
        assert_eq!(id1, id2);
        assert!(id1.starts_with("bundle_"));
    }

    #[test]
    fn bundle_id_changes_with_goal() {
        let sources = vec!["src_aaa".to_string()];
        let id1 = compute_bundle_id(Some("goal A"), &sources, &[]);
        let id2 = compute_bundle_id(Some("goal B"), &sources, &[]);
        assert_ne!(id1, id2);
    }

    #[test]
    fn bundle_id_changes_with_sources() {
        let id1 = compute_bundle_id(None, &["src_aaa".to_string()], &[]);
        let id2 = compute_bundle_id(None, &["src_bbb".to_string()], &[]);
        assert_ne!(id1, id2);
    }

    #[test]
    fn source_id_none_fields() {
        let id = compute_source_id(None, None, None, None);
        assert!(id.starts_with("src_"));
        // Should be deterministic even with all None
        let id2 = compute_source_id(None, None, None, None);
        assert_eq!(id, id2);
    }

    #[test]
    fn fetch_id_roundtrip_serialization() {
        let item = EvidenceBundleFetchedItem {
            fetch_id: "fetch_aabbccdd".to_string(),
            source_id: Some("src_112233".to_string()),
            url: Some("https://example.com".to_string()),
            locator: None,
            fetched: true,
            content_type: None,
            language: None,
            selected_span: None,
            line_start: Some(1),
            line_end: Some(10),
            text: Some("fn main() {}".to_string()),
            truncated: false,
            trust: FetchTrust::ExternalUntrusted,
            trust_markers: TrustMarkers::default(),
            warnings: vec![],
        };
        let json = serde_json::to_value(&item).unwrap();
        let restored: EvidenceBundleFetchedItem = serde_json::from_value(json).unwrap();
        assert_eq!(item.fetch_id, restored.fetch_id);
        assert_eq!(item.source_id, restored.source_id);
        assert_eq!(item.url, restored.url);
        assert_eq!(item.fetched, restored.fetched);
        assert_eq!(item.text, restored.text);
    }

    #[test]
    fn source_card_roundtrip_serialization() {
        let source = EvidenceBundleSource {
            source_id: "src_aabbccdd".to_string(),
            original_id: Some("src_uuid123".to_string()),
            url: Some("https://docs.rs/axum".to_string()),
            title: Some("axum".to_string()),
            source_kind: Some(SourceKind::OfficialDocs),
            source_role: Some("documentation".to_string()),
            provider_id: Some("duckduckgo".to_string()),
            rank: Some(0),
            score: Some(0.95),
            rank_reasons: vec!["rrf_multi_provider".to_string()],
            trust: TrustLevel::ExternalUntrusted,
            trust_markers: TrustMarkers::default(),
            quality: None,
            stable: None,
            structured_repo_fetch: None,
            metadata: None,
        };
        let json = serde_json::to_value(&source).unwrap();
        let restored: EvidenceBundleSource = serde_json::from_value(json).unwrap();
        assert_eq!(source.source_id, restored.source_id);
        assert_eq!(source.url, restored.url);
        assert_eq!(source.title, restored.title);
        assert_eq!(source.trust, restored.trust);
    }

    #[test]
    fn gap_serialization() {
        let gap = EvidenceGap {
            kind: EvidenceGapKind::FetchFailed,
            message: "fetch timed out".to_string(),
            source_id: None,
            provider_id: Some("duckduckgo".to_string()),
        };
        let json = serde_json::to_value(&gap).unwrap();
        assert_eq!(json["kind"], "fetch_failed");
        let restored: EvidenceGap = serde_json::from_value(json).unwrap();
        assert_eq!(gap, restored);
    }
}
