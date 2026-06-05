//! MCP tool implementations for the metasearch server.
//!
//! Two tools are exposed:
//! - `web_search`       — live metasearch.
//! - `provider_status`  — diagnostic report of configured providers.

use std::sync::Arc;

use eggsearch_core::config::Mode;
use eggsearch_core::WebSearchRequest;
use eggsearch_meta::response::ProviderStatus;
use serde::{Deserialize, Serialize};

use crate::policy::{live_allowed, policy_message, Policy};
use crate::state::ServerState;

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
    pub safe_search: Option<eggsearch_core::SafeSearch>,
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

/// Run the `web_search` tool against the shared adapter. The response
/// is serialized as JSON and returned to the MCP caller.
pub async fn run_web_search(
    state: Arc<ServerState>,
    args: WebSearchArgs,
) -> Result<serde_json::Value, String> {
    if matches!(live_allowed(state.config.search.mode), Policy::Deny) {
        return Err(policy_message("web_search"));
    }

    let req = WebSearchRequest {
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
        return Err(format!("invalid query: {e}"));
    }

    let effective_providers = state.config.resolve_providers(&args.providers);
    if effective_providers.is_empty() {
        return Err("no providers are enabled in config".to_string());
    }
    let (_, unknown) = state.adapter.select_engines(&effective_providers);
    if !unknown.is_empty() {
        return Err(format!(
            "unknown provider id(s): {}",
            unknown.join(", ")
        ));
    }

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
        // All providers failed; the changeover §19 acceptance criteria
        // require a structured error in that case. Return a JSON-shaped
        // error string instead of a successful payload so the MCP
        // tool wrapper can report it.
        return Err(format!(
            "all providers failed: {}",
            providers_failed
                .iter()
                .filter_map(|v| v.get("message").and_then(|m| m.as_str()))
                .collect::<Vec<_>>()
                .join("; ")
        ));
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

fn mode_str(mode: Mode) -> &'static str {
    match mode {
        Mode::Off => "off",
        Mode::Live => "live",
    }
}
