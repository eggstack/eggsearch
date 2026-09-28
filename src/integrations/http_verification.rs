use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

const HTTP_TIMEOUT: Duration = Duration::from_secs(5);
const BODY_LIMIT: usize = 64 * 1024;
const SESSION_LIMIT: usize = 256;
const PROTOCOL_VERSION: &str = "2025-06-18";

pub(super) async fn verify(endpoint: &str, required_tools: &[&str]) -> Result<()> {
    let http = eggfetch_core::Client::builder()
        .follow_redirects(false)
        .timeout(eggfetch_core::Timeout {
            pool: Some(HTTP_TIMEOUT),
            connect: Some(HTTP_TIMEOUT),
            write: Some(HTTP_TIMEOUT),
            read: Some(HTTP_TIMEOUT),
            total: Some(HTTP_TIMEOUT),
        })
        .max_decoded_body_size(BODY_LIMIT)
        .build();
    verify_with_client(
        &http,
        "http://127.0.0.1:11320/healthz",
        endpoint,
        required_tools,
    )
    .await
}

async fn verify_with_client(
    http: &eggfetch_core::Client,
    health_endpoint: &str,
    endpoint: &str,
    required_tools: &[&str],
) -> Result<()> {
    let mut health = http
        .get(health_endpoint)
        .context("HTTP health check failed")?
        .send()
        .await
        .context("HTTP health check failed")?;
    if !health.status().is_success() {
        bail!("HTTP health check returned {}", health.status());
    }
    let payload: Value = health.json().await.context("invalid /healthz JSON")?;
    if payload.get("service").and_then(Value::as_str) != Some("eggsearch")
        || payload.get("status").and_then(Value::as_str) != Some("ready")
    {
        bail!("/healthz did not identify a ready eggsearch service");
    }

    let (initialize, session_id) = request(
        http,
        endpoint,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "eggsearch-integration-verifier", "version": env!("CARGO_PKG_VERSION")}
            }
        }),
        None,
        true,
    )
    .await
    .context("MCP HTTP initialize failed")?;
    if initialize["result"]["protocolVersion"].as_str() != Some(PROTOCOL_VERSION) {
        bail!("MCP HTTP initialize returned an unsupported protocol response");
    }
    let session_id = session_id.context("MCP HTTP initialize omitted its session ID")?;

    request(
        http,
        endpoint,
        json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
        Some(&session_id),
        false,
    )
    .await
    .context("MCP HTTP initialized notification failed")?;
    let (tools_response, _) = request(
        http,
        endpoint,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        Some(&session_id),
        true,
    )
    .await
    .context("MCP HTTP tools/list failed")?;
    let tools = tools_response["result"]["tools"]
        .as_array()
        .context("MCP HTTP tools/list returned no tools array")?;
    let names = tools
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<Vec<_>>();
    let missing = required_tools
        .iter()
        .filter(|required| !names.contains(required))
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        bail!(
            "MCP server is missing required tools: {}",
            missing.join(", ")
        );
    }
    let mut close = http
        .delete(endpoint)
        .context("MCP HTTP session close failed")?
        .header("Mcp-Session-Id", &session_id)
        .header("MCP-Protocol-Version", PROTOCOL_VERSION)
        .send()
        .await
        .context("MCP HTTP session close failed")?;
    if !close.status().is_success() {
        bail!("MCP HTTP session close returned {}", close.status());
    }
    let _ = close
        .bytes()
        .await
        .context("MCP HTTP session close body failed")?;
    Ok(())
}

async fn request(
    http: &eggfetch_core::Client,
    endpoint: &str,
    message: Value,
    session_id: Option<&str>,
    response_expected: bool,
) -> Result<(Value, Option<String>)> {
    let mut request = http
        .post(endpoint)
        .context("invalid fixed MCP HTTP endpoint")?
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .body(message.to_string());
    if let Some(session_id) = session_id {
        request = request
            .header("Mcp-Session-Id", session_id)
            .header("MCP-Protocol-Version", PROTOCOL_VERSION);
    }
    let mut response = request.send().await.context("MCP HTTP request failed")?;
    if !response.status().is_success() {
        bail!("MCP HTTP request returned {}", response.status());
    }
    if !response_expected {
        return Ok((Value::Null, None));
    }
    let session_id = response
        .headers()
        .get("Mcp-Session-Id")
        .map(|value| {
            let value = value.to_str().context("invalid MCP session ID header")?;
            if value.is_empty() || value.len() > SESSION_LIMIT {
                bail!("MCP session ID exceeded its bound");
            }
            Ok(value.to_string())
        })
        .transpose()?;
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let body = response
        .bytes()
        .await
        .context("MCP HTTP response body failed")?;
    if body.len() > BODY_LIMIT {
        bail!("MCP HTTP response exceeded its bound");
    }
    let payload = if content_type.starts_with("text/event-stream") {
        parse_sse(&body)?
    } else if content_type.starts_with("application/json") {
        serde_json::from_slice(&body).context("invalid MCP HTTP JSON response")?
    } else {
        bail!("MCP HTTP response had an unsupported content type");
    };
    Ok((payload, session_id))
}

fn parse_sse(body: &[u8]) -> Result<Value> {
    let body = std::str::from_utf8(body)
        .context("MCP HTTP SSE response was not UTF-8")?
        .replace("\r\n", "\n");
    for event in body.split("\n\n") {
        let data = event
            .lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .map(str::trim_start)
            .collect::<Vec<_>>()
            .join("\n");
        if !data.is_empty() {
            if data.len() > BODY_LIMIT {
                bail!("MCP HTTP SSE event exceeded its bound");
            }
            return serde_json::from_str(&data).context("invalid MCP HTTP SSE data event");
        }
    }
    bail!("MCP HTTP SSE response contained no data event")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        extract::State,
        http::{Request as HttpRequest, StatusCode},
        response::Response,
        routing::any,
        Router,
    };
    use std::{
        net::SocketAddr,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
    };

    #[derive(Clone, Copy)]
    enum ReplyMode {
        Ready,
        MissingTool,
        MalformedInitialize,
        Oversized,
        OversizedSession,
        Stalled,
        NonSuccess,
        Redirect,
    }

    struct TestState {
        mode: ReplyMode,
        session_closed: AtomicBool,
    }

    async fn serve(mode: ReplyMode, timeout: Duration) -> Result<()> {
        let state = Arc::new(TestState {
            mode,
            session_closed: AtomicBool::new(false),
        });
        let app = Router::new()
            .route("/healthz", any(health))
            .route("/mcp", any(mcp))
            .with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let address: SocketAddr = listener.local_addr()?;
        let task = tokio::spawn(async move { axum::serve(listener, app).await });
        let http = eggfetch_core::Client::builder()
            .follow_redirects(false)
            .timeout(eggfetch_core::Timeout {
                pool: Some(timeout),
                connect: Some(timeout),
                write: Some(timeout),
                read: Some(timeout),
                total: Some(timeout),
            })
            .max_decoded_body_size(BODY_LIMIT)
            .build();
        let result = verify_with_client(
            &http,
            &format!("http://{address}/healthz"),
            &format!("http://{address}/mcp"),
            &["web_search", "repo_search"],
        )
        .await;
        let session_closed = state.session_closed.load(Ordering::SeqCst);
        task.abort();
        if matches!(mode, ReplyMode::Ready) && !session_closed {
            bail!("verification did not close its created session");
        }
        result
    }

    async fn health() -> Response<Body> {
        json_response(json!({"service":"eggsearch","status":"ready"}))
    }

    async fn mcp(
        State(state): State<Arc<TestState>>,
        request: HttpRequest<Body>,
    ) -> Response<Body> {
        use axum::response::IntoResponse;
        if let ReplyMode::Stalled = state.mode {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        if matches!(state.mode, ReplyMode::NonSuccess) {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        if matches!(state.mode, ReplyMode::Redirect) {
            return (
                StatusCode::TEMPORARY_REDIRECT,
                [(axum::http::header::LOCATION, "/mcp")],
            )
                .into_response();
        }
        if request.method() == axum::http::Method::DELETE {
            state.session_closed.store(true, Ordering::SeqCst);
            return StatusCode::OK.into_response();
        }
        let bytes = match to_bytes(request.into_body(), BODY_LIMIT + 1).await {
            Ok(bytes) => bytes,
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        };
        let message: Value = match serde_json::from_slice(&bytes) {
            Ok(message) => message,
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        };
        if message["method"] == "notifications/initialized" {
            return StatusCode::ACCEPTED.into_response();
        }
        if message["method"] == "initialize" {
            if matches!(state.mode, ReplyMode::MalformedInitialize) {
                return (
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    "{malformed",
                )
                    .into_response();
            }
            if matches!(state.mode, ReplyMode::Oversized) {
                return (
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    format!("{{\"padding\":\"{}\"}}", "x".repeat(BODY_LIMIT + 1)),
                )
                    .into_response();
            }
            let mut response = json_response(json!({
                "jsonrpc":"2.0",
                "id":1,
                "result":{"protocolVersion":PROTOCOL_VERSION,"capabilities":{},"serverInfo":{"name":"test","version":"1"}}
            }));
            response.headers_mut().insert(
                "Mcp-Session-Id",
                if matches!(state.mode, ReplyMode::OversizedSession) {
                    axum::http::HeaderValue::from_bytes(&vec![b'x'; SESSION_LIMIT + 1])
                        .expect("valid oversized test header")
                } else {
                    axum::http::HeaderValue::from_static("bounded-test-session")
                },
            );
            return response;
        }
        if message["method"] == "tools/list" {
            let tools = if matches!(state.mode, ReplyMode::MissingTool) {
                vec![json!({"name":"web_search"})]
            } else {
                vec![json!({"name":"web_search"}), json!({"name":"repo_search"})]
            };
            return json_response(json!({"jsonrpc":"2.0","id":2,"result":{"tools":tools}}));
        }
        StatusCode::BAD_REQUEST.into_response()
    }

    fn json_response(value: Value) -> Response<Body> {
        Response::builder()
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from(value.to_string()))
            .expect("test response")
    }

    #[test]
    fn parses_bounded_sse_data_event() {
        let parsed = parse_sse(b"event: message\r\ndata: {\"jsonrpc\":\"2.0\",\"id\":1}\r\n\r\n")
            .expect("valid SSE message");
        assert_eq!(parsed["id"], 1);
    }

    #[test]
    fn rejects_sse_without_data_or_with_invalid_json() {
        assert!(parse_sse(b"event: message\n\n").is_err());
        assert!(parse_sse(b"data: {bad json}\n\n").is_err());
    }

    #[test]
    fn rejects_oversized_sse_response() {
        assert!(parse_sse(&vec![b'a'; BODY_LIMIT + 1]).is_err());
    }

    #[tokio::test]
    async fn verifies_tools_and_closes_session() {
        serve(ReplyMode::Ready, HTTP_TIMEOUT)
            .await
            .expect("ready server verifies and closes session");
    }

    #[tokio::test]
    async fn rejects_missing_tools_and_malformed_initialize() {
        assert!(serve(ReplyMode::MissingTool, HTTP_TIMEOUT).await.is_err());
        assert!(serve(ReplyMode::MalformedInitialize, HTTP_TIMEOUT)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn rejects_oversized_timeout_status_and_redirect_responses() {
        assert!(serve(ReplyMode::Oversized, HTTP_TIMEOUT).await.is_err());
        assert!(serve(ReplyMode::OversizedSession, HTTP_TIMEOUT)
            .await
            .is_err());
        assert!(serve(ReplyMode::Stalled, Duration::from_millis(50))
            .await
            .is_err());
        assert!(serve(ReplyMode::NonSuccess, HTTP_TIMEOUT).await.is_err());
        assert!(serve(ReplyMode::Redirect, HTTP_TIMEOUT).await.is_err());
    }
}
