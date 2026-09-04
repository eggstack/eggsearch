use std::{net::SocketAddr, time::Duration};

use axum::Router;
use eggsearch::{
    core::config::AppConfig,
    mcp::{build_server, http, McpPath, ServeOptions},
};
use reqwest::{Client, Response};
use serde_json::{json, Value};
use tokio::{task::JoinHandle, time::sleep};
use tokio_util::sync::CancellationToken;

struct TestServer {
    client: Client,
    address: SocketAddr,
    cancellation_token: CancellationToken,
    task: JoinHandle<Result<(), std::io::Error>>,
}

impl TestServer {
    async fn start() -> Self {
        let options = ServeOptions {
            bind: "127.0.0.1:0".parse().unwrap(),
            path: "/mcp".parse::<McpPath>().unwrap(),
        };
        let listener = tokio::net::TcpListener::bind(options.bind).await.unwrap();
        let address = listener.local_addr().unwrap();
        let cancellation_token = CancellationToken::new();
        let server = build_server(AppConfig::default()).unwrap();
        let app = http::router(server, &options, cancellation_token.clone());
        let shutdown = cancellation_token.clone();
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown.cancelled_owned())
                .await
        });
        let test_server = Self {
            client: Client::new(),
            address,
            cancellation_token,
            task,
        };
        test_server.wait_until_ready().await;
        test_server
    }

    fn url(&self) -> String {
        format!("http://{}/mcp", self.address)
    }

    async fn wait_until_ready(&self) {
        let url = format!("http://{}{}", self.address, http::HEALTH_PATH);
        for _ in 0..50 {
            if self.client.get(&url).send().await.is_ok() {
                return;
            }
            sleep(Duration::from_millis(10)).await;
        }
        panic!("health endpoint did not become ready");
    }

    async fn stop(self) {
        self.cancellation_token.cancel();
        tokio::time::timeout(Duration::from_secs(2), self.task)
            .await
            .expect("HTTP server should stop")
            .expect("HTTP server task should not panic")
            .expect("HTTP server should stop cleanly");
    }
}

async fn initialize_legacy(server: &TestServer) -> (String, Value) {
    let response = server
        .client
        .post(server.url())
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .body(
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {"name": "eggsearch-test", "version": "1"}
                }
            })
            .to_string(),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let session_id = response
        .headers()
        .get("Mcp-Session-Id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    (session_id, sse_payload(response).await)
}

async fn sse_payload(response: Response) -> Value {
    let body = response.text().await.unwrap();
    let data = body
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .find(|line| !line.is_empty())
        .expect("SSE response should contain a data event");
    serde_json::from_str(data).unwrap()
}

async fn legacy_request(server: &TestServer, session_id: &str, message: Value) -> Response {
    server
        .client
        .post(server.url())
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", "2025-06-18")
        .header("Mcp-Session-Id", session_id)
        .body(message.to_string())
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn health_is_bounded_identified_and_does_not_use_mcp_state() {
    let server = TestServer::start().await;
    let response = server
        .client
        .get(format!("http://{}{}", server.address, http::HEALTH_PATH))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["content-type"], "application/json");
    let body = response.bytes().await.unwrap();
    assert!(body.len() <= 256);
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap(),
        json!({
            "service": "eggsearch",
            "status": "ready",
            "version": env!("CARGO_PKG_VERSION"),
            "protocol": "streamable-http"
        })
    );
    server.stop().await;
}

#[tokio::test]
async fn legacy_http_lifecycle_lists_tools_and_calls_local_tool() {
    let server = TestServer::start().await;
    let (session_id, initialize) = initialize_legacy(&server).await;
    assert_eq!(initialize["result"]["serverInfo"]["name"], "eggsearch");
    assert_eq!(initialize["result"]["protocolVersion"], "2025-06-18");

    let initialized = legacy_request(
        &server,
        &session_id,
        json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
    )
    .await;
    assert_eq!(initialized.status(), 202);

    let tools = sse_payload(
        legacy_request(
            &server,
            &session_id,
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        )
        .await,
    )
    .await;
    let names = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(names.len(), 10);
    assert!(names.contains(&"web_search"));
    assert!(names.contains(&"build_evidence_bundle"));

    let tool_result = sse_payload(
        legacy_request(
            &server,
            &session_id,
            json!({
                "jsonrpc":"2.0",
                "id":3,
                "method":"tools/call",
                "params":{"name":"provider_status","arguments":{}}
            }),
        )
        .await,
    )
    .await;
    assert_eq!(tool_result["result"]["isError"], false);
    server.stop().await;
}

#[tokio::test]
async fn current_http_protocol_uses_request_metadata_without_a_session() {
    let server = TestServer::start().await;
    let metadata = json!({
        "io.modelcontextprotocol/protocolVersion":"2026-07-28",
        "io.modelcontextprotocol/clientInfo":{"name":"eggsearch-current-test","version":"1"},
        "io.modelcontextprotocol/clientCapabilities":{}
    });
    let discover = server
        .client
        .post(server.url())
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", "2026-07-28")
        .header("Mcp-Method", "server/discover")
        .body(
            json!({
                "jsonrpc":"2.0",
                "id":1,
                "method":"server/discover",
                "params":{"_meta":metadata.clone()}
            })
            .to_string(),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(discover.status(), 200);
    assert!(discover.headers().get("Mcp-Session-Id").is_none());
    let discover = sse_payload(discover).await;
    assert_eq!(
        discover["result"]["supportedVersions"]
            .as_array()
            .unwrap()
            .len(),
        5
    );

    let tools = server
        .client
        .post(server.url())
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", "2026-07-28")
        .header("Mcp-Method", "tools/list")
        .body(
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{"_meta":metadata}})
                .to_string(),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(tools.status(), 200);
    let tools = sse_payload(tools).await;
    assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 10);
    server.stop().await;
}

#[tokio::test]
async fn invalid_host_origin_content_type_session_and_body_are_rejected() {
    let server = TestServer::start().await;
    let initialize_body = json!({
        "jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1"}
        }
    })
    .to_string();
    let base = server
        .client
        .post(server.url())
        .header("Accept", "application/json, text/event-stream")
        .body(initialize_body.clone());
    assert_eq!(
        base.try_clone()
            .unwrap()
            .header("Content-Type", "application/json")
            .header("Host", "evil.example")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        base.try_clone()
            .unwrap()
            .header("Content-Type", "application/json")
            .header("Origin", "http://evil.example")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        base.try_clone()
            .unwrap()
            .header("Content-Type", "text/plain")
            .send()
            .await
            .unwrap()
            .status(),
        415
    );
    assert_eq!(
        base.try_clone()
            .unwrap()
            .header("Content-Type", "application/json")
            .header("Mcp-Session-Id", "missing-session")
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    let oversized = "x".repeat(http::MAX_REQUEST_BODY_BYTES + 1);
    assert_eq!(
        server
            .client
            .post(server.url())
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .body(oversized)
            .send()
            .await
            .unwrap()
            .status(),
        413
    );
    server.stop().await;
}

#[tokio::test]
async fn repeated_sessions_can_be_terminated_and_do_not_affect_new_sessions() {
    let server = TestServer::start().await;
    for _ in 1..=3 {
        let (session_id, initialize) = initialize_legacy(&server).await;
        assert_eq!(initialize["id"], 1);
        let response = server
            .client
            .request(reqwest::Method::DELETE, server.url())
            .header("Accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", "2025-06-18")
            .header("Mcp-Session-Id", session_id)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 202);
    }
    let (_, initialize) = initialize_legacy(&server).await;
    assert_eq!(initialize["result"]["serverInfo"]["name"], "eggsearch");
    server.stop().await;
}

#[test]
fn stdio_and_http_use_the_same_tool_definitions() {
    let server = build_server(AppConfig::default()).unwrap();
    let http_server = server.clone();
    let stdio = server
        .tool_definitions()
        .into_iter()
        .map(|tool| tool.name.to_string())
        .collect::<Vec<_>>();
    let http = http_server
        .tool_definitions()
        .into_iter()
        .map(|tool| tool.name.to_string())
        .collect::<Vec<_>>();
    assert_eq!(stdio, http);
}

#[test]
fn router_requires_loopback_options() {
    let options = ServeOptions {
        bind: "192.0.2.1:11320".parse().unwrap(),
        ..ServeOptions::default()
    };
    assert!(options.validate().is_err());
    let _: Router = http::router(
        build_server(AppConfig::default()).unwrap(),
        &ServeOptions::default(),
        CancellationToken::new(),
    );
}
