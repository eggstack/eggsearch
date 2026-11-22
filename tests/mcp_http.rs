use std::{net::SocketAddr, time::Duration};

use axum::Router;
use eggfetch_core::{Client, Response};
use eggsearch::{
    core::config::AppConfig,
    mcp::{build_server, http, McpPath, ServeOptions},
};
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
        Self::start_with_config(AppConfig::default()).await
    }

    async fn start_with_config(config: AppConfig) -> Self {
        let options = ServeOptions {
            bind: "127.0.0.1:0".parse().unwrap(),
            path: "/mcp".parse::<McpPath>().unwrap(),
        };
        let listener = tokio::net::TcpListener::bind(options.bind).await.unwrap();
        let address = listener.local_addr().unwrap();
        let cancellation_token = CancellationToken::new();
        let server = build_server(config).unwrap();
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
            if self.client.get(url.as_str()).unwrap().send().await.is_ok() {
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
        .post(server.url().as_str())
        .unwrap()
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
    assert_eq!(response.status().as_u16(), 200);
    let session_id = response
        .headers()
        .get("Mcp-Session-Id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    (session_id, sse_payload(response).await)
}

async fn sse_payload(mut response: Response) -> Value {
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
        .post(server.url().as_str())
        .unwrap()
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
    let mut response = server
        .client
        .get(format!("http://{}{}", server.address, http::HEALTH_PATH).as_str())
        .unwrap()
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
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
    assert_eq!(initialized.status().as_u16(), 202);

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
    assert_eq!(names.len(), 9);
    assert!(names.contains(&"knowledge_brief"));
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
async fn compact_profile_is_used_by_initialized_http_tools_list() {
    let mut config = AppConfig::default();
    config.mcp.schema_profile = eggsearch::core::config::McpSchemaProfile::Compact;
    let server = TestServer::start_with_config(config).await;
    let (session_id, _) = initialize_legacy(&server).await;
    let tools = sse_payload(
        legacy_request(
            &server,
            &session_id,
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        )
        .await,
    )
    .await;
    let listed = tools["result"]["tools"].as_array().unwrap();
    let names = listed
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(names.len(), 9);
    assert!(names.contains(&"knowledge_brief"));
    assert!(names.contains(&"security_search"));
    assert!(names.contains(&"research_search"));
    assert!(listed.iter().all(|tool| tool.get("outputSchema").is_none()));
    assert!(serde_json::to_vec(listed).unwrap().len() <= 36_000);
    let compact_call = sse_payload(
        legacy_request(
            &server,
            &session_id,
            json!({
                "jsonrpc":"2.0",
                "id":3,
                "method":"tools/call",
                "params":{"name":"provider_status","arguments":{"probe":false}}
            }),
        )
        .await,
    )
    .await;
    assert_eq!(compact_call["result"]["isError"], false);

    let full_server = TestServer::start().await;
    let (full_session, _) = initialize_legacy(&full_server).await;
    let full_call = sse_payload(
        legacy_request(
            &full_server,
            &full_session,
            json!({
                "jsonrpc":"2.0",
                "id":3,
                "method":"tools/call",
                "params":{"name":"provider_status","arguments":{"probe":false}}
            }),
        )
        .await,
    )
    .await;
    assert_eq!(
        compact_call["result"]["structuredContent"]["mcp_advertisement"]["schema_profile"],
        "compact"
    );
    assert_eq!(
        compact_call["result"]["structuredContent"]["mcp_advertisement"]["tool_preset"],
        "full"
    );
    assert_eq!(
        full_call["result"]["structuredContent"]["mcp_advertisement"]["schema_profile"],
        "full"
    );
    let mut compact_content = compact_call["result"]["structuredContent"].clone();
    let mut full_content = full_call["result"]["structuredContent"].clone();
    for content in [&mut compact_content, &mut full_content] {
        content
            .as_object_mut()
            .expect("provider_status payload is an object")
            .remove("mcp_advertisement");
    }
    assert_eq!(
        compact_content, full_content,
        "the schema profile must not change provider_status beyond the advertisement block"
    );
    full_server.stop().await;
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
        .post(server.url().as_str())
        .unwrap()
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
    assert_eq!(discover.status().as_u16(), 200);
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
        .post(server.url().as_str())
        .unwrap()
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
    assert_eq!(tools.status().as_u16(), 200);
    let tools = sse_payload(tools).await;
    let listed = tools["result"]["tools"].as_array().unwrap();
    assert_eq!(listed.len(), 9);
    assert!(listed.iter().any(|tool| tool["name"] == "knowledge_brief"));
    server.stop().await;
}

async fn modern_call(server: &TestServer, id: u64, name: &str, arguments: Value) -> Value {
    let metadata = json!({
        "io.modelcontextprotocol/protocolVersion":"2026-07-28",
        "io.modelcontextprotocol/clientInfo":{"name":"eggsearch-current-test","version":"1"},
        "io.modelcontextprotocol/clientCapabilities":{}
    });
    let response = server
        .client
        .post(server.url().as_str())
        .unwrap()
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", "2026-07-28")
        .header("Mcp-Method", "tools/call")
        .header("Mcp-Name", name)
        .body(
            json!({
                "jsonrpc":"2.0",
                "id":id,
                "method":"tools/call",
                "params":{"name":name,"arguments":arguments,"_meta":metadata}
            })
            .to_string(),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    sse_payload(response).await
}

async fn modern_list(server: &TestServer, id: u64) -> Value {
    let metadata = json!({
        "io.modelcontextprotocol/protocolVersion":"2026-07-28",
        "io.modelcontextprotocol/clientInfo":{"name":"eggsearch-current-test","version":"1"},
        "io.modelcontextprotocol/clientCapabilities":{}
    });
    let response = server
        .client
        .post(server.url().as_str())
        .unwrap()
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", "2026-07-28")
        .header("Mcp-Method", "tools/list")
        .body(
            json!({"jsonrpc":"2.0","id":id,"method":"tools/list","params":{"_meta":metadata}})
                .to_string(),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    sse_payload(response).await
}

#[tokio::test]
async fn modern_call_returns_structured_success() {
    let server = TestServer::start().await;
    let result = modern_call(&server, 10, "provider_status", json!({})).await;
    assert_eq!(result["result"]["isError"], false);
    assert!(
        result["result"]["structuredContent"].is_object(),
        "modern success must carry native structuredContent: {result}"
    );
    assert!(
        result["result"]["structuredContent"]["providers"].is_array(),
        "provider_status structuredContent must include providers"
    );
    assert!(
        !result["result"]["content"].as_array().unwrap().is_empty(),
        "text fallback must be present for older clients"
    );
    server.stop().await;
}

#[tokio::test]
async fn modern_call_returns_repairable_tool_error_not_invalid_params() {
    let server = TestServer::start().await;
    let result = modern_call(
        &server,
        11,
        "repo_search",
        json!({
            "query": "test",
            "goal": "debug",
            "workflow": "security_review",
        }),
    )
    .await;
    assert_eq!(
        result["result"]["isError"], true,
        "semantic conflict must be a repairable tool error: {result}"
    );
    assert_eq!(
        result["result"]["structuredContent"]["code"],
        "conflicting_arguments"
    );
    assert!(
        result["result"]["structuredContent"]["repair"].is_object(),
        "repairable error must carry bounded repair hint: {result}"
    );
    assert!(
        result.get("error").is_none(),
        "repairable failure must not become JSON-RPC invalid_params: {result}"
    );
    server.stop().await;
}

#[tokio::test]
async fn modern_call_wrong_shape_is_tool_error_with_repair_guidance() {
    let server = TestServer::start().await;
    let result = modern_call(
        &server,
        12,
        "provider_status",
        json!({"probe": "not-a-bool"}),
    )
    .await;
    assert_eq!(
        result["result"]["isError"], true,
        "rmcp surfaces input-schema decode failures as tool errors: {result}"
    );
    let text = result["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("failed to deserialize") || text.contains("invalid type"),
        "decode failure must explain the shape problem: {result}"
    );
    server.stop().await;
}

#[tokio::test]
async fn modern_tools_list_is_deterministic_and_carries_output_schemas() {
    let server = TestServer::start().await;
    let first = modern_list(&server, 20).await;
    let second = modern_list(&server, 21).await;
    let first_tools = first["result"]["tools"].as_array().unwrap();
    let second_tools = second["result"]["tools"].as_array().unwrap();
    assert_eq!(first_tools.len(), 9);
    assert_eq!(first_tools, second_tools);
    let names: Vec<&str> = first_tools
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(
        names, sorted,
        "tools/list must be deterministically ordered"
    );
    for tool in first_tools {
        assert!(
            tool["outputSchema"].is_object(),
            "tool {} must advertise outputSchema",
            tool["name"]
        );
        assert_eq!(tool["outputSchema"]["type"], "object");
    }
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
    let build_base = || {
        server
            .client
            .post(server.url().as_str())
            .unwrap()
            .header("Accept", "application/json, text/event-stream")
            .body(initialize_body.clone())
    };
    assert_eq!(
        build_base()
            .header("Content-Type", "application/json")
            .header("Host", "evil.example")
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
        403
    );
    assert_eq!(
        build_base()
            .header("Content-Type", "application/json")
            .header("Origin", "http://evil.example")
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
        403
    );
    assert_eq!(
        build_base()
            .header("Content-Type", "text/plain")
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
        415
    );
    assert_eq!(
        build_base()
            .header("Content-Type", "application/json")
            .header("Mcp-Session-Id", "missing-session")
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
        404
    );
    let oversized = "x".repeat(http::MAX_REQUEST_BODY_BYTES + 1);
    assert_eq!(
        server
            .client
            .post(server.url().as_str())
            .unwrap()
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .body(oversized)
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
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
            .request(eggfetch_core::Method::DELETE, server.url().as_str())
            .unwrap()
            .header("Accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", "2025-06-18")
            .header("Mcp-Session-Id", session_id.as_str())
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 202);
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

#[tokio::test]
async fn minimal_preset_advertises_three_tools_and_still_executes_hidden_tools() {
    let mut config = AppConfig::default();
    config.mcp.tool_preset = eggsearch::core::config::McpToolPreset::Minimal;
    let server = TestServer::start_with_config(config).await;
    let (session_id, initialize) = initialize_legacy(&server).await;
    let instructions = initialize["result"]["instructions"]
        .as_str()
        .expect("initialize must carry instructions");
    assert!(
        !instructions.contains("provider_status"),
        "minimal instructions must not name a hidden tool: {instructions}"
    );
    assert!(instructions.contains("web_search"));

    let listed = sse_payload(
        legacy_request(
            &server,
            &session_id,
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        )
        .await,
    )
    .await;
    let tools = listed["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["build_evidence_bundle", "fetch", "web_search"]);
    assert!(names.contains(&"fetch"));
    assert!(!names.contains(&"web_fetch"));
    assert!(!names.contains(&"batch_fetch"));
    assert!(!names.contains(&"provider_status"));
    assert!(listed.get("error").is_none());
    assert!(
        listed["result"].get("_meta").is_none(),
        "advertisement must not add a tools/list _meta signal: {listed}"
    );
    assert!(tools.iter().all(|tool| tool.get("_meta").is_none()));

    let hidden = sse_payload(
        legacy_request(
            &server,
            &session_id,
            json!({
                "jsonrpc":"2.0",
                "id":3,
                "method":"tools/call",
                "params":{"name":"provider_status","arguments":{"probe":false}}
            }),
        )
        .await,
    )
    .await;
    assert!(
        hidden.get("error").is_none(),
        "transport must not reject a hidden tool: {hidden}"
    );
    assert_eq!(hidden["result"]["isError"], false, "{hidden}");
    let payload = &hidden["result"]["structuredContent"];
    assert!(payload["providers"].is_array());
    assert_eq!(payload["mcp_advertisement"]["tool_preset"], "minimal");
    assert_eq!(payload["mcp_advertisement"]["schema_profile"], "full");
    assert_eq!(
        payload["mcp_advertisement"]["advertised_tools"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        payload["mcp_advertisement"]["hidden_tools"]
            .as_array()
            .unwrap()
            .len(),
        6
    );
    assert!(payload["mcp_advertisement"]["hidden_tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool == "knowledge_brief"));
    let advertised = ["build_evidence_bundle", "fetch", "web_search"];
    let mut saw_summary_steps = false;
    for recipe in payload["workflow_recipes"].as_array().expect("recipes") {
        if let Some(step_tools) = recipe["step_tools"].as_array() {
            for tool in step_tools {
                let tool = tool.as_str().unwrap();
                assert!(
                    advertised.contains(&tool),
                    "summary step_tools leaked hidden tool {tool}: {recipe}"
                );
                saw_summary_steps = true;
            }
        }
    }
    assert!(saw_summary_steps, "summary recipes must expose step_tools");
    assert!(
        payload["next_actions_omitted"].as_u64().unwrap() >= 1,
        "dropped pointers must be counted: {payload}"
    );

    let full_recipes = sse_payload(
        legacy_request(
            &server,
            &session_id,
            json!({
                "jsonrpc":"2.0",
                "id":4,
                "method":"tools/call",
                "params":{
                    "name":"provider_status",
                    "arguments":{"probe":false,"recipe_detail":"full"}
                }
            }),
        )
        .await,
    )
    .await;
    let payload = &full_recipes["result"]["structuredContent"];
    let mut saw_steps = false;
    for recipe in payload["workflow_recipes"].as_array().expect("recipes") {
        for step in recipe["steps"].as_array().into_iter().flatten() {
            let tool = step["tool"].as_str().unwrap();
            assert!(
                advertised.contains(&tool),
                "full recipe steps leaked hidden tool {tool}: {recipe}"
            );
            saw_steps = true;
        }
        for fallback in recipe["fallbacks"].as_array().into_iter().flatten() {
            let tool = fallback["tool"].as_str().unwrap();
            assert!(
                advertised.contains(&tool),
                "full recipe fallbacks leaked hidden tool {tool}: {recipe}"
            );
        }
    }
    assert!(saw_steps, "full recipes must expose steps");
    assert!(
        payload["next_actions_omitted"].as_u64().unwrap() >= 1,
        "dropped pointers must be counted: {payload}"
    );
    server.stop().await;
}

#[tokio::test]
async fn fetch_aliases_remain_callable_but_unadvertised_over_mcp_http() {
    use httpmock::prelude::*;

    let upstream = MockServer::start_async().await;
    let _page = upstream
        .mock_async(|when, then| {
            when.method(GET).path("/doc");
            then.status(200)
                .header("content-type", "text/html; charset=utf-8")
                .body("<!doctype html><html><head><title>Fetch alias</title></head><body><p>bounded fixture</p></body></html>");
        })
        .await;
    let url = upstream.url("/doc");
    let workspace = tempfile::tempdir().expect("workspace");
    let root_name = workspace
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .expect("workspace root name")
        .to_string();
    std::fs::write(
        workspace.path().join("lib.rs"),
        "pub fn workspace_fetch_fixture() {}\n",
    )
    .expect("write workspace fixture");
    let mut config = AppConfig::default();
    config.mcp.tool_preset = eggsearch::core::config::McpToolPreset::Minimal;
    config.fetch.allow_localhost = true;
    config.fetch.allow_private_network = true;
    config.local.enabled = true;
    config.local.roots = vec![workspace.path().to_path_buf()];
    let server = TestServer::start_with_config(config).await;
    let (session_id, _) = initialize_legacy(&server).await;

    let listed = sse_payload(
        legacy_request(
            &server,
            &session_id,
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        )
        .await,
    )
    .await;
    let listed_names: Vec<&str> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        listed_names,
        ["build_evidence_bundle", "fetch", "web_search"]
    );
    assert!(!listed_names.contains(&"web_fetch"));
    assert!(!listed_names.contains(&"batch_fetch"));

    let web = sse_payload(
        legacy_request(
            &server,
            &session_id,
            json!({
                "jsonrpc":"2.0","id":3,"method":"tools/call",
                "params":{"name":"web_fetch","arguments":{"url":url}}
            }),
        )
        .await,
    )
    .await;
    assert!(web.get("error").is_none(), "legacy web_fetch route: {web}");
    assert_eq!(web["result"]["isError"], false, "{web}");
    let web_payload = &web["result"]["structuredContent"];
    assert!(
        web_payload["url"].is_string(),
        "legacy response shape: {web_payload}"
    );
    assert!(
        web_payload.get("results").is_none(),
        "legacy direct response must stay direct: {web_payload}"
    );

    let batch = sse_payload(
        legacy_request(
            &server,
            &session_id,
            json!({
                "jsonrpc":"2.0","id":4,"method":"tools/call",
                "params":{"name":"batch_fetch","arguments":{"items":[{"type":"web","url":url}]}}
            }),
        )
        .await,
    )
    .await;
    assert!(
        batch.get("error").is_none(),
        "legacy batch_fetch route: {batch}"
    );
    assert_eq!(batch["result"]["isError"], false, "{batch}");
    let batch_payload = &batch["result"]["structuredContent"];
    assert_eq!(batch_payload["results"].as_array().unwrap().len(), 1);
    assert!(batch_payload["fetched"].is_number());

    let unified = sse_payload(
        legacy_request(
            &server,
            &session_id,
            json!({
                "jsonrpc":"2.0","id":5,"method":"tools/call",
                "params":{"name":"fetch","arguments":{"items":{"type":"web","url":url}}}
            }),
        )
        .await,
    )
    .await;
    assert!(
        unified.get("error").is_none(),
        "canonical fetch route: {unified}"
    );
    assert_eq!(unified["result"]["isError"], false, "{unified}");
    let unified_payload = &unified["result"]["structuredContent"];
    assert_eq!(unified_payload["results"].as_array().unwrap().len(), 1);
    assert!(
        unified_payload.get("url").is_none(),
        "single canonical fetch stays enveloped: {unified_payload}"
    );

    let mixed = sse_payload(
        legacy_request(
            &server,
            &session_id,
            json!({
                "jsonrpc":"2.0","id":7,"method":"tools/call",
                "params":{"name":"fetch","arguments":{"items":[
                    {"type":"web","url":url},
                    {"type":"repo","host":"workspace","owner":root_name,"repo":"lib.rs","path":"lib.rs"}
                ]}}
            }),
        )
        .await,
    )
    .await;
    assert!(
        mixed.get("error").is_none(),
        "mixed canonical fetch route: {mixed}"
    );
    assert_eq!(mixed["result"]["isError"], false, "{mixed}");
    let mixed_payload = &mixed["result"]["structuredContent"];
    let mixed_results = mixed_payload["results"].as_array().expect("mixed results");
    assert_eq!(mixed_results.len(), 2, "{mixed_payload}");
    assert_eq!(mixed_results[0]["index"], 0);
    assert_eq!(mixed_results[0]["item_type"], "web");
    assert_eq!(mixed_results[0]["ok"], true);
    assert!(mixed_results[0]["response"]["url"].is_string());
    assert_eq!(mixed_results[1]["index"], 1);
    assert_eq!(mixed_results[1]["item_type"], "repo");
    assert_eq!(mixed_results[1]["ok"], true);
    assert!(mixed_results[1]["response"]["text"]
        .as_str()
        .expect("workspace text")
        .contains("workspace_fetch_fixture"));

    let repo = sse_payload(
        legacy_request(
            &server,
            &session_id,
            json!({
                "jsonrpc":"2.0","id":8,"method":"tools/call",
                "params":{"name":"fetch","arguments":{"items":{
                    "type":"repo","host":"workspace","owner":root_name,"repo":"lib.rs","path":"lib.rs"
                }}}
            }),
        )
        .await,
    )
    .await;
    assert!(
        repo.get("error").is_none(),
        "scalar repo fetch route: {repo}"
    );
    assert_eq!(repo["result"]["isError"], false, "{repo}");
    let repo_results = repo["result"]["structuredContent"]["results"]
        .as_array()
        .expect("scalar repo result envelope");
    assert_eq!(repo_results.len(), 1);
    assert_eq!(repo_results[0]["item_type"], "repo");
    assert_eq!(repo_results[0]["ok"], true);
    assert!(repo_results[0]["response"]["text"]
        .as_str()
        .expect("scalar workspace text")
        .contains("workspace_fetch_fixture"));

    let unknown = sse_payload(
        legacy_request(
            &server,
            &session_id,
            json!({
                "jsonrpc":"2.0","id":6,"method":"tools/call",
                "params":{"name":"not_a_tool","arguments":{}}
            }),
        )
        .await,
    )
    .await;
    assert!(
        unknown.get("error").is_some(),
        "unknown tool name must be rejected: {unknown}"
    );

    server.stop().await;
}

#[cfg(feature = "live-mcp-smoke")]
async fn live_mcp_call(
    server: &TestServer,
    session_id: &str,
    request_id: u64,
    tool_name: &str,
    arguments: Value,
) -> Value {
    sse_payload(
        legacy_request(
            server,
            session_id,
            json!({
                "jsonrpc":"2.0",
                "id":request_id,
                "method":"tools/call",
                "params":{"name":tool_name,"arguments":arguments}
            }),
        )
        .await,
    )
    .await
}

#[cfg(feature = "live-mcp-smoke")]
fn live_mcp_payload<'a>(tool_name: &str, response: &'a Value) -> &'a Value {
    assert!(
        response.get("error").is_none(),
        "{tool_name} returned a JSON-RPC error: {response}"
    );
    assert_eq!(
        response["result"]["isError"],
        json!(false),
        "{tool_name} returned a tool error: {response}"
    );
    let payload = &response["result"]["structuredContent"];
    assert!(
        payload.is_object(),
        "{tool_name} returned no structured content: {response}"
    );
    payload
}

#[cfg(feature = "live-mcp-smoke")]
#[tokio::test]
#[ignore = "requires live public internet and Chrome/Chromium; uses default keyless providers only"]
async fn live_canonical_tools_and_aliases_work_over_mcp_http() {
    let mut config = AppConfig::default();
    assert!(
        !cfg!(feature = "mock"),
        "live MCP smoke must not compile mock providers"
    );
    assert!(
        cfg!(feature = "pdf"),
        "PDF must be enabled for this smoke test"
    );
    assert!(
        cfg!(feature = "browser"),
        "browser must be enabled for this smoke test"
    );
    assert!(config.fetch.pdf_enabled, "PDF must be enabled by default");
    assert!(
        config.fetch.browser.enabled,
        "browser must be enabled by default"
    );
    assert_eq!(
        config.search.default_providers,
        vec![
            "duckduckgo".to_string(),
            "brave".to_string(),
            "yahoo".to_string()
        ]
    );
    assert!(
        config.search.api.is_empty(),
        "live keyless smoke must not need API provider configuration"
    );
    config.search.mode = eggsearch::core::config::Mode::Live;
    config.search.timeout_ms = 20_000;
    let server = TestServer::start_with_config(config).await;
    let (session_id, _) = initialize_legacy(&server).await;
    let initialized = legacy_request(
        &server,
        &session_id,
        json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
    )
    .await;
    assert_eq!(initialized.status().as_u16(), 202);

    let listed = sse_payload(
        legacy_request(
            &server,
            &session_id,
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        )
        .await,
    )
    .await;
    let names = listed["result"]["tools"]
        .as_array()
        .expect("tools/list array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool name"))
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "build_evidence_bundle",
            "fetch",
            "knowledge_brief",
            "provider_status",
            "repo_map",
            "repo_search",
            "research_search",
            "security_search",
            "web_search",
        ]
    );

    let web_search = live_mcp_call(
        &server,
        &session_id,
        3,
        "web_search",
        json!({"query":"Rust programming language official documentation","max_results":5}),
    )
    .await;
    let web_payload = live_mcp_payload("web_search", &web_search);
    let sources = web_payload["results"]
        .as_array()
        .expect("web_search source cards");
    assert!(
        !sources.is_empty(),
        "live web_search returned no source cards: {web_payload}"
    );
    let providers_used = sources
        .iter()
        .flat_map(|source| source["providers"].as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    assert!(
        providers_used
            .iter()
            .any(|provider| ["duckduckgo", "brave", "yahoo"].contains(provider)),
        "live web_search returned no source from a built-in keyless default provider: {web_payload}"
    );
    let source_for_bundle = sources[0].clone();
    assert!(
        source_for_bundle["url"].as_str().is_some(),
        "live source has no URL: {source_for_bundle}"
    );

    let fetch = live_mcp_call(
        &server,
        &session_id,
        4,
        "fetch",
        json!({"items":{"type":"web","url":"https://httpbin.org/html","max_chars":2000}}),
    )
    .await;
    let fetch_payload = live_mcp_payload("fetch", &fetch);
    let fetch_results = fetch_payload["results"].as_array().expect("fetch results");
    assert_eq!(fetch_results.len(), 1, "{fetch_payload}");
    assert_eq!(fetch_results[0]["ok"], true, "{fetch_payload}");
    assert!(
        fetch_results[0]["response"]["text"]
            .as_str()
            .is_some_and(|text| !text.trim().is_empty()),
        "fetch returned no document text: {fetch_payload}"
    );

    let provider_status = live_mcp_call(
        &server,
        &session_id,
        5,
        "provider_status",
        json!({"probe":true}),
    )
    .await;
    let provider_payload = live_mcp_payload("provider_status", &provider_status);
    let outcomes = provider_payload["probe"]["outcomes"]
        .as_array()
        .expect("provider probe outcomes");
    assert!(
        !outcomes.is_empty(),
        "provider_status made no live probe attempts"
    );
    assert!(
        outcomes.iter().any(|outcome| outcome["attempted"] == true),
        "provider_status did not probe any routable default provider: {provider_payload}"
    );

    let repo_search = live_mcp_call(
        &server,
        &session_id,
        6,
        "repo_search",
        json!({"query":"axum","goal":"dependency","ecosystem":"crates.io","package":"axum","version":"0.7.0","max_results":5}),
    )
    .await;
    let repo_search_payload = live_mcp_payload("repo_search", &repo_search);
    let repo_search_has_evidence = repo_search_payload["results"]
        .as_array()
        .is_some_and(|results| !results.is_empty())
        || repo_search_payload["groups"]
            .as_array()
            .is_some_and(|groups| !groups.is_empty())
        || repo_search_payload["package_resolution"].is_object();
    assert!(
        repo_search_has_evidence,
        "live repo_search produced no evidence or resolution: {repo_search_payload}"
    );

    let knowledge_brief = live_mcp_call(
        &server,
        &session_id,
        7,
        "knowledge_brief",
        json!({"topic":"Rust 1.89 release","model_id":"unknown-live-smoke-model","max_items":3}),
    )
    .await;
    let brief_payload = live_mcp_payload("knowledge_brief", &knowledge_brief);
    assert!(
        brief_payload["coverage"]["selected_providers"]
            .as_array()
            .is_some_and(|providers| !providers.is_empty()),
        "knowledge_brief did not select live providers: {brief_payload}"
    );

    let repo_map = live_mcp_call(
        &server,
        &session_id,
        8,
        "repo_map",
        json!({"owner":"tokio-rs","repo":"axum","max_depth":1,"max_entries":40}),
    )
    .await;
    let repo_map_payload = live_mcp_payload("repo_map", &repo_map);
    assert!(
        repo_map_payload["root_entries"].is_array()
            || repo_map_payload["entries"].is_array()
            || repo_map_payload["mode"] == "fallback_search",
        "live repo_map returned neither structure nor fallback evidence: {repo_map_payload}"
    );

    let security_search = live_mcp_call(
        &server,
        &session_id,
        9,
        "security_search",
        json!({"query":"CVE-2024-3094","cve_id":"CVE-2024-3094"}),
    )
    .await;
    let security_payload = live_mcp_payload("security_search", &security_search);
    assert!(
        security_payload["resolved_identifiers"]["cve_ids"]
            .as_array()
            .is_some_and(|ids| ids.iter().any(|id| id == "CVE-2024-3094")),
        "live security_search did not resolve its CVE: {security_payload}"
    );

    let research_search = live_mcp_call(
        &server,
        &session_id,
        10,
        "research_search",
        json!({"query":"Rust async runtime architecture Tokio","research_domain":"software_architecture","depth":"quick","max_results":5}),
    )
    .await;
    let research_payload = live_mcp_payload("research_search", &research_search);
    assert!(
        research_payload["groups"]
            .as_array()
            .is_some_and(|groups| !groups.is_empty()),
        "live research_search returned no evidence groups: {research_payload}"
    );

    let evidence_bundle = live_mcp_call(
        &server,
        &session_id,
        11,
        "build_evidence_bundle",
        json!({"goal":"Live MCP tool smoke","sources":[source_for_bundle]}),
    )
    .await;
    let bundle_payload = live_mcp_payload("build_evidence_bundle", &evidence_bundle);
    assert!(
        bundle_payload["bundle_id"]
            .as_str()
            .is_some_and(|id| id.starts_with("bundle_")),
        "bundle did not preserve a deterministic identity: {bundle_payload}"
    );

    let web_fetch = live_mcp_call(
        &server,
        &session_id,
        12,
        "web_fetch",
        json!({"url":"https://scrapifydatalabs.com/playground/js-rendered","max_chars":2000,"render":"browser","cache_policy":"bypass"}),
    )
    .await;
    let web_fetch_payload = live_mcp_payload("web_fetch", &web_fetch);
    assert_eq!(
        web_fetch_payload["transport"], "browser",
        "explicit browser fetch must report browser transport: {web_fetch_payload}"
    );
    assert!(
        web_fetch_payload["text"]
            .as_str()
            .is_some_and(|text| !text.trim().is_empty()),
        "live browser web_fetch returned no expected page text: {web_fetch_payload}"
    );

    let batch_fetch = live_mcp_call(
        &server,
        &session_id,
        13,
        "batch_fetch",
        json!({"items":[{"type":"web","url":"https://httpbin.org/html"}]}),
    )
    .await;
    let batch_payload = live_mcp_payload("batch_fetch", &batch_fetch);
    assert_eq!(batch_payload["results"].as_array().map(Vec::len), Some(1));
    assert_eq!(batch_payload["results"][0]["ok"], true, "{batch_payload}");

    let repo_fetch = live_mcp_call(
        &server,
        &session_id,
        14,
        "repo_fetch",
        json!({"host":"github","owner":"tokio-rs","repo":"axum","ref_name":"main","path":"Cargo.toml"}),
    )
    .await;
    let repo_fetch_payload = live_mcp_payload("repo_fetch", &repo_fetch);
    assert_eq!(repo_fetch_payload["fetched"], true, "{repo_fetch_payload}");
    assert!(
        repo_fetch_payload["text"]
            .as_str()
            .is_some_and(|text| text.contains("[package]") || text.contains("[workspace]")),
        "live repo_fetch returned unexpected content: {repo_fetch_payload}"
    );

    let pdf_fetch = live_mcp_call(
        &server,
        &session_id,
        15,
        "fetch",
        json!({"items":{"type":"web","url":"https://www.rfc-editor.org/rfc/rfc9110.pdf","max_chars":4000,"pdf":{"pages":"1"},"cache_policy":"bypass"}}),
    )
    .await;
    let pdf_payload = live_mcp_payload("fetch (PDF)", &pdf_fetch);
    let pdf_result = &pdf_payload["results"][0];
    assert_eq!(pdf_result["ok"], true, "{pdf_payload}");
    let pdf_response = &pdf_result["response"];
    assert_eq!(pdf_response["document"]["kind"], "pdf", "{pdf_payload}");
    let pdf_text = pdf_response["text"].as_str().unwrap_or("");
    let normalized_pdf_text = pdf_text.to_ascii_lowercase();
    assert!(
        normalized_pdf_text.contains("http") && normalized_pdf_text.contains("semantics"),
        "live PDF extraction returned no RFC 9110 title text: {pdf_payload}"
    );

    println!("live default provider IDs returned: {providers_used:?}");
    println!("live MCP HTTP smoke passed: nine advertised tools, three callable aliases, default keyless providers, PDF extraction, and browser rendering; no mock feature");
    server.stop().await;
}
