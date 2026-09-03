use std::time::Duration;

use crate::core::query::{Freshness, SafeSearch, SearchDateRange, SearchIntent};

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
        }
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
    }
}
