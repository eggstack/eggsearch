//! Server state: shared state passed to every tool call.

use std::path::PathBuf;
use std::sync::Arc;

use eggsearch_core::config::AppConfig;
use eggsearch_fetch::{ArtifactStore, FetchCache, ReqwestFetchProvider, RobotsCache};
use eggsearch_local::LocalCorpus;
use eggsearch_meta::ProviderRegistry;
use tracing::warn;

/// Shared state for the MCP server. Cheap to clone (all fields are Arc).
#[derive(Clone)]
pub struct ServerState {
    pub config: Arc<AppConfig>,
    pub providers: Arc<ProviderRegistry>,
    pub fetch: Arc<ReqwestFetchProvider>,
    pub corpus: Arc<LocalCorpus>,
    pub cache: Arc<FetchCache>,
    pub artifacts: Arc<ArtifactStore>,
    pub robots: Arc<RobotsCache>,
}

impl std::fmt::Debug for ServerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerState")
            .field("mode", &self.config.search.mode)
            .field("providers", &self.providers.ids())
            .finish()
    }
}

impl ServerState {
    /// Build a new server state, opening the local index and creating
    /// any required directories.
    pub fn build(config: AppConfig) -> anyhow::Result<Self> {
        let config = Arc::new(config);
        let providers = Arc::new(ProviderRegistry::with_default_providers());

        std::fs::create_dir_all(&config.search.cache_dir).ok();
        std::fs::create_dir_all(&config.search.artifact_dir).ok();
        std::fs::create_dir_all(&config.search.local.index_dir).ok();

        let artifacts = Arc::new(ArtifactStore::new(&config.search.artifact_dir)?);
        let cache = Arc::new(FetchCache::default());
        let robots_client = reqwest::Client::builder()
            .user_agent(config.search.live.user_agent.clone())
            .build()?;
        let robots = Arc::new(RobotsCache::new(robots_client));
        let fetch = Arc::new(ReqwestFetchProvider::new(
            artifacts.clone(),
            cache.clone(),
            robots.clone(),
        )?);

        let corpus = Arc::new(LocalCorpus::open_or_create(
            &config.search.local.index_dir,
        )?);

        Ok(Self {
            config,
            providers,
            fetch,
            corpus,
            cache,
            artifacts,
            robots,
        })
    }

    /// Build with a custom index dir (used for tests).
    pub fn build_at(config: AppConfig, index_dir: PathBuf) -> anyhow::Result<Self> {
        let mut cfg = config;
        cfg.search.local.index_dir = index_dir;
        Self::build(cfg)
    }
}

#[allow(dead_code)]
fn _silence_warn(w: &str) {
    if w.is_empty() {
        warn!("");
    }
}
