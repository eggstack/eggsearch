//! Deterministic ranking pipeline for suggested fetch candidates.
//!
//! This module provides a scoring model that ranks fetch candidates by
//! provenance stability, evidence confidence, source role, query
//! context matching, and mode-aware signals. The ranking is fully
//! deterministic — no network access, no ML, no randomness.

use crate::core::code_evidence::{EvidenceConfidence, SourceRole};
use crate::core::fetch::ExtractMode;
use crate::core::repo_query::RepoQueryHints;
use crate::core::source_card::SourceKind;

/// Stable rank-reason identifiers. Serialized as snake_case strings.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FetchRankReason {
    /// Commit-pinned raw permalink (most stable).
    PinnedRawPermalink,
    /// Commit-pinned browser permalink.
    PinnedBrowserPermalink,
    /// Mutable raw content URL.
    MutableRawUrl,
    /// Mutable browser source URL.
    MutableBrowserUrl,
    /// Generic web page URL.
    GenericWebUrl,
    /// Code evidence present but sparse/ambiguous.
    SparseCodeEvidence,
    /// Evidence confidence is exact.
    ExactConfidence,
    /// Evidence confidence is strong.
    StrongConfidence,
    /// Evidence confidence is weak.
    WeakConfidence,
    /// Evidence confidence is unknown.
    UnknownConfidence,
    /// Source role is implementation code.
    SourceRoleImplementation,
    /// Source role is documentation.
    SourceRoleDocumentation,
    /// Source role is README.
    SourceRoleReadme,
    /// Source role is example code.
    SourceRoleExample,
    /// Source role is test code.
    SourceRoleTest,
    /// Source role is changelog.
    SourceRoleChangelog,
    /// Source role is migration guide.
    SourceRoleMigration,
    /// Source role is benchmark.
    SourceRoleBenchmark,
    /// Source role is configuration.
    SourceRoleConfiguration,
    /// Source kind is official docs.
    KindOfficialDocs,
    /// Source kind is package registry.
    KindPackageRegistry,
    /// Source kind is release notes.
    KindReleaseNotes,
    /// Source kind is issue thread.
    KindIssueThread,
    /// Source kind is pull request.
    KindPullRequest,
    /// Source kind is security advisory.
    KindSecurityAdvisory,
    /// Source kind is source file.
    KindSourceFile,
    /// Symbol hint matches candidate.
    SymbolHintMatch,
    /// Path hint matches candidate.
    PathHintMatch,
    /// Language hint matches candidate.
    LanguageHintMatch,
    /// File hint matches candidate.
    FileHintMatch,
    /// Error context present (exact-error mode).
    ErrorContextMatch,
    /// Version or migration context present.
    VersionMigrationContext,
    /// Package name matches candidate.
    PackageNameMatch,
    /// Requested source type matches.
    SourceTypeMatch,
    /// Authoritative advisory source (OSV, NVD, GHSA, RustSec).
    AuthoritativeAdvisory,
    /// Vendor-published advisory or patch.
    VendorAdvisory,
    /// Primary research source (specs, official docs).
    PrimaryResearchSource,
    /// Reference implementation.
    ReferenceImplementation,
    /// Benchmark or measurement.
    BenchmarkSource,
    /// Security consideration source.
    SecurityConsideration,
}

impl FetchRankReason {
    /// Return the stable snake_case string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PinnedRawPermalink => "pinned_raw_permalink",
            Self::PinnedBrowserPermalink => "pinned_browser_permalink",
            Self::MutableRawUrl => "mutable_raw_url",
            Self::MutableBrowserUrl => "mutable_browser_url",
            Self::GenericWebUrl => "generic_web_url",
            Self::SparseCodeEvidence => "sparse_code_evidence",
            Self::ExactConfidence => "exact_confidence",
            Self::StrongConfidence => "strong_confidence",
            Self::WeakConfidence => "weak_confidence",
            Self::UnknownConfidence => "unknown_confidence",
            Self::SourceRoleImplementation => "source_role_implementation",
            Self::SourceRoleDocumentation => "source_role_documentation",
            Self::SourceRoleReadme => "source_role_readme",
            Self::SourceRoleExample => "source_role_example",
            Self::SourceRoleTest => "source_role_test",
            Self::SourceRoleChangelog => "source_role_changelog",
            Self::SourceRoleMigration => "source_role_migration",
            Self::SourceRoleBenchmark => "source_role_benchmark",
            Self::SourceRoleConfiguration => "source_role_configuration",
            Self::KindOfficialDocs => "kind_official_docs",
            Self::KindPackageRegistry => "kind_package_registry",
            Self::KindReleaseNotes => "kind_release_notes",
            Self::KindIssueThread => "kind_issue_thread",
            Self::KindPullRequest => "kind_pull_request",
            Self::KindSecurityAdvisory => "kind_security_advisory",
            Self::KindSourceFile => "kind_source_file",
            Self::SymbolHintMatch => "symbol_hint_match",
            Self::PathHintMatch => "path_hint_match",
            Self::LanguageHintMatch => "language_hint_match",
            Self::FileHintMatch => "file_hint_match",
            Self::ErrorContextMatch => "error_context_match",
            Self::VersionMigrationContext => "version_migration_context",
            Self::PackageNameMatch => "package_name_match",
            Self::SourceTypeMatch => "source_type_match",
            Self::AuthoritativeAdvisory => "authoritative_advisory",
            Self::VendorAdvisory => "vendor_advisory",
            Self::PrimaryResearchSource => "primary_research_source",
            Self::ReferenceImplementation => "reference_implementation",
            Self::BenchmarkSource => "benchmark_source",
            Self::SecurityConsideration => "security_consideration",
        }
    }
}

/// Mode for fetch ranking. Controls which scoring signals are active.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FetchRankMode {
    /// Normal repo search: docs, source, issues, releases.
    #[default]
    Normal,
    /// Exact-error mode: compiler errors, runtime exceptions.
    ExactError,
    /// Package/migration mode: version upgrades, dependency changes.
    PackageMigration,
    /// Security mode: vulnerability advisories.
    Security,
    /// Research mode: multi-source evidence discovery.
    Research,
}

/// Contextual signals from the query and request for scoring.
#[derive(Clone, Debug, Default)]
pub struct RankContext {
    /// Whether the query has a symbol hint.
    pub has_symbol_hint: bool,
    /// Whether the query has a path hint.
    pub has_path_hint: bool,
    /// Whether the query has a language hint.
    pub has_language_hint: bool,
    /// Whether the query has a file hint.
    pub has_file_hint: bool,
    /// Whether error context is present (exact-error mode).
    pub has_error_context: bool,
    /// Whether version or migration context is present.
    pub has_version_context: bool,
    /// Whether a package name is present.
    pub has_package_name: bool,
    /// The ranking mode.
    pub mode: FetchRankMode,
}

impl RankContext {
    /// Build from `RepoQueryHints` and mode.
    pub fn from_hints(hints: &RepoQueryHints, mode: FetchRankMode) -> Self {
        Self {
            has_symbol_hint: hints.symbol.is_some(),
            has_path_hint: hints.path.is_some(),
            has_language_hint: hints.language.is_some(),
            has_file_hint: hints.file.is_some(),
            has_error_context: mode == FetchRankMode::ExactError,
            has_version_context: mode == FetchRankMode::PackageMigration,
            has_package_name: false,
            mode,
        }
    }
}

/// An internal candidate for ranking. Converted to/from the public
/// suggested-fetch types by the caller.
#[derive(Clone, Debug)]
pub struct FetchCandidate {
    /// The URL to fetch.
    pub url: String,
    /// Structured repo_fetch locator, when available.
    pub structured_repo_fetch: bool,
    /// The result group this candidate belongs to (as a string label).
    pub group: String,
    /// Expected content kind.
    pub expected_kind: SourceKind,
    /// Recommended extract mode.
    pub recommended_extract_mode: Option<ExtractMode>,
    /// Original position in the group (for deterministic tie-breaking).
    pub original_order: usize,
    /// Source kind from the card metadata.
    pub source_kind: SourceKind,
    /// Source role from code evidence, if present.
    pub source_role: Option<SourceRole>,
    /// Evidence confidence from code evidence, if present.
    pub evidence_confidence: Option<EvidenceConfidence>,
    /// Whether the URL is a commit-pinned permalink.
    pub is_pinned_permalink: bool,
    /// Whether the URL is a raw content URL.
    pub is_raw_url: bool,
    /// Whether the URL is a mutable browser URL.
    pub is_browser_url: bool,
    /// The domain of the URL.
    pub domain: String,
    /// Accumulated score.
    pub score: i32,
    /// Reasons for the score.
    pub reasons: Vec<FetchRankReason>,
    /// Information gain estimate (0.0 to 1.0).
    pub information_gain: f32,
    /// Whether the candidate is from a stable/pinned source.
    pub stable: bool,
}

/// Extract domain from a URL.
pub fn extract_domain(url: &str) -> String {
    url.split("://")
        .nth(1)
        .and_then(|h| h.split('/').next())
        .unwrap_or("")
        .to_string()
}

/// Determine if a URL is a commit-pinned permalink.
#[allow(dead_code)]
pub(crate) fn is_pinned_permalink(url: &str) -> bool {
    // GitHub/GitLab permalinks contain a 40-char hex SHA in the path
    // e.g. github.com/owner/repo/blob/abc123...def/path
    if let Some(path_start) = url.split("://").nth(1) {
        let path = path_start.split('/').skip(2).collect::<Vec<_>>().join("/");
        // Look for a segment after blob/tree that looks like a SHA
        for segment in path.split('/') {
            if segment.len() >= 12 && segment.chars().all(|c| c.is_ascii_hexdigit()) {
                return true;
            }
        }
    }
    false
}

/// Determine if a URL is a raw content URL.
#[allow(dead_code)]
pub(crate) fn is_raw_url(url: &str) -> bool {
    url.contains("raw.githubusercontent.com") || url.contains("/raw/") || url.contains("raw.")
}

/// Score provenance and stability signals.
fn score_provenance(candidate: &mut FetchCandidate) {
    if candidate.is_pinned_permalink && candidate.is_raw_url {
        candidate.score += 30;
        candidate.reasons.push(FetchRankReason::PinnedRawPermalink);
        candidate.stable = true;
    } else if candidate.is_pinned_permalink {
        candidate.score += 20;
        candidate
            .reasons
            .push(FetchRankReason::PinnedBrowserPermalink);
        candidate.stable = true;
    } else if candidate.structured_repo_fetch {
        // Structured repo_fetch has a stable locator
        candidate.score += 15;
        candidate.stable = true;
    } else if candidate.is_raw_url {
        candidate.score += 10;
        candidate.reasons.push(FetchRankReason::MutableRawUrl);
    } else if candidate.is_browser_url {
        candidate.score += 5;
        candidate.reasons.push(FetchRankReason::MutableBrowserUrl);
    } else {
        candidate.score += 0;
        candidate.reasons.push(FetchRankReason::GenericWebUrl);
    }

    // Penalize sparse code evidence (present but no URLs derived)
    if candidate.evidence_confidence == Some(EvidenceConfidence::Unknown)
        && candidate.source_kind == SourceKind::SourceFile
    {
        candidate.score -= 5;
        candidate.reasons.push(FetchRankReason::SparseCodeEvidence);
    }
}

/// Score evidence confidence signals.
fn score_evidence_confidence(candidate: &mut FetchCandidate) {
    match candidate.evidence_confidence {
        Some(EvidenceConfidence::Exact) => {
            candidate.score += 15;
            candidate.reasons.push(FetchRankReason::ExactConfidence);
        }
        Some(EvidenceConfidence::Strong) => {
            candidate.score += 10;
            candidate.reasons.push(FetchRankReason::StrongConfidence);
        }
        Some(EvidenceConfidence::Weak) => {
            candidate.score += 5;
            candidate.reasons.push(FetchRankReason::WeakConfidence);
        }
        Some(EvidenceConfidence::Unknown) => {
            candidate.score -= 5;
            candidate.reasons.push(FetchRankReason::UnknownConfidence);
        }
        None => {}
    }
}

/// Score source role signals for normal coding investigation.
fn score_source_role_normal(candidate: &mut FetchCandidate) {
    match candidate.source_role {
        Some(SourceRole::Implementation) => {
            candidate.score += 10;
            candidate
                .reasons
                .push(FetchRankReason::SourceRoleImplementation);
        }
        Some(SourceRole::Documentation) => {
            candidate.score += 10;
            candidate
                .reasons
                .push(FetchRankReason::SourceRoleDocumentation);
        }
        Some(SourceRole::Readme) => {
            candidate.score += 8;
            candidate.reasons.push(FetchRankReason::SourceRoleReadme);
        }
        Some(SourceRole::Example) => {
            candidate.score += 5;
            candidate.reasons.push(FetchRankReason::SourceRoleExample);
        }
        Some(SourceRole::Test) => {
            candidate.score += 3;
            candidate.reasons.push(FetchRankReason::SourceRoleTest);
        }
        Some(SourceRole::Changelog) => {
            candidate.score += 5;
            candidate.reasons.push(FetchRankReason::SourceRoleChangelog);
        }
        Some(SourceRole::Migration) => {
            candidate.score += 5;
            candidate.reasons.push(FetchRankReason::SourceRoleMigration);
        }
        Some(SourceRole::Benchmark) => {
            candidate.score += 3;
            candidate.reasons.push(FetchRankReason::SourceRoleBenchmark);
        }
        Some(SourceRole::Configuration) => {
            candidate.score += 2;
            candidate
                .reasons
                .push(FetchRankReason::SourceRoleConfiguration);
        }
        Some(SourceRole::Build) | Some(SourceRole::Unknown) | None => {}
    }
}

/// Score source kind signals for normal mode.
fn score_source_kind_normal(candidate: &mut FetchCandidate) {
    match candidate.source_kind {
        SourceKind::OfficialDocs => {
            candidate.score += 10;
            candidate.reasons.push(FetchRankReason::KindOfficialDocs);
        }
        SourceKind::PackageRegistry => {
            candidate.score += 8;
            candidate.reasons.push(FetchRankReason::KindPackageRegistry);
        }
        SourceKind::ReleaseNotes => {
            candidate.score += 5;
            candidate.reasons.push(FetchRankReason::KindReleaseNotes);
        }
        SourceKind::IssueThread => {
            candidate.score += 5;
            candidate.reasons.push(FetchRankReason::KindIssueThread);
        }
        SourceKind::PullRequest => {
            candidate.score += 5;
            candidate.reasons.push(FetchRankReason::KindPullRequest);
        }
        SourceKind::SourceFile => {
            candidate.score += 5;
            candidate.reasons.push(FetchRankReason::KindSourceFile);
        }
        _ => {}
    }
}

/// Score for exact-error mode.
fn score_exact_error(candidate: &mut FetchCandidate) {
    // In exact-error mode, issues and changelogs get high priority
    match candidate.source_kind {
        SourceKind::IssueThread => {
            candidate.score += 25;
            candidate.reasons.push(FetchRankReason::KindIssueThread);
        }
        SourceKind::PullRequest => {
            candidate.score += 20;
            candidate.reasons.push(FetchRankReason::KindPullRequest);
        }
        SourceKind::ReleaseNotes => {
            candidate.score += 15;
            candidate.reasons.push(FetchRankReason::KindReleaseNotes);
        }
        SourceKind::SourceFile => {
            candidate.score += 10;
            candidate.reasons.push(FetchRankReason::KindSourceFile);
        }
        SourceKind::OfficialDocs => {
            candidate.score += 5;
            candidate.reasons.push(FetchRankReason::KindOfficialDocs);
        }
        _ => {}
    }

    // Changelog/migration source roles get a boost in error mode
    match candidate.source_role {
        Some(SourceRole::Changelog) => {
            candidate.score += 10;
            candidate.reasons.push(FetchRankReason::SourceRoleChangelog);
        }
        Some(SourceRole::Migration) => {
            candidate.score += 8;
            candidate.reasons.push(FetchRankReason::SourceRoleMigration);
        }
        _ => {}
    }
}

/// Score for package/migration mode.
fn score_package_migration(candidate: &mut FetchCandidate) {
    match candidate.source_kind {
        SourceKind::PackageRegistry => {
            candidate.score += 15;
            candidate.reasons.push(FetchRankReason::KindPackageRegistry);
        }
        SourceKind::ReleaseNotes => {
            candidate.score += 20;
            candidate.reasons.push(FetchRankReason::KindReleaseNotes);
        }
        SourceKind::OfficialDocs => {
            candidate.score += 10;
            candidate.reasons.push(FetchRankReason::KindOfficialDocs);
        }
        SourceKind::SourceFile => {
            candidate.score += 5;
            candidate.reasons.push(FetchRankReason::KindSourceFile);
        }
        _ => {}
    }

    match candidate.source_role {
        Some(SourceRole::Changelog) => {
            candidate.score += 15;
            candidate.reasons.push(FetchRankReason::SourceRoleChangelog);
        }
        Some(SourceRole::Migration) => {
            candidate.score += 15;
            candidate.reasons.push(FetchRankReason::SourceRoleMigration);
        }
        _ => {}
    }
}

/// Score for security mode.
fn score_security(candidate: &mut FetchCandidate) {
    match candidate.source_kind {
        SourceKind::SecurityAdvisory => {
            candidate.score += 30;
            candidate
                .reasons
                .push(FetchRankReason::KindSecurityAdvisory);
            candidate
                .reasons
                .push(FetchRankReason::AuthoritativeAdvisory);
        }
        SourceKind::ReleaseNotes => {
            candidate.score += 15;
            candidate.reasons.push(FetchRankReason::KindReleaseNotes);
        }
        SourceKind::IssueThread => {
            candidate.score += 10;
            candidate.reasons.push(FetchRankReason::KindIssueThread);
        }
        _ => {}
    }
}

/// Score for research mode.
fn score_research(candidate: &mut FetchCandidate) {
    match candidate.source_kind {
        SourceKind::OfficialDocs => {
            candidate.score += 15;
            candidate
                .reasons
                .push(FetchRankReason::PrimaryResearchSource);
        }
        SourceKind::SourceFile => {
            candidate.score += 10;
            candidate
                .reasons
                .push(FetchRankReason::ReferenceImplementation);
        }
        SourceKind::SecurityAdvisory => {
            candidate.score += 10;
            candidate
                .reasons
                .push(FetchRankReason::SecurityConsideration);
        }
        SourceKind::ReleaseNotes => {
            candidate.score += 5;
            candidate.reasons.push(FetchRankReason::KindReleaseNotes);
        }
        _ => {}
    }

    if let Some(SourceRole::Benchmark) = candidate.source_role {
        candidate.score += 10;
        candidate.reasons.push(FetchRankReason::BenchmarkSource);
    }
}

/// Score query-context matching signals.
fn score_query_context(candidate: &mut FetchCandidate, ctx: &RankContext) {
    if ctx.has_symbol_hint && candidate.source_kind == SourceKind::SourceFile {
        candidate.score += 10;
        candidate.reasons.push(FetchRankReason::SymbolHintMatch);
    }
    if ctx.has_path_hint {
        candidate.score += 8;
        candidate.reasons.push(FetchRankReason::PathHintMatch);
    }
    if ctx.has_language_hint {
        candidate.score += 5;
        candidate.reasons.push(FetchRankReason::LanguageHintMatch);
    }
    if ctx.has_file_hint {
        candidate.score += 5;
        candidate.reasons.push(FetchRankReason::FileHintMatch);
    }
    if ctx.has_error_context {
        candidate.score += 10;
        candidate.reasons.push(FetchRankReason::ErrorContextMatch);
    }
    if ctx.has_version_context {
        candidate.score += 5;
        candidate
            .reasons
            .push(FetchRankReason::VersionMigrationContext);
    }
    if ctx.has_package_name {
        candidate.score += 8;
        candidate.reasons.push(FetchRankReason::PackageNameMatch);
    }
}

/// Score a single candidate based on context.
pub fn score_candidate(candidate: &mut FetchCandidate, ctx: &RankContext) {
    score_provenance(candidate);
    score_evidence_confidence(candidate);

    match ctx.mode {
        FetchRankMode::Normal => {
            score_source_role_normal(candidate);
            score_source_kind_normal(candidate);
        }
        FetchRankMode::ExactError => {
            score_exact_error(candidate);
        }
        FetchRankMode::PackageMigration => {
            score_package_migration(candidate);
        }
        FetchRankMode::Security => {
            score_security(candidate);
        }
        FetchRankMode::Research => {
            score_research(candidate);
        }
    }

    score_query_context(candidate, ctx);
}

/// Configuration for diversity caps.
#[derive(Clone, Debug)]
pub struct DiversityConfig {
    /// Max suggestions from the same domain. 0 = no cap.
    pub max_per_domain: usize,
    /// Max suggestions from the same group. 0 = no cap.
    pub max_per_group: usize,
    /// Total suggestion cap. 0 = no cap.
    pub total_cap: usize,
}

impl Default for DiversityConfig {
    fn default() -> Self {
        Self {
            max_per_domain: 2,
            max_per_group: 2,
            total_cap: 8,
        }
    }
}

/// Rank and select candidates with diversity caps.
///
/// Scores all candidates, applies diversity caps, sorts by score
/// descending (stable sort preserves original order for ties),
/// and returns the top candidates.
pub fn rank_and_select(
    mut candidates: Vec<FetchCandidate>,
    ctx: &RankContext,
    config: &DiversityConfig,
) -> Vec<FetchCandidate> {
    // Score all candidates
    for candidate in &mut candidates {
        score_candidate(candidate, ctx);
    }

    // Sort by score descending (stable for tie-breaking)
    candidates.sort_by_key(|b| std::cmp::Reverse(b.score));

    // Apply diversity caps
    let mut selected = Vec::new();
    let mut domain_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut group_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    for mut candidate in candidates {
        if config.total_cap > 0 && selected.len() >= config.total_cap {
            break;
        }

        // Check domain cap
        if config.max_per_domain > 0 {
            let count = domain_counts.get(&candidate.domain).copied().unwrap_or(0);
            if count >= config.max_per_domain {
                continue;
            }
        }

        // Check group cap
        if config.max_per_group > 0 {
            let count = group_counts.get(&candidate.group).copied().unwrap_or(0);
            if count >= config.max_per_group {
                continue;
            }
        }

        // Update counts
        if config.max_per_domain > 0 {
            *domain_counts.entry(candidate.domain.clone()).or_insert(0) += 1;
        }
        if config.max_per_group > 0 {
            *group_counts.entry(candidate.group.clone()).or_insert(0) += 1;
        }

        // Compute information gain based on domain/group diversity
        let domain_count = domain_counts.get(&candidate.domain).copied().unwrap_or(1);
        let group_count = group_counts.get(&candidate.group).copied().unwrap_or(1);
        candidate.information_gain = if domain_count == 1 && group_count == 1 {
            1.0
        } else if domain_count <= 2 && group_count <= 2 {
            0.7
        } else {
            0.4
        };

        selected.push(candidate);
    }

    // Re-assign sequential priorities after diversity filtering
    for (i, candidate) in selected.iter_mut().enumerate() {
        candidate.original_order = i;
    }

    selected
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_candidate(
        url: &str,
        source_kind: SourceKind,
        group: &str,
        order: usize,
    ) -> FetchCandidate {
        FetchCandidate {
            url: url.to_string(),
            structured_repo_fetch: false,
            group: group.to_string(),
            expected_kind: SourceKind::Unknown,
            recommended_extract_mode: None,
            original_order: order,
            source_kind,
            source_role: None,
            evidence_confidence: None,
            is_pinned_permalink: false,
            is_raw_url: false,
            is_browser_url: url.starts_with("http"),
            domain: extract_domain(url),
            score: 0,
            reasons: Vec::new(),
            information_gain: 0.0,
            stable: false,
        }
    }

    #[test]
    fn pinned_raw_permalink_scores_highest() {
        let mut a = make_candidate(
            "https://raw.githubusercontent.com/owner/repo/abc123def456/src/lib.rs",
            SourceKind::SourceFile,
            "source",
            0,
        );
        a.is_pinned_permalink = true;
        a.is_raw_url = true;

        let mut b = make_candidate(
            "https://github.com/owner/repo/blob/main/src/lib.rs",
            SourceKind::SourceFile,
            "source",
            1,
        );
        b.is_browser_url = true;

        let ctx = RankContext::default();
        score_candidate(&mut a, &ctx);
        score_candidate(&mut b, &ctx);

        assert!(
            a.score > b.score,
            "pinned raw permalink ({}) should score higher than browser URL ({})",
            a.score,
            b.score
        );
    }

    #[test]
    fn exact_confidence_beats_unknown() {
        let mut a = make_candidate(
            "https://github.com/owner/repo/blob/abc/src/lib.rs",
            SourceKind::SourceFile,
            "source",
            0,
        );
        a.evidence_confidence = Some(EvidenceConfidence::Exact);

        let mut b = make_candidate(
            "https://example.com/docs",
            SourceKind::OfficialDocs,
            "docs",
            1,
        );
        b.evidence_confidence = Some(EvidenceConfidence::Unknown);

        let ctx = RankContext::default();
        score_candidate(&mut a, &ctx);
        score_candidate(&mut b, &ctx);

        assert!(
            a.score > b.score,
            "exact confidence ({}) should beat unknown ({})",
            a.score,
            b.score
        );
    }

    #[test]
    fn official_docs_outrank_source_for_generic_query() {
        let mut a = make_candidate(
            "https://docs.example.com/api",
            SourceKind::OfficialDocs,
            "docs",
            0,
        );

        let mut b = make_candidate(
            "https://github.com/owner/repo/blob/main/src/lib.rs",
            SourceKind::SourceFile,
            "source",
            1,
        );
        b.is_browser_url = true;

        let ctx = RankContext {
            mode: FetchRankMode::Normal,
            ..Default::default()
        };
        score_candidate(&mut a, &ctx);
        score_candidate(&mut b, &ctx);

        assert!(
            a.score > b.score,
            "official docs ({}) should outrank source ({}) for generic query",
            a.score,
            b.score
        );
    }

    #[test]
    fn source_with_symbol_hint_outranks_docs() {
        let mut a = make_candidate(
            "https://github.com/owner/repo/blob/abc/src/lib.rs",
            SourceKind::SourceFile,
            "source",
            0,
        );
        a.is_pinned_permalink = true;
        a.is_raw_url = true;
        a.evidence_confidence = Some(EvidenceConfidence::Exact);
        a.source_role = Some(SourceRole::Implementation);

        let mut b = make_candidate(
            "https://docs.example.com/api",
            SourceKind::OfficialDocs,
            "docs",
            1,
        );

        let ctx = RankContext {
            has_symbol_hint: true,
            mode: FetchRankMode::Normal,
            ..Default::default()
        };
        score_candidate(&mut a, &ctx);
        score_candidate(&mut b, &ctx);

        assert!(
            a.score > b.score,
            "source with symbol hint ({}) should outrank docs ({})",
            a.score,
            b.score
        );
    }

    #[test]
    fn diversity_caps_prevent_domain_dominance() {
        let candidates = vec![
            make_candidate(
                "https://same.example.com/a",
                SourceKind::OfficialDocs,
                "docs",
                0,
            ),
            make_candidate(
                "https://same.example.com/b",
                SourceKind::SourceFile,
                "source",
                1,
            ),
            make_candidate(
                "https://same.example.com/c",
                SourceKind::ReleaseNotes,
                "releases",
                2,
            ),
            make_candidate(
                "https://other.example.com/d",
                SourceKind::IssueThread,
                "issues",
                3,
            ),
        ];

        let ctx = RankContext::default();
        let config = DiversityConfig {
            max_per_domain: 2,
            max_per_group: 2,
            total_cap: 8,
        };
        let selected = rank_and_select(candidates, &ctx, &config);

        let same_domain_count = selected
            .iter()
            .filter(|c| c.domain == "same.example.com")
            .count();
        assert!(
            same_domain_count <= 2,
            "expected at most 2 from same domain, got {same_domain_count}"
        );
    }

    #[test]
    fn diversity_caps_prevent_group_dominance() {
        let candidates = vec![
            make_candidate(
                "https://a.example.com/1",
                SourceKind::SourceFile,
                "source",
                0,
            ),
            make_candidate(
                "https://b.example.com/2",
                SourceKind::SourceFile,
                "source",
                1,
            ),
            make_candidate(
                "https://c.example.com/3",
                SourceKind::SourceFile,
                "source",
                2,
            ),
            make_candidate(
                "https://d.example.com/4",
                SourceKind::OfficialDocs,
                "docs",
                3,
            ),
        ];

        let ctx = RankContext::default();
        let config = DiversityConfig {
            max_per_domain: 10,
            max_per_group: 2,
            total_cap: 8,
        };
        let selected = rank_and_select(candidates, &ctx, &config);

        let source_count = selected.iter().filter(|c| c.group == "source").count();
        assert!(
            source_count <= 2,
            "expected at most 2 from same group, got {source_count}"
        );
    }

    #[test]
    fn equal_scores_retain_original_order() {
        let candidates = vec![
            make_candidate(
                "https://a.example.com/1",
                SourceKind::OfficialDocs,
                "docs",
                0,
            ),
            make_candidate(
                "https://b.example.com/2",
                SourceKind::OfficialDocs,
                "docs",
                1,
            ),
            make_candidate(
                "https://c.example.com/3",
                SourceKind::OfficialDocs,
                "docs",
                2,
            ),
        ];

        let ctx = RankContext::default();
        let config = DiversityConfig {
            max_per_domain: 1,
            max_per_group: 0,
            total_cap: 8,
        };
        let selected = rank_and_select(candidates, &ctx, &config);

        // With domain cap of 1, only one from each domain
        assert_eq!(selected.len(), 3);
        // Original order should be preserved (stable sort)
        assert_eq!(selected[0].domain, "a.example.com");
        assert_eq!(selected[1].domain, "b.example.com");
        assert_eq!(selected[2].domain, "c.example.com");
    }

    #[test]
    fn exact_error_mode_boosts_issues() {
        let mut a = make_candidate(
            "https://github.com/owner/repo/issues/123",
            SourceKind::IssueThread,
            "issues",
            0,
        );

        let mut b = make_candidate(
            "https://docs.example.com/api",
            SourceKind::OfficialDocs,
            "docs",
            1,
        );

        let ctx = RankContext {
            mode: FetchRankMode::ExactError,
            has_error_context: true,
            ..Default::default()
        };
        score_candidate(&mut a, &ctx);
        score_candidate(&mut b, &ctx);

        assert!(
            a.score > b.score,
            "issue thread ({}) should outrank docs ({}) in exact-error mode",
            a.score,
            b.score
        );
    }

    #[test]
    fn security_mode_boosts_advisories() {
        let mut a = make_candidate(
            "https://osv.dev/vulnerability/CVE-2024-0001",
            SourceKind::SecurityAdvisory,
            "advisories",
            0,
        );

        let mut b = make_candidate(
            "https://blog.example.com/exploit",
            SourceKind::Unknown,
            "discussion",
            1,
        );

        let ctx = RankContext {
            mode: FetchRankMode::Security,
            ..Default::default()
        };
        score_candidate(&mut a, &ctx);
        score_candidate(&mut b, &ctx);

        assert!(
            a.score > b.score,
            "security advisory ({}) should outrank blog ({}) in security mode",
            a.score,
            b.score
        );
    }

    #[test]
    fn package_mode_boosts_release_notes() {
        let mut a = make_candidate(
            "https://github.com/owner/repo/releases/tag/v2.0",
            SourceKind::ReleaseNotes,
            "releases",
            0,
        );

        let mut b = make_candidate(
            "https://docs.example.com/api",
            SourceKind::OfficialDocs,
            "docs",
            1,
        );

        let ctx = RankContext {
            mode: FetchRankMode::PackageMigration,
            has_version_context: true,
            has_package_name: true,
            ..Default::default()
        };
        score_candidate(&mut a, &ctx);
        score_candidate(&mut b, &ctx);

        assert!(
            a.score > b.score,
            "release notes ({}) should outrank docs ({}) in package/migration mode",
            a.score,
            b.score
        );
    }

    #[test]
    fn information_gain_computed_correctly() {
        let candidates = vec![
            make_candidate(
                "https://a.example.com/1",
                SourceKind::OfficialDocs,
                "docs",
                0,
            ),
            make_candidate(
                "https://b.example.com/2",
                SourceKind::SourceFile,
                "source",
                1,
            ),
        ];

        let ctx = RankContext::default();
        let config = DiversityConfig::default();
        let selected = rank_and_select(candidates, &ctx, &config);

        // First candidate should have high info gain (new domain, new group)
        assert_eq!(selected[0].information_gain, 1.0);
    }

    #[test]
    fn empty_candidates_produce_empty_output() {
        let ctx = RankContext::default();
        let config = DiversityConfig::default();
        let selected = rank_and_select(vec![], &ctx, &config);
        assert!(selected.is_empty());
    }

    #[test]
    fn total_cap_enforced() {
        let candidates: Vec<FetchCandidate> = (0..20)
            .map(|i| {
                make_candidate(
                    &format!("https://{i}.example.com/page"),
                    SourceKind::OfficialDocs,
                    "docs",
                    i,
                )
            })
            .collect();

        let ctx = RankContext::default();
        let config = DiversityConfig {
            max_per_domain: 100,
            max_per_group: 100,
            total_cap: 5,
        };
        let selected = rank_and_select(candidates, &ctx, &config);
        assert_eq!(selected.len(), 5);
    }

    #[test]
    fn fetch_rank_reason_as_str_is_stable() {
        assert_eq!(
            FetchRankReason::PinnedRawPermalink.as_str(),
            "pinned_raw_permalink"
        );
        assert_eq!(
            FetchRankReason::ExactConfidence.as_str(),
            "exact_confidence"
        );
        assert_eq!(
            FetchRankReason::AuthoritativeAdvisory.as_str(),
            "authoritative_advisory"
        );
        assert_eq!(
            FetchRankReason::PrimaryResearchSource.as_str(),
            "primary_research_source"
        );
    }

    #[test]
    fn pinned_permalink_detected_correctly() {
        assert!(is_pinned_permalink(
            "https://github.com/owner/repo/blob/abc123def456789012345678901234567890abcd/src/lib.rs"
        ));
        assert!(!is_pinned_permalink(
            "https://github.com/owner/repo/blob/main/src/lib.rs"
        ));
        assert!(!is_pinned_permalink("https://docs.example.com/api"));
    }

    #[test]
    fn raw_url_detected_correctly() {
        assert!(is_raw_url(
            "https://raw.githubusercontent.com/owner/repo/main/src/lib.rs"
        ));
        assert!(is_raw_url("https://example.com/raw/file.rs"));
        assert!(!is_raw_url(
            "https://github.com/owner/repo/blob/main/src/lib.rs"
        ));
    }

    #[test]
    fn structured_repo_fetch_gets_stability_boost() {
        let mut a = make_candidate(
            "https://github.com/owner/repo/blob/main/src/lib.rs",
            SourceKind::SourceFile,
            "source",
            0,
        );
        a.structured_repo_fetch = true;

        let mut b = make_candidate(
            "https://example.com/docs",
            SourceKind::OfficialDocs,
            "docs",
            1,
        );

        let ctx = RankContext::default();
        score_candidate(&mut a, &ctx);
        score_candidate(&mut b, &ctx);

        assert!(
            a.score >= b.score,
            "structured repo_fetch candidate ({}) should score at least as high as docs ({})",
            a.score,
            b.score
        );
    }

    #[test]
    fn source_role_implementation_scores_in_normal_mode() {
        let mut a = make_candidate(
            "https://github.com/owner/repo/blob/abc/src/lib.rs",
            SourceKind::SourceFile,
            "source",
            0,
        );
        a.is_pinned_permalink = true;
        a.is_raw_url = true;
        a.evidence_confidence = Some(EvidenceConfidence::Exact);
        a.source_role = Some(SourceRole::Implementation);

        let ctx = RankContext {
            mode: FetchRankMode::Normal,
            ..Default::default()
        };
        score_candidate(&mut a, &ctx);

        assert!(a.score > 0);
        assert!(a
            .reasons
            .contains(&FetchRankReason::SourceRoleImplementation));
    }

    #[test]
    fn source_role_test_low_score_in_normal_mode() {
        let mut a = make_candidate(
            "https://github.com/owner/repo/blob/abc/tests/test.rs",
            SourceKind::SourceFile,
            "tests",
            0,
        );
        a.source_role = Some(SourceRole::Test);

        let ctx = RankContext {
            mode: FetchRankMode::Normal,
            ..Default::default()
        };
        score_candidate(&mut a, &ctx);

        // Test files get a small boost, not a penalty
        assert!(a.score >= 0);
    }
}
