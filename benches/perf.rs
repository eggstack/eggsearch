use criterion::{black_box, criterion_group, criterion_main, Criterion};
use eggsearch::core::conflict::detect_entity_scoped_conflicts;
use eggsearch::core::evidence_postprocess::{materialize_evidence_roles, resolve_workflow_model};
use eggsearch::core::result::TrustLevel;
use eggsearch::core::retrieval_status::{
    summarize_retrieval, EvidenceAbsenceKind, RetrievalDimensionStatus, TruncationEvidence,
};
use eggsearch::core::source_card::{SourceCard, SourceKind};

fn bench_serialize_web_search_response(c: &mut Criterion) {
    let cards: Vec<serde_json::Value> = (0..10)
        .map(|i| {
            serde_json::json!({
                "id": format!("src_{:032x}", i),
                "stable_id": format!("src_{:016x}", i * 7919),
                "url": format!("https://example.com/result/{i}"),
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
                        "title": format!("Page {i}"),
                        "snippet": format!("Snippet for result {i}"),
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
                format!("Result {i}"),
                format!("https://example{i}.com/result/{i}"),
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
            provider_id: Some(format!("provider_{i}")),
            message: format!("dimension {i}"),
            query: Some(format!("query_{i}")),
            ..Default::default()
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

fn bench_summarize_retrieval_20_dimensions(c: &mut Criterion) {
    let dimensions: Vec<RetrievalDimensionStatus> = (0..20)
        .map(|i| RetrievalDimensionStatus {
            evidence_role: match i % 5 {
                0 => eggsearch::core::evidence_role::EvidenceRole::PrimaryImplementation,
                1 => eggsearch::core::evidence_role::EvidenceRole::AuthoritativeSecurityAdvisory,
                2 => eggsearch::core::evidence_role::EvidenceRole::OfficialDocumentation,
                3 => eggsearch::core::evidence_role::EvidenceRole::ManifestOrDependencyMetadata,
                _ => eggsearch::core::evidence_role::EvidenceRole::CommunityDiscussion,
            },
            absence_kind: match i % 4 {
                0 => EvidenceAbsenceKind::NoMatchingEvidenceFound,
                1 => EvidenceAbsenceKind::ProviderFailed,
                2 => EvidenceAbsenceKind::DeadlinePreventedCompletion,
                _ => EvidenceAbsenceKind::NotApplicable,
            },
            provider_id: Some(format!("provider_{i}")),
            message: format!("dimension {i}"),
            query: Some(format!("query_{i}")),
            ..Default::default()
        })
        .collect();

    c.bench_function("summarize_retrieval_20_dimensions", |b| {
        b.iter_batched(
            || dimensions.clone(),
            |dims| {
                black_box(summarize_retrieval(black_box(dims)));
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

fn bench_attempt_ledger_construction(c: &mut Criterion) {
    use eggsearch::core::retrieval_status::{RetrievalAttempt, RetrievalAttemptOutcome};

    let attempts: Vec<RetrievalAttempt> = (0..50)
        .map(|i| RetrievalAttempt {
            provider_id: format!("provider_{}", i % 5),
            subquery_id: Some(format!("subquery_{}", i % 10)),
            operation_id: None,
            intended_roles: vec![match i % 4 {
                0 => eggsearch::core::evidence_role::EvidenceRole::PrimaryImplementation,
                1 => eggsearch::core::evidence_role::EvidenceRole::AuthoritativeSecurityAdvisory,
                2 => eggsearch::core::evidence_role::EvidenceRole::OfficialDocumentation,
                _ => eggsearch::core::evidence_role::EvidenceRole::CommunityDiscussion,
            }],
            outcome: match i % 6 {
                0 => RetrievalAttemptOutcome::SuccessWithResults,
                1 => RetrievalAttemptOutcome::SuccessZeroResults,
                2 => RetrievalAttemptOutcome::Failed,
                3 => RetrievalAttemptOutcome::TimedOut,
                4 => RetrievalAttemptOutcome::RateLimited,
                _ => RetrievalAttemptOutcome::TruncatedAfterPartialSuccess,
            },
            result_count: i % 10,
            error_class: if i % 3 == 0 {
                Some("test_error".to_string())
            } else {
                None
            },
            deadline_interrupted: i % 7 == 0,
            truncated: i % 5 == 0,
            truncation_evidence: if i % 5 == 0 {
                TruncationEvidence::ConfirmedByEggsearch
            } else {
                TruncationEvidence::None
            },
            query_fingerprint: None,
            duration_ms: Some((i as u64) * 10),
        })
        .collect();

    c.bench_function("attempt_ledger_50_attempts", |b| {
        b.iter_batched(
            || attempts.clone(),
            |atts| {
                black_box(eggsearch::core::retrieval_status::attempts_to_failures(
                    black_box(&atts),
                ));
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

fn bench_build_inventory_100_entries(c: &mut Criterion) {
    use eggsearch::core::code_evidence::SourceRole;
    use eggsearch::meta::local_inventory_cache::{InventoryEntry, RootInventory};
    use std::path::PathBuf;
    use std::time::Instant;

    let entries: Vec<InventoryEntry> = (0..100)
        .map(|i| InventoryEntry {
            root_index: 0,
            relative_path: format!("src/module_{i}.rs"),
            absolute_path: PathBuf::from(format!("/fake/src/module_{i}.rs")),
            size: 1024 + (i * 128) as u64,
            language: Some("rust".to_string()),
            role: SourceRole::Implementation,
            is_binary: false,
            mtime_secs: 1700000000 + i as u64,
            fingerprint: 0,
        })
        .collect();

    c.bench_function("build_inventory_100_entries", |b| {
        b.iter_batched(
            || entries.clone(),
            |ents| {
                let root = RootInventory {
                    root_index: 0,
                    root_path: PathBuf::from("/fake"),
                    entries: ents,
                    built_at: Instant::now(),
                    head_commit: None,
                    entry_count: 100,
                    truncated: false,
                    truncation_reason: None,
                    uses_git_backend: false,
                    untracked_count: 0,
                    index_mtime_secs: None,
                    status_hash: None,
                };
                black_box(root);
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

fn bench_inventory_search_100_entries(c: &mut Criterion) {
    use eggsearch::core::code_evidence::SourceRole;
    use eggsearch::meta::local_inventory_cache::{score_inventory_entry, InventoryEntry};
    use std::path::PathBuf;

    let entries: Vec<InventoryEntry> = (0..100)
        .map(|i| InventoryEntry {
            root_index: 0,
            relative_path: format!("src/module_{i}.rs"),
            absolute_path: PathBuf::from(format!("/fake/src/module_{i}.rs")),
            size: 1024 + (i * 128) as u64,
            language: Some("rust".to_string()),
            role: SourceRole::Implementation,
            is_binary: false,
            mtime_secs: 1700000000 + i as u64,
            fingerprint: 0,
        })
        .collect();

    let query = "module_42";
    let query_lower = query.to_lowercase();
    let query_tokens: Vec<&str> = query_lower.split_whitespace().collect();

    c.bench_function("inventory_search_100_entries", |b| {
        b.iter(|| {
            for entry in &entries {
                black_box(score_inventory_entry(
                    black_box(entry),
                    black_box(&query_lower),
                    black_box(&query_tokens),
                ));
            }
        });
    });
}

fn bench_inventory_search_1000_entries(c: &mut Criterion) {
    use eggsearch::core::code_evidence::SourceRole;
    use eggsearch::meta::local_inventory_cache::{score_inventory_entry, InventoryEntry};
    use std::path::PathBuf;

    let entries: Vec<InventoryEntry> = (0..1000)
        .map(|i| InventoryEntry {
            root_index: 0,
            relative_path: format!("src/module_{i:04}.rs"),
            absolute_path: PathBuf::from(format!("/fake/src/module_{i:04}.rs")),
            size: 1024 + (i * 128) as u64,
            language: Some("rust".to_string()),
            role: SourceRole::Implementation,
            is_binary: false,
            mtime_secs: 1700000000 + i as u64,
            fingerprint: 0,
        })
        .collect();

    let query = "module_0500";
    let query_lower = query.to_lowercase();
    let query_tokens: Vec<&str> = query_lower.split_whitespace().collect();

    c.bench_function("inventory_search_1000_entries", |b| {
        b.iter(|| {
            for entry in &entries {
                black_box(score_inventory_entry(
                    black_box(entry),
                    black_box(&query_lower),
                    black_box(&query_tokens),
                ));
            }
        });
    });
}

fn bench_repo_map_50_entries(c: &mut Criterion) {
    use eggsearch::core::code_metadata::CodeHost;
    use eggsearch::core::repo_map::RepoMapRequest;
    use eggsearch::meta::forge_adapter::{
        build_response, EntryKind, ForgeRawEntry, ForgeTreeResponse, ResolvedRepositoryIdentity,
    };

    let request = RepoMapRequest {
        query: "test".to_string(),
        host: Some(CodeHost::Github),
        owner: "owner".to_string(),
        repo: "test-repo".to_string(),
        ref_name: Some("main".to_string()),
        commit_sha: None,
        max_entries: None,
        max_depth: Some(3),
        include_files: Some(true),
        include_directories: Some(true),
        include_ci: Some(false),
        include_security: Some(false),
        timeout_ms: None,
        providers: vec![],
    };

    c.bench_function("repo_map_50_entries", |b| {
        b.iter(|| {
            let resp = ForgeTreeResponse {
                entries: vec![
                    ForgeRawEntry {
                        path: "src/main.rs".to_string(),
                        kind: EntryKind::File,
                        size: Some(1024),
                        object_sha: None,
                    };
                    50
                ],
                identity: ResolvedRepositoryIdentity {
                    requested_ref: Some("main".to_string()),
                    resolved_ref_name: Some("main".to_string()),
                    resolved_commit_sha: Some("abc123def456".to_string()),
                    tree_sha: Some("tree123".to_string()),
                    default_branch: Some("main".to_string()),
                },
                truncated_by_provider: false,
                warnings: vec![],
                provider_id: "github".to_string(),
                endpoint_origin: Some("https://api.github.com".to_string()),
                response_bytes_observed: 5000,
                response_cap_applied: false,
                dns_policy_class: None,
                aggregate_byte_cap_reached: false,
                aggregate_limit: 10485760,
                aggregate_remaining: 10480760,
                request_count: 1,
                exhausted_by: None,
            };
            black_box(build_response(
                black_box(&request),
                black_box(resp),
                true,
                true,
                false,
                false,
                None,
            ));
        });
    });
}

fn bench_retrieval_summary_50_attempts(c: &mut Criterion) {
    use eggsearch::core::retrieval_status::{RetrievalAttempt, RetrievalAttemptOutcome};

    let attempts: Vec<RetrievalAttempt> = (0..50)
        .map(|i| RetrievalAttempt {
            provider_id: format!("provider_{}", i % 5),
            subquery_id: Some(format!("subquery_{}", i % 10)),
            operation_id: None,
            intended_roles: vec![match i % 4 {
                0 => eggsearch::core::evidence_role::EvidenceRole::PrimaryImplementation,
                1 => eggsearch::core::evidence_role::EvidenceRole::AuthoritativeSecurityAdvisory,
                2 => eggsearch::core::evidence_role::EvidenceRole::OfficialDocumentation,
                _ => eggsearch::core::evidence_role::EvidenceRole::CommunityDiscussion,
            }],
            outcome: match i % 6 {
                0 => RetrievalAttemptOutcome::SuccessWithResults,
                1 => RetrievalAttemptOutcome::SuccessZeroResults,
                2 => RetrievalAttemptOutcome::Failed,
                3 => RetrievalAttemptOutcome::TimedOut,
                4 => RetrievalAttemptOutcome::RateLimited,
                _ => RetrievalAttemptOutcome::TruncatedAfterPartialSuccess,
            },
            result_count: i % 10,
            error_class: if i % 3 == 0 {
                Some("test_error".to_string())
            } else {
                None
            },
            deadline_interrupted: i % 7 == 0,
            truncated: i % 5 == 0,
            truncation_evidence: if i % 5 == 0 {
                TruncationEvidence::ConfirmedByEggsearch
            } else {
                TruncationEvidence::None
            },
            query_fingerprint: None,
            duration_ms: Some((i as u64) * 10),
        })
        .collect();

    c.bench_function("retrieval_summary_50_attempts", |b| {
        b.iter_batched(
            || attempts.clone(),
            |atts| {
                black_box(
                    eggsearch::core::evidence_postprocess::build_retrieval_summary_from_attempts(
                        black_box(&atts),
                    ),
                );
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

fn bench_conflict_detection_20_vuln_cards(c: &mut Criterion) {
    use eggsearch::core::conflict::detect_entity_scoped_conflicts;
    use eggsearch::core::security::VulnerabilityMetadata;
    use eggsearch::core::source_card::{SourceCard, SourceKind, SourceMetadata};

    let cards: Vec<SourceCard> = (0..20)
        .map(|i| {
            let version = format!("{}.0.0", i + 1);
            SourceCard {
                id: format!("vuln_{i:032x}"),
                stable_id: Some(format!("vuln_{i:032x}")),
                title: format!("Advisory for pkg-{}", i % 5),
                url: format!("https://example.com/CVE-2024-{i:04}"),
                providers: vec!["test".to_string()],
                score: Some(1.0),
                trust: eggsearch::core::result::TrustLevel::ExternalUntrusted,
                fetched: false,
                snippet: None,
                trust_markers: eggsearch::core::sanitize::TrustMarkers::default(),
                metadata: SourceMetadata {
                    source_kind: SourceKind::SecurityAdvisory,
                    vulnerability: Some(Box::new(VulnerabilityMetadata {
                        cve_ids: vec![format!("CVE-2024-{i:04}")],
                        ecosystem: Some("npm".to_string()),
                        package: Some(format!("pkg-{}", i % 5)),
                        patched_versions: vec![version],
                        ..Default::default()
                    })),
                    ..Default::default()
                },
                quality: None,
            }
        })
        .collect();

    c.bench_function("conflict_detection_20_vuln_cards", |b| {
        b.iter(|| {
            black_box(detect_entity_scoped_conflicts(black_box(&cards)));
        });
    });
}

fn bench_build_forge_response_50_entries(c: &mut Criterion) {
    use eggsearch::core::code_metadata::CodeHost;
    use eggsearch::core::repo_map::RepoMapRequest;
    use eggsearch::meta::forge_adapter::{
        build_response, EntryKind, ForgeRawEntry, ForgeTreeResponse, ResolvedRepositoryIdentity,
    };

    let request = RepoMapRequest {
        query: "test".to_string(),
        host: Some(CodeHost::Github),
        owner: "owner".to_string(),
        repo: "test-repo".to_string(),
        ref_name: Some("main".to_string()),
        commit_sha: None,
        max_entries: None,
        max_depth: Some(3),
        include_files: Some(true),
        include_directories: Some(true),
        include_ci: Some(false),
        include_security: Some(false),
        timeout_ms: None,
        providers: vec![],
    };

    c.bench_function("build_forge_response_50_entries", |b| {
        b.iter(|| {
            let resp = ForgeTreeResponse {
                entries: vec![
                    ForgeRawEntry {
                        path: "src/main.rs".to_string(),
                        kind: EntryKind::File,
                        size: Some(1024),
                        object_sha: None,
                    };
                    50
                ],
                identity: ResolvedRepositoryIdentity {
                    requested_ref: Some("main".to_string()),
                    resolved_ref_name: Some("main".to_string()),
                    resolved_commit_sha: Some("abc123def456".to_string()),
                    tree_sha: Some("tree123".to_string()),
                    default_branch: Some("main".to_string()),
                },
                truncated_by_provider: false,
                warnings: vec![],
                provider_id: "github".to_string(),
                endpoint_origin: Some("https://api.github.com".to_string()),
                response_bytes_observed: 5000,
                response_cap_applied: false,
                dns_policy_class: None,
                aggregate_byte_cap_reached: false,
                aggregate_limit: 10485760,
                aggregate_remaining: 10480760,
                request_count: 1,
                exhausted_by: None,
            };
            black_box(build_response(
                black_box(&request),
                black_box(resp),
                true,
                true,
                false,
                false,
                None,
            ));
        });
    });
}

fn bench_build_forge_response_200_entries(c: &mut Criterion) {
    use eggsearch::core::code_metadata::CodeHost;
    use eggsearch::core::repo_map::RepoMapRequest;
    use eggsearch::meta::forge_adapter::{
        build_response, EntryKind, ForgeRawEntry, ForgeTreeResponse, ResolvedRepositoryIdentity,
    };

    let request = RepoMapRequest {
        query: "test".to_string(),
        host: Some(CodeHost::Github),
        owner: "owner".to_string(),
        repo: "test-repo".to_string(),
        ref_name: Some("main".to_string()),
        commit_sha: None,
        max_entries: None,
        max_depth: Some(5),
        include_files: Some(true),
        include_directories: Some(true),
        include_ci: Some(false),
        include_security: Some(false),
        timeout_ms: None,
        providers: vec![],
    };

    c.bench_function("build_forge_response_200_entries", |b| {
        b.iter(|| {
            let resp = ForgeTreeResponse {
                entries: (0..200)
                    .map(|i| ForgeRawEntry {
                        path: format!("src/module_{i:04}.rs"),
                        kind: EntryKind::File,
                        size: Some(1024 + i as u64),
                        object_sha: None,
                    })
                    .collect(),
                identity: ResolvedRepositoryIdentity {
                    requested_ref: Some("main".to_string()),
                    resolved_ref_name: Some("main".to_string()),
                    resolved_commit_sha: Some("abc123def456".to_string()),
                    tree_sha: Some("tree123".to_string()),
                    default_branch: Some("main".to_string()),
                },
                truncated_by_provider: false,
                warnings: vec![],
                provider_id: "github".to_string(),
                endpoint_origin: Some("https://api.github.com".to_string()),
                response_bytes_observed: 50000,
                response_cap_applied: false,
                dns_policy_class: None,
                aggregate_byte_cap_reached: false,
                aggregate_limit: 10485760,
                aggregate_remaining: 10435760,
                request_count: 3,
                exhausted_by: None,
            };
            black_box(build_response(
                black_box(&request),
                black_box(resp),
                true,
                true,
                false,
                false,
                None,
            ));
        });
    });
}

fn bench_inventory_search_near_cap(c: &mut Criterion) {
    use eggsearch::core::code_evidence::SourceRole;
    use eggsearch::meta::local_inventory_cache::{score_inventory_entry, InventoryEntry};
    use std::path::PathBuf;

    let cap = 4096;
    let entries: Vec<InventoryEntry> = (0..cap)
        .map(|i| InventoryEntry {
            root_index: 0,
            relative_path: format!("src/module_{i:04}.rs"),
            absolute_path: PathBuf::from(format!("/fake/src/module_{i:04}.rs")),
            size: 1024 + (i * 128) as u64,
            language: Some("rust".to_string()),
            role: SourceRole::Implementation,
            is_binary: false,
            mtime_secs: 1700000000 + i as u64,
            fingerprint: 0,
        })
        .collect();

    let query = "module_2048";
    let query_lower = query.to_lowercase();
    let query_tokens: Vec<&str> = query_lower.split_whitespace().collect();

    c.bench_function("inventory_search_near_cap_4096", |b| {
        b.iter(|| {
            for entry in &entries {
                black_box(score_inventory_entry(
                    black_box(entry),
                    black_box(&query_lower),
                    black_box(&query_tokens),
                ));
            }
        });
    });
}

fn bench_capability_partition(c: &mut Criterion) {
    use eggsearch::core::evidence_role::EvidenceRole;
    use eggsearch::meta::dispatch::partition_roles_for_engine;
    use eggsearch::meta::engines::{error::EngineError, models::SearchResult};
    use eggsearch::meta::engines::{BoxFuture, SearchEngine};

    struct BenchEngine;

    impl SearchEngine for BenchEngine {
        fn name(&self) -> &'static str {
            "bench"
        }

        fn search<'a>(
            &'a self,
            _request: &'a eggsearch::meta::engines::EngineSearchRequest,
        ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
            Box::pin(async { Ok(Vec::new()) })
        }

        fn supports_role(&self, role: &EvidenceRole) -> bool {
            !matches!(role, EvidenceRole::CommunityDiscussion)
        }
    }

    let engine = BenchEngine;
    let role_cycle = [
        EvidenceRole::PrimaryImplementation,
        EvidenceRole::OfficialDocumentation,
        EvidenceRole::ManifestOrDependencyMetadata,
        EvidenceRole::CommunityDiscussion,
    ];
    let mut group = c.benchmark_group("capability_partition");
    for size in [1usize, 4, 16, 64] {
        let roles: Vec<_> = (0..size)
            .map(|i| role_cycle[i % role_cycle.len()])
            .collect();
        group.bench_function(format!("{size}_roles"), |b| {
            b.iter(|| {
                black_box(partition_roles_for_engine(&engine, black_box(&roles)));
            });
        });
    }
    group.finish();
}

fn bench_mixed_retrieval_summary(c: &mut Criterion) {
    use eggsearch::core::evidence_role::EvidenceRole;
    use eggsearch::core::retrieval_status::{RetrievalAttempt, RetrievalAttemptOutcome};

    let outcomes = [
        RetrievalAttemptOutcome::SuccessWithResults,
        RetrievalAttemptOutcome::SuccessZeroResults,
        RetrievalAttemptOutcome::SkippedCapabilityUnavailable,
        RetrievalAttemptOutcome::SkippedByPolicy,
        RetrievalAttemptOutcome::Failed,
        RetrievalAttemptOutcome::InterruptedByDeadline,
        RetrievalAttemptOutcome::SuccessWithResults,
    ];
    let attempts: Vec<RetrievalAttempt> = outcomes
        .into_iter()
        .enumerate()
        .map(|(index, outcome)| RetrievalAttempt {
            provider_id: format!("provider_{}", index % 4),
            subquery_id: Some(format!("advisory_{index}")),
            operation_id: None,
            intended_roles: vec![EvidenceRole::AuthoritativeSecurityAdvisory],
            outcome,
            result_count: if index == 0 { 1 } else { 0 },
            error_class: None,
            deadline_interrupted: index == 5,
            truncated: index == 6,
            truncation_evidence: if index == 6 {
                TruncationEvidence::ConfirmedByProvider
            } else {
                TruncationEvidence::None
            },
            query_fingerprint: None,
            duration_ms: Some(index as u64),
        })
        .collect();

    c.bench_function("retrieval_summary_mixed_outcomes", |b| {
        b.iter_batched(
            || attempts.clone(),
            |attempts| {
                black_box(
                    eggsearch::core::evidence_postprocess::build_retrieval_summary_from_attempts(
                        black_box(&attempts),
                    ),
                );
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

fn bench_provider_scoped_advisory_conversion(c: &mut Criterion) {
    use eggsearch::core::evidence_role::EvidenceRole;
    use eggsearch::core::retrieval_status::{RetrievalAttempt, RetrievalAttemptOutcome};

    let mut group = c.benchmark_group("provider_scoped_advisory_conversion");
    for provider_count in [1usize, 4, 8, 16] {
        group.bench_function(format!("{provider_count}_providers"), |b| {
            b.iter_batched(
                || {
                    (0..provider_count)
                        .map(|index| RetrievalAttempt {
                            provider_id: format!("advisory_{index}"),
                            subquery_id: Some("advisory_by_cve".to_string()),
            operation_id: None,
                            intended_roles: vec![
                                EvidenceRole::AuthoritativeSecurityAdvisory,
                            ],
                            outcome: if index % 4 == 0 {
                                RetrievalAttemptOutcome::SuccessZeroResults
                            } else if index % 4 == 1 {
                                RetrievalAttemptOutcome::SuccessWithResults
                            } else if index % 4 == 2 {
                                RetrievalAttemptOutcome::Failed
                            } else {
                                RetrievalAttemptOutcome::SkippedCapabilityUnavailable
                            },
                            result_count: if index % 4 == 1 { 1 } else { 0 },
                            error_class: None,
                            deadline_interrupted: false,
                            truncated: false,
                            truncation_evidence: TruncationEvidence::None,
                            query_fingerprint: None,
                            duration_ms: Some(index as u64),
                        })
                        .collect::<Vec<_>>()
                },
                |attempts| {
                    black_box(
                        eggsearch::core::evidence_postprocess::build_retrieval_summary_from_attempts(
                            black_box(&attempts),
                        ),
                    );
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
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
    bench_summarize_retrieval_20_dimensions,
    bench_attempt_ledger_construction,
    bench_build_inventory_100_entries,
    bench_inventory_search_100_entries,
    bench_inventory_search_1000_entries,
    bench_repo_map_50_entries,
    bench_retrieval_summary_50_attempts,
    bench_conflict_detection_20_vuln_cards,
    bench_build_forge_response_50_entries,
    bench_build_forge_response_200_entries,
    bench_inventory_search_near_cap,
    bench_capability_partition,
    bench_mixed_retrieval_summary,
    bench_provider_scoped_advisory_conversion,
);
criterion_main!(benches);
