//! Suggested fetch generation for `security_search` result groups.

use crate::core::security::{
    SecurityIdentifiers, SecurityResultGroup, SecurityResultGroupKind, SecuritySuggestedFetch,
};

/// Generate suggested fetches for security groups.
///
/// Suggests authoritative advisory URLs for identified vulnerability IDs,
/// ecosystem-specific package pages, and top results from each group.
pub fn generate_security_suggested_fetches(
    groups: &[SecurityResultGroup],
    resolved_ids: &SecurityIdentifiers,
    ecosystem: Option<&str>,
    package: Option<&str>,
) -> Vec<SecuritySuggestedFetch> {
    let mut fetches = Vec::new();

    // Always suggest OSV for any identified CVE/GHSA (priority 0)
    for cve_id in &resolved_ids.cve_ids {
        fetches.push(SecuritySuggestedFetch {
            url: format!("https://osv.dev/vulnerability/{cve_id}"),
            reason: format!("OSV entry for {cve_id}"),
            group: SecurityResultGroupKind::AuthoritativeAdvisories,
            priority: 0,
        });
    }
    for ghsa_id in &resolved_ids.ghsa_ids {
        fetches.push(SecuritySuggestedFetch {
            url: format!("https://github.com/advisories/{ghsa_id}"),
            reason: format!("GitHub Advisory entry for {ghsa_id}"),
            group: SecurityResultGroupKind::AuthoritativeAdvisories,
            priority: 0,
        });
    }
    for osv_id in &resolved_ids.osv_ids {
        fetches.push(SecuritySuggestedFetch {
            url: format!("https://osv.dev/vulnerability/{osv_id}"),
            reason: format!("OSV entry for {osv_id}"),
            group: SecurityResultGroupKind::AuthoritativeAdvisories,
            priority: 0,
        });
    }

    // If we have a package + ecosystem, suggest the ecosystem's security page (priority 1)
    if let (Some(pkg), Some(eco)) = (package, ecosystem) {
        match eco {
            "crates.io" => fetches.push(SecuritySuggestedFetch {
                url: format!("https://crates.io/crates/{pkg}"),
                reason: format!("{pkg} on crates.io (check security advisories tab)"),
                group: SecurityResultGroupKind::PackageAdvisories,
                priority: 1,
            }),
            "npm" => fetches.push(SecuritySuggestedFetch {
                url: format!("https://www.npmjs.com/package/{pkg}"),
                reason: format!("{pkg} on npm (check security advisories)"),
                group: SecurityResultGroupKind::PackageAdvisories,
                priority: 1,
            }),
            _ => {}
        }
    }

    // Add top results from each group (priority 2)
    for group in groups {
        for card in group.results.iter().take(2) {
            fetches.push(SecuritySuggestedFetch {
                url: card.url.clone(),
                reason: card.title.clone(),
                group: group.kind,
                priority: 2,
            });
        }
    }

    fetches
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::result::TrustLevel;
    use crate::core::source_card::{SourceCard, SourceKind};

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
        let fetches = generate_security_suggested_fetches(&[], &ids, None, None);
        assert!(fetches.iter().any(|f| f.url.contains("CVE-2024-0001")));
    }

    #[test]
    fn suggests_osv_for_ghsa_id() {
        let ids = SecurityIdentifiers {
            ghsa_ids: vec!["GHSA-test-1234-abcd".to_string()],
            ..Default::default()
        };
        let fetches = generate_security_suggested_fetches(&[], &ids, None, None);
        assert!(fetches
            .iter()
            .any(|f| f.url.contains("GHSA-test-1234-abcd")));
    }

    #[test]
    fn suggests_crates_io_for_package() {
        let ids = SecurityIdentifiers::default();
        let fetches =
            generate_security_suggested_fetches(&[], &ids, Some("crates.io"), Some("serde"));
        assert!(fetches
            .iter()
            .any(|f| f.url.contains("crates.io/crates/serde")));
    }

    #[test]
    fn suggests_npm_for_package() {
        let ids = SecurityIdentifiers::default();
        let fetches = generate_security_suggested_fetches(&[], &ids, Some("npm"), Some("lodash"));
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
        let fetches = generate_security_suggested_fetches(&[group], &ids, None, None);
        let group_fetches: Vec<_> = fetches.iter().filter(|f| f.priority == 2).collect();
        assert_eq!(group_fetches.len(), 2);
    }
}
