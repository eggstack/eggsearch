use criterion::{black_box, criterion_group, criterion_main, Criterion};
use eggsearch::core::conflict::detect_entity_scoped_conflicts;
use eggsearch::core::evidence_postprocess::{materialize_evidence_roles, resolve_workflow_model};
use eggsearch::core::result::TrustLevel;
use eggsearch::core::retrieval_status::{
    summarize_retrieval, EvidenceAbsenceKind, RetrievalDimensionStatus,
};
use eggsearch::core::source_card::{SourceCard, SourceKind};

fn bench_serialize_web_search_response(c: &mut Criterion) {
    let cards: Vec<serde_json::Value> = (0..10)
        .map(|i| {
            serde_json::json!({
                "id": format!("src_{:032x}", i),
                "stable_id": format!("src_{:016x}", i * 7919),
                "url": format!("https://example.com/result/{}", i),
                "title": format!("Result {} - Example Page Title", i),
                "snippet": "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.",
                "trust": "external_untrusted",
                "metadata": {
                    "source_kind": if i % 3 == 0 { "official_docs" } else if i % 3 == 1 { "source_repository" } else { "news" },
                    "domain": format!("example{}.com", i),
                    "rank_reasons": ["rrf_multi_provider", "intent_match"]
                },
                "quality": {
                    "confidence": "high",
                    "relevance": "strong",
                    "authority": "official",
                    "freshness": "recent",
                    "evidence_strength": "structured_metadata",
                    "uncertainty_reasons": [],
                    "quality_reasons": ["official_docs", "fresh_timestamp"]
                },
                "trust_markers": {
                    "text_sanitized": true,
                    "text_truncated": false,
                    "text_framed": false,
                    "control_chars_removed": 0,
                    "injection_hits": 0
                }
            })
        })
        .collect();

    let response = serde_json::json!({
        "query": "axum router middleware documentation",
        "max_results": 10,
        "results": cards,
        "warnings": [],
        "trust_markers": {
            "text_sanitized": 10,
            "text_truncated": 0,
            "text_framed": 0,
            "control_chars_removed": 0,
            "injection_hits": 0
        }
    });

    c.bench_function("serialize_web_search_response_10_cards", |b| {
        b.iter(|| black_box(serde_json::to_value(black_box(&response)).unwrap()));
    });
}

fn bench_serialize_provider_status(c: &mut Criterion) {
    let status = serde_json::json!({
        "providers": [
            {
                "id": "duckduckgo",
                "display_name": "DuckDuckGo",
                "kind": "html_scrape",
                "enabled": true,
                "default": true,
                "requires_api_key": false,
                "configured": false,
                "capabilities": {
                    "safe_search": false,
                    "freshness": false,
                    "language": false,
                    "region": false,
                    "page_count": false,
                    "time_range": false,
                    "sort": false,
                    "categories": false,
                    "code_search": false,
                    "issue_search": false,
                    "release_search": false,
                    "repo_filter": false,
                    "path_filter": false,
                    "language_filter": false,
                    "symbol_search": false,
                    "supports_result_timestamps": false
                }
            },
            {
                "id": "github_code",
                "display_name": "GitHub Code Search",
                "kind": "api_key",
                "enabled": true,
                "default": false,
                "requires_api_key": true,
                "configured": true,
                "capabilities": {
                    "safe_search": false,
                    "freshness": false,
                    "language": false,
                    "region": false,
                    "page_count": false,
                    "time_range": false,
                    "sort": false,
                    "categories": false,
                    "code_search": true,
                    "issue_search": false,
                    "release_search": false,
                    "repo_filter": true,
                    "path_filter": true,
                    "language_filter": true,
                    "symbol_search": false,
                    "supports_result_timestamps": false
                }
            }
        ],
        "server_capabilities": {
            "generic_search": true,
            "explicit_fetch": true,
            "batch_fetch": true,
            "repo_search": true,
            "repo_fetch": true,
            "repo_map": true,
            "security_search": true,
            "research_search": true,
            "evidence_bundle": true,
            "pdf_fetch": false,
            "local_workspace": false
        }
    });

    c.bench_function("serialize_provider_status", |b| {
        b.iter(|| black_box(serde_json::to_value(black_box(&status)).unwrap()));
    });
}

fn bench_identity_hash(c: &mut Criterion) {
    let urls = vec![
        "https://docs.rs/axum/latest/axum/struct.Router.html",
        "https://github.com/tokio-rs/axum/blob/main/axum/src/routing/mod.rs",
        "https://raw.githubusercontent.com/tokio-rs/axum/main/axum/src/lib.rs",
        "https://stackoverflow.com/questions/12345678/how-to-use-axum",
        "https://crates.io/crates/axum",
        "https://blog.rust-lang.org/2024/01/01/axum-0.7.html",
        "https://gitlab.com/group/project/-/blob/main/src/main.rs",
        "https://codeberg.org/owner/repo/src/branch/main/README.md",
        "https://www.typescriptlang.org/docs/handbook/2/types-from-types.html",
        "https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Array/map",
    ];

    // FNV-1a 64-bit hash (same algorithm used by eggsearch identity module)
    fn fnv1a64(data: &[u8]) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for &byte in data {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    c.bench_function("fnv1a64_hash_10_urls", |b| {
        b.iter(|| {
            for url in &urls {
                black_box(fnv1a64(black_box(url.as_bytes())));
            }
        });
    });

    // Benchmark with the versioned prefix eggsearch uses
    fn eggsearch_id_hash(entity: &str, data: &[u8]) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        // eggsearch-id-v1\0
        for &b in b"eggsearch-id-v1\0" {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        // entity sub-namespace
        for &b in entity.as_bytes() {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        // null separator (matches eggsearch::core::identity::entity_prefix)
        hash ^= 0;
        hash = hash.wrapping_mul(0x100000001b3);
        for &b in data {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    c.bench_function("eggsearch_id_hash_10_urls", |b| {
        b.iter(|| {
            for url in &urls {
                black_box(eggsearch_id_hash("source", black_box(url.as_bytes())));
            }
        });
    });
}

fn bench_metadata_construction(c: &mut Criterion) {
    c.bench_function("build_10_source_cards", |b| {
        b.iter(|| {
            let cards: Vec<serde_json::Value> = (0..10)
                .map(|i| {
                    let kind = match i % 5 {
                        0 => "official_docs",
                        1 => "source_repository",
                        2 => "issue_thread",
                        3 => "release_notes",
                        _ => "tutorial",
                    };
                    let authority = match i % 5 {
                        0 => "primary",
                        1 => "maintainer",
                        2 => "community",
                        3 => "package_registry",
                        _ => "news_or_blog",
                    };
                    serde_json::json!({
                        "id": format!("src_{:032x}", i),
                        "url": format!("https://example{}.com/page", i),
                        "title": format!("Page {}", i),
                        "snippet": format!("Snippet for result {}", i),
                        "trust": "external_untrusted",
                        "metadata": {
                            "source_kind": kind,
                            "domain": format!("example{}.com", i),
                            "rank_reasons": ["rrf_multi_provider"]
                        },
                        "quality": {
                            "confidence": if i % 2 == 0 { "high" } else { "medium" },
                            "relevance": "strong",
                            "authority": authority,
                            "freshness": "recent",
                            "evidence_strength": "snippet_only",
                            "uncertainty_reasons": [],
                            "quality_reasons": []
                        }
                    })
                })
                .collect();
            black_box(cards);
        });
    });
}

fn make_source_cards(n: usize) -> Vec<SourceCard> {
    (0..n)
        .map(|i| {
            let mut card = SourceCard::new(
                format!("Result {}", i),
                format!("https://example{}.com/result/{}", i, i),
                vec![format!("provider_{}", i % 3)],
                Some(0.5),
                TrustLevel::ExternalUntrusted,
            );
            card.metadata.source_kind = match i % 4 {
                0 => SourceKind::OfficialDocs,
                1 => SourceKind::SourceRepository,
                2 => SourceKind::IssueThread,
                _ => SourceKind::SecurityAdvisory,
            };
            card
        })
        .collect()
}

fn bench_materialize_evidence_roles(c: &mut Criterion) {
    c.bench_function("materialize_evidence_roles_10_cards", |b| {
        b.iter_batched(
            || make_source_cards(10),
            |mut cards| {
                materialize_evidence_roles(black_box(&mut cards));
                black_box(&cards);
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

fn bench_resolve_workflow_model(c: &mut Criterion) {
    let tools = [
        "repo_search",
        "research_search",
        "security_search",
        "web_search",
    ];
    let profiles = [None, Some("security"), Some("research")];
    let domains = [None, Some("architecture_decision"), Some("security_review")];

    c.bench_function("resolve_workflow_model_12_combinations", |b| {
        b.iter(|| {
            for tool in &tools {
                for profile in &profiles {
                    for domain in &domains {
                        black_box(resolve_workflow_model(
                            black_box(tool),
                            black_box(*profile),
                            black_box(*domain),
                            false,
                        ));
                    }
                }
            }
        });
    });
}

fn bench_detect_entity_scoped_conflicts(c: &mut Criterion) {
    c.bench_function("detect_entity_scoped_conflicts_10_cards", |b| {
        b.iter_batched(
            || make_source_cards(10),
            |cards| {
                black_box(detect_entity_scoped_conflicts(black_box(&cards)));
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

fn bench_summarize_retrieval(c: &mut Criterion) {
    let dimensions: Vec<RetrievalDimensionStatus> = (0..5)
        .map(|i| RetrievalDimensionStatus {
            evidence_role: match i % 3 {
                0 => eggsearch::core::evidence_role::EvidenceRole::PrimaryImplementation,
                1 => eggsearch::core::evidence_role::EvidenceRole::AuthoritativeSecurityAdvisory,
                _ => eggsearch::core::evidence_role::EvidenceRole::OfficialDocumentation,
            },
            absence_kind: match i % 4 {
                0 => EvidenceAbsenceKind::NoMatchingEvidenceFound,
                1 => EvidenceAbsenceKind::ProviderFailed,
                2 => EvidenceAbsenceKind::DeadlinePreventedCompletion,
                _ => EvidenceAbsenceKind::NotApplicable,
            },
            provider_id: Some(format!("provider_{}", i)),
            message: format!("dimension {}", i),
            query: Some(format!("query_{}", i)),
        })
        .collect();

    c.bench_function("summarize_retrieval_5_dimensions", |b| {
        b.iter_batched(
            || dimensions.clone(),
            |dims| {
                black_box(summarize_retrieval(black_box(dims)));
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

criterion_group!(
    benches,
    bench_serialize_web_search_response,
    bench_serialize_provider_status,
    bench_identity_hash,
    bench_metadata_construction,
    bench_materialize_evidence_roles,
    bench_resolve_workflow_model,
    bench_detect_entity_scoped_conflicts,
    bench_summarize_retrieval,
);
criterion_main!(benches);
