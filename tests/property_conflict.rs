use eggsearch::core::conflict::{
    detect_benchmark_conflicts, detect_date_conflicts, detect_entity_scoped_conflicts,
    detect_mutable_vs_pinned, detect_provider_metadata_conflicts, detect_version_range_conflicts,
    extract_entity_key,
};
use eggsearch::core::evidence_postprocess::detect_structured_conflicts;
use eggsearch::core::result::TrustLevel;
use eggsearch::core::security::{SeverityLevel, VulnerabilitySource};
use eggsearch::core::source_card::SourceCard;
use proptest::prelude::*;

fn source_id_strategy() -> impl Strategy<Value = String> {
    "[a-z_]{3,15}"
}

fn field_strategy() -> impl Strategy<Value = String> {
    "[a-z_]{3,20}"
}

fn value_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 ._-]{1,50}"
}

fn card_strategy() -> impl Strategy<Value = SourceCard> {
    (
        "[a-z_]{3,15}",
        "https://[a-z]+\\.com/[a-z0-9/_.-]{1,30}",
        "[a-zA-Z0-9 ]{5,30}",
    )
        .prop_map(|(provider, url, title)| {
            SourceCard::new(
                &title,
                &url,
                vec![provider],
                Some(0.5),
                TrustLevel::ExternalUntrusted,
            )
        })
}

fn card_with_stable_id_and_vuln_strategy() -> impl Strategy<Value = SourceCard> {
    (
        "[a-z_]{3,15}",
        "https://[a-z]+\\.com/[a-z0-9/_.-]{1,30}",
        "[a-zA-Z0-9 ]{5,30}",
        "CVE-[0-9]{4}-[0-9]{4,8}",
        ">=?\\s*[0-9]+\\.[0-9]+\\s*(<\\s*[0-9]+\\.[0-9]+)?",
        "[0-9]{4}-[0-9]{2}-[0-9]{2}",
    )
        .prop_map(|(provider, url, title, cve, patched, published)| {
            let id_hash = crate_id_hash(&provider, &url, &title);
            let mut card = SourceCard::new(
                &title,
                &url,
                vec![provider],
                Some(0.5),
                TrustLevel::ExternalUntrusted,
            );
            card.stable_id = Some(format!("src_{:016x}", id_hash));
            card.metadata.vulnerability =
                Some(Box::new(eggsearch::core::security::VulnerabilityMetadata {
                    cve_ids: vec![cve],
                    ghsa_ids: vec![],
                    osv_ids: vec![],
                    patched_versions: vec![patched],
                    severity: Some(SeverityLevel::Medium),
                    published_at: Some(published),
                    source: VulnerabilitySource::Osv,
                    ..Default::default()
                }));
            card
        })
}

fn crate_id_hash(provider: &str, url: &str, title: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    provider.hash(&mut h);
    url.hash(&mut h);
    title.hash(&mut h);
    h.finish()
}

proptest! {
    #[test]
    fn detect_version_range_conflicts_deterministic(
        a in source_id_strategy(),
        b in source_id_strategy(),
        field in field_strategy(),
        val_a in value_strategy(),
        val_b in value_strategy(),
    ) {
        let ids_a = vec![a];
        let ids_b = vec![b];
        let c1 = detect_version_range_conflicts(&ids_a, &ids_b, &field, &val_a, &val_b);
        let c2 = detect_version_range_conflicts(&ids_a, &ids_b, &field, &val_a, &val_b);
        prop_assert_eq!(c1.as_ref().map(|c| &c.id), c2.as_ref().map(|c| &c.id),
            "conflict ID must be deterministic");
    }

    #[test]
    fn detect_date_conflicts_deterministic(
        a in source_id_strategy(),
        b in source_id_strategy(),
        field in field_strategy(),
        val_a in value_strategy(),
        val_b in value_strategy(),
    ) {
        let ids_a = vec![a];
        let ids_b = vec![b];
        let c1 = detect_date_conflicts(&ids_a, &ids_b, &field, &val_a, &val_b);
        let c2 = detect_date_conflicts(&ids_a, &ids_b, &field, &val_a, &val_b);
        prop_assert_eq!(c1.as_ref().map(|c| &c.id), c2.as_ref().map(|c| &c.id),
            "conflict ID must be deterministic");
    }

    #[test]
    fn detect_provider_metadata_conflicts_deterministic(
        a in source_id_strategy(),
        b in source_id_strategy(),
        field in field_strategy(),
        val_a in value_strategy(),
        val_b in value_strategy(),
    ) {
        let ids_a = vec![a];
        let ids_b = vec![b];
        let c1 = detect_provider_metadata_conflicts(&ids_a, &ids_b, &field, &val_a, &val_b);
        let c2 = detect_provider_metadata_conflicts(&ids_a, &ids_b, &field, &val_a, &val_b);
        prop_assert_eq!(c1.as_ref().map(|c| &c.id), c2.as_ref().map(|c| &c.id),
            "conflict ID must be deterministic");
    }

    #[test]
    fn detect_benchmark_conflicts_deterministic(
        a in source_id_strategy(),
        b in source_id_strategy(),
        name in field_strategy(),
        val_a in value_strategy(),
        val_b in value_strategy(),
    ) {
        let ids_a = vec![a];
        let ids_b = vec![b];
        let c1 = detect_benchmark_conflicts(&ids_a, &ids_b, &name, &val_a, &val_b);
        let c2 = detect_benchmark_conflicts(&ids_a, &ids_b, &name, &val_a, &val_b);
        prop_assert_eq!(c1.as_ref().map(|c| &c.id), c2.as_ref().map(|c| &c.id),
            "conflict ID must be deterministic");
    }

    #[test]
    fn no_conflict_when_values_equal(
        a in source_id_strategy(),
        b in source_id_strategy(),
        field in field_strategy(),
        val in value_strategy(),
    ) {
        let ids_a = vec![a];
        let ids_b = vec![b];
        prop_assert!(detect_version_range_conflicts(&ids_a, &ids_b, &field, &val, &val).is_none());
        prop_assert!(detect_date_conflicts(&ids_a, &ids_b, &field, &val, &val).is_none());
        prop_assert!(detect_provider_metadata_conflicts(&ids_a, &ids_b, &field, &val, &val).is_none());
        prop_assert!(detect_benchmark_conflicts(&ids_a, &ids_b, &field, &val, &val).is_none());
    }

    #[test]
    fn conflict_when_values_differ(
        a in source_id_strategy(),
        b in source_id_strategy(),
        field in field_strategy(),
    ) {
        let ids_a = vec![a];
        let ids_b = vec![b];
        prop_assert!(detect_version_range_conflicts(&ids_a, &ids_b, &field, "1.0", "2.0").is_some());
        prop_assert!(detect_date_conflicts(&ids_a, &ids_b, &field, "2024-01-01", "2024-06-15").is_some());
        prop_assert!(detect_provider_metadata_conflicts(&ids_a, &ids_b, &field, "MIT", "Apache-2.0").is_some());
        prop_assert!(detect_benchmark_conflicts(&ids_a, &ids_b, &field, "1500", "2200").is_some());
    }

    #[test]
    fn source_ids_sorted_and_deduped(
        ids_a in proptest::collection::vec("[a-z_]{3,15}", 1..5),
        ids_b in proptest::collection::vec("[a-z_]{3,15}", 1..5),
        field in field_strategy(),
    ) {
        let val_a = "1.0";
        let val_b = "2.0";
        if let Some(conflict) = detect_version_range_conflicts(&ids_a, &ids_b, &field, val_a, val_b) {
            let mut expected: Vec<String> = ids_a.into_iter().chain(ids_b).collect();
            expected.sort();
            expected.dedup();
            prop_assert_eq!(conflict.source_ids, expected);
        }
    }

    #[test]
    fn single_source_no_cross_source_conflict(
        card in card_with_stable_id_and_vuln_strategy(),
    ) {
        let conflicts = detect_entity_scoped_conflicts(&[card]);
        prop_assert!(conflicts.is_empty(),
            "single source card should not produce cross-source conflicts");
    }

    #[test]
    fn empty_cards_produce_no_conflicts(
        cards in proptest::collection::vec(card_strategy(), 0..10),
    ) {
        let conflicts = detect_entity_scoped_conflicts(&cards);
        let single_source_groups: usize = cards.iter()
            .filter(|c| c.metadata.vulnerability.is_some() || c.metadata.code_evidence.is_some())
            .count();
        if single_source_groups < 2 {
            prop_assert!(conflicts.is_empty(),
                "cards without matching entity keys should produce no conflicts");
        }
    }

    #[test]
    fn detect_entity_scoped_conflicts_order_independent(
        card1 in card_with_stable_id_and_vuln_strategy(),
        card2 in card_with_stable_id_and_vuln_strategy(),
    ) {
        let conflicts_ab = detect_entity_scoped_conflicts(&[card1.clone(), card2.clone()]);
        let conflicts_ba = detect_entity_scoped_conflicts(&[card2, card1]);

        prop_assert_eq!(conflicts_ab.len(), conflicts_ba.len(),
            "conflict count must be order-independent");

        let mut ids_ab: Vec<String> = conflicts_ab.iter().map(|c| c.id.clone()).collect();
        let mut ids_ba: Vec<String> = conflicts_ba.iter().map(|c| c.id.clone()).collect();
        ids_ab.sort();
        ids_ba.sort();
        prop_assert_eq!(ids_ab, ids_ba,
            "conflict IDs must be order-independent");
    }

    #[test]
    fn extract_entity_key_deterministic(
        card in card_strategy(),
    ) {
        let key1 = extract_entity_key(&card);
        let key2 = extract_entity_key(&card);
        prop_assert_eq!(key1, key2);
    }

    #[test]
    fn compute_conflict_id_deterministic(
        a in source_id_strategy(),
        b in source_id_strategy(),
        field in field_strategy(),
    ) {
        let mut all_ids = vec![a, b];
        all_ids.sort();
        all_ids.dedup();
        let c1 = detect_version_range_conflicts(&all_ids, &[], &field, "1.0", "2.0");
        let c2 = detect_version_range_conflicts(&all_ids, &[], &field, "1.0", "2.0");
        if let (Some(c1), Some(c2)) = (c1, c2) {
            prop_assert_eq!(c1.id, c2.id,
                "conflict ID with same inputs must be deterministic");
        }
    }

    #[test]
    fn different_fields_produce_different_ids(
        a in source_id_strategy(),
        b in source_id_strategy(),
    ) {
        let ids_a = vec![a];
        let ids_b = vec![b];
        let c1 = detect_version_range_conflicts(&ids_a, &ids_b, "field_a", "1.0", "2.0");
        let c2 = detect_version_range_conflicts(&ids_a, &ids_b, "field_b", "1.0", "2.0");
        if let (Some(c1), Some(c2)) = (c1, c2) {
            prop_assert_ne!(c1.id, c2.id,
                "different fields must produce different IDs");
        }
    }

    #[test]
    fn mutable_vs_pinned_conflict_requires_both_sides(
        m in proptest::collection::vec("[a-z_]{3,15}", 0..5),
        p in proptest::collection::vec("[a-z_]{3,15}", 0..5),
    ) {
        let result = detect_mutable_vs_pinned(&m, &p);
        if m.is_empty() || p.is_empty() {
            prop_assert!(result.is_none(),
                "empty mutable or pinned list should produce no conflict");
        } else {
            prop_assert!(result.is_some(),
                "non-empty mutable and pinned should produce a conflict");
        }
    }

    #[test]
    fn unrelated_repositories_do_not_produce_mutable_vs_pinned_conflict(
        repo_a in "[a-z]{3,10}",
        repo_b in "[a-z]{3,10}",
    ) {
        prop_assume!(repo_a != repo_b);
        let mut card_a = SourceCard::new(
            "implementation from org-a",
            format!("https://github.com/org-a/{repo_a}/blob/main/src/lib.rs"),
            vec!["github_code".to_string()],
            Some(0.8),
            TrustLevel::ExternalUntrusted,
        );
        card_a.metadata.code_evidence = Some(
            eggsearch::core::code_evidence::CodeEvidence {
                owner: Some("org-a".to_string()),
                repo: Some(repo_a.clone()),
                ref_name: Some("main".to_string()),
                path: Some("src/lib.rs".to_string()),
                commit_sha: Some("abc123def456".to_string()),
                language: Some("rust".to_string()),
                ..Default::default()
            },
        );

        let mut card_b = SourceCard::new(
            "implementation from org-b",
            format!("https://github.com/org-b/{repo_b}/blob/main/src/main.rs"),
            vec!["github_code".to_string()],
            Some(0.8),
            TrustLevel::ExternalUntrusted,
        );
        card_b.metadata.code_evidence = Some(
            eggsearch::core::code_evidence::CodeEvidence {
                owner: Some("org-b".to_string()),
                repo: Some(repo_b.clone()),
                ref_name: Some("main".to_string()),
                path: Some("src/main.rs".to_string()),
                commit_sha: None,
                language: Some("rust".to_string()),
                ..Default::default()
            },
        );

        let conflicts = detect_structured_conflicts(&[card_a, card_b]);
        let mutable_vs_pinned: Vec<_> = conflicts.iter()
            .filter(|c| matches!(c.conflict_class, eggsearch::core::conflict::ConflictClass::MutableVsCommitPinnedContent))
            .collect();
        prop_assert!(mutable_vs_pinned.is_empty(),
            "mutable-vs-pinned conflict must NOT be reported for cards from unrelated repositories (org-a/{repo_a} vs org-b/{repo_b}). Cross-entity conflicts produce false positives.");
    }

    #[test]
    fn no_cross_entity_conflict(
        cards in proptest::collection::vec(card_with_stable_id_and_vuln_strategy(), 0..10),
    ) {
        let conflicts = detect_entity_scoped_conflicts(&cards);
        for conflict in &conflicts {
            prop_assert!(!conflict.source_ids.is_empty(),
                "every conflict must have at least one source ID");
        }
    }

    #[test]
    fn every_source_id_corresponds_to_emitted_value(
        cards in proptest::collection::vec(card_with_stable_id_and_vuln_strategy(), 2..8),
    ) {
        let conflicts = detect_entity_scoped_conflicts(&cards);
        for conflict in &conflicts {
            prop_assert!(!conflict.values.is_empty(),
                "conflict must have at least one value");
            prop_assert!(conflict.source_ids.len() >= 2,
                "conflict must reference at least 2 sources");
            prop_assert!(conflict.values.len() <= conflict.source_ids.len(),
                "number of distinct values must not exceed number of sources");
        }
    }

    #[test]
    fn conflict_id_stable_under_card_permutation(
        card1 in card_with_stable_id_and_vuln_strategy(),
        card2 in card_with_stable_id_and_vuln_strategy(),
    ) {
        let conflicts_ab = detect_entity_scoped_conflicts(&[card1.clone(), card2.clone()]);
        let conflicts_ba = detect_entity_scoped_conflicts(&[card2, card1]);
        let mut ids_ab: Vec<String> = conflicts_ab.iter().map(|c| c.id.clone()).collect();
        let mut ids_ba: Vec<String> = conflicts_ba.iter().map(|c| c.id.clone()).collect();
        ids_ab.sort();
        ids_ba.sort();
        prop_assert_eq!(ids_ab, ids_ba,
            "conflict IDs must be stable under card permutation");
    }
}

fn make_vuln_card(
    stable_id: &str,
    cve: &str,
    package: Option<&str>,
    ecosystem: Option<&str>,
    patched: Vec<&str>,
    published: Option<&str>,
) -> SourceCard {
    let mut card = SourceCard::new(
        format!("advisory for {cve}").as_str(),
        format!("https://example.com/{cve}").as_str(),
        vec!["osv".to_string()],
        Some(0.5),
        TrustLevel::ExternalUntrusted,
    );
    card.stable_id = Some(stable_id.to_string());
    card.metadata.vulnerability =
        Some(Box::new(eggsearch::core::security::VulnerabilityMetadata {
            cve_ids: vec![cve.to_string()],
            package: package.map(String::from),
            ecosystem: ecosystem.map(String::from),
            patched_versions: patched.into_iter().map(String::from).collect(),
            published_at: published.map(String::from),
            source: VulnerabilitySource::Osv,
            ..Default::default()
        }));
    card
}

#[test]
fn one_card_two_versions_no_conflict() {
    let card = make_vuln_card(
        "src_a",
        "CVE-2024-0001",
        None,
        None,
        vec![">=1.0 <2.0", ">=2.0 <3.0"],
        Some("2024-01-01"),
    );
    let conflicts = detect_entity_scoped_conflicts(&[card]);
    assert!(
        conflicts.is_empty(),
        "single card with two patched versions must not produce a conflict"
    );
}

#[test]
fn same_version_set_different_order_no_conflict() {
    let card1 = make_vuln_card(
        "src_1",
        "CVE-2024-0001",
        None,
        None,
        vec![">=1.0 <2.0", ">=2.0 <3.0"],
        Some("2024-01-01"),
    );
    let card2 = make_vuln_card(
        "src_2",
        "CVE-2024-0001",
        None,
        None,
        vec![">=2.0 <3.0", ">=1.0 <2.0"],
        Some("2024-01-01"),
    );
    let conflicts = detect_entity_scoped_conflicts(&[card1, card2]);
    let version_conflicts: Vec<_> = conflicts
        .iter()
        .filter(|c| c.compared_fields.iter().any(|f| f == "patched_versions"))
        .collect();
    assert!(
        version_conflicts.is_empty(),
        "two cards with the same patched-version set in different order must not produce a conflict"
    );
}

#[test]
fn genuinely_different_version_sets_produce_conflict() {
    let card1 = make_vuln_card(
        "src_1",
        "CVE-2024-0001",
        None,
        None,
        vec![">=1.0 <2.0"],
        Some("2024-01-01"),
    );
    let card2 = make_vuln_card(
        "src_2",
        "CVE-2024-0001",
        None,
        None,
        vec![">=1.5 <3.0"],
        Some("2024-01-01"),
    );
    let conflicts = detect_entity_scoped_conflicts(&[card1, card2]);
    let version_conflicts: Vec<_> = conflicts
        .iter()
        .filter(|c| c.compared_fields.iter().any(|f| f == "patched_versions"))
        .collect();
    assert!(
        !version_conflicts.is_empty(),
        "two cards with genuinely different patched-version sets must produce a conflict"
    );
}

#[test]
fn same_cve_different_package_no_conflict() {
    let card1 = make_vuln_card(
        "src_1",
        "CVE-2024-0001",
        Some("crate-a"),
        Some("crates.io"),
        vec![">=1.0 <2.0"],
        Some("2024-01-01"),
    );
    let card2 = make_vuln_card(
        "src_2",
        "CVE-2024-0001",
        Some("crate-b"),
        Some("crates.io"),
        vec![">=1.5 <3.0"],
        Some("2024-01-01"),
    );
    let conflicts = detect_entity_scoped_conflicts(&[card1, card2]);
    let version_conflicts: Vec<_> = conflicts
        .iter()
        .filter(|c| c.compared_fields.iter().any(|f| f == "patched_versions"))
        .collect();
    assert!(
        version_conflicts.is_empty(),
        "same CVE but different package must NOT produce a package-range conflict"
    );
}

#[test]
fn same_owner_repo_different_hosts_no_conflict() {
    use eggsearch::core::code_metadata::CodeHost;

    let mut card_github = SourceCard::new(
        "repo on github",
        "https://github.com/org/repo",
        vec!["github_code".to_string()],
        Some(0.8),
        TrustLevel::ExternalUntrusted,
    );
    card_github.stable_id = Some("src_github".to_string());
    card_github.metadata.code_evidence = Some(eggsearch::core::code_evidence::CodeEvidence {
        host: Some(CodeHost::Github),
        owner: Some("org".to_string()),
        repo: Some("repo".to_string()),
        ref_name: Some("main".to_string()),
        path: Some("src/lib.rs".to_string()),
        commit_sha: Some("abc123".to_string()),
        ..Default::default()
    });

    let mut card_gitlab = SourceCard::new(
        "repo on gitlab",
        "https://gitlab.com/org/repo",
        vec!["gitlab_code".to_string()],
        Some(0.8),
        TrustLevel::ExternalUntrusted,
    );
    card_gitlab.stable_id = Some("src_gitlab".to_string());
    card_gitlab.metadata.code_evidence = Some(eggsearch::core::code_evidence::CodeEvidence {
        host: Some(CodeHost::Gitlab),
        owner: Some("org".to_string()),
        repo: Some("repo".to_string()),
        ref_name: Some("main".to_string()),
        path: Some("src/main.rs".to_string()),
        commit_sha: None,
        ..Default::default()
    });

    let conflicts = detect_entity_scoped_conflicts(&[card_github, card_gitlab]);
    let mutable_vs_pinned: Vec<_> = conflicts
        .iter()
        .filter(|c| {
            matches!(
                c.conflict_class,
                eggsearch::core::conflict::ConflictClass::MutableVsCommitPinnedContent
            )
        })
        .collect();
    assert!(
        mutable_vs_pinned.is_empty(),
        "same owner/repo on different hosts must NOT produce a repository conflict"
    );
}

#[test]
fn duplicate_provider_aggregation_no_extra_source() {
    let mut card1 = SourceCard::new(
        "advisory 1",
        "https://example.com/cve-1",
        vec!["osv".to_string(), "github_advisory".to_string()],
        Some(0.5),
        TrustLevel::ExternalUntrusted,
    );
    card1.stable_id = Some("src_1".to_string());
    card1.metadata.vulnerability =
        Some(Box::new(eggsearch::core::security::VulnerabilityMetadata {
            cve_ids: vec!["CVE-2024-0001".to_string()],
            patched_versions: vec![">=1.0 <2.0".to_string()],
            published_at: Some("2024-01-01".to_string()),
            source: VulnerabilitySource::Osv,
            ..Default::default()
        }));

    let mut card2 = SourceCard::new(
        "advisory 2",
        "https://example.com/cve-1-other",
        vec!["osv".to_string()],
        Some(0.5),
        TrustLevel::ExternalUntrusted,
    );
    card2.stable_id = Some("src_2".to_string());
    card2.metadata.vulnerability =
        Some(Box::new(eggsearch::core::security::VulnerabilityMetadata {
            cve_ids: vec!["CVE-2024-0001".to_string()],
            patched_versions: vec![">=1.0 <2.0".to_string()],
            published_at: Some("2024-01-01".to_string()),
            source: VulnerabilitySource::Osv,
            ..Default::default()
        }));

    let conflicts = detect_entity_scoped_conflicts(&[card1, card2]);
    let version_conflicts: Vec<_> = conflicts
        .iter()
        .filter(|c| c.compared_fields.iter().any(|f| f == "patched_versions"))
        .collect();
    assert!(
        version_conflicts.is_empty(),
        "duplicate aggregated provider contributions with same version must not produce a conflict"
    );
}
