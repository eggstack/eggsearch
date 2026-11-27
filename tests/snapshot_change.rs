use std::sync::Arc;

use eggsearch::core::config::AppConfig;
use eggsearch::core::fetch::{FetchCachePolicy, FetchSnapshotRequest, SnapshotCompare};
use eggsearch::mcp::projection::ResponseDetail;
use eggsearch::mcp::state::ServerState;
use eggsearch::mcp::tools::{run_web_fetch, WebFetchArgs};
use httpmock::prelude::*;

async fn compare_with(
    state: Arc<ServerState>,
    url: String,
    detail: ResponseDetail,
) -> serde_json::Value {
    run_web_fetch(
        state,
        WebFetchArgs {
            url,
            max_chars: Some(12_000),
            cache_policy: Some(FetchCachePolicy::Bypass),
            snapshot: Some(FetchSnapshotRequest {
                compare: SnapshotCompare::Line,
                ..Default::default()
            }),
            response_detail: Some(detail),
            ..Default::default()
        },
    )
    .await
    .expect("fetch succeeds")
}

async fn compare(state: Arc<ServerState>, url: String) -> serde_json::Value {
    compare_with(state, url, ResponseDetail::Brief).await
}

fn snapshot_state(path: std::path::PathBuf, enabled: bool) -> Arc<ServerState> {
    let mut config = AppConfig::default();
    config.fetch.allow_localhost = true;
    config.fetch.allow_private_network = true;
    config.fetch.snapshots.enabled = enabled;
    config.fetch.snapshots.path = Some(path);
    Arc::new(ServerState::build(config).expect("state builds"))
}

fn private_directory(directory: &std::path::Path) -> std::path::PathBuf {
    let private = directory.join("private");
    std::fs::create_dir(&private).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    private
}

fn warning_text(response: &serde_json::Value) -> String {
    response["warnings"]
        .as_array()
        .map(|warnings| {
            warnings
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
                .join(" | ")
        })
        .unwrap_or_default()
}

fn count_snapshots(path: &std::path::Path) -> i64 {
    let connection = rusqlite::Connection::open(path).unwrap();
    connection
        .query_row("SELECT COUNT(*) FROM snapshots", [], |row| row.get(0))
        .unwrap()
}

fn html_mock_body(paragraph: &str) -> String {
    format!("<html><body><p>{paragraph}</p><p>雪 remains</p></body></html>")
}

#[tokio::test]
async fn compares_extracted_content_and_never_stores_html() {
    let server = MockServer::start();
    let mut original = server.mock(|when, then| {
        when.method(GET).path("/page");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body("<html><head><meta name=\"private\" content=\"RAW_HTML_SECRET\"></head><body><p>Alpha extracted text</p><p>snow remains</p></body></html>");
    });
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("private").join("snapshots.sqlite3");
    let state = snapshot_state(path.clone(), true);
    let url = server.url("/page");

    let first = compare(state.clone(), url.clone()).await;
    assert_eq!(first["change"]["status"], "new");
    let same = compare(state.clone(), url.clone()).await;
    assert_eq!(same["change"]["status"], "same");
    assert_eq!(same["change"]["hunks"], serde_json::json!([]));

    original.delete();
    server.mock(|when, then| {
        when.method(GET).path("/page");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body("<html><body><p>Beta extracted text</p><p>snow remains</p></body></html>");
    });
    let changed = compare(state, url).await;
    assert_eq!(changed["change"]["status"], "changed");
    assert_eq!(changed["change"]["hunks"].as_array().unwrap().len(), 1);

    let connection = rusqlite::Connection::open(path).unwrap();
    let stored: String = connection
        .query_row("SELECT text FROM snapshots", [], |row| row.get(0))
        .unwrap();
    assert!(stored.contains("Beta extracted text"));
    assert!(stored.contains("snow remains"));
    assert!(!stored.contains("<html>"));
    assert!(!stored.contains("RAW_HTML_SECRET"));
}

#[tokio::test]
async fn disabled_store_warns_without_opening_a_file() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/page");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body("<html><body><p>Content</p></body></html>");
    });
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("private").join("snapshots.sqlite3");
    let state = snapshot_state(path.clone(), false);

    let response = compare(state, server.url("/page")).await;
    assert!(response.get("change").is_none());
    assert!(warning_text(&response).contains("snapshot_store_disabled"));
    assert!(!path.exists());
}

#[tokio::test]
async fn removed_origin_reports_removed_and_retains_the_snapshot() {
    let server = MockServer::start();
    let mut present = server.mock(|when, then| {
        when.method(GET).path("/page");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(html_mock_body("Alpha"));
    });
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("private").join("snapshots.sqlite3");
    let state = snapshot_state(path.clone(), true);
    let url = server.url("/page");

    let first = compare(state.clone(), url.clone()).await;
    assert_eq!(first["change"]["status"], "new");
    assert_eq!(count_snapshots(&path), 1);

    present.delete();
    server.mock(|when, then| {
        when.method(GET).path("/page");
        then.status(404).body("gone");
    });
    let removed = compare(state, url).await;
    assert_eq!(removed["change"]["status"], "removed");
    assert_eq!(removed["change"]["hunks"], serde_json::json!([]));
    assert_eq!(removed["change"]["chars_added"], 0);
    assert_eq!(removed["change"]["chars_removed"], 0);
    assert_eq!(count_snapshots(&path), 1);
}

#[tokio::test]
async fn corrupt_store_degrades_without_failing_the_fetch() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/page");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(html_mock_body("Alpha"));
    });
    let directory = tempfile::tempdir().unwrap();
    let private = private_directory(directory.path());
    let path = private.join("snapshots.sqlite3");
    std::fs::write(&path, b"not sqlite").unwrap();
    let state = snapshot_state(path.clone(), true);

    let response = compare(state, server.url("/page")).await;
    assert!(response.get("change").is_none());
    assert!(warning_text(&response).contains("snapshot_store_corrupt"));
    assert!(response["text"]
        .as_str()
        .is_some_and(|text| text.contains("Alpha")));
    assert_eq!(std::fs::read(&path).unwrap(), b"not sqlite");
}

#[tokio::test]
async fn oversized_caller_key_is_rejected_before_any_fetch() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/page");
        then.status(200).body("never read");
    });
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("private").join("snapshots.sqlite3");
    let state = snapshot_state(path.clone(), true);

    let error = run_web_fetch(
        state,
        WebFetchArgs {
            url: server.url("/page"),
            snapshot: Some(FetchSnapshotRequest {
                compare: SnapshotCompare::Line,
                key: Some("k".repeat(129)),
                ..Default::default()
            }),
            ..Default::default()
        },
    )
    .await
    .expect_err("an oversized caller key must be rejected");
    assert!(error
        .error_payload()
        .to_string()
        .contains("invalid_snapshot_key"));
    mock.assert_calls(0);
    assert!(!path.exists());
}

#[tokio::test]
async fn default_compare_never_opens_the_store() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/page");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(html_mock_body("Alpha"));
    });
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("private").join("snapshots.sqlite3");
    let state = snapshot_state(path.clone(), true);

    let response = run_web_fetch(
        state,
        WebFetchArgs {
            url: server.url("/page"),
            snapshot: Some(FetchSnapshotRequest::default()),
            ..Default::default()
        },
    )
    .await
    .expect("fetch succeeds");
    assert!(response.get("change").is_none());
    assert!(!path.exists());
}
