//! Compact `SourceCard` representation passed to agents.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::result::TrustLevel;
use crate::core::sanitize::TrustMarkers;

/// Deterministic classification of a result's source type, derived
/// from URL/domain heuristics. Helps smaller models choose which
/// result to fetch first.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    /// Unrecognized or non-classifiable source.
    #[default]
    Unknown,
    /// Official language/library documentation (e.g. docs.rs, MDN).
    OfficialDocs,
    /// Package registry listing (e.g. crates.io, npm, PyPI).
    PackageRegistry,
    /// Source code repository (broad fallback for code-host URLs).
    SourceRepository,
    /// Repository root page (e.g. `github.com/owner/repo`).
    RepositoryRoot,
    /// Directory or tree view within a repository.
    SourceDirectory,
    /// Individual source file (e.g. blob URL).
    SourceFile,
    /// Issue, discussion, or pull request thread.
    IssueThread,
    /// Pull request (distinct from issues when distinguishable).
    PullRequest,
    /// Release notes or changelog entry.
    ReleaseNotes,
    /// Tag page (distinct from release pages when distinguishable).
    Tag,
    /// Commit view.
    Commit,
    /// Security advisory or vulnerability database entry.
    SecurityAdvisory,
    /// API reference or specification page.
    Reference,
    /// News article or press coverage.
    News,
    /// Tutorial, guide, or educational content.
    Tutorial,
    /// Community forum or discussion board.
    Forum,
}

/// Deterministic rank-reason tag explaining why a result received
/// its score. Always a short enum-like string, never generated prose.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RankReason {
    /// Appeared in results from multiple providers (RRF boost).
    RrfMultiProvider,
    /// Ranked highly by a single provider's native ordering.
    RrfProviderRank,
    /// Domain is a known official-docs source.
    DomainPriorDocs,
    /// Domain is a known source-code hosting platform.
    DomainPriorCode,
    /// Domain is a known security-advisory source.
    DomainPriorSecurity,
    /// Domain is a known release-notes source.
    DomainPriorRelease,
    /// Query intent matched the page's topic or title.
    IntentMatch,
    /// Page was recently published or updated.
    FreshnessMatch,
    /// Page title matched the query exactly.
    ExactTitleMatch,
    /// Canonical URL deduplicated with another result.
    CanonicalDedup,
    /// Result came from a native GitHub issues provider.
    ProviderNativeIssueSearch,
    /// Result came from a native GitHub releases provider.
    ProviderNativeReleaseSearch,
    /// Result came from a native advisory provider (e.g. OSV).
    ProviderNativeAdvisorySearch,
    /// URL matches the requested owner/repo.
    RepoOwnerMatch,
    /// Path/file/language/symbol hint matched the result.
    HintMatch,
    /// Security identifier (CVE/GHSA/RustSec) detected in URL, title, or snippet.
    AdvisoryIdentifierMatch,
    /// Result is a CISA Known Exploited Vulnerabilities entry.
    KevMatch,
    /// Result is a vendor/project security advisory.
    VendorAdvisory,
    /// Result is a package-ecosystem security advisory.
    PackageAdvisory,
    /// Result is defensive guidance or hardening documentation.
    DefensiveGuidance,
}

/// Structured issue metadata from native GitHub issues providers.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[allow(missing_docs)]
pub struct IssueMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<crate::core::code_metadata::CodeHost>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_pull_request: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<String>,
}

impl IssueMetadata {
    /// Field-wise merge used by RRF deduplication. `self` wins for
    /// every present field; `other` is only consulted for fields that
    /// are `None` on `self`. `Vec` fields concatenate, deduplicated,
    /// preserving `self`'s elements first.
    pub fn merge(self, other: IssueMetadata) -> IssueMetadata {
        let mut labels = self.labels;
        for label in other.labels {
            if !labels.contains(&label) {
                labels.push(label);
            }
        }
        IssueMetadata {
            host: self.host.or(other.host),
            owner: self.owner.or(other.owner),
            repo: self.repo.or(other.repo),
            number: self.number.or(other.number),
            state: self.state.or(other.state),
            is_pull_request: self.is_pull_request.or(other.is_pull_request),
            labels,
            created_at: self.created_at.or(other.created_at),
            updated_at: self.updated_at.or(other.updated_at),
            closed_at: self.closed_at.or(other.closed_at),
        }
    }
}

/// Structured release metadata from native GitHub releases providers.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[allow(missing_docs)]
pub struct ReleaseMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<crate::core::code_metadata::CodeHost>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prerelease: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
}

impl ReleaseMetadata {
    /// Field-wise merge used by RRF deduplication. `self` wins for
    /// every present field; `other` is only consulted for fields that
    /// are `None` on `self`.
    pub fn merge(self, other: ReleaseMetadata) -> ReleaseMetadata {
        ReleaseMetadata {
            host: self.host.or(other.host),
            owner: self.owner.or(other.owner),
            repo: self.repo.or(other.repo),
            tag: self.tag.or(other.tag),
            name: self.name.or(other.name),
            draft: self.draft.or(other.draft),
            prerelease: self.prerelease.or(other.prerelease),
            created_at: self.created_at.or(other.created_at),
            published_at: self.published_at.or(other.published_at),
        }
    }
}

/// Deterministic metadata attached to each `SourceCard` to help
/// agents choose which result to inspect first. All fields are
/// computed from URL/domain heuristics — no generated prose.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SourceMetadata {
    /// Deterministic source-type classification.
    #[serde(default)]
    pub source_kind: SourceKind,
    /// Extracted domain (e.g. `"docs.rs"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    /// Deterministic reasons this result scored where it did.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rank_reasons: Vec<RankReason>,
    /// Structured code/repo metadata, present only for code-host URLs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<crate::core::code_metadata::CodeMetadata>,
    /// Structured issue metadata, present for native issue provider results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<IssueMetadata>,
    /// Structured release metadata, present for native release provider results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<ReleaseMetadata>,
    /// Structured vulnerability metadata, present for native advisory provider results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vulnerability: Option<Box<crate::core::security::VulnerabilityMetadata>>,
    /// Structured code evidence with derived URLs, source role, and match metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_evidence: Option<crate::core::code_evidence::CodeEvidence>,
}

/// A single normalized result returned to MCP callers.
///
/// This is the canonical, provider-agnostic output model. It is deliberately
/// small: agents should fetch full content via a separate `web_fetch` tool
/// rather than rely on snippets.
///
/// `web_search` is discovery-only and returns `SourceCard` values with
/// `fetched = false`. `web_fetch` returns a separate fetched-document
/// response for one explicit URL.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SourceCard {
    /// Per-response identifier, e.g. `src_<uuid>`. Unique within a
    /// single `web_search` response.
    pub id: String,
    /// Result title.
    pub title: String,
    /// Canonical URL.
    pub url: String,
    /// Short text snippet (truncated, never full content).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    /// All upstream engines that contributed to this card.
    #[serde(default)]
    pub providers: Vec<String>,
    /// Optional aggregate score (e.g. RRF). Higher is more relevant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    /// Trust label; for live web results this is `external_untrusted`.
    pub trust: TrustLevel,
    /// Whether the underlying content was fetched. `web_search` is
    /// discovery-only and always returns cards with `fetched = false`;
    /// full-page retrieval is handled by the separate `web_fetch`
    /// tool, which returns its own response type rather than a
    /// `SourceCard`.
    pub fetched: bool,
    /// What eggsearch did to the title/snippet text on this card
    /// (control-char stripping, length bounding, framing, marker
    /// scanning). Default-initialized to a zero record on cards that
    /// have not yet been sanitized; later pipeline stages replace it
    /// with the actual counts.
    #[serde(default)]
    pub trust_markers: TrustMarkers,
    /// Deterministic metadata helping agents choose which result to
    /// inspect first. Populated by the adapter after aggregation.
    #[serde(default, skip_serializing_if = "is_default_metadata")]
    pub metadata: SourceMetadata,
}

fn is_default_metadata(m: &SourceMetadata) -> bool {
    m.source_kind == SourceKind::Unknown
        && m.domain.is_none()
        && m.rank_reasons.is_empty()
        && m.code.is_none()
        && m.issue.is_none()
        && m.release.is_none()
        && m.vulnerability.is_none()
        && m.code_evidence.is_none()
}

impl SourceCard {
    /// Build a fresh `SourceCard` with the given title, url, providers, score,
    /// and trust label. A unique id of the form `src_<uuid>` is generated.
    ///
    /// # Examples
    ///
    /// ```
    /// use eggsearch::core::{SourceCard, TrustLevel};
    ///
    /// let card = SourceCard::new(
    ///     "tower-http - Rust",
    ///     "https://docs.rs/tower-http",
    ///     vec!["duckduckgo".to_string(), "brave".to_string()],
    ///     Some(0.0327),
    ///     TrustLevel::ExternalUntrusted,
    /// )
    /// .with_snippet("Middleware and utilities for HTTP clients and servers.");
    ///
    /// assert_eq!(card.title, "tower-http - Rust");
    /// assert!(card.id.starts_with("src_"));
    /// assert!(!card.fetched);
    /// assert!(card.snippet.is_some());
    /// ```
    pub fn new(
        title: impl Into<String>,
        url: impl Into<String>,
        providers: Vec<String>,
        score: Option<f64>,
        trust: TrustLevel,
    ) -> Self {
        Self {
            id: format!("src_{}", Uuid::new_v4().simple()),
            title: title.into(),
            url: url.into(),
            snippet: None,
            providers,
            score,
            trust,
            fetched: false,
            trust_markers: TrustMarkers::default(),
            metadata: SourceMetadata::default(),
        }
    }

    /// Attach a snippet to this card. Convenience for the
    /// `SourceCard::new(...).with_snippet(...)` builder pattern.
    pub fn with_snippet(mut self, s: impl Into<String>) -> Self {
        self.snippet = Some(s.into());
        self
    }

    /// Attach `TrustMarkers` describing what eggsearch did to the
    /// title/snippet text on this card. The pipeline populates this
    /// after sanitization; the constructor leaves it at
    /// `TrustMarkers::default()`.
    pub fn with_trust_markers(mut self, m: TrustMarkers) -> Self {
        self.trust_markers = m;
        self
    }

    /// Attach deterministic source metadata to this card.
    pub fn with_metadata(mut self, m: SourceMetadata) -> Self {
        self.metadata = m;
        self
    }
}

/// Classify a URL into a deterministic `SourceKind` using domain
/// and path heuristics. Returns `Unknown` when the URL does not
/// match any known pattern.
///
/// For code-host URLs (GitHub, GitLab, Codeberg), this delegates to
/// the more precise `code_metadata::classify_and_extract` for
/// narrower classification. For non-code-host URLs, domain heuristics
/// are used directly.
pub fn classify_source_kind(url: &str) -> SourceKind {
    use url::Url;

    let parsed = match Url::parse(url) {
        Ok(u) => u,
        Err(_) => return SourceKind::Unknown,
    };
    let host = parsed.host_str().unwrap_or("");
    let path = parsed.path();

    // Code-host URLs: delegate to the precise classifier.
    if matches!(host, "github.com" | "gitlab.com" | "codeberg.org") {
        let (kind, _, _) = crate::core::code_metadata::classify_and_extract(url);
        return kind;
    }

    // Official docs domains
    if host == "docs.rs"
        || host.ends_with(".readthedocs.io")
        || host.ends_with(".readthedocs.org")
        || host == "doc.rust-lang.org"
        || host == "doc.python.org"
        || host == "docs.python.org"
        || host == "developer.mozilla.org"
        || host == "go.dev"
        || host == "pkg.go.dev"
        || host == "doc.npmjs.com"
    {
        return SourceKind::OfficialDocs;
    }

    // Package registries
    if host == "crates.io"
        || host == "npmjs.com"
        || host == "www.npmjs.com"
        || host == "pypi.org"
        || host == "rubygems.org"
        || host == "pkg.go.dev"
    {
        return SourceKind::PackageRegistry;
    }

    // Security advisories
    if host == "osv.dev"
        || host == "nvd.nist.gov"
        || host == "security.snyk.io"
        || host == "cve.mitre.org"
    {
        return SourceKind::SecurityAdvisory;
    }

    // Release notes (CHANGELOG files)
    if path.contains("CHANGELOG") || path.contains("changelog") || path.contains("CHANGES") {
        return SourceKind::ReleaseNotes;
    }

    // Tutorials (heuristic: common tutorial sites)
    if host.contains("tutorial")
        || host.contains("learn")
        || host == "dev.to"
        || host == "medium.com"
        || host == "stackoverflow.com"
        || host == "stackexchange.com"
    {
        return SourceKind::Tutorial;
    }

    // Forums
    if host.contains("forum") || host == "discourse.org" || host.ends_with(".discourse.app") {
        return SourceKind::Forum;
    }

    // News
    if host.contains("news")
        || host.contains("blog")
        || host == "arstechnica.com"
        || host == "theverge.com"
        || host == "techcrunch.com"
    {
        return SourceKind::News;
    }

    SourceKind::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_card_defaults() {
        let c = SourceCard::new(
            "hello",
            "https://example.com",
            vec!["duckduckgo".to_string()],
            Some(0.5),
            TrustLevel::ExternalUntrusted,
        );
        assert_eq!(c.title, "hello");
        assert_eq!(c.url, "https://example.com");
        assert_eq!(c.providers, vec!["duckduckgo".to_string()]);
        assert_eq!(c.score, Some(0.5));
        assert!(!c.fetched);
        assert!(c.snippet.is_none());
    }

    #[test]
    fn with_snippet_sets_field() {
        let c = SourceCard::new(
            "t",
            "https://example.com",
            vec!["a".to_string()],
            None,
            TrustLevel::ExternalUntrusted,
        )
        .with_snippet("a snippet");
        assert_eq!(c.snippet.as_deref(), Some("a snippet"));
    }

    #[test]
    fn id_starts_with_src_prefix() {
        let c = SourceCard::new(
            "t",
            "https://example.com",
            vec!["a".to_string()],
            None,
            TrustLevel::ExternalUntrusted,
        );
        assert!(c.id.starts_with("src_"));
    }

    #[test]
    fn serde_roundtrip() {
        let c = SourceCard::new(
            "Example",
            "https://example.com",
            vec!["duckduckgo".to_string(), "brave".to_string()],
            Some(0.016),
            TrustLevel::ExternalUntrusted,
        )
        .with_snippet("An example snippet.");
        let json = serde_json::to_string(&c).unwrap();
        let parsed: SourceCard = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.title, c.title);
        assert_eq!(parsed.url, c.url);
        assert_eq!(parsed.providers, c.providers);
        assert_eq!(parsed.score, c.score);
        assert_eq!(parsed.trust, c.trust);
        assert_eq!(parsed.snippet, c.snippet);
        assert_eq!(parsed.metadata, c.metadata);
    }

    #[test]
    fn serde_skips_none_optional_fields() {
        let c = SourceCard::new(
            "Example",
            "https://example.com",
            vec!["duckduckgo".to_string()],
            None,
            TrustLevel::ExternalUntrusted,
        );
        let json = serde_json::to_string(&c).unwrap();
        assert!(!json.contains("\"snippet\":null"));
        assert!(!json.contains("\"score\":null"));
        let parsed: SourceCard = serde_json::from_str(&json).unwrap();
        assert!(parsed.snippet.is_none());
        assert!(parsed.score.is_none());
    }

    #[test]
    fn new_card_default_trust_markers_is_zero() {
        let c = SourceCard::new(
            "t",
            "https://example.com",
            vec!["a".to_string()],
            None,
            TrustLevel::ExternalUntrusted,
        );
        assert_eq!(c.trust_markers, TrustMarkers::default());
        assert!(!c.trust_markers.text_sanitized);
        assert_eq!(c.trust_markers.injection_hits, 0);
    }

    #[test]
    fn with_trust_markers_sets_field() {
        let markers = TrustMarkers {
            text_sanitized: true,
            text_truncated: true,
            text_framed: false,
            control_chars_removed: 2,
            injection_hits: 1,
        };
        let c = SourceCard::new(
            "t",
            "https://example.com",
            vec!["a".to_string()],
            None,
            TrustLevel::ExternalUntrusted,
        )
        .with_trust_markers(markers.clone());
        assert_eq!(c.trust_markers, markers);
    }

    #[test]
    fn source_kind_default_is_unknown() {
        assert_eq!(SourceKind::default(), SourceKind::Unknown);
    }

    #[test]
    fn source_metadata_default_is_empty() {
        let m = SourceMetadata::default();
        assert_eq!(m.source_kind, SourceKind::Unknown);
        assert!(m.domain.is_none());
        assert!(m.rank_reasons.is_empty());
        assert!(m.code.is_none());
        assert!(m.issue.is_none());
        assert!(m.release.is_none());
        assert!(m.vulnerability.is_none());
        assert!(m.code_evidence.is_none());
    }

    #[test]
    fn classify_source_kind_docs_rs() {
        assert_eq!(
            classify_source_kind("https://docs.rs/tower-http/latest/tower_http/"),
            SourceKind::OfficialDocs
        );
    }

    #[test]
    fn classify_source_kind_github_issues() {
        assert_eq!(
            classify_source_kind("https://github.com/tokio-rs/axum/issues/123"),
            SourceKind::IssueThread
        );
    }

    #[test]
    fn classify_source_kind_github_releases() {
        assert_eq!(
            classify_source_kind("https://github.com/tokio-rs/axum/releases/tag/v0.7.0"),
            SourceKind::ReleaseNotes
        );
    }

    #[test]
    fn classify_source_kind_osv() {
        assert_eq!(
            classify_source_kind("https://osv.dev/vulnerability/GHSA-xxxx"),
            SourceKind::SecurityAdvisory
        );
    }

    #[test]
    fn classify_source_kind_unknown_for_random_url() {
        assert_eq!(
            classify_source_kind("https://example.com/some/page"),
            SourceKind::Unknown
        );
    }

    #[test]
    fn source_metadata_with_code_evidence_roundtrip() {
        use crate::core::code_evidence::{
            CodeEvidence, CodeEvidenceReason, EvidenceConfidence, SourceRole,
        };

        let evidence = CodeEvidence {
            host: Some(crate::core::code_metadata::CodeHost::Github),
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
                "https://raw.githubusercontent.com/tokio-rs/axum/main/src/lib.rs".to_string(),
            ),
            evidence_confidence: Some(EvidenceConfidence::Strong),
            evidence_reasons: vec![CodeEvidenceReason::RawUrlDerived, CodeEvidenceReason::LanguageMatch],
            ..Default::default()
        };

        let meta = SourceMetadata {
            source_kind: SourceKind::SourceFile,
            domain: Some("github.com".to_string()),
            code_evidence: Some(evidence.clone()),
            ..Default::default()
        };

        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains("code_evidence"));
        let parsed: SourceMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.code_evidence, Some(evidence));
        assert_eq!(parsed.source_kind, SourceKind::SourceFile);
        assert_eq!(parsed.domain.as_deref(), Some("github.com"));
    }
}
