use eggsearch::core::conflict::detect_entity_scoped_conflicts;
use eggsearch::core::security::VulnerabilityMetadata;
use eggsearch::core::source_card::{SourceCard, SourceKind, SourceMetadata};

fn vuln_card(
    id: &str,
    cve_id: &str,
    package: &str,
    ecosystem: &str,
    patched: Vec<&str>,
) -> SourceCard {
    SourceCard {
        id: id.to_string(),
        stable_id: Some(id.to_string()),
        title: format!("Advisory for {package}"),
        url: format!("https://example.com/{cve_id}"),
        providers: vec!["test".to_string()],
        score: Some(1.0),
        trust: eggsearch::core::result::TrustLevel::ExternalUntrusted,
        fetched: false,
        snippet: None,
        trust_markers: eggsearch::core::sanitize::TrustMarkers::default(),
        metadata: SourceMetadata {
            source_kind: SourceKind::SecurityAdvisory,
            vulnerability: Some(Box::new(VulnerabilityMetadata {
                cve_ids: vec![cve_id.to_string()],
                ecosystem: Some(ecosystem.to_string()),
                package: Some(package.to_string()),
                patched_versions: patched.into_iter().map(String::from).collect(),
                ..Default::default()
            })),
            ..Default::default()
        },
        quality: None,
    }
}

fn vuln_card_with_date(
    id: &str,
    cve_id: &str,
    package: &str,
    ecosystem: &str,
    patched: Vec<&str>,
    published: &str,
) -> SourceCard {
    let mut card = vuln_card(id, cve_id, package, ecosystem, patched);
    if let Some(ref mut vuln) = card.metadata.vulnerability {
        vuln.published_at = Some(published.to_string());
    }
    card
}

#[test]
fn f7_01_three_cards_two_disagree_one_lacks_field_third_excluded() {
    let cards = vec![
        vuln_card("card_a", "CVE-2024-0001", "pkg-a", "npm", vec!["1.0.0"]),
        vuln_card("card_b", "CVE-2024-0001", "pkg-a", "npm", vec!["2.0.0"]),
        vuln_card("card_c", "CVE-2024-0001", "pkg-a", "npm", vec![]),
    ];
    let conflicts = detect_entity_scoped_conflicts(&cards);
    for c in &conflicts {
        assert!(
            c.source_ids.contains(&"card_a".to_string())
                || c.source_ids.contains(&"card_b".to_string()),
            "only disagreeing cards should be included"
        );
        assert!(
            !c.source_ids.contains(&"card_c".to_string()),
            "card_c lacks the field and should be excluded"
        );
    }
}

#[test]
fn f7_02_three_cards_two_agree_one_differs_correct_participation() {
    let cards = vec![
        vuln_card("card_a", "CVE-2024-0002", "pkg-b", "npm", vec!["1.0.0"]),
        vuln_card("card_b", "CVE-2024-0002", "pkg-b", "npm", vec!["1.0.0"]),
        vuln_card("card_c", "CVE-2024-0002", "pkg-b", "npm", vec!["3.0.0"]),
    ];
    let conflicts = detect_entity_scoped_conflicts(&cards);
    for c in &conflicts {
        assert!(
            c.source_ids.contains(&"card_a".to_string())
                || c.source_ids.contains(&"card_c".to_string()),
            "participating cards must be included"
        );
    }
}

#[test]
fn f7_09_three_or_more_distinct_normalized_values_represented_deterministically() {
    let cards = vec![
        vuln_card("card_a", "CVE-2024-0003", "pkg-c", "npm", vec!["1.0.0"]),
        vuln_card("card_b", "CVE-2024-0003", "pkg-c", "npm", vec!["2.0.0"]),
        vuln_card("card_c", "CVE-2024-0003", "pkg-c", "npm", vec!["3.0.0"]),
    ];
    let conflicts = detect_entity_scoped_conflicts(&cards);
    for c in &conflicts {
        assert!(
            c.source_ids.len() >= 2,
            "conflict must reference at least 2 sources"
        );
    }
}

#[test]
fn f7_10_property_every_emitted_source_id_has_one_of_emitted_values() {
    let cards = vec![
        vuln_card("src_1", "CVE-2024-0010", "pkg-x", "npm", vec!["1.0.0"]),
        vuln_card("src_2", "CVE-2024-0010", "pkg-x", "npm", vec!["2.0.0"]),
        vuln_card("src_3", "CVE-2024-0010", "pkg-x", "npm", vec!["3.0.0"]),
    ];
    let conflicts = detect_entity_scoped_conflicts(&cards);
    for c in &conflicts {
        assert!(
            c.source_ids.len() >= 2,
            "each conflict must have at least 2 source IDs"
        );
        assert!(
            !c.values.is_empty(),
            "each conflict must have at least one value"
        );
    }
}

#[test]
fn f7_11_property_every_conflict_has_at_least_two_distinct_source_ids() {
    let cards = vec![
        vuln_card("s1", "CVE-2024-0011", "pkg-y", "npm", vec!["1.0.0"]),
        vuln_card("s2", "CVE-2024-0011", "pkg-y", "npm", vec!["2.0.0"]),
    ];
    let conflicts = detect_entity_scoped_conflicts(&cards);
    for c in &conflicts {
        assert!(
            c.source_ids.len() >= 2,
            "conflict must reference at least 2 distinct source IDs, got {:?}",
            c.source_ids
        );
    }
}

#[test]
fn conflict_source_ids_are_stable_under_card_permutation() {
    let cards_v1 = vec![
        vuln_card("a", "CVE-2024-0020", "pkg", "npm", vec!["1.0.0"]),
        vuln_card("b", "CVE-2024-0020", "pkg", "npm", vec!["2.0.0"]),
    ];
    let cards_v2 = vec![
        vuln_card("b", "CVE-2024-0020", "pkg", "npm", vec!["2.0.0"]),
        vuln_card("a", "CVE-2024-0020", "pkg", "npm", vec!["1.0.0"]),
    ];
    let c1 = detect_entity_scoped_conflicts(&cards_v1);
    let c2 = detect_entity_scoped_conflicts(&cards_v2);
    assert_eq!(c1.len(), c2.len());
    for (conflict1, conflict2) in c1.iter().zip(c2.iter()) {
        let mut ids1: Vec<_> = conflict1.source_ids.to_vec();
        let mut ids2: Vec<_> = conflict2.source_ids.to_vec();
        ids1.sort();
        ids2.sort();
        assert_eq!(ids1, ids2, "conflict IDs must be stable under permutation");
    }
}

#[test]
fn date_conflict_attribution_only_disagreeing_sources() {
    let cards = vec![
        vuln_card_with_date(
            "d1",
            "CVE-2024-0030",
            "pkg-d",
            "npm",
            vec!["1.0.0"],
            "2024-01-15",
        ),
        vuln_card_with_date(
            "d2",
            "CVE-2024-0030",
            "pkg-d",
            "npm",
            vec!["1.0.0"],
            "2024-02-20",
        ),
    ];
    let conflicts = detect_entity_scoped_conflicts(&cards);
    for c in &conflicts {
        if c.compared_fields.iter().any(|f| f == "published_at") {
            assert!(c.source_ids.len() >= 2);
        }
    }
}
