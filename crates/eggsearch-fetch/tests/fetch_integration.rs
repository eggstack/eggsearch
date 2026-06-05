use eggsearch_fetch::{ArtifactStore, ExtractMode, FetchCache, FetchProvider, FetchRequest, ReqwestFetchProvider, RobotsCache};
use std::sync::Arc;
use tempfile::tempdir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn fetch_basic_html() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/page"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"<html><head><title>Mock Page</title></head>
               <body><h1>Hello</h1><p>World <b>bold</b>.</p>
               <script>evil()</script>
               <nav id="primary">x</nav>
               </body></html>"#,
        ))
        .mount(&server)
        .await;

    let art_dir = tempdir().unwrap();
    let artifacts = Arc::new(ArtifactStore::new(art_dir.path()).unwrap());
    let cache = Arc::new(FetchCache::default());
    let http = reqwest::Client::builder().build().unwrap();
    let robots = Arc::new(RobotsCache::new(http));
    // Pre-populate robots cache to allow everything for the mock host.
    // (Mock server is reachable at 127.0.0.1; we let it through because
    //  the mock doesn't return robots.txt.)
    let _ = robots; // silence unused

    let provider = ReqwestFetchProvider::new(artifacts, cache, robots).unwrap();
    let url = format!("{}/page", server.uri());
    let req = FetchRequest {
        url: url::Url::parse(&url).unwrap(),
        max_bytes: 1024 * 1024,
        timeout_ms: 4000,
        extract_mode: ExtractMode::Readability,
        respect_robots_txt: false,
    };
    let doc = provider.fetch(req).await.expect("fetch");
    assert_eq!(doc.title.as_deref(), Some("Mock Page"));
    assert!(doc.text.contains("Hello"));
    assert!(doc.text.contains("World"));
    assert!(!doc.text.contains("evil()"));
}

#[tokio::test]
async fn fetch_respects_size_limit() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/big"))
        .respond_with(ResponseTemplate::new(200).set_body_string("a".repeat(2048)))
        .mount(&server)
        .await;
    let art_dir = tempdir().unwrap();
    let artifacts = Arc::new(ArtifactStore::new(art_dir.path()).unwrap());
    let cache = Arc::new(FetchCache::default());
    let http = reqwest::Client::builder().build().unwrap();
    let robots = Arc::new(RobotsCache::new(http));
    let provider = ReqwestFetchProvider::new(artifacts, cache, robots).unwrap();
    let url = format!("{}/big", server.uri());
    let req = FetchRequest {
        url: url::Url::parse(&url).unwrap(),
        max_bytes: 100, // way too small
        timeout_ms: 4000,
        extract_mode: ExtractMode::Raw,
        respect_robots_txt: false,
    };
    let res = provider.fetch(req).await;
    assert!(res.is_err(), "expected too-large error, got {res:?}");
}

#[tokio::test]
async fn fetch_bad_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/missing"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let art_dir = tempdir().unwrap();
    let artifacts = Arc::new(ArtifactStore::new(art_dir.path()).unwrap());
    let cache = Arc::new(FetchCache::default());
    let http = reqwest::Client::builder().build().unwrap();
    let robots = Arc::new(RobotsCache::new(http));
    let provider = ReqwestFetchProvider::new(artifacts, cache, robots).unwrap();
    let url = format!("{}/missing", server.uri());
    let req = FetchRequest {
        url: url::Url::parse(&url).unwrap(),
        max_bytes: 1024,
        timeout_ms: 4000,
        extract_mode: ExtractMode::Raw,
        respect_robots_txt: false,
    };
    let res = provider.fetch(req).await;
    assert!(matches!(res, Err(eggsearch_fetch::FetchError::BadStatus(404))));
}
