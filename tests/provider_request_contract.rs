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
