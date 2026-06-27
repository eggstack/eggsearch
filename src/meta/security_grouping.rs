//! Security result classification and grouping for `security_search`.
//!
//! Classifies `SourceCard` results into security-oriented groups
//! (advisories, patches, exploits, defensive guidance, etc.) and
//! produces grouped `SecurityResultGroup` bundles.

use crate::core::security::{SecurityResultGroup, SecurityResultGroupKind};
use crate::core::SourceCard;
use crate::core::SourceKind;

/// Classify a single source card into a security result group.
pub fn classify_security_result(card: &SourceCard) -> SecurityResultGroupKind {
    let url_lower = card.url.to_lowercase();

    // Authoritative advisory databases
    if url_lower.contains("osv.dev") || url_lower.contains("nvd.nist.gov") {
        return SecurityResultGroupKind::AuthoritativeAdvisories;
    }
    if url_lower.contains("github.com/advisories") || url_lower.contains("ghsa") {
        return SecurityResultGroupKind::AuthoritativeAdvisories;
    }
    if url_lower.contains("rustsec.org") || url_lower.contains("rust advisory") {
        return SecurityResultGroupKind::AuthoritativeAdvisories;
    }

    // Vendor advisories
    if url_lower.contains("advisory") || url_lower.contains("/security/advisories") {
        return SecurityResultGroupKind::VendorAdvisories;
    }
    if card.metadata.source_kind == SourceKind::SecurityAdvisory {
        return SecurityResultGroupKind::VendorAdvisories;
    }

    // Patch commits or releases
    if url_lower.contains("/commit/") || url_lower.contains("/pull/") {
        return SecurityResultGroupKind::PatchCommitsOrReleases;
    }
    if url_lower.contains("release") || url_lower.contains("changelog") {
        return SecurityResultGroupKind::PatchCommitsOrReleases;
    }

    // Exploit discussion
    if url_lower.contains("exploit")
        || url_lower.contains("poc")
        || url_lower.contains("proof-of-concept")
        || url_lower.contains("metasploit")
    {
        return SecurityResultGroupKind::ExploitDiscussion;
    }

    // Defensive guidance
    if url_lower.contains("mitigation")
        || url_lower.contains("hardening")
        || url_lower.contains("defensive")
        || url_lower.contains("best-practice")
    {
        return SecurityResultGroupKind::DefensiveGuidance;
    }

    // Package advisories (issue-like on package registries)
    if url_lower.contains("/issues/")
        && (url_lower.contains("github.com") || url_lower.contains("gitlab.com"))
    {
        return SecurityResultGroupKind::PackageAdvisories;
    }

    // General context for other security-relevant results
    SecurityResultGroupKind::GeneralContext
}

/// Group source cards into security result groups.
pub fn group_security_results(
    results: &[SourceCard],
    max_per_group: Option<usize>,
) -> Vec<SecurityResultGroup> {
    let mut groups: Vec<SecurityResultGroup> = Vec::new();

    for card in results {
        let kind = classify_security_result(card);

        // Find or create group
        let group = groups.iter_mut().find(|g| g.kind == kind);
        if let Some(group) = group {
            let at_limit = max_per_group.is_some_and(|cap| group.results.len() >= cap);
            if !at_limit {
                group.results.push(card.clone());
            } else {
                group.truncated = true;
            }
        } else {
            groups.push(SecurityResultGroup {
                kind,
                label: security_group_label(kind),
                results: vec![card.clone()],
                truncated: false,
            });
        }
    }

    // Sort groups by kind for deterministic output
    groups.sort_by_key(|g| format!("{:?}", g.kind));
    groups
}

/// Map a security result group kind to a human-readable label.
pub fn security_group_label(kind: SecurityResultGroupKind) -> String {
    match kind {
        SecurityResultGroupKind::AuthoritativeAdvisories => "Authoritative Advisories".to_string(),
        SecurityResultGroupKind::VendorAdvisories => "Vendor Advisories".to_string(),
        SecurityResultGroupKind::PackageAdvisories => "Package Advisories".to_string(),
        SecurityResultGroupKind::KevEntries => "Known Exploited Vulnerabilities".to_string(),
        SecurityResultGroupKind::PatchCommitsOrReleases => "Patches & Fixes".to_string(),
        SecurityResultGroupKind::ExploitDiscussion => "Exploit Discussion".to_string(),
        SecurityResultGroupKind::DefensiveGuidance => "Defensive Guidance".to_string(),
        SecurityResultGroupKind::GeneralContext => "General Context".to_string(),
        SecurityResultGroupKind::Other => "Other".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::result::TrustLevel;

    fn make_card(source_kind: SourceKind, url: &str) -> SourceCard {
        let mut card = SourceCard::new(
            "Test",
            url,
            vec!["test".to_string()],
            None,
            TrustLevel::ExternalUntrusted,
        );
        card.metadata.source_kind = source_kind;
        card
    }

    #[test]
    fn classify_osv_as_authoritative() {
        let card = make_card(
            SourceKind::SecurityAdvisory,
            "https://osv.dev/vulnerability/GHSA-test",
        );
        assert_eq!(
            classify_security_result(&card),
            SecurityResultGroupKind::AuthoritativeAdvisories
        );
    }

    #[test]
    fn classify_nvd_as_authoritative() {
        let card = make_card(
            SourceKind::SecurityAdvisory,
            "https://nvd.nist.gov/vuln/detail/CVE-2024-0001",
        );
        assert_eq!(
            classify_security_result(&card),
            SecurityResultGroupKind::AuthoritativeAdvisories
        );
    }

    #[test]
    fn classify_github_advisory_as_authoritative() {
        let card = make_card(
            SourceKind::SecurityAdvisory,
            "https://github.com/advisories/GHSA-test",
        );
        assert_eq!(
            classify_security_result(&card),
            SecurityResultGroupKind::AuthoritativeAdvisories
        );
    }

    #[test]
    fn classify_advisory_as_vendor() {
        let card = make_card(
            SourceKind::Reference,
            "https://example.com/security/advisory",
        );
        assert_eq!(
            classify_security_result(&card),
            SecurityResultGroupKind::VendorAdvisories
        );
    }

    #[test]
    fn classify_commit_as_patch() {
        let card = make_card(
            SourceKind::Reference,
            "https://github.com/foo/bar/commit/abc123",
        );
        assert_eq!(
            classify_security_result(&card),
            SecurityResultGroupKind::PatchCommitsOrReleases
        );
    }

    #[test]
    fn classify_exploit_url() {
        let card = make_card(SourceKind::Reference, "https://example.com/exploit/poc");
        assert_eq!(
            classify_security_result(&card),
            SecurityResultGroupKind::ExploitDiscussion
        );
    }

    #[test]
    fn classify_mitigation_as_defensive() {
        let card = make_card(
            SourceKind::Reference,
            "https://example.com/mitigation-guide",
        );
        assert_eq!(
            classify_security_result(&card),
            SecurityResultGroupKind::DefensiveGuidance
        );
    }

    #[test]
    fn classify_github_issues_as_package_advisory() {
        let card = make_card(
            SourceKind::IssueThread,
            "https://github.com/foo/bar/issues/123",
        );
        assert_eq!(
            classify_security_result(&card),
            SecurityResultGroupKind::PackageAdvisories
        );
    }

    #[test]
    fn classify_unknown_as_general_context() {
        let card = make_card(SourceKind::Unknown, "https://example.com/some-article");
        assert_eq!(
            classify_security_result(&card),
            SecurityResultGroupKind::GeneralContext
        );
    }

    #[test]
    fn group_results_basic() {
        let cards = vec![
            make_card(
                SourceKind::SecurityAdvisory,
                "https://osv.dev/vulnerability/X",
            ),
            make_card(SourceKind::Reference, "https://example.com/exploit/poc"),
            make_card(SourceKind::Reference, "https://example.com/mitigation"),
        ];
        let groups = group_security_results(&cards, None);
        assert!(groups.len() >= 3);
    }

    #[test]
    fn group_results_max_per_group() {
        let cards: Vec<SourceCard> = (0..5)
            .map(|i| {
                make_card(
                    SourceKind::SecurityAdvisory,
                    &format!("https://osv.dev/vuln/{i}"),
                )
            })
            .collect();
        let groups = group_security_results(&cards, Some(2));
        let auth_group = groups
            .iter()
            .find(|g| g.kind == SecurityResultGroupKind::AuthoritativeAdvisories)
            .unwrap();
        assert_eq!(auth_group.results.len(), 2);
        assert!(auth_group.truncated);
    }

    #[test]
    fn group_label_map() {
        assert_eq!(
            security_group_label(SecurityResultGroupKind::AuthoritativeAdvisories),
            "Authoritative Advisories"
        );
        assert_eq!(
            security_group_label(SecurityResultGroupKind::ExploitDiscussion),
            "Exploit Discussion"
        );
        assert_eq!(
            security_group_label(SecurityResultGroupKind::Other),
            "Other"
        );
    }
}
