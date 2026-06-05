//! Query/request types accepted by the MCP `web_search` tool.

use serde::{Deserialize, Serialize};

/// Safe-search mode. Mapped to per-engine filters by the adapter.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SafeSearch {
    Off,
    #[default]
    Moderate,
    Strict,
}

impl SafeSearch {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Moderate => "moderate",
            Self::Strict => "strict",
        }
    }
}

/// Input shape for the MCP `web_search` tool.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebSearchRequest {
    /// Search query string. Must be non-empty after trimming.
    pub query: String,
    /// Maximum number of cards to return. Capped by the server.
    #[serde(default)]
    pub max_results: Option<usize>,
    /// Specific provider IDs to use; empty means "all enabled".
    #[serde(default)]
    pub providers: Vec<String>,
    /// Safe-search mode.
    #[serde(default)]
    pub safe_search: Option<SafeSearch>,
    /// Optional per-request timeout in milliseconds.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

impl WebSearchRequest {
    /// Build a request with the given query, applying defaults.
    pub fn new<Q: Into<String>>(query: Q) -> Self {
        Self {
            query: query.into(),
            max_results: None,
            providers: Vec::new(),
            safe_search: None,
            timeout_ms: None,
        }
    }

    /// Validate the request, returning a human-readable error string if invalid.
    pub fn validate(&self, max_query_chars: usize, max_results_cap: usize) -> Result<(), String> {
        if self.query.trim().is_empty() {
            return Err("query must not be empty".to_string());
        }
        if self.query.chars().count() > max_query_chars {
            return Err(format!(
                "query must be <= {max_query_chars} characters"
            ));
        }
        if let Some(n) = self.max_results {
            if n == 0 {
                return Err("max_results must be > 0".to_string());
            }
            if n > max_results_cap {
                return Err(format!("max_results must be <= {max_results_cap}"));
            }
        }
        Ok(())
    }

    /// Effective max_results, defaulting to the given default.
    pub fn effective_max_results(&self, default: usize, cap: usize) -> usize {
        self.max_results.unwrap_or(default).clamp(1, cap)
    }
}
