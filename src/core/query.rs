//! Query/request types accepted by the MCP `web_search` tool.

use serde::{Deserialize, Serialize};

use crate::core::error::{CoreError, CoreResult};

/// Safe-search mode. Reserved for provider-specific enforcement; the
/// current HTML providers do not enforce it. When a `web_search`
/// request supplies this field, the server emits an advisory warning
/// rather than silently claiming enforcement.
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

/// Search intent hint. Signals what kind of result the caller is
/// looking for so post-RRF reranking can apply bounded domain
/// priors. The intent is a retrieval hint only — it must not
/// trigger multi-step research behavior.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SearchIntent {
    /// General web search.
    #[default]
    Web,
    /// Documentation search.
    Docs,
    /// Code / repository search.
    Code,
    /// Issue tracker search.
    Issues,
    /// Release / changelog search.
    Releases,
    /// Security advisory search.
    Security,
    /// News article search.
    News,
}

impl SearchIntent {
    /// Stable lowercase string form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Docs => "docs",
            Self::Code => "code",
            Self::Issues => "issues",
            Self::Releases => "releases",
            Self::Security => "security",
            Self::News => "news",
        }
    }
}

/// Freshness hint. Signals how recent the caller wants results to
/// be. Best-effort: providers that do not support date filters
/// ignore this and the adapter applies no local freshness
/// filtering.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Freshness {
    /// No freshness preference.
    #[default]
    Any,
    /// Within the last day.
    Day,
    /// Within the last week.
    Week,
    /// Within the last month.
    Month,
    /// Within the last year.
    Year,
}

impl Freshness {
    /// Stable lowercase string form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
            Self::Year => "year",
        }
    }
}

/// Input shape for the MCP `web_search` tool.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebSearchRequest {
    /// Search query string. Must be non-empty after trimming.
    pub query: String,
    /// Maximum number of SourceCards to return. The server may clamp
    /// this to its configured cap and return a warning.
    #[serde(default)]
    pub max_results: Option<usize>,
    /// Specific provider IDs to use; empty means "all enabled".
    #[serde(default)]
    pub providers: Vec<String>,
    /// Safe-search mode. Reserved for provider-specific enforcement;
    /// the current HTML providers do not enforce it. The MCP tool
    /// layer emits an advisory warning on the response when this
    /// field is supplied.
    #[serde(default)]
    pub safe_search: Option<SafeSearch>,
    /// Optional per-request timeout in milliseconds.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Search intent hint. Signals the kind of result the caller is
    /// looking for so post-RRF reranking can apply bounded domain
    /// priors. Default is `Web`.
    #[serde(default)]
    pub intent: SearchIntent,
    /// Freshness hint. Signals how recent results should be.
    /// Best-effort; providers that do not support date filters
    /// ignore this. Default is `Any`.
    #[serde(default)]
    pub freshness: Freshness,
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
    /// req.validate(512).expect("request is valid");
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
            intent: SearchIntent::default(),
            freshness: Freshness::default(),
        }
    }

    /// Validate the request, returning an error if invalid.
    pub fn validate(&self, max_query_chars: usize) -> CoreResult<()> {
        if self.query.trim().is_empty() {
            return Err(CoreError::InvalidQuery("query must not be empty".into()));
        }
        if self.query.chars().count() > max_query_chars {
            return Err(CoreError::InvalidQuery(format!(
                "query must be <= {max_query_chars} characters"
            )));
        }
        if let Some(0) = self.max_results {
            return Err(CoreError::InvalidQuery("max_results must be > 0".into()));
        }
        Ok(())
    }

    /// Effective max_results, defaulting to the given default.
    pub fn effective_max_results(&self, default: usize, cap: usize) -> usize {
        resolve_max_results(self.max_results, default, cap).effective
    }
}

/// Result of resolving the effective max_results for a request.
pub struct MaxResultsResolution {
    /// The effective max_results value to use (clamped to `[1, cap]`).
    pub effective: usize,
    /// Whether clamping was applied.
    pub clamped: bool,
    /// Optional warning message when clamping occurred.
    pub warning: Option<String>,
}

/// Resolve the effective max_results, applying the default and clamping
/// to the server cap. Returns the effective count and an optional
/// warning when clamping occurred.
pub fn resolve_max_results(
    requested: Option<usize>,
    default_max_results: usize,
    max_results_cap: usize,
) -> MaxResultsResolution {
    let requested_or_default = requested.unwrap_or(default_max_results);
    let effective = requested_or_default.clamp(1, max_results_cap);
    let clamped = requested_or_default > max_results_cap;

    let warning = clamped.then(|| {
        format!(
            "Requested max_results={} exceeded server cap={}; using {}.",
            requested_or_default, max_results_cap, effective
        )
    });

    MaxResultsResolution {
        effective,
        clamped,
        warning,
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
        assert!(req.validate(512).is_err());
    }

    #[test]
    fn validate_rejects_oversized_query() {
        let req = WebSearchRequest::new("a".repeat(1000));
        assert!(req.validate(512).is_err());
    }

    #[test]
    fn validate_rejects_zero_max_results() {
        let mut req = WebSearchRequest::new("test");
        req.max_results = Some(0);
        assert!(req.validate(512).is_err());
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

    #[test]
    fn resolve_max_results_defaults_and_clamps() {
        let r = resolve_max_results(None, 10, 50);
        assert_eq!(r.effective, 10);
        assert!(!r.clamped);
        assert!(r.warning.is_none());
    }

    #[test]
    fn resolve_max_results_clamps_oversized_with_warning() {
        let r = resolve_max_results(Some(100), 10, 50);
        assert_eq!(r.effective, 50);
        assert!(r.clamped);
        assert!(r.warning.is_some());
        assert!(r.warning.unwrap().contains("exceeded server cap"));
    }

    #[test]
    fn resolve_max_results_clamps_to_one() {
        let r = resolve_max_results(Some(0), 10, 50);
        assert_eq!(r.effective, 1);
    }

    #[test]
    fn resolve_max_results_within_cap_no_warning() {
        let r = resolve_max_results(Some(5), 10, 50);
        assert_eq!(r.effective, 5);
        assert!(!r.clamped);
        assert!(r.warning.is_none());
    }

    #[test]
    fn resolve_max_results_within_cap_override() {
        let r = resolve_max_results(Some(30), 10, 50);
        assert_eq!(r.effective, 30);
        assert!(!r.clamped);
        assert!(r.warning.is_none());
    }

    #[test]
    fn resolve_max_results_at_cap_no_warning() {
        let r = resolve_max_results(Some(50), 10, 50);
        assert_eq!(r.effective, 50);
        assert!(!r.clamped);
        assert!(r.warning.is_none());
    }
}
