use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
    time::Duration,
};

use axum::{
    body::Body,
    extract::Request,
    http::{header, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use serde::Serialize;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use crate::{core::config::AppConfig, mcp::EggsearchServer};

pub const DEFAULT_BIND: SocketAddr =
    SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 11320);
pub const DEFAULT_PATH: &str = "/mcp";
pub const HEALTH_PATH: &str = "/healthz";
pub const MAX_REQUEST_BODY_BYTES: usize = 1024 * 1024;
pub const MAX_HEADER_COUNT: usize = 64;
pub const MAX_HEADER_BYTES: usize = 16 * 1024;
pub const MAX_HEADER_VALUE_BYTES: usize = 8 * 1024;
pub const MAX_SESSION_ID_BYTES: usize = 256;
pub const MAX_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
pub const SHUTDOWN_DRAIN_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_HEALTH_BODY_BYTES: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpPath(String);

impl McpPath {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for McpPath {
    type Err = String;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        if raw.len() > 128 {
            return Err("MCP path must not exceed 128 bytes".to_string());
        }
        if raw == HEALTH_PATH {
            return Err("MCP path must not be /healthz".to_string());
        }
        if !raw.starts_with('/') || raw == "/" || raw.starts_with("//") {
            return Err("MCP path must be an absolute path other than /".to_string());
        }
        let normalized = raw.trim_end_matches('/');
        if normalized.is_empty() || normalized == HEALTH_PATH || normalized.contains("//") {
            return Err("MCP path must not contain empty path segments".to_string());
        }
        if normalized.bytes().any(|byte| {
            !(byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~'))
        }) {
            return Err("MCP path contains an unsupported character".to_string());
        }
        Ok(Self(normalized.to_string()))
    }
}

impl std::fmt::Display for McpPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServeOptions {
    pub bind: SocketAddr,
    pub path: McpPath,
}

impl Default for ServeOptions {
    fn default() -> Self {
        Self {
            bind: DEFAULT_BIND,
            path: McpPath::from_str(DEFAULT_PATH).expect("default MCP path is valid"),
        }
    }
}

impl ServeOptions {
    pub fn validate(&self) -> anyhow::Result<()> {
        if !self.bind.ip().is_loopback() {
            anyhow::bail!(
                "non-loopback MCP serving is not enabled by this release; use stdio/local loopback or implement the remote-auth deployment plan"
            );
        }
        if self.path.as_str() == HEALTH_PATH {
            anyhow::bail!("MCP path must not be /healthz");
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    service: &'static str,
    status: &'static str,
    version: &'static str,
    protocol: &'static str,
}

pub fn router(
    server: EggsearchServer,
    options: &ServeOptions,
    cancellation_token: CancellationToken,
) -> Router {
    let http_config = StreamableHttpServerConfig::default()
        .with_allowed_hosts(["localhost", "127.0.0.1", "::1"])
        .with_allowed_origins(["http://localhost", "http://127.0.0.1", "http://[::1]"])
        .with_max_request_body_bytes(MAX_REQUEST_BODY_BYTES)
        .with_cancellation_token(cancellation_token);
    let service = StreamableHttpService::new(
        move || Ok(server.clone()),
        std::sync::Arc::new(LocalSessionManager::default()),
        http_config,
    );

    Router::new()
        .route(HEALTH_PATH, get(healthz))
        .nest_service(options.path.as_str(), service)
        .layer(middleware::from_fn(request_timeout))
        .layer(middleware::from_fn(request_header_limits))
}

pub async fn run(cfg: &AppConfig, options: ServeOptions) -> anyhow::Result<()> {
    run_with_cancellation(cfg, options, CancellationToken::new()).await
}

pub async fn run_with_cancellation(
    cfg: &AppConfig,
    options: ServeOptions,
    cancellation_token: CancellationToken,
) -> anyhow::Result<()> {
    options.validate()?;
    let server = crate::mcp::build_server(cfg.clone())?;
    let listener = TcpListener::bind(options.bind).await?;
    let address = listener.local_addr()?;
    let app = router(server, &options, cancellation_token.clone());
    tracing::info!(
        bind = %address,
        path = %options.path,
        health = %format!("http://{address}{HEALTH_PATH}"),
        "starting eggsearch MCP server over Streamable HTTP"
    );

    let server_token = cancellation_token.clone();
    let mut server_task = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(server_token.cancelled_owned())
            .await
    });
    let signal = shutdown_signal();
    tokio::pin!(signal);

    tokio::select! {
        result = &mut server_task => {
            result??;
        }
        _ = &mut signal => {
            cancellation_token.cancel();
            match tokio::time::timeout(SHUTDOWN_DRAIN_TIMEOUT, &mut server_task).await {
                Ok(result) => result??,
                Err(_) => {
                    server_task.abort();
                    let _ = server_task.await;
                    anyhow::bail!("timed out draining Streamable HTTP connections");
                }
            }
        }
    }
    tracing::info!("eggsearch MCP server stopped");
    Ok(())
}

async fn healthz() -> Response {
    let body = serde_json::to_vec(&HealthResponse {
        service: "eggsearch",
        status: "ready",
        version: env!("CARGO_PKG_VERSION"),
        protocol: "streamable-http",
    })
    .expect("health response is serializable");
    if body.len() > MAX_HEALTH_BODY_BYTES {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "health response exceeded bound",
        )
            .into_response();
    }
    let mut response = Response::new(Body::from(body.clone()));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&body.len().to_string()).expect("body length is valid"),
    );
    response
}

async fn request_header_limits(request: Request, next: Next) -> Response {
    let header_count = request.headers().len();
    let header_bytes = request
        .headers()
        .iter()
        .map(|(name, value)| name.as_str().len() + value.len())
        .sum::<usize>();
    let oversized_value = request
        .headers()
        .iter()
        .any(|(_, value)| value.len() > MAX_HEADER_VALUE_BYTES);
    let oversized_session_id = request
        .headers()
        .get("mcp-session-id")
        .is_some_and(|value| value.len() > MAX_SESSION_ID_BYTES);
    if header_count > MAX_HEADER_COUNT
        || header_bytes > MAX_HEADER_BYTES
        || oversized_value
        || oversized_session_id
    {
        return (
            StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
            "request headers too large",
        )
            .into_response();
    }
    next.run(request).await
}

async fn request_timeout(request: Request, next: Next) -> Response {
    match tokio::time::timeout(MAX_REQUEST_TIMEOUT, next.run(request)).await {
        Ok(response) => response,
        Err(_) => (StatusCode::REQUEST_TIMEOUT, "request timed out").into_response(),
    }
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("SIGTERM handler can be installed");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = sigterm.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c()
            .await
            .expect("Ctrl-C handler can be installed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_parser_normalizes_trailing_slashes() {
        assert_eq!(McpPath::from_str("/mcp/").unwrap().as_str(), "/mcp");
    }

    #[test]
    fn path_parser_rejects_health_and_ambiguous_paths() {
        for path in ["/", "/healthz", "mcp", "/mcp//x", "/mcp?x=1", "/mcp\\x"] {
            assert!(McpPath::from_str(path).is_err(), "accepted {path}");
        }
    }

    #[test]
    fn serve_options_reject_non_loopback() {
        let options = ServeOptions {
            bind: "0.0.0.0:11320".parse().unwrap(),
            ..ServeOptions::default()
        };
        let error = options
            .validate()
            .expect_err("non-loopback bind should fail");
        assert!(error.to_string().contains("non-loopback"));
    }
}
