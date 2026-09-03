use std::sync::Arc;
use std::time::Duration;

use eggsearch::core::config::{ApiProviderConfig, AppConfig};
use eggsearch::core::evidence_role::EvidenceRole;
use eggsearch::core::provider::{
    built_in_provider_descriptor, credential_requirement, is_optional_api_provider,
    CredentialRequirement, KNOWN_PROVIDER_IDS, OPTIONAL_API_PROVIDER_IDS,
};
use eggsearch::meta::engines::request::{EngineSearchRequest, RepoScope};
use eggsearch::meta::engines::FirecrawlDeveloperEngine;
use eggsearch::meta::engines::SearchEngine;
use eggsearch::meta::MetadataSearchAdapter;

fn engine_req(query: &str, max_results: usize) -> EngineSearchRequest {
    EngineSearchRequest::simple(query, max_results, Duration::from_secs(5))
}

#[test]
fn provider_inventory_reflects_new_count() {
    assert!(KNOWN_PROVIDER_IDS.contains(&"firecrawl_developer"));
    assert_eq!(KNOWN_PROVIDER_IDS.len(), 35);
    assert!(OPTIONAL_API_PROVIDER_IDS.contains(&"firecrawl_developer"));
    assert!(is_optional_api_provider("firecrawl_developer"));
    assert_eq!(
        credential_requirement("firecrawl_developer"),
        CredentialRequirement::Optional
    );
    assert_eq!(
        credential_requirement("brave_api"),
        CredentialRequirement::Required
    );
    assert_eq!(
        credential_requirement("duckduckgo"),
        CredentialRequirement::None
    );
}

#[test]
fn descriptor_does_not_require_key_or_claim_code_search() {
    let desc =
        built_in_provider_descriptor("firecrawl_developer", true, false, true, true, None, None)
            .expect("descriptor");
    assert_eq!(desc.id, "firecrawl_developer");
    assert!(!desc.requires_api_key);
    assert!(!desc.capabilities.supports_code_search);
    assert!(!desc.capabilities.supports_release_search);
    assert!(!desc.capabilities.supports_scholarly_search);
    assert!(!desc.capabilities.supports_repo_indexing);
    assert!(desc.capabilities.supports_issue_search);
    assert!(desc.capabilities.supports_repo_filter);
}

#[test]
fn default_config_disables_firecrawl_and_default_resolution_unchanged() {
    let cfg = AppConfig::default();
    assert_eq!(
        cfg.search.providers.get("firecrawl_developer"),
        Some(&false)
    );
    let resolved = cfg.resolve_providers(&[]).expect("defaults resolve");
    assert!(!resolved.contains(&"firecrawl_developer".to_string()));
    assert!(resolved.contains(&"duckduckgo".to_string()));
}

#[test]
fn provider_builds_and_routable_keyless_when_enabled() {
    let mut cfg = AppConfig::default();
    cfg.search
        .providers
        .insert("firecrawl_developer".to_string(), true);
    assert!(cfg.provider_is_available("firecrawl_developer"));
    let enabled = cfg.effective_provider_ids();
    assert!(enabled.contains(&"firecrawl_developer".to_string()));
    let (engines, skipped) =
        eggsearch::meta::adapter::build_default_engines(&enabled, None, None, &cfg.search.api)
            .expect("build");
    assert!(engines.iter().any(|e| e.name() == "firecrawl_developer"));
    assert!(!skipped.iter().any(|s| s.id == "firecrawl_developer"));
    let state = eggsearch::mcp::state::ServerState::build(cfg).expect("state builds keyless");
    let status = state.adapter.provider_status();
    let desc = status
        .iter()
        .find(|d| d.id == "firecrawl_developer")
        .expect("status contains firecrawl");
    assert!(desc.enabled);
    assert!(desc.configured);
    assert!(desc.routable);
    assert!(desc.skip_code.is_none());
    assert!(!desc.requires_api_key);
}

#[test]
fn absent_optional_key_does_not_produce_missing_api_key() {
    let mut cfg = AppConfig::default();
    cfg.search
        .providers
        .insert("firecrawl_developer".to_string(), true);
    cfg.search.api.insert(
        "firecrawl_developer".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: Some("EGGSEARCH_TEST_FIRECRAWL_MISSING_KEY".to_string()),
            base_url: None,
        },
    );
    assert!(std::env::var("EGGSEARCH_TEST_FIRECRAWL_MISSING_KEY").is_err());
    cfg.validate().expect("optional missing key validates");
    assert!(cfg.provider_is_available("firecrawl_developer"));
    let enabled = cfg.effective_provider_ids();
    let (_, skipped) =
        eggsearch::meta::adapter::build_default_engines(&enabled, None, None, &cfg.search.api)
            .expect("build");
    assert!(!skipped.iter().any(|s| s.id == "firecrawl_developer"));
}

#[test]
fn explicitly_empty_optional_credential_falls_back_keyless() {
    std::env::set_var("EGGSEARCH_TEST_FIRECRAWL_EMPTY", "");
    let mut cfg = AppConfig::default();
    cfg.search
        .providers
        .insert("firecrawl_developer".to_string(), true);
    cfg.search.api.insert(
        "firecrawl_developer".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: Some("EGGSEARCH_TEST_FIRECRAWL_EMPTY".to_string()),
            base_url: None,
        },
    );
    cfg.validate()
        .expect("empty optional key validates keyless");
    let key = eggsearch::core::config::optional_api_key("firecrawl_developer", &cfg.search.api);
    assert!(key.is_none());
    assert!(eggsearch::core::config::optional_api_key_misconfigured(
        "firecrawl_developer",
        &cfg.search.api
    ));
    assert!(cfg.provider_is_available("firecrawl_developer"));
    std::env::remove_var("EGGSEARCH_TEST_FIRECRAWL_EMPTY");
}

#[test]
fn capability_partitioning_skips_for_unsupported_roles() {
    let client = Arc::new(reqwest::Client::new());
    let engine = FirecrawlDeveloperEngine {
        client,
        api_key: None,
        base_url: None,
    };
    assert!(engine.supports_role(&EvidenceRole::OfficialDocumentation));
    assert!(engine.supports_role(&EvidenceRole::IssueOrIncidentDiscussion));
    assert!(engine.supports_role(&EvidenceRole::PullRequestOrDesignReview));
    assert!(!engine.supports_role(&EvidenceRole::PrimaryImplementation));
    assert!(!engine.supports_role(&EvidenceRole::ReleaseNoteOrChangelog));
    assert!(!engine.supports_role(&EvidenceRole::AuthoritativeSecurityAdvisory));
    assert!(!engine.supports_role(&EvidenceRole::UnknownOrWeakContext));
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn optional_key_adds_authorization_header() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/developer")
            .header("Authorization", "Bearer test-key-123");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"success": true, "results": []}"#);
    });
    let client = reqwest::Client::new();
    let mut req = engine_req("how do I configure retries", 5);
    req.excerpt_count = 2;
    let batch = eggsearch::meta::engines::firecrawl_developer::search(
        &client,
        Some("test-key-123"),
        Some(&server.url("/developer")),
        &req,
    )
    .await
    .expect("search succeeds");
    assert!(batch.results.is_empty());
    mock.assert();
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn keyless_request_omits_authorization_header() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let authed = server.mock(|when, then| {
        when.method(POST)
            .path("/developer")
            .header("Authorization", "Bearer anything");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"success": true, "results": []}"#);
    });
    let plain = server.mock(|when, then| {
        when.method(POST).path("/developer");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"success": true, "results": []}"#);
    });
    let client = reqwest::Client::new();
    let req = engine_req("keyless query", 5);
    eggsearch::meta::engines::firecrawl_developer::search(
        &client,
        None,
        Some(&server.url("/developer")),
        &req,
    )
    .await
    .expect("keyless succeeds");
    assert_eq!(authed.hits(), 0);
    assert!(plain.hits() >= 1);
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn owner_repo_scope_maps_to_repos_filter() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/developer")
            .body_contains("tokio-rs/axum");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"success": true, "results": []}"#);
    });
    let client = reqwest::Client::new();
    let mut req = engine_req("retry behavior", 5);
    req.repo_scope = RepoScope::new("tokio-rs", "axum");
    eggsearch::meta::engines::firecrawl_developer::search(
        &client,
        None,
        Some(&server.url("/developer")),
        &req,
    )
    .await
    .expect("scoped search succeeds");
    mock.assert();
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn issue_intent_restricts_types() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/developer")
            .body_contains("pull_request");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"success": true, "results": []}"#);
    });
    let client = reqwest::Client::new();
    let mut req = engine_req("panic in router", 5);
    req.intent = eggsearch::core::query::SearchIntent::Issues;
    eggsearch::meta::engines::firecrawl_developer::search(
        &client,
        None,
        Some(&server.url("/developer")),
        &req,
    )
    .await
    .expect("issues intent succeeds");
    mock.assert();
    let any_mock = server.mock(|when, then| {
        when.method(POST).path("/no-doc");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"success": true, "results": []}"#);
    });
    let doc_probe = server.mock(|when, then| {
        when.method(POST).path("/no-doc").body_contains("\"doc\"");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"success": true, "results": []}"#);
    });
    let mut req2 = engine_req("panic in router", 5);
    req2.intent = eggsearch::core::query::SearchIntent::Issues;
    eggsearch::meta::engines::firecrawl_developer::search(
        &client,
        None,
        Some(&server.url("/no-doc")),
        &req2,
    )
    .await
    .expect("second succeeds");
    assert!(any_mock.hits() >= 1);
    assert_eq!(
        doc_probe.hits(),
        0,
        "issue intent must not request doc type"
    );
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn result_parsing_handles_all_four_prefixes_and_title_fallback() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/developer");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                "success": true,
                "results": [
                    {"id": "issue:o/r#1", "type": "issue", "url": "https://github.com/o/r/issues/1", "title": "Issue one", "passages": [{"text": "issue passage"}]},
                    {"id": "pull_request:o/r#2", "type": "pull_request", "url": "https://github.com/o/r/pull/2", "title": "PR two", "passages": [{"text": "pr passage"}]},
                    {"id": "readme:o/r", "type": "readme", "url": "https://github.com/o/r", "title": "readme", "passages": [{"text": "readme passage"}]},
                    {"id": "doc:example", "type": "doc", "url": "https://example.com/docs/a", "passages": [{"text": "doc passage"}]}
                ]
            }"#,
            );
    });
    let client = reqwest::Client::new();
    let mut req = engine_req("test", 10);
    req.excerpt_count = 2;
    let batch = eggsearch::meta::engines::firecrawl_developer::search(
        &client,
        None,
        Some(&server.url("/developer")),
        &req,
    )
    .await
    .expect("parse succeeds");
    assert_eq!(batch.results.len(), 4);
    assert_eq!(batch.results[3].title, "https://example.com/docs/a");
    for r in &batch.results {
        assert!(!r.excerpts.is_empty());
        assert!(matches!(
            r.excerpts[0].provenance,
            eggsearch::core::source_card::ExcerptProvenance::ProviderPassage
        ));
    }
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn passages_respect_bounds() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/developer");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                "success": true,
                "results": [
                    {"id": "issue:o/r#1", "type": "issue", "url": "https://github.com/o/r/issues/1", "title": "T", "passages": [{"text": "a"}, {"text": "b"}, {"text": "c"}, {"text": "d"}, {"text": "e"}]}
                ]
            }"#,
            );
    });
    let client = reqwest::Client::new();
    let mut req = engine_req("test", 5);
    req.excerpt_count = 3;
    let batch = eggsearch::meta::engines::firecrawl_developer::search(
        &client,
        None,
        Some(&server.url("/developer")),
        &req,
    )
    .await
    .expect("ok");
    assert_eq!(batch.results.len(), 1);
    assert!(batch.results[0].excerpts.len() <= 3);
    assert!(batch.results[0]
        .excerpts
        .iter()
        .all(|e| e.text.chars().count() <= eggsearch::core::source_card::MAX_EXCERPT_CHARS));
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn unindexed_scope_preserved_distinctly_from_zero_matches() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/developer");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                "success": true,
                "results": [],
                "repos": [{"repo": "tokio-rs/axum", "indexed": false, "types": {"issue": false, "pullRequest": false, "readme": false}}]
            }"#,
            );
    });
    let client = Arc::new(reqwest::Client::new());
    let engine: Arc<dyn SearchEngine> = Arc::new(FirecrawlDeveloperEngine {
        client,
        api_key: None,
        base_url: Some(server.url("/developer")),
    });
    let mut req = engine_req("nothing matches", 5);
    req.repo_scope = RepoScope::new("tokio-rs", "axum");
    let batch = engine.search_batch(&req).await.expect("batch succeeds");
    assert!(batch.results.is_empty());
    assert!(batch.retrieval_metadata.has_unindexed());
    assert_eq!(
        batch.retrieval_metadata.unindexed_scopes(),
        vec!["tokio-rs/axum"]
    );
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn rate_limit_enters_normal_failure_path() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/developer");
        then.status(429).body("Too Many Requests");
    });
    let client = reqwest::Client::new();
    let req = engine_req("test", 5);
    let err = eggsearch::meta::engines::firecrawl_developer::search(
        &client,
        None,
        Some(&server.url("/developer")),
        &req,
    )
    .await
    .expect_err("429 fails");
    match err {
        eggsearch::meta::engines::error::EngineError::BadStatus { status, .. } => {
            assert_eq!(status, 429)
        }
        other => panic!("expected 429, got {other:?}"),
    }
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn oversized_body_rejected_by_shared_cap() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let big = "a".repeat(2 * 1024 * 1024 + 1024);
    let body = format!(
        "{{\"success\": true, \"results\": [{{\"id\": \"doc:x\", \"type\": \"doc\", \"url\": \"https://example.com/a\", \"title\": \"T\", \"passages\": [{{\"text\": \"{big}\"}}]}}]}}"
    );
    server.mock(|when, then| {
        when.method(POST).path("/developer");
        then.status(200)
            .header("content-type", "application/json")
            .body(body.clone());
    });
    let client = reqwest::Client::new();
    let req = engine_req("test", 5);
    let err = eggsearch::meta::engines::firecrawl_developer::search(
        &client,
        None,
        Some(&server.url("/developer")),
        &req,
    )
    .await
    .expect_err("oversized fails");
    match err {
        eggsearch::meta::engines::error::EngineError::ParseFailed { reason, .. } => {
            assert!(reason.contains("too large"))
        }
        other => panic!("expected ParseFailed, got {other:?}"),
    }
}

#[test]
fn api_key_never_rendered_in_debug_or_error() {
    let client = Arc::new(reqwest::Client::new());
    let engine = FirecrawlDeveloperEngine {
        client,
        api_key: Some("super-secret-key".to_string()),
        base_url: None,
    };
    let debug = format!("{}:{}", engine.name(), "firecrawl_developer");
    assert!(!debug.contains("super-secret-key"));
    let err = eggsearch::meta::engines::error::EngineError::BadStatus {
        engine: "firecrawl_developer",
        status: 401,
    };
    assert!(!err.to_string().contains("super-secret-key"));
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn repo_search_emits_unindexed_warning() {
    use eggsearch::core::repo_search::RepoSearchRequest;
    let firecrawl_mock = httpmock::MockServer::start();
    firecrawl_mock.mock(|when, then| {
        when.method(httpmock::Method::POST).path("/developer");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                "success": true,
                "results": [],
                "repos": [{"repo": "tokio-rs/axum", "indexed": false}]
            }"#,
            );
    });
    let client = Arc::new(reqwest::Client::new());
    let engine: Arc<dyn SearchEngine> = Arc::new(FirecrawlDeveloperEngine {
        client,
        api_key: None,
        base_url: Some(firecrawl_mock.url("/developer")),
    });
    let adapter = MetadataSearchAdapter::from_engines(vec![engine], Duration::from_secs(5));
    let req = RepoSearchRequest {
        query: "middleware".to_string(),
        owner: Some("tokio-rs".to_string()),
        repo: Some("axum".to_string()),
        ..Default::default()
    };
    let resp = adapter.repo_search(&req, 10, 50, None, None).await;
    let warnings: Vec<String> = resp.warnings.iter().map(|w| w.message.clone()).collect();
    assert!(
        warnings.iter().any(|m| m.contains("scope_unindexed")
            && m.contains("tokio-rs/axum")
            && m.contains("firecrawl_developer")),
        "expected unindexed warning, got {warnings:?}"
    );
}
