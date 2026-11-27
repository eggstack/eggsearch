#![cfg(feature = "mock")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use eggsearch::core::config::AppConfig;
use eggsearch::core::knowledge_delta::{parse_caller_override, parse_refresh, CutoffOverrideInput};
use eggsearch::mcp::projection::{project_with_limits, ProjectionLimits, ResponseDetail};
use eggsearch::mcp::state::ServerState;
use eggsearch::mcp::tools::{map_tool_result, run_knowledge_brief, KnowledgeBriefArgs};
use eggsearch::meta::adapter::MetadataSearchAdapter;
use eggsearch::meta::knowledge_delta::{
    run_knowledge_brief as run_meta_brief, BriefQuery, BriefRun, EvidenceFetchFuture,
    EvidenceIndex, FetchedEvidence, IndexRecord, KnowledgeEvidenceFetcher,
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

fn undated(url: &str) -> MockResult {
    MockResult::new("widget-runtime notes", url, "duckduckgo")
        .with_snippet("compatibility notes for widget-runtime")
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

#[tokio::test]
async fn cutoff_near_miss_fields_return_repairable_cutoff_override_error() {
    for field in ["cutoff", "knowledge_cutoff", "cutoff_date"] {
        let mut raw = serde_json::json!({"topic": "widget-runtime"});
        raw[field] = serde_json::json!("2024-06-01");
        let args = serde_json::from_value::<KnowledgeBriefArgs>(raw).expect("MCP args");
        let error = run_knowledge_brief(state(Vec::new(), AppConfig::default()), args)
            .await
            .expect_err("near-miss cutoff field must fail closed");
        assert_eq!(error.code().as_str(), "invalid_semantic_value");
        let result = map_tool_result(Err(error)).expect("repairable MCP tool error");
        assert_eq!(result.is_error, Some(true));
        let payload = result.structured_content.expect("error payload");
        assert_eq!(payload["code"], "invalid_semantic_value");
        assert!(payload["message"].as_str().unwrap().contains(field));
        assert_eq!(payload["repair"]["field"], "cutoff_override");
        assert_eq!(
            payload["repair"]["accepted"],
            serde_json::json!(["date", "precision", "source", "reason"])
        );
    }

    let valid = serde_json::from_value::<KnowledgeBriefArgs>(serde_json::json!({
        "topic": "widget-runtime",
        "cutoff_override": {
            "date": "2024-06-01",
            "precision": "day",
            "source": "caller",
            "reason": "test cutoff"
        }
    }))
    .expect("valid cutoff_override remains accepted");
    assert_eq!(valid.cutoff_override.unwrap().date, "2024-06-01");
}

#[tokio::test]
async fn null_cutoff_fields_are_treated_as_absent() {
    for field in ["cutoff", "knowledge_cutoff", "cutoff_date"] {
        let mut raw = serde_json::json!({"topic": "widget-runtime"});
        raw[field] = serde_json::Value::Null;
        let args = serde_json::from_value::<KnowledgeBriefArgs>(raw)
            .expect("a present null must deserialize");
        assert!(args.cutoff_field_near_miss.is_none());
        if let Err(error) = run_knowledge_brief(state(Vec::new(), AppConfig::default()), args).await
        {
            assert_ne!(
                error.code().as_str(),
                "invalid_semantic_value",
                "{field}: null must not produce a near-miss rejection"
            );
        }
    }
}

#[test]
fn null_cutoff_with_override_applies_the_override() {
    let args = serde_json::from_value::<KnowledgeBriefArgs>(serde_json::json!({
        "topic": "widget-runtime",
        "cutoff": null,
        "cutoff_override": {
            "date": "2024-06-01",
            "precision": "day",
            "source": "caller",
            "reason": "test cutoff"
        }
    }))
    .expect("null cutoff alongside a valid override");
    assert!(args.cutoff_field_near_miss.is_none());
    let cutoff = args.cutoff_override.expect("override retained");
    assert_eq!(cutoff.date, "2024-06-01");
    assert!(parse_caller_override(&cutoff).is_ok());
}

#[tokio::test]
async fn non_null_cutoff_near_misses_still_fail_closed() {
    for value in [
        serde_json::json!("2024"),
        serde_json::json!(""),
        serde_json::json!(2024),
        serde_json::json!([]),
        serde_json::json!({}),
    ] {
        let raw = serde_json::json!({"topic": "widget-runtime", "cutoff": value});
        let args = serde_json::from_value::<KnowledgeBriefArgs>(raw).expect("MCP args");
        let error = run_knowledge_brief(state(Vec::new(), AppConfig::default()), args)
            .await
            .expect_err("non-null near-miss must fail closed");
        assert_eq!(error.code().as_str(), "invalid_semantic_value");
        let result = map_tool_result(Err(error)).expect("repairable MCP tool error");
        let payload = result.structured_content.expect("error payload");
        assert!(payload["message"].as_str().unwrap().contains("cutoff"));
        assert_eq!(payload["repair"]["field"], "cutoff_override");
    }
}

fn meta_query(refresh: &str, cutoff: &str, retain: bool, related_topics: bool) -> BriefQuery {
    BriefQuery {
        topic: "widget-runtime".into(),
        model_id: None,
        caller_cutoff: Some(parse_caller_override(&override_day(cutoff)).unwrap()),
        max_items: 8,
        max_chars: 6000,
        refresh: parse_refresh(Some(refresh)).unwrap(),
        retain,
        related_topics,
        language: None,
        partition: "anonymous".into(),
    }
}

async fn meta_brief(
    server: &ServerState,
    config: &AppConfig,
    config_dir: &std::path::Path,
    query: &BriefQuery,
    evidence_fetcher: Option<&dyn KnowledgeEvidenceFetcher>,
) -> serde_json::Value {
    run_meta_brief(
        &BriefRun {
            adapter: server.adapter.as_ref(),
            config,
            max_results_cap: config.search.max_results_cap,
            config_dir: config_dir.to_path_buf(),
            evidence_fetcher,
        },
        query,
    )
    .await
    .expect("meta brief")
}

fn index_record(id: &str, title: &str, excerpt: &str, date: &str) -> IndexRecord {
    IndexRecord {
        partition: "anonymous".into(),
        canonical_url: format!("https://example.com/{id}"),
        source_id: format!("src_{id}"),
        title: title.into(),
        excerpt: excerpt.into(),
        trust: "external_untrusted".into(),
        basis: "publication".into(),
        precision: "day".into(),
        normalized_date: Some(date.into()),
        kdelta_id: format!("kdelta_{id}"),
        conflicted: false,
        temporal_relationship: "after_cutoff".into(),
    }
}

fn private_index_path(root: &std::path::Path) -> std::path::PathBuf {
    let parent = root.join("private-index");
    std::fs::create_dir(&parent).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    parent.join("index.sqlite")
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
                source_title: Some("Widget runtime evidence source".into()),
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

struct BridgeFetcher;

impl KnowledgeEvidenceFetcher for BridgeFetcher {
    fn fetch<'a>(&'a self, _url: &'a str, _topic: &'a str) -> EvidenceFetchFuture<'a> {
        Box::pin(async {
            Ok(FetchedEvidence {
                excerpt: "pkg:widget-runtime bridges to pkg:zebra-lib evidence".into(),
                source_title: Some("Widget runtime bridge".into()),
                spans: vec![serde_json::json!({"kind": "document_chunk", "span": "bridge"})],
                cacheable: true,
                source_truncated: false,
            })
        })
    }
}

#[tokio::test]
async fn after_cutoff_publication_is_emitted_and_labeled() {
    let engines = vec![MockEngine::success(
        "hn_algolia",
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
    assert_eq!(deltas[0]["trust"], "external_untrusted");
    assert_eq!(
        deltas[0]["id"],
        eggsearch::core::identity::knowledge_delta_id(
            "https://example.com/new",
            "publication",
            "day",
            "2024-06-02"
        )
    );
    assert_eq!(
        value["coverage"]["selected_providers"],
        serde_json::json!(["hn_algolia"])
    );
    assert_eq!(
        value["coverage"]["timestamp_capable_providers"],
        serde_json::json!(["hn_algolia"])
    );
    assert!(
        !value["warnings"].as_array().unwrap().iter().any(|warning| {
            warning
                .as_str()
                .unwrap_or_default()
                .starts_with("knowledge_brief_cutoff_unverifiable_no_timestamp_providers:")
        })
    );
    assert_eq!(value["coverage"]["empty_state_reason"], "deltas_emitted");
    let catalog_as_of = eggsearch::meta::knowledge_delta::BUNDLED_CUTOFF_SNAPSHOT
        .iter()
        .map(|row| row.verified_at)
        .max();
    assert_eq!(
        value["model_resolution"]["catalog_as_of"],
        serde_json::json!(catalog_as_of)
    );
    assert_eq!(value["coverage"]["no_temporal_claim"], false);
    assert_eq!(value["coverage"]["selected_cards"], 1);
    assert_eq!(value["coverage"]["dated"], 1);
    assert_eq!(value["coverage"]["undated"], 0);
    assert_eq!(value["coverage"]["not_after_cutoff"], 0);
    assert_eq!(
        deltas[0]["payload_chars"].as_u64().unwrap() as usize,
        deltas[0].to_string().chars().count()
    );
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
async fn configured_query_limit_bounds_topic_and_generated_hints() {
    let mut cfg = AppConfig::default();
    cfg.search.max_query_chars = 15;
    let value = run_knowledge_brief(
        state(vec![MockEngine::success("mock", vec![])], cfg.clone()),
        brief("topic", Some(override_day("2024-06-01"))),
    )
    .await
    .expect("bounded subqueries");
    assert_eq!(value["coverage"]["subqueries"], 2);

    let mut too_long = brief("0123456789abcdef", None);
    too_long.refresh = Some("index".into());
    assert!(run_knowledge_brief(
        state(vec![MockEngine::success("mock", vec![])], cfg),
        too_long,
    )
    .await
    .is_err());
}

#[tokio::test]
async fn emitted_payload_budget_includes_the_payload_chars_field() {
    let engines = || {
        vec![MockEngine::success(
            "mock",
            vec![dated("https://example.com/new", "2024-06-02")],
        )]
    };
    let cutoff = Some(override_day("2024-06-01"));
    let full = run_knowledge_brief(
        state(engines(), AppConfig::default()),
        brief("widget-runtime", cutoff.clone()),
    )
    .await
    .expect("unbounded delta");
    let delta_chars = full["deltas"][0]["payload_chars"].as_u64().unwrap() as usize;
    assert_eq!(delta_chars, full["deltas"][0].to_string().chars().count());

    let mut bounded = brief("widget-runtime", cutoff);
    bounded.max_chars = Some(delta_chars - 1);
    let limited = run_knowledge_brief(state(engines(), AppConfig::default()), bounded)
        .await
        .expect("delta beyond the budget is omitted");
    assert!(limited["deltas"].as_array().unwrap().is_empty());
    assert_eq!(limited["coverage"]["empty_state_reason"], "deltas_emitted");
    assert!(limited["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|warning| {
            warning
                .as_str()
                .unwrap_or_default()
                .starts_with("knowledge_brief_deltas_emitted:")
        }));
    assert_eq!(limited["budget_exhausted"], true);
    assert_eq!(limited["truncated"], false);
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
    assert_eq!(
        same["coverage"]["empty_state_reason"],
        "all_sources_at_or_before_cutoff"
    );
    assert_eq!(same["coverage"]["at_or_before_cutoff"], 1);
    assert_eq!(same["coverage"]["no_temporal_claim"], false);
    assert!(same["warnings"].as_array().unwrap().iter().any(|warning| {
        warning
            .as_str()
            .unwrap_or_default()
            .starts_with("knowledge_brief_all_sources_at_or_before_cutoff:")
    }));

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
    let catalog_as_of = eggsearch::meta::knowledge_delta::BUNDLED_CUTOFF_SNAPSHOT
        .iter()
        .map(|row| row.verified_at)
        .max();
    assert_eq!(
        unknown["model_resolution"]["catalog_as_of"],
        serde_json::json!(catalog_as_of)
    );
    assert!(unknown["deltas"].as_array().unwrap().is_empty());
    assert_eq!(unknown["coverage"]["no_temporal_claim"], true);
    assert_eq!(
        unknown["coverage"]["empty_state_reason"],
        "no_cutoff_resolved"
    );
    assert!(unknown["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|warning| {
            warning
                .as_str()
                .unwrap_or_default()
                .starts_with("knowledge_brief_no_cutoff_resolved:")
        }));
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
async fn undated_default_provider_reports_date_reason_and_capability() {
    let value = run_knowledge_brief(
        state(
            vec![MockEngine::success(
                "duckduckgo",
                vec![undated("https://example.com/undated")],
            )],
            AppConfig::default(),
        ),
        brief("widget-runtime", Some(override_day("2024-06-01"))),
    )
    .await
    .expect("undated result is a completed search");
    assert!(value["deltas"].as_array().unwrap().is_empty());
    assert_eq!(value["coverage"]["selected_cards"], 1);
    assert_eq!(value["coverage"]["dated"], 0);
    assert_eq!(value["coverage"]["undated"], 1);
    assert_eq!(value["coverage"]["at_or_before_cutoff"], 0);
    assert_eq!(value["coverage"]["not_after_cutoff"], 0);
    assert_eq!(value["coverage"]["empty_state_reason"], "no_dated_sources");
    assert_eq!(value["coverage"]["no_temporal_claim"], false);
    assert_eq!(
        value["coverage"]["selected_providers"],
        serde_json::json!(["duckduckgo"])
    );
    assert!(value["coverage"]["timestamp_capable_providers"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(value["warnings"].as_array().unwrap().iter().any(|warning| {
        warning
            .as_str()
            .unwrap_or_default()
            .starts_with("knowledge_brief_no_dated_sources:")
    }));
}

#[tokio::test]
async fn resolved_cutoff_without_timestamp_provider_warns_in_mcp_response() {
    let value = run_knowledge_brief(
        state(
            vec![MockEngine::success(
                "duckduckgo",
                vec![undated("https://example.com/undated")],
            )],
            AppConfig::default(),
        ),
        KnowledgeBriefArgs {
            topic: "widget-runtime".into(),
            model_id: Some("gpt-6-luna".into()),
            ..KnowledgeBriefArgs::default()
        },
    )
    .await
    .expect("mock-backed brief with a resolving model");
    assert_eq!(value["model_resolution"]["status"], "resolved");
    assert!(value["coverage"]["timestamp_capable_providers"]
        .as_array()
        .unwrap()
        .is_empty());
    let response = map_tool_result(Ok(value)).expect("successful MCP response");
    assert_eq!(response.is_error, Some(false));
    let payload = response.structured_content.expect("structured response");
    let warning = payload["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(serde_json::Value::as_str)
        .find(|warning| {
            warning.starts_with("knowledge_brief_cutoff_unverifiable_no_timestamp_providers:")
        })
        .expect("cutoff unverifiability warning");
    assert!(warning.contains(payload["model_resolution"]["cutoff_date"].as_str().unwrap()));
    assert!(warning.contains(payload["model_resolution"]["precision"].as_str().unwrap()));
    assert!(warning.contains(payload["model_resolution"]["origin"].as_str().unwrap()));
    assert!(warning.contains("the dated comparison is unverified"));
}

#[tokio::test]
async fn date_accounting_matches_selected_cards_and_emitted_deltas() {
    let cards = vec![
        undated("https://undated.example.com/item"),
        dated("https://old.example.net/item", "2024-05-31"),
        dated("https://new.example.org/item", "2024-06-02"),
    ];
    let value = run_knowledge_brief(
        state(
            vec![MockEngine::success("hn_algolia", cards)],
            AppConfig::default(),
        ),
        brief("widget-runtime", Some(override_day("2024-06-01"))),
    )
    .await
    .expect("dated and undated results");
    assert_eq!(value["coverage"]["selected_cards"], 3);
    assert_eq!(value["coverage"]["dated"], 2);
    assert_eq!(value["coverage"]["undated"], 1);
    assert_eq!(
        value["coverage"]["dated"].as_u64().unwrap()
            + value["coverage"]["undated"].as_u64().unwrap(),
        3
    );
    assert_eq!(value["coverage"]["at_or_before_cutoff"], 1);
    assert_eq!(value["coverage"]["not_after_cutoff"], 1);
    assert_eq!(value["deltas"].as_array().unwrap().len(), 1);
    assert_eq!(value["coverage"]["empty_state_reason"], "deltas_emitted");
}

#[tokio::test]
async fn disabled_index_refresh_reports_disabled_and_creates_no_file() {
    let dir = tempfile::tempdir().unwrap();
    let config_dir = dir.path().join("config");
    let cfg = AppConfig::default();
    let server = state(vec![MockEngine::success("mock", vec![])], cfg.clone());
    let query = meta_query("index", "2024-06-01", false, false);
    let value = meta_brief(&server, &cfg, &config_dir, &query, None).await;
    assert!(value["deltas"].as_array().unwrap().is_empty());
    assert_eq!(value["coverage"]["index_status"], "disabled");
    assert_ne!(value["coverage"]["empty_state_reason"], "deltas_emitted");
    assert!(value["warnings"].as_array().unwrap().iter().any(|warning| {
        warning
            .as_str()
            .unwrap_or_default()
            .starts_with("knowledge_brief_index_disabled:")
    }));
    assert!(value["warnings"].as_array().unwrap().iter().any(|warning| {
        warning.as_str().unwrap_or_default()
            == "index refresh did not search live; freshness is unverified"
    }));
    assert!(!config_dir.join("knowledge-delta").exists());
}

#[tokio::test]
async fn enabled_but_unseeded_index_reports_not_seeded() {
    let dir = tempfile::tempdir().unwrap();
    let config_dir = dir.path().join("config");
    let path = private_index_path(dir.path());
    let mut cfg = AppConfig::default();
    cfg.knowledge_delta.index.enabled = true;
    cfg.knowledge_delta.index.path = Some(path.to_string_lossy().to_string());
    let server = state(vec![MockEngine::success("mock", vec![])], cfg.clone());
    let query = meta_query("index", "2024-06-01", false, false);
    let value = meta_brief(&server, &cfg, &config_dir, &query, None).await;
    assert_eq!(value["coverage"]["index_status"], "open");
    assert_eq!(value["coverage"]["empty_state_reason"], "index_not_seeded");
    assert!(value["warnings"].as_array().unwrap().iter().any(|warning| {
        warning
            .as_str()
            .unwrap_or_default()
            .starts_with("knowledge_brief_index_not_seeded:")
    }));
}

#[tokio::test]
async fn seeded_index_without_a_post_cutoff_match_reports_no_matching_hits() {
    let dir = tempfile::tempdir().unwrap();
    let config_dir = dir.path().join("config");
    let path = private_index_path(dir.path());
    let mut cfg = AppConfig::default();
    cfg.knowledge_delta.index.enabled = true;
    cfg.knowledge_delta.index.path = Some(path.to_string_lossy().to_string());
    let index = EvidenceIndex::open(&path, &cfg.knowledge_delta.index).unwrap();
    index
        .retain(&[index_record(
            "before-cutoff",
            "Widget runtime old record",
            "old widget-runtime evidence",
            "2024-05-31",
        )])
        .unwrap();
    drop(index);
    let server = state(vec![MockEngine::success("mock", vec![])], cfg.clone());
    let query = meta_query("index", "2024-06-01", false, false);
    let value = meta_brief(&server, &cfg, &config_dir, &query, None).await;
    assert_eq!(value["coverage"]["index_status"], "open");
    assert_eq!(
        value["coverage"]["empty_state_reason"],
        "no_matching_index_hits"
    );
}

#[tokio::test]
async fn stale_retained_index_reports_index_stale() {
    let dir = tempfile::tempdir().unwrap();
    let config_dir = dir.path().join("config");
    let path = private_index_path(dir.path());
    let mut cfg = AppConfig::default();
    cfg.knowledge_delta.index.enabled = true;
    cfg.knowledge_delta.index.path = Some(path.to_string_lossy().to_string());
    cfg.knowledge_delta.index.retention_days = 1;
    let index = EvidenceIndex::open(&path, &cfg.knowledge_delta.index).unwrap();
    index
        .retain(&[index_record(
            "stale-record",
            "Widget runtime old record",
            "old widget-runtime evidence",
            "2024-05-31",
        )])
        .unwrap();
    drop(index);
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE candidate_events SET indexed_at = 1 WHERE kdelta_id = ?1",
        ["kdelta_stale-record"],
    )
    .unwrap();
    drop(conn);
    let server = state(vec![MockEngine::success("mock", vec![])], cfg.clone());
    let query = meta_query("index", "2024-06-01", false, false);
    let value = meta_brief(&server, &cfg, &config_dir, &query, None).await;
    assert_eq!(value["coverage"]["index_status"], "open");
    assert_eq!(value["coverage"]["empty_state_reason"], "index_stale");
    assert!(value["warnings"].as_array().unwrap().iter().any(|warning| {
        warning
            .as_str()
            .unwrap_or_default()
            .starts_with("knowledge_brief_index_stale:")
    }));
}

#[cfg(unix)]
#[tokio::test]
async fn index_permission_warning_names_directory_and_required_mode() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let parent = dir.path().join("shared");
    std::fs::create_dir(&parent).unwrap();
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = parent.join("index.sqlite");
    let mut cfg = AppConfig::default();
    cfg.knowledge_delta.index.enabled = true;
    cfg.knowledge_delta.index.path = Some(path.to_string_lossy().to_string());
    let mut args = brief("widget-runtime", Some(override_day("2024-06-01")));
    args.refresh = Some("prefer_live".into());
    let value = run_knowledge_brief(
        state(vec![MockEngine::success("duckduckgo", vec![])], cfg),
        args,
    )
    .await
    .expect("shared-parent refusal is a warning");
    assert_eq!(value["coverage"]["index_status"], "offline");
    let warning = value["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(serde_json::Value::as_str)
        .find(|warning| warning.contains("knowledge index directory"))
        .expect("directory permission warning");
    assert!(warning.contains(parent.to_str().unwrap()));
    assert!(warning.contains("0755"));
    assert!(warning.contains("0700"));
}

#[tokio::test]
async fn retain_uses_fetched_public_evidence_and_index_mode_is_offline() {
    let dir = tempfile::tempdir().unwrap();
    let path = private_index_path(dir.path());
    let mut cfg = AppConfig::default();
    cfg.fetch.sanitize_output = false;
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
    assert_eq!(hits[0].record.title, "Widget runtime evidence source");
    assert_eq!(hits[0].record.excerpt, "fetched public evidence passage");
    let conn = rusqlite::Connection::open(&path).unwrap();
    let (stored_topic, stored_subject): (String, Option<String>) = conn
        .query_row(
            "SELECT topic, primary_subject FROM candidate_events WHERE kdelta_id = ?1",
            [&hits[0].record.kdelta_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(stored_topic, "");
    assert_ne!(stored_subject.as_deref(), Some("widget-runtime"));
    conn.execute(
        "UPDATE candidate_events SET trust = 'trusted' WHERE kdelta_id = ?1",
        [&hits[0].record.kdelta_id],
    )
    .unwrap();
    drop(conn);
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
    assert_eq!(indexed["deltas"][0]["trust"], "external_untrusted");
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
    let path = private_index_path(dir.path());
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

#[tokio::test]
async fn cold_start_enable_seed_restart_and_related_reuse() {
    let dir = tempfile::tempdir().unwrap();
    let config_dir = dir.path().join("config");
    let disabled_cfg = AppConfig::default();
    let disabled_server = state(
        vec![MockEngine::success("hn_algolia", vec![])],
        disabled_cfg.clone(),
    );
    let rejected = run_knowledge_brief(
        Arc::clone(&disabled_server),
        KnowledgeBriefArgs {
            retain: true,
            ..brief("widget-runtime", Some(override_day("2024-06-01")))
        },
    )
    .await;
    assert!(rejected.is_err());
    let disabled_query = meta_query("live", "2024-06-01", true, false);
    let disabled = meta_brief(
        &disabled_server,
        &disabled_cfg,
        &config_dir,
        &disabled_query,
        None,
    )
    .await;
    assert_eq!(disabled["coverage"]["index_status"], "disabled");
    assert!(!config_dir.join("knowledge-delta").exists());

    let path = private_index_path(dir.path());
    let mut cfg = disabled_cfg;
    cfg.knowledge_delta.index.enabled = true;
    cfg.knowledge_delta.index.path = Some(path.to_string_lossy().to_string());
    let server = state(
        vec![MockEngine::success(
            "hn_algolia",
            vec![dated("https://example.com/bridge", "2024-06-02")],
        )],
        cfg.clone(),
    );
    let fetcher = BridgeFetcher;
    let live_query = meta_query("live", "2024-06-01", true, false);
    let live = meta_brief(&server, &cfg, &config_dir, &live_query, Some(&fetcher)).await;
    assert_eq!(live["deltas"].as_array().unwrap().len(), 1);
    assert!(path.exists());
    let live_delta = &live["deltas"][0];
    let retained_id = live_delta["id"].as_str().unwrap().to_string();
    assert_eq!(live_delta["trust"], "external_untrusted");
    assert_eq!(live_delta["date_basis"], "publication");
    drop(live);
    drop(server);

    let index = EvidenceIndex::open(&path, &cfg.knowledge_delta.index).unwrap();
    index
        .retain(&[index_record(
            "neighbor",
            "Zebra-lib companion update",
            "pkg:zebra-lib provides a standalone dependency feature",
            "2024-06-03",
        )])
        .unwrap();
    drop(index);

    let restarted_server = state(vec![MockEngine::success("hn_algolia", vec![])], cfg.clone());
    let index_query = meta_query("index", "2024-06-01", false, false);
    let indexed = meta_brief(&restarted_server, &cfg, &config_dir, &index_query, None).await;
    assert_eq!(indexed["deltas"][0]["id"], retained_id);
    assert_eq!(indexed["deltas"][0]["provenance"], "index_only");
    assert_eq!(indexed["deltas"][0]["trust"], "external_untrusted");
    assert_eq!(indexed["deltas"][0]["date_basis"], "publication");
    assert_eq!(indexed["deltas"][0]["precision"], "day");

    let related_query = meta_query("index", "2024-06-01", false, true);
    let related = meta_brief(&restarted_server, &cfg, &config_dir, &related_query, None).await;
    assert_eq!(related["coverage"]["expansion"], "added");
    let neighbor = related["deltas"]
        .as_array()
        .unwrap()
        .iter()
        .find(|delta| delta["expanded"] == true)
        .expect("related indexed neighbor");
    assert!(!neighbor["spans"].as_array().unwrap().is_empty());
    assert!(neighbor["spans"][0]["span"].as_str().is_some());
}

#[tokio::test]
async fn ordinary_live_call_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let config_dir = dir.path().join("config");
    let cfg = AppConfig::default();
    let server = state(
        vec![MockEngine::success(
            "hn_algolia",
            vec![dated("https://example.com/new", "2024-06-02")],
        )],
        cfg.clone(),
    );
    let query = meta_query("live", "2024-06-01", false, false);
    let value = meta_brief(&server, &cfg, &config_dir, &query, None).await;
    assert_eq!(value["coverage"]["index_status"], "not_opened");
    assert!(!config_dir
        .join("knowledge-delta")
        .join("anonymous")
        .join("index.sqlite")
        .exists());
}
