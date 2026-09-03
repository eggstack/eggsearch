#![cfg(feature = "mock")]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use eggsearch::core::config::AppConfig;
use eggsearch::core::query::{
    domain_matches_filter, normalize_domain, Freshness, SearchDateRange, SearchIntent,
    WebSearchRequest,
};
use eggsearch::mcp::state::ServerState;
use eggsearch::meta::adapter::MetadataSearchAdapter;
use eggsearch::meta::engines::{EngineSearchRequest, SearchEngine};
use eggsearch::meta::mock::{mock_engines, MockEngine, MockResult};
use eggsearch::meta::provider_diagnostics::CapabilityEnforcementTelemetry;

fn adapter_with(engines: Vec<MockEngine>) -> MetadataSearchAdapter {
    MetadataSearchAdapter::from_engines(mock_engines(engines), Duration::from_secs(5))
}

struct CapturingEngine {
    name: &'static str,
    seen: Arc<Mutex<Option<EngineSearchRequest>>>,
}

impl SearchEngine for CapturingEngine {
    fn name(&self) -> &'static str {
        self.name
    }
    fn search<'a>(
        &'a self,
        request: &'a EngineSearchRequest,
    ) -> eggsearch::meta::engines::BoxFuture<
        'a,
        Result<
            Vec<eggsearch::meta::engines::models::SearchResult>,
            eggsearch::meta::engines::error::EngineError,
        >,
    > {
        let seen = Arc::clone(&self.seen);
        let req = request.clone();
        Box::pin(async move {
            *seen.lock().unwrap() = Some(req);
            Ok(Vec::new())
        })
    }
}

#[tokio::test]
async fn engine_request_migration_preserves_constraints() {
    let seen: Arc<Mutex<Option<EngineSearchRequest>>> = Arc::new(Mutex::new(None));
    let engine: Arc<dyn SearchEngine> = Arc::new(CapturingEngine {
        name: "mock_a",
        seen: Arc::clone(&seen),
    });
    let adapter = MetadataSearchAdapter::from_engines(vec![engine], Duration::from_secs(5));
    let mut req = WebSearchRequest::new("rust axum");
    req.intent = SearchIntent::News;
    req.safe_search = Some(eggsearch::core::query::SafeSearch::Strict);
    req.freshness = Freshness::Week;
    req.language = Some("en".to_string());
    req.region = Some("US".to_string());
    req.include_domains = vec!["example.com".to_string()];
    let _ = adapter.web_search(&req, 5, 50).await;
    let captured = seen.lock().unwrap().clone().expect("engine was called");
    assert!(captured.query.contains("rust axum"));
    assert_eq!(captured.intent, SearchIntent::News);
    assert_eq!(
        captured.safe_search,
        Some(eggsearch::core::query::SafeSearch::Strict)
    );
    assert_eq!(captured.freshness, Freshness::Week);
    assert_eq!(captured.language.as_deref(), Some("en"));
    assert_eq!(captured.region.as_deref(), Some("US"));
    assert_eq!(captured.include_domains, vec!["example.com".to_string()]);
}

#[tokio::test]
async fn multiquery_dispatch_uses_same_contract() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![MockResult::new("T", "https://example.com", "mock_a")],
    )];
    let adapter = adapter_with(engines);
    let req = eggsearch::core::repo_search::RepoSearchRequest {
        query: "test".to_string(),
        ..Default::default()
    };
    let resp = adapter.repo_search(&req, 5, 50, None, None).await;
    let _ = resp.groups.len() + resp.warnings.len();
}

#[test]
fn date_parsing_accepts_leap_and_rejects_invalid() {
    let mut req = WebSearchRequest::new("test");
    req.date_range = Some(SearchDateRange::new("2024-02-29", "2024-03-01"));
    assert!(req.validate(512).is_ok());

    let mut bad = WebSearchRequest::new("test");
    bad.date_range = Some(SearchDateRange::new("2023-02-29", "2023-03-01"));
    assert!(bad.validate(512).is_err());

    let mut reversed = WebSearchRequest::new("test");
    reversed.date_range = Some(SearchDateRange::new("2024-03-01", "2024-02-01"));
    let err = reversed.validate(512).unwrap_err().to_string();
    assert!(err.contains("start must be <="));

    let mut invalid = WebSearchRequest::new("test");
    invalid.date_range = Some(SearchDateRange::new("2024-13-01", "2024-12-01"));
    assert!(invalid.validate(512).is_err());
}

#[test]
fn freshness_plus_date_range_is_rejected() {
    let mut req = WebSearchRequest::new("test");
    req.freshness = Freshness::Week;
    req.date_range = Some(SearchDateRange::new("2024-01-01", "2024-01-31"));
    let err = req.validate(512).unwrap_err().to_string();
    assert!(err.contains("mutually exclusive"));
}

#[test]
fn domain_normalization_and_matching() {
    assert_eq!(normalize_domain("Example.COM").unwrap(), "example.com");
    assert_eq!(
        normalize_domain(" docs.example.com ").unwrap(),
        "docs.example.com"
    );
    assert!(normalize_domain("https://example.com").is_err());
    assert!(normalize_domain("example.com:443").is_err());
    assert!(normalize_domain("user@example.com").is_err());
    assert!(normalize_domain("example.com/path").is_err());
    assert!(normalize_domain("*.example.com").is_err());
    assert!(normalize_domain("bad..example.com").is_err());
    assert!(normalize_domain(".example.com").is_err());

    assert!(domain_matches_filter("example.com", "example.com"));
    assert!(domain_matches_filter("docs.example.com", "example.com"));
    assert!(!domain_matches_filter("notexample.com", "example.com"));
    assert!(!domain_matches_filter(
        "example.com.evil.com",
        "example.com"
    ));
}

#[tokio::test]
async fn domain_post_filtering_before_truncation_and_within_cap() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![
            MockResult::new("A", "https://example.com/a", "mock_a"),
            MockResult::new("B", "https://other.com/b", "mock_a"),
            MockResult::new("C", "https://docs.example.com/c", "mock_a"),
            MockResult::new("D", "https://notexample.com/d", "mock_a"),
        ],
    )];
    let adapter = adapter_with(engines);
    let mut req = WebSearchRequest::new("test");
    req.include_domains = vec!["example.com".to_string()];
    let resp = adapter.web_search(&req, 10, 50).await;
    let urls: Vec<&str> = resp.results.iter().map(|c| c.url.as_str()).collect();
    assert!(urls.contains(&"https://example.com/a"));
    assert!(urls.contains(&"https://docs.example.com/c"));
    assert!(!urls.contains(&"https://other.com/b"));
    assert!(!urls.contains(&"https://notexample.com/d"));

    let engines2 = vec![MockEngine::success(
        "mock_a",
        vec![
            MockResult::new("A", "https://example.com/a", "mock_a"),
            MockResult::new("B", "https://other.com/b", "mock_a"),
        ],
    )];
    let adapter2 = adapter_with(engines2);
    let mut req2 = WebSearchRequest::new("test");
    req2.exclude_domains = vec!["example.com".to_string()];
    let resp2 = adapter2.web_search(&req2, 10, 50).await;
    let urls2: Vec<&str> = resp2.results.iter().map(|c| c.url.as_str()).collect();
    assert!(!urls2.contains(&"https://example.com/a"));

    let seen: Arc<Mutex<Option<EngineSearchRequest>>> = Arc::new(Mutex::new(None));
    let engine: Arc<dyn SearchEngine> = Arc::new(CapturingEngine {
        name: "mock_a",
        seen: Arc::clone(&seen),
    });
    let adapter3 = MetadataSearchAdapter::from_engines(vec![engine], Duration::from_secs(5));
    let mut req3 = WebSearchRequest::new("test");
    req3.include_domains = vec!["example.com".to_string()];
    let _ = adapter3.web_search(&req3, 10, 50).await;
    let captured = seen.lock().unwrap().clone().expect("called");
    assert!(
        captured.max_results <= 50,
        "candidate pool must never exceed cap, got {}",
        captured.max_results
    );
}

#[tokio::test]
async fn brave_web_params_present() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let web_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("q", "rust")
            .query_param("safesearch", "strict")
            .query_param("freshness", "pw")
            .query_param("search_lang", "en")
            .query_param("country", "US");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"web": {"results": []}}"#);
    });
    let client = reqwest::Client::new();
    let mut req = EngineSearchRequest::simple("rust", 5, Duration::from_secs(5));
    req.safe_search = Some(eggsearch::core::query::SafeSearch::Strict);
    req.freshness = Freshness::Week;
    req.language = Some("en".to_string());
    req.region = Some("US".to_string());
    eggsearch::meta::engines::brave_api::search(&client, "k", Some(&server.url("/search")), &req)
        .await
        .expect("ok");
    web_mock.assert();
}

#[tokio::test]
async fn unsupported_constraints_omitted() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/search").query_param("q", "rust");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"web": {"results": []}}"#);
    });
    let client = reqwest::Client::new();
    let mut req = EngineSearchRequest::simple("rust", 5, Duration::from_secs(5));
    req.language = Some("not-a-locale!!!".to_string());
    req.region = Some("TOOLONGREGIONNAME".to_string());
    eggsearch::meta::engines::brave_api::search(&client, "k", Some(&server.url("/search")), &req)
        .await
        .expect("ok");
    mock.assert();
}

#[test]
fn telemetry_native_vs_local() {
    let mut req = WebSearchRequest::new("test");
    req.safe_search = Some(eggsearch::core::query::SafeSearch::Strict);
    req.include_domains = vec!["example.com".to_string()];
    let tele = CapabilityEnforcementTelemetry::for_web_search(
        &req,
        &["brave_api".to_string(), "duckduckgo".to_string()],
    );
    assert!(tele.enforced.iter().any(|c| c == "safe_search"));
    assert!(tele.approximated.iter().any(|c| c == "domain_filters"));
    assert!(!tele.enforced.iter().any(|c| c == "domain_filters"));

    let req2 = WebSearchRequest::new("test");
    let tele2 = CapabilityEnforcementTelemetry::for_web_search(&req2, &["duckduckgo".to_string()]);
    assert!(tele2.requested.is_empty());
}

#[test]
fn legacy_fixtures_deserialize() {
    let json = r#"{"query": "rust", "max_results": 5}"#;
    let req: WebSearchRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.query, "rust");
    assert!(req.date_range.is_none());
    assert!(req.include_domains.is_empty());
    assert!(req.language.is_none());
    assert!(req.validate(512).is_ok());
}

#[tokio::test]
async fn health_and_timeout_preserved() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![MockResult::new("T", "https://example.com", "mock_a")],
    )];
    let adapter = adapter_with(engines);
    let req = WebSearchRequest::new("test");
    let resp = adapter.web_search(&req, 5, 50).await;
    assert!(resp.providers_failed.is_empty());
    assert_eq!(resp.results.len(), 1);
}

#[tokio::test]
async fn server_state_web_search_with_new_fields() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![MockResult::new("T", "https://example.com", "mock_a")],
    )];
    let adapter =
        MetadataSearchAdapter::from_engines(mock_engines(engines), Duration::from_secs(5));
    let cfg = AppConfig::default();
    let state = Arc::new(ServerState::with_adapter(cfg, Arc::new(adapter)));
    let args = eggsearch::mcp::tools::WebSearchArgs {
        query: "test".to_string(),
        max_results: None,
        providers: vec![],
        safe_search: None,
        timeout_ms: None,
        intent: None,
        freshness: None,
        date_range: None,
        include_domains: Vec::new(),
        exclude_domains: Vec::new(),
        language: None,
        region: None,
        excerpt_count: None,
    };
    let v = eggsearch::mcp::tools::run_web_search(state, args)
        .await
        .expect("ok");
    assert!(v.get("results").is_some());
    assert!(v.get("capability_enforcement").is_some());
}
