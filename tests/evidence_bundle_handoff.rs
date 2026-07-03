use eggsearch::core::evidence_bundle::{
    compute_bundle_id, EvidenceBundleRequest, EvidenceFetchInput, EvidenceGapKind,
    EvidenceSourceInput,
};
use eggsearch::core::quality::{ResultConfidence, ResultQuality};
use eggsearch::core::result::TrustLevel;
use eggsearch::core::source_card::{SourceKind, SourceMetadata};
use eggsearch::meta::evidence_bundle::build_evidence_bundle;

fn make_source(url: &str, title: &str, provider: &str) -> EvidenceSourceInput {
    EvidenceSourceInput {
        id: Some(format!("src_{}", url.len())),
        url: Some(url.to_string()),
        title: Some(title.to_string()),
        snippet: None,
        providers: vec![provider.to_string()],
        score: Some(0.9),
        trust: Some(TrustLevel::ExternalUntrusted),
        trust_markers: None,
        metadata: None,
        quality: None,
    }
}

fn make_local_source(url: &str, title: &str) -> EvidenceSourceInput {
    EvidenceSourceInput {
        id: None,
        url: Some(url.to_string()),
        title: Some(title.to_string()),
        snippet: None,
        providers: vec!["local_workspace".to_string()],
        score: None,
        trust: Some(TrustLevel::LocalTrusted),
        trust_markers: None,
        metadata: None,
        quality: None,
    }
}

fn make_fetch(url: &str, source_id: Option<&str>) -> EvidenceFetchInput {
    EvidenceFetchInput {
        source_id: source_id.map(String::from),
        url: Some(url.to_string()),
        locator: None,
        fetched: true,
        content_type: None,
        language: None,
        selected_span: None,
        code_span_id: None,
        line_start: None,
        line_end: None,
        text: Some("fn main() {}".to_string()),
        truncated: false,
        trust: None,
        trust_markers: None,
        warnings: vec![],
    }
}

fn empty_request() -> EvidenceBundleRequest {
    EvidenceBundleRequest {
        goal: None,
        sources: vec![],
        fetches: vec![],
        include_unfetched_sources: None,
        max_sources: None,
        max_fetched_items: None,
        max_total_chars: None,
        warnings: vec![],
        research_claims: None,
        research_conflicts: None,
    }
}

#[test]
fn bundle_preserves_source_ids() {
    let req = EvidenceBundleRequest {
        goal: Some("test".into()),
        sources: vec![
            make_source("https://docs.rs/axum", "axum", "duckduckgo"),
            make_source("https://crates.io/axum", "axum on crates.io", "brave"),
        ],
        fetches: vec![],
        ..empty_request()
    };

    let bundle = build_evidence_bundle(req);
    assert_eq!(bundle.sources.len(), 2);
    let source_ids: Vec<&str> = bundle
        .sources
        .iter()
        .map(|s| s.source_id.as_str())
        .collect();
    for id in &source_ids {
        assert!(
            id.starts_with("src_"),
            "Source ID should start with src_: {id}"
        );
    }
}

#[test]
fn bundle_preserves_fetch_ids() {
    let req = EvidenceBundleRequest {
        sources: vec![make_source("https://docs.rs/axum", "axum", "duckduckgo")],
        fetches: vec![make_fetch("https://docs.rs/axum", None)],
        ..empty_request()
    };

    let bundle = build_evidence_bundle(req);
    assert_eq!(bundle.fetched_items.len(), 1);
    assert!(bundle.fetched_items[0].fetch_id.starts_with("fetch_"));
}

#[test]
fn bundle_preserves_code_span_metadata() {
    let req = EvidenceBundleRequest {
        sources: vec![EvidenceSourceInput {
            id: None,
            url: Some("https://github.com/tokio-rs/axum/blob/main/src/lib.rs".into()),
            title: Some("axum/src/lib.rs".into()),
            snippet: None,
            providers: vec!["duckduckgo".into()],
            score: None,
            trust: None,
            trust_markers: None,
            metadata: Some(SourceMetadata {
                source_kind: SourceKind::SourceFile,
                domain: Some("github.com".into()),
                ..Default::default()
            }),
            quality: None,
        }],
        fetches: vec![EvidenceFetchInput {
            source_id: None,
            url: Some("https://raw.githubusercontent.com/tokio-rs/axum/main/src/lib.rs".into()),
            locator: None,
            fetched: true,
            content_type: Some("text/plain".into()),
            language: Some("rust".into()),
            selected_span: None,
            code_span_id: Some("span_aabbccdd".into()),
            line_start: Some(1),
            line_end: Some(50),
            text: Some("use tower::Service;".into()),
            truncated: false,
            trust: None,
            trust_markers: None,
            warnings: vec![],
        }],
        ..empty_request()
    };

    let bundle = build_evidence_bundle(req);
    assert_eq!(bundle.fetched_items.len(), 1);
    assert_eq!(
        bundle.fetched_items[0].code_span_id.as_deref(),
        Some("span_aabbccdd")
    );
    assert_eq!(bundle.fetched_items[0].line_start, Some(1));
    assert_eq!(bundle.fetched_items[0].line_end, Some(50));
    assert_eq!(
        bundle.fetched_items[0].content_type.as_deref(),
        Some("text/plain")
    );
    assert_eq!(bundle.fetched_items[0].language.as_deref(), Some("rust"));
}

#[test]
fn bundle_preserves_security_context() {
    use eggsearch::core::source_card::SourceMetadata;

    let req = EvidenceBundleRequest {
        sources: vec![EvidenceSourceInput {
            id: None,
            url: Some("https://osv.dev/vulnerability/GHSA-xxxx-xxxx".into()),
            title: Some("Security Advisory".into()),
            snippet: Some("Critical vulnerability".into()),
            providers: vec!["duckduckgo".into()],
            score: None,
            trust: None,
            trust_markers: None,
            metadata: Some(SourceMetadata {
                source_kind: SourceKind::SecurityAdvisory,
                domain: Some("osv.dev".into()),
                ..Default::default()
            }),
            quality: None,
        }],
        fetches: vec![],
        ..empty_request()
    };

    let bundle = build_evidence_bundle(req);
    assert_eq!(bundle.sources.len(), 1);
    assert_eq!(
        bundle.sources[0].source_kind,
        Some(SourceKind::SecurityAdvisory)
    );
}

#[test]
fn bundle_preserves_research_claims() {
    use eggsearch::core::research::{ResearchClaim, ResearchClaimType, ResearchConflict};

    let claims = vec![ResearchClaim {
        id: "claim_1".into(),
        text: "axum is faster than actix-web".into(),
        claim_type: ResearchClaimType::Performance,
        confidence: ResultConfidence::High,
        supporting_source_ids: vec!["src_a".into()],
        conflicting_source_ids: vec![],
        missing_evidence: vec![],
        source_quality_notes: vec![],
    }];

    let conflicts = vec![ResearchConflict {
        id: "conflict_1".into(),
        topic: "performance".into(),
        claim_ids: vec!["claim_1".into()],
        side_a_source_ids: vec!["src_a".into()],
        side_b_source_ids: vec!["src_b".into()],
        notes: vec!["conflicting benchmarks".into()],
    }];

    let req = EvidenceBundleRequest {
        sources: vec![make_source("https://a.com", "a", "test")],
        fetches: vec![],
        research_claims: Some(claims),
        research_conflicts: Some(conflicts),
        ..empty_request()
    };

    let bundle = build_evidence_bundle(req);
    assert!(bundle.research_claims.is_some());
    let claims = bundle.research_claims.unwrap();
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].id, "claim_1");
    assert_eq!(claims[0].claim_type, ResearchClaimType::Performance);

    assert!(bundle.research_conflicts.is_some());
    let conflicts = bundle.research_conflicts.unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].id, "conflict_1");
}

#[test]
fn bundle_gap_analysis_detects_missing_fetches() {
    let req = EvidenceBundleRequest {
        sources: vec![
            make_source("https://docs.rs/axum", "axum", "duckduckgo"),
            make_source("https://crates.io/axum", "axum on crates.io", "brave"),
        ],
        fetches: vec![],
        ..empty_request()
    };

    let bundle = build_evidence_bundle(req);
    assert!(
        bundle
            .gaps
            .iter()
            .any(|g| g.kind == EvidenceGapKind::SourceUnfetched),
        "Expected SourceUnfetched gap, got: {:?}",
        bundle.gaps
    );
}

#[test]
fn bundle_gap_analysis_detects_all_external_untrusted() {
    let req = EvidenceBundleRequest {
        sources: vec![
            make_source("https://a.com", "a", "test"),
            make_source("https://b.com", "b", "test"),
        ],
        fetches: vec![],
        ..empty_request()
    };

    let bundle = build_evidence_bundle(req);
    assert!(
        bundle
            .gaps
            .iter()
            .any(|g| g.kind == EvidenceGapKind::AllResultsExternalUntrusted),
        "Expected AllResultsExternalUntrusted gap, got: {:?}",
        bundle.gaps
    );
}

#[test]
fn bundle_limit_enforcement_max_sources() {
    let sources: Vec<EvidenceSourceInput> = (0..10)
        .map(|i| {
            make_source(
                &format!("https://example.com/{i}"),
                &format!("page {i}"),
                "test",
            )
        })
        .collect();

    let req = EvidenceBundleRequest {
        sources,
        max_sources: Some(3),
        ..empty_request()
    };

    let bundle = build_evidence_bundle(req);
    assert_eq!(bundle.sources.len(), 3);
    assert!(bundle.limits.sources_truncated);
    assert_eq!(bundle.limits.max_sources, 3);
}

#[test]
fn bundle_limit_enforcement_max_fetched_items() {
    let fetches: Vec<EvidenceFetchInput> = (0..10)
        .map(|i| make_fetch(&format!("https://example.com/{i}"), None))
        .collect();

    let req = EvidenceBundleRequest {
        fetches,
        max_fetched_items: Some(3),
        ..empty_request()
    };

    let bundle = build_evidence_bundle(req);
    assert_eq!(bundle.fetched_items.len(), 3);
    assert!(bundle.limits.fetched_items_truncated);
    assert_eq!(bundle.limits.max_fetched_items, 3);
}

#[test]
fn bundle_limit_enforcement_max_total_chars() {
    let req = EvidenceBundleRequest {
        fetches: vec![
            EvidenceFetchInput {
                source_id: None,
                url: Some("https://a.com".into()),
                locator: None,
                fetched: true,
                content_type: None,
                language: None,
                selected_span: None,
                code_span_id: None,
                line_start: None,
                line_end: None,
                text: Some("a".repeat(80)),
                truncated: false,
                trust: None,
                trust_markers: None,
                warnings: vec![],
            },
            EvidenceFetchInput {
                source_id: None,
                url: Some("https://b.com".into()),
                locator: None,
                fetched: true,
                content_type: None,
                language: None,
                selected_span: None,
                code_span_id: None,
                line_start: None,
                line_end: None,
                text: Some("b".repeat(80)),
                truncated: false,
                trust: None,
                trust_markers: None,
                warnings: vec![],
            },
        ],
        max_total_chars: Some(100),
        ..empty_request()
    };

    let bundle = build_evidence_bundle(req);
    assert!(bundle.limits.total_chars_exceeded);
}

#[test]
fn bundle_preserves_trust_labels() {
    let req = EvidenceBundleRequest {
        sources: vec![
            make_source("https://a.com", "a", "test"),
            make_source("https://b.com", "b", "test"),
            make_local_source("workspace://root/src/main.rs", "main.rs"),
        ],
        fetches: vec![],
        ..empty_request()
    };

    let bundle = build_evidence_bundle(req);
    assert_eq!(bundle.trust_summary.external_untrusted_count, 2);
    assert_eq!(bundle.trust_summary.local_trusted_count, 1);
}

#[test]
fn bundle_source_links_connect_sources_to_fetches() {
    let source = make_source("https://docs.rs/axum", "axum", "duckduckgo");
    let req = EvidenceBundleRequest {
        sources: vec![source],
        fetches: vec![make_fetch("https://docs.rs/axum", None)],
        ..empty_request()
    };

    let bundle = build_evidence_bundle(req);
    assert_eq!(bundle.source_links.len(), 1);
    assert!(matches!(
        bundle.source_links[0].link_reason,
        eggsearch::core::evidence_bundle::EvidenceBundleLinkReason::UrlMatch
    ));
    // The fetch should be linked to the source
    assert!(bundle.fetched_items[0].source_id.is_some());
}

#[test]
fn bundle_provider_summary_tracks_providers() {
    let req = EvidenceBundleRequest {
        sources: vec![
            make_source("https://a.com", "a", "duckduckgo"),
            make_source("https://b.com", "b", "duckduckgo"),
            make_source("https://c.com", "c", "brave"),
        ],
        fetches: vec![],
        ..empty_request()
    };

    let bundle = build_evidence_bundle(req);
    assert!(bundle
        .provider_summary
        .providers_used
        .contains(&"duckduckgo".to_string()));
    assert!(bundle
        .provider_summary
        .providers_used
        .contains(&"brave".to_string()));
    assert_eq!(bundle.provider_summary.providers_used.len(), 2);

    let dd_count = bundle
        .provider_summary
        .per_provider_counts
        .iter()
        .find(|c| c.provider_id == "duckduckgo")
        .unwrap();
    assert_eq!(dd_count.count, 2);
}

#[test]
fn bundle_deterministic_id_across_identical_inputs() {
    let make_req = || EvidenceBundleRequest {
        goal: Some("debug error".into()),
        sources: vec![make_source("https://docs.rs/axum", "axum", "test")],
        fetches: vec![make_fetch("https://docs.rs/axum", None)],
        ..empty_request()
    };

    let b1 = build_evidence_bundle(make_req());
    let b2 = build_evidence_bundle(make_req());
    assert_eq!(b1.bundle_id, b2.bundle_id);
    assert!(b1.bundle_id.starts_with("bundle_"));
}

#[test]
fn bundle_id_changes_with_different_sources() {
    let id1 = compute_bundle_id(Some("goal"), &["src_a".into()], &[]);
    let id2 = compute_bundle_id(Some("goal"), &["src_b".into()], &[]);
    assert_ne!(id1, id2);
}

#[test]
fn bundle_id_changes_with_different_goals() {
    let sources = vec!["src_aaa".into()];
    let id1 = compute_bundle_id(Some("goal A"), &sources, &[]);
    let id2 = compute_bundle_id(Some("goal B"), &sources, &[]);
    assert_ne!(id1, id2);
}

#[test]
fn bundle_preserves_quality_metadata() {
    let req = EvidenceBundleRequest {
        sources: vec![EvidenceSourceInput {
            id: None,
            url: Some("https://docs.rs/axum".into()),
            title: Some("axum".into()),
            snippet: None,
            providers: vec!["test".into()],
            score: None,
            trust: None,
            trust_markers: None,
            metadata: None,
            quality: Some(ResultQuality {
                confidence: ResultConfidence::High,
                ..Default::default()
            }),
        }],
        fetches: vec![],
        ..empty_request()
    };

    let bundle = build_evidence_bundle(req);
    assert_eq!(bundle.sources.len(), 1);
    let quality = bundle.sources[0].quality.as_ref().unwrap();
    assert_eq!(quality.confidence, ResultConfidence::High);
}

#[test]
fn bundle_no_sources_no_fetches() {
    let req = empty_request();
    let bundle = build_evidence_bundle(req);
    assert!(bundle.sources.is_empty());
    assert!(bundle.fetched_items.is_empty());
    assert!(bundle.source_links.is_empty());
    assert!(bundle.bundle_id.starts_with("bundle_"));
}

#[test]
fn bundle_default_limits() {
    let req = empty_request();
    let bundle = build_evidence_bundle(req);
    assert_eq!(bundle.limits.max_sources, 50);
    assert_eq!(bundle.limits.max_fetched_items, 20);
    assert_eq!(bundle.limits.max_total_chars, 100_000);
    assert!(!bundle.limits.sources_truncated);
    assert!(!bundle.limits.fetched_items_truncated);
    assert!(!bundle.limits.total_chars_exceeded);
}
