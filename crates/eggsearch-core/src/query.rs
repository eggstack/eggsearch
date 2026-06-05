//! Query types accepted by providers and MCP tools.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SafeSearch {
    Off,
    #[default]
    Moderate,
    Strict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    Day,
    Week,
    Month,
    Year,
    Any,
}

impl Default for Freshness {
    fn default() -> Self {
        Self::Any
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SearchCategory {
    General,
    Documentation,
    PackageRegistry,
    Reference,
    News,
}

impl SearchCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Documentation => "documentation",
            Self::PackageRegistry => "package_registry",
            Self::Reference => "reference",
            Self::News => "news",
        }
    }
}

/// A normalized search query shared across providers and tools.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchQuery {
    pub query: String,
    pub max_results: usize,
    pub language: Option<String>,
    pub region: Option<String>,
    pub safe_search: SafeSearch,
    pub freshness: Option<Freshness>,
    pub include_domains: Vec<String>,
    pub exclude_domains: Vec<String>,
    pub categories: Vec<SearchCategory>,
    /// Optional override of provider IDs to use; empty means "all enabled".
    pub providers: Vec<String>,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            query: String::new(),
            max_results: 8,
            language: None,
            region: None,
            safe_search: SafeSearch::Moderate,
            freshness: Some(Freshness::Any),
            include_domains: Vec::new(),
            exclude_domains: Vec::new(),
            categories: vec![SearchCategory::General],
            providers: Vec::new(),
        }
    }
}

impl SearchQuery {
    pub fn new<Q: Into<String>>(query: Q) -> Self {
        Self {
            query: query.into(),
            ..Self::default()
        }
    }

    pub fn with_max_results(mut self, n: usize) -> Self {
        self.max_results = n;
        self
    }

    pub fn with_providers<I, S>(mut self, providers: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.providers = providers.into_iter().map(Into::into).collect();
        self
    }

    /// Validate the query, returning an error string if invalid.
    pub fn validate(&self) -> Result<(), String> {
        if self.query.trim().is_empty() {
            return Err("query must not be empty".to_string());
        }
        if self.max_results == 0 {
            return Err("max_results must be > 0".to_string());
        }
        if self.max_results > 100 {
            return Err("max_results must be <= 100".to_string());
        }
        Ok(())
    }
}
