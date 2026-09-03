use std::sync::Arc;
use std::time::Duration;

use eggsearch::core::config::{ApiProviderConfig, AppConfig};
use eggsearch::core::provider::{
    built_in_provider_descriptor, credential_requirement, is_api_provider, CredentialRequirement,
    API_PROVIDER_IDS, KNOWN_PROVIDER_IDS,
};
use eggsearch::core::query::{Freshness, SearchDateRange, WebSearchRequest};
use eggsearch::meta::engines::request::EngineSearchRequest;
use eggsearch::meta::engines::ExaEngine;
use eggsearch::meta::engines::SearchEngine;
use eggsearch::meta::provider_diagnostics::CapabilityEnforcementTelemetry;

fn engine_req(query: &str, max_results: usize) -> EngineSearchRequest {
    EngineSearchRequest::simple(query, max_results, Duration::from_secs(5))
}

#[test]
fn provider_inventory_reflects_new_count() {
    assert!(KNOWN_PROVIDER_IDS.contains(&"exa"));
    assert!(API_PROVIDER_IDS.contains(&"exa"));
    assert!(is_api_provider("exa"));
    assert_eq!(KNOWN_PROVIDER_IDS.len(), 36);
    assert_eq!(
        credential_requirement("exa"),
        CredentialRequirement::Required
    );
}

#[test]
fn descriptor_claims_only_implemented_capabilities() {
    let desc = built_in_provider_descriptor("exa", true, false, true, true, None, None)
        .expect("descriptor");
    assert_eq!(desc.id, "exa");
    assert_eq!(desc.display_name, "Exa Semantic Search");
    assert_eq!(desc.kind, eggsearch::core::provider::ProviderKind::ApiKey);
    assert!(desc.requires_api_key);
    assert!(desc.capabilities.supports_freshness);
    assert!(desc.capabilities.supports_domain_filters);
    assert!(desc.capabilities.supports_result_timestamps);
    assert!(!desc.capabilities.supports_safe_search);
    assert!(!desc.capabilities.supports_language);
    assert!(!desc.capabilities.supports_region);
    assert!(!desc.capabilities.supports_news);
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
        "exa".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: Some("EGGSEARCH_TEST_EXA_MISSING_KEY".to_string()),
            base_url: None,
        },
    );
    assert!(std::env::var("EGGSEARCH_TEST_EXA_MISSING_KEY").is_err());
    assert!(!cfg.provider_is_available("exa"));
    let enabled = cfg.effective_provider_ids();
    assert!(!enabled.contains(&"exa".to_string()));
    let (_, skipped) =
        eggsearch::meta::adapter::build_default_engines(&enabled, None, None, &cfg.search.api)
            .expect("build");
    assert!(!skipped.iter().any(|s| s.id == "exa"));
    let explicit_enabled = vec!["exa".to_string()];
    let (_, skipped_explicit) = eggsearch::meta::adapter::build_default_engines(
        &explicit_enabled,
        None,
        None,
        &cfg.search.api,
    )
    .expect("explicit build");
    let skip = skipped_explicit
        .iter()
        .find(|s| s.id == "exa")
        .expect("exa skipped when credential missing");
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
        .find(|d| d.id == "exa")
        .expect("exa in status");
    assert!(!desc.routable);
    assert!(desc.skip_code.is_some());
    assert!(desc.skip_reason.is_some());
}

#[test]
fn default_config_leaves_exa_out_of_defaults() {
    let cfg = AppConfig::default();
    let resolved = cfg.resolve_providers(&[]).expect("defaults resolve");
    assert!(!resolved.contains(&"exa".to_string()));
    assert!(resolved.contains(&"duckduckgo".to_string()));
}

#[test]
fn configured_exa_still_not_in_defaults_without_operator_opt_in() {
    std::env::set_var("EGGSEARCH_TEST_EXA_KEY_DEFAULTS", "test-key");
    let mut cfg = AppConfig::default();
    cfg.search.api.insert(
        "exa".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: Some("EGGSEARCH_TEST_EXA_KEY_DEFAULTS".to_string()),
            base_url: None,
        },
    );
    assert!(cfg.provider_is_available("exa"));
    let resolved = cfg.resolve_providers(&[]).expect("defaults resolve");
    assert!(
        !resolved.contains(&"exa".to_string()),
        "exa must never join defaults automatically, got {resolved:?}"
    );
    let explicit = cfg
        .resolve_providers(&["exa".to_string()])
        .expect("explicit exa resolves when configured");
    assert_eq!(explicit, vec!["exa".to_string()]);
    std::env::remove_var("EGGSEARCH_TEST_EXA_KEY_DEFAULTS");
}

#[test]
fn telemetry_reports_native_domain_and_date_enforcement() {
    let mut req = WebSearchRequest::new("test");
    req.include_domains = vec!["example.com".to_string()];
    req.date_range = Some(SearchDateRange::new("2024-01-01", "2024-01-31"));
    let tele = CapabilityEnforcementTelemetry::for_web_search(&req, &["exa".to_string()]);
    assert!(tele.enforced.iter().any(|c| c == "domain_filters"));
    assert!(tele.enforced.iter().any(|c| c == "date_range"));
    assert!(!tele.approximated.iter().any(|c| c == "domain_filters"));

    let mut req2 = WebSearchRequest::new("test");
    req2.include_domains = vec!["example.com".to_string()];
    let tele2 = CapabilityEnforcementTelemetry::for_web_search(&req2, &["duckduckgo".to_string()]);
    assert!(tele2.approximated.iter().any(|c| c == "domain_filters"));
    assert!(!tele2.enforced.iter().any(|c| c == "domain_filters"));

    let mut req3 = WebSearchRequest::new("test");
    req3.freshness = Freshness::Week;
    let tele3 = CapabilityEnforcementTelemetry::for_web_search(&req3, &["exa".to_string()]);
    assert!(tele3.enforced.iter().any(|c| c == "freshness"));
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn configured_key_sends_x_api_key_header() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/search")
            .header("x-api-key", "test-exa-key-123");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"results": []}"#);
    });
    let client = reqwest::Client::new();
    let results = eggsearch::meta::engines::exa::search(
        &client,
        "test-exa-key-123",
        Some(&server.url("/search")),
        &engine_req("rust", 5),
    )
    .await
    .expect("search succeeds");
    assert!(results.is_empty());
    mock.assert();
}

#[test]
fn api_key_never_rendered_in_diagnostics() {
    let client = Arc::new(reqwest::Client::new());
    let engine = ExaEngine {
        client,
        api_key: "super-secret-exa-key".to_string(),
        base_url: None,
    };
    let label = format!("{}:{}", engine.name(), "exa");
    assert!(!label.contains("super-secret-exa-key"));
    let err = eggsearch::meta::engines::error::EngineError::BadStatus {
        engine: "exa",
        status: 401,
    };
    assert!(!err.to_string().contains("super-secret-exa-key"));
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn default_request_has_no_summary_or_fetch_fields() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/search")
            .json_body(serde_json::json!({
                "query": "rust",
                "numResults": 5,
                "type": "auto"
            }));
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"results": []}"#);
    });
    let client = reqwest::Client::new();
    eggsearch::meta::engines::exa::search(
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
async fn exact_date_range_maps_to_published_dates() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/search")
            .json_body(serde_json::json!({
                "query": "rust",
                "numResults": 5,
                "type": "auto",
                "startPublishedDate": "2024-01-01T00:00:00.000Z",
                "endPublishedDate": "2024-01-31T23:59:59.999Z"
            }));
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"results": []}"#);
    });
    let client = reqwest::Client::new();
    let mut req = engine_req("rust", 5);
    req.date_range = Some(SearchDateRange::new("2024-01-01", "2024-01-31"));
    eggsearch::meta::engines::exa::search(&client, "k", Some(&server.url("/search")), &req)
        .await
        .expect("ok");
    mock.assert();
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn relative_freshness_sends_utc_start_without_end() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/search")
            .body_contains("startPublishedDate");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"results": []}"#);
    });
    let end_probe = server.mock(|when, then| {
        when.method(POST)
            .path("/search")
            .body_contains("endPublishedDate");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"results": []}"#);
    });
    let client = reqwest::Client::new();
    let mut req = engine_req("rust", 5);
    req.freshness = Freshness::Week;
    eggsearch::meta::engines::exa::search(&client, "k", Some(&server.url("/search")), &req)
        .await
        .expect("ok");
    mock.assert();
    assert_eq!(
        end_probe.hits(),
        0,
        "relative freshness must omit endPublishedDate"
    );
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn include_exclude_domains_map_natively() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/search")
            .json_body(serde_json::json!({
                "query": "rust",
                "numResults": 5,
                "type": "auto",
                "includeDomains": ["example.com"],
                "excludeDomains": ["spam.example"]
            }));
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"results": []}"#);
    });
    let client = reqwest::Client::new();
    let mut req = engine_req("rust", 5);
    req.include_domains = vec!["example.com".to_string()];
    req.exclude_domains = vec!["spam.example".to_string()];
    eggsearch::meta::engines::exa::search(&client, "k", Some(&server.url("/search")), &req)
        .await
        .expect("ok");
    mock.assert();
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn published_date_maps_to_generic_timestamp() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/search");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                "results": [
                    {"title": "Fresh", "url": "https://example.com/fresh", "publishedDate": "2024-05-01T10:00:00.000Z"},
                    {"title": "Old", "url": "https://example.com/old", "publishedDate": "2020-01-01T00:00:00.000Z"}
                ]
            }"#,
            );
    });
    let client = reqwest::Client::new();
    let results = eggsearch::meta::engines::exa::search(
        &client,
        "k",
        Some(&server.url("/search")),
        &engine_req("test", 10),
    )
    .await
    .expect("ok");
    assert_eq!(results.len(), 2);
    assert!(results[0].published_at.is_some());
    assert!(results[1].published_at.is_some());
    let fresh = results[0].published_at.as_deref().expect("fresh ts");
    assert!(fresh.contains("2024-05-01"));
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn invalid_published_date_keeps_valid_result() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/search");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                "results": [
                    {"title": "Bad date", "url": "https://example.com/a", "publishedDate": "not-a-date"},
                    {"title": "Good", "url": "https://example.com/b", "publishedDate": "2024-01-15"}
                ]
            }"#,
            );
    });
    let client = reqwest::Client::new();
    let results = eggsearch::meta::engines::exa::search(
        &client,
        "k",
        Some(&server.url("/search")),
        &engine_req("test", 10),
    )
    .await
    .expect("ok");
    assert_eq!(results.len(), 2);
    assert!(results[0].published_at.is_none());
    assert!(results[1].published_at.is_some());
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn highlights_absent_without_demand_then_bounded() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let with_mock = server.mock(|when, then| {
        when.method(POST)
            .path("/search")
            .body_contains("highlights");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"results": []}"#);
    });
    let plain_mock = server.mock(|when, then| {
        when.method(POST).path("/plain");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"results": []}"#);
    });
    let highlight_probe = server.mock(|when, then| {
        when.method(POST).path("/plain").body_contains("highlights");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"results": []}"#);
    });
    let client = reqwest::Client::new();
    let mut req = engine_req("rust", 5);
    req.excerpt_count = 2;
    eggsearch::meta::engines::exa::search(&client, "k", Some(&server.url("/search")), &req)
        .await
        .expect("ok");
    with_mock.assert();
    let plain = engine_req("rust", 5);
    eggsearch::meta::engines::exa::search(&client, "k", Some(&server.url("/plain")), &plain)
        .await
        .expect("ok");
    plain_mock.assert();
    assert_eq!(
        highlight_probe.hits(),
        0,
        "highlights must not be requested without excerpt demand"
    );

    server.mock(|when, then| {
        when.method(POST).path("/bounded");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                "results": [
                    {"title": "T", "url": "https://example.com/a",
                     "highlights": ["h1", "", "h2", "h3", "h4"],
                     "highlightScores": [0.9, 0.1, 0.8, 0.2, 0.3]}
                ]
            }"#,
            );
    });
    let mut bounded = engine_req("test", 5);
    bounded.excerpt_count = 3;
    let results = eggsearch::meta::engines::exa::search(
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
        eggsearch::core::source_card::ExcerptProvenance::ProviderHighlight
    )));
    assert!(results[0].excerpts.iter().all(|e| e.text.chars().count()
        <= eggsearch::core::source_card::MAX_EXCERPT_CHARS
        || !e.text.is_empty()));
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn highlight_scores_are_provider_local_and_deterministic() {
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
                     "highlights": ["low", "high", "mid"],
                     "highlightScores": [0.1, 0.9, 0.5]}
                ]
            }"#,
            );
    });
    let client = reqwest::Client::new();
    let mut req = engine_req("test", 5);
    req.excerpt_count = 3;
    let first =
        eggsearch::meta::engines::exa::search(&client, "k", Some(&server.url("/search")), &req)
            .await
            .expect("ok");
    let second =
        eggsearch::meta::engines::exa::search(&client, "k", Some(&server.url("/search")), &req)
            .await
            .expect("ok");
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].excerpts.len(), 3);
    assert_eq!(first[0].excerpts[0].score, Some(0.1));
    assert_eq!(first[0].excerpts[1].score, Some(0.9));
    assert_eq!(first[0].excerpts[2].score, Some(0.5));
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
        let err = eggsearch::meta::engines::exa::search(
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
                assert_eq!(engine, "exa");
                assert_eq!(got, status);
                assert!(!err.to_string().contains("test-exa-key"));
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
        "{{\"results\": [{{\"title\": \"T\", \"url\": \"https://example.com/a\", \"highlights\": [\"{big}\"]}}]}}"
    );
    server.mock(|when, then| {
        when.method(POST).path("/search");
        then.status(200)
            .header("content-type", "application/json")
            .body(body.clone());
    });
    let client = reqwest::Client::new();
    let err = eggsearch::meta::engines::exa::search(
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
