//! Phase 13 Workstream 6: Research evidence regression corpus.
//!
//! Covers the deterministic evidence analysis pipeline:
//! - Claim extraction from grouped results
//! - Conflict detection (counterpoints, quality disagreements)
//! - Source quality classification
//! - Evidence gap detection (no primary, no recent, secondary-only, etc.)
//! - Full analyze_research_evidence orchestrator
//!
//! Run via:
//! ```bash
//! cargo test --features mock --test research_evidence_corpus
//! ```

use eggsearch::core::quality::ResultConfidence;
use eggsearch::core::research::{
    ResearchClaimType, ResearchConflict, ResearchEvidenceGap, ResearchEvidenceGapKind,
    ResearchQualitySignal, ResearchResultGroup, ResearchResultGroupKind, ResearchSourceClass,
};
use eggsearch::core::result::TrustLevel;
use eggsearch::core::source_card::{SourceCard, SourceKind, SourceMetadata};
use eggsearch::meta::research_evidence_analysis::{
    analyze_research_evidence, classify_quality_signals, classify_source_class, compute_source_qualities,
    detect_conflicts, detect_evidence_gaps, extract_claims,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

fn make_card_with_id(id: &str, source_kind: SourceKind, url: &str) -> SourceCard {
    let mut card = make_card(source_kind, url);
    card.id = id.to_string();
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

// ===========================================================================
// 1. Architecture decision with primary docs and counterpoint
// ===========================================================================

#[test]
fn architecture_decision_with_primary_docs_and_counterpoint() {
    let primary_cards = vec![
        make_card(
            SourceKind::OfficialDocs,
            "https://docs.rs/axum",
        ),
        make_card(
            SourceKind::OfficialDocs,
            "https://docs.rs/axum/0.7/router/index.html",
        ),
    ];
    let counterpoint_cards = vec![
        make_card(SourceKind::Unknown, "https://example.com/criticism"),
        make_card(SourceKind::Unknown, "https://example.com/drawbacks"),
    ];
    let groups = vec![
        make_group(ResearchResultGroupKind::OfficialDocs, primary_cards),
        make_group(ResearchResultGroupKind::Counterpoints, counterpoint_cards),
    ];

    let (claims, conflicts, source_quality, _gaps) =
        analyze_research_evidence(&groups, Some("axum router middleware"));

    // Claims are produced from groups with 2+ results
    assert!(!claims.is_empty(), "must produce claims from non-empty groups");

    // The OfficialDocs group produces an Architecture claim
    let docs_claim = claims
        .iter()
        .find(|c| c.id.contains("OfficialDocs"))
        .expect("must have an OfficialDocs claim");
    assert_eq!(docs_claim.claim_type, ResearchClaimType::Architecture);
    assert_eq!(docs_claim.supporting_source_ids.len(), 2);

    // Counterpoint claim is produced
    let counterpoint_claim = claims
        .iter()
        .find(|c| c.id.contains("Counterpoints"))
        .expect("must have a Counterpoints claim");
    assert!(!counterpoint_claim.conflicting_source_ids.is_empty());

    // Conflicts link counterpoints to other sources
    assert!(!conflicts.is_empty(), "must detect counterpoint conflict");
    let conflict = &conflicts[0];
    assert_eq!(conflict.id, "conflict_counterpoints_0");
    assert!(!conflict.side_a_source_ids.is_empty());
    assert!(!conflict.side_b_source_ids.is_empty());

    // Source quality is computed for all cards
    assert_eq!(source_quality.len(), 4);
}

// ===========================================================================
// 2. Library comparison with benchmark source
// ===========================================================================

#[test]
fn library_comparison_with_benchmark_source() {
    let benchmark_cards = vec![
        make_card(SourceKind::Unknown, "https://example.com/axum-benchmark-2024"),
        make_card(SourceKind::Unknown, "https://example.com/actix-benchmark-results"),
    ];
    let groups = vec![make_group(
        ResearchResultGroupKind::Benchmarks,
        benchmark_cards,
    )];

    let (claims, _conflicts, source_quality, _gaps) =
        analyze_research_evidence(&groups, None);

    // Benchmark group produces Performance claim type
    assert!(!claims.is_empty());
    let benchmark_claim = &claims[0];
    assert_eq!(benchmark_claim.claim_type, ResearchClaimType::Performance);

    // Benchmark sources get appropriate quality signals
    assert_eq!(source_quality.len(), 2);
    for sq in &source_quality {
        assert_eq!(sq.source_class, ResearchSourceClass::Benchmark);
        assert!(
            sq.quality_signals
                .contains(&ResearchQualitySignal::ReproducibleBenchmark),
            "benchmark sources must have ReproducibleBenchmark signal"
        );
    }
}

// ===========================================================================
// 3. Migration planning with changelog/release source
// ===========================================================================

#[test]
fn migration_planning_with_changelog_release_source() {
    let release_cards = vec![
        make_card(
            SourceKind::ReleaseNotes,
            "https://github.com/foo/bar/releases/tag/v2.0",
        ),
        make_card(
            SourceKind::ReleaseNotes,
            "https://github.com/foo/bar/blob/main/CHANGELOG.md",
        ),
    ];
    let groups = vec![make_group(
        ResearchResultGroupKind::ReleaseNotes,
        release_cards,
    )];

    let (_claims, _conflicts, source_quality, _gaps) =
        analyze_research_evidence(&groups, None);

    // Release notes source class is recognized
    assert_eq!(source_quality.len(), 2);
    for sq in &source_quality {
        assert_eq!(
            sq.source_class,
            ResearchSourceClass::ReleaseNotes,
            "release URLs must be classified as ReleaseNotes"
        );
    }
}

// ===========================================================================
// 4. Security research with advisory source
// ===========================================================================

#[test]
fn security_research_with_advisory_source() {
    let advisory_cards = vec![
        make_card(
            SourceKind::SecurityAdvisory,
            "https://osv.dev/vulnerability/GHSA-test-1234",
        ),
        make_card(
            SourceKind::SecurityAdvisory,
            "https://nvd.nist.gov/vuln/detail/CVE-2024-0001",
        ),
    ];
    let groups = vec![make_group(
        ResearchResultGroupKind::SecurityConsiderations,
        advisory_cards,
    )];

    let (_claims, _conflicts, source_quality, _gaps) =
        analyze_research_evidence(&groups, None);

    // Advisory sources are classified as SecurityAdvisory
    assert_eq!(source_quality.len(), 2);
    for sq in &source_quality {
        assert_eq!(
            sq.source_class,
            ResearchSourceClass::SecurityAdvisory,
            "security advisory URLs must be classified as SecurityAdvisory"
        );
        assert!(
            sq.is_primary,
            "security advisory sources must be marked as primary"
        );
    }
}

// ===========================================================================
// 5. Only secondary sources → evidence gap detected
// ===========================================================================

#[test]
fn only_secondary_sources_detected() {
    // Each group has only 1 result → "thin evidence" gap
    let groups = vec![
        make_group(
            ResearchResultGroupKind::CommunityDiscussion,
            vec![make_card(SourceKind::Unknown, "https://stackoverflow.com/q/1")],
        ),
        make_group(
            ResearchResultGroupKind::RecentNews,
            vec![make_card(SourceKind::News, "https://example.com/news")],
        ),
    ];

    let (_claims, _conflicts, _source_quality, gaps) =
        analyze_research_evidence(&groups, None);

    assert!(
        gaps.iter()
            .any(|g| g.kind == ResearchEvidenceGapKind::OnlySecondarySources),
        "must detect only_secondary_sources gap when all groups have ≤1 result"
    );
}

// ===========================================================================
// 6. Stale source set → no_recent_source gap detected
// ===========================================================================

#[test]
fn stale_source_set_no_recent_source_gap() {
    let groups = vec![
        make_group(
            ResearchResultGroupKind::OfficialDocs,
            vec![
                make_card(SourceKind::OfficialDocs, "https://docs.rs/axum"),
                make_card(SourceKind::OfficialDocs, "https://docs.rs/serde"),
            ],
        ),
    ];

    let (_claims, _conflicts, _source_quality, gaps) =
        analyze_research_evidence(&groups, None);

    // No RecentNews group → NoRecentSource gap should fire
    assert!(
        gaps.iter()
            .any(|g| g.kind == ResearchEvidenceGapKind::NoRecentSource),
        "must detect no_recent_source gap when no RecentNews group present"
    );
}

// ===========================================================================
// 7. Conflicting evidence unresolved → conflict with side A/B IDs
// ===========================================================================

#[test]
fn conflicting_evidence_unresolved_produces_conflict_with_sides() {
    let normal_cards = vec![
        make_card_with_id(
            "src_normal_1",
            SourceKind::OfficialDocs,
            "https://docs.rs/axum",
        ),
        make_card_with_id(
            "src_normal_2",
            SourceKind::OfficialDocs,
            "https://docs.rs/serde",
        ),
    ];
    let counterpoint_cards = vec![
        make_card_with_id(
            "src_counter_1",
            SourceKind::Unknown,
            "https://example.com/criticism",
        ),
        make_card_with_id(
            "src_counter_2",
            SourceKind::Unknown,
            "https://example.com/drawbacks",
        ),
    ];
    let groups = vec![
        make_group(ResearchResultGroupKind::OfficialDocs, normal_cards),
        make_group(ResearchResultGroupKind::Counterpoints, counterpoint_cards),
    ];

    let (claims, conflicts, _source_quality, gaps) =
        analyze_research_evidence(&groups, Some("axum vs actix"));

    // Conflicts are detected
    assert!(
        !conflicts.is_empty(),
        "must detect conflict from counterpoints group"
    );

    let conflict = &conflicts[0];
    assert_eq!(conflict.id, "conflict_counterpoints_0");
    assert!(!conflict.side_a_source_ids.is_empty(), "side A must have source IDs");
    assert!(!conflict.side_b_source_ids.is_empty(), "side B must have source IDs");
    assert_eq!(
        conflict.topic, "Counterpoint evidence found",
        "conflict topic must be set"
    );

    // ConflictingEvidenceUnresolved gap may also appear if no high-confidence claim
    // (depends on quality_summary)
}

// ===========================================================================
// 8. No primary source found → evidence gap detected
// ===========================================================================

#[test]
fn no_primary_source_found_detected() {
    let groups = vec![
        make_group(
            ResearchResultGroupKind::CommunityDiscussion,
            vec![
                make_card(SourceKind::Unknown, "https://stackoverflow.com/q/1"),
                make_card(SourceKind::Unknown, "https://stackoverflow.com/q/2"),
            ],
        ),
    ];

    let (_claims, _conflicts, _source_quality, gaps) =
        analyze_research_evidence(&groups, None);

    assert!(
        gaps.iter()
            .any(|g| g.kind == ResearchEvidenceGapKind::NoPrimarySource),
        "must detect no_primary_source gap when no PrimarySources or OfficialDocs groups exist"
    );
}

// ===========================================================================
// Additional: classify_source_class tests
// ===========================================================================

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
fn source_class_benchmark_from_url() {
    let card = make_card(SourceKind::Unknown, "https://example.com/benchmark-results");
    assert_eq!(classify_source_class(&card), ResearchSourceClass::Benchmark);
}

#[test]
fn source_class_paper_from_url() {
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
fn source_class_stackoverflow_is_forum() {
    let card = make_card(SourceKind::Unknown, "https://stackoverflow.com/q/12345");
    assert_eq!(
        classify_source_class(&card),
        ResearchSourceClass::ForumThread
    );
}

// ===========================================================================
// Additional: classify_quality_signals tests
// ===========================================================================

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

// ===========================================================================
// Additional: extract_claims tests
// ===========================================================================

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
fn claims_skips_single_result_groups() {
    let cards = vec![make_card(SourceKind::OfficialDocs, "https://docs.rs/axum")];
    let group = make_group(ResearchResultGroupKind::OfficialDocs, cards);
    let claims = extract_claims(&[group], None);

    assert!(claims.is_empty(), "single-result groups must be skipped");
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
    assert!(
        claims.len() <= 10,
        "claims must be bounded at 10, got {}",
        claims.len()
    );
}

// ===========================================================================
// Additional: detect_conflicts tests
// ===========================================================================

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
    assert!(
        conflicts.len() <= 5,
        "conflicts must be bounded at 5, got {}",
        conflicts.len()
    );
}

// ===========================================================================
// Additional: detect_evidence_gaps tests
// ===========================================================================

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
    assert!(
        gaps.iter()
            .any(|g| g.kind == ResearchEvidenceGapKind::NoPrimarySource)
    );
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
    assert!(
        gaps.iter()
            .any(|g| g.kind == ResearchEvidenceGapKind::NoBenchmarkSource)
    );
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
    assert!(
        gaps.iter()
            .any(|g| g.kind == ResearchEvidenceGapKind::NoRecentSource)
    );
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
    assert!(
        gaps.iter()
            .any(|g| g.kind == ResearchEvidenceGapKind::ConflictingEvidenceUnresolved)
    );
}

#[test]
fn gaps_bounded() {
    let groups = vec![];
    let gaps = detect_evidence_gaps(&groups, &[], &[], None);
    assert!(
        gaps.len() <= 9,
        "gaps must be bounded at 9, got {}",
        gaps.len()
    );
}

// ===========================================================================
// Additional: full analyze_research_evidence orchestrator
// ===========================================================================

#[test]
fn analyze_empty_groups() {
    let (claims, conflicts, source_quality, evidence_gaps) =
        analyze_research_evidence(&[], None);
    assert!(claims.is_empty());
    assert!(conflicts.is_empty());
    assert!(source_quality.is_empty());
    assert!(!evidence_gaps.is_empty(), "empty groups should produce gaps");
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

// ===========================================================================
// Additional: version context missing gap
// ===========================================================================

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
        "ReleaseNotes group present → VersionContextMissing should not fire"
    );
}

// ===========================================================================
// Additional: compute_source_qualities
// ===========================================================================

#[test]
fn compute_source_qualities_for_mixed_groups() {
    let groups = vec![
        make_group(
            ResearchResultGroupKind::OfficialDocs,
            vec![
                make_card(SourceKind::OfficialDocs, "https://docs.rs/axum"),
                make_card(SourceKind::OfficialDocs, "https://docs.rs/serde"),
            ],
        ),
        make_group(
            ResearchResultGroupKind::Benchmarks,
            vec![make_card(
                SourceKind::Unknown,
                "https://example.com/benchmark",
            )],
        ),
    ];

    let qualities = compute_source_qualities(&groups);
    assert_eq!(qualities.len(), 3);

    let docs_qualities: Vec<_> = qualities
        .iter()
        .filter(|q| q.source_class == ResearchSourceClass::OfficialDocs)
        .collect();
    assert_eq!(docs_qualities.len(), 2);
    for q in docs_qualities {
        assert!(q.is_primary);
        assert!(!q.is_stale);
    }

    let bench_quality = qualities
        .iter()
        .find(|q| q.source_class == ResearchSourceClass::Benchmark)
        .expect("must have benchmark quality");
    assert!(
        bench_quality
            .quality_signals
            .contains(&ResearchQualitySignal::ReproducibleBenchmark)
    );
}

// ===========================================================================
// Additional: Deterministic claim IDs
// ===========================================================================

#[test]
fn claim_ids_are_deterministic() {
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
    let claims_a = extract_claims(&groups, None);
    let claims_b = extract_claims(&groups, None);

    assert_eq!(claims_a.len(), claims_b.len());
    for (a, b) in claims_a.iter().zip(claims_b.iter()) {
        assert_eq!(a.id, b.id, "claim IDs must be deterministic");
        assert_eq!(a.text, b.text);
        assert_eq!(a.claim_type, b.claim_type);
    }
}

// ===========================================================================
// Additional: Serde roundtrip for ResearchClaim
// ===========================================================================

#[test]
fn research_claim_serde_roundtrip() {
    let claim = eggsearch::core::research::ResearchClaim {
        id: "claim_test".to_string(),
        text: "Test claim".to_string(),
        claim_type: ResearchClaimType::Performance,
        confidence: ResultConfidence::High,
        supporting_source_ids: vec!["src_1".to_string()],
        conflicting_source_ids: vec!["src_2".to_string()],
        missing_evidence: vec!["benchmark data".to_string()],
        source_quality_notes: vec!["2 results".to_string()],
    };

    let json = serde_json::to_string(&claim).unwrap();
    let parsed: eggsearch::core::research::ResearchClaim = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.id, "claim_test");
    assert_eq!(parsed.claim_type, ResearchClaimType::Performance);
    assert_eq!(parsed.confidence, ResultConfidence::High);
}
