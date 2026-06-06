//! Server state: shared state passed to every tool call.

use std::sync::Arc;
use std::time::Duration;

use crate::core::config::AppConfig;
use crate::meta::MetadataSearchAdapter;

/// Shared state for the MCP server. Cheap to clone (all fields are Arc).
#[derive(Clone)]
pub struct ServerState {
    pub config: Arc<AppConfig>,
    pub adapter: Arc<MetadataSearchAdapter>,
}

impl std::fmt::Debug for ServerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerState")
            .field("mode", &self.config.search.mode)
            .field("providers", &self.adapter.provider_ids())
            .finish()
    }
}

impl ServerState {
    /// Build a new server state.
    ///
    /// The adapter is constructed from the effective enabled provider
    /// list, with a hard global timeout equal to the config's
    /// `timeout_ms`. The MCP server starts and runs without any index
    /// directory, database, or persistent state.
    pub fn build(config: AppConfig) -> anyhow::Result<Self> {
        let config = Arc::new(config);

        let enabled: Vec<String> = config
            .search
            .providers
            .iter()
            .filter_map(|(id, on)| if *on { Some(id.clone()) } else { None })
            .collect();

        let global_timeout = Duration::from_millis(config.search.timeout_ms);
        let adapter = MetadataSearchAdapter::new(enabled, global_timeout)?;

        Ok(Self {
            config,
            adapter: Arc::new(adapter),
        })
    }

    /// Build a server state from a pre-constructed adapter. Intended for
    /// tests and for callers that want to wire custom upstream engines
    /// (e.g. mocks).
    pub fn with_adapter(
        config: AppConfig,
        adapter: std::sync::Arc<MetadataSearchAdapter>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            adapter,
        }
    }
}
