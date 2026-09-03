use std::sync::Arc;
use std::time::Duration;

use eggsearch::core::config::{ApiProviderConfig, AppConfig};
use eggsearch::core::provider::{
    built_in_provider_descriptor, credential_requirement, is_api_provider, CredentialRequirement,
    API_PROVIDER_IDS, KNOWN_PROVIDER_IDS,
};
use eggsearch::core::query::{Freshness, SearchDateRange, WebSearchRequest};
use eggsearch::meta::engines::request::EngineSearchRequest;
use eggsearch::meta::engines::SearchEngine;
use eggsearch::meta::engines::TavilyEngine;
use eggsearch::meta::provider_diagnostics::CapabilityEnforcementTelemetry;

fn engine_req(query: &str, max_results: usize) -> EngineSearchRequest {
    EngineSearchRequest::simple(query, max_results, Duration::from_secs(5))
}

#[test]
fn provider_inventory_reflects_new_count() {
    assert!(KNOWN_PROVIDER_IDS.contains(&"tavily"));
    assert!(API_PROVIDER_IDS.contains(&"tavily"));
    assert!(is_api_provider("tavily"));
    assert_eq!(KNOWN_PROVIDER_IDS.len(), 37);
    assert_eq!(
        credential_requirement("tavily"),
        CredentialRequirement::Required
    );
}

#[test]
fn descriptor_claims_only_implemented_capabilities() {
    let desc = built_in_provider_descriptor("tavily", true, false, true, true, None, None)
        .expect("descriptor");
    assert_eq!(desc.id, "tavily");
    assert_eq!(desc.display_name, "Tavily Search");
    assert_eq!(desc.kind, eggsearch::core::provider::ProviderKind::ApiKey);
    assert!(desc.requires_api_key);
    assert!(desc.capabilities.supports_safe_search);
    assert!(desc.capabilities.supports_freshness);
    assert!(desc.capabilities.supports_language);
    assert!(desc.capabilities.supports_region);
    assert!(desc.capabilities.supports_domain_filters);
    assert!(desc.capabilities.supports_news);
    assert!(!desc.capabilities.supports_result_timestamps);
    assert!(!desc.capabilities.supports_code_search);
    assert!(!desc.capabilities.supports_issue_search);
    assert!(!desc.capabilities.supports_release_search);
    assert!(!desc.capabilities.supports_scholarly_search);
    assert!(!desc.capabilities.supports_repo_indexing);
}

#[test]
fn missing_credential_yields_missing_api_key_skip() {
    let mut cfg = AppConfig::default();
    cfg.search.api.insert(
        "tavily".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: Some("EGGSEARCH_TEST_TAVILY_MISSING_KEY".to_string()),
            base_url: None,
        },
    );
    assert!(std::env::var("EGGSEARCH_TEST_TAVILY_MISSING_KEY").is_err());
    assert!(!cfg.provider_is_available("tavily"));
    let enabled = cfg.effective_provider_ids();
    assert!(!enabled.contains(&"tavily".to_string()));
    let explicit_enabled = vec!["tavily".to_string()];
    let (_, skipped_explicit) = eggsearch::meta::adapter::build_default_engines(
        &explicit_enabled,
        None,
        None,
        &cfg.search.api,
    )
    .expect("explicit build");
    let skip = skipped_explicit
        .iter()
        .find(|s| s.id == "tavily")
        .expect("tavily skipped when credential missing");
    assert!(
        skip.reason.contains("missing_api_key"),
        "expected missing_api_key skip, got {}",
        skip.reason
    );
    let state = eggsearch::mcp::state::ServerState::build(AppConfig::default())
        .expect("default state builds keyless");
    let status = state.adapter.provider_status();
    let desc = status
        .iter()
        .find(|d| d.id == "tavily")
        .expect("tavily in status");
    assert!(!desc.routable);
    assert!(desc.skip_code.is_some());
    assert!(desc.skip_reason.is_some());
}

#[test]
fn default_config_leaves_tavily_out_of_defaults() {
    let cfg = AppConfig::default();
    let resolved = cfg.resolve_providers(&[]).expect("defaults resolve");
    assert!(!resolved.contains(&"tavily".to_string()));
    assert!(resolved.contains(&"duckduckgo".to_string()));
}

#[test]
fn configured_tavily_still_not_in_defaults_without_operator_opt_in() {
    std::env::set_var("EGGSEARCH_TEST_TAVILY_KEY_DEFAULTS", "test-key");
    let mut cfg = AppConfig::default();
    cfg.search.api.insert(
        "tavily".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: Some("EGGSEARCH_TEST_TAVILY_KEY_DEFAULTS".to_string()),
            base_url: None,
        },
    );
    assert!(cfg.provider_is_available("tavily"));
    let resolved = cfg.resolve_providers(&[]).expect("defaults resolve");
    assert!(
        !resolved.contains(&"tavily".to_string()),
        "tavily must never join defaults automatically, got {resolved:?}"
    );
    let explicit = cfg
        .resolve_providers(&["tavily".to_string()])
        .expect("explicit tavily resolves when configured");
    assert_eq!(explicit, vec!["tavily".to_string()]);
    std::env::remove_var("EGGSEARCH_TEST_TAVILY_KEY_DEFAULTS");
}

#[test]
fn telemetry_reports_native_constraints() {
    let mut req = WebSearchRequest::new("test");
    req.include_domains = vec!["example.com".to_string()];
    req.date_range = Some(SearchDateRange::new("2024-01-01", "2024-01-31"));
    let tele = CapabilityEnforcementTelemetry::for_web_search(&req, &["tavily".to_string()]);
    assert!(tele.enforced.iter().any(|c| c == "domain_filters"));
    assert!(tele.enforced.iter().any(|c| c == "date_range"));

    let mut req2 = WebSearchRequest::new("test");
    req2.freshness = Freshness::Week;
    let tele2 = CapabilityEnforcementTelemetry::for_web_search(&req2, &["tavily".to_string()]);
    assert!(tele2.enforced.iter().any(|c| c == "freshness"));

    let mut req3 = WebSearchRequest::new("test");
    req3.safe_search = Some(eggsearch::core::query::SafeSearch::Strict);
    let tele3 = CapabilityEnforcementTelemetry::for_web_search(&req3, &["tavily".to_string()]);
    assert!(tele3.enforced.iter().any(|c| c == "safe_search"));

    let mut req4 = WebSearchRequest::new("test");
    req4.language = Some("en".to_string());
    req4.region = Some("US".to_string());
    let tele4 = CapabilityEnforcementTelemetry::for_web_search(&req4, &["tavily".to_string()]);
    assert!(tele4.enforced.iter().any(|c| c == "language"));
    assert!(tele4.enforced.iter().any(|c| c == "region"));

    let mut req5 = WebSearchRequest::new("test");
    req5.intent = eggsearch::core::query::SearchIntent::News;
    let tele5 = CapabilityEnforcementTelemetry::for_web_search(&req5, &["tavily".to_string()]);
    assert!(tele5.enforced.iter().any(|c| c == "news"));
}

#[test]
fn api_key_never_rendered_in_diagnostics() {
    let client = Arc::new(reqwest::Client::new());
    let engine = TavilyEngine {
        client,
        api_key: "super-secret-tavily-key".to_string(),
        base_url: None,
    };
    let label = format!("{}:{}", engine.name(), "tavily");
    assert!(!label.contains("super-secret-tavily-key"));
    let err = eggsearch::meta::engines::error::EngineError::BadStatus {
        engine: "tavily",
        status: 401,
    };
    assert!(!err.to_string().contains("super-secret-tavily-key"));
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn configured_key_sends_bearer_header() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/search")
            .header("Authorization", "Bearer test-tavily-key-123");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"results": []}"#);
    });
    let client = reqwest::Client::new();
    let results = eggsearch::meta::engines::tavily::search(
        &client,
        "test-tavily-key-123",
        Some(&server.url("/search")),
        &engine_req("rust", 5),
    )
    .await
    .expect("search succeeds");
    assert!(results.is_empty());
    mock.assert();
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn default_request_disables_answer_raw_content_and_auto_parameters() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/search")
            .json_body(serde_json::json!({
                "query": "rust",
                "search_depth": "basic",
                "max_results": 5,
                "chunks_per_source": 1,
                "topic": "general",
                "include_answer": false,
                "include_raw_content": false,
                "include_images": false,
                "auto_parameters": false
            }));
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"results": []}"#);
    });
    let client = reqwest::Client::new();
    eggsearch::meta::engines::tavily::search(
        &client,
        "k",
        Some(&server.url("/search")),
        &engine_req("rust", 5),
    )
    .await
    .expect("ok");
    mock.assert();
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn forbidden_response_fields_never_requested() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/search");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"results": []}"#);
    });
    let client = reqwest::Client::new();
    let mut req = engine_req("rust", 5);
    req.excerpt_count = 2;
    req.include_domains = vec!["example.com".to_string()];
    let url = server.url("/search");
    eggsearch::meta::engines::tavily::search(&client, "k", Some(&url), &req)
        .await
        .expect("ok");
    let hits = server.mock(|when, then| {
        when.method(POST).path("/probe");
        then.status(200).body("{}");
    });
    assert_eq!(hits.hits(), 0);
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn exact_date_range_maps_to_start_end_dates() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/search")
            .json_body(serde_json::json!({
                "query": "rust",
                "search_depth": "basic",
                "max_results": 5,
                "chunks_per_source": 1,
                "topic": "general",
                "start_date": "2024-01-01",
                "end_date": "2024-01-31",
                "include_answer": false,
                "include_raw_content": false,
                "include_images": false,
                "auto_parameters": false
            }));
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"results": []}"#);
    });
    let client = reqwest::Client::new();
    let mut req = engine_req("rust", 5);
    req.date_range = Some(SearchDateRange::new("2024-01-01", "2024-01-31"));
    eggsearch::meta::engines::tavily::search(&client, "k", Some(&server.url("/search")), &req)
        .await
        .expect("ok");
    mock.assert();
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn relative_freshness_sends_time_range_without_dates() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/search")
            .json_body(serde_json::json!({
                "query": "rust",
                "search_depth": "basic",
                "max_results": 5,
                "chunks_per_source": 1,
                "topic": "general",
                "time_range": "week",
                "include_answer": false,
                "include_raw_content": false,
                "include_images": false,
                "auto_parameters": false
            }));
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"results": []}"#);
    });
    let client = reqwest::Client::new();
    let mut req = engine_req("rust", 5);
    req.freshness = Freshness::Week;
    eggsearch::meta::engines::tavily::search(&client, "k", Some(&server.url("/search")), &req)
        .await
        .expect("ok");
    mock.assert();
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn include_exclude_domains_use_strict_filter_mode() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/search")
            .json_body(serde_json::json!({
                "query": "rust",
                "search_depth": "basic",
                "max_results": 5,
                "chunks_per_source": 1,
                "topic": "general",
                "include_domains": ["example.com"],
                "exclude_domains": ["spam.example"],
                "include_domains_mode": "filter",
                "include_answer": false,
                "include_raw_content": false,
                "include_images": false,
                "auto_parameters": false
            }));
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"results": []}"#);
    });
    let client = reqwest::Client::new();
    let mut req = engine_req("rust", 5);
    req.include_domains = vec!["example.com".to_string()];
    req.exclude_domains = vec!["spam.example".to_string()];
    eggsearch::meta::engines::tavily::search(&client, "k", Some(&server.url("/search")), &req)
        .await
        .expect("ok");
    mock.assert();
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn news_intent_routes_to_news_topic() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/search")
            .json_body(serde_json::json!({
                "query": "election",
                "search_depth": "basic",
                "max_results": 5,
                "chunks_per_source": 1,
                "topic": "news",
                "include_answer": false,
                "include_raw_content": false,
                "include_images": false,
                "auto_parameters": false
            }));
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"results": []}"#);
    });
    let client = reqwest::Client::new();
    let mut req = engine_req("election", 5);
    req.intent = eggsearch::core::query::SearchIntent::News;
    eggsearch::meta::engines::tavily::search(&client, "k", Some(&server.url("/search")), &req)
        .await
        .expect("ok");
    mock.assert();
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn safe_search_maps_off_false_and_moderate_strict_true() {
    use eggsearch::core::query::SafeSearch;
    use httpmock::prelude::*;
    for (mode, expected) in [
        (SafeSearch::Off, false),
        (SafeSearch::Moderate, true),
        (SafeSearch::Strict, true),
    ] {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/search")
                .json_body(serde_json::json!({
                    "query": "test",
                    "search_depth": "basic",
                    "max_results": 5,
                    "chunks_per_source": 1,
                    "topic": "general",
                    "safe_search": expected,
                    "include_answer": false,
                    "include_raw_content": false,
                    "include_images": false,
                    "auto_parameters": false
                }));
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"results": []}"#);
        });
        let client = reqwest::Client::new();
        let mut req = engine_req("test", 5);
        req.safe_search = Some(mode);
        eggsearch::meta::engines::tavily::search(&client, "k", Some(&server.url("/search")), &req)
            .await
            .expect("ok");
        mock.assert();
    }
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn language_region_mapped_only_when_representable() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/search")
            .json_body(serde_json::json!({
                "query": "test",
                "search_depth": "basic",
                "max_results": 5,
                "chunks_per_source": 1,
                "topic": "general",
                "country": "united states",
                "language": "en",
                "filter_by_language": true,
                "include_answer": false,
                "include_raw_content": false,
                "include_images": false,
                "auto_parameters": false
            }));
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"results": []}"#);
    });
    let client = reqwest::Client::new();
    let mut req = engine_req("test", 5);
    req.language = Some("en".to_string());
    req.region = Some("US".to_string());
    eggsearch::meta::engines::tavily::search(&client, "k", Some(&server.url("/search")), &req)
        .await
        .expect("ok");
    mock.assert();

    let server2 = MockServer::start();
    let mock2 = server2.mock(|when, then| {
        when.method(POST)
            .path("/search")
            .json_body(serde_json::json!({
                "query": "test",
                "search_depth": "basic",
                "max_results": 5,
                "chunks_per_source": 1,
                "topic": "general",
                "include_answer": false,
                "include_raw_content": false,
                "include_images": false,
                "auto_parameters": false
            }));
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"results": []}"#);
    });
    let mut bad = engine_req("test", 5);
    bad.language = Some("not-a-locale!!!".to_string());
    bad.region = Some("USA".to_string());
    eggsearch::meta::engines::tavily::search(&client, "k", Some(&server2.url("/search")), &bad)
        .await
        .expect("ok");
    mock2.assert();
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn country_omitted_for_news_topic() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/search")
            .json_body(serde_json::json!({
                "query": "election",
                "search_depth": "basic",
                "max_results": 5,
                "chunks_per_source": 1,
                "topic": "news",
                "include_answer": false,
                "include_raw_content": false,
                "include_images": false,
                "auto_parameters": false
            }));
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"results": []}"#);
    });
    let client = reqwest::Client::new();
    let mut req = engine_req("election", 5);
    req.intent = eggsearch::core::query::SearchIntent::News;
    req.region = Some("US".to_string());
    eggsearch::meta::engines::tavily::search(&client, "k", Some(&server.url("/search")), &req)
        .await
        .expect("ok");
    mock.assert();
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn chunks_bounded_and_converted_to_excerpts() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let with_mock = server.mock(|when, then| {
        when.method(POST)
            .path("/search")
            .body_contains("chunks_per_source");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"results": []}"#);
    });
    let client = reqwest::Client::new();
    let mut req = engine_req("rust", 5);
    req.excerpt_count = 2;
    eggsearch::meta::engines::tavily::search(&client, "k", Some(&server.url("/search")), &req)
        .await
        .expect("ok");
    with_mock.assert();

    server.mock(|when, then| {
        when.method(POST).path("/bounded");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                "results": [
                    {"title": "T", "url": "https://example.com/a",
                     "content": "h1 [...]  [...] h2 [...] h3 [...] h4"}
                ]
            }"#,
            );
    });
    let mut bounded = engine_req("test", 5);
    bounded.excerpt_count = 3;
    let results = eggsearch::meta::engines::tavily::search(
        &client,
        "k",
        Some(&server.url("/bounded")),
        &bounded,
    )
    .await
    .expect("ok");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].excerpts.len(), 3);
    assert!(results[0].excerpts.iter().all(|e| matches!(
        e.provenance,
        eggsearch::core::source_card::ExcerptProvenance::ProviderSnippet
    )));
    assert_eq!(results[0].snippet.as_deref(), Some("h1"));

    let mut plain = engine_req("test", 5);
    plain.excerpt_count = 0;
    server.mock(|when, then| {
        when.method(POST).path("/plain");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{"results": [{"title": "T", "url": "https://example.com/a", "content": "only [...] extra"}]}"#,
            );
    });
    let results_plain =
        eggsearch::meta::engines::tavily::search(&client, "k", Some(&server.url("/plain")), &plain)
            .await
            .expect("ok");
    assert_eq!(results_plain.len(), 1);
    assert!(results_plain[0].excerpts.is_empty());
    assert_eq!(results_plain[0].snippet.as_deref(), Some("only"));
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn chunk_conversion_is_deterministic() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/search");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                "results": [
                    {"title": "T", "url": "https://example.com/a",
                     "content": "low [...] high [...] mid"}
                ]
            }"#,
            );
    });
    let client = reqwest::Client::new();
    let mut req = engine_req("test", 5);
    req.excerpt_count = 3;
    let first =
        eggsearch::meta::engines::tavily::search(&client, "k", Some(&server.url("/search")), &req)
            .await
            .expect("ok");
    let second =
        eggsearch::meta::engines::tavily::search(&client, "k", Some(&server.url("/search")), &req)
            .await
            .expect("ok");
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].excerpts.len(), 3);
    assert_eq!(first[0].excerpts[0].text, "low");
    assert_eq!(first[0].excerpts[1].text, "high");
    assert_eq!(first[0].excerpts[2].text, "mid");
    let first_json = serde_json::to_value(&first).expect("serializable");
    let second_json = serde_json::to_value(&second).expect("serializable");
    assert_eq!(first_json, second_json, "conversion must be deterministic");
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn error_statuses_map_to_provider_failures() {
    use httpmock::prelude::*;
    for status in [400_u16, 401, 402, 429, 500] {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/search");
            then.status(status).body("error envelope");
        });
        let client = reqwest::Client::new();
        let err = eggsearch::meta::engines::tavily::search(
            &client,
            "k",
            Some(&server.url("/search")),
            &engine_req("test", 5),
        )
        .await
        .expect_err("status must fail");
        match err {
            eggsearch::meta::engines::error::EngineError::BadStatus {
                engine,
                status: got,
            } => {
                assert_eq!(engine, "tavily");
                assert_eq!(got, status);
                assert!(!err.to_string().contains("test-tavily-key"));
            }
            other => panic!("expected BadStatus({status}), got {other:?}"),
        }
    }
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn oversized_body_rejected_by_shared_cap() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let big = "a".repeat(2 * 1024 * 1024 + 1024);
    let body = format!(
        "{{\"results\": [{{\"title\": \"T\", \"url\": \"https://example.com/a\", \"content\": \"{big}\"}}]}}"
    );
    server.mock(|when, then| {
        when.method(POST).path("/search");
        then.status(200)
            .header("content-type", "application/json")
            .body(body.clone());
    });
    let client = reqwest::Client::new();
    let err = eggsearch::meta::engines::tavily::search(
        &client,
        "k",
        Some(&server.url("/search")),
        &engine_req("test", 5),
    )
    .await
    .expect_err("oversized must fail");
    match err {
        eggsearch::meta::engines::error::EngineError::ParseFailed { reason, .. } => {
            assert!(reason.contains("too large"), "reason: {reason}")
        }
        other => panic!("expected ParseFailed, got {other:?}"),
    }
}
