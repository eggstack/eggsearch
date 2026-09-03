//! Query/request types accepted by the MCP `web_search` tool.

use serde::{Deserialize, Deserializer, Serialize};

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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, schemars::JsonSchema)]
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

impl<'de> Deserialize<'de> for SearchIntent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        SearchIntent::from_alias(&s).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "invalid search intent `{s}`; valid values: web, docs, code, issues, releases, security, news"
            ))
        })
    }
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

    /// Map a string (possibly an alias from a weaker model) to the
    /// canonical variant. Returns `None` for unrecognized values.
    fn from_alias(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            // web
            "web" | "general" | "general_web" => Some(Self::Web),
            // docs
            "docs" | "doc" | "documentation" => Some(Self::Docs),
            // code
            "code" | "source" | "source_code" | "repo" | "repository" | "repositories"
            | "github" | "gitlab" => Some(Self::Code),
            // issues
            "issues" | "issue" | "bug" | "bugs" | "discussion" | "discussions" | "pr"
            | "pull_request" => Some(Self::Issues),
            // releases
            "releases" | "release" | "changelog" | "changelogs" | "changes" | "migration" => {
                Some(Self::Releases)
            }
            // security
            "security" | "sec" | "advisory" | "advisories" | "cve" | "vulnerability"
            | "vulnerabilities" | "vuln" | "vulns" => Some(Self::Security),
            // news
            "news" | "current_events" => Some(Self::News),
            _ => None,
        }
    }
}

/// Freshness hint. Signals how recent the caller wants results to
/// be. Best-effort: providers that do not support date filters
/// ignore this and the adapter applies no local freshness
/// filtering.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, schemars::JsonSchema)]
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

impl<'de> Deserialize<'de> for Freshness {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Freshness::from_alias(&s).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "invalid freshness `{s}`; valid values: any, day, week, month, year"
            ))
        })
    }
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

    /// Map a string (possibly an alias from a weaker model) to the
    /// canonical variant. Returns `None` for unrecognized values.
    fn from_alias(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "any" | "none" | "all" => Some(Self::Any),
            "day" | "today" | "24h" | "1d" => Some(Self::Day),
            "week" | "7d" | "weekly" => Some(Self::Week),
            "month" | "30d" | "monthly" | "latest" | "recent" => Some(Self::Month),
            "year" | "365d" | "yearly" | "12mo" => Some(Self::Year),
            _ => None,
        }
    }
}

/// Exact calendar date range for provider-neutral freshness filtering.
///
/// Both bounds are ISO `YYYY-MM-DD` dates. When present, the range is
/// mutually exclusive with a non-`any` relative [`Freshness`] hint to
/// avoid provider-precedence ambiguity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchDateRange {
    /// Inclusive start date in `YYYY-MM-DD` form.
    pub start: String,
    /// Inclusive end date in `YYYY-MM-DD` form.
    pub end: String,
}

impl SearchDateRange {
    /// Build a date range from two ISO date strings.
    pub fn new<S: Into<String>>(start: S, end: S) -> Self {
        Self {
            start: start.into(),
            end: end.into(),
        }
    }
}

/// Maximum entries allowed per domain include/exclude list.
pub const MAX_DOMAIN_FILTERS: usize = 32;
/// Maximum total hostname length (DNS-compatible).
pub const MAX_HOSTNAME_LEN: usize = 253;
/// Maximum single DNS label length.
pub const MAX_LABEL_LEN: usize = 63;
/// Maximum language hint length.
pub const MAX_LANGUAGE_LEN: usize = 32;
/// Maximum region hint length.
pub const MAX_REGION_LEN: usize = 32;

/// Normalize a single domain filter entry to a lowercase hostname.
///
/// Rejects schemes, credentials, ports, paths, query strings,
/// fragments, empty labels, and wildcard syntax.
pub fn normalize_domain(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("domain must not be empty".to_string());
    }
    if trimmed.len() > MAX_HOSTNAME_LEN {
        return Err(format!(
            "domain '{trimmed}' exceeds max length {MAX_HOSTNAME_LEN}"
        ));
    }
    if trimmed.contains("://") {
        return Err(format!("domain '{trimmed}' must not contain a scheme"));
    }
    if trimmed.contains('@') {
        return Err(format!("domain '{trimmed}' must not contain credentials"));
    }
    if trimmed.contains('/') || trimmed.contains('?') || trimmed.contains('#') {
        return Err(format!(
            "domain '{trimmed}' must not contain a path, query, or fragment"
        ));
    }
    if trimmed.contains(':') {
        return Err(format!("domain '{trimmed}' must not contain a port"));
    }
    if trimmed.contains('*') {
        return Err(format!(
            "domain '{trimmed}' must not contain wildcard syntax"
        ));
    }
    if trimmed.chars().any(|c| c.is_whitespace()) {
        return Err(format!("domain '{trimmed}' must not contain whitespace"));
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with('.') || lower.ends_with('.') {
        return Err(format!("domain '{trimmed}' must not have empty labels"));
    }
    if lower.contains("..") {
        return Err(format!("domain '{trimmed}' must not have empty labels"));
    }
    for label in lower.split('.') {
        if label.is_empty() {
            return Err(format!("domain '{trimmed}' must not have empty labels"));
        }
        if label.len() > MAX_LABEL_LEN {
            return Err(format!(
                "domain '{trimmed}' has a label exceeding {MAX_LABEL_LEN} chars"
            ));
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err(format!(
                "domain '{trimmed}' has a label with leading/trailing hyphen"
            ));
        }
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(format!(
                "domain '{trimmed}' must contain only alphanumeric characters and hyphens"
            ));
        }
    }
    Ok(lower)
}

/// Returns `true` when `host` matches `filter` on a label boundary.
///
/// `example.com` matches `example.com` and `docs.example.com` but not
/// `notexample.com`. Both inputs should already be lowercase hostnames.
pub fn domain_matches_filter(host: &str, filter: &str) -> bool {
    if host == filter {
        return true;
    }
    host.len() > filter.len()
        && host.ends_with(filter)
        && host.as_bytes()[host.len() - filter.len() - 1] == b'.'
}

/// Extract the lowercase hostname from an HTTP(S) URL for domain filtering.
///
/// Returns `None` when the URL does not parse or has no host.
pub fn hostname_from_url(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()))
}

/// Validate a language hint string.
///
/// Bounded and syntactically conservative: 2-32 chars, starts with a
/// letter, contains only alphanumerics, hyphens, and underscores, and
/// ends with an alphanumeric. Provider support is best-effort unless
/// capability-enforced.
pub fn validate_language(value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("language must not be empty".to_string());
    }
    if trimmed.len() < 2 || trimmed.len() > MAX_LANGUAGE_LEN {
        return Err(format!("language must be 2-{MAX_LANGUAGE_LEN} chars"));
    }
    validate_locale_syntax(trimmed, "language")
}

/// Validate a region hint string.
///
/// Same conservative syntax as [`validate_language`]; documented as
/// best-effort unless a provider natively enforces it.
pub fn validate_region(value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("region must not be empty".to_string());
    }
    if trimmed.len() < 2 || trimmed.len() > MAX_REGION_LEN {
        return Err(format!("region must be 2-{MAX_REGION_LEN} chars"));
    }
    validate_locale_syntax(trimmed, "region")
}

fn validate_locale_syntax(value: &str, field: &str) -> Result<(), String> {
    let bytes = value.as_bytes();
    if !bytes[0].is_ascii_alphabetic() {
        return Err(format!("{field} must start with a letter"));
    }
    if !bytes[bytes.len() - 1].is_ascii_alphanumeric() {
        return Err(format!("{field} must end with an alphanumeric"));
    }
    if !value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!(
            "{field} must contain only alphanumerics, hyphens, and underscores"
        ));
    }
    Ok(())
}

fn parse_iso_date(value: &str) -> Result<chrono::NaiveDate, String> {
    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| format!("invalid date '{value}'; expected YYYY-MM-DD"))
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
    /// Exact calendar date range (`YYYY-MM-DD` start/end). Mutually
    /// exclusive with a non-`any` relative `freshness`.
    #[serde(default)]
    pub date_range: Option<SearchDateRange>,
    /// Include-only domain filters (lowercase hostnames). Enforced
    /// locally on result URLs; provider-native enforcement is tracked
    /// separately in capability telemetry.
    #[serde(default)]
    pub include_domains: Vec<String>,
    /// Exclude domain filters (lowercase hostnames). Enforced locally.
    #[serde(default)]
    pub exclude_domains: Vec<String>,
    /// Language hint (e.g. `en`, `en-US`). Bounded conservative
    /// syntax; provider support is best-effort unless enforced.
    #[serde(default)]
    pub language: Option<String>,
    /// Region hint (e.g. `US`, `GB`). Bounded conservative syntax;
    /// provider support is best-effort unless enforced.
    #[serde(default)]
    pub region: Option<String>,
    /// Optional excerpt demand: how many additional source-derived
    /// excerpts each `SourceCard` may carry (0-3). Defaults to zero so
    /// search output remains compact discovery-only cards.
    #[serde(default)]
    pub excerpt_count: Option<usize>,
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
            date_range: None,
            include_domains: Vec::new(),
            exclude_domains: Vec::new(),
            language: None,
            region: None,
            excerpt_count: None,
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
        if let Some(0) = self.timeout_ms {
            return Err(CoreError::InvalidQuery("timeout_ms must be > 0".into()));
        }
        if let Some(range) = &self.date_range {
            let start = parse_iso_date(range.start.trim()).map_err(CoreError::InvalidQuery)?;
            let end = parse_iso_date(range.end.trim()).map_err(CoreError::InvalidQuery)?;
            if start > end {
                return Err(CoreError::InvalidQuery(
                    "date_range start must be <= end".into(),
                ));
            }
            if self.freshness != Freshness::Any {
                return Err(CoreError::InvalidQuery(
                    "date_range and freshness are mutually exclusive".into(),
                ));
            }
        }
        if self.include_domains.len() > MAX_DOMAIN_FILTERS {
            return Err(CoreError::InvalidQuery(format!(
                "include_domains must contain <= {MAX_DOMAIN_FILTERS} entries"
            )));
        }
        if self.exclude_domains.len() > MAX_DOMAIN_FILTERS {
            return Err(CoreError::InvalidQuery(format!(
                "exclude_domains must contain <= {MAX_DOMAIN_FILTERS} entries"
            )));
        }
        let mut normalized_include = Vec::with_capacity(self.include_domains.len());
        for raw in &self.include_domains {
            match normalize_domain(raw) {
                Ok(n) => normalized_include.push(n),
                Err(reason) => {
                    return Err(CoreError::InvalidQuery(format!(
                        "invalid include_domains entry: {reason}"
                    )));
                }
            }
        }
        let mut normalized_exclude = Vec::with_capacity(self.exclude_domains.len());
        for raw in &self.exclude_domains {
            match normalize_domain(raw) {
                Ok(n) => normalized_exclude.push(n),
                Err(reason) => {
                    return Err(CoreError::InvalidQuery(format!(
                        "invalid exclude_domains entry: {reason}"
                    )));
                }
            }
        }
        for host in &normalized_include {
            if normalized_exclude.iter().any(|e| e == host) {
                return Err(CoreError::InvalidQuery(format!(
                    "domain '{host}' appears in both include_domains and exclude_domains"
                )));
            }
        }
        if let Some(lang) = &self.language {
            validate_language(lang)
                .map_err(|reason| CoreError::InvalidQuery(format!("invalid language: {reason}")))?;
        }
        if let Some(region) = &self.region {
            validate_region(region)
                .map_err(|reason| CoreError::InvalidQuery(format!("invalid region: {reason}")))?;
        }
        if let Some(count) = self.excerpt_count {
            if count > crate::core::source_card::MAX_EXCERPT_REQUEST_COUNT {
                return Err(CoreError::InvalidQuery(format!(
                    "excerpt_count must be <= {}",
                    crate::core::source_card::MAX_EXCERPT_REQUEST_COUNT
                )));
            }
        }
        Ok(())
    }

    /// Effective excerpt demand, defaulting to zero (compact cards).
    pub fn effective_excerpt_count(&self) -> usize {
        self.excerpt_count
            .unwrap_or(0)
            .min(crate::core::source_card::MAX_EXCERPT_REQUEST_COUNT)
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
    let clamped = requested_or_default == 0 || requested_or_default > max_results_cap;

    let value_name = if requested.is_some() {
        "Requested"
    } else {
        "Default"
    };
    let warning = match requested_or_default {
        0 => Some(format!(
            "{value_name} max_results=0 is below the minimum of 1; using 1."
        )),
        n if n > max_results_cap => Some(format!(
            "{value_name} max_results={requested_or_default} exceeded server cap={max_results_cap}; using {effective}."
        )),
        _ => None,
    };

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
    fn validate_rejects_zero_timeout_ms() {
        let mut req = WebSearchRequest::new("test");
        req.timeout_ms = Some(0);
        let err = req.validate(512).unwrap_err();
        assert!(err.to_string().contains("timeout_ms"));
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
    fn resolve_max_results_default_over_cap_names_default() {
        let r = resolve_max_results(None, 100, 50);
        assert_eq!(r.effective, 50);
        assert!(r.clamped);
        assert_eq!(
            r.warning.as_deref(),
            Some("Default max_results=100 exceeded server cap=50; using 50.")
        );
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
        assert!(r.clamped);
        assert!(r.warning.is_some());
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

    // --- SearchIntent alias deserialization ---

    #[test]
    fn search_intent_alias_documentation() {
        let v: SearchIntent = serde_json::from_str("\"documentation\"").unwrap();
        assert_eq!(v, SearchIntent::Docs);
    }

    #[test]
    fn search_intent_alias_github() {
        let v: SearchIntent = serde_json::from_str("\"github\"").unwrap();
        assert_eq!(v, SearchIntent::Code);
    }

    #[test]
    fn search_intent_alias_bug() {
        let v: SearchIntent = serde_json::from_str("\"bug\"").unwrap();
        assert_eq!(v, SearchIntent::Issues);
    }

    #[test]
    fn search_intent_alias_changelog() {
        let v: SearchIntent = serde_json::from_str("\"changelog\"").unwrap();
        assert_eq!(v, SearchIntent::Releases);
    }

    #[test]
    fn search_intent_alias_cve() {
        let v: SearchIntent = serde_json::from_str("\"cve\"").unwrap();
        assert_eq!(v, SearchIntent::Security);
    }

    #[test]
    fn search_intent_canonical_roundtrip() {
        let intent = SearchIntent::Docs;
        let json = serde_json::to_string(&intent).unwrap();
        assert_eq!(json, "\"docs\"");
        let parsed: SearchIntent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, intent);
    }

    #[test]
    fn search_intent_invalid_string_fails() {
        let result = serde_json::from_str::<SearchIntent>("\"documentationn\"");
        assert!(result.is_err(), "misspelled alias should fail");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("valid values"),
            "error should list valid values: {err}"
        );
    }

    // --- Freshness alias deserialization ---

    #[test]
    fn freshness_alias_24h() {
        let v: Freshness = serde_json::from_str("\"24h\"").unwrap();
        assert_eq!(v, Freshness::Day);
    }

    #[test]
    fn freshness_alias_latest() {
        let v: Freshness = serde_json::from_str("\"latest\"").unwrap();
        assert_eq!(v, Freshness::Month);
    }

    #[test]
    fn freshness_canonical_roundtrip() {
        let freshness = Freshness::Month;
        let json = serde_json::to_string(&freshness).unwrap();
        assert_eq!(json, "\"month\"");
        let parsed: Freshness = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, freshness);
    }

    #[test]
    fn freshness_invalid_string_fails() {
        let result = serde_json::from_str::<Freshness>("\"banana\"");
        assert!(result.is_err(), "invalid freshness should fail");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("valid values"),
            "error should list valid values: {err}"
        );
    }

    #[test]
    fn freshness_alias_recent_maps_to_month() {
        let v: Freshness = serde_json::from_str("\"recent\"").unwrap();
        assert_eq!(v, Freshness::Month);
    }

    #[test]
    fn search_intent_recent_is_not_news() {
        // "recent" should NOT map to News; it mixes intent and freshness.
        let result = serde_json::from_str::<SearchIntent>("\"recent\"");
        assert!(
            result.is_err(),
            "\"recent\" should not be accepted as a search intent"
        );
    }

    #[test]
    fn date_range_accepts_leap_day() {
        let mut req = WebSearchRequest::new("test");
        req.date_range = Some(SearchDateRange::new("2024-02-29", "2024-03-01"));
        assert!(req.validate(512).is_ok());
    }

    #[test]
    fn date_range_rejects_invalid_calendar_date() {
        let mut req = WebSearchRequest::new("test");
        req.date_range = Some(SearchDateRange::new("2023-02-29", "2023-03-01"));
        assert!(req.validate(512).is_err());
    }

    #[test]
    fn date_range_rejects_reversed_range() {
        let mut req = WebSearchRequest::new("test");
        req.date_range = Some(SearchDateRange::new("2024-03-01", "2024-02-01"));
        assert!(req.validate(512).is_err());
    }

    #[test]
    fn date_range_mutually_exclusive_with_freshness() {
        let mut req = WebSearchRequest::new("test");
        req.freshness = Freshness::Week;
        req.date_range = Some(SearchDateRange::new("2024-01-01", "2024-01-31"));
        let err = req.validate(512).unwrap_err().to_string();
        assert!(err.contains("mutually exclusive"));
    }

    #[test]
    fn domain_normalization_lowercases_and_trims() {
        assert_eq!(normalize_domain("Example.COM").unwrap(), "example.com");
    }

    #[test]
    fn domain_rejects_scheme_port_path_wildcard() {
        assert!(normalize_domain("https://example.com").is_err());
        assert!(normalize_domain("example.com:443").is_err());
        assert!(normalize_domain("example.com/path").is_err());
        assert!(normalize_domain("*.example.com").is_err());
        assert!(normalize_domain("bad..example.com").is_err());
    }

    #[test]
    fn domain_overlap_rejected() {
        let mut req = WebSearchRequest::new("test");
        req.include_domains = vec!["example.com".to_string()];
        req.exclude_domains = vec!["EXAMPLE.com".to_string()];
        assert!(req.validate(512).is_err());
    }

    #[test]
    fn domain_matches_exact_and_subdomain_not_deceptive() {
        assert!(domain_matches_filter("example.com", "example.com"));
        assert!(domain_matches_filter("docs.example.com", "example.com"));
        assert!(!domain_matches_filter("notexample.com", "example.com"));
    }

    #[test]
    fn language_region_validation() {
        assert!(validate_language("en").is_ok());
        assert!(validate_language("en-US").is_ok());
        assert!(validate_language("x").is_err());
        assert!(validate_region("US").is_ok());
        assert!(validate_region("x").is_err());
        assert!(validate_region("US!").is_err());
    }

    #[test]
    fn legacy_request_deserializes_without_new_fields() {
        let req: WebSearchRequest = serde_json::from_str(r#"{"query":"rust"}"#).unwrap();
        assert!(req.date_range.is_none());
        assert!(req.include_domains.is_empty());
        assert!(req.excerpt_count.is_none());
        assert_eq!(req.effective_excerpt_count(), 0);
        assert!(req.validate(512).is_ok());
    }

    #[test]
    fn excerpt_count_defaults_to_zero_and_caps() {
        let req = WebSearchRequest::new("test");
        assert_eq!(req.effective_excerpt_count(), 0);
        let mut capped = WebSearchRequest::new("test");
        capped.excerpt_count = Some(99);
        assert_eq!(
            capped.effective_excerpt_count(),
            crate::core::source_card::MAX_EXCERPT_REQUEST_COUNT
        );
        assert!(capped.validate(512).is_err());
        let mut ok = WebSearchRequest::new("test");
        ok.excerpt_count = Some(2);
        assert!(ok.validate(512).is_ok());
        assert_eq!(ok.effective_excerpt_count(), 2);
    }
}
