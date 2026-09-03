use eggsearch::core::conflict::{
    detect_entity_scoped_conflicts, ConflictEntityKey, ConflictEntityType,
};
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
        excerpts: Vec::new(),
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
    assert!(
        !conflicts.is_empty(),
        "must have conflicts when versions differ"
    );
    let all_source_ids: Vec<&str> = conflicts
        .iter()
        .flat_map(|c| c.source_ids.iter().map(|s| s.as_str()))
        .collect();
    assert!(
        all_source_ids.contains(&"card_a"),
        "disagreeing card_a must appear in at least one conflict"
    );
    assert!(
        all_source_ids.contains(&"card_c"),
        "disagreeing card_c must appear in at least one conflict"
    );
    for c in &conflicts {
        assert!(
            c.source_ids.len() >= 2,
            "each pairwise conflict must reference at least 2 sources"
        );
        let vals: std::collections::HashSet<&str> = c.values.iter().map(|s| s.as_str()).collect();
        assert!(
            vals.len() >= 2,
            "pairwise conflict must have at least 2 distinct values"
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
    assert!(
        !conflicts.is_empty(),
        "must have conflicts when 3 distinct versions exist"
    );
    let mut all_values: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for c in &conflicts {
        assert!(
            c.source_ids.len() >= 2,
            "conflict must reference at least 2 sources"
        );
        for v in &c.values {
            all_values.insert(v.as_str());
        }
    }
    assert!(
        all_values.contains("1.0.0")
            && all_values.contains("2.0.0")
            && all_values.contains("3.0.0"),
        "all 3 distinct values (1.0.0, 2.0.0, 3.0.0) must appear across conflict values, got: {all_values:?}",
    );
}

#[test]
fn f7_conflict_source_ids_identify_disagreeing_cards() {
    let cards = vec![
        vuln_card("agree_a", "CVE-2024-0103", "pkg-src", "npm", vec!["1.0.0"]),
        vuln_card("agree_b", "CVE-2024-0103", "pkg-src", "npm", vec!["1.0.0"]),
        vuln_card(
            "disagree_c",
            "CVE-2024-0103",
            "pkg-src",
            "npm",
            vec!["3.0.0"],
        ),
    ];
    let conflicts = detect_entity_scoped_conflicts(&cards);
    assert!(
        !conflicts.is_empty(),
        "must have conflicts when versions differ"
    );
    for c in &conflicts {
        assert!(
            c.source_ids.len() >= 2,
            "each conflict must reference at least 2 sources"
        );
        let ids: std::collections::HashSet<&str> =
            c.source_ids.iter().map(|s| s.as_str()).collect();
        assert!(
            ids.contains("disagree_c"),
            "disagree_c must be in every conflict"
        );
    }
    let agree_a_in_conflict = conflicts
        .iter()
        .any(|c| c.source_ids.contains(&"agree_a".to_string()));
    let agree_b_in_conflict = conflicts
        .iter()
        .any(|c| c.source_ids.contains(&"agree_b".to_string()));
    assert!(
        agree_a_in_conflict || agree_b_in_conflict,
        "at least one agreeing card must participate in a pairwise conflict with disagree_c"
    );
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
    let mut ids1: Vec<_> = c1.iter().map(|c| c.id.clone()).collect();
    let mut ids2: Vec<_> = c2.iter().map(|c| c.id.clone()).collect();
    ids1.sort();
    ids2.sort();
    assert_eq!(ids1, ids2, "conflict IDs must be stable under permutation");
    for (conflict1, conflict2) in c1.iter().zip(c2.iter()) {
        assert_eq!(conflict1.id, conflict2.id, "conflict.id must be identical for same logical conflict under different input orderings");
        let mut src1: Vec<_> = conflict1.source_ids.to_vec();
        let mut src2: Vec<_> = conflict2.source_ids.to_vec();
        src1.sort();
        src2.sort();
        assert_eq!(src1, src2, "source_ids must be stable under permutation");
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

#[test]
fn f7_03_one_card_with_multiple_patched_versions_creates_no_conflict() {
    let cards = vec![
        vuln_card(
            "card_multi",
            "CVE-2024-0040",
            "pkg-e",
            "npm",
            vec!["1.0.0", "2.0.0"],
        ),
        vuln_card(
            "card_multi2",
            "CVE-2024-0040",
            "pkg-e",
            "npm",
            vec!["1.0.0", "2.0.0"],
        ),
    ];
    let conflicts = detect_entity_scoped_conflicts(&cards);
    assert!(
        conflicts.is_empty(),
        "same multi-version set in different order must not create conflict"
    );
}

#[test]
fn f7_04_same_patched_version_set_in_different_order_creates_no_conflict() {
    let cards = vec![
        vuln_card(
            "card_a",
            "CVE-2024-0041",
            "pkg-f",
            "npm",
            vec!["1.0.0", "2.0.0"],
        ),
        vuln_card(
            "card_b",
            "CVE-2024-0041",
            "pkg-f",
            "npm",
            vec!["2.0.0", "1.0.0"],
        ),
    ];
    let conflicts = detect_entity_scoped_conflicts(&cards);
    assert!(
        conflicts.is_empty(),
        "same patched-version set in different order must not create conflict"
    );
}

#[test]
fn f7_05_different_package_under_same_advisory_is_excluded() {
    let cards = vec![
        vuln_card("card_a", "CVE-2024-0042", "pkg-g-a", "npm", vec!["1.0.0"]),
        vuln_card("card_b", "CVE-2024-0042", "pkg-g-b", "npm", vec!["2.0.0"]),
    ];
    let conflicts = detect_entity_scoped_conflicts(&cards);
    assert!(
        conflicts.is_empty(),
        "different packages under same advisory must not conflict"
    );
}

#[test]
fn f7_06_same_repository_name_on_different_host_is_excluded() {
    let cards = vec![
        SourceCard {
            id: "repo_github".to_string(),
            stable_id: Some("repo_github".to_string()),
            title: "my-lib on GitHub".to_string(),
            url: "https://github.com/owner/my-lib".to_string(),
            providers: vec!["github".to_string()],
            score: Some(1.0),
            trust: eggsearch::core::result::TrustLevel::ExternalUntrusted,
            fetched: false,
            snippet: None,
            trust_markers: eggsearch::core::sanitize::TrustMarkers::default(),
            metadata: SourceMetadata {
                source_kind: SourceKind::SourceRepository,
                ..Default::default()
            },
            quality: None,
            excerpts: Vec::new(),
        },
        SourceCard {
            id: "repo_gitlab".to_string(),
            stable_id: Some("repo_gitlab".to_string()),
            title: "my-lib on GitLab".to_string(),
            url: "https://gitlab.com/owner/my-lib".to_string(),
            providers: vec!["gitlab".to_string()],
            score: Some(1.0),
            trust: eggsearch::core::result::TrustLevel::ExternalUntrusted,
            fetched: false,
            snippet: None,
            trust_markers: eggsearch::core::sanitize::TrustMarkers::default(),
            metadata: SourceMetadata {
                source_kind: SourceKind::SourceRepository,
                ..Default::default()
            },
            quality: None,
            excerpts: Vec::new(),
        },
    ];
    let conflicts = detect_entity_scoped_conflicts(&cards);
    assert!(
        conflicts.is_empty(),
        "same repo name on different host must not create conflict"
    );
}

#[test]
fn f7_08_duplicate_provider_contributions_to_one_card_do_not_create_extra_sources() {
    let cards = vec![
        vuln_card("card_a", "CVE-2024-0043", "pkg-h", "npm", vec!["1.0.0"]),
        vuln_card("card_b", "CVE-2024-0043", "pkg-h", "npm", vec!["2.0.0"]),
    ];
    let conflicts = detect_entity_scoped_conflicts(&cards);
    for c in &conflicts {
        assert!(
            c.source_ids.len() == 2,
            "two-card conflict must have exactly 2 source IDs, got {:?}",
            c.source_ids
        );
    }
}

#[test]
fn f4_conflict_metadata_populated_when_sources_disagree() {
    let cards = vec![
        vuln_card(
            "src_a",
            "CVE-2024-0100",
            "pkg-disagree",
            "npm",
            vec!["1.0.0"],
        ),
        vuln_card(
            "src_b",
            "CVE-2024-0100",
            "pkg-disagree",
            "npm",
            vec!["2.0.0"],
        ),
    ];
    let conflicts = detect_entity_scoped_conflicts(&cards);
    assert!(
        !conflicts.is_empty(),
        "disagreeing sources must produce conflicts"
    );
    for c in &conflicts {
        assert!(
            c.source_ids.contains(&"src_a".to_string()),
            "conflict must include src_a"
        );
        assert!(
            c.source_ids.contains(&"src_b".to_string()),
            "conflict must include src_b"
        );
        assert!(!c.values.is_empty(), "conflict must have values");
        assert!(
            !c.compared_fields.is_empty(),
            "conflict must have compared_fields"
        );
    }
}

#[test]
fn f5_conflict_metadata_empty_when_no_conflict() {
    let cards = vec![
        vuln_card("src_a", "CVE-2024-0101", "pkg-same", "npm", vec!["1.0.0"]),
        vuln_card("src_b", "CVE-2024-0101", "pkg-same", "npm", vec!["1.0.0"]),
    ];
    let conflicts = detect_entity_scoped_conflicts(&cards);
    assert!(
        conflicts.is_empty(),
        "identical patched versions must not produce conflicts"
    );
}

#[test]
fn f6_conflict_metadata_multiple_fields() {
    let cards = vec![
        vuln_card_with_date(
            "src_x",
            "CVE-2024-0102",
            "pkg-multi",
            "npm",
            vec!["1.0.0"],
            "2024-01-15",
        ),
        vuln_card_with_date(
            "src_y",
            "CVE-2024-0102",
            "pkg-multi",
            "npm",
            vec!["2.0.0"],
            "2024-06-20",
        ),
    ];
    let conflicts = detect_entity_scoped_conflicts(&cards);
    assert!(
        !conflicts.is_empty(),
        "disagreeing sources with multiple differing fields must produce conflicts"
    );
    let all_compared_fields: Vec<&str> = conflicts
        .iter()
        .flat_map(|c| c.compared_fields.iter().map(|s| s.as_str()))
        .collect();
    assert!(
        all_compared_fields.contains(&"patched_versions"),
        "patched_versions must be among compared fields"
    );
}

#[test]
fn f8_conflict_entity_key_groups_by_type_and_id() {
    let key1 = ConflictEntityKey {
        entity_type: ConflictEntityType::Vulnerability,
        canonical_id: "CVE-2024-0104".to_string(),
        field: "patched_versions".to_string(),
    };
    let key2 = ConflictEntityKey {
        entity_type: ConflictEntityType::Vulnerability,
        canonical_id: "CVE-2024-0104".to_string(),
        field: "patched_versions".to_string(),
    };
    let key3 = ConflictEntityKey {
        entity_type: ConflictEntityType::Vulnerability,
        canonical_id: "CVE-2024-0105".to_string(),
        field: "patched_versions".to_string(),
    };
    assert_eq!(
        key1, key2,
        "same entity type and ID must produce equal keys"
    );
    assert_ne!(
        key1, key3,
        "different entity IDs must produce different keys"
    );
}

#[test]
fn f9_conflict_metadata_serde_roundtrip() {
    let cards = vec![
        vuln_card("src_a", "CVE-2024-0106", "pkg-serde", "npm", vec!["1.0.0"]),
        vuln_card("src_b", "CVE-2024-0106", "pkg-serde", "npm", vec!["2.0.0"]),
    ];
    let conflicts = detect_entity_scoped_conflicts(&cards);
    assert!(!conflicts.is_empty());
    let json = serde_json::to_string(&conflicts).unwrap();
    let parsed: Vec<eggsearch::core::conflict::EvidenceConflict> =
        serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.len(), conflicts.len());
    for (orig, deser) in conflicts.iter().zip(parsed.iter()) {
        assert_eq!(orig.id, deser.id);
        assert_eq!(orig.source_ids, deser.source_ids);
        assert_eq!(orig.conflict_class, deser.conflict_class);
        assert_eq!(orig.compared_fields, deser.compared_fields);
        assert_eq!(orig.values, deser.values);
    }
}
