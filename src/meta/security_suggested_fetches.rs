//! Suggested fetch generation for `security_search` result groups.

use crate::core::fetch::ExtractMode;
use crate::core::security::{
    SecurityIdentifiers, SecurityResultGroup, SecurityResultGroupKind, SecuritySuggestedFetch,
};
use crate::core::security_applicability::DependencyFinding;
use crate::core::source_card::SourceKind;
use crate::meta::fetch_ranking::{extract_domain, FetchCandidate, FetchRankMode, RankContext};

/// Generate suggested fetches for security groups.
///
/// Suggests authoritative advisory URLs for identified vulnerability IDs,
/// ecosystem-specific package pages, top results from each group,
/// and dependency file locators when findings are available.
/// All candidates are scored and ranked via the fetch ranking pipeline
/// in `FetchRankMode::Security` before being returned.
pub fn generate_security_suggested_fetches(
    groups: &[SecurityResultGroup],
    resolved_ids: &SecurityIdentifiers,
    ecosystem: Option<&str>,
    package: Option<&str>,
    dependency_findings: &[DependencyFinding],
) -> Vec<SecuritySuggestedFetch> {
    let mut candidates = Vec::new();

    // ── Tier 1: Authoritative advisory URLs for identified CVE/GHSA/OSV ──

    for cve_id in &resolved_ids.cve_ids {
        candidates.push(FetchCandidate {
            url: format!("https://osv.dev/vulnerability/{cve_id}"),
            structured_repo_fetch: false,
            group: "AuthoritativeAdvisories".to_string(),
            expected_kind: SourceKind::SecurityAdvisory,
            recommended_extract_mode: None,
            original_order: candidates.len(),
            source_kind: SourceKind::SecurityAdvisory,
            source_role: None,
            evidence_confidence: None,
            is_pinned_permalink: false,
            is_raw_url: false,
            is_browser_url: true,
            domain: extract_domain("https://osv.dev"),
            score: 0,
            reasons: Vec::new(),
            information_gain: 0.0,
            stable: false,
            source_card_stable_id: None,
        });
    }
    for ghsa_id in &resolved_ids.ghsa_ids {
        candidates.push(FetchCandidate {
            url: format!("https://github.com/advisories/{ghsa_id}"),
            structured_repo_fetch: false,
            group: "AuthoritativeAdvisories".to_string(),
            expected_kind: SourceKind::SecurityAdvisory,
            recommended_extract_mode: None,
            original_order: candidates.len(),
            source_kind: SourceKind::SecurityAdvisory,
            source_role: None,
            evidence_confidence: None,
            is_pinned_permalink: false,
            is_raw_url: false,
            is_browser_url: true,
            domain: extract_domain("https://github.com"),
            score: 0,
            reasons: Vec::new(),
            information_gain: 0.0,
            stable: false,
            source_card_stable_id: None,
        });
    }
    for osv_id in &resolved_ids.osv_ids {
        candidates.push(FetchCandidate {
            url: format!("https://osv.dev/vulnerability/{osv_id}"),
            structured_repo_fetch: false,
            group: "AuthoritativeAdvisories".to_string(),
            expected_kind: SourceKind::SecurityAdvisory,
            recommended_extract_mode: None,
            original_order: candidates.len(),
            source_kind: SourceKind::SecurityAdvisory,
            source_role: None,
            evidence_confidence: None,
            is_pinned_permalink: false,
            is_raw_url: false,
            is_browser_url: true,
            domain: extract_domain("https://osv.dev"),
            score: 0,
            reasons: Vec::new(),
            information_gain: 0.0,
            stable: false,
            source_card_stable_id: None,
        });
    }

    // ── Tier 2: Ecosystem-specific package pages ──

    if let (Some(pkg), Some(eco)) = (package, ecosystem) {
        match eco {
            "crates.io" => candidates.push(FetchCandidate {
                url: format!("https://crates.io/crates/{pkg}"),
                structured_repo_fetch: false,
                group: "PackageAdvisories".to_string(),
                expected_kind: SourceKind::PackageRegistry,
                recommended_extract_mode: Some(ExtractMode::Markdown),
                original_order: candidates.len(),
                source_kind: SourceKind::PackageRegistry,
                source_role: None,
                evidence_confidence: None,
                is_pinned_permalink: false,
                is_raw_url: false,
                is_browser_url: true,
                domain: extract_domain("https://crates.io"),
                score: 0,
                reasons: Vec::new(),
                information_gain: 0.0,
                stable: false,
                source_card_stable_id: None,
            }),
            "npm" => candidates.push(FetchCandidate {
                url: format!("https://www.npmjs.com/package/{pkg}"),
                structured_repo_fetch: false,
                group: "PackageAdvisories".to_string(),
                expected_kind: SourceKind::PackageRegistry,
                recommended_extract_mode: Some(ExtractMode::Markdown),
                original_order: candidates.len(),
                source_kind: SourceKind::PackageRegistry,
                source_role: None,
                evidence_confidence: None,
                is_pinned_permalink: false,
                is_raw_url: false,
                is_browser_url: true,
                domain: extract_domain("https://www.npmjs.com"),
                score: 0,
                reasons: Vec::new(),
                information_gain: 0.0,
                stable: false,
                source_card_stable_id: None,
            }),
            _ => {}
        }
    }

    // ── Tier 3: Top results from each group ──

    for group in groups {
        for card in group.results.iter().take(2) {
            let source_kind = source_kind_for_group(group.kind);
            let (source_role, evidence_confidence) = card
                .metadata
                .code_evidence
                .as_ref()
                .map(|ce| (ce.source_role, ce.evidence_confidence))
                .unwrap_or((None, None));

            candidates.push(FetchCandidate {
                url: card.url.clone(),
                structured_repo_fetch: false,
                group: group_label(group.kind),
                expected_kind: source_kind,
                recommended_extract_mode: recommended_extract_mode_for_group(group.kind),
                original_order: candidates.len(),
                source_kind,
                source_role,
                evidence_confidence,
                is_pinned_permalink: false,
                is_raw_url: false,
                is_browser_url: card.url.starts_with("http"),
                domain: extract_domain(&card.url),
                score: 0,
                reasons: Vec::new(),
                information_gain: 0.0,
                stable: false,
                source_card_stable_id: card.stable_id.clone(),
            });
        }
    }

    // ── Tier 4: Dependency file locators (for findings with source files) ──

    for finding in dependency_findings.iter().take(4) {
        if let Some(ref source_file) = finding.source_file {
            // Construct a workspace:// pseudo-URL for local dependency findings
            let url = if source_file.starts_with('/') || source_file.contains('\\') {
                // Absolute path — use workspace pseudo-URL
                let root_name = std::path::Path::new(source_file)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("workspace");
                format!("workspace://{root_name}/{source_file}")
            } else {
                // Relative path — use as-is
                source_file.to_string()
            };

            candidates.push(FetchCandidate {
                url,
                structured_repo_fetch: true,
                group: "PackageAdvisories".to_string(),
                expected_kind: SourceKind::SourceFile,
                recommended_extract_mode: Some(ExtractMode::Text),
                original_order: candidates.len(),
                source_kind: SourceKind::SourceFile,
                source_role: None,
                evidence_confidence: finding.confidence.map(|c| match c {
                    crate::core::security_applicability::ApplicabilityConfidence::High => {
                        crate::core::code_evidence::EvidenceConfidence::Exact
                    }
                    crate::core::security_applicability::ApplicabilityConfidence::Medium => {
                        crate::core::code_evidence::EvidenceConfidence::Strong
                    }
                    crate::core::security_applicability::ApplicabilityConfidence::Low => {
                        crate::core::code_evidence::EvidenceConfidence::Weak
                    }
                }),
                is_pinned_permalink: false,
                is_raw_url: false,
                is_browser_url: false,
                domain: "workspace".to_string(),
                score: 0,
                reasons: vec![crate::meta::fetch_ranking::FetchRankReason::KindSourceFile],
                information_gain: 0.3,
                stable: false,
                source_card_stable_id: None,
            });
        }
    }

    // ── Rank and select ──

    let ctx = RankContext {
        mode: FetchRankMode::Security,
        ..Default::default()
    };

    let config = crate::meta::fetch_ranking::DiversityConfig {
        max_per_domain: 2,
        max_per_group: 2,
        total_cap: 8,
    };

    let ranked = crate::meta::fetch_ranking::rank_and_select(candidates, &ctx, &config);

    // ── Convert back to SecuritySuggestedFetch ──

    ranked
        .into_iter()
        .enumerate()
        .map(|(i, candidate)| {
            let group_kind = group_kind_from_label(&candidate.group);
            let reason = candidate
                .reasons
                .first()
                .map(|r| r.as_str().to_string())
                .unwrap_or_else(|| "suggested".to_string());

            let reason_code = match candidate.group.as_str() {
                "AuthoritativeAdvisories" => Some("primary_advisory".to_string()),
                "VendorAdvisories" => Some("vendor_guidance".to_string()),
                "PackageAdvisories" if candidate.url.starts_with("workspace://") => {
                    Some("dependency_context".to_string())
                }
                "PackageAdvisories" => Some("database_record".to_string()),
                "KevEntries" => Some("kev_context".to_string()),
                "PatchCommitsOrReleases" => Some("patch_evidence".to_string()),
                "DefensiveGuidance" => Some("defensive_guidance".to_string()),
                _ => None,
            };

            let mut advisory_ids: Vec<String> = Vec::new();
            for id in &resolved_ids.cve_ids {
                if !advisory_ids.contains(id) {
                    advisory_ids.push(id.clone());
                }
            }
            for id in &resolved_ids.ghsa_ids {
                if !advisory_ids.contains(id) {
                    advisory_ids.push(id.clone());
                }
            }
            for id in &resolved_ids.osv_ids {
                if !advisory_ids.contains(id) {
                    advisory_ids.push(id.clone());
                }
            }
            for id in &resolved_ids.rustsec_ids {
                if !advisory_ids.contains(id) {
                    advisory_ids.push(id.clone());
                }
            }

            SecuritySuggestedFetch {
                url: candidate.url,
                reason,
                group: group_kind,
                priority: i as u8,
                score: Some(candidate.score),
                rank_reasons: candidate
                    .reasons
                    .iter()
                    .map(|r| r.as_str().to_string())
                    .collect(),
                information_gain: Some(candidate.information_gain),
                stable_id: None,
                source_id: candidate.source_card_stable_id,
                reason_code,
                advisory_ids,
                package: package.map(String::from),
                version: None,
            }
        })
        .collect()
}

fn source_kind_for_group(kind: SecurityResultGroupKind) -> SourceKind {
    match kind {
        SecurityResultGroupKind::AuthoritativeAdvisories => SourceKind::SecurityAdvisory,
        SecurityResultGroupKind::VendorAdvisories => SourceKind::SecurityAdvisory,
        SecurityResultGroupKind::PackageAdvisories => SourceKind::PackageRegistry,
        SecurityResultGroupKind::KevEntries => SourceKind::SecurityAdvisory,
        SecurityResultGroupKind::PatchCommitsOrReleases => SourceKind::ReleaseNotes,
        SecurityResultGroupKind::ExploitDiscussion => SourceKind::IssueThread,
        SecurityResultGroupKind::DefensiveGuidance => SourceKind::OfficialDocs,
        SecurityResultGroupKind::GeneralContext => SourceKind::Reference,
        SecurityResultGroupKind::Other => SourceKind::Unknown,
    }
}

fn group_label(kind: SecurityResultGroupKind) -> String {
    match kind {
        SecurityResultGroupKind::AuthoritativeAdvisories => "AuthoritativeAdvisories".to_string(),
        SecurityResultGroupKind::VendorAdvisories => "VendorAdvisories".to_string(),
        SecurityResultGroupKind::PackageAdvisories => "PackageAdvisories".to_string(),
        SecurityResultGroupKind::KevEntries => "KevEntries".to_string(),
        SecurityResultGroupKind::PatchCommitsOrReleases => "PatchCommitsOrReleases".to_string(),
        SecurityResultGroupKind::ExploitDiscussion => "ExploitDiscussion".to_string(),
        SecurityResultGroupKind::DefensiveGuidance => "DefensiveGuidance".to_string(),
        SecurityResultGroupKind::GeneralContext => "GeneralContext".to_string(),
        SecurityResultGroupKind::Other => "Other".to_string(),
    }
}

fn group_kind_from_label(label: &str) -> SecurityResultGroupKind {
    match label {
        "AuthoritativeAdvisories" => SecurityResultGroupKind::AuthoritativeAdvisories,
        "VendorAdvisories" => SecurityResultGroupKind::VendorAdvisories,
        "PackageAdvisories" => SecurityResultGroupKind::PackageAdvisories,
        "KevEntries" => SecurityResultGroupKind::KevEntries,
        "PatchCommitsOrReleases" => SecurityResultGroupKind::PatchCommitsOrReleases,
        "ExploitDiscussion" => SecurityResultGroupKind::ExploitDiscussion,
        "DefensiveGuidance" => SecurityResultGroupKind::DefensiveGuidance,
        "GeneralContext" => SecurityResultGroupKind::GeneralContext,
        _ => SecurityResultGroupKind::Other,
    }
}

fn recommended_extract_mode_for_group(kind: SecurityResultGroupKind) -> Option<ExtractMode> {
    match kind {
        SecurityResultGroupKind::AuthoritativeAdvisories => Some(ExtractMode::Markdown),
        SecurityResultGroupKind::VendorAdvisories => Some(ExtractMode::Markdown),
        SecurityResultGroupKind::PackageAdvisories => Some(ExtractMode::Markdown),
        SecurityResultGroupKind::PatchCommitsOrReleases => Some(ExtractMode::Markdown),
        SecurityResultGroupKind::DefensiveGuidance => Some(ExtractMode::Markdown),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::code_evidence::{CodeEvidence, EvidenceConfidence, SourceRole};
    use crate::core::result::TrustLevel;
    use crate::core::source_card::SourceCard;

    fn make_card(title: &str, url: &str) -> SourceCard {
        let mut card = SourceCard::new(
            title,
            url,
            vec!["test".to_string()],
            None,
            TrustLevel::ExternalUntrusted,
        );
        card.metadata.source_kind = SourceKind::SecurityAdvisory;
        card
    }

    fn make_group(kind: SecurityResultGroupKind, cards: Vec<SourceCard>) -> SecurityResultGroup {
        SecurityResultGroup {
            kind,
            label: format!("{kind:?}"),
            results: cards,
            truncated: false,
            quality_summary: None,
        }
    }

    #[test]
    fn suggests_osv_for_cve_id() {
        let ids = SecurityIdentifiers {
            cve_ids: vec!["CVE-2024-0001".to_string()],
            ..Default::default()
        };
        let fetches = generate_security_suggested_fetches(&[], &ids, None, None, &[]);
        assert!(fetches.iter().any(|f| f.url.contains("CVE-2024-0001")));
    }

    #[test]
    fn suggests_osv_for_ghsa_id() {
        let ids = SecurityIdentifiers {
            ghsa_ids: vec!["GHSA-test-1234-abcd".to_string()],
            ..Default::default()
        };
        let fetches = generate_security_suggested_fetches(&[], &ids, None, None, &[]);
        assert!(fetches
            .iter()
            .any(|f| f.url.contains("GHSA-test-1234-abcd")));
    }

    #[test]
    fn suggests_crates_io_for_package() {
        let ids = SecurityIdentifiers::default();
        let fetches =
            generate_security_suggested_fetches(&[], &ids, Some("crates.io"), Some("serde"), &[]);
        assert!(fetches
            .iter()
            .any(|f| f.url.contains("crates.io/crates/serde")));
    }

    #[test]
    fn suggests_npm_for_package() {
        let ids = SecurityIdentifiers::default();
        let fetches =
            generate_security_suggested_fetches(&[], &ids, Some("npm"), Some("lodash"), &[]);
        assert!(fetches
            .iter()
            .any(|f| f.url.contains("npmjs.com/package/lodash")));
    }

    #[test]
    fn includes_top_results_from_groups() {
        let group = make_group(
            SecurityResultGroupKind::GeneralContext,
            vec![
                make_card("Article 1", "https://example.com/1"),
                make_card("Article 2", "https://example.com/2"),
                make_card("Article 3", "https://example.com/3"),
            ],
        );
        let ids = SecurityIdentifiers::default();
        let fetches = generate_security_suggested_fetches(&[group], &ids, None, None, &[]);
        // Should have 2 results (capped at 2 per group) after ranking
        let group_fetches: Vec<_> = fetches
            .iter()
            .filter(|f| f.group == SecurityResultGroupKind::GeneralContext)
            .collect();
        assert_eq!(group_fetches.len(), 2);
    }

    #[test]
    fn advisory_sources_outrank_community_discussion_in_security_mode() {
        let groups = [
            make_group(
                SecurityResultGroupKind::GeneralContext,
                vec![make_card(
                    "Blog post about CVE",
                    "https://blog.example.com/exploit-writeup",
                )],
            ),
            make_group(
                SecurityResultGroupKind::AuthoritativeAdvisories,
                vec![make_card(
                    "OSV Advisory",
                    "https://osv.dev/vulnerability/CVE-2024-0001",
                )],
            ),
        ];
        let ids = SecurityIdentifiers::default();
        let fetches = generate_security_suggested_fetches(
            &[groups[0].clone(), groups[1].clone()],
            &ids,
            None,
            None,
            &[],
        );

        // The authoritative advisory should rank higher than the blog post
        let advisory_pos = fetches
            .iter()
            .position(|f| f.url.contains("osv.dev"))
            .expect("advisory should be present");
        let blog_pos = fetches
            .iter()
            .position(|f| f.url.contains("blog.example.com"))
            .expect("blog should be present");
        assert!(
            advisory_pos < blog_pos,
            "advisory (pos {advisory_pos}) should outrank blog (pos {blog_pos})"
        );
    }

    #[test]
    fn score_and_rank_reasons_are_populated() {
        let ids = SecurityIdentifiers {
            cve_ids: vec!["CVE-2024-0001".to_string()],
            ..Default::default()
        };
        let fetches = generate_security_suggested_fetches(&[], &ids, None, None, &[]);
        assert!(!fetches.is_empty());
        let fetch = &fetches[0];
        assert!(fetch.score.is_some(), "score should be populated");
        assert!(
            !fetch.rank_reasons.is_empty(),
            "rank_reasons should be populated"
        );
    }

    #[test]
    fn reason_code_is_populated_for_all_group_types() {
        let groups = vec![
            make_group(
                SecurityResultGroupKind::AuthoritativeAdvisories,
                vec![make_card(
                    "Advisory",
                    "https://osv.dev/vulnerability/CVE-2024-0001",
                )],
            ),
            make_group(
                SecurityResultGroupKind::VendorAdvisories,
                vec![make_card(
                    "Vendor Advisory",
                    "https://example.com/vendor-advisory",
                )],
            ),
            make_group(
                SecurityResultGroupKind::PackageAdvisories,
                vec![make_card(
                    "Package Advisory",
                    "https://crates.io/crates/serde",
                )],
            ),
            make_group(
                SecurityResultGroupKind::KevEntries,
                vec![make_card(
                    "KEV Entry",
                    "https://www.cisa.gov/known-exploited-vulnerabilities",
                )],
            ),
            make_group(
                SecurityResultGroupKind::PatchCommitsOrReleases,
                vec![make_card(
                    "Patch Release",
                    "https://github.com/example/repo/releases/tag/v1.2.3",
                )],
            ),
            make_group(
                SecurityResultGroupKind::DefensiveGuidance,
                vec![make_card(
                    "Mitigation Guide",
                    "https://example.com/mitigation",
                )],
            ),
        ];
        let ids = SecurityIdentifiers {
            cve_ids: vec!["CVE-2024-0001".to_string()],
            ..Default::default()
        };
        let fetches = generate_security_suggested_fetches(&groups, &ids, None, None, &[]);

        for fetch in &fetches {
            let expected = match fetch.group {
                SecurityResultGroupKind::AuthoritativeAdvisories => Some("primary_advisory"),
                SecurityResultGroupKind::VendorAdvisories => Some("vendor_guidance"),
                SecurityResultGroupKind::PackageAdvisories => Some("database_record"),
                SecurityResultGroupKind::KevEntries => Some("kev_context"),
                SecurityResultGroupKind::PatchCommitsOrReleases => Some("patch_evidence"),
                SecurityResultGroupKind::DefensiveGuidance => Some("defensive_guidance"),
                _ => None,
            };
            assert_eq!(
                fetch.reason_code.as_deref(),
                expected,
                "reason_code mismatch for group {:?}: expected {:?}, got {:?}",
                fetch.group,
                expected,
                fetch.reason_code
            );
        }
    }

    #[test]
    fn dependency_context_reason_code_for_workspace_urls() {
        let finding = DependencyFinding {
            ecosystem: crate::core::package::PackageEcosystem::Npm,
            package: "qs".to_string(),
            version: Some("6.5.3".to_string()),
            source_file: Some("/app/package-lock.json".to_string()),
            source_line: Some(100),
            source_kind: crate::core::security_applicability::DependencySource::LockFile,
            confidence: Some(crate::core::security_applicability::ApplicabilityConfidence::Medium),
            relation: Some(crate::core::security_applicability::DependencyRelation::Transitive),
        };
        let ids = SecurityIdentifiers::default();
        let fetches = generate_security_suggested_fetches(&[], &ids, None, None, &[finding]);

        let dep_fetch = fetches
            .iter()
            .find(|f| f.url.contains("workspace://"))
            .expect("workspace dependency fetch should be present");
        assert_eq!(
            dep_fetch.reason_code.as_deref(),
            Some("dependency_context"),
            "workspace dependency fetch should have dependency_context reason_code"
        );
    }

    #[test]
    fn diversity_caps_work() {
        // Create many groups to test that diversity caps are applied
        let groups: Vec<SecurityResultGroup> = (0..10)
            .map(|i| {
                make_group(
                    SecurityResultGroupKind::GeneralContext,
                    vec![make_card(
                        &format!("Article {i}"),
                        &format!("https://same-domain.example.com/article-{i}"),
                    )],
                )
            })
            .collect();
        let ids = SecurityIdentifiers::default();
        let fetches = generate_security_suggested_fetches(&groups, &ids, None, None, &[]);
        // Total cap should limit results
        assert!(
            fetches.len() <= 8,
            "total cap should limit results to 8, got {}",
            fetches.len()
        );
        // Domain cap should limit same-domain results
        let same_domain = fetches
            .iter()
            .filter(|f| f.url.contains("same-domain.example.com"))
            .count();
        assert!(
            same_domain <= 2,
            "domain cap should limit same-domain results to 2, got {same_domain}"
        );
    }

    #[test]
    fn information_gain_is_populated() {
        let groups = vec![make_group(
            SecurityResultGroupKind::AuthoritativeAdvisories,
            vec![make_card(
                "Advisory",
                "https://osv.dev/vulnerability/CVE-2024-0001",
            )],
        )];
        let ids = SecurityIdentifiers::default();
        let fetches = generate_security_suggested_fetches(&groups, &ids, None, None, &[]);
        assert!(!fetches.is_empty());
        for fetch in &fetches {
            assert!(
                fetch.information_gain.is_some(),
                "information_gain should be populated"
            );
        }
    }

    #[test]
    fn code_evidence_source_role_propagates() {
        let mut card = make_card("Source", "https://github.com/example/advisory");
        card.metadata.source_kind = SourceKind::SecurityAdvisory;
        card.metadata.code_evidence = Some(CodeEvidence {
            source_role: Some(SourceRole::Documentation),
            evidence_confidence: Some(EvidenceConfidence::Strong),
            ..Default::default()
        });
        let group = make_group(SecurityResultGroupKind::AuthoritativeAdvisories, vec![card]);
        let ids = SecurityIdentifiers::default();
        let fetches = generate_security_suggested_fetches(&[group], &ids, None, None, &[]);
        // The advisory should be scored and ranked
        let advisory = fetches
            .iter()
            .find(|f| f.url.contains("github.com/example/advisory"))
            .expect("advisory should be present");
        assert!(
            advisory.score.unwrap() > 0,
            "advisory with code evidence should have positive score"
        );
    }

    #[test]
    fn empty_groups_still_generates_id_suggestions() {
        let ids = SecurityIdentifiers {
            cve_ids: vec!["CVE-2024-0001".to_string()],
            ghsa_ids: vec!["GHSA-test-1234-abcd".to_string()],
            osv_ids: vec!["GHSA-osv-5678-efgh".to_string()],
            ..Default::default()
        };
        let fetches = generate_security_suggested_fetches(&[], &ids, None, None, &[]);
        // All 3 are AuthoritativeAdvisories; diversity group cap limits to 2
        assert_eq!(fetches.len(), 2);
        assert!(fetches
            .iter()
            .all(|f| f.group == SecurityResultGroupKind::AuthoritativeAdvisories));
    }

    #[test]
    fn group_label_roundtrips() {
        let labels = [
            "AuthoritativeAdvisories",
            "VendorAdvisories",
            "PackageAdvisories",
            "KevEntries",
            "PatchCommitsOrReleases",
            "ExploitDiscussion",
            "DefensiveGuidance",
            "GeneralContext",
            "Other",
        ];
        for label in labels {
            let kind = group_kind_from_label(label);
            assert_eq!(group_label(kind), label);
        }
    }

    #[test]
    fn advisory_group_gets_security_advisory_source_kind() {
        assert_eq!(
            source_kind_for_group(SecurityResultGroupKind::AuthoritativeAdvisories),
            SourceKind::SecurityAdvisory
        );
        assert_eq!(
            source_kind_for_group(SecurityResultGroupKind::VendorAdvisories),
            SourceKind::SecurityAdvisory
        );
        assert_eq!(
            source_kind_for_group(SecurityResultGroupKind::PackageAdvisories),
            SourceKind::PackageRegistry
        );
        assert_eq!(
            source_kind_for_group(SecurityResultGroupKind::DefensiveGuidance),
            SourceKind::OfficialDocs
        );
        assert_eq!(
            source_kind_for_group(SecurityResultGroupKind::GeneralContext),
            SourceKind::Reference
        );
    }
}
