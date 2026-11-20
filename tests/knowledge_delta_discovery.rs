#![cfg(feature = "mock")]

use std::sync::Arc;
use std::time::Duration;

use eggsearch::core::config::AppConfig;
use eggsearch::core::identity::knowledge_delta_id;
use eggsearch::core::knowledge_delta::{CutoffOverrideInput, KnowledgeIndexConfig};
use eggsearch::mcp::state::ServerState;
use eggsearch::mcp::tools::{run_knowledge_brief, KnowledgeBriefArgs};
use eggsearch::meta::adapter::MetadataSearchAdapter;
use eggsearch::meta::knowledge_delta::{subject_id, EvidenceIndex, IndexRecord};
use eggsearch::meta::mock::{mock_engines, MockEngine, MockResult};
use proptest::prelude::*;
use rusqlite::Connection;

fn index_config(path: &std::path::Path) -> KnowledgeIndexConfig {
    KnowledgeIndexConfig {
        enabled: true,
        path: Some(path.to_string_lossy().to_string()),
        max_bytes: 1024 * 1024,
        max_documents: 20,
        max_excerpt_chars: 200,
        retention_days: 365,
    }
}

fn record(id: &str, url: &str, topic: &str, excerpt: &str, subject: &str) -> IndexRecord {
    IndexRecord {
        partition: "anonymous".into(),
        canonical_url: url.into(),
        source_id: format!("src_{id}"),
        title: topic.into(),
        excerpt: excerpt.into(),
        trust: "external_untrusted".into(),
        basis: "publication".into(),
        precision: "day".into(),
        normalized_date: Some("2024-07-02".into()),
        kdelta_id: knowledge_delta_id(url, "publication", "day", "2024-07-02"),
        conflicted: false,
        temporal_relationship: "after_cutoff".into(),
        topic: topic.into(),
        primary_subject: Some(subject.into()),
    }
}

fn state(cfg: AppConfig) -> Arc<ServerState> {
    state_with_results(cfg, vec![])
}

fn state_with_results(cfg: AppConfig, results: Vec<MockResult>) -> Arc<ServerState> {
    let adapter = MetadataSearchAdapter::from_engines(
        mock_engines(vec![MockEngine::success("mock", results)]),
        Duration::from_secs(5),
    );
    Arc::new(ServerState::with_adapter(cfg, Arc::new(adapter)))
}

fn args(related: bool) -> KnowledgeBriefArgs {
    KnowledgeBriefArgs {
        topic: "widget-runtime".into(),
        cutoff_override: Some(CutoffOverrideInput {
            date: "2024-06-01".into(),
            precision: "day".into(),
            source: "caller".into(),
            reason: "fixture cutoff".into(),
        }),
        related_topics: related,
        ..KnowledgeBriefArgs::default()
    }
}

#[tokio::test]
async fn omitted_name_is_found_only_through_a_source_backed_edge() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let index = EvidenceIndex::open(&path, &index_config(&path)).unwrap();
    let neighbor = record(
        "neighbor",
        "https://example.com/left-pad",
        "left-pad",
        "wheels published after the cutoff",
        "left-pad",
    );
    let expected = neighbor.kdelta_id.clone();
    index
        .retain(&[
            record(
                "bridge",
                "https://example.com/bridge",
                "widget-runtime",
                "see `left-pad` wheels",
                "widget-runtime",
            ),
            neighbor,
        ])
        .unwrap();
    drop(index);

    let mut cfg = AppConfig::default();
    cfg.knowledge_delta.index = index_config(&path);
    let direct = run_knowledge_brief(state(cfg.clone()), args(false))
        .await
        .expect("direct");
    let direct_ids: Vec<_> = direct["deltas"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_string())
        .collect();
    assert!(!direct_ids.contains(&expected));
    assert_eq!(direct["coverage"]["expansion"], "not_requested");

    let expanded = run_knowledge_brief(state(cfg), args(true))
        .await
        .expect("expanded");
    let hit = expanded["deltas"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == expected)
        .expect("expanded delta");
    assert_eq!(hit["expanded"], true);
    assert!(!hit["edge_source_ids"].as_array().unwrap().is_empty());
    let spans = hit["spans"].as_array().unwrap();
    assert!(!spans.is_empty());
    assert_eq!(spans[0]["source_id"], hit["edge_source_ids"][0]);
    assert!(spans[0]["span"].as_str().unwrap().starts_with("excerpt:"));
    assert_eq!(expanded["coverage"]["expansion"], "added");
}

#[tokio::test]
async fn missing_edge_adds_no_expanded_delta() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let index = EvidenceIndex::open(&path, &index_config(&path)).unwrap();
    index
        .retain(&[record(
            "orphan",
            "https://example.com/left-pad",
            "left-pad",
            "an indexed event without a bridge",
            "left-pad",
        )])
        .unwrap();
    drop(index);
    let mut cfg = AppConfig::default();
    cfg.knowledge_delta.index = index_config(&path);
    let value = run_knowledge_brief(state(cfg), args(true))
        .await
        .expect("no edge");
    assert!(value["deltas"].as_array().unwrap().is_empty());
    assert_eq!(value["coverage"]["expansion"], "no_new_evidence");
}

#[tokio::test]
async fn generic_text_and_missing_edge_add_no_expanded_delta() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let index = EvidenceIndex::open(&path, &index_config(&path)).unwrap();
    index
        .retain(&[record(
            "generic",
            "https://example.com/generic",
            "widget-runtime",
            "release compatibility update",
            "widget-runtime",
        )])
        .unwrap();
    drop(index);
    let mut cfg = AppConfig::default();
    cfg.knowledge_delta.index = index_config(&path);
    let value = run_knowledge_brief(state(cfg), args(true))
        .await
        .expect("no edge");
    assert!(value["deltas"].as_array().unwrap().is_empty());
    assert_eq!(value["coverage"]["expansion"], "no_new_evidence");
}

#[tokio::test]
async fn unchanged_direct_topic_result_is_not_counted_as_expansion() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let index = EvidenceIndex::open(&path, &index_config(&path)).unwrap();
    let neighbor = record(
        "neighbor",
        "https://example.com/left-pad",
        "left-pad",
        "wheels published after the cutoff",
        "left-pad",
    );
    let expected = neighbor.kdelta_id.clone();
    index
        .retain(&[
            record(
                "bridge",
                "https://example.com/bridge",
                "widget-runtime",
                "pkg:left-pad is compatible",
                "widget-runtime",
            ),
            neighbor,
        ])
        .unwrap();
    drop(index);

    let mut cfg = AppConfig::default();
    cfg.knowledge_delta.index = index_config(&path);
    let mut result = MockResult::new("left-pad release", "https://example.com/left-pad", "mock");
    result.snippet = Some("wheels published after the cutoff".into());
    result.published_at = Some("2024-07-02".into());
    let value = run_knowledge_brief(state_with_results(cfg, vec![result]), args(true))
        .await
        .expect("direct result remains available");
    let deltas = value["deltas"].as_array().unwrap();
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0]["id"], expected);
    assert_eq!(deltas[0]["expanded"], false);
    assert_eq!(value["coverage"]["expansion"], "no_new_evidence");
}

#[tokio::test]
async fn longer_identifier_prefix_does_not_alias_the_exact_topic() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let index = EvidenceIndex::open(&path, &index_config(&path)).unwrap();
    index
        .retain(&[
            record(
                "bridge",
                "https://example.com/bridge",
                "widget-runtime",
                "pkg:widget-runtime-extra has a separate identifier",
                "widget-runtime",
            ),
            record(
                "same-topic",
                "https://example.com/same-topic",
                "widget-runtime",
                "indexed evidence for the unchanged topic",
                "widget-runtime",
            ),
        ])
        .unwrap();
    drop(index);
    let mut cfg = AppConfig::default();
    cfg.knowledge_delta.index = index_config(&path);
    let value = run_knowledge_brief(state(cfg), args(true))
        .await
        .expect("exact topic does not match a longer token");
    assert!(value["deltas"].as_array().unwrap().is_empty());
    assert_eq!(value["coverage"]["expansion"], "no_new_evidence");
}

fn expanded_edge_ids(path: &std::path::Path) -> Vec<String> {
    let conn = Connection::open(path).unwrap();
    let mut statement = conn
        .prepare(
            "SELECT edge_id FROM subject_evidence_edges WHERE partition = ?1 AND from_subject = ?2 ORDER BY edge_id",
        )
        .unwrap();
    statement
        .query_map(["anonymous", "widget-runtime"], |row| row.get(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(8))]

    #[test]
    fn subject_identity_is_stable_and_distinguishes_exact_tokens(
        token in "[a-z]{3,12}",
        suffix in "[a-z]{1,8}"
    ) {
        let longer = format!("{token}-{suffix}");
        prop_assert_eq!(subject_id("package", &token), subject_id("package", &token));
        prop_assert_ne!(subject_id("package", &token), subject_id("package", &longer));
        prop_assert_ne!(subject_id("package", &token), subject_id("repository", &token));
    }

    #[test]
    fn expansion_fanout_stays_bounded(edge_count in 33usize..80) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.sqlite");
        let mut cfg = index_config(&path);
        cfg.max_bytes = 8 * 1024 * 1024;
        cfg.max_documents = 256;
        cfg.max_excerpt_chars = 256;
        let index = EvidenceIndex::open(&path, &cfg).unwrap();
        let mut records = Vec::new();
        for number in 0..edge_count {
            let token = format!("package-{number:03}");
            records.push(record(
                &format!("bridge-{number:03}"),
                &format!("https://example.com/bridge/{number}"),
                "widget-runtime",
                &format!("pkg:{token} update"),
                "widget-runtime",
            ));
            records.push(record(
                &format!("target-{number:03}"),
                &format!("https://example.com/target/{number}"),
                &token,
                "indexed candidate event",
                &token,
            ));
        }
        index.retain(&records).unwrap();
        let hits = index
            .expand("anonymous", &["widget-runtime".into()], 128)
            .unwrap();
        prop_assert!(hits.len() <= 32);
        prop_assert_eq!(hits.len(), 32);
        prop_assert!(hits.iter().all(|hit| hit.expanded));
        prop_assert!(hits.iter().all(|hit| !hit.edge_source_ids.is_empty()));
    }

    #[test]
    fn edge_pruning_is_independent_of_insertion_order(edge_count in 65usize..90) {
        let dir = tempfile::tempdir().unwrap();
        let forward_path = dir.path().join("forward.sqlite");
        let reverse_path = dir.path().join("reverse.sqlite");
        let mut cfg = index_config(&forward_path);
        cfg.max_bytes = 8 * 1024 * 1024;
        cfg.max_documents = 256;
        cfg.max_excerpt_chars = 256;
        let mut records = Vec::new();
        for number in 0..edge_count {
            let token = format!("package-{number:03}");
            records.push(record(
                &format!("bridge-{number:03}"),
                &format!("https://example.com/bridge/{number}"),
                "widget-runtime",
                &format!("pkg:{token} update"),
                "widget-runtime",
            ));
        }
        let forward = EvidenceIndex::open(&forward_path, &cfg).unwrap();
        forward.retain(&records).unwrap();
        drop(forward);
        let reverse = EvidenceIndex::open(&reverse_path, &cfg).unwrap();
        records.reverse();
        reverse.retain(&records).unwrap();
        drop(reverse);
        let forward_edges = expanded_edge_ids(&forward_path);
        let reverse_edges = expanded_edge_ids(&reverse_path);
        prop_assert_eq!(forward_edges.len(), 64);
        prop_assert_eq!(forward_edges, reverse_edges);
    }
}
