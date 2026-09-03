use std::time::Duration;

use crate::core::query::{Freshness, SafeSearch, SearchDateRange, SearchIntent};

/// Provider-neutral repository scope for engines that support
/// server-side repository filtering (e.g. Firecrawl Developer `repos`).
///
/// Populated from `RepoSearchRequest::resolved_repo_locator()` earlier in
/// the planner so engines never reparse `owner/repo` from free text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RepoScope {
    /// Repository owner or namespace.
    pub owner: String,
    /// Repository name.
    pub repo: String,
}

impl RepoScope {
    /// Build a scope from owner/repo parts, trimming whitespace.
    /// Returns `None` when either part is empty.
    pub fn new(owner: &str, repo: &str) -> Option<Self> {
        let owner = owner.trim();
        let repo = repo.trim();
        if owner.is_empty() || repo.is_empty() {
            return None;
        }
        if owner.contains('/') || repo.contains('/') || repo.contains(' ') || owner.contains(' ') {
            return None;
        }
        Some(Self {
            owner: owner.to_string(),
            repo: repo.to_string(),
        })
    }

    /// Canonical `owner/repo` slug for upstream filters.
    pub fn slug(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }
}

#[derive(Clone, Debug)]
pub struct EngineSearchRequest {
    pub query: String,
    pub max_results: usize,
    pub timeout: Duration,
    pub intent: SearchIntent,
    pub safe_search: Option<SafeSearch>,
    pub freshness: Freshness,
    pub date_range: Option<SearchDateRange>,
    pub include_domains: Vec<String>,
    pub exclude_domains: Vec<String>,
    pub language: Option<String>,
    pub region: Option<String>,
    pub excerpt_count: usize,
    /// Optional provider-neutral repository scope. Engines that support
    /// native repo filtering use it; all others ignore it.
    pub repo_scope: Option<RepoScope>,
}

impl EngineSearchRequest {
    pub fn new(query: String, max_results: usize, timeout: Duration) -> Self {
        Self {
            query,
            max_results,
            timeout,
            intent: SearchIntent::default(),
            safe_search: None,
            freshness: Freshness::default(),
            date_range: None,
            include_domains: Vec::new(),
            exclude_domains: Vec::new(),
            language: None,
            region: None,
            excerpt_count: 0,
            repo_scope: None,
        }
    }

    pub fn simple(query: &str, max_results: usize, timeout: Duration) -> Self {
        Self::new(query.to_string(), max_results, timeout)
    }

    pub fn from_web_request(
        req: &crate::core::query::WebSearchRequest,
        query: String,
        max_results: usize,
        timeout: Duration,
    ) -> Self {
        Self {
            query,
            max_results,
            timeout,
            intent: req.intent,
            safe_search: req.safe_search,
            freshness: req.freshness,
            date_range: req.date_range.clone(),
            include_domains: req.include_domains.clone(),
            exclude_domains: req.exclude_domains.clone(),
            language: req.language.clone(),
            region: req.region.clone(),
            excerpt_count: req
                .excerpt_count
                .unwrap_or(0)
                .min(crate::core::source_card::MAX_EXCERPT_REQUEST_COUNT),
            repo_scope: None,
        }
    }

    /// Whether any provider should return additional excerpts.
    pub fn wants_excerpts(&self) -> bool {
        self.excerpt_count > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_constructor_applies_defaults() {
        let req = EngineSearchRequest::simple("rust", 10, Duration::from_secs(5));
        assert_eq!(req.query, "rust");
        assert_eq!(req.max_results, 10);
        assert_eq!(req.intent, SearchIntent::Web);
        assert_eq!(req.safe_search, None);
        assert_eq!(req.freshness, Freshness::Any);
        assert!(req.date_range.is_none());
        assert!(req.include_domains.is_empty());
        assert!(req.exclude_domains.is_empty());
        assert!(req.language.is_none());
        assert!(req.region.is_none());
        assert_eq!(req.excerpt_count, 0);
        assert!(!req.wants_excerpts());
        assert!(req.repo_scope.is_none());
    }

    #[test]
    fn from_web_request_copies_constraints() {
        let mut web = crate::core::query::WebSearchRequest::new("rust");
        web.intent = SearchIntent::News;
        web.safe_search = Some(SafeSearch::Strict);
        web.freshness = Freshness::Week;
        web.language = Some("en".to_string());
        web.region = Some("US".to_string());
        web.include_domains = vec!["example.com".to_string()];
        let engine = EngineSearchRequest::from_web_request(
            &web,
            "rust".to_string(),
            5,
            Duration::from_secs(3),
        );
        assert_eq!(engine.intent, SearchIntent::News);
        assert_eq!(engine.safe_search, Some(SafeSearch::Strict));
        assert_eq!(engine.freshness, Freshness::Week);
        assert_eq!(engine.language.as_deref(), Some("en"));
        assert_eq!(engine.region.as_deref(), Some("US"));
        assert_eq!(engine.include_domains, vec!["example.com".to_string()]);
        assert!(engine.repo_scope.is_none());
    }

    #[test]
    fn repo_scope_validates_parts() {
        let scope = RepoScope::new("tokio-rs", "axum").expect("valid");
        assert_eq!(scope.slug(), "tokio-rs/axum");
        assert!(RepoScope::new("", "axum").is_none());
        assert!(RepoScope::new("tokio-rs", "").is_none());
        assert!(RepoScope::new("a/b", "c").is_none());
        assert!(RepoScope::new("a", "b c").is_none());
    }
}
