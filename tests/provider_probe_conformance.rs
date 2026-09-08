#![cfg(feature = "mock")]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use eggsearch::core::config::AppConfig;
use eggsearch::core::provider::{built_in_provider_descriptor, KNOWN_PROVIDER_IDS};
use eggsearch::mcp::state::ServerState;
use eggsearch::mcp::tools::{run_provider_status, run_provider_status_async, ProviderStatusArgs};
use eggsearch::meta::mock::{mock_engines, MockEngine, MockFailure, MockResult};
use eggsearch::meta::probe::{
    bound_probe_message, probe_providers, ProviderProbeRequest, PROBE_AGGREGATE_TIMEOUT_MS,
    PROBE_MAX_CONCURRENCY, PROBE_MAX_MESSAGE_CHARS, PROBE_MAX_RESULTS,
    PROBE_PER_PROVIDER_TIMEOUT_MS,
};
use eggsearch::meta::MetadataSearchAdapter;

fn mock_state(engines: Vec<MockEngine>) -> Arc<ServerState> {
    let cfg = AppConfig::default();
    let adapter =
        MetadataSearchAdapter::from_engines(mock_engines(engines), Duration::from_secs(5));
    Arc::new(ServerState::with_adapter(cfg, Arc::new(adapter)))
}

fn mock_adapter(engines: Vec<MockEngine>) -> MetadataSearchAdapter {
    MetadataSearchAdapter::from_engines(mock_engines(engines), Duration::from_secs(5))
}

#[test]
#[allow(clippy::assertions_on_constants)]
fn probe_budget_constants_are_bounded() {
    assert_eq!(PROBE_MAX_RESULTS, 1);
    assert!(PROBE_PER_PROVIDER_TIMEOUT_MS > 0);
    assert!(PROBE_AGGREGATE_TIMEOUT_MS >= PROBE_PER_PROVIDER_TIMEOUT_MS);
    assert!((1..=8).contains(&PROBE_MAX_CONCURRENCY));
    assert!(PROBE_MAX_MESSAGE_CHARS <= 512);
}

#[tokio::test]
async fn probe_success_records_health() {
    let adapter = mock_adapter(vec![MockEngine::success(
        "duckduckgo",
        vec![MockResult::new(
            "Hit",
            "https://example.com/hit",
            "duckduckgo",
        )],
    )]);
    let summary = probe_providers(&adapter, ProviderProbeRequest::default()).await;
    assert!(summary.requested);
    assert!(summary.implemented);
    assert_eq!(summary.started, 1);
    assert_eq!(summary.succeeded, 1);
    assert_eq!(summary.failed, 0);
    let outcome = summary
        .outcomes
        .iter()
        .find(|o| o.provider_id == "duckduckgo")
        .expect("duckduckgo outcome");
    assert!(outcome.attempted);
    assert!(outcome.routable);
    assert!(outcome.success);
    assert!(outcome.failure_class.is_none());
    assert!(outcome.latency_ms.is_some());
    assert!(outcome.skip_code.is_none());
    let view = adapter.health().health_view("duckduckgo");
    assert_eq!(
        view.status,
        eggsearch::meta::provider_diagnostics::ProviderHealthStatus::Healthy
    );
}

#[tokio::test]
async fn probe_missing_api_key_is_skipped_not_failed() {
    let adapter = mock_adapter(vec![MockEngine::success(
        "duckduckgo",
        vec![MockResult::new("Hit", "https://example.com/", "duckduckgo")],
    )]);
    let summary = probe_providers(
        &adapter,
        ProviderProbeRequest {
            providers: vec!["brave_api".to_string()],
            timeout_per_provider_ms: None,
            max_results: None,
        },
    )
    .await;
    assert_eq!(summary.started, 0);
    assert_eq!(summary.skipped, 1);
    let outcome = &summary.outcomes[0];
    assert_eq!(outcome.provider_id, "brave_api");
    assert!(!outcome.attempted);
    assert!(!outcome.routable);
    assert!(!outcome.success);
    assert!(outcome.skip_code.is_some());
}

#[tokio::test]
async fn probe_unknown_provider_uses_stable_skip_code() {
    let adapter = mock_adapter(vec![]);
    let summary = probe_providers(
        &adapter,
        ProviderProbeRequest {
            providers: vec!["ghost_xyz".to_string()],
            timeout_per_provider_ms: None,
            max_results: None,
        },
    )
    .await;
    assert_eq!(summary.started, 0);
    assert_eq!(summary.skipped, 1);
    let outcome = &summary.outcomes[0];
    assert!(!outcome.attempted);
    assert_eq!(
        outcome.skip_code,
        Some(eggsearch::core::provider::ProviderSkipCode::UnknownProvider)
    );
}

#[tokio::test]
async fn probe_timeout_is_bounded_failure() {
    let adapter = mock_adapter(vec![MockEngine::hang("duckduckgo")]);
    let summary = probe_providers(
        &adapter,
        ProviderProbeRequest {
            providers: vec!["duckduckgo".to_string()],
            timeout_per_provider_ms: Some(50),
            max_results: None,
        },
    )
    .await;
    assert_eq!(summary.started, 1);
    assert_eq!(summary.failed, 1);
    let outcome = &summary.outcomes[0];
    assert!(outcome.attempted);
    assert!(!outcome.success);
    assert_eq!(outcome.failure_class.as_deref(), Some("timeout"));
    assert!(outcome.latency_ms.unwrap_or(0) < 5_000);
}

#[tokio::test]
async fn probe_http_error_preserves_status() {
    let adapter = mock_adapter(vec![MockEngine::failure(
        "duckduckgo",
        MockFailure::HttpStatus(503),
    )]);
    let summary = probe_providers(
        &adapter,
        ProviderProbeRequest {
            providers: vec!["duckduckgo".to_string()],
            timeout_per_provider_ms: None,
            max_results: None,
        },
    )
    .await;
    let outcome = &summary.outcomes[0];
    assert!(!outcome.success);
    assert_eq!(outcome.failure_class.as_deref(), Some("http_status"));
    assert_eq!(outcome.http_status, Some(503));
}

#[tokio::test]
async fn probe_rate_limit_maps_to_rate_limited() {
    let adapter = mock_adapter(vec![MockEngine::failure(
        "duckduckgo",
        MockFailure::HttpStatus(429),
    )]);
    let summary = probe_providers(
        &adapter,
        ProviderProbeRequest {
            providers: vec!["duckduckgo".to_string()],
            timeout_per_provider_ms: None,
            max_results: None,
        },
    )
    .await;
    let outcome = &summary.outcomes[0];
    assert_eq!(outcome.failure_class.as_deref(), Some("rate_limited"));
    assert_eq!(outcome.http_status, Some(429));
}

#[tokio::test]
async fn probe_parser_failure_maps_correctly() {
    let adapter = mock_adapter(vec![MockEngine::failure("duckduckgo", MockFailure::Parse)]);
    let summary = probe_providers(
        &adapter,
        ProviderProbeRequest {
            providers: vec!["duckduckgo".to_string()],
            timeout_per_provider_ms: None,
            max_results: None,
        },
    )
    .await;
    let outcome = &summary.outcomes[0];
    assert_eq!(outcome.failure_class.as_deref(), Some("parse_error"));
    assert!(outcome.http_status.is_none());
}

#[tokio::test]
async fn probe_network_failure_maps_correctly() {
    let adapter = mock_adapter(vec![MockEngine::failure(
        "duckduckgo",
        MockFailure::Network,
    )]);
    let summary = probe_providers(
        &adapter,
        ProviderProbeRequest {
            providers: vec!["duckduckgo".to_string()],
            timeout_per_provider_ms: None,
            max_results: None,
        },
    )
    .await;
    let outcome = &summary.outcomes[0];
    assert_eq!(outcome.failure_class.as_deref(), Some("network_error"));
}

#[tokio::test]
async fn probe_panic_is_contained() {
    let adapter = mock_adapter(vec![MockEngine::failure("duckduckgo", MockFailure::Panic)]);
    let summary = probe_providers(
        &adapter,
        ProviderProbeRequest {
            providers: vec!["duckduckgo".to_string()],
            timeout_per_provider_ms: None,
            max_results: None,
        },
    )
    .await;
    let outcome = &summary.outcomes[0];
    assert!(outcome.attempted);
    assert!(!outcome.success);
    assert_eq!(outcome.failure_class.as_deref(), Some("panic"));
}

#[tokio::test]
async fn probe_failures_update_cooldown() {
    let adapter = mock_adapter(vec![MockEngine::failure(
        "duckduckgo",
        MockFailure::HttpStatus(429),
    )]);
    for _ in 0..3 {
        let _ = probe_providers(
            &adapter,
            ProviderProbeRequest {
                providers: vec!["duckduckgo".to_string()],
                timeout_per_provider_ms: None,
                max_results: None,
            },
        )
        .await;
    }
    assert!(adapter.health().is_in_cooldown("duckduckgo"));
}

#[tokio::test]
async fn probe_explicit_request_still_routes_after_degraded() {
    use eggsearch::meta::provider_diagnostics::resolve_provider_routing;
    let adapter = mock_adapter(vec![MockEngine::failure(
        "duckduckgo",
        MockFailure::Timeout,
    )]);
    let _ = probe_providers(
        &adapter,
        ProviderProbeRequest {
            providers: vec!["duckduckgo".to_string()],
            timeout_per_provider_ms: Some(20),
            max_results: None,
        },
    )
    .await;
    let cfg = AppConfig::default();
    let decision = resolve_provider_routing(
        &["duckduckgo".to_string()],
        None,
        adapter.provider_ids(),
        &cfg,
        adapter.health(),
        true,
    )
    .expect("explicit routing should succeed");
    assert_eq!(decision.selected_providers, vec!["duckduckgo".to_string()]);
}

#[test]
fn probe_messages_are_bounded_and_sanitized() {
    let long = "x".repeat(PROBE_MAX_MESSAGE_CHARS + 100);
    let bounded = bound_probe_message(&long).expect("bounded");
    assert!(bounded.chars().count() <= PROBE_MAX_MESSAGE_CHARS + 1);
    assert!(bound_probe_message("  \u{0}  ").is_none());
    let with_control = "ok\u{0}\u{1}test";
    let cleaned = bound_probe_message(with_control).expect("cleaned");
    assert!(!cleaned.contains('\u{0}'));
}

#[tokio::test]
async fn probe_never_leaks_credentials() {
    std::env::set_var("EGGSEARCH_PROBE_TEST_KEY", "ghp_super_secret_123");
    let adapter = mock_adapter(vec![MockEngine::failure(
        "duckduckgo",
        MockFailure::Network,
    )]);
    let summary = probe_providers(
        &adapter,
        ProviderProbeRequest {
            providers: vec!["duckduckgo".to_string()],
            timeout_per_provider_ms: None,
            max_results: None,
        },
    )
    .await;
    std::env::remove_var("EGGSEARCH_PROBE_TEST_KEY");
    let json = serde_json::to_string(&summary).unwrap();
    assert!(!json.contains("ghp_super_secret_123"));
    assert!(!json.contains("EGGSEARCH_PROBE_TEST_KEY"));
}

#[tokio::test]
async fn provider_status_probe_true_uses_shared_service() {
    let state = mock_state(vec![MockEngine::success(
        "duckduckgo",
        vec![MockResult::new(
            "Hit",
            "https://example.com/hit",
            "duckduckgo",
        )],
    )]);
    let v = run_provider_status_async(
        state,
        ProviderStatusArgs {
            probe: true,
            recipe_detail: None,
        },
    )
    .await
    .expect("ok");
    let probe = v["probe"].as_object().expect("probe object");
    assert_eq!(probe["requested"], serde_json::json!(true));
    assert_eq!(probe["implemented"], serde_json::json!(true));
    assert!(probe["outcomes"].is_array());
}

#[test]
fn provider_status_probe_false_is_cheap_and_network_free() {
    let state = mock_state(vec![MockEngine::hang("duckduckgo")]);
    let v = run_provider_status(
        state,
        ProviderStatusArgs {
            probe: false,
            recipe_detail: None,
        },
    )
    .expect("ok");
    assert_eq!(v["probe"]["requested"], serde_json::json!(false));
    assert_eq!(v["probe"]["implemented"], serde_json::json!(true));
}

#[test]
fn capability_descriptors_are_source_of_truth() {
    let brave = built_in_provider_descriptor("brave_api", true, false, true, false, None, None)
        .expect("brave_api");
    assert!(brave.capabilities.supports_safe_search);
    assert!(brave.capabilities.supports_freshness);
    assert!(brave.capabilities.supports_language);
    assert!(brave.capabilities.supports_region);
    assert!(!brave.capabilities.supports_domain_filters);
    assert!(brave.capabilities.supports_news);
    assert!(brave.capabilities.supports_result_timestamps);

    let exa =
        built_in_provider_descriptor("exa", true, false, true, false, None, None).expect("exa");
    assert!(exa.capabilities.supports_freshness);
    assert!(exa.capabilities.supports_domain_filters);
    assert!(exa.capabilities.supports_result_timestamps);
    assert!(!exa.capabilities.supports_safe_search);
    assert!(!exa.capabilities.supports_language);
    assert!(!exa.capabilities.supports_region);
    assert!(!exa.capabilities.supports_news);

    let tavily = built_in_provider_descriptor("tavily", true, false, true, false, None, None)
        .expect("tavily");
    assert!(tavily.capabilities.supports_safe_search);
    assert!(tavily.capabilities.supports_freshness);
    assert!(tavily.capabilities.supports_language);
    assert!(tavily.capabilities.supports_region);
    assert!(tavily.capabilities.supports_domain_filters);
    assert!(tavily.capabilities.supports_news);
    assert!(!tavily.capabilities.supports_result_timestamps);

    for id in ["duckduckgo", "brave", "startpage", "yahoo", "mojeek"] {
        let desc =
            built_in_provider_descriptor(id, true, false, true, false, None, None).expect(id);
        assert!(
            !desc.capabilities.supports_domain_filters,
            "{id} must not claim native domain filters"
        );
    }
}

#[test]
fn web_search_capability_telemetry_matches_descriptors() {
    use eggsearch::core::query::WebSearchRequest;
    use eggsearch::meta::provider_diagnostics::CapabilityEnforcementTelemetry;

    let mut req = WebSearchRequest::new("example");
    req.include_domains = vec!["example.com".to_string()];

    let native = CapabilityEnforcementTelemetry::for_web_search(&req, &["exa".to_string()]);
    assert!(native.enforced.contains(&"domain_filters".to_string()));

    let native_tavily =
        CapabilityEnforcementTelemetry::for_web_search(&req, &["tavily".to_string()]);
    assert!(native_tavily
        .enforced
        .contains(&"domain_filters".to_string()));

    let approx = CapabilityEnforcementTelemetry::for_web_search(&req, &["duckduckgo".to_string()]);
    assert!(approx.approximated.contains(&"domain_filters".to_string()));

    let brave_req = WebSearchRequest::new("example");
    let brave_tel =
        CapabilityEnforcementTelemetry::for_web_search(&brave_req, &["brave_api".to_string()]);
    assert!(brave_tel.requested.is_empty());
}

#[test]
fn all_known_ids_have_descriptors() {
    for id in KNOWN_PROVIDER_IDS {
        assert!(
            built_in_provider_descriptor(id, true, false, true, false, None, None).is_some(),
            "missing descriptor for {id}"
        );
    }
}

#[test]
fn probe_timeout_sink_observes_narrow_request() {
    let sink: Arc<Mutex<Option<std::time::Duration>>> = Arc::new(Mutex::new(None));
    let engine = eggsearch::meta::mock::MockEngine::record_timeout("duckduckgo", sink.clone());
    let adapter =
        MetadataSearchAdapter::from_engines(vec![Arc::new(engine)], Duration::from_secs(5));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(probe_providers(
        &adapter,
        ProviderProbeRequest {
            providers: vec!["duckduckgo".to_string()],
            timeout_per_provider_ms: Some(1234),
            max_results: None,
        },
    ));
    let observed = sink.lock().unwrap().expect("timeout recorded");
    assert_eq!(observed, Duration::from_millis(1234));
}
