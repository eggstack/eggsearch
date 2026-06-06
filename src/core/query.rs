//! Query/request types accepted by the MCP `web_search` tool.

use serde::{Deserialize, Serialize};

use crate::core::error::{CoreError, CoreResult};

/// Safe-search mode. Mapped to per-engine filters by the adapter.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum SafeSearch {
    /// No safe-search filtering.
    Off,
    /// Default moderate filtering.
    #[default]
    Moderate,
    /// Strict filtering.
    Strict,
}

impl SafeSearch {
    /// Stable lowercase string form (`"off"`, `"moderate"`, `"strict"`).
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
    ///
    /// # Examples
    ///
    /// ```
    /// use eggsearch::core::WebSearchRequest;
    ///
    /// let mut req = WebSearchRequest::new("rust axum middleware");
    /// req.max_results = Some(10);
    /// req.providers = vec!["duckduckgo".to_string()];
    /// req.timeout_ms = Some(8_000);
    ///
    /// // Validate against the server's limits before dispatching.
    /// req.validate(512, 50).expect("request is valid");
    /// assert_eq!(req.query, "rust axum middleware");
    /// assert_eq!(req.max_results, Some(10));
    /// ```
    pub fn new<Q: Into<String>>(query: Q) -> Self {
        Self {
            query: query.into(),
            max_results: None,
            providers: Vec::new(),
            safe_search: None,
            timeout_ms: None,
        }
    }

    /// Validate the request, returning an error if invalid.
    pub fn validate(&self, max_query_chars: usize, max_results_cap: usize) -> CoreResult<()> {
        if self.query.trim().is_empty() {
            return Err(CoreError::InvalidQuery("query must not be empty".into()));
        }
        if self.query.chars().count() > max_query_chars {
            return Err(CoreError::InvalidQuery(format!(
                "query must be <= {max_query_chars} characters"
            )));
        }
        if let Some(n) = self.max_results {
            if n == 0 {
                return Err(CoreError::InvalidQuery("max_results must be > 0".into()));
            }
            if n > max_results_cap {
                return Err(CoreError::InvalidQuery(format!(
                    "max_results must be <= {max_results_cap}"
                )));
            }
        }
        Ok(())
    }

    /// Effective max_results, defaulting to the given default.
    pub fn effective_max_results(&self, default: usize, cap: usize) -> usize {
        self.max_results.unwrap_or(default).clamp(1, cap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_search_as_str() {
        assert_eq!(SafeSearch::Off.as_str(), "off");
        assert_eq!(SafeSearch::Moderate.as_str(), "moderate");
        assert_eq!(SafeSearch::Strict.as_str(), "strict");
    }

    #[test]
    fn safe_search_default_is_moderate() {
        assert_eq!(SafeSearch::default(), SafeSearch::Moderate);
    }

    #[test]
    fn validate_rejects_empty_query() {
        let req = WebSearchRequest::new("   ");
        assert!(req.validate(512, 50).is_err());
    }

    #[test]
    fn validate_rejects_oversized_query() {
        let req = WebSearchRequest::new("a".repeat(1000));
        assert!(req.validate(512, 50).is_err());
    }

    #[test]
    fn validate_rejects_zero_max_results() {
        let mut req = WebSearchRequest::new("test");
        req.max_results = Some(0);
        assert!(req.validate(512, 50).is_err());
    }

    #[test]
    fn validate_rejects_oversized_max_results() {
        let mut req = WebSearchRequest::new("test");
        req.max_results = Some(100);
        assert!(req.validate(512, 50).is_err());
    }

    #[test]
    fn effective_max_results_defaults() {
        let req = WebSearchRequest::new("test");
        assert_eq!(req.effective_max_results(10, 50), 10);
    }

    #[test]
    fn effective_max_results_clamps_to_cap() {
        let mut req = WebSearchRequest::new("test");
        req.max_results = Some(100);
        assert_eq!(req.effective_max_results(10, 50), 50);
    }

    #[test]
    fn effective_max_results_clamps_to_one() {
        let mut req = WebSearchRequest::new("test");
        req.max_results = Some(0);
        assert_eq!(req.effective_max_results(10, 50), 1);
    }
}
