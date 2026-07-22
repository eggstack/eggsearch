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
}
