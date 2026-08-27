//! Deterministic result quality and uncertainty metadata.
//!
//! Quality fields are **heuristic metadata** computed from URL/domain
//! heuristics, provider signals, and structured result metadata. They
//! are NOT truth judgments or factual correctness claims. Agents should
//! use them to decide when to fetch more evidence, not as proof of
//! accuracy.

use serde::{Deserialize, Serialize};

/// Overall confidence that a result is relevant and accurate.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ResultConfidence {
    /// High confidence: structured evidence, exact match, authoritative source.
    High,
    /// Medium confidence: recognizable source but missing some signals.
    Medium,
    /// Low confidence: only title/snippet imply relevance.
    Low,
    /// Cannot determine confidence (default).
    #[default]
    Unknown,
}

/// How well the result matches the query intent.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RelevanceEstimate {
    /// Exact phrase or symbol match in title/snippet.
    Exact,
    /// All query tokens present or strong structural match.
    Strong,
    /// Some query tokens present.
    Partial,
    /// Only provider rank suggests relevance.
    Weak,
    /// Cannot determine relevance (default).
    #[default]
    Unknown,
}

/// Authority tier of the source.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityEstimate {
    /// Direct source (e.g. the exact file, exact symbol definition).
    Primary,
    /// Official documentation or project site.
    Official,
    /// Maintainer-authored content (issue, PR, release notes).
    Maintainer,
    /// Package registry listing.
    PackageRegistry,
    /// Community content (blog, forum, Stack Overflow).
    Community,
    /// News article or press coverage.
    NewsOrBlog,
    /// Cannot determine authority (default).
    #[default]
    Unknown,
}

/// How recent the content is relative to the current time.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessEstimate {
    /// Within the last week.
    Current,
    /// Within the last month.
    Recent,
    /// Older than a month but has a timestamp.
    Historical,
    /// Has no timestamp.
    Undated,
    /// Older than 6 months.
    Stale,
    /// Cannot determine freshness (default).
    #[default]
    Unknown,
}

/// Strength of the evidence behind the result.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStrength {
    /// Exact code span with line range or permalink.
    ExactCodeSpan,
    /// Exact identifier match (symbol, error code, CVE ID).
    ExactIdentifier,
    /// Structured metadata (issue, release, advisory payload).
    StructuredMetadata,
    /// Only a text snippet is available.
    SnippetOnly,
    /// Only a URL is available.
    UrlOnly,
    /// Cannot determine evidence strength (default).
    #[default]
    Unknown,
}

/// Deterministic reason a result has uncertainty.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum UncertaintyReason {
    /// No text snippet available for the result.
    NoSnippet,
    /// No timestamp or date evidence.
    NoTimestamp,
    /// Only generic web providers returned this result.
    GenericProviderOnly,
    /// One or more providers failed before this result was returned.
    ProviderFailed,
    /// The result matches via fuzzy text search, not exact identifiers.
    FuzzyQueryMatch,
    /// No exact phrase match found in title or snippet.
    NoExactPhraseMatch,
    /// The repository or source is ambiguous.
    AmbiguousRepository,
    /// Version match could not be verified against advisory metadata.
    UnverifiedVersionMatch,
    /// Source is low-authority (blog, forum, unknown domain).
    LowAuthoritySource,
    /// Conflicting information across providers.
    ConflictingSources,
    /// Result title or snippet was truncated.
    ResultTruncated,
    /// A fetch is suggested to verify or complete the evidence.
    FetchSuggested,
}

/// Deterministic reason a result has high quality.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum QualityReason {
    /// URL matches exact owner/repo.
    ExactRepoMatch,
    /// URL matches exact file path.
    ExactPathMatch,
    /// Symbol name found in title, snippet, or structured metadata.
    ExactSymbolMatch,
    /// Title or snippet contains the exact error phrase.
    ExactErrorPhraseMatch,
    /// Source is official documentation.
    OfficialDocs,
    /// Source is from a maintainer (issue, PR, release).
    MaintainerSource,
    /// Source is a primary security advisory.
    PrimaryAdvisory,
    /// Source is a package registry with structured metadata.
    PackageRegistryMetadata,
    /// Timestamp is recent (within 30 days).
    FreshTimestamp,
    /// Code evidence has a commit-pinned permalink.
    CommitPinnedEvidence,
    /// Structured code metadata is present (host, path, language).
    StructuredCodeEvidence,
}

/// Per-result quality metadata. All fields are deterministic and
/// computed from URL/domain heuristics, provider signals, and
/// structured result metadata.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResultQuality {
    /// Overall confidence that this result is relevant and accurate.
    #[serde(default)]
    pub confidence: ResultConfidence,
    /// How well the result matches the query intent.
    #[serde(default)]
    pub relevance: RelevanceEstimate,
    /// Authority tier of the source.
    #[serde(default)]
    pub authority: AuthorityEstimate,
    /// How recent the content is.
    #[serde(default)]
    pub freshness: FreshnessEstimate,
    /// Strength of the evidence behind the result.
    #[serde(default)]
    pub evidence_strength: EvidenceStrength,
    /// Deterministic reasons for uncertainty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uncertainty_reasons: Vec<UncertaintyReason>,
    /// Deterministic reasons for high quality.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quality_reasons: Vec<QualityReason>,
}

impl ResultQuality {
    /// Is this result high or medium confidence?
    pub fn is_usable(&self) -> bool {
        matches!(
            self.confidence,
            ResultConfidence::High | ResultConfidence::Medium
        )
    }
}

/// Aggregate quality summary for a group of results.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GroupQualitySummary {
    /// Count of high-confidence results.
    pub high_confidence_count: usize,
    /// Count of low-confidence results.
    pub low_confidence_count: usize,
    /// Count of primary/official authority sources.
    pub primary_source_count: usize,
    /// Count of exact/strong evidence results.
    pub exact_evidence_count: usize,
    /// Aggregate quality warnings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// Aggregate uncertainty summary for a search response.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchUncertaintySummary {
    /// Number of providers that failed.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub provider_failures: usize,
    /// Whether provider selection fell back to defaults.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub degraded_provider_selection: bool,
    /// Whether some providers were skipped but others remain.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub partial_provider_selection: bool,
    /// Number of results with low or unknown confidence.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub low_confidence_results: usize,
    /// Aggregate quality warnings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

fn is_zero_usize(n: &usize) -> bool {
    *n == 0
}

/// Compute `ResultQuality` for a `SourceCard`.
///
/// This is a pure function with no network access. It uses:
/// - `source_kind` from URL heuristics
/// - `code` / `code_evidence` metadata
/// - `issue` / `release` / `vulnerability` structured metadata
/// - `providers` list (generic vs native)
/// - `snippet` presence
/// - `rank_reasons`
pub fn compute_card_quality(card: &crate::core::source_card::SourceCard) -> ResultQuality {
    compute_card_quality_with_now(card, chrono::Utc::now())
}

/// Compute `ResultQuality` for a `SourceCard` with an explicit timestamp.
///
/// Accepts `now` for deterministic testing. Prefer [`compute_card_quality`]
/// for production callers.
pub fn compute_card_quality_with_now(
    card: &crate::core::source_card::SourceCard,
    now: chrono::DateTime<chrono::Utc>,
) -> ResultQuality {
    use crate::core::source_card::{RankReason, SourceKind};

    let mut authority = AuthorityEstimate::Unknown;
    let mut evidence_strength = EvidenceStrength::Unknown;
    let mut uncertainty_reasons: Vec<UncertaintyReason> = Vec::new();
    let mut quality_reasons: Vec<QualityReason> = Vec::new();

    let meta = &card.metadata;
    let has_snippet = card.snippet.is_some();

    // --- Evidence strength ---
    if let Some(ce) = &meta.code_evidence {
        if ce.permalink_url.is_some() || ce.raw_permalink_url.is_some() {
            evidence_strength = EvidenceStrength::ExactCodeSpan;
            quality_reasons.push(QualityReason::CommitPinnedEvidence);
            quality_reasons.push(QualityReason::StructuredCodeEvidence);
        } else if ce.raw_url.is_some() || ce.browser_url.is_some() {
            evidence_strength = EvidenceStrength::ExactCodeSpan;
            quality_reasons.push(QualityReason::StructuredCodeEvidence);
        }
    } else if meta.vulnerability.is_some() || meta.issue.is_some() || meta.release.is_some() {
        evidence_strength = EvidenceStrength::StructuredMetadata;
    } else if has_snippet {
        evidence_strength = EvidenceStrength::SnippetOnly;
    } else {
        evidence_strength = EvidenceStrength::UrlOnly;
        uncertainty_reasons.push(UncertaintyReason::NoSnippet);
    }

    // Check for exact identifier matches in rank reasons
    let has_exact_identifier = meta.rank_reasons.iter().any(|r| {
        matches!(
            r,
            RankReason::AdvisoryIdentifierMatch
                | RankReason::ExactErrorPhraseMatch
                | RankReason::ErrorCodeMatch
        )
    });
    if has_exact_identifier {
        if evidence_strength != EvidenceStrength::ExactCodeSpan {
            evidence_strength = EvidenceStrength::ExactIdentifier;
        }
        quality_reasons.push(QualityReason::ExactErrorPhraseMatch);
    }

    // --- Authority ---
    match meta.source_kind {
        SourceKind::OfficialDocs | SourceKind::Reference => {
            authority = AuthorityEstimate::Official;
            quality_reasons.push(QualityReason::OfficialDocs);
        }
        SourceKind::PackageRegistry => {
            authority = AuthorityEstimate::PackageRegistry;
            quality_reasons.push(QualityReason::PackageRegistryMetadata);
        }
        SourceKind::SecurityAdvisory => {
            authority = AuthorityEstimate::Primary;
            quality_reasons.push(QualityReason::PrimaryAdvisory);
        }
        SourceKind::IssueThread | SourceKind::PullRequest => {
            if card
                .providers
                .iter()
                .any(|p| p.contains("github") || p.contains("gitlab") || p.contains("gitea"))
            {
                authority = AuthorityEstimate::Maintainer;
                quality_reasons.push(QualityReason::MaintainerSource);
            } else {
                authority = AuthorityEstimate::Community;
            }
        }
        SourceKind::ReleaseNotes | SourceKind::Tag | SourceKind::Commit => {
            if card
                .providers
                .iter()
                .any(|p| p.contains("github") || p.contains("gitlab") || p.contains("gitea"))
            {
                authority = AuthorityEstimate::Maintainer;
                quality_reasons.push(QualityReason::MaintainerSource);
            }
        }
        SourceKind::News => authority = AuthorityEstimate::NewsOrBlog,
        SourceKind::Tutorial | SourceKind::Forum => authority = AuthorityEstimate::Community,
        SourceKind::SourceFile
        | SourceKind::SourceRepository
        | SourceKind::RepositoryRoot
        | SourceKind::SourceDirectory => {
            if card
                .providers
                .iter()
                .any(|p| p.contains("github") || p.contains("gitlab") || p.contains("gitea"))
            {
                authority = AuthorityEstimate::Maintainer;
                quality_reasons.push(QualityReason::MaintainerSource);
            }
            if let Some(code) = &meta.code {
                if code.owner.is_some() && code.repo.is_some() && code.path.is_some() {
                    quality_reasons.push(QualityReason::ExactPathMatch);
                }
            }
        }
        _ => {}
    }

    // Low authority for unknown sources
    if authority == AuthorityEstimate::Unknown {
        uncertainty_reasons.push(UncertaintyReason::LowAuthoritySource);
    }

    // --- Confidence ---
    let is_native_provider = card
        .providers
        .iter()
        .any(|p| p.contains("github") || p.contains("gitlab") || p.contains("gitea") || p == "osv");

    let has_structured = meta.issue.is_some()
        || meta.release.is_some()
        || meta.vulnerability.is_some()
        || meta.code_evidence.is_some();

    let has_exact_match = meta.rank_reasons.iter().any(|r| {
        matches!(
            r,
            RankReason::ExactErrorPhraseMatch
                | RankReason::ErrorCodeMatch
                | RankReason::AdvisoryIdentifierMatch
                | RankReason::RepoOwnerMatch
                | RankReason::HintMatch
        )
    });

    let confidence = match meta.source_kind {
        SourceKind::OfficialDocs | SourceKind::PackageRegistry | SourceKind::SecurityAdvisory => {
            ResultConfidence::High
        }
        _ if has_structured && is_native_provider && has_exact_match => ResultConfidence::High,
        _ if has_structured || is_native_provider => ResultConfidence::Medium,
        _ if has_exact_match => ResultConfidence::Medium,
        _ if has_snippet => ResultConfidence::Low,
        _ => {
            uncertainty_reasons.push(UncertaintyReason::GenericProviderOnly);
            ResultConfidence::Low
        }
    };

    // --- Relevance ---
    let relevance = if meta.rank_reasons.contains(&RankReason::ExactTitleMatch) {
        quality_reasons.push(QualityReason::ExactSymbolMatch);
        RelevanceEstimate::Exact
    } else if has_exact_match || meta.rank_reasons.contains(&RankReason::RrfMultiProvider) {
        RelevanceEstimate::Strong
    } else if meta.rank_reasons.contains(&RankReason::IntentMatch) {
        RelevanceEstimate::Partial
    } else if has_snippet {
        RelevanceEstimate::Weak
    } else {
        RelevanceEstimate::Unknown
    };

    // --- Freshness ---
    let (freshness, freshness_uncertainty) =
        if let Some(ts) = freshness_timestamp_from_metadata(meta) {
            if let Some(dt) = parse_timestamp_str(ts) {
                let age = now.signed_duration_since(dt);
                let fresh = if age <= chrono::Duration::days(7) {
                    quality_reasons.push(QualityReason::FreshTimestamp);
                    FreshnessEstimate::Current
                } else if age <= chrono::Duration::days(30) {
                    quality_reasons.push(QualityReason::FreshTimestamp);
                    FreshnessEstimate::Recent
                } else if age <= chrono::Duration::days(180) {
                    FreshnessEstimate::Historical
                } else {
                    FreshnessEstimate::Stale
                };
                (fresh, false)
            } else {
                (FreshnessEstimate::Undated, true)
            }
        } else {
            (FreshnessEstimate::Undated, true)
        };
    if freshness_uncertainty {
        uncertainty_reasons.push(UncertaintyReason::NoTimestamp);
    }

    // Provider failure uncertainty
    if card.providers.is_empty() {
        uncertainty_reasons.push(UncertaintyReason::ProviderFailed);
    }

    // Deduplicate
    uncertainty_reasons.sort();
    uncertainty_reasons.dedup();
    quality_reasons.sort();
    quality_reasons.dedup();

    ResultQuality {
        confidence,
        relevance,
        authority,
        freshness,
        evidence_strength,
        uncertainty_reasons,
        quality_reasons,
    }
}

/// Compute a `GroupQualitySummary` for a set of results.
pub fn compute_group_quality(
    results: &[crate::core::source_card::SourceCard],
) -> GroupQualitySummary {
    let mut high = 0;
    let mut low = 0;
    let mut primary = 0;
    let mut exact = 0;

    for card in results {
        let q = compute_card_quality(card);
        match q.confidence {
            ResultConfidence::High => high += 1,
            ResultConfidence::Low | ResultConfidence::Unknown => low += 1,
            _ => {}
        }
        if matches!(
            q.authority,
            AuthorityEstimate::Primary
                | AuthorityEstimate::Official
                | AuthorityEstimate::Maintainer
        ) {
            primary += 1;
        }
        if matches!(
            q.evidence_strength,
            EvidenceStrength::ExactCodeSpan | EvidenceStrength::ExactIdentifier
        ) {
            exact += 1;
        }
    }

    let mut warnings = Vec::new();
    if low > 0 && low == results.len() {
        warnings.push("all results have low or unknown confidence".to_string());
    }
    if results.len() > 3 && exact == 0 {
        warnings.push("no exact evidence matches in group".to_string());
    }

    GroupQualitySummary {
        high_confidence_count: high,
        low_confidence_count: low,
        primary_source_count: primary,
        exact_evidence_count: exact,
        warnings,
    }
}

fn freshness_timestamp_from_metadata(
    meta: &crate::core::source_card::SourceMetadata,
) -> Option<&str> {
    if let Some(ref issue) = meta.issue {
        issue.updated_at.as_deref().or(issue.created_at.as_deref())
    } else if let Some(ref release) = meta.release {
        release
            .published_at
            .as_deref()
            .or(release.created_at.as_deref())
    } else if let Some(ref vuln) = meta.vulnerability {
        vuln.published_at.as_deref()
    } else {
        None
    }
}

fn parse_timestamp_str(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    if s.is_empty() {
        return None;
    }
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::result::TrustLevel;
    use crate::core::source_card::{SourceCard, SourceKind, SourceMetadata};

    fn make_card(source_kind: SourceKind, url: &str) -> SourceCard {
        let mut card = SourceCard::new(
            "Test",
            url,
            vec!["duckduckgo".to_string()],
            Some(0.05),
            TrustLevel::ExternalUntrusted,
        );
        card.metadata = SourceMetadata {
            source_kind,
            ..Default::default()
        };
        card
    }

    #[test]
    fn official_docs_high_confidence() {
        let card = make_card(SourceKind::OfficialDocs, "https://docs.rs/axum");
        let q = compute_card_quality(&card);
        assert_eq!(q.confidence, ResultConfidence::High);
        assert_eq!(q.authority, AuthorityEstimate::Official);
        assert!(q.quality_reasons.contains(&QualityReason::OfficialDocs));
    }

    #[test]
    fn package_registry_high_confidence() {
        let card = make_card(SourceKind::PackageRegistry, "https://crates.io/axum");
        let q = compute_card_quality(&card);
        assert_eq!(q.confidence, ResultConfidence::High);
        assert_eq!(q.authority, AuthorityEstimate::PackageRegistry);
    }

    #[test]
    fn security_advisory_high_confidence() {
        let card = make_card(
            SourceKind::SecurityAdvisory,
            "https://osv.dev/vulnerability/GHSA-xxxx",
        );
        let q = compute_card_quality(&card);
        assert_eq!(q.confidence, ResultConfidence::High);
        assert_eq!(q.authority, AuthorityEstimate::Primary);
    }

    #[test]
    fn generic_low_confidence_no_snippet() {
        let card = make_card(SourceKind::Unknown, "https://example.com/page");
        let q = compute_card_quality(&card);
        assert_eq!(q.confidence, ResultConfidence::Low);
        assert_eq!(q.evidence_strength, EvidenceStrength::UrlOnly);
        assert!(q
            .uncertainty_reasons
            .contains(&UncertaintyReason::NoSnippet));
    }

    #[test]
    fn generic_low_confidence_with_snippet() {
        let mut card = make_card(SourceKind::Unknown, "https://example.com/page");
        card.snippet = Some("Some text".to_string());
        let q = compute_card_quality(&card);
        assert_eq!(q.confidence, ResultConfidence::Low);
        assert_eq!(q.evidence_strength, EvidenceStrength::SnippetOnly);
    }

    #[test]
    fn native_provider_medium_confidence() {
        let mut card = make_card(
            SourceKind::IssueThread,
            "https://github.com/foo/bar/issues/1",
        );
        card.providers = vec!["github_issues".to_string()];
        card.metadata.issue = Some(crate::core::source_card::IssueMetadata {
            number: Some(1),
            ..Default::default()
        });
        let q = compute_card_quality(&card);
        assert_eq!(q.confidence, ResultConfidence::Medium);
        assert_eq!(q.authority, AuthorityEstimate::Maintainer);
    }

    #[test]
    fn news_source_low_authority() {
        let card = make_card(SourceKind::News, "https://techcrunch.com/article");
        let q = compute_card_quality(&card);
        assert_eq!(q.authority, AuthorityEstimate::NewsOrBlog);
    }

    #[test]
    fn freshness_undated_when_no_timestamp() {
        let card = make_card(SourceKind::Unknown, "https://example.com");
        let q = compute_card_quality(&card);
        assert_eq!(q.freshness, FreshnessEstimate::Undated);
        assert!(q
            .uncertainty_reasons
            .contains(&UncertaintyReason::NoTimestamp));
    }

    #[test]
    fn freshness_current_with_recent_timestamp() {
        let mut card = make_card(
            SourceKind::IssueThread,
            "https://github.com/foo/bar/issues/1",
        );
        card.metadata.issue = Some(crate::core::source_card::IssueMetadata {
            updated_at: Some(chrono::Utc::now().to_rfc3339()),
            ..Default::default()
        });
        let q = compute_card_quality(&card);
        assert_eq!(q.freshness, FreshnessEstimate::Current);
        assert!(q.quality_reasons.contains(&QualityReason::FreshTimestamp));
    }

    #[test]
    fn relevance_exact_from_title_match() {
        let mut card = make_card(SourceKind::OfficialDocs, "https://docs.rs/axum");
        card.metadata
            .rank_reasons
            .push(crate::core::source_card::RankReason::ExactTitleMatch);
        let q = compute_card_quality(&card);
        assert_eq!(q.relevance, RelevanceEstimate::Exact);
    }

    #[test]
    fn relevance_strong_from_rrf_multi_provider() {
        let mut card = make_card(SourceKind::Unknown, "https://example.com");
        card.metadata
            .rank_reasons
            .push(crate::core::source_card::RankReason::RrfMultiProvider);
        let q = compute_card_quality(&card);
        assert_eq!(q.relevance, RelevanceEstimate::Strong);
    }

    #[test]
    fn group_quality_summary_all_low() {
        let cards = vec![
            make_card(SourceKind::Unknown, "https://example.com/1"),
            make_card(SourceKind::Unknown, "https://example.com/2"),
        ];
        let summary = compute_group_quality(&cards);
        assert_eq!(summary.high_confidence_count, 0);
        assert_eq!(summary.low_confidence_count, 2);
        assert!(!summary.warnings.is_empty());
    }

    #[test]
    fn group_quality_summary_mixed() {
        let mut cards = vec![
            make_card(SourceKind::OfficialDocs, "https://docs.rs/axum"),
            make_card(SourceKind::Unknown, "https://example.com"),
        ];
        cards[1].snippet = Some("text".to_string());
        let summary = compute_group_quality(&cards);
        assert_eq!(summary.high_confidence_count, 1);
        assert_eq!(summary.low_confidence_count, 1);
    }

    #[test]
    fn result_quality_is_usable() {
        let q = ResultQuality {
            confidence: ResultConfidence::High,
            ..Default::default()
        };
        assert!(q.is_usable());

        let q = ResultQuality {
            confidence: ResultConfidence::Low,
            ..Default::default()
        };
        assert!(!q.is_usable());
    }

    #[test]
    fn serde_roundtrip() {
        let q = ResultQuality {
            confidence: ResultConfidence::Medium,
            relevance: RelevanceEstimate::Strong,
            authority: AuthorityEstimate::Maintainer,
            freshness: FreshnessEstimate::Current,
            evidence_strength: EvidenceStrength::StructuredMetadata,
            uncertainty_reasons: vec![UncertaintyReason::NoTimestamp],
            quality_reasons: vec![QualityReason::MaintainerSource],
        };
        let json = serde_json::to_string(&q).unwrap();
        let parsed: ResultQuality = serde_json::from_str(&json).unwrap();
        assert_eq!(q, parsed);
    }

    #[test]
    fn group_quality_summary_serde_roundtrip() {
        let s = GroupQualitySummary {
            high_confidence_count: 3,
            low_confidence_count: 1,
            primary_source_count: 2,
            exact_evidence_count: 1,
            warnings: vec!["test warning".to_string()],
        };
        let json = serde_json::to_string(&s).unwrap();
        let parsed: GroupQualitySummary = serde_json::from_str(&json).unwrap();
        assert_eq!(s, parsed);
    }

    // --- Quality metadata population across result paths ---

    #[test]
    fn code_result_with_raw_permalink_high_confidence() {
        use crate::core::code_evidence::{
            CodeEvidence, CodeEvidenceReason, EvidenceConfidence, SourceRole,
        };
        use crate::core::code_metadata::CodeHost;

        let mut card = make_card(
            SourceKind::SourceFile,
            "https://github.com/tokio-rs/axum/blob/main/src/lib.rs",
        );
        card.providers = vec!["github_code".to_string()];
        card.metadata.code_evidence = Some(CodeEvidence {
            host: Some(CodeHost::Github),
            owner: Some("tokio-rs".to_string()),
            repo: Some("axum".to_string()),
            ref_name: Some("main".to_string()),
            path: Some("src/lib.rs".to_string()),
            language: Some("rust".to_string()),
            source_role: Some(SourceRole::Implementation),
            browser_url: Some("https://github.com/tokio-rs/axum/blob/main/src/lib.rs".to_string()),
            raw_url: Some(
                "https://raw.githubusercontent.com/tokio-rs/axum/main/src/lib.rs".to_string(),
            ),
            permalink_url: Some(
                "https://github.com/tokio-rs/axum/blob/main/src/lib.rs".to_string(),
            ),
            raw_permalink_url: Some(
                "https://raw.githubusercontent.com/tokio-rs/axum/main/src/lib.rs".to_string(),
            ),
            evidence_confidence: Some(EvidenceConfidence::Strong),
            evidence_reasons: vec![
                CodeEvidenceReason::RawUrlDerived,
                CodeEvidenceReason::LanguageMatch,
            ],
            ..Default::default()
        });
        card.metadata.code = Some(crate::core::code_metadata::CodeMetadata {
            host: Some(CodeHost::Github),
            owner: Some("tokio-rs".to_string()),
            repo: Some("axum".to_string()),
            ref_name: Some("main".to_string()),
            path: Some("src/lib.rs".to_string()),
            language: Some("rust".to_string()),
            ..Default::default()
        });

        let q = compute_card_quality(&card);
        assert_eq!(
            q.confidence,
            ResultConfidence::Medium,
            "source_file with structured evidence but no exact match gets Medium"
        );
        assert_eq!(q.evidence_strength, EvidenceStrength::ExactCodeSpan);
        assert!(q
            .quality_reasons
            .contains(&QualityReason::CommitPinnedEvidence));
        assert!(q
            .quality_reasons
            .contains(&QualityReason::StructuredCodeEvidence));
        assert_eq!(q.authority, AuthorityEstimate::Maintainer);
        assert!(q.quality_reasons.contains(&QualityReason::MaintainerSource));
        assert!(q.quality_reasons.contains(&QualityReason::ExactPathMatch));
    }

    #[test]
    fn code_result_with_raw_url_only_no_permalink() {
        use crate::core::code_evidence::{
            CodeEvidence, CodeEvidenceReason, EvidenceConfidence, SourceRole,
        };
        use crate::core::code_metadata::CodeHost;

        let mut card = make_card(
            SourceKind::SourceFile,
            "https://github.com/tokio-rs/axum/blob/main/src/lib.rs",
        );
        card.providers = vec!["github_code".to_string()];
        card.metadata.code_evidence = Some(CodeEvidence {
            host: Some(CodeHost::Github),
            owner: Some("tokio-rs".to_string()),
            repo: Some("axum".to_string()),
            path: Some("src/lib.rs".to_string()),
            language: Some("rust".to_string()),
            source_role: Some(SourceRole::Implementation),
            raw_url: Some(
                "https://raw.githubusercontent.com/tokio-rs/axum/main/src/lib.rs".to_string(),
            ),
            evidence_confidence: Some(EvidenceConfidence::Strong),
            evidence_reasons: vec![CodeEvidenceReason::RawUrlDerived],
            // No permalink_url, no raw_permalink_url
            ..Default::default()
        });

        let q = compute_card_quality(&card);
        assert_eq!(q.evidence_strength, EvidenceStrength::ExactCodeSpan);
        assert!(
            !q.quality_reasons
                .contains(&QualityReason::CommitPinnedEvidence),
            "no raw_permalink_url means no CommitPinnedEvidence"
        );
        assert!(q
            .quality_reasons
            .contains(&QualityReason::StructuredCodeEvidence));
    }

    #[test]
    fn generic_snippet_only_not_high_confidence() {
        let mut card = make_card(SourceKind::Unknown, "https://example.com/page");
        card.snippet = Some("A generic snippet about something".to_string());
        let q = compute_card_quality(&card);
        assert_ne!(
            q.confidence,
            ResultConfidence::High,
            "generic snippet-only result must not be High confidence"
        );
        assert_eq!(q.evidence_strength, EvidenceStrength::SnippetOnly);
        assert_eq!(q.authority, AuthorityEstimate::Unknown);
        assert!(q
            .uncertainty_reasons
            .contains(&UncertaintyReason::LowAuthoritySource));
    }

    #[test]
    fn official_docs_has_official_authority() {
        let card = make_card(SourceKind::OfficialDocs, "https://doc.rust-lang.org/book");
        let q = compute_card_quality(&card);
        assert_eq!(q.authority, AuthorityEstimate::Official);
        assert!(q.quality_reasons.contains(&QualityReason::OfficialDocs));
        assert_eq!(q.confidence, ResultConfidence::High);
    }

    #[test]
    fn package_registry_has_package_registry_authority() {
        let card = make_card(SourceKind::PackageRegistry, "https://crates.io/axum");
        let q = compute_card_quality(&card);
        assert_eq!(q.authority, AuthorityEstimate::PackageRegistry);
        assert!(q
            .quality_reasons
            .contains(&QualityReason::PackageRegistryMetadata));
        assert_eq!(q.confidence, ResultConfidence::High);
    }

    #[test]
    fn security_advisory_has_primary_authority() {
        let card = make_card(
            SourceKind::SecurityAdvisory,
            "https://nvd.nist.gov/vuln/detail/CVE-2024-1234",
        );
        let q = compute_card_quality(&card);
        assert_eq!(q.authority, AuthorityEstimate::Primary);
        assert!(q.quality_reasons.contains(&QualityReason::PrimaryAdvisory));
        assert_eq!(q.confidence, ResultConfidence::High);
    }

    #[test]
    fn local_workspace_source_card_gets_quality() {
        use crate::core::code_evidence::{
            CodeEvidence, CodeEvidenceReason, EvidenceConfidence, SourceRole,
        };
        use crate::core::code_metadata::CodeHost;

        let mut card = SourceCard::new(
            "lib.rs — workspace",
            "workspace://eggsearch/src/lib.rs",
            vec!["local_workspace".to_string()],
            Some(0.8),
            TrustLevel::LocalTrusted,
        );
        card.metadata = SourceMetadata {
            source_kind: SourceKind::SourceFile,
            domain: None,
            rank_reasons: vec![],
            code: Some(crate::core::code_metadata::CodeMetadata {
                host: Some(CodeHost::Unknown),
                owner: Some("eggsearch".to_string()),
                repo: Some("src/lib.rs".to_string()),
                path: Some("src/lib.rs".to_string()),
                language: Some("rust".to_string()),
                ..Default::default()
            }),
            code_evidence: Some(CodeEvidence {
                host: Some(CodeHost::Unknown),
                owner: Some("eggsearch".to_string()),
                repo: Some("src/lib.rs".to_string()),
                path: Some("src/lib.rs".to_string()),
                language: Some("rust".to_string()),
                source_role: Some(SourceRole::Implementation),
                raw_url: Some("workspace://eggsearch/src/lib.rs".to_string()),
                evidence_confidence: Some(EvidenceConfidence::Strong),
                evidence_reasons: vec![CodeEvidenceReason::LanguageMatch],
                ..Default::default()
            }),
            ..Default::default()
        };

        let q = compute_card_quality(&card);
        assert_ne!(
            q.confidence,
            ResultConfidence::Unknown,
            "local workspace card must have non-Unknown confidence"
        );
        assert_eq!(q.evidence_strength, EvidenceStrength::ExactCodeSpan);
        assert!(q
            .quality_reasons
            .contains(&QualityReason::StructuredCodeEvidence));
        assert!(q.quality_reasons.contains(&QualityReason::ExactPathMatch));
    }

    #[test]
    fn local_workspace_with_code_metadata_maintainer_authority() {
        let mut card = SourceCard::new(
            "lib.rs — workspace",
            "workspace://eggsearch/src/lib.rs",
            vec!["local_workspace".to_string()],
            Some(0.8),
            TrustLevel::LocalTrusted,
        );
        card.metadata = SourceMetadata {
            source_kind: SourceKind::SourceFile,
            code: Some(crate::core::code_metadata::CodeMetadata {
                owner: Some("eggsearch".to_string()),
                repo: Some("myrepo".to_string()),
                path: Some("src/lib.rs".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let q = compute_card_quality(&card);
        // local_workspace doesn't contain github/gitlab/gitea, so authority is Unknown
        // but code evidence and path match still work
        assert_eq!(q.evidence_strength, EvidenceStrength::UrlOnly);
        assert!(q.quality_reasons.contains(&QualityReason::ExactPathMatch));
    }

    #[test]
    fn research_group_all_cards_have_quality() {
        let cards = vec![
            {
                let mut c = make_card(SourceKind::OfficialDocs, "https://docs.rs/axum");
                c.snippet = Some("Axum docs".to_string());
                c
            },
            {
                let mut c = make_card(SourceKind::Tutorial, "https://tokio.rs/blog");
                c.snippet = Some("Tutorial content".to_string());
                c
            },
            {
                let mut c = make_card(
                    SourceKind::IssueThread,
                    "https://github.com/foo/bar/issues/1",
                );
                c.providers = vec!["github_issues".to_string()];
                c.metadata.issue = Some(crate::core::source_card::IssueMetadata {
                    number: Some(1),
                    ..Default::default()
                });
                c
            },
            {
                let mut c = make_card(SourceKind::Unknown, "https://example.com/guide");
                c.snippet = Some("Some guide".to_string());
                c
            },
        ];

        // Verify compute_card_quality returns a quality value for every card
        for card in &cards {
            let q = compute_card_quality(card);
            assert_ne!(
                q.confidence,
                ResultConfidence::Unknown,
                "card {} should have non-Unknown confidence",
                card.url
            );
        }

        // Verify group quality summary is correctly computed
        let summary = compute_group_quality(&cards);
        assert_eq!(summary.high_confidence_count, 1);
        // Tutorial (no native provider, no structured) = Low; Unknown snippet = Low
        assert_eq!(summary.low_confidence_count, 2);
        assert!(summary.primary_source_count >= 1);
    }

    #[test]
    fn group_quality_summary_uncertainty_counts_low_confidence() {
        let cards = vec![
            make_card(SourceKind::Unknown, "https://example.com/1"),
            make_card(SourceKind::Unknown, "https://example.com/2"),
            make_card(SourceKind::Unknown, "https://example.com/3"),
        ];
        let summary = compute_group_quality(&cards);
        assert_eq!(summary.low_confidence_count, 3);
        assert_eq!(summary.high_confidence_count, 0);
        assert!(summary.warnings.iter().any(|w| w.contains("all results")));
    }

    #[test]
    fn group_quality_summary_high_not_counted_as_low() {
        let cards = vec![
            make_card(SourceKind::OfficialDocs, "https://docs.rs/foo"),
            make_card(SourceKind::PackageRegistry, "https://crates.io/foo"),
            make_card(SourceKind::SecurityAdvisory, "https://osv.dev/vuln/CVE-1"),
        ];
        let summary = compute_group_quality(&cards);
        assert_eq!(summary.high_confidence_count, 3);
        assert_eq!(summary.low_confidence_count, 0);
        assert!(summary.warnings.is_empty());
    }

    #[test]
    fn group_quality_summary_exact_evidence_counts_code_with_permalink() {
        use crate::core::code_evidence::CodeEvidence;
        use crate::core::code_metadata::CodeHost;

        let mut card = make_card(
            SourceKind::SourceFile,
            "https://github.com/foo/bar/blob/main/src/main.rs",
        );
        card.metadata.code_evidence = Some(CodeEvidence {
            host: Some(CodeHost::Github),
            raw_permalink_url: Some(
                "https://raw.githubusercontent.com/foo/bar/abc123/src/main.rs".to_string(),
            ),
            ..Default::default()
        });

        let cards = vec![card, make_card(SourceKind::Unknown, "https://example.com")];
        let summary = compute_group_quality(&cards);
        assert_eq!(summary.exact_evidence_count, 1);
    }

    #[test]
    fn native_release_with_provider_maintainer_authority() {
        let mut card = make_card(
            SourceKind::ReleaseNotes,
            "https://github.com/tokio-rs/axum/releases/tag/v0.7.0",
        );
        card.providers = vec!["github_releases".to_string()];
        card.metadata.release = Some(crate::core::source_card::ReleaseMetadata {
            tag: Some("v0.7.0".to_string()),
            published_at: Some(chrono::Utc::now().to_rfc3339()),
            ..Default::default()
        });
        let q = compute_card_quality(&card);
        assert_eq!(q.authority, AuthorityEstimate::Maintainer);
        assert!(q.quality_reasons.contains(&QualityReason::MaintainerSource));
    }

    #[test]
    fn non_native_issue_community_authority() {
        let mut card = make_card(
            SourceKind::IssueThread,
            "https://stackoverflow.com/questions/12345",
        );
        card.snippet = Some("How do I...".to_string());
        // No github/gitlab/gitea in providers
        let q = compute_card_quality(&card);
        assert_eq!(q.authority, AuthorityEstimate::Community);
    }

    #[test]
    fn advisory_identifier_match_boosts_evidence_strength() {
        let mut card = make_card(SourceKind::SecurityAdvisory, "https://example.com/advisory");
        card.metadata
            .rank_reasons
            .push(crate::core::source_card::RankReason::AdvisoryIdentifierMatch);
        let q = compute_card_quality(&card);
        // SecurityAdvisory already has Primary authority + High confidence
        // AdvisoryIdentifierMatch should push evidence to ExactIdentifier
        assert!(matches!(
            q.evidence_strength,
            EvidenceStrength::ExactIdentifier | EvidenceStrength::ExactCodeSpan
        ));
    }

    #[test]
    fn empty_providers_triggers_provider_failed_uncertainty() {
        let mut card = make_card(SourceKind::Unknown, "https://example.com");
        card.providers = vec![];
        let q = compute_card_quality(&card);
        assert!(q
            .uncertainty_reasons
            .contains(&UncertaintyReason::ProviderFailed));
    }
}
