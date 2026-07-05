//! Deterministic research evidence analysis: claims, conflicts, quality, gaps.
//!
//! This module provides pure functions that analyze grouped research results
//! to extract structured claims, detect conflicts, classify source quality,
//! and identify evidence gaps. All functions are deterministic and bounded.

use crate::core::quality::ResultConfidence;
use crate::core::research::{
    ResearchClaim, ResearchClaimType, ResearchConflict, ResearchEvidenceGap,
    ResearchEvidenceGapKind, ResearchQualitySignal, ResearchResultGroup, ResearchResultGroupKind,
    ResearchSourceClass, ResearchSourceQuality,
};
use crate::core::source_card::SourceCard;
use crate::core::workflow::AgentNextAction;

/// Maximum claims returned by `extract_claims`.
const MAX_CLAIMS: usize = 10;

/// Maximum conflicts returned by `detect_conflicts`.
const MAX_CONFLICTS: usize = 5;

/// Maximum gaps returned by `detect_evidence_gaps`.
const MAX_GAPS: usize = 9;

/// Map `SourceKind` + URL heuristics to `ResearchSourceClass`.
pub fn classify_source_class(card: &SourceCard) -> ResearchSourceClass {
    let url_lower = card.url.to_lowercase();
    let title_lower = card.title.to_lowercase();

    match card.metadata.source_kind {
        crate::core::source_card::SourceKind::OfficialDocs => ResearchSourceClass::OfficialDocs,
        crate::core::source_card::SourceKind::PackageRegistry => ResearchSourceClass::ReferenceDocs,
        crate::core::source_card::SourceKind::SecurityAdvisory => {
            ResearchSourceClass::SecurityAdvisory
        }
        crate::core::source_card::SourceKind::ReleaseNotes => ResearchSourceClass::ReleaseNotes,
        crate::core::source_card::SourceKind::News => ResearchSourceClass::NewsArticle,
        crate::core::source_card::SourceKind::Tutorial => classify_tutorial(&url_lower),
        crate::core::source_card::SourceKind::Forum => ResearchSourceClass::ForumThread,
        crate::core::source_card::SourceKind::Reference => classify_reference(&url_lower),
        crate::core::source_card::SourceKind::IssueThread => classify_issue(&card.metadata.issue),
        crate::core::source_card::SourceKind::PullRequest => ResearchSourceClass::EngineeringBlog,
        crate::core::source_card::SourceKind::SourceFile
        | crate::core::source_card::SourceKind::SourceRepository
        | crate::core::source_card::SourceKind::RepositoryRoot
        | crate::core::source_card::SourceKind::Tag
        | crate::core::source_card::SourceKind::Commit
        | crate::core::source_card::SourceKind::SourceDirectory => {
            ResearchSourceClass::RepositorySource
        }
        crate::core::source_card::SourceKind::Unknown => classify_unknown(&url_lower, &title_lower),
    }
}

fn classify_tutorial(url_lower: &str) -> ResearchSourceClass {
    if url_lower.contains("dev.to") || url_lower.contains("medium.com") {
        ResearchSourceClass::EngineeringBlog
    } else if url_lower.contains("blog.rust-lang.org")
        || url_lower.contains("blog.")
        || url_lower.contains("medium.com")
    {
        ResearchSourceClass::VendorBlog
    } else {
        ResearchSourceClass::EngineeringBlog
    }
}

fn classify_reference(url_lower: &str) -> ResearchSourceClass {
    if url_lower.contains("ietf.org")
        || url_lower.contains("w3.org")
        || url_lower.contains("whatwg.org")
        || url_lower.contains("rfc-editor.org")
        || url_lower.contains(".rfc")
        || url_lower.contains("/rfc")
    {
        ResearchSourceClass::StandardSpec
    } else {
        ResearchSourceClass::ReferenceDocs
    }
}

fn classify_issue(issue: &Option<crate::core::source_card::IssueMetadata>) -> ResearchSourceClass {
    if issue
        .as_ref()
        .is_some_and(|i| i.is_pull_request == Some(true))
    {
        ResearchSourceClass::EngineeringBlog
    } else {
        ResearchSourceClass::MaintainerIssue
    }
}

fn classify_unknown(url_lower: &str, title_lower: &str) -> ResearchSourceClass {
    // Academic sources
    if url_lower.contains("arxiv.org")
        || url_lower.contains("acm.org")
        || url_lower.contains("ieee.org")
    {
        return ResearchSourceClass::Paper;
    }

    // Standards/spec bodies
    if url_lower.contains("ietf.org")
        || url_lower.contains("w3.org")
        || url_lower.contains("rfc-editor.org")
        || url_lower.contains(".rfc")
        || url_lower.contains("/rfc")
    {
        return ResearchSourceClass::StandardSpec;
    }

    // Benchmark/performance keywords
    if url_lower.contains("benchmark")
        || title_lower.contains("benchmark")
        || title_lower.contains("performance comparison")
    {
        return ResearchSourceClass::Benchmark;
    }

    // Community Q&A / forums
    if url_lower.contains("stackoverflow.com")
        || url_lower.contains("reddit.com")
        || url_lower.contains("forum.")
    {
        return ResearchSourceClass::ForumThread;
    }

    // Blog domains
    if url_lower.contains("github.com/blog") || url_lower.contains("github.blog") {
        return ResearchSourceClass::EngineeringBlog;
    }

    ResearchSourceClass::Unknown
}

/// Deterministic quality signal assignment based on source class and URL.
pub fn classify_quality_signals(
    card: &SourceCard,
    source_class: ResearchSourceClass,
) -> Vec<ResearchQualitySignal> {
    let url_lower = card.url.to_lowercase();
    let mut signals = Vec::new();

    match source_class {
        ResearchSourceClass::OfficialDocs | ResearchSourceClass::StandardSpec => {
            signals.push(ResearchQualitySignal::PrimarySource);
            signals.push(ResearchQualitySignal::MaintainedCurrent);
        }
        ResearchSourceClass::SecurityAdvisory => {
            signals.push(ResearchQualitySignal::PrimarySource);
            signals.push(ResearchQualitySignal::VersionSpecific);
        }
        ResearchSourceClass::Benchmark => {
            signals.push(ResearchQualitySignal::ReproducibleBenchmark);
        }
        ResearchSourceClass::Paper => {
            signals.push(ResearchQualitySignal::PeerReviewed);
        }
        ResearchSourceClass::RepositorySource => {
            if looks_like_commit_url(&url_lower) {
                signals.push(ResearchQualitySignal::CommitPinned);
            } else {
                signals.push(ResearchQualitySignal::MaintainedCurrent);
            }
        }
        ResearchSourceClass::MaintainerIssue | ResearchSourceClass::EngineeringBlog => {
            signals.push(ResearchQualitySignal::MaintainerAuthored);
        }
        ResearchSourceClass::VendorBlog => {
            signals.push(ResearchQualitySignal::MarketingSource);
        }
        ResearchSourceClass::ForumThread => {
            signals.push(ResearchQualitySignal::SecondarySource);
            signals.push(ResearchQualitySignal::AnecdotalSource);
        }
        ResearchSourceClass::NewsArticle => {
            signals.push(ResearchQualitySignal::SecondarySource);
        }
        ResearchSourceClass::ReferenceDocs
        | ResearchSourceClass::ReleaseNotes
        | ResearchSourceClass::Unknown => {}
    }

    // Check for stale content (> 1 year via URL date patterns)
    if looks_stale(&url_lower) {
        signals.push(ResearchQualitySignal::StaleSource);
    }

    signals
}

fn looks_like_commit_url(url_lower: &str) -> bool {
    // Check for 40-char hex sha patterns in URL path
    let path = url_lower.split('?').next().unwrap_or("");
    path.matches('/').count() >= 4
        && path
            .split('/')
            .any(|seg| seg.len() == 40 && seg.chars().all(|c| c.is_ascii_hexdigit()))
}

fn looks_stale(url_lower: &str) -> bool {
    // Check for year patterns like /2020/, /2021/, etc.
    for year in 2015..2025 {
        let pattern = format!("/{year}/");
        if url_lower.contains(&pattern) {
            return true;
        }
    }
    false
}

/// Compute source quality metadata for all cards in all groups.
pub fn compute_source_qualities(groups: &[ResearchResultGroup]) -> Vec<ResearchSourceQuality> {
    let mut qualities = Vec::new();

    for group in groups {
        for card in &group.results {
            let source_class = classify_source_class(card);
            let quality_signals = classify_quality_signals(card, source_class);
            let is_primary = quality_signals.contains(&ResearchQualitySignal::PrimarySource);
            let is_stale = quality_signals.contains(&ResearchQualitySignal::StaleSource);
            let evidence_notes = build_evidence_notes(source_class, &quality_signals);

            let source_id = card.stable_id.clone().unwrap_or_else(|| card.id.clone());

            qualities.push(ResearchSourceQuality {
                source_id,
                source_class,
                quality_signals,
                is_stale,
                is_primary,
                evidence_notes,
            });
        }
    }

    qualities
}

fn build_evidence_notes(
    source_class: ResearchSourceClass,
    signals: &[ResearchQualitySignal],
) -> Vec<String> {
    let mut notes = Vec::new();

    let class_note = match source_class {
        ResearchSourceClass::OfficialDocs => "official docs",
        ResearchSourceClass::ReferenceDocs => "reference documentation",
        ResearchSourceClass::RepositorySource => "repository source",
        ResearchSourceClass::MaintainerIssue => "maintainer issue",
        ResearchSourceClass::ReleaseNotes => "release notes",
        ResearchSourceClass::Benchmark => "benchmark data",
        ResearchSourceClass::Paper => "academic paper",
        ResearchSourceClass::StandardSpec => "standards specification",
        ResearchSourceClass::SecurityAdvisory => "security advisory",
        ResearchSourceClass::VendorBlog => "vendor blog",
        ResearchSourceClass::EngineeringBlog => "engineering blog",
        ResearchSourceClass::ForumThread => "community forum",
        ResearchSourceClass::NewsArticle => "news article",
        ResearchSourceClass::Unknown => "unclassified source",
    };
    notes.push(class_note.to_string());

    if signals.contains(&ResearchQualitySignal::MaintainedCurrent) {
        notes.push("maintained".to_string());
    }
    if signals.contains(&ResearchQualitySignal::CommitPinned) {
        notes.push("commit-pinned".to_string());
    }
    if signals.contains(&ResearchQualitySignal::PrimarySource) {
        notes.push("primary source".to_string());
    }
    if signals.contains(&ResearchQualitySignal::PeerReviewed) {
        notes.push("peer-reviewed".to_string());
    }
    if signals.contains(&ResearchQualitySignal::StaleSource) {
        notes.push("potentially stale".to_string());
    }

    notes
}

/// Extract deterministic claims from grouped research results.
///
/// When `query` is provided, claim text references the original query for context.
pub fn extract_claims(groups: &[ResearchResultGroup], query: Option<&str>) -> Vec<ResearchClaim> {
    let mut claims = Vec::new();
    let mut claim_index: usize = 0;

    for group in groups {
        if group.results.is_empty() || group.results.len() < 2 {
            continue;
        }
        if claims.len() >= MAX_CLAIMS {
            break;
        }

        let claim_type = group_kind_to_claim_type(group.kind);
        let confidence = compute_claim_confidence(group);
        let supporting_source_ids: Vec<String> = group
            .results
            .iter()
            .map(|c| c.stable_id.clone().unwrap_or_else(|| c.id.clone()))
            .collect();

        let query_context = query.unwrap_or("the research topic");

        let (conflicting_source_ids, text) = if group.kind == ResearchResultGroupKind::Counterpoints
        {
            (
                supporting_source_ids.clone(),
                format!(
                    "Counterpoint evidence from {} challenges findings on: {}",
                    group.label, query_context
                ),
            )
        } else {
            (
                Vec::new(),
                format!(
                    "Evidence from {} supports findings on: {}",
                    group.label, query_context
                ),
            )
        };

        let source_quality_notes = build_source_quality_notes(group);
        let missing_evidence = suggest_missing_evidence(&claim_type, group);

        claims.push(ResearchClaim {
            id: format!("claim_{:?}_{}", group.kind, claim_index),
            text,
            claim_type,
            confidence,
            supporting_source_ids,
            conflicting_source_ids,
            missing_evidence,
            source_quality_notes,
        });

        claim_index += 1;
    }

    // If there are counterpoints, add conflicting IDs to the most recent non-counterpoint claim
    if let Some(counterpoint_group) = groups
        .iter()
        .find(|g| g.kind == ResearchResultGroupKind::Counterpoints)
    {
        if !counterpoint_group.results.is_empty() {
            let counterpoint_ids: Vec<String> = counterpoint_group
                .results
                .iter()
                .map(|c| c.stable_id.clone().unwrap_or_else(|| c.id.clone()))
                .collect();

            // Find the last non-counterpoint claim and add conflicting IDs
            if let Some(last_non_counterpoint) = claims
                .iter_mut()
                .rev()
                .find(|c| !c.id.contains("Counterpoints"))
            {
                last_non_counterpoint
                    .conflicting_source_ids
                    .extend(counterpoint_ids);
            }
        }
    }

    claims
}

/// Build source-informed quality notes for a claim.
fn build_source_quality_notes(group: &ResearchResultGroup) -> Vec<String> {
    let mut notes = vec![format!("{} results", group.results.len())];

    let classes: Vec<ResearchSourceClass> =
        group.results.iter().map(classify_source_class).collect();

    let unique_classes: Vec<ResearchSourceClass> = {
        let mut seen = std::collections::HashSet::new();
        classes
            .into_iter()
            .filter(|c| seen.insert(std::mem::discriminant(c)))
            .collect()
    };

    if !unique_classes.is_empty() {
        let class_names: Vec<String> = unique_classes
            .iter()
            .map(|c| format!("{c:?}").to_lowercase().replace('_', " "))
            .collect();
        notes.push(format!("from {}", class_names.join(", ")));
    }

    let primary_count = group
        .results
        .iter()
        .filter(|c| {
            matches!(
                classify_source_class(c),
                ResearchSourceClass::OfficialDocs
                    | ResearchSourceClass::SecurityAdvisory
                    | ResearchSourceClass::Paper
                    | ResearchSourceClass::StandardSpec
            )
        })
        .count();
    if primary_count > 0 {
        notes.push(format!("{primary_count} primary source(s)"));
    }

    notes
}

/// Suggest missing evidence based on claim type and available source classes.
fn suggest_missing_evidence(
    claim_type: &ResearchClaimType,
    group: &ResearchResultGroup,
) -> Vec<String> {
    let mut missing = Vec::new();
    let classes: Vec<ResearchSourceClass> =
        group.results.iter().map(classify_source_class).collect();

    match claim_type {
        ResearchClaimType::Performance
            if !classes
                .iter()
                .any(|c| matches!(c, ResearchSourceClass::Benchmark)) =>
        {
            missing.push("benchmark data".to_string());
        }
        ResearchClaimType::Security
            if !classes
                .iter()
                .any(|c| matches!(c, ResearchSourceClass::SecurityAdvisory)) =>
        {
            missing.push("security advisory".to_string());
        }
        ResearchClaimType::Maintenance
            if !classes
                .iter()
                .any(|c| matches!(c, ResearchSourceClass::ReleaseNotes)) =>
        {
            missing.push("release notes".to_string());
        }
        _ => {}
    }

    if !classes.iter().any(|c| {
        matches!(
            c,
            ResearchSourceClass::OfficialDocs | ResearchSourceClass::ReferenceDocs
        )
    }) {
        missing.push("official documentation".to_string());
    }

    missing
}

fn group_kind_to_claim_type(kind: ResearchResultGroupKind) -> ResearchClaimType {
    match kind {
        ResearchResultGroupKind::PrimarySources
        | ResearchResultGroupKind::OfficialDocs
        | ResearchResultGroupKind::Specifications
        | ResearchResultGroupKind::ReferenceImplementations
        | ResearchResultGroupKind::DesignDiscussions => ResearchClaimType::Architecture,
        ResearchResultGroupKind::Benchmarks => ResearchClaimType::Performance,
        ResearchResultGroupKind::SecurityConsiderations => ResearchClaimType::Security,
        ResearchResultGroupKind::IssueThreads | ResearchResultGroupKind::ReleaseNotes => {
            ResearchClaimType::Maintenance
        }
        ResearchResultGroupKind::CommunityDiscussion => ResearchClaimType::Ecosystem,
        ResearchResultGroupKind::Counterpoints => ResearchClaimType::Architecture,
        ResearchResultGroupKind::AcademicOrFormalSources => ResearchClaimType::Architecture,
        ResearchResultGroupKind::RecentNews => ResearchClaimType::Ecosystem,
        ResearchResultGroupKind::Unknown => ResearchClaimType::Unknown,
    }
}

fn compute_claim_confidence(group: &ResearchResultGroup) -> ResultConfidence {
    if let Some(ref qs) = group.quality_summary {
        if qs.high_confidence_count > 0 {
            return ResultConfidence::High;
        }
        if qs.primary_source_count > 0 {
            return ResultConfidence::Medium;
        }
    }

    ResultConfidence::Unknown
}

/// Detect conflicts between sources.
pub fn detect_conflicts(
    groups: &[ResearchResultGroup],
    claims: &[ResearchClaim],
) -> Vec<ResearchConflict> {
    let mut conflicts = Vec::new();

    // 1. Counterpoints group creates a conflict
    if let Some(counterpoint_group) = groups
        .iter()
        .find(|g| g.kind == ResearchResultGroupKind::Counterpoints)
    {
        if !counterpoint_group.results.is_empty() && conflicts.len() < MAX_CONFLICTS {
            let counterpoint_ids: Vec<String> = counterpoint_group
                .results
                .iter()
                .map(|c| c.stable_id.clone().unwrap_or_else(|| c.id.clone()))
                .collect();

            let claim_ids: Vec<String> = claims
                .iter()
                .filter(|c| c.id.contains("Counterpoints"))
                .map(|c| c.id.clone())
                .collect();

            let side_a_ids: Vec<String> = groups
                .iter()
                .filter(|g| g.kind != ResearchResultGroupKind::Counterpoints)
                .flat_map(|g| {
                    g.results
                        .iter()
                        .map(|c| c.stable_id.clone().unwrap_or_else(|| c.id.clone()))
                })
                .take(3)
                .collect();

            conflicts.push(ResearchConflict {
                id: "conflict_counterpoints_0".to_string(),
                topic: "Counterpoint evidence found".to_string(),
                claim_ids,
                side_a_source_ids: side_a_ids,
                side_b_source_ids: counterpoint_ids,
                notes: vec!["Sources present opposing viewpoints".to_string()],
            });
        }
    }

    // 2. Quality disagreement: groups where cards span very different quality tiers
    for group in groups {
        if conflicts.len() >= MAX_CONFLICTS {
            break;
        }
        if group.results.len() < 2 {
            continue;
        }

        let classes: Vec<ResearchSourceClass> =
            group.results.iter().map(classify_source_class).collect();

        let has_high_quality = classes.iter().any(|c| {
            matches!(
                c,
                ResearchSourceClass::OfficialDocs
                    | ResearchSourceClass::StandardSpec
                    | ResearchSourceClass::SecurityAdvisory
            )
        });
        let has_low_quality = classes.iter().any(|c| {
            matches!(
                c,
                ResearchSourceClass::ForumThread | ResearchSourceClass::NewsArticle
            )
        });

        if has_high_quality && has_low_quality {
            let source_ids: Vec<String> = group
                .results
                .iter()
                .map(|c| c.stable_id.clone().unwrap_or_else(|| c.id.clone()))
                .collect();

            let conflict_id = format!("conflict_quality_{:?}", group.kind);
            conflicts.push(ResearchConflict {
                id: conflict_id,
                topic: "Source quality disagreement".to_string(),
                claim_ids: Vec::new(),
                side_a_source_ids: source_ids.clone(),
                side_b_source_ids: source_ids,
                notes: vec!["Mixed quality tiers within the same group".to_string()],
            });
        }
    }

    conflicts.truncate(MAX_CONFLICTS);
    conflicts
}

/// Detect evidence gaps in the research results.
pub fn detect_evidence_gaps(
    groups: &[ResearchResultGroup],
    claims: &[ResearchClaim],
    conflicts: &[ResearchConflict],
    query: Option<&str>,
) -> Vec<ResearchEvidenceGap> {
    let mut gaps = Vec::new();

    let has_kind = |kind: ResearchResultGroupKind| -> bool {
        groups
            .iter()
            .any(|g| g.kind == kind && !g.results.is_empty())
    };

    let all_source_ids: Vec<String> = groups
        .iter()
        .flat_map(|g| {
            g.results
                .iter()
                .map(|c| c.stable_id.clone().unwrap_or_else(|| c.id.clone()))
        })
        .collect();

    let all_claim_ids: Vec<String> = claims.iter().map(|c| c.id.clone()).collect();

    // 1. No primary source
    if !has_kind(ResearchResultGroupKind::PrimarySources)
        && !has_kind(ResearchResultGroupKind::OfficialDocs)
        && gaps.len() < MAX_GAPS
    {
        gaps.push(ResearchEvidenceGap {
            kind: ResearchEvidenceGapKind::NoPrimarySource,
            message: "No primary or official source found in the results".to_string(),
            affected_claim_ids: all_claim_ids.clone(),
            affected_source_ids: Vec::new(),
            recommended_actions: vec![AgentNextAction::new(
                "web_search",
                "fetch_primary_source",
                3,
                serde_json::json!({"query": "official documentation", "intent": "docs"}),
                Vec::new(),
            )],
        });
    }

    // 2. No recent source
    if !has_kind(ResearchResultGroupKind::RecentNews) && gaps.len() < MAX_GAPS {
        gaps.push(ResearchEvidenceGap {
            kind: ResearchEvidenceGapKind::NoRecentSource,
            message: "No recent news or discussion found".to_string(),
            affected_claim_ids: all_claim_ids.clone(),
            affected_source_ids: Vec::new(),
            recommended_actions: vec![AgentNextAction::new(
                "web_search",
                "fetch_recent_news",
                3,
                serde_json::json!({"query": "recent news", "intent": "news", "freshness": "month"}),
                Vec::new(),
            )],
        });
    }

    // 3. No benchmark source
    if !has_kind(ResearchResultGroupKind::Benchmarks) && gaps.len() < MAX_GAPS {
        gaps.push(ResearchEvidenceGap {
            kind: ResearchEvidenceGapKind::NoBenchmarkSource,
            message: "No benchmark or performance data found".to_string(),
            affected_claim_ids: all_claim_ids.clone(),
            affected_source_ids: Vec::new(),
            recommended_actions: vec![AgentNextAction::new(
                "web_search",
                "fetch_benchmarks",
                3,
                serde_json::json!({"query": "benchmarks performance", "intent": "code"}),
                Vec::new(),
            )],
        });
    }

    // 4. No security source
    if !has_kind(ResearchResultGroupKind::SecurityConsiderations) && gaps.len() < MAX_GAPS {
        gaps.push(ResearchEvidenceGap {
            kind: ResearchEvidenceGapKind::NoSecuritySource,
            message: "No security considerations found".to_string(),
            affected_claim_ids: all_claim_ids.clone(),
            affected_source_ids: Vec::new(),
            recommended_actions: vec![AgentNextAction::new(
                "web_search",
                "fetch_security",
                3,
                serde_json::json!({"query": "security vulnerabilities", "intent": "security"}),
                Vec::new(),
            )],
        });
    }

    // 5. No release notes / changelog
    if !has_kind(ResearchResultGroupKind::ReleaseNotes) && gaps.len() < MAX_GAPS {
        gaps.push(ResearchEvidenceGap {
            kind: ResearchEvidenceGapKind::NoMigrationChangelog,
            message: "No release notes or changelog found".to_string(),
            affected_claim_ids: all_claim_ids.clone(),
            affected_source_ids: Vec::new(),
            recommended_actions: vec![AgentNextAction::new(
                "web_search",
                "fetch_changelog",
                3,
                serde_json::json!({"query": "changelog release notes", "intent": "releases"}),
                Vec::new(),
            )],
        });
    }

    // 6. Only secondary sources
    let total_results: usize = groups.iter().map(|g| g.results.len()).sum();
    if total_results > 0 && groups.iter().all(|g| g.results.len() <= 1) && gaps.len() < MAX_GAPS {
        gaps.push(ResearchEvidenceGap {
            kind: ResearchEvidenceGapKind::OnlySecondarySources,
            message: "All groups contain only a single source — evidence is thin".to_string(),
            affected_claim_ids: all_claim_ids.clone(),
            affected_source_ids: all_source_ids.clone(),
            recommended_actions: vec![AgentNextAction::new(
                "web_search",
                "fetch_more_sources",
                3,
                serde_json::json!({"query": "additional sources", "intent": "docs"}),
                Vec::new(),
            )],
        });
    }

    // 7. Conflicting evidence unresolved
    if !conflicts.is_empty()
        && !claims
            .iter()
            .any(|c| c.confidence == ResultConfidence::High)
        && gaps.len() < MAX_GAPS
    {
        gaps.push(ResearchEvidenceGap {
            kind: ResearchEvidenceGapKind::ConflictingEvidenceUnresolved,
            message: "Sources conflict but no high-confidence claim resolves the disagreement"
                .to_string(),
            affected_claim_ids: all_claim_ids.clone(),
            affected_source_ids: all_source_ids.clone(),
            recommended_actions: vec![AgentNextAction::new(
                "web_search",
                "resolve_conflict",
                3,
                serde_json::json!({"query": "authoritative source", "intent": "docs"}),
                Vec::new(),
            )],
        });
    }

    // 8. Version context missing: query mentions versions but no release notes group
    if let Some(q) = query {
        let q_lower = q.to_lowercase();
        let has_version_hint = q_lower.contains("v1.")
            || q_lower.contains("v2.")
            || q_lower.contains("v3.")
            || q_lower.contains("version ")
            || q_lower.contains("migrate")
            || q_lower.contains("migration")
            || q_lower.contains("breaking change")
            || q_lower.contains("changelog");
        if has_version_hint
            && !has_kind(ResearchResultGroupKind::ReleaseNotes)
            && gaps.len() < MAX_GAPS
        {
            gaps.push(ResearchEvidenceGap {
                kind: ResearchEvidenceGapKind::VersionContextMissing,
                message: "Query references versions or migration but no release notes found"
                    .to_string(),
                affected_claim_ids: all_claim_ids,
                affected_source_ids: all_source_ids,
                recommended_actions: vec![AgentNextAction::new(
                    "web_search",
                    "fetch_release_notes",
                    3,
                    serde_json::json!({"query": "release notes changelog", "intent": "releases"}),
                    Vec::new(),
                )],
            });
        }
    }

    gaps
}

/// Top-level orchestrator: analyze research evidence and return all results.
pub fn analyze_research_evidence(
    groups: &[ResearchResultGroup],
    query: Option<&str>,
) -> (
    Vec<ResearchClaim>,
    Vec<ResearchConflict>,
    Vec<ResearchSourceQuality>,
    Vec<ResearchEvidenceGap>,
) {
    let source_quality = compute_source_qualities(groups);
    let claims = extract_claims(groups, query);
    let conflicts = detect_conflicts(groups, &claims);
    let evidence_gaps = detect_evidence_gaps(groups, &claims, &conflicts, query);
    (claims, conflicts, source_quality, evidence_gaps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::result::TrustLevel;
    use crate::core::source_card::{SourceKind, SourceMetadata};

    fn make_card(source_kind: SourceKind, url: &str) -> SourceCard {
        let mut card = SourceCard::new(
            "Test",
            url,
            vec!["test".to_string()],
            None,
            TrustLevel::ExternalUntrusted,
        );
        card.metadata = SourceMetadata {
            source_kind,
            ..Default::default()
        };
        card
    }

    fn make_group(kind: ResearchResultGroupKind, cards: Vec<SourceCard>) -> ResearchResultGroup {
        ResearchResultGroup {
            kind,
            label: format!("{kind:?}"),
            results: cards,
            truncated: false,
            quality_summary: None,
        }
    }

    // ---- classify_source_class tests ----

    #[test]
    fn source_class_official_docs() {
        let card = make_card(SourceKind::OfficialDocs, "https://docs.rs/axum");
        assert_eq!(
            classify_source_class(&card),
            ResearchSourceClass::OfficialDocs
        );
    }

    #[test]
    fn source_class_security_advisory() {
        let card = make_card(
            SourceKind::SecurityAdvisory,
            "https://osv.dev/vulnerability/GHSA-xxxx",
        );
        assert_eq!(
            classify_source_class(&card),
            ResearchSourceClass::SecurityAdvisory
        );
    }

    #[test]
    fn source_class_benchmark_url() {
        let card = make_card(SourceKind::Unknown, "https://example.com/benchmark-results");
        assert_eq!(classify_source_class(&card), ResearchSourceClass::Benchmark);
    }

    #[test]
    fn source_class_paper_url() {
        let card = make_card(SourceKind::Unknown, "https://arxiv.org/abs/2301.00001");
        assert_eq!(classify_source_class(&card), ResearchSourceClass::Paper);
    }

    #[test]
    fn source_class_forum() {
        let card = make_card(SourceKind::Forum, "https://forum.example.com/t/topic");
        assert_eq!(
            classify_source_class(&card),
            ResearchSourceClass::ForumThread
        );
    }

    #[test]
    fn source_class_standard_spec() {
        let card = make_card(
            SourceKind::Reference,
            "https://www.rfc-editor.org/rfc/rfc9110",
        );
        assert_eq!(
            classify_source_class(&card),
            ResearchSourceClass::StandardSpec
        );
    }

    #[test]
    fn source_class_reference_docs() {
        let card = make_card(SourceKind::Reference, "https://docs.example.com/api");
        assert_eq!(
            classify_source_class(&card),
            ResearchSourceClass::ReferenceDocs
        );
    }

    #[test]
    fn source_class_tutorial_dev_to() {
        let card = make_card(SourceKind::Tutorial, "https://dev.to/foo/tutorial");
        assert_eq!(
            classify_source_class(&card),
            ResearchSourceClass::EngineeringBlog
        );
    }

    #[test]
    fn source_class_repository_source() {
        let card = make_card(
            SourceKind::SourceFile,
            "https://github.com/tokio-rs/axum/blob/main/src/lib.rs",
        );
        assert_eq!(
            classify_source_class(&card),
            ResearchSourceClass::RepositorySource
        );
    }

    #[test]
    fn source_class_issue_thread() {
        let card = make_card(
            SourceKind::IssueThread,
            "https://github.com/tokio-rs/axum/issues/123",
        );
        assert_eq!(
            classify_source_class(&card),
            ResearchSourceClass::MaintainerIssue
        );
    }

    #[test]
    fn source_class_pull_request() {
        let card = make_card(
            SourceKind::PullRequest,
            "https://github.com/tokio-rs/axum/pull/789",
        );
        assert_eq!(
            classify_source_class(&card),
            ResearchSourceClass::EngineeringBlog
        );
    }

    #[test]
    fn source_class_unknown_stackoverflow() {
        let card = make_card(SourceKind::Unknown, "https://stackoverflow.com/q/123");
        assert_eq!(
            classify_source_class(&card),
            ResearchSourceClass::ForumThread
        );
    }

    // ---- classify_quality_signals tests ----

    #[test]
    fn quality_signals_official_docs() {
        let card = make_card(SourceKind::OfficialDocs, "https://docs.rs/axum");
        let signals = classify_quality_signals(&card, ResearchSourceClass::OfficialDocs);
        assert!(signals.contains(&ResearchQualitySignal::PrimarySource));
        assert!(signals.contains(&ResearchQualitySignal::MaintainedCurrent));
    }

    #[test]
    fn quality_signals_benchmark() {
        let card = make_card(SourceKind::Unknown, "https://example.com/benchmark");
        let signals = classify_quality_signals(&card, ResearchSourceClass::Benchmark);
        assert!(signals.contains(&ResearchQualitySignal::ReproducibleBenchmark));
    }

    #[test]
    fn quality_signals_stale_source() {
        let card = make_card(SourceKind::Unknown, "https://example.com/2020/old-post");
        let signals = classify_quality_signals(&card, ResearchSourceClass::Unknown);
        assert!(signals.contains(&ResearchQualitySignal::StaleSource));
    }

    #[test]
    fn quality_signals_forum() {
        let card = make_card(SourceKind::Forum, "https://forum.example.com/t/topic");
        let signals = classify_quality_signals(&card, ResearchSourceClass::ForumThread);
        assert!(signals.contains(&ResearchQualitySignal::SecondarySource));
        assert!(signals.contains(&ResearchQualitySignal::AnecdotalSource));
    }

    #[test]
    fn quality_signals_paper() {
        let card = make_card(SourceKind::Unknown, "https://arxiv.org/abs/2301.00001");
        let signals = classify_quality_signals(&card, ResearchSourceClass::Paper);
        assert!(signals.contains(&ResearchQualitySignal::PeerReviewed));
    }

    // ---- claims extraction tests ----

    #[test]
    fn claims_from_non_empty_groups() {
        let cards = vec![
            make_card(SourceKind::OfficialDocs, "https://docs.rs/axum"),
            make_card(SourceKind::OfficialDocs, "https://docs.rs/serde"),
        ];
        let group = make_group(ResearchResultGroupKind::OfficialDocs, cards);
        let claims = extract_claims(&[group], None);
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].claim_type, ResearchClaimType::Architecture);
        assert_eq!(claims[0].supporting_source_ids.len(), 2);
    }

    #[test]
    fn claims_bounded_at_10() {
        let groups: Vec<ResearchResultGroup> = (0..15)
            .map(|i| {
                let cards = vec![
                    make_card(
                        SourceKind::OfficialDocs,
                        &format!("https://docs.example.com/{i}a"),
                    ),
                    make_card(
                        SourceKind::OfficialDocs,
                        &format!("https://docs.example.com/{i}b"),
                    ),
                ];
                make_group(ResearchResultGroupKind::OfficialDocs, cards)
            })
            .collect();
        let claims = extract_claims(&groups, None);
        assert!(claims.len() <= MAX_CLAIMS);
    }

    #[test]
    fn counterpoint_claim_has_conflicting_ids() {
        let normal_cards = vec![
            make_card(SourceKind::OfficialDocs, "https://docs.rs/axum"),
            make_card(SourceKind::OfficialDocs, "https://docs.rs/serde"),
        ];
        let counterpoint_cards = vec![
            make_card(SourceKind::Unknown, "https://example.com/criticism"),
            make_card(SourceKind::Unknown, "https://example.com/drawbacks"),
        ];
        let groups = vec![
            make_group(ResearchResultGroupKind::OfficialDocs, normal_cards),
            make_group(ResearchResultGroupKind::Counterpoints, counterpoint_cards),
        ];
        let claims = extract_claims(&groups, None);
        // The counterpoint group should produce a claim with conflicting IDs
        let counterpoint_claim = claims.iter().find(|c| c.id.contains("Counterpoints"));
        assert!(counterpoint_claim.is_some());
        assert!(!counterpoint_claim
            .unwrap()
            .conflicting_source_ids
            .is_empty());
    }

    #[test]
    fn claims_skips_single_result_groups() {
        let cards = vec![make_card(SourceKind::OfficialDocs, "https://docs.rs/axum")];
        let group = make_group(ResearchResultGroupKind::OfficialDocs, cards);
        let claims = extract_claims(&[group], None);
        assert!(claims.is_empty());
    }

    // ---- conflict detection tests ----

    #[test]
    fn counterpoints_create_conflict() {
        let normal_cards = vec![
            make_card(SourceKind::OfficialDocs, "https://docs.rs/axum"),
            make_card(SourceKind::OfficialDocs, "https://docs.rs/serde"),
        ];
        let counterpoint_cards = vec![
            make_card(SourceKind::Unknown, "https://example.com/criticism"),
            make_card(SourceKind::Unknown, "https://example.com/drawbacks"),
        ];
        let groups = vec![
            make_group(ResearchResultGroupKind::OfficialDocs, normal_cards),
            make_group(ResearchResultGroupKind::Counterpoints, counterpoint_cards),
        ];
        let claims = extract_claims(&groups, None);
        let conflicts = detect_conflicts(&groups, &claims);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].id, "conflict_counterpoints_0");
        assert_eq!(conflicts[0].topic, "Counterpoint evidence found");
    }

    #[test]
    fn conflicts_bounded_at_5() {
        let groups: Vec<ResearchResultGroup> = (0..8)
            .map(|i| {
                let cards = vec![
                    make_card(
                        SourceKind::OfficialDocs,
                        &format!("https://docs.example.com/{i}a"),
                    ),
                    make_card(
                        SourceKind::Unknown,
                        &format!("https://stackoverflow.com/{i}"),
                    ),
                ];
                make_group(ResearchResultGroupKind::OfficialDocs, cards)
            })
            .collect();
        let conflicts = detect_conflicts(&groups, &[]);
        assert!(conflicts.len() <= MAX_CONFLICTS);
    }

    // ---- evidence gaps tests ----

    #[test]
    fn gap_no_primary_when_absent() {
        let groups = vec![make_group(
            ResearchResultGroupKind::CommunityDiscussion,
            vec![
                make_card(SourceKind::Unknown, "https://stackoverflow.com/q/1"),
                make_card(SourceKind::Unknown, "https://stackoverflow.com/q/2"),
            ],
        )];
        let gaps = detect_evidence_gaps(&groups, &[], &[], None);
        assert!(gaps
            .iter()
            .any(|g| g.kind == ResearchEvidenceGapKind::NoPrimarySource));
    }

    #[test]
    fn gap_no_benchmark_when_absent() {
        let groups = vec![make_group(
            ResearchResultGroupKind::OfficialDocs,
            vec![
                make_card(SourceKind::OfficialDocs, "https://docs.rs/axum"),
                make_card(SourceKind::OfficialDocs, "https://docs.rs/serde"),
            ],
        )];
        let gaps = detect_evidence_gaps(&groups, &[], &[], None);
        assert!(gaps
            .iter()
            .any(|g| g.kind == ResearchEvidenceGapKind::NoBenchmarkSource));
    }

    #[test]
    fn gaps_bounded() {
        let groups = vec![];
        let gaps = detect_evidence_gaps(&groups, &[], &[], None);
        assert!(gaps.len() <= MAX_GAPS);
    }

    #[test]
    fn gap_no_recent_when_absent() {
        let groups = vec![make_group(
            ResearchResultGroupKind::OfficialDocs,
            vec![
                make_card(SourceKind::OfficialDocs, "https://docs.rs/axum"),
                make_card(SourceKind::OfficialDocs, "https://docs.rs/serde"),
            ],
        )];
        let gaps = detect_evidence_gaps(&groups, &[], &[], None);
        assert!(gaps
            .iter()
            .any(|g| g.kind == ResearchEvidenceGapKind::NoRecentSource));
    }

    #[test]
    fn gap_conflicting_unresolved_when_conflicts_exist() {
        let groups = vec![make_group(
            ResearchResultGroupKind::OfficialDocs,
            vec![
                make_card(SourceKind::OfficialDocs, "https://docs.rs/axum"),
                make_card(SourceKind::OfficialDocs, "https://docs.rs/serde"),
            ],
        )];
        let conflicts = vec![ResearchConflict {
            id: "test_conflict".to_string(),
            topic: "test".to_string(),
            claim_ids: vec![],
            side_a_source_ids: vec![],
            side_b_source_ids: vec![],
            notes: vec![],
        }];
        let gaps = detect_evidence_gaps(&groups, &[], &conflicts, None);
        assert!(gaps
            .iter()
            .any(|g| g.kind == ResearchEvidenceGapKind::ConflictingEvidenceUnresolved));
    }

    // ---- full analysis tests ----

    #[test]
    fn analyze_empty_groups() {
        let (claims, conflicts, source_quality, evidence_gaps) =
            analyze_research_evidence(&[], None);
        assert!(claims.is_empty());
        assert!(conflicts.is_empty());
        assert!(source_quality.is_empty());
        assert!(!evidence_gaps.is_empty()); // should detect gaps
    }

    #[test]
    fn analyze_with_mixed_groups() {
        let groups = vec![
            make_group(
                ResearchResultGroupKind::OfficialDocs,
                vec![
                    make_card(SourceKind::OfficialDocs, "https://docs.rs/axum"),
                    make_card(SourceKind::OfficialDocs, "https://docs.rs/serde"),
                ],
            ),
            make_group(
                ResearchResultGroupKind::Counterpoints,
                vec![
                    make_card(SourceKind::Unknown, "https://example.com/criticism"),
                    make_card(SourceKind::Unknown, "https://example.com/drawbacks"),
                ],
            ),
        ];
        let (claims, conflicts, source_quality, evidence_gaps) =
            analyze_research_evidence(&groups, None);
        assert!(!claims.is_empty());
        assert!(!conflicts.is_empty());
        assert_eq!(source_quality.len(), 4);
        assert!(!evidence_gaps.is_empty());
    }

    // ---- new Phase 9 tests ----

    #[test]
    fn claim_extraction_with_counterpoint_group() {
        let primary_cards = vec![
            make_card(SourceKind::OfficialDocs, "https://docs.rs/axum"),
            make_card(SourceKind::OfficialDocs, "https://docs.rs/serde"),
            make_card(SourceKind::OfficialDocs, "https://docs.rs/tokio"),
        ];
        let counterpoint_cards = vec![
            make_card(SourceKind::Unknown, "https://example.com/criticism"),
            make_card(SourceKind::Unknown, "https://example.com/drawbacks"),
        ];
        let groups = vec![
            make_group(ResearchResultGroupKind::PrimarySources, primary_cards),
            make_group(ResearchResultGroupKind::Counterpoints, counterpoint_cards),
        ];
        let claims = extract_claims(&groups, None);
        assert!(!claims.is_empty());
        let primary_claim = claims.iter().find(|c| c.id.contains("PrimarySources"));
        assert!(primary_claim.is_some());
        let claim = primary_claim.unwrap();
        assert_eq!(claim.supporting_source_ids.len(), 3);
        assert!(!claim.conflicting_source_ids.is_empty());
    }

    #[test]
    fn quality_signal_for_stale_source() {
        let card = make_card(
            SourceKind::Tutorial,
            "https://example.com/2020/old-tutorial",
        );
        let source_class = classify_source_class(&card);
        let signals = classify_quality_signals(&card, source_class);
        assert!(
            signals.contains(&ResearchQualitySignal::StaleSource),
            "expected StaleSource signal for tutorial with old year in URL"
        );
    }

    #[test]
    fn full_analysis_with_mixed_groups() {
        let groups = vec![
            make_group(
                ResearchResultGroupKind::PrimarySources,
                vec![
                    make_card(SourceKind::OfficialDocs, "https://docs.rs/axum"),
                    make_card(SourceKind::OfficialDocs, "https://docs.rs/serde"),
                    make_card(SourceKind::OfficialDocs, "https://docs.rs/tokio"),
                ],
            ),
            make_group(
                ResearchResultGroupKind::Counterpoints,
                vec![
                    make_card(SourceKind::Unknown, "https://example.com/criticism"),
                    make_card(SourceKind::Unknown, "https://example.com/drawbacks"),
                ],
            ),
            make_group(
                ResearchResultGroupKind::Benchmarks,
                vec![make_card(
                    SourceKind::Unknown,
                    "https://example.com/benchmark-results",
                )],
            ),
            make_group(ResearchResultGroupKind::OfficialDocs, vec![]),
        ];
        let (claims, conflicts, source_quality, evidence_gaps) =
            analyze_research_evidence(&groups, None);
        // Non-empty groups with 2+ results produce claims
        assert!(!claims.is_empty());
        // Counterpoints produce conflicts
        assert!(!conflicts.is_empty());
        // Source quality computed for all 6 cards
        assert_eq!(source_quality.len(), 6);
        // Empty OfficialDocs group doesn't count as having OfficialDocs,
        // so NoPrimarySource gap should be present (PrimarySources is non-empty
        // but the check is PrimarySources || OfficialDocs — PrimarySources IS
        // present so NoPrimarySource should NOT be present)
        assert!(
            !evidence_gaps
                .iter()
                .any(|g| g.kind == ResearchEvidenceGapKind::NoPrimarySource),
            "PrimarySources group is non-empty, so NoPrimarySource should not fire"
        );
    }

    #[test]
    fn source_class_arxiv_is_paper() {
        let card = make_card(SourceKind::Unknown, "https://arxiv.org/abs/2301.00001");
        assert_eq!(classify_source_class(&card), ResearchSourceClass::Paper);
    }

    #[test]
    fn source_class_ietf_is_standard_spec() {
        let card = make_card(SourceKind::Unknown, "https://www.ietf.org/rfc/rfc9110");
        assert_eq!(
            classify_source_class(&card),
            ResearchSourceClass::StandardSpec
        );
    }

    #[test]
    fn source_class_stackoverflow_is_forum() {
        let card = make_card(SourceKind::Unknown, "https://stackoverflow.com/q/12345");
        assert_eq!(
            classify_source_class(&card),
            ResearchSourceClass::ForumThread
        );
    }

    #[test]
    fn source_class_medium_is_engineering_blog() {
        let card = make_card(SourceKind::Tutorial, "https://medium.com/@user/article");
        assert_eq!(
            classify_source_class(&card),
            ResearchSourceClass::EngineeringBlog
        );
    }

    #[test]
    fn source_class_github_blog_is_engineering_blog() {
        let card = make_card(SourceKind::Unknown, "https://github.blog/2024-01-01-post");
        assert_eq!(
            classify_source_class(&card),
            ResearchSourceClass::EngineeringBlog
        );
    }

    #[test]
    fn version_context_missing_gap_detected() {
        let groups = vec![make_group(
            ResearchResultGroupKind::PrimarySources,
            vec![
                make_card(SourceKind::OfficialDocs, "https://docs.rs/axum"),
                make_card(SourceKind::OfficialDocs, "https://docs.rs/serde"),
            ],
        )];
        let gaps = detect_evidence_gaps(&groups, &[], &[], Some("migrate from v1 to v2"));
        assert!(
            gaps.iter()
                .any(|g| g.kind == ResearchEvidenceGapKind::VersionContextMissing),
            "expected VersionContextMissing when query has version hints and no ReleaseNotes group"
        );
    }

    #[test]
    fn version_context_not_missing_when_release_notes_present() {
        let groups = vec![make_group(
            ResearchResultGroupKind::ReleaseNotes,
            vec![
                make_card(
                    SourceKind::ReleaseNotes,
                    "https://github.com/foo/releases/tag/v2.0",
                ),
                make_card(
                    SourceKind::ReleaseNotes,
                    "https://github.com/foo/releases/tag/v1.0",
                ),
            ],
        )];
        let gaps = detect_evidence_gaps(&groups, &[], &[], Some("changelog for v2.0"));
        assert!(
            !gaps
                .iter()
                .any(|g| g.kind == ResearchEvidenceGapKind::VersionContextMissing),
            "ReleaseNotes group is present, so VersionContextMissing should not fire"
        );
    }

    #[test]
    fn claim_ids_are_deterministic() {
        let groups = vec![
            make_group(
                ResearchResultGroupKind::PrimarySources,
                vec![
                    make_card(SourceKind::OfficialDocs, "https://docs.rs/axum"),
                    make_card(SourceKind::OfficialDocs, "https://docs.rs/serde"),
                ],
            ),
            make_group(
                ResearchResultGroupKind::Counterpoints,
                vec![
                    make_card(SourceKind::Unknown, "https://example.com/criticism"),
                    make_card(SourceKind::Unknown, "https://example.com/drawbacks"),
                ],
            ),
        ];
        let claims_a = extract_claims(&groups, None);
        let claims_b = extract_claims(&groups, None);
        assert_eq!(claims_a.len(), claims_b.len());
        for (a, b) in claims_a.iter().zip(claims_b.iter()) {
            assert_eq!(a.id, b.id, "claim IDs must be deterministic");
            assert_eq!(a.text, b.text);
            assert_eq!(a.claim_type, b.claim_type);
            assert_eq!(a.confidence, b.confidence);
            assert_eq!(a.supporting_source_ids, b.supporting_source_ids);
            assert_eq!(a.conflicting_source_ids, b.conflicting_source_ids);
        }
    }
}
