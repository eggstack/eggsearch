//! MCP tool implementations for the metasearch server.
//!
//! Three tools are exposed:
//! - `web_search`       — live metasearch.
//! - `web_fetch`        — explicit URL fetch.
//! - `provider_status`  — diagnostic report of configured providers.

use std::sync::Arc;

use crate::core::config::Mode;
use crate::core::WebSearchRequest;
use crate::meta::response::ProviderStatus;
use serde::{Deserialize, Serialize};

use crate::fetch::FetchClient;
use crate::mcp::policy::{live_allowed, policy_message, Policy};
use crate::mcp::state::ServerState;

/// Error from a tool call, tagged by whether it reflects bad client
/// input (`Validation`) or a server-side/runtime issue (`Internal`).
#[derive(Debug)]
pub enum ToolError {
    Validation(String),
    Internal(String),
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(msg) | Self::Internal(msg) => write!(f, "{msg}"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebSearchArgs {
    /// Search query string. Must be non-empty.
    pub query: String,
    /// Maximum number of results to return. Defaults to the server's
    /// configured `max_results` and capped at `max_results_cap`.
    #[serde(default)]
    pub max_results: Option<usize>,
    /// Specific provider IDs to query; empty means "use the server's
    /// configured defaults".
    #[serde(default)]
    pub providers: Vec<String>,
    /// Optional safe-search mode.
    #[serde(default)]
    pub safe_search: Option<crate::core::SafeSearch>,
    /// Optional per-request timeout override in milliseconds.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProviderStatusArgs {
    /// Reserved for future use. The changeover MVP always reports
    /// configuration only; live network probes are not implemented.
    #[serde(default)]
    pub probe: bool,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebFetchArgs {
    /// The URL to fetch. Must be a valid HTTP(S) URL.
    pub url: String,
    /// Maximum characters to extract. Defaults to server config.
    #[serde(default)]
    pub max_chars: Option<usize>,
    /// Timeout in milliseconds. Defaults to server config.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Extraction mode: "text" (default), "markdown", "metadata_only".
    #[serde(default)]
    pub extract_mode: Option<crate::core::fetch::ExtractMode>,
    /// Whether to include extracted links. Default false.
    #[serde(default)]
    pub include_links: Option<bool>,
}

/// Run the `web_search` tool against the shared adapter. The response
/// is serialized as JSON and returned to the MCP caller.
pub async fn run_web_search(
    state: Arc<ServerState>,
    args: WebSearchArgs,
) -> Result<serde_json::Value, ToolError> {
    if matches!(live_allowed(state.config.search.mode), Policy::Deny) {
        return Err(ToolError::Internal(policy_message("web_search")));
    }

    let mut req = WebSearchRequest {
        query: args.query.clone(),
        max_results: args.max_results,
        providers: args.providers.clone(),
        safe_search: args.safe_search,
        timeout_ms: args.timeout_ms,
    };

    if let Err(e) = req.validate(
        state.config.search.max_query_chars,
        state.config.search.max_results_cap,
    ) {
        return Err(ToolError::Validation(format!("invalid query: {e}")));
    }

    let effective_providers = state
        .config
        .resolve_providers(&args.providers)
        .map_err(|e| ToolError::Internal(format!("provider resolution failed: {}", e)))?;
    let (_, unknown) = state.adapter.select_engines(&effective_providers);
    if !unknown.is_empty() {
        return Err(ToolError::Validation(format!(
            "unknown provider id(s): {}",
            unknown.join(", ")
        )));
    }

    // Ensure the adapter queries exactly the resolved set, not all
    // enabled engines (which would differ when providers is empty).
    req.providers = effective_providers.clone();

    let resp = state
        .adapter
        .web_search(
            &req,
            state.config.search.max_results,
            state.config.search.max_results_cap,
        )
        .await;

    let mut warnings: Vec<String> = resp
        .warnings
        .iter()
        .map(|w| format!("[{}] {}", w.provider_id, w.message))
        .collect();
    warnings.insert(
        0,
        "Live web results are untrusted external content.".to_string(),
    );

    if args.safe_search.is_some() {
        warnings.push(
            "safe_search is not enforced by current HTML providers; results may include unexpected content".to_string()
        );
    }

    let providers_failed: Vec<serde_json::Value> = resp
        .providers_failed
        .iter()
        .map(|f| {
            serde_json::json!({
                "id": f.id,
                "error_class": f.error_class,
                "message": f.message,
            })
        })
        .collect();

    let payload = serde_json::json!({
        "query": resp.query,
        "mode": resp.mode,
        "results": resp.results,
        "providers_queried": resp.providers_queried,
        "providers_failed": providers_failed,
        "warnings": warnings,
    });

    if providers_failed.len() == effective_providers.len()
        && !effective_providers.is_empty()
        && resp.results.is_empty()
    {
        return Err(ToolError::Internal(format!(
            "all providers failed: {}",
            providers_failed
                .iter()
                .filter_map(|v| v.get("message").and_then(|m| m.as_str()))
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }

    Ok(payload)
}

/// Run the `provider_status` tool.
pub fn run_provider_status(
    state: Arc<ServerState>,
    _args: ProviderStatusArgs,
) -> Result<serde_json::Value, String> {
    let statuses: Vec<ProviderStatus> = state.adapter.provider_status();
    let payload = serde_json::json!({
        "providers": statuses,
        "mode": mode_str(state.config.search.mode),
    });
    Ok(payload)
}

/// Run the `web_fetch` tool.
pub async fn run_web_fetch(
    state: Arc<ServerState>,
    args: WebFetchArgs,
) -> Result<serde_json::Value, ToolError> {
    use crate::core::fetch::ExtractMode;

    if args.url.trim().is_empty() {
        return Err(ToolError::Validation("url must not be empty".into()));
    }

    let limits = state.config.fetch_limits();
    let user_agent = state.config.fetch_user_agent();
    let client = FetchClient::new(limits, user_agent)
        .map_err(|e| ToolError::Internal(format!("failed to create fetch client: {e}")))?;

    let extract_mode = args.extract_mode.unwrap_or(ExtractMode::Text);

    let response = client
        .fetch(
            &args.url,
            args.max_chars,
            extract_mode,
            args.include_links.unwrap_or(false),
        )
        .await;

    match response {
        Ok(resp) => {
            let payload = serde_json::json!({
                "url": resp.url,
                "final_url": resp.final_url,
                "title": resp.title,
                "description": resp.description,
                "content_type": resp.content_type,
                "status": resp.status,
                "fetched": resp.fetched,
                "truncated": resp.truncated,
                "trust": "external_untrusted",
                "text": resp.text,
                "links": resp.links,
                "warnings": resp.warnings,
            });
            Ok(payload)
        }
        Err(e) => Err(ToolError::Internal(format!("{}: {}", e.error_code(), e))),
    }
}

fn mode_str(mode: Mode) -> &'static str {
    match mode {
        Mode::Off => "off",
        Mode::Live => "live",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn safe_search_warning_emitted_when_requested() {
        use crate::core::config::AppConfig;
        use crate::mcp::state::ServerState;
        use std::sync::Arc;

        let cfg = AppConfig::default();
        let state = Arc::new(ServerState::build(cfg).unwrap());

        let args = WebSearchArgs {
            query: "test query".to_string(),
            max_results: Some(5),
            providers: vec![],
            safe_search: Some(crate::core::SafeSearch::Strict),
            timeout_ms: None,
        };

        let result = run_web_search(state, args).await;
        assert!(result.is_ok());
        let value = result.unwrap();
        let warnings = value.get("warnings").unwrap().as_array().unwrap();
        assert!(warnings
            .iter()
            .any(|w| w.as_str().unwrap().contains("safe_search")));
    }
}
