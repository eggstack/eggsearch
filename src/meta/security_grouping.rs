//! Security result classification and grouping for `security_search`.
//!
//! Classifies `SourceCard` results into security-oriented groups
//! (advisories, patches, exploits, defensive guidance, etc.) and
//! produces grouped `SecurityResultGroup` bundles.

use crate::core::security::{SecurityResultGroup, SecurityResultGroupKind};
use crate::core::SourceCard;
use crate::core::SourceKind;
use crate::meta::grouping::{build_card_groups, BuiltGroup};

const CANONICAL_GROUP_ORDER: &[SecurityResultGroupKind] = &[
    SecurityResultGroupKind::AuthoritativeAdvisories,
    SecurityResultGroupKind::VendorAdvisories,
    SecurityResultGroupKind::PackageAdvisories,
    SecurityResultGroupKind::KevEntries,
    SecurityResultGroupKind::PatchCommitsOrReleases,
    SecurityResultGroupKind::ExploitDiscussion,
    SecurityResultGroupKind::DefensiveGuidance,
    SecurityResultGroupKind::GeneralContext,
    SecurityResultGroupKind::Other,
];

/// Classify a single source card into a security result group.
pub fn classify_security_result(card: &SourceCard) -> SecurityResultGroupKind {
    let url = card.url.as_str();
    let url_lower = url.to_ascii_lowercase();
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
        || url_contains_token_ci(url, "poc")
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

fn url_contains_token_ci(url: &str, token: &str) -> bool {
    url.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|part| part.eq_ignore_ascii_case(token))
}

/// Group source cards into security result groups.
pub fn group_security_results(
    results: &[SourceCard],
    max_per_group: Option<usize>,
) -> Vec<SecurityResultGroup> {
    let max_per_group = max_per_group.unwrap_or(usize::MAX);
    build_card_groups(
        results.to_vec(),
        classify_security_result,
        CANONICAL_GROUP_ORDER,
        security_group_label,
        max_per_group,
        None,
        |_, _| {},
    )
    .into_iter()
    .map(into_security_group)
    .collect()
}

/// Map a security result group kind to a human-readable label.
pub fn security_group_label(kind: SecurityResultGroupKind) -> String {
    match kind {
        SecurityResultGroupKind::AuthoritativeAdvisories => "Authoritative Advisories",
        SecurityResultGroupKind::VendorAdvisories => "Vendor Advisories",
        SecurityResultGroupKind::PackageAdvisories => "Package Advisories",
        SecurityResultGroupKind::KevEntries => "Known Exploited Vulnerabilities",
        SecurityResultGroupKind::PatchCommitsOrReleases => "Patches & Fixes",
        SecurityResultGroupKind::ExploitDiscussion => "Exploit Discussion",
        SecurityResultGroupKind::DefensiveGuidance => "Defensive Guidance",
        SecurityResultGroupKind::GeneralContext => "General Context",
        SecurityResultGroupKind::Other => "Other",
    }
    .to_string()
}

fn into_security_group(group: BuiltGroup<SecurityResultGroupKind>) -> SecurityResultGroup {
    SecurityResultGroup {
        kind: group.kind,
        label: group.label,
        results: group.results,
        truncated: group.truncated,
        quality_summary: Some(group.quality_summary),
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
    fn classify_poc_only_as_token() {
        let card = make_card(SourceKind::Reference, "https://example.com/pocket-guide");
        assert_eq!(
            classify_security_result(&card),
            SecurityResultGroupKind::GeneralContext
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
    fn group_results_orders_primary_evidence_before_context() {
        let cards = vec![
            make_card(SourceKind::Unknown, "https://example.com/context"),
            make_card(
                SourceKind::Reference,
                "https://vendor.example.com/security/advisory",
            ),
            make_card(
                SourceKind::Reference,
                "https://github.com/foo/bar/commit/abc",
            ),
            make_card(SourceKind::Reference, "https://example.com/exploit/poc"),
            make_card(SourceKind::Reference, "https://example.com/mitigation"),
        ];
        let groups = group_security_results(&cards, None);
        let kinds: Vec<_> = groups.iter().map(|g| g.kind).collect();
        assert_eq!(
            kinds,
            vec![
                SecurityResultGroupKind::VendorAdvisories,
                SecurityResultGroupKind::PatchCommitsOrReleases,
                SecurityResultGroupKind::ExploitDiscussion,
                SecurityResultGroupKind::DefensiveGuidance,
                SecurityResultGroupKind::GeneralContext,
            ]
        );
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
    fn group_results_zero_max_per_group_yields_empty_truncated_group() {
        let cards = vec![make_card(
            SourceKind::SecurityAdvisory,
            "https://osv.dev/vuln/one",
        )];
        let groups = group_security_results(&cards, Some(0));
        let auth_group = groups
            .iter()
            .find(|g| g.kind == SecurityResultGroupKind::AuthoritativeAdvisories)
            .unwrap();
        assert!(auth_group.results.is_empty());
        assert!(auth_group.truncated);
        assert!(auth_group.quality_summary.is_some());
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
