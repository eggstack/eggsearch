//! Search provider trait and shared context.

use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::CoreResult;
use crate::query::SearchQuery;
use crate::result::{SearchResult, SearchWarning};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NetworkMode {
    /// Live network access is permitted.
    Live,
    /// Network access is forbidden; providers must rely on caches or local data.
    LocalOnly,
    /// No search at all; tools should be disabled.
    Off,
}

#[derive(Clone, Debug)]
pub struct SearchContext {
    pub request_id: Uuid,
    pub timeout: Duration,
    pub user_agent: String,
    pub network_mode: NetworkMode,
}

impl SearchContext {
    pub fn live() -> Self {
        Self {
            request_id: Uuid::new_v4(),
            timeout: Duration::from_secs(8),
            user_agent: format!("eggsearch/{}", env!("CARGO_PKG_VERSION")),
            network_mode: NetworkMode::Live,
        }
    }

    pub fn local_only() -> Self {
        Self {
            request_id: Uuid::new_v4(),
            timeout: Duration::from_secs(8),
            user_agent: format!("eggsearch/{}", env!("CARGO_PKG_VERSION")),
            network_mode: NetworkMode::LocalOnly,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchProviderResponse {
    pub provider_id: String,
    pub query: SearchQuery,
    pub results: Vec<SearchResult>,
    pub warnings: Vec<SearchWarning>,
    pub raw_response_hash: Option<String>,
    pub elapsed_ms: u64,
}

impl SearchProviderResponse {
    pub fn empty(provider_id: impl Into<String>, query: SearchQuery) -> Self {
        Self {
            provider_id: provider_id.into(),
            query,
            results: Vec::new(),
            warnings: Vec::new(),
            raw_response_hash: None,
            elapsed_ms: 0,
        }
    }
}

#[async_trait]
pub trait SearchProvider: Send + Sync {
    /// Stable identifier for this provider, used in config and source cards.
    fn id(&self) -> &'static str;

    /// Returns the categories this provider can answer.
    fn categories(&self) -> &[crate::query::SearchCategory] {
        // default: general provider
        &[crate::query::SearchCategory::General]
    }

    /// Whether this provider requires network access.
    fn is_online(&self) -> bool {
        true
    }

    /// Execute a search against this provider.
    async fn search(
        &self,
        query: SearchQuery,
        ctx: SearchContext,
    ) -> CoreResult<SearchProviderResponse>;
}
