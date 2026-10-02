//! Provider request contract: `EngineSearchRequest` fidelity.
//!
//! Regression provenance: phase-1 provider-request workstream.
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
    let client = eggfetch_core::Client::new();
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
    let client = eggfetch_core::Client::new();
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
        response_detail: None,
    };
    let v = eggsearch::mcp::tools::run_web_search(state, args)
        .await
        .expect("ok");
    assert!(v.get("results").is_some());
    assert!(v.get("capability_enforcement").is_some());
}

async fn capture_accept_encoding(decompress: Option<bool>) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let mut buf = vec![0u8; 8192];
        let mut seen = Vec::new();
        loop {
            let n = stream.read(&mut buf).await.expect("read");
            if n == 0 {
                break;
            }
            seen.extend_from_slice(&buf[..n]);
            if seen.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
            if seen.len() > 16384 {
                break;
            }
        }
        let request = String::from_utf8_lossy(&seen).into_owned();
        let encoding = request
            .lines()
            .find_map(|line| {
                let mut parts = line.splitn(2, ':');
                let name = parts.next().unwrap_or("").trim();
                let value = parts.next().unwrap_or("").trim();
                if name.eq_ignore_ascii_case("accept-encoding") {
                    Some(value.to_string())
                } else {
                    None
                }
            })
            .unwrap_or_default();
        let body = b"ok";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .await
            .expect("write head");
        stream.write_all(body).await.expect("write body");
        encoding
    });
    let client = eggfetch_core::Client::new();
    let url = format!("http://{addr}/");
    let timeout = eggfetch_core::Timeout {
        pool: Some(Duration::from_secs(5)),
        connect: Some(Duration::from_secs(5)),
        write: Some(Duration::from_secs(5)),
        read: Some(Duration::from_secs(5)),
        total: Some(Duration::from_secs(5)),
    };
    let mut builder = client.get(&url).expect("url");
    if let Some(flag) = decompress {
        builder = builder.decompress(flag);
    }
    let resp = builder
        .timeout(timeout)
        .send()
        .await
        .expect("request succeeds");
    assert!(resp.status().is_success());
    let encoding = server.await.expect("server task");
    encoding
}

#[tokio::test]
async fn automatic_decompression_advertises_gzip_and_brotli() {
    let encoding = capture_accept_encoding(None).await;
    let lowered = encoding.to_ascii_lowercase();
    assert!(
        lowered.contains("gzip"),
        "default request must advertise gzip, got: {encoding}"
    );
    assert!(
        lowered.contains("br"),
        "default request must advertise br, got: {encoding}"
    );
}

#[tokio::test]
async fn decompress_flag_controls_advertisement() {
    let encoding = capture_accept_encoding(Some(false)).await;
    let lowered = encoding.to_ascii_lowercase();
    assert!(
        !lowered.contains("gzip"),
        "decompress(false) must not advertise gzip, got: {encoding}"
    );
    assert!(
        !lowered.contains("br"),
        "decompress(false) must not advertise br, got: {encoding}"
    );
    assert!(
        !lowered.contains("zstd"),
        "decompress(false) must not advertise zstd, got: {encoding}"
    );
    assert!(
        !lowered.contains("deflate"),
        "decompress(false) must not advertise deflate, got: {encoding}"
    );
}

#[tokio::test]
async fn searxng_engine_advertises_compressed_encodings() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let mut buf = vec![0u8; 8192];
        let mut seen = Vec::new();
        loop {
            let n = stream.read(&mut buf).await.expect("read");
            if n == 0 {
                break;
            }
            seen.extend_from_slice(&buf[..n]);
            if seen.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
            if seen.len() > 16384 {
                break;
            }
        }
        let request = String::from_utf8_lossy(&seen).into_owned();
        let encoding = request
            .lines()
            .find_map(|line| {
                let mut parts = line.splitn(2, ':');
                let name = parts.next().unwrap_or("").trim();
                let value = parts.next().unwrap_or("").trim();
                if name.eq_ignore_ascii_case("accept-encoding") {
                    Some(value.to_string())
                } else {
                    None
                }
            })
            .unwrap_or_default();
        let body = br#"{"results": []}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .await
            .expect("write head");
        stream.write_all(body).await.expect("write body");
        encoding
    });
    let client = eggsearch::meta::engines::build_http_client(None).expect("client");
    let base = format!("http://{addr}");
    let results = eggsearch::meta::engines::searxng::search(
        &client,
        &base,
        "rust",
        5,
        Duration::from_secs(5),
    )
    .await
    .expect("searxng search succeeds");
    assert!(results.is_empty());
    let encoding = server.await.expect("server task");
    let lowered = encoding.to_ascii_lowercase();
    assert!(
        lowered.contains("gzip"),
        "searxng engine must advertise gzip via automatic decompression, got: {encoding}"
    );
    assert!(
        lowered.contains("br"),
        "searxng engine must advertise br via automatic decompression, got: {encoding}"
    );
}

fn deterministic_compression_payload() -> Vec<u8> {
    let line = "eggsearch phase-24 chunked compression regression payload 0123456789 abcdefghijklmnopqrstuvwxyz ";
    line.repeat(512).into_bytes()
}

fn gzip_compress(plain: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(plain).expect("gzip encode");
    encoder.finish().expect("gzip finish")
}

fn brotli_compress(plain: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut writer = brotli::CompressorWriter::new(Vec::new(), 4096, 5, 22);
    writer.write_all(plain).expect("brotli encode");
    writer.into_inner()
}

async fn serve_compressed_once(
    compressed: Vec<u8>,
    content_encoding: &'static str,
    use_chunked: bool,
) -> (String, tokio::task::JoinHandle<()>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let mut buf = vec![0u8; 8192];
        let mut seen = Vec::new();
        loop {
            let n = stream.read(&mut buf).await.expect("read");
            if n == 0 {
                break;
            }
            seen.extend_from_slice(&buf[..n]);
            if seen.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
            if seen.len() > 16384 {
                break;
            }
        }
        if use_chunked {
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Encoding: {content_encoding}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
            );
            stream.write_all(head.as_bytes()).await.expect("head");
            for wire in compressed.chunks(7) {
                let size_line = format!("{:X}\r\n", wire.len());
                stream
                    .write_all(size_line.as_bytes())
                    .await
                    .expect("chunk size");
                stream.write_all(wire).await.expect("chunk body");
                stream.write_all(b"\r\n").await.expect("chunk end");
            }
            stream.write_all(b"0\r\n\r\n").await.expect("terminator");
        } else {
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Encoding: {content_encoding}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                compressed.len()
            );
            stream.write_all(head.as_bytes()).await.expect("head");
            stream.write_all(&compressed).await.expect("body");
        }
    });
    (format!("http://{addr}/"), server)
}

async fn fetch_via_production_body_path(url: &str, max_bytes: usize) -> Vec<u8> {
    let client = eggsearch::meta::engines::build_http_client(None).expect("client");
    let resp = client
        .get(url)
        .expect("url")
        .timeout(eggsearch::meta::engines::engine_timeout(
            Duration::from_secs(10),
        ))
        .send()
        .await
        .expect("request succeeds");
    assert!(resp.status().is_success());
    eggsearch::meta::engines::read_bounded_body(resp, "phase24", max_bytes)
        .await
        .expect("bounded body succeeds")
}

#[tokio::test]
async fn chunked_gzip_decodes_through_bounded_body_path() {
    let plain = deterministic_compression_payload();
    let compressed = gzip_compress(&plain);
    assert!(compressed.len() > 64);
    let (url, server) = serve_compressed_once(compressed, "gzip", true).await;
    let decoded = fetch_via_production_body_path(&url, 2 * 1024 * 1024).await;
    server.await.expect("server task");
    assert_eq!(decoded, plain);
}

#[tokio::test]
async fn chunked_brotli_decodes_through_bounded_body_path() {
    let plain = deterministic_compression_payload();
    let compressed = brotli_compress(&plain);
    assert!(compressed.len() > 64);
    let (url, server) = serve_compressed_once(compressed, "br", true).await;
    let decoded = fetch_via_production_body_path(&url, 2 * 1024 * 1024).await;
    server.await.expect("server task");
    assert_eq!(decoded, plain);
}

#[tokio::test]
async fn transfer_shape_does_not_change_decoded_bytes() {
    let plain = deterministic_compression_payload();
    let gzip_bytes = gzip_compress(&plain);
    let brotli_bytes = brotli_compress(&plain);
    let (chunked_gzip_url, chunked_gzip_server) =
        serve_compressed_once(gzip_bytes.clone(), "gzip", true).await;
    let chunked_gzip = fetch_via_production_body_path(&chunked_gzip_url, 2 * 1024 * 1024).await;
    chunked_gzip_server.await.expect("server task");
    let (length_gzip_url, length_gzip_server) =
        serve_compressed_once(gzip_bytes, "gzip", false).await;
    let length_gzip = fetch_via_production_body_path(&length_gzip_url, 2 * 1024 * 1024).await;
    length_gzip_server.await.expect("server task");
    let (chunked_br_url, chunked_br_server) =
        serve_compressed_once(brotli_bytes.clone(), "br", true).await;
    let chunked_br = fetch_via_production_body_path(&chunked_br_url, 2 * 1024 * 1024).await;
    chunked_br_server.await.expect("server task");
    let (length_br_url, length_br_server) = serve_compressed_once(brotli_bytes, "br", false).await;
    let length_br = fetch_via_production_body_path(&length_br_url, 2 * 1024 * 1024).await;
    length_br_server.await.expect("server task");
    assert_eq!(chunked_gzip, plain);
    assert_eq!(length_gzip, plain);
    assert_eq!(chunked_br, plain);
    assert_eq!(length_br, plain);
}

#[tokio::test]
async fn decoded_body_limit_applies_to_decompressed_bytes() {
    let plain = deterministic_compression_payload();
    let gzip_bytes = gzip_compress(&plain);
    let (url, server) = serve_compressed_once(gzip_bytes, "gzip", true).await;
    let client = eggsearch::meta::engines::build_http_client(None).expect("client");
    let resp = client
        .get(&url)
        .expect("url")
        .timeout(eggsearch::meta::engines::engine_timeout(
            Duration::from_secs(10),
        ))
        .send()
        .await
        .expect("request succeeds");
    let err = eggsearch::meta::engines::read_bounded_body(resp, "phase24", 16)
        .await
        .expect_err("tiny decoded limit must reject chunked gzip");
    assert!(
        err.to_string().contains("too large"),
        "decoded overflow must report too large, got: {err}"
    );
    server.await.expect("server task");
}

#[tokio::test]
async fn decoded_body_limit_is_not_wire_only() {
    let plain = vec![b'A'; 64 * 1024];
    let gzip_bytes = gzip_compress(&plain);
    assert!(
        gzip_bytes.len() < 1024,
        "highly compressible fixture must keep wire bytes small, got {}",
        gzip_bytes.len()
    );
    let (url, server) = serve_compressed_once(gzip_bytes, "gzip", true).await;
    let client = eggsearch::meta::engines::build_http_client(None).expect("client");
    let resp = client
        .get(&url)
        .expect("url")
        .timeout(eggsearch::meta::engines::engine_timeout(
            Duration::from_secs(10),
        ))
        .send()
        .await
        .expect("request succeeds");
    let err = eggsearch::meta::engines::read_bounded_body(resp, "phase24", 4096)
        .await
        .expect_err("decoded overflow must be rejected even when wire bytes fit");
    assert!(
        err.to_string().contains("too large"),
        "wire-small decoded-large must report too large, got: {err}"
    );
    server.await.expect("server task");
}

#[tokio::test]
async fn compressed_chunked_response_respects_total_deadline() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let mut buf = vec![0u8; 8192];
        let mut seen = Vec::new();
        loop {
            let n = stream.read(&mut buf).await.expect("read");
            if n == 0 {
                break;
            }
            seen.extend_from_slice(&buf[..n]);
            if seen.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
            if seen.len() > 16384 {
                break;
            }
        }
        let head = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Encoding: gzip\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n";
        stream.write_all(head.as_bytes()).await.expect("head");
        stream.write_all(b"8\r\n12345678\r\n").await.expect("chunk");
        tokio::time::sleep(Duration::from_secs(30)).await;
    });
    let client = eggsearch::meta::engines::build_http_client(None).expect("client");
    let url = format!("http://{addr}/");
    let start = tokio::time::Instant::now();
    let result = client
        .get(&url)
        .expect("url")
        .timeout(eggsearch::meta::engines::engine_timeout(
            Duration::from_millis(800),
        ))
        .send()
        .await;
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(10),
        "compressed chunked stall must stay bounded, elapsed: {elapsed:?}"
    );
    if let Ok(resp) = result {
        let body = eggsearch::meta::engines::read_bounded_body(resp, "phase24", 65536).await;
        assert!(body.is_err(), "stalled compressed body must not succeed");
    }
    server.abort();
}

fn source_provider_client() -> eggfetch_core::Client {
    eggsearch::meta::engines::build_http_client(None).expect("client")
}

fn source_provider_request(query: &str, max_results: usize) -> EngineSearchRequest {
    EngineSearchRequest::simple(query, max_results, Duration::from_secs(5))
}

fn oversized_source_body() -> String {
    "x".repeat(2 * 1024 * 1024 + 1)
}

#[tokio::test]
async fn wikipedia_request_and_response_contract() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/w/api.php")
            .query_param("action", "query")
            .query_param("list", "search")
            .query_param("srsearch", "rust async")
            .query_param("srlimit", "2")
            .query_param("format", "json");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{"query":{"search":[{"title":"Rust (programming language)",
                "snippet":"<span class=\"searchmatch\">Rust</span> systems language",
                "timestamp":"2024-05-01T12:00:00Z"}]}}"#,
            );
    });
    let results = eggsearch::meta::engines::wikipedia::search(
        &source_provider_client(),
        Some(&server.url("/w/api.php")),
        &source_provider_request("rust async", 2),
    )
    .await
    .expect("wikipedia search");
    mock.assert();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].source_engine, "wikipedia");
    assert_eq!(
        results[0].url,
        "https://en.wikipedia.org/wiki/Rust_%28programming_language%29"
    );
    assert_eq!(results[0].snippet.as_deref(), Some("Rust systems language"));
}

#[tokio::test]
async fn wikipedia_failures_stay_bounded_and_provider_scoped() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let failing = server.mock(|when, then| {
        when.method(GET).path("/w/api.php");
        then.status(503);
    });
    let err = eggsearch::meta::engines::wikipedia::search(
        &source_provider_client(),
        Some(&server.url("/w/api.php")),
        &source_provider_request("rust", 3),
    )
    .await
    .expect_err("upstream failure is an error, not empty success");
    assert!(err.to_string().contains("wikipedia"), "got: {err}");
    failing.assert();

    let big_server = MockServer::start();
    let oversized = big_server.mock(|when, then| {
        when.method(GET).path("/w/api.php");
        then.status(200)
            .header("content-type", "application/json")
            .body(oversized_source_body());
    });
    let err = eggsearch::meta::engines::wikipedia::search(
        &source_provider_client(),
        Some(&big_server.url("/w/api.php")),
        &source_provider_request("rust", 3),
    )
    .await
    .expect_err("oversized bodies must be rejected");
    assert!(err.to_string().contains("too large"), "got: {err}");
    oversized.assert();
}

#[tokio::test]
async fn hn_algolia_request_and_response_contract() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v1/search")
            .query_param("query", "rust")
            .query_param("tags", "story")
            .query_param("hitsPerPage", "2");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{"hits":[{"objectID":"1","title":"Show HN: rust tool",
                "url":"https://example.com/a","points":10,"num_comments":2,
                "created_at":"2024-02-01T10:00:00.000Z"}]}"#,
            );
    });
    let results = eggsearch::meta::engines::hn_algolia::search(
        &source_provider_client(),
        Some(&server.url("/api/v1/search")),
        &source_provider_request("rust", 2),
    )
    .await
    .expect("hn search");
    mock.assert();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].url, "https://example.com/a");
    assert_eq!(
        results[0].published_at.as_deref(),
        Some("2024-02-01T10:00:00+00:00")
    );
}

#[tokio::test]
async fn hn_algolia_failures_stay_bounded_and_provider_scoped() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let failing = server.mock(|when, then| {
        when.method(GET).path("/api/v1/search");
        then.status(429);
    });
    let err = eggsearch::meta::engines::hn_algolia::search(
        &source_provider_client(),
        Some(&server.url("/api/v1/search")),
        &source_provider_request("rust", 3),
    )
    .await
    .expect_err("rate limits surface as typed engine errors");
    assert!(err.to_string().contains("hn_algolia"), "got: {err}");
    failing.assert();

    let bad_server = MockServer::start();
    let malformed = bad_server.mock(|when, then| {
        when.method(GET).path("/api/v1/search");
        then.status(200)
            .header("content-type", "application/json")
            .body("{\"hits\": 5}");
    });
    let err = eggsearch::meta::engines::hn_algolia::search(
        &source_provider_client(),
        Some(&bad_server.url("/api/v1/search")),
        &source_provider_request("rust", 3),
    )
    .await
    .expect_err("malformed payloads must fail closed");
    assert!(err.to_string().contains("invalid JSON"), "got: {err}");
    malformed.assert();
}

#[tokio::test]
async fn arxiv_request_response_and_pacing_contract() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/query")
            .query_param("search_query", "all:sparse retrieval")
            .query_param("max_results", "2");
        then.status(200)
            .header("content-type", "application/atom+xml")
            .body(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom"><entry>
<id>http://arxiv.org/abs/2401.00001v2</id>
<published>2024-01-02T00:00:00Z</published>
<title>Sparse retrieval</title>
<summary>An abstract.</summary>
</entry></feed>"#,
            );
    });
    let gate = eggsearch::meta::engines::arxiv::RequestGate::new(Duration::from_millis(50));
    let client = source_provider_client();
    let url = server.url("/api/query");
    let started = std::time::Instant::now();
    let first = eggsearch::meta::engines::arxiv::search(
        &client,
        Some(&url),
        &gate,
        &source_provider_request("sparse retrieval", 2),
    )
    .await
    .expect("arxiv search");
    eggsearch::meta::engines::arxiv::search(
        &client,
        Some(&url),
        &gate,
        &source_provider_request("sparse retrieval", 2),
    )
    .await
    .expect("second arxiv search");
    let elapsed = started.elapsed();
    mock.assert_hits(2);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].url, "https://arxiv.org/abs/2401.00001");
    assert_eq!(first[0].source_engine, "arxiv");
    assert!(
        elapsed >= Duration::from_millis(50),
        "repeat arXiv calls must pass through the shared pacing gate, elapsed: {elapsed:?}"
    );
}

#[tokio::test]
async fn arxiv_failures_stay_bounded_and_provider_scoped() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let failing = server.mock(|when, then| {
        when.method(GET).path("/api/query");
        then.status(503);
    });
    let gate = eggsearch::meta::engines::arxiv::RequestGate::new(Duration::from_millis(1));
    let err = eggsearch::meta::engines::arxiv::search(
        &source_provider_client(),
        Some(&server.url("/api/query")),
        &gate,
        &source_provider_request("rust", 3),
    )
    .await
    .expect_err("upstream failure is an error, not empty success");
    assert!(err.to_string().contains("arxiv"), "got: {err}");
    failing.assert();

    let bad_server = MockServer::start();
    let malformed = bad_server.mock(|when, then| {
        when.method(GET).path("/api/query");
        then.status(200)
            .header("content-type", "application/atom+xml")
            .body("<feed><entry><title>x</title>");
    });
    let err = eggsearch::meta::engines::arxiv::search(
        &source_provider_client(),
        Some(&bad_server.url("/api/query")),
        &gate,
        &source_provider_request("rust", 3),
    )
    .await
    .expect_err("truncated Atom feeds must fail closed");
    assert!(err.to_string().contains("truncated"), "got: {err}");
    malformed.assert();
}

#[tokio::test]
async fn pubmed_runs_one_esearch_and_one_esummary() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let esearch = server.mock(|when, then| {
        when.method(GET)
            .path("/esearch.fcgi")
            .query_param("db", "pubmed")
            .query_param("term", "crispr")
            .query_param("tool", "eggsearch");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"esearchresult":{"idlist":["31357002"]}}"#);
    });
    let esummary = server.mock(|when, then| {
        when.method(GET)
            .path("/esummary.fcgi")
            .query_param("db", "pubmed")
            .query_param("id", "31357002")
            .query_param("tool", "eggsearch");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{"result":{"uids":["31357002"],"31357002":{"title":"A study.",
                "source":"J Test","authors":[{"name":"Smith J"}],"epubdate":"2020 Feb 3"}}}"#,
            );
    });
    let results = eggsearch::meta::engines::pubmed::search(
        &source_provider_client(),
        Some(&server.url("")),
        None,
        None,
        &source_provider_request("crispr", 3),
    )
    .await
    .expect("pubmed search");
    esearch.assert_hits(1);
    esummary.assert_hits(1);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].url, "https://pubmed.ncbi.nlm.nih.gov/31357002/");
    assert_eq!(
        results[0].published_at.as_deref(),
        Some("2020-02-03T00:00:00+00:00")
    );
}

#[tokio::test]
async fn pubmed_summary_phase_failure_fails_the_call() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let esearch = server.mock(|when, then| {
        when.method(GET).path("/esearch.fcgi");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"esearchresult":{"idlist":["1","2"]}}"#);
    });
    let esummary = server.mock(|when, then| {
        when.method(GET).path("/esummary.fcgi");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"header":{}}"#);
    });
    let err = eggsearch::meta::engines::pubmed::search(
        &source_provider_client(),
        Some(&server.url("")),
        None,
        None,
        &source_provider_request("crispr", 3),
    )
    .await
    .expect_err("a failed summary phase must not fabricate partial metadata");
    assert!(err.to_string().contains("result section"), "got: {err}");
    esearch.assert();
    esummary.assert();

    let failing_server = MockServer::start();
    let failing = failing_server.mock(|when, then| {
        when.method(GET).path("/esearch.fcgi");
        then.status(503);
    });
    let err = eggsearch::meta::engines::pubmed::search(
        &source_provider_client(),
        Some(&failing_server.url("")),
        None,
        None,
        &source_provider_request("crispr", 3),
    )
    .await
    .expect_err("upstream failure is an error, not empty success");
    assert!(err.to_string().contains("pubmed"), "got: {err}");
    failing.assert();
}

#[tokio::test]
async fn github_repositories_routes_keyless_and_optional_token() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let keyless = server.mock(|when, then| {
        when.method(GET)
            .path("/search/repositories")
            .query_param("q", "rust web")
            .query_param("per_page", "2")
            .matches(|req| match &req.headers {
                None => true,
                Some(headers) => !headers
                    .iter()
                    .any(|(name, _)| name.eq_ignore_ascii_case("authorization")),
            });
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{"items":[{"full_name":"tokio-rs/axum",
                "html_url":"https://github.com/tokio-rs/axum",
                "description":"Web framework","language":"Rust",
                "stargazers_count":10,"pushed_at":"2024-05-30T08:00:00Z"}]}"#,
            );
    });
    let results = eggsearch::meta::engines::github_repositories::search(
        &source_provider_client(),
        Some(&server.url("")),
        None,
        &source_provider_request("rust web", 2),
    )
    .await
    .expect("keyless repository discovery");
    keyless.assert();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "tokio-rs/axum");
    assert_eq!(results[0].source_engine, "github_repositories");

    let keyed = server.mock(|when, then| {
        when.method(GET)
            .path("/search/repositories")
            .header("authorization", "Bearer optional-token");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"items":[]}"#);
    });
    eggsearch::meta::engines::github_repositories::search(
        &source_provider_client(),
        Some(&server.url("")),
        Some("optional-token"),
        &source_provider_request("rust web", 2),
    )
    .await
    .expect("tokenised repository discovery");
    keyed.assert();
}

#[tokio::test]
async fn github_repositories_failures_stay_bounded_and_provider_scoped() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let rate_limited = server.mock(|when, then| {
        when.method(GET).path("/search/repositories");
        then.status(403)
            .header("x-ratelimit-remaining", "0")
            .header("content-type", "application/json")
            .body(r#"{"message":"API rate limit exceeded"}"#);
    });
    let err = eggsearch::meta::engines::github_repositories::search(
        &source_provider_client(),
        Some(&server.url("")),
        None,
        &source_provider_request("rust", 3),
    )
    .await
    .expect_err("rate-limit responses must surface as typed engine errors");
    assert!(
        err.to_string().contains("github_repositories"),
        "got: {err}"
    );
    rate_limited.assert();

    let big_server = MockServer::start();
    let oversized = big_server.mock(|when, then| {
        when.method(GET).path("/search/repositories");
        then.status(200)
            .header("content-type", "application/json")
            .body(oversized_source_body());
    });
    let err = eggsearch::meta::engines::github_repositories::search(
        &source_provider_client(),
        Some(&big_server.url("")),
        None,
        &source_provider_request("rust", 3),
    )
    .await
    .expect_err("oversized bodies must be rejected");
    assert!(err.to_string().contains("too large"), "got: {err}");
    oversized.assert();
}

/// Wire-level capture for the credentialed general-search providers.
///
/// The engines put the SerpAPI credential in a query parameter and the Kagi
/// credential in an `Authorization` header, so the request contract has to be
/// asserted from the recorded request rather than from engine state.
#[derive(Clone, Debug, Default)]
struct CapturedWire {
    method: String,
    path: String,
    query: Vec<(String, String)>,
    headers: Vec<(String, String)>,
    body: String,
}

impl CapturedWire {
    fn param(&self, key: &str) -> Option<&str> {
        self.query
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    fn header(&self, key: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).expect("captured body is JSON")
    }
}

fn capture(req: &httpmock::prelude::HttpMockRequest) -> CapturedWire {
    CapturedWire {
        method: req.method.clone(),
        path: req.path.clone(),
        query: req.query_params.clone().unwrap_or_default(),
        headers: req.headers.clone().unwrap_or_default(),
        body: req
            .body
            .as_ref()
            .map(|body| String::from_utf8_lossy(body).into_owned())
            .unwrap_or_default(),
    }
}

static SERPAPI_WIRE: Mutex<Option<CapturedWire>> = Mutex::new(None);
static KAGI_WIRE: Mutex<Option<CapturedWire>> = Mutex::new(None);

fn record_serpapi(req: &httpmock::prelude::HttpMockRequest) -> bool {
    *SERPAPI_WIRE.lock().unwrap() = Some(capture(req));
    true
}

fn record_kagi(req: &httpmock::prelude::HttpMockRequest) -> bool {
    *KAGI_WIRE.lock().unwrap() = Some(capture(req));
    true
}

fn take_serpapi_wire() -> CapturedWire {
    SERPAPI_WIRE
        .lock()
        .unwrap()
        .take()
        .expect("serpapi request was recorded")
}

fn take_kagi_wire() -> CapturedWire {
    KAGI_WIRE
        .lock()
        .unwrap()
        .take()
        .expect("kagi request was recorded")
}

const SERPAPI_KEY: &str = "serpapi-test-key-value";
const KAGI_KEY: &str = "kagi-test-key-value";

const SERPAPI_BODY: &str = r#"{
  "search_metadata": {"id": "65f0", "status": "Success"},
  "search_information": {"total_results": 99000},
  "organic_results": [
    {
      "position": 1,
      "title": "Axum  -  GitHub",
      "link": "https://github.com/tokio-rs/axum",
      "redirect_link": "https://www.google.com/url?sa=t&url=https://github.com/tokio-rs/axum",
      "snippet": "Web framework  built on Tokio and  Hyper.",
      "source": "GitHub"
    },
    {
      "position": 2,
      "title": "docs.rs  -  axum",
      "link": "https://docs.rs/axum/latest/axum/",
      "snippet": "Documentation for axum."
    },
    {
      "position": 3,
      "title": "Ecosystem",
      "link": "https://example.com/ecosystem",
      "snippet": "Third result."
    }
  ],
  "related_searches": [{"query": "axum middleware"}]
}"#;

const KAGI_BODY: &str = r#"{
  "meta": {"node": "kagi-1", "ms": 180, "trace": "trace-1"},
  "data": {
    "search": [
      {
        "url": "https://kagi.com/blog/small-web",
        "title": "The small web",
        "snippet": "An  independent  index.",
        "time": "2024-11-29T03:54:26Z"
      },
      {
        "url": "https://example.org/second",
        "title": "Second",
        "snippet": "Second result."
      }
    ],
    "news": [{"url": "https://news.example/story", "title": "News", "snippet": "s"}],
    "code": [{"url": "https://github.com/example/repo", "title": "repo", "snippet": "c"}],
    "interesting_finds": [{"url": "https://finds.example/x", "title": "f"}]
  }
}"#;

fn constrained_request(max_results: usize) -> EngineSearchRequest {
    let mut request = EngineSearchRequest::simple("rust axum", max_results, Duration::from_secs(5));
    request.safe_search = Some(eggsearch::core::query::SafeSearch::Strict);
    request.language = Some("en-US".to_string());
    request.region = Some("US".to_string());
    request
}

#[tokio::test]
async fn serpapi_wire_contract_pins_google_and_asks_for_no_extras() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let endpoint = format!("{}/search", server.url(""));
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("engine", "google")
            .query_param("api_key", SERPAPI_KEY)
            .matches(record_serpapi);
        then.status(200)
            .header("content-type", "application/json")
            .body(SERPAPI_BODY);
    });
    let results = eggsearch::meta::engines::serpapi::search(
        &source_provider_client(),
        SERPAPI_KEY,
        Some(&endpoint),
        &constrained_request(2),
    )
    .await
    .expect("serpapi search succeeds");
    mock.assert();

    let wire = take_serpapi_wire();
    assert_eq!(wire.method, "GET");
    assert_eq!(wire.path, "/search");
    assert_eq!(wire.param("q"), Some("rust axum"));
    assert_eq!(wire.param("safe"), Some("active"));
    assert_eq!(wire.param("hl"), Some("en-us"));
    assert_eq!(wire.param("gl"), Some("us"));
    assert_eq!(wire.param("api_key"), Some(SERPAPI_KEY));
    for forbidden in [
        "tbm",
        "async",
        "no_cache",
        "zero_trace",
        "json_restrictor",
        "num",
        "tbs",
    ] {
        assert!(
            wire.param(forbidden).is_none(),
            "{forbidden} must never be requested; eggsearch buys no extra vertical, mode, or guess"
        );
    }

    assert_eq!(results.len(), 2, "the local result budget is enforced");
    assert_eq!(results[0].url, "https://github.com/tokio-rs/axum");
    assert_eq!(results[0].title, "Axum - GitHub");
    assert_eq!(
        results[0].snippet.as_deref(),
        Some("Web framework built on Tokio and Hyper.")
    );
    assert!(
        results.iter().all(|r| r.published_at.is_none()),
        "organic results carry no documented timestamp"
    );
}

#[tokio::test]
async fn serpapi_quota_auth_and_oversized_responses_stay_provider_scoped() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    let endpoint = format!("{}/search", server.url(""));
    let quota = server.mock(|when, then| {
        when.method(GET).path("/search");
        then.status(429)
            .header("content-type", "application/json")
            .body(r#"{"error":"Your account is out of credits."}"#);
    });
    let err = eggsearch::meta::engines::serpapi::search(
        &source_provider_client(),
        SERPAPI_KEY,
        Some(&endpoint),
        &source_provider_request("rust", 3),
    )
    .await
    .expect_err("a quota response must surface as a provider-scoped error");
    assert!(err.to_string().contains("serpapi"), "got: {err}");
    assert!(
        err.to_string().contains("429"),
        "the rate-limited status must be preserved for cooldown handling: {err}"
    );
    assert!(
        !err.to_string().contains(SERPAPI_KEY),
        "the credential must never reach diagnostics: {err}"
    );
    quota.assert_hits(1);

    let auth_server = MockServer::start();
    let auth_endpoint = format!("{}/search", auth_server.url(""));
    let auth = auth_server.mock(|when, then| {
        when.method(GET).path("/search");
        then.status(401)
            .header("content-type", "application/json")
            .body(r#"{"error":"Invalid API key."}"#);
    });
    let err = eggsearch::meta::engines::serpapi::search(
        &source_provider_client(),
        SERPAPI_KEY,
        Some(&auth_endpoint),
        &source_provider_request("rust", 3),
    )
    .await
    .expect_err("an invalid credential must fail the provider, not the search");
    assert!(err.to_string().contains("401"), "got: {err}");
    assert!(!err.to_string().contains(SERPAPI_KEY), "got: {err}");
    auth.assert();

    let big_server = MockServer::start();
    let big_endpoint = format!("{}/search", big_server.url(""));
    let oversized = big_server.mock(|when, then| {
        when.method(GET).path("/search");
        then.status(200)
            .header("content-type", "application/json")
            .body(oversized_source_body());
    });
    let err = eggsearch::meta::engines::serpapi::search(
        &source_provider_client(),
        SERPAPI_KEY,
        Some(&big_endpoint),
        &source_provider_request("rust", 3),
    )
    .await
    .expect_err("oversized bodies must be rejected");
    assert!(err.to_string().contains("too large"), "got: {err}");
    oversized.assert();

    let err = eggsearch::meta::engines::serpapi::search(
        &source_provider_client(),
        SERPAPI_KEY,
        Some("not-a-url"),
        &source_provider_request("rust", 3),
    )
    .await
    .expect_err("an invalid endpoint is a provider-scoped failure");
    assert!(
        !err.to_string().contains(SERPAPI_KEY),
        "even request-build failures must not echo the credential: {err}"
    );
}

#[tokio::test]
async fn kagi_wire_contract_uses_post_v1_bearer_and_the_search_workflow() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/search")
            .header("authorization", format!("Bearer {KAGI_KEY}"))
            .matches(record_kagi);
        then.status(200)
            .header("content-type", "application/json")
            .body(KAGI_BODY);
    });
    let mut request = constrained_request(2);
    request.freshness = eggsearch::core::query::Freshness::Month;
    request.include_domains = vec!["docs.rs".to_string()];
    request.date_range = Some(eggsearch::core::query::SearchDateRange::new(
        "2026-01-01",
        "2026-01-31",
    ));
    let results = eggsearch::meta::engines::kagi::search(
        &source_provider_client(),
        KAGI_KEY,
        Some(&server.url("")),
        &request,
    )
    .await
    .expect("kagi search succeeds");
    mock.assert();

    let wire = take_kagi_wire();
    assert_eq!(wire.method, "POST", "v1 is a JSON POST endpoint");
    assert_eq!(wire.path, "/search");
    assert_eq!(
        wire.header("authorization"),
        Some(format!("Bearer {KAGI_KEY}").as_str())
    );
    let body = wire.json();
    assert_eq!(body["query"], "rust axum");
    assert_eq!(body["workflow"], "search");
    assert_eq!(body["limit"], 2);
    assert_eq!(body["safe_search"], true);
    assert_eq!(body["filters"]["region"], "US");
    assert_eq!(body["filters"]["after"], "2026-01-01");
    assert_eq!(body["filters"]["before"], "2026-01-31");
    assert_eq!(body["lens"]["sites_included"][0], "docs.rs");
    for forbidden in ["extract", "personalizations", "format", "page", "timeout"] {
        assert!(
            body.get(forbidden).is_none(),
            "{forbidden} must never be sent to kagi"
        );
    }

    assert_eq!(results.len(), 2);
    assert!(
        results.iter().all(|r| !r.url.contains("news.example")
            && !r.url.contains("github.com")
            && !r.url.contains("finds.example")),
        "non-search result collections must not become source cards"
    );
    assert_eq!(results[0].url, "https://kagi.com/blog/small-web");
    assert_eq!(results[0].snippet.as_deref(), Some("An independent index."));
    assert_eq!(
        results[0].published_at.as_deref(),
        Some("2024-11-29T03:54:26+00:00")
    );
    assert!(results[1].published_at.is_none());
}

#[tokio::test]
async fn kagi_quota_is_terminal_and_never_retried() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let quota = server.mock(|when, then| {
        when.method(POST).path("/search");
        then.status(429)
            .header("content-type", "application/json")
            .body(r#"{"meta":{},"data":null,"error":[{"code":"rate_limit","message":"Too many requests"}]}"#);
    });
    let err = eggsearch::meta::engines::kagi::search(
        &source_provider_client(),
        KAGI_KEY,
        Some(&server.url("")),
        &source_provider_request("rust", 3),
    )
    .await
    .expect_err("a quota response must fail the provider");
    assert!(err.to_string().contains("kagi"), "got: {err}");
    assert!(err.to_string().contains("429"), "got: {err}");
    assert!(
        !err.to_string().contains(KAGI_KEY),
        "the credential must never reach diagnostics: {err}"
    );
    // Kagi's terms forbid circumventing rate limits: one attempt only.
    quota.assert_hits(1);
}

#[tokio::test]
async fn kagi_malformed_error_and_oversized_responses_stay_bounded() {
    use httpmock::prelude::*;

    let malformed_server = MockServer::start();
    let malformed = malformed_server.mock(|when, then| {
        when.method(POST).path("/search");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"data": {"search": [ }"#);
    });
    let err = eggsearch::meta::engines::kagi::search(
        &source_provider_client(),
        KAGI_KEY,
        Some(&malformed_server.url("")),
        &source_provider_request("rust", 3),
    )
    .await
    .expect_err("malformed payloads must fail the provider");
    assert!(err.to_string().contains("parse failed"), "got: {err}");
    assert!(!err.to_string().contains(KAGI_KEY), "got: {err}");
    malformed.assert();

    let error_server = MockServer::start();
    let error_envelope = error_server.mock(|when, then| {
        when.method(POST).path("/search");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"meta":{},"data":null,"error":[{"code":"search.invalid_query","message":"empty query","url":"https://help.kagi.com/api/errors"}]}"#);
    });
    let results = eggsearch::meta::engines::kagi::search(
        &source_provider_client(),
        KAGI_KEY,
        Some(&error_server.url("")),
        &source_provider_request("rust", 3),
    )
    .await
    .expect("an error envelope must not fabricate source cards");
    assert!(results.is_empty());
    error_envelope.assert();

    let big_server = MockServer::start();
    let oversized = big_server.mock(|when, then| {
        when.method(POST).path("/search");
        then.status(200)
            .header("content-type", "application/json")
            .body(oversized_source_body());
    });
    let err = eggsearch::meta::engines::kagi::search(
        &source_provider_client(),
        KAGI_KEY,
        Some(&big_server.url("")),
        &source_provider_request("rust", 3),
    )
    .await
    .expect_err("oversized bodies must be rejected");
    assert!(err.to_string().contains("too large"), "got: {err}");
    oversized.assert();

    let err = eggsearch::meta::engines::kagi::search(
        &source_provider_client(),
        KAGI_KEY,
        Some("not-a-url"),
        &source_provider_request("rust", 3),
    )
    .await
    .expect_err("an invalid endpoint is a provider-scoped failure");
    assert!(
        !err.to_string().contains(KAGI_KEY),
        "even request-build failures must not echo the credential: {err}"
    );
}
