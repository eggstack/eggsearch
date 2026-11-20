#![cfg(feature = "mock")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use eggsearch::core::config::AppConfig;
use eggsearch::core::knowledge_delta::{parse_caller_override, parse_refresh, CutoffOverrideInput};
use eggsearch::mcp::projection::{project_with_limits, ProjectionLimits, ResponseDetail};
use eggsearch::mcp::state::ServerState;
use eggsearch::mcp::tools::{run_knowledge_brief, KnowledgeBriefArgs};
use eggsearch::meta::adapter::MetadataSearchAdapter;
use eggsearch::meta::knowledge_delta::{
    run_knowledge_brief as run_meta_brief, BriefQuery, BriefRun, EvidenceFetchFuture,
    EvidenceIndex, FetchedEvidence, KnowledgeEvidenceFetcher,
};
use eggsearch::meta::mock::{mock_engines, MockEngine, MockFailure, MockResult};

fn state(engines: Vec<MockEngine>, cfg: AppConfig) -> Arc<ServerState> {
    let adapter =
        MetadataSearchAdapter::from_engines(mock_engines(engines), Duration::from_secs(5));
    Arc::new(ServerState::with_adapter(cfg, Arc::new(adapter)))
}

fn dated(url: &str, published_at: &str) -> MockResult {
    let mut result = MockResult::new("widget-runtime notes", url, "mock");
    result.snippet = Some("compatibility notes for widget-runtime".into());
    result.published_at = Some(published_at.into());
    result
}

fn override_day(date: &str) -> CutoffOverrideInput {
    CutoffOverrideInput {
        date: date.into(),
        precision: "day".into(),
        source: "caller".into(),
        reason: "test cutoff".into(),
    }
}

fn brief(topic: &str, cutoff: Option<CutoffOverrideInput>) -> KnowledgeBriefArgs {
    KnowledgeBriefArgs {
        topic: topic.into(),
        cutoff_override: cutoff,
        ..KnowledgeBriefArgs::default()
    }
}

struct FixedFetcher {
    calls: AtomicUsize,
    cacheable: bool,
}

impl KnowledgeEvidenceFetcher for FixedFetcher {
    fn fetch<'a>(&'a self, _url: &'a str, _topic: &'a str) -> EvidenceFetchFuture<'a> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        let cacheable = self.cacheable;
        Box::pin(async move {
            Ok(FetchedEvidence {
                excerpt: "fetched public evidence passage".into(),
                source_title: Some("Fetched source title".into()),
                spans: vec![serde_json::json!({
                    "kind": "document_chunk",
                    "chunk_id": "chunk_fixture",
                    "block_start": 0,
                    "block_end": 0,
                })],
                cacheable,
                source_truncated: false,
            })
        })
    }
}

#[tokio::test]
async fn after_cutoff_publication_is_emitted_and_labeled() {
    let engines = vec![MockEngine::success(
        "mock",
        vec![dated("https://example.com/new", "2024-06-02")],
    )];
    let value = run_knowledge_brief(
        state(engines, AppConfig::default()),
        brief("widget-runtime", Some(override_day("2024-06-01"))),
    )
    .await
    .expect("brief");
    let deltas = value["deltas"].as_array().expect("deltas");
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0]["date_basis"], "publication");
    assert_eq!(deltas[0]["temporal_relationship"], "after_cutoff");
    assert!(deltas[0]["id"].as_str().unwrap().starts_with("kdelta_"));
    let tools: Vec<_> = value["next_actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["tool"].as_str().unwrap())
        .collect();
    assert!(tools
        .iter()
        .all(|tool| *tool == "fetch" || *tool == "research_search"));
    assert_eq!(value["coverage"]["index_status"], "not_opened");
    let projected = project_with_limits(
        "knowledge_brief",
        value.clone(),
        ResponseDetail::Brief,
        ProjectionLimits::default(),
    );
    assert!(projected.get("deltas").is_some());
    assert!(projected.get("model_resolution").is_some());
}

#[tokio::test]
async fn same_day_and_unknown_model_make_no_temporal_claim() {
    let engines = vec![MockEngine::success(
        "mock",
        vec![dated("https://example.com/same", "2024-06-01")],
    )];
    let same = run_knowledge_brief(
        state(engines, AppConfig::default()),
        brief("widget-runtime", Some(override_day("2024-06-01"))),
    )
    .await
    .expect("same day");
    assert!(same["deltas"].as_array().unwrap().is_empty());

    let unknown = run_knowledge_brief(
        state(
            vec![MockEngine::success(
                "mock",
                vec![dated("https://example.com/new", "2024-06-02")],
            )],
            AppConfig::default(),
        ),
        KnowledgeBriefArgs {
            topic: "widget-runtime".into(),
            model_id: Some("unknown/model".into()),
            ..KnowledgeBriefArgs::default()
        },
    )
    .await
    .expect("unknown model is not an error");
    assert_eq!(unknown["model_resolution"]["status"], "unknown");
    assert!(unknown["deltas"].as_array().unwrap().is_empty());
    assert_eq!(unknown["coverage"]["no_temporal_claim"], true);
}

#[tokio::test]
async fn malformed_override_and_total_failure_are_explicit() {
    let bad = run_knowledge_brief(
        state(
            vec![MockEngine::success("mock", vec![])],
            AppConfig::default(),
        ),
        brief(
            "widget-runtime",
            Some(CutoffOverrideInput {
                date: "2024-06-01".into(),
                precision: "day".into(),
                source: "provider-official".into(),
                reason: "must not pass".into(),
            }),
        ),
    )
    .await;
    assert!(
        bad.is_err(),
        "provider-official caller override must be rejected"
    );

    let failed = run_knowledge_brief(
        state(
            vec![MockEngine::failure("mock", MockFailure::Network)],
            AppConfig::default(),
        ),
        brief("widget-runtime", Some(override_day("2024-06-01"))),
    )
    .await;
    assert!(
        failed.is_err(),
        "total provider failure is not an empty success"
    );
}

#[tokio::test]
async fn retain_uses_fetched_public_evidence_and_index_mode_is_offline() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let mut cfg = AppConfig::default();
    cfg.knowledge_delta.index.enabled = true;
    cfg.knowledge_delta.index.path = Some(path.to_string_lossy().to_string());
    let server = state(
        vec![MockEngine::success(
            "mock",
            vec![dated("https://example.com/new", "2024-06-02")],
        )],
        cfg.clone(),
    );
    let fetcher = FixedFetcher {
        calls: AtomicUsize::new(0),
        cacheable: true,
    };
    let cutoff = parse_caller_override(&override_day("2024-06-01")).unwrap();
    let query = BriefQuery {
        topic: "widget-runtime".into(),
        model_id: None,
        caller_cutoff: Some(cutoff),
        max_items: 8,
        max_chars: 6000,
        refresh: parse_refresh(None).unwrap(),
        retain: true,
        related_topics: false,
        language: None,
        partition: "anonymous".into(),
    };
    let live = run_meta_brief(
        &BriefRun {
            adapter: server.adapter.as_ref(),
            config: &cfg,
            max_results_cap: cfg.search.max_results_cap,
            config_dir: dir.path().to_path_buf(),
            evidence_fetcher: Some(&fetcher),
        },
        &query,
    )
    .await
    .expect("live retain");
    assert_eq!(live["coverage"]["fetch_invoked"], true);
    assert_eq!(live["coverage"]["fetch_completed"], 1);
    assert_eq!(fetcher.calls.load(Ordering::Relaxed), 1);
    assert_eq!(
        live["deltas"][0]["excerpt"],
        "fetched public evidence passage"
    );
    assert!(!live["deltas"][0]["spans"].as_array().unwrap().is_empty());
    assert!(path.exists());

    let index = EvidenceIndex::open(&path, &cfg.knowledge_delta.index).unwrap();
    let hits = index.lookup("anonymous", "widget-runtime", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].record.title, "Fetched source title");
    assert_eq!(hits[0].record.excerpt, "fetched public evidence passage");
    drop(index);

    let offline = state(
        vec![MockEngine::failure("mock", MockFailure::Network)],
        cfg.clone(),
    );
    let indexed = run_meta_brief(
        &BriefRun {
            adapter: offline.adapter.as_ref(),
            config: &cfg,
            max_results_cap: cfg.search.max_results_cap,
            config_dir: dir.path().to_path_buf(),
            evidence_fetcher: None,
        },
        &BriefQuery {
            refresh: parse_refresh(Some("index")).unwrap(),
            retain: false,
            ..query
        },
    )
    .await
    .expect("index mode does not require live retrieval");
    assert_eq!(indexed["coverage"]["index_status"], "open");
    assert_eq!(indexed["deltas"][0]["provenance"], "index_only");
    assert_eq!(indexed["deltas"][0]["precision"], "day");
}

#[tokio::test]
async fn live_call_does_not_open_the_index_and_disabled_retain_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let mut cfg = AppConfig::default();
    cfg.knowledge_delta.index.enabled = true;
    cfg.knowledge_delta.index.path = Some(path.to_string_lossy().to_string());
    let live = run_knowledge_brief(
        state(
            vec![MockEngine::success(
                "mock",
                vec![dated("https://example.com/new", "2024-06-02")],
            )],
            cfg,
        ),
        brief("widget-runtime", Some(override_day("2024-06-01"))),
    )
    .await
    .expect("live");
    assert_eq!(live["coverage"]["index_status"], "not_opened");
    assert_eq!(live["coverage"]["fetch_invoked"], true);
    assert_eq!(live["coverage"]["fetch_attempted"], 1);
    assert_eq!(live["coverage"]["fetch_completed"], 0);
    assert!(!path.exists());

    let rejected = run_knowledge_brief(
        state(
            vec![MockEngine::success("mock", vec![])],
            AppConfig::default(),
        ),
        KnowledgeBriefArgs {
            retain: true,
            ..brief("widget-runtime", Some(override_day("2024-06-01")))
        },
    )
    .await;
    assert!(
        rejected.is_err(),
        "retain without an enabled index must be rejected"
    );
}

#[tokio::test]
async fn retain_skips_fetched_evidence_not_marked_cacheable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let mut cfg = AppConfig::default();
    cfg.knowledge_delta.index.enabled = true;
    cfg.knowledge_delta.index.path = Some(path.to_string_lossy().to_string());
    let server = state(
        vec![MockEngine::success(
            "mock",
            vec![dated("https://example.com/new", "2024-06-02")],
        )],
        cfg.clone(),
    );
    let fetcher = FixedFetcher {
        calls: AtomicUsize::new(0),
        cacheable: false,
    };
    let query = BriefQuery {
        topic: "widget-runtime".into(),
        model_id: None,
        caller_cutoff: Some(parse_caller_override(&override_day("2024-06-01")).unwrap()),
        max_items: 8,
        max_chars: 6000,
        refresh: parse_refresh(None).unwrap(),
        retain: true,
        related_topics: false,
        language: None,
        partition: "anonymous".into(),
    };
    let live = run_meta_brief(
        &BriefRun {
            adapter: server.adapter.as_ref(),
            config: &cfg,
            max_results_cap: cfg.search.max_results_cap,
            config_dir: dir.path().to_path_buf(),
            evidence_fetcher: Some(&fetcher),
        },
        &query,
    )
    .await
    .expect("live brief with non-cacheable fetch");
    assert_eq!(live["coverage"]["fetch_completed"], 1);
    assert_eq!(fetcher.calls.load(Ordering::Relaxed), 1);
    assert_eq!(
        live["deltas"][0]["excerpt"],
        "fetched public evidence passage"
    );
    let index = EvidenceIndex::open(&path, &cfg.knowledge_delta.index).unwrap();
    assert!(index
        .lookup("anonymous", "widget-runtime", 10)
        .unwrap()
        .is_empty());
}
