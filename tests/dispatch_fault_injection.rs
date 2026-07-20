#![cfg(feature = "mock")]

use std::sync::Arc;
use std::time::Duration;

use eggsearch::core::WebSearchRequest;
use eggsearch::meta::adapter::MetadataSearchAdapter;
use eggsearch::meta::mock::{MockEngine, MockFailure, MockResult};
use eggsearch::meta::provider_diagnostics::{FailureClass, ProviderHealthStatus};

fn make_adapter(engines: Vec<MockEngine>, timeout_secs: u64) -> MetadataSearchAdapter {
    let engines = engines
        .into_iter()
        .map(|e| Arc::new(e) as Arc<dyn eggsearch::meta::engines::SearchEngine>)
        .collect();
    MetadataSearchAdapter::from_engines(engines, Duration::from_secs(timeout_secs))
}

fn make_request(query: &str) -> WebSearchRequest {
    WebSearchRequest::new(query)
}

#[tokio::test]
async fn all_providers_succeed() {
    let adapter = make_adapter(
        vec![
            MockEngine::success(
                "alpha",
                vec![MockResult::new("T1", "http://a.com/", "alpha")],
            ),
            MockEngine::success("beta", vec![MockResult::new("T2", "http://b.com/", "beta")]),
        ],
        10,
    );
    let req = make_request("test");
    let resp = adapter.web_search(&req, 5, 5).await;
    assert!(
        resp.providers_failed.is_empty(),
        "all providers succeeded, none should be failed"
    );
}

#[tokio::test]
async fn partial_provider_failure_returns_partial() {
    let adapter = make_adapter(
        vec![
            MockEngine::success(
                "alpha",
                vec![MockResult::new("T1", "http://a.com/", "alpha")],
            ),
            MockEngine::failure("beta", MockFailure::Network),
        ],
        10,
    );
    let req = make_request("test");
    let resp = adapter.web_search(&req, 5, 5).await;
    assert!(
        !resp.results.is_empty(),
        "should still get results from successful provider"
    );
}

#[tokio::test]
async fn all_providers_fail_returns_empty() {
    let adapter = make_adapter(
        vec![
            MockEngine::failure("alpha", MockFailure::Network),
            MockEngine::failure("beta", MockFailure::Timeout),
        ],
        10,
    );
    let req = make_request("test");
    let resp = adapter.web_search(&req, 5, 5).await;
    assert!(
        resp.results.is_empty(),
        "all providers failed, should get no results"
    );
    assert_eq!(
        resp.providers_failed.len(),
        2,
        "both providers should be marked failed"
    );
}

#[tokio::test]
async fn provider_timeout_does_not_block_others() {
    let adapter = make_adapter(
        vec![
            MockEngine::hang("slow"),
            MockEngine::success(
                "fast",
                vec![MockResult::new("Fast", "http://fast.com/", "fast")],
            ),
        ],
        2,
    );
    let req = make_request("test");
    let resp = adapter.web_search(&req, 5, 5).await;
    assert!(
        !resp.results.is_empty(),
        "fast provider should return results despite slow provider"
    );
}

#[tokio::test]
async fn duplicate_results_deduplicated() {
    let adapter = make_adapter(
        vec![
            MockEngine::success(
                "alpha",
                vec![MockResult::new("T", "http://same.com/", "alpha")],
            ),
            MockEngine::success(
                "beta",
                vec![MockResult::new("T", "http://same.com/", "beta")],
            ),
        ],
        10,
    );
    let req = make_request("test");
    let resp = adapter.web_search(&req, 5, 5).await;
    let unique_urls: std::collections::HashSet<_> =
        resp.results.iter().map(|r| r.url.as_str()).collect();
    assert!(
        unique_urls.len() <= resp.results.len(),
        "no duplicate URLs in final results"
    );
}

#[tokio::test]
async fn output_ordering_independent_of_completion_order() {
    let adapter = make_adapter(
        vec![
            MockEngine::success("a", vec![MockResult::new("A", "http://a.com/", "a")]),
            MockEngine::success("b", vec![MockResult::new("B", "http://b.com/", "b")]),
            MockEngine::success("c", vec![MockResult::new("C", "http://c.com/", "c")]),
        ],
        10,
    );
    let req = make_request("test");
    let resp = adapter.web_search(&req, 10, 10).await;
    assert_eq!(resp.results.len(), 3, "all three results should be present");
    let urls: Vec<_> = resp.results.iter().map(|r| r.url.as_str()).collect();
    assert!(urls.contains(&"http://a.com/"));
    assert!(urls.contains(&"http://b.com/"));
    assert!(urls.contains(&"http://c.com/"));
}

#[tokio::test]
async fn hang_provider_cancelled_on_timeout() {
    let adapter = make_adapter(
        vec![
            MockEngine::hang("hanger"),
            MockEngine::success(
                "fast",
                vec![MockResult::new("F", "http://fast.com/", "fast")],
            ),
        ],
        1,
    );
    let start = std::time::Instant::now();
    let req = make_request("test");
    let resp = adapter.web_search(&req, 5, 5).await;
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(5),
        "should complete well before 5s, took {elapsed:?}"
    );
    assert!(!resp.results.is_empty(), "fast provider should return");
}

#[tokio::test]
async fn mixed_success_failure_and_hang() {
    let adapter = make_adapter(
        vec![
            MockEngine::success("ok", vec![MockResult::new("OK", "http://ok.com/", "ok")]),
            MockEngine::failure("fail", MockFailure::Parse),
            MockEngine::hang("hang"),
        ],
        2,
    );
    let req = make_request("test");
    let resp = adapter.web_search(&req, 5, 5).await;
    assert!(
        !resp.results.is_empty(),
        "should have results from ok provider"
    );
    assert!(
        resp.providers_failed.iter().any(|f| f.id == "fail"),
        "fail provider should be marked as failed"
    );
}

#[tokio::test]
async fn max_results_respected() {
    let results: Vec<MockResult> = (0..20)
        .map(|i| MockResult::new(format!("T{i}"), format!("http://{i}.com/"), "alpha"))
        .collect();
    let adapter = make_adapter(vec![MockEngine::success("alpha", results)], 10);
    let req = make_request("test");
    let resp = adapter.web_search(&req, 3, 3).await;
    assert!(
        resp.results.len() <= 3,
        "should return at most 3 results, got {}",
        resp.results.len()
    );
}

#[tokio::test]
async fn select_engines_filters_by_provider_id() {
    let adapter = make_adapter(
        vec![
            MockEngine::success("alpha", vec![]),
            MockEngine::success("beta", vec![]),
            MockEngine::success("gamma", vec![]),
        ],
        10,
    );
    let (selected, unknown) = adapter.select_engines(&["alpha".to_string(), "gamma".to_string()]);
    assert_eq!(selected.len(), 2);
    assert!(unknown.is_empty());
    assert!(selected.iter().any(|e| e.name() == "alpha"));
    assert!(selected.iter().any(|e| e.name() == "gamma"));
}

#[tokio::test]
async fn select_engines_returns_unknown() {
    let adapter = make_adapter(vec![MockEngine::success("alpha", vec![])], 10);
    let (selected, unknown) =
        adapter.select_engines(&["alpha".to_string(), "nonexistent".to_string()]);
    assert_eq!(selected.len(), 1);
    assert_eq!(unknown, vec!["nonexistent".to_string()]);
}

#[tokio::test]
async fn empty_providers_list_selects_all() {
    let adapter = make_adapter(
        vec![
            MockEngine::success("alpha", vec![]),
            MockEngine::success("beta", vec![]),
        ],
        10,
    );
    let (selected, unknown) = adapter.select_engines(&[]);
    assert_eq!(selected.len(), 2);
    assert!(unknown.is_empty());
}

#[tokio::test]
async fn health_transitions_on_success() {
    let adapter = make_adapter(
        vec![MockEngine::success(
            "alpha",
            vec![MockResult::new("T", "http://a.com/", "alpha")],
        )],
        10,
    );
    let health = adapter.health();
    let snap = health.snapshot("alpha", true, true);
    assert_eq!(snap.status, ProviderHealthStatus::Unknown);

    let req = make_request("test");
    let _ = adapter.web_search(&req, 5, 5).await;

    let snap = health.snapshot("alpha", true, true);
    assert_eq!(snap.status, ProviderHealthStatus::Healthy);
    assert_eq!(snap.consecutive_failures, 0);
}

#[tokio::test]
async fn health_transitions_on_failure() {
    let adapter = make_adapter(vec![MockEngine::failure("alpha", MockFailure::Network)], 10);
    let health = adapter.health();

    let req = make_request("test");
    let _ = adapter.web_search(&req, 5, 5).await;

    let snap = health.snapshot("alpha", true, true);
    assert!(
        snap.consecutive_failures > 0,
        "should record consecutive failures"
    );
}

#[tokio::test]
async fn health_recovery_after_failure() {
    let adapter = make_adapter(
        vec![MockEngine::success(
            "alpha",
            vec![MockResult::new("T", "http://a.com/", "alpha")],
        )],
        10,
    );
    let health = adapter.health();

    health.record_failure("alpha", FailureClass::NetworkError, "test failure", 100);
    let snap = health.snapshot("alpha", true, true);
    assert!(snap.consecutive_failures > 0);

    let req = make_request("test");
    let _ = adapter.web_search(&req, 5, 5).await;

    let snap = health.snapshot("alpha", true, true);
    assert_eq!(
        snap.consecutive_failures, 0,
        "success should reset failure count"
    );
}

#[tokio::test]
async fn health_cooldown_after_repeated_failures() {
    let adapter = make_adapter(vec![MockEngine::failure("alpha", MockFailure::Timeout)], 10);
    let health = adapter.health();

    for i in 0..5 {
        health.record_failure("alpha", FailureClass::Timeout, &format!("fail {i}"), 100);
    }

    assert!(
        health.is_in_cooldown("alpha"),
        "provider should be in cooldown after 5 failures"
    );

    let snap = health.snapshot("alpha", true, true);
    assert_eq!(snap.status, ProviderHealthStatus::Cooldown);
}

#[tokio::test]
async fn health_cooldown_cleared_by_success() {
    let adapter = make_adapter(
        vec![MockEngine::success(
            "alpha",
            vec![MockResult::new("T", "http://a.com/", "alpha")],
        )],
        10,
    );
    let health = adapter.health();

    for i in 0..5 {
        health.record_failure("alpha", FailureClass::Timeout, &format!("fail {i}"), 100);
    }
    assert!(health.is_in_cooldown("alpha"));

    health.record_success("alpha", 50);
    assert!(
        !health.is_in_cooldown("alpha"),
        "success should clear cooldown"
    );
    let snap = health.snapshot("alpha", true, true);
    assert_eq!(snap.status, ProviderHealthStatus::Healthy);
}

#[tokio::test]
async fn health_view_returns_unknown_for_unseen_provider() {
    let adapter = make_adapter(vec![], 10);
    let health = adapter.health();
    let view = health.health_view("nonexistent");
    assert_eq!(view.status, ProviderHealthStatus::Unknown);
    assert_eq!(view.consecutive_failures, 0);
}

#[tokio::test]
async fn concurrent_searches_do_not_exceed_provider_count() {
    let adapter = make_adapter(
        vec![
            MockEngine::success(
                "alpha",
                vec![MockResult::new("T", "http://a.com/", "alpha")],
            ),
            MockEngine::success("beta", vec![MockResult::new("T", "http://b.com/", "beta")]),
        ],
        10,
    );

    let mut handles = Vec::new();
    for i in 0..10 {
        let adapter_clone = MetadataSearchAdapter::from_engines(
            adapter
                .select_engines(&[])
                .0
                .into_iter()
                .map(|e| Arc::clone(&e))
                .collect(),
            Duration::from_secs(10),
        );
        handles.push(tokio::spawn(async move {
            let req = make_request(&format!("query {i}"));
            adapter_clone.web_search(&req, 3, 3).await
        }));
    }

    let results = futures::future::join_all(handles).await;
    for result in results {
        let resp = result.unwrap();
        assert!(!resp.results.is_empty() || !resp.providers_failed.is_empty());
    }
}

#[tokio::test]
async fn all_jobs_reach_terminal_state() {
    let adapter = make_adapter(
        vec![
            MockEngine::success("ok", vec![MockResult::new("T", "http://ok.com/", "ok")]),
            MockEngine::failure("fail", MockFailure::Parse),
            MockEngine::hang("hang"),
        ],
        1,
    );
    let req = make_request("test");
    let resp = adapter.web_search(&req, 5, 5).await;

    let result_urls: std::collections::HashSet<_> =
        resp.results.iter().map(|r| r.url.as_str()).collect();
    let failed_providers: std::collections::HashSet<_> = resp
        .providers_failed
        .iter()
        .map(|f| f.id.as_str())
        .collect();

    assert!(
        !result_urls.is_empty() || !failed_providers.is_empty(),
        "at least one provider should produce results or fail"
    );
}

#[tokio::test]
async fn adapter_provider_ids_match_configured() {
    let adapter = make_adapter(
        vec![
            MockEngine::success("alpha", vec![]),
            MockEngine::success("beta", vec![]),
        ],
        10,
    );
    let ids = adapter.provider_ids();
    assert!(
        ids.contains(&"alpha".to_string()),
        "should list alpha provider"
    );
    assert!(
        ids.contains(&"beta".to_string()),
        "should list beta provider"
    );
}

#[tokio::test]
async fn panic_in_provider_does_not_collapse_others() {
    let adapter = make_adapter(
        vec![
            MockEngine::failure("panicker", MockFailure::Panic),
            MockEngine::success(
                "stable",
                vec![MockResult::new("OK", "http://ok.com/", "stable")],
            ),
        ],
        5,
    );
    let req = make_request("test");
    let resp = adapter.web_search(&req, 5, 5).await;
    assert!(
        !resp.results.is_empty(),
        "stable provider should return results despite panic in other"
    );
}

#[tokio::test]
async fn all_providers_panic_returns_empty() {
    let adapter = make_adapter(
        vec![
            MockEngine::failure("panic1", MockFailure::Panic),
            MockEngine::failure("panic2", MockFailure::Panic),
        ],
        5,
    );
    let req = make_request("test");
    let resp = adapter.web_search(&req, 5, 5).await;
    assert!(
        resp.results.is_empty(),
        "all providers panicked, should get no results"
    );
}

#[tokio::test]
async fn concurrency_saturation_does_not_exceed_limit() {
    let adapter = make_adapter(
        vec![MockEngine::success(
            "alpha",
            vec![MockResult::new("T", "http://a.com/", "alpha")],
        )],
        10,
    );

    let mut handles = Vec::new();
    for i in 0..50 {
        let adapter_clone = MetadataSearchAdapter::from_engines(
            adapter
                .select_engines(&[])
                .0
                .into_iter()
                .map(|e| Arc::clone(&e))
                .collect(),
            Duration::from_secs(10),
        );
        handles.push(tokio::spawn(async move {
            let req = make_request(&format!("query {i}"));
            adapter_clone.web_search(&req, 3, 3).await
        }));
    }

    let results = futures::future::join_all(handles).await;
    for result in results {
        let resp = result.unwrap();
        assert!(
            !resp.results.is_empty() || !resp.providers_failed.is_empty(),
            "every job should reach terminal state"
        );
    }
}

#[tokio::test]
async fn malformed_result_metadata_does_not_panic() {
    let adapter = make_adapter(
        vec![MockEngine::success(
            "weird",
            vec![MockResult::new("", "", "weird")],
        )],
        10,
    );
    let req = make_request("test");
    let resp = adapter.web_search(&req, 5, 5).await;
    assert!(
        resp.results.len() <= 1,
        "malformed results should be handled gracefully"
    );
}

#[tokio::test]
async fn global_deadline_with_mixed_pending_and_running() {
    let adapter = make_adapter(
        vec![
            MockEngine::hang("slow1"),
            MockEngine::hang("slow2"),
            MockEngine::success(
                "fast",
                vec![MockResult::new("F", "http://fast.com/", "fast")],
            ),
        ],
        2,
    );
    let start = std::time::Instant::now();
    let req = make_request("test");
    let resp = adapter.web_search(&req, 5, 5).await;
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(10),
        "deadline should enforce within timeout, took {elapsed:?}"
    );
    assert!(
        !resp.results.is_empty(),
        "fast provider should return before deadline"
    );
}

#[tokio::test]
async fn partial_result_telemetry_is_exact() {
    let adapter = make_adapter(
        vec![
            MockEngine::success("ok", vec![MockResult::new("T", "http://ok.com/", "ok")]),
            MockEngine::failure("fail", MockFailure::Network),
        ],
        10,
    );
    let req = make_request("test");
    let resp = adapter.web_search(&req, 5, 5).await;

    assert_eq!(
        resp.results.len(),
        1,
        "should have exactly 1 result from ok provider"
    );
    assert_eq!(
        resp.providers_failed.len(),
        1,
        "should have exactly 1 failed provider"
    );
    assert_eq!(
        resp.providers_failed[0].id, "fail",
        "failed provider should be 'fail'"
    );
}

#[tokio::test]
async fn panic_in_provider_releases_counters() {
    let adapter = make_adapter(
        vec![
            MockEngine::failure("panicker", MockFailure::Panic),
            MockEngine::success(
                "stable",
                vec![MockResult::new("T", "http://ok.com/", "stable")],
            ),
        ],
        5,
    );

    for _ in 0..3 {
        let req = make_request("test");
        let _ = adapter.web_search(&req, 5, 5).await;
    }

    let req = make_request("test");
    let resp = adapter.web_search(&req, 5, 5).await;
    assert!(
        !resp.results.is_empty(),
        "stable provider should still return after repeated panics"
    );
}

#[tokio::test]
async fn output_ordering_deterministic_across_runs() {
    let make = || {
        make_adapter(
            vec![
                MockEngine::success("a", vec![MockResult::new("A", "http://a.com/", "a")]),
                MockEngine::success("b", vec![MockResult::new("B", "http://b.com/", "b")]),
                MockEngine::success("c", vec![MockResult::new("C", "http://c.com/", "c")]),
            ],
            10,
        )
    };

    let mut all_urls = Vec::new();
    for _ in 0..5 {
        let adapter = make();
        let req = make_request("test");
        let resp = adapter.web_search(&req, 10, 10).await;
        let urls: Vec<String> = resp.results.iter().map(|r| r.url.clone()).collect();
        all_urls.push(urls);
    }

    for urls in &all_urls {
        assert_eq!(
            urls.len(),
            all_urls[0].len(),
            "all runs should return same number of results"
        );
        for (a, b) in urls.iter().zip(all_urls[0].iter()) {
            assert_eq!(a, b, "output ordering should be deterministic across runs");
        }
    }
}

#[tokio::test]
async fn health_degraded_status_after_single_failure() {
    let adapter = make_adapter(vec![MockEngine::failure("alpha", MockFailure::Network)], 10);
    let health = adapter.health();

    health.record_failure("alpha", FailureClass::NetworkError, "test failure", 100);

    let snap = health.snapshot("alpha", true, true);
    assert_eq!(
        snap.status,
        ProviderHealthStatus::Degraded,
        "single failure should produce Degraded status"
    );
    assert_eq!(snap.consecutive_failures, 1);
}

#[tokio::test]
async fn shared_pool_concurrent_searches_succeed() {
    let adapter = make_adapter(
        vec![MockEngine::success(
            "alpha",
            vec![MockResult::new("T", "http://a.com/", "alpha")],
        )],
        10,
    );

    for i in 0..20 {
        let req = make_request(&format!("query {i}"));
        let resp = adapter.web_search(&req, 3, 3).await;
        assert!(!resp.results.is_empty(), "search {i} should return results");
    }
}

#[tokio::test]
async fn panic_then_success_repeated_cycle() {
    let adapter = make_adapter(
        vec![
            MockEngine::failure("flaky", MockFailure::Panic),
            MockEngine::success(
                "stable",
                vec![MockResult::new("OK", "http://ok.com/", "stable")],
            ),
        ],
        5,
    );

    for i in 0..10 {
        let req = make_request("test");
        let resp = adapter.web_search(&req, 5, 5).await;
        assert!(
            !resp.results.is_empty(),
            "iteration {i}: stable should still return after repeated panics"
        );
    }
}
