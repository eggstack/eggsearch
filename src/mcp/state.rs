//! Server state: shared state passed to every tool call.

use std::sync::Arc;
use std::time::Duration;

use tracing;

use crate::core::config::AppConfig;
use crate::fetch::FetchClient;
use crate::meta::MetadataSearchAdapter;

/// Shared state for the MCP server. Cheap to clone (all fields are Arc).
#[derive(Clone)]
pub struct ServerState {
    pub config: Arc<AppConfig>,
    pub adapter: Arc<MetadataSearchAdapter>,
    /// Shared HTTP fetch client. `None` when `[fetch].enabled = false`
    /// or when built via [`ServerState::with_adapter`] (tests, custom
    /// adapters). The `fetch_allowed` policy check upstream of every
    /// fetch call should make the `None` case unreachable, but the
    /// type allows the disabled state for clean error reporting.
    pub fetch_client: Option<Arc<FetchClient>>,
}

impl std::fmt::Debug for ServerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerState")
            .field("mode", &self.config.search.mode)
            .field("providers", &self.adapter.provider_ids())
            .field("fetch_enabled", &self.config.fetch.enabled)
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
        config.validate()?;

        let config = Arc::new(config);

        let enabled: Vec<String> = config
            .search
            .providers
            .iter()
            .filter_map(|(id, on)| if *on { Some(id.clone()) } else { None })
            .collect();

        let global_timeout = Duration::from_millis(config.search.timeout_ms);
        let user_agent = Some(config.fetch.user_agent.clone());

        let searxng_requested = enabled.iter().any(|id| id == "searxng");
        let searxng_base_url = config.search.searxng.base_url.clone();
        let searxng_base_url_is_empty = searxng_base_url
            .as_deref()
            .map(str::is_empty)
            .unwrap_or(true);
        if searxng_requested
            && (config.search.searxng.enabled
                || !searxng_base_url_is_empty)
        {
            if !config.search.searxng.enabled {
                tracing::warn!(
                    "[search].providers.searxng = true but [search].searxng.enabled = false; \
                     the searxng provider will be skipped"
                );
            } else if searxng_base_url_is_empty {
                tracing::warn!(
                    "[search].providers.searxng = true but [search].searxng.base_url is empty; \
                     the searxng provider will be skipped"
                );
            }
        }
        let searxng_base_url = if config.search.searxng.enabled {
            searxng_base_url
        } else {
            None
        };

        let adapter = MetadataSearchAdapter::new(
            enabled,
            global_timeout,
            user_agent,
            searxng_base_url,
            config.search.sanitize_output,
        )?;

        let misconfigured = config.misconfigured_default_providers();
        for id in &misconfigured {
            tracing::warn!(
                provider_id = %id,
                "provider listed in [search].default_providers is not enabled; \
                 it will be silently skipped. Enable it in [search].providers or \
                 remove it from default_providers."
            );
        }

        if config.search.live.user_agent.is_some() {
            tracing::warn!(
                "[search].live.user_agent is reserved for future use and is not yet applied. \
                 The vendored HTML engines use a hard-coded browser-like user agent."
            );
        }
        if config
            .search
            .live
            .respect_robots_txt
            .is_some_and(|v| v)
        {
            tracing::warn!(
                "[search].live.respect_robots_txt is reserved for future use and is not yet applied. \
                 web_fetch does not consult robots.txt in the current build."
            );
        }

        let fetch_client = if config.fetch.enabled {
            let limits = config.fetch_limits();
            let ua = config.fetch_user_agent();
            match FetchClient::new(limits, ua, config.fetch.sanitize_output) {
                Ok(c) => Some(Arc::new(c)),
                Err(e) => {
                    tracing::warn!(error = %e, "failed to build shared fetch client; web_fetch will fail at call time");
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            config,
            adapter: Arc::new(adapter),
            fetch_client,
        })
    }

    /// Build a server state from a pre-constructed adapter. Intended for
    /// tests and for callers that want to wire custom upstream engines
    /// (e.g. mocks). Builds a `FetchClient` from the config when
    /// `[fetch].enabled = true`; otherwise `fetch_client` is `None`.
    ///
    /// The pre-constructed adapter must already have its
    /// `sanitize_output` flag set (the production default is `true`).
    /// The `FetchClient` honors `config.fetch.sanitize_output`.
    pub fn with_adapter(config: AppConfig, adapter: std::sync::Arc<MetadataSearchAdapter>) -> Self {
        let config = Arc::new(config);
        let fetch_client = if config.fetch.enabled {
            let limits = config.fetch_limits();
            let ua = config.fetch_user_agent();
            FetchClient::new(limits, ua, config.fetch.sanitize_output)
                .ok()
                .map(Arc::new)
        } else {
            None
        };
        Self {
            config,
            adapter,
            fetch_client,
        }
    }

    /// Returns the shared fetch client, if fetch is enabled. Callers
    /// should already have run the `fetch_allowed` policy check; this
    /// helper exists for clean error reporting when the client is
    /// unexpectedly absent.
    pub fn fetch_client(&self) -> Option<Arc<FetchClient>> {
        self.fetch_client.clone()
    }
}
