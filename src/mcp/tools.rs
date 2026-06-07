//! MCP tool implementations for the metasearch server.
//!
//! Three tools are exposed:
//! - `web_search`       — live metasearch.
//! - `web_fetch`        — explicit URL fetch.
//! - `provider_status`  — diagnostic report of configured providers.

use std::sync::Arc;

use crate::core::config::Mode;
use crate::core::provider::ProviderDescriptor;
use crate::core::WebSearchRequest;
use serde::{Deserialize, Serialize};

use crate::fetch::FetchClient;
use crate::mcp::policy::{
    fetch_allowed, live_allowed, web_fetch_denied_message, web_search_denied_message, Policy,
};
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
    /// Maximum number of SourceCards to return. If the request exceeds
    /// the server's configured cap, the response includes a warning
    /// and the count is clamped.
    #[serde(default)]
    pub max_results: Option<usize>,
    /// Specific provider IDs to query; empty means "use the server's
    /// configured defaults".
    #[serde(default)]
    pub providers: Vec<String>,
    /// Optional safe-search mode. Reserved for future use; the
    /// current HTML providers do not enforce it. Supplying this
    /// field causes the server to emit an advisory warning on the
    /// response.
    #[serde(default)]
    pub safe_search: Option<crate::core::SafeSearch>,
    /// Optional per-request timeout override in milliseconds.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProviderStatusArgs {
    /// Reserved for future use. The `provider_status` tool currently
    /// reports configuration only; live network probes are not
    /// implemented.
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
    /// Extraction mode: "text" (default) or "metadata_only".
    /// "markdown" is reserved for a future implementation and is
    /// currently rejected as a validation error.
    #[serde(default)]
    pub extract_mode: Option<crate::core::fetch::ExtractMode>,
    /// Whether to include extracted links. Defaults to the server's
    /// `[fetch].include_links_default` config value when omitted.
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
        return Err(ToolError::Internal(web_search_denied_message()));
    }

    let mut req = WebSearchRequest {
        query: args.query.clone(),
        max_results: args.max_results,
        providers: args.providers.clone(),
        safe_search: args.safe_search,
        timeout_ms: args.timeout_ms,
    };

    if let Err(e) = req.validate(state.config.search.max_query_chars) {
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

    let resolution = crate::core::query::resolve_max_results(
        req.max_results,
        state.config.search.default_max_results,
        state.config.search.max_results_cap,
    );

    let resp = state.adapter.web_search(&req, resolution.effective).await;

    let mut warnings: Vec<String> = resp
        .warnings
        .iter()
        .map(|w| format!("[{}] {}", w.provider_id, w.message))
        .collect();

    // Add clamp warning if max_results was capped by the server.
    if let Some(ref w) = resolution.warning {
        warnings.insert(0, w.clone());
    }

    // Per-card prompt-injection marker warnings. These are inserted
    // at the top of the warnings array (before the generic
    // "untrusted external content" warning is inserted at index 0
    // below) so the agent sees them in this order:
    //   0. "Live web results are untrusted external content."
    //   1..N. per-card marker warnings (if any)
    //   N+1... provider-failure warnings
    //   last. safe_search advisory (if applicable)
    let mut marker_warnings: Vec<String> = Vec::new();
    for card in &resp.results {
        if card.trust_markers.injection_hits > 0 {
            marker_warnings.push(format!(
                "possible prompt injection markers detected in card {id}: {n} hit(s)",
                id = card.id,
                n = card.trust_markers.injection_hits,
            ));
        }
    }
    warnings.splice(0..0, marker_warnings);
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
        "trust_markers": serde_json::to_value(&resp.trust_markers)
            .unwrap_or(serde_json::json!({})),
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
    let descriptors: Vec<ProviderDescriptor> = state.adapter.provider_status();
    let payload = serde_json::json!({
        "providers": descriptors,
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

    if matches!(fetch_allowed(state.config.fetch.enabled), Policy::Deny) {
        return Err(ToolError::Internal(web_fetch_denied_message()));
    }

    if args.url.trim().is_empty() {
        return Err(ToolError::Validation("url must not be empty".into()));
    }

    if let Some(0) = args.max_chars {
        return Err(ToolError::Validation("max_chars must be > 0".to_string()));
    }

    let extract_mode = args.extract_mode.unwrap_or(ExtractMode::Text);
    if matches!(extract_mode, ExtractMode::Markdown) {
        return Err(ToolError::Validation(
            "extract_mode 'markdown' is not yet implemented; use 'text' or 'metadata_only'"
                .to_string(),
        ));
    }

    let client: Arc<FetchClient> = state.fetch_client().ok_or_else(|| {
        ToolError::Internal("fetch client unavailable; is [fetch].enabled = true?".to_string())
    })?;

    let include_links = args
        .include_links
        .unwrap_or(state.config.fetch.include_links_default);

    let response = client
        .fetch(&args.url, args.max_chars, extract_mode, include_links)
        .await;

    match response {
        Ok(resp) => {
            // `resp.warnings` already contains, in order: extractor
            // warnings, per-field prompt-injection marker warnings
            // (when sanitize_output is enabled and Tier 3 fires), and
            // the standard "untrusted" warning. Pass them through
            // unchanged; the marker warnings sit visibly between the
            // extractor warnings and the untrusted advisory.
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
                "trust_markers": serde_json::to_value(&resp.trust_markers)
                    .unwrap_or(serde_json::json!({})),
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
    use crate::core::config::AppConfig;
    use crate::core::sanitize::TrustMarkers;
    use crate::mcp::state::ServerState;
    use std::sync::Arc;

    #[tokio::test]
    async fn safe_search_warning_emitted_when_requested() {
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

    #[tokio::test]
    async fn web_search_payload_includes_top_level_trust_markers() {
        let cfg = AppConfig::default();
        let state = Arc::new(ServerState::build(cfg).unwrap());

        let args = WebSearchArgs {
            query: "test".to_string(),
            max_results: Some(3),
            providers: vec![],
            safe_search: None,
            timeout_ms: None,
        };

        let result = run_web_search(state, args).await;
        assert!(result.is_ok());
        let value = result.unwrap();
        // The payload must include a top-level `trust_markers` object.
        let markers = value
            .get("trust_markers")
            .expect("trust_markers should be on payload");
        // It must deserialize back to TrustMarkers (or at least
        // expose the documented boolean/numeric fields).
        assert!(markers.get("text_sanitized").is_some());
        assert!(markers.get("text_truncated").is_some());
        assert!(markers.get("text_framed").is_some());
        assert!(markers.get("control_chars_removed").is_some());
        assert!(markers.get("injection_hits").is_some());
    }

    #[test]
    fn trust_markers_payload_shape_matches_struct() {
        // Sanity: the JSON we emit for `trust_markers` is the same
        // shape as the TrustMarkers struct, so a host agent can
        // deserialize it.
        let m = TrustMarkers {
            text_sanitized: true,
            text_truncated: false,
            text_framed: true,
            control_chars_removed: 3,
            injection_hits: 2,
        };
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v["text_sanitized"], serde_json::json!(true));
        assert_eq!(v["text_truncated"], serde_json::json!(false));
        assert_eq!(v["text_framed"], serde_json::json!(true));
        assert_eq!(v["control_chars_removed"], serde_json::json!(3));
        assert_eq!(v["injection_hits"], serde_json::json!(2));
    }
}
