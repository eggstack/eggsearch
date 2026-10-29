//! Server state: shared state passed to every tool call.

use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Deserialize;
use tracing;

use crate::core::config::AppConfig;
#[cfg(feature = "browser")]
use crate::fetch::browser::ProfileManager;
#[cfg(feature = "browser")]
use crate::fetch::browser::{discover_browser, BrowserLifecycle};
use crate::fetch::cache::FetchCache;
use crate::fetch::origin::{OriginController, OriginPolicy};
use crate::fetch::FetchClient;
use crate::meta::engines::kev::KevClient;
use crate::meta::local_backend::LocalWorkspaceBackend;
use crate::meta::local_inventory::{discover_local_repos, LocalRepoIdentity};
use crate::meta::MetadataSearchAdapter;

/// TTL for the cached local workspace inventory. The cache is
/// intentionally short because workspace roots can change frequently
/// during agent sessions, and the discovery cost is mostly bounded by
/// filesystem scans of configured roots.
const LOCAL_INVENTORY_CACHE_TTL: Duration = Duration::from_secs(30);

/// Cached snapshot of local workspace repository discovery.
pub struct LocalInventoryCache {
    pub inventory: Vec<LocalRepoIdentity>,
    pub fetched_at: Instant,
}

/// Host-injected scope carried only on a CodeGG-managed local MCP process.
/// The request field is hidden from the generated tool schema; this token
/// authenticates the private transport instead of treating that omission as
/// an authorization boundary.
#[derive(Clone, Deserialize, schemars::JsonSchema)]
pub struct HostLocalScope {
    pub root: Option<PathBuf>,
    pub token: String,
}

impl std::fmt::Debug for HostLocalScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostLocalScope")
            .field("root", &self.root)
            .field("token", &"<redacted>")
            .finish()
    }
}

const HOST_LOCAL_SCOPE_TOKEN_ENV: &str = "CODEGG_EGGSEARCH_HOST_SCOPE_TOKEN";
const HOST_LOCAL_BACKEND_CACHE_CAPACITY: usize = 8;

/// Request-local view over a local backend and its root-specific inventory
/// cache. Standalone requests reuse the startup-configured view; authenticated
/// host-scoped requests use an isolated bounded-cache entry.
#[derive(Clone)]
pub struct LocalWorkspaceView {
    backend: Option<Arc<LocalWorkspaceBackend>>,
    inventory_cache: Arc<Mutex<Option<LocalInventoryCache>>>,
}

impl LocalWorkspaceView {
    pub fn backend(&self) -> Option<&LocalWorkspaceBackend> {
        self.backend.as_deref()
    }

    pub fn is_enabled(&self) -> bool {
        self.backend()
            .is_some_and(LocalWorkspaceBackend::is_enabled)
    }

    pub async fn inventory(&self) -> Vec<LocalRepoIdentity> {
        {
            if let Ok(cache) = self.inventory_cache.lock() {
                if let Some(snapshot) = cache.as_ref() {
                    if snapshot.fetched_at.elapsed() < LOCAL_INVENTORY_CACHE_TTL {
                        return snapshot.inventory.clone();
                    }
                }
            }
        }

        let backend = match self.backend.as_deref() {
            Some(backend) if backend.is_enabled() => backend,
            _ => return Vec::new(),
        };
        let roots = backend.roots();
        let mut local_config = backend.config().clone();
        local_config.enabled = true;
        let roots_for_walk: Vec<PathBuf> = roots.iter().map(|(_, path)| path.clone()).collect();
        let inventory = match tokio::task::spawn_blocking(move || {
            let mut config = local_config;
            config.roots = roots_for_walk;
            discover_local_repos(&config, 2)
        })
        .await
        {
            Ok(inventory) => inventory,
            Err(error) => {
                tracing::error!(%error, "local repository inventory task failed");
                return Vec::new();
            }
        };
        if let Ok(mut cache) = self.inventory_cache.lock() {
            *cache = Some(LocalInventoryCache {
                inventory: inventory.clone(),
                fetched_at: Instant::now(),
            });
        }
        inventory
    }
}

#[derive(Default)]
struct HostScopedLocalBackendCache {
    entries: Option<lru::LruCache<String, Arc<LocalWorkspaceView>>>,
}

impl HostScopedLocalBackendCache {
    fn get_or_insert(
        &mut self,
        key: String,
        config: crate::core::local::LocalConfig,
    ) -> Result<Arc<LocalWorkspaceView>, String> {
        let entries = self.entries.get_or_insert_with(|| {
            lru::LruCache::new(
                NonZeroUsize::new(HOST_LOCAL_BACKEND_CACHE_CAPACITY)
                    .expect("non-zero local backend cache capacity"),
            )
        });
        if let Some(view) = entries.get(&key) {
            return Ok(view.clone());
        }
        let backend = LocalWorkspaceBackend::new(config)
            .map_err(|error| format!("host-scoped local workspace is unavailable: {error}"))?;
        let view = Arc::new(LocalWorkspaceView {
            backend: Some(Arc::new(backend)),
            inventory_cache: Arc::new(Mutex::new(None)),
        });
        entries.put(key, view.clone());
        Ok(view)
    }
}

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
    /// Per-origin concurrency control and circuit breaker.
    pub origin_controller: Option<Arc<OriginController>>,
    /// In-memory fetch cache for raw responses and derived documents.
    pub fetch_cache: Option<Arc<FetchCache>>,
    /// CISA KEV catalog client with TTL cache.
    pub kev_client: Arc<KevClient>,
    /// Local workspace search backend. `None` when `[local].enabled = false`.
    pub local_backend: Option<Arc<LocalWorkspaceBackend>>,
    /// TTL-cached snapshot of local repository discovery. Avoids
    /// re-walking configured roots on every `repo_fetch` / `repo_map`
    /// call. Public so tests can construct `ServerState` instances
    /// with custom local-backend configurations.
    pub local_inventory_cache: Arc<Mutex<Option<LocalInventoryCache>>>,
    /// Bounded cache of host-authorized one-root local backend views. This
    /// contains indexes only; host authorization is checked for every call.
    host_scoped_local_backends: Arc<Mutex<HostScopedLocalBackendCache>>,
    /// Browser profile manager. `None` when browser feature is not
    /// compiled in or when persistent profiles are disabled.
    #[cfg(feature = "browser")]
    pub profile_manager: Option<Arc<ProfileManager>>,
    /// Shared browser lifecycle. `None` when browser feature is not
    /// compiled in or when browser rendering is disabled. Holds a
    /// warm browser process for reuse across requests.
    #[cfg(feature = "browser")]
    pub browser_lifecycle: Option<Arc<BrowserLifecycle>>,
    /// Global cap on concurrent headless renders, sized by
    /// `[fetch.browser].global_concurrency`. `None` when the browser
    /// feature is not compiled in or rendering is disabled.
    #[cfg(feature = "browser")]
    pub browser_global_semaphore: Option<Arc<tokio::sync::Semaphore>>,
    /// Cached browser discovery result computed at startup. Stored to
    /// avoid re-probing the filesystem on every `provider_status` call.
    #[cfg(feature = "browser")]
    pub browser_discovery_state: crate::fetch::browser::types::BrowserDiscoveryState,
}

impl std::fmt::Debug for ServerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("ServerState");
        d.field("mode", &self.config.search.mode)
            .field("providers", &self.adapter.provider_ids())
            .field("fetch_enabled", &self.config.fetch.enabled)
            .field("origin_controller", &self.origin_controller.is_some())
            .field("fetch_cache", &self.fetch_cache.is_some())
            .field("kev_client", &"<KevClient>")
            .field("local_enabled", &self.local_backend.is_some());
        #[cfg(feature = "browser")]
        {
            d.field("profile_manager", &self.profile_manager.is_some());
            d.field("browser_lifecycle", &self.browser_lifecycle.is_some());
            d.field("browser_discovery_state", &self.browser_discovery_state);
        }
        d.finish()
    }
}

impl ServerState {
    /// Effective per-origin headless-render cap: the tighter of
    /// `[fetch].origin_browser_concurrency` and
    /// `[fetch.browser].per_origin_concurrency`. Both default to 1, and both
    /// describe the same limit, so the smaller wins rather than silently
    /// ignoring either knob.
    fn effective_browser_concurrency(config: &AppConfig) -> usize {
        config
            .fetch
            .origin_browser_concurrency
            .min(config.fetch.browser.per_origin_concurrency)
    }

    /// Build a new server state.
    ///
    /// The adapter is constructed from the effective enabled provider
    /// list, with a hard global timeout equal to the config's
    /// `timeout_ms`. The MCP server starts and runs without any index
    /// directory, database, or persistent state.
    pub fn build(config: AppConfig) -> anyhow::Result<Self> {
        config.validate()?;

        let config = Arc::new(config);

        let enabled = config.effective_provider_ids();

        let global_timeout = Duration::from_millis(config.search.timeout_ms);
        let user_agent = Some(config.fetch.user_agent.clone());

        let searxng_requested = enabled.iter().any(|id| id == "searxng");
        let searxng_base_url = config.search.searxng.base_url.clone();
        let searxng_base_url_is_empty = searxng_base_url
            .as_deref()
            .map(str::is_empty)
            .unwrap_or(true);
        if searxng_requested && (config.search.searxng.enabled || !searxng_base_url_is_empty) {
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

        let adapter = MetadataSearchAdapter::new_with_egress(
            enabled,
            global_timeout,
            user_agent,
            searxng_base_url,
            config.search.sanitize_output,
            config.search.default_providers.clone(),
            &config.search.api,
            config.search.multiquery_concurrency,
            config.search.multiquery_provider_concurrency,
            &config.egress,
        )?;

        let misconfigured = config.misconfigured_default_providers();
        for id in &misconfigured {
            tracing::warn!(
                provider_id = %id,
                "provider listed in [search].default_providers is not enabled; \
                 it will be skipped. Enable it in [search].providers, configure \
                 a usable [search].api entry, or remove it from default_providers."
            );
        }

        if config.search.live.user_agent.is_some() {
            tracing::warn!(
                "[search].live.user_agent is reserved for future use and is not yet applied. \
                 The vendored HTML engines use a hard-coded browser-like user agent."
            );
        }
        if config.search.live.respect_robots_txt.is_some_and(|v| v) {
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

        let origin_controller = if config.fetch.enabled {
            let policy = OriginPolicy {
                http_concurrency: config.fetch.origin_http_concurrency,
                browser_concurrency: Self::effective_browser_concurrency(&config),
                retry_max_attempts: config.fetch.retry_max_attempts,
                retry_base_delay_ms: config.fetch.retry_base_delay_ms,
                retry_max_delay_ms: config.fetch.retry_max_delay_ms,
                circuit_failure_threshold: config.fetch.origin_circuit_failure_threshold,
                circuit_duration_ms: config.fetch.origin_circuit_duration_ms,
            };
            Some(Arc::new(OriginController::new(policy, 1024)))
        } else {
            None
        };

        let fetch_cache = if config.fetch.enabled && config.fetch.cache.enabled {
            Some(Arc::new(FetchCache::new(
                config.fetch.cache.memory_max_entries,
                config.fetch.cache.derived_max_entries,
                config.fetch.cache.memory_max_bytes,
                config.fetch.cache.derived_max_bytes,
            )))
        } else {
            None
        };

        let kev_client = Arc::new(KevClient::new(
            crate::meta::engines::build_http_client(None).expect("kev HTTP client builds"),
        ));

        #[cfg(feature = "browser")]
        let profile_manager = {
            let bp = &config.fetch.browser.persistent_profiles;
            if bp.enabled {
                match ProfileManager::new(
                    bp.profiles_dir.as_deref(),
                    true,
                    bp.allowed_profiles.clone(),
                ) {
                    Ok(mgr) => {
                        tracing::info!(
                            profiles_dir = %mgr.root_dir().display(),
                            "persistent browser profiles enabled"
                        );
                        Some(Arc::new(mgr))
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to initialize browser profile manager; persistent profiles will be unavailable");
                        None
                    }
                }
            } else {
                None
            }
        };

        #[cfg(feature = "browser")]
        let browser_discovery_state = discover_browser(config.fetch.browser.executable.as_deref());
        #[cfg(feature = "browser")]
        let browser_lifecycle = if config.fetch.browser.enabled {
            let discovery = browser_discovery_state.discovery().cloned();
            let render_policy: crate::fetch::browser::RenderPolicy = serde_json::from_value(
                serde_json::Value::String(config.fetch.browser.policy.clone()),
            )
            .unwrap_or_default();
            let browser_config = crate::fetch::browser::BrowserConfig {
                enabled: config.fetch.browser.enabled,
                policy: render_policy,
                executable: config.fetch.browser.executable.clone(),
                startup_timeout_ms: config.fetch.browser.startup_timeout_ms,
                navigation_timeout_ms: config.fetch.browser.navigation_timeout_ms,
                post_load_wait_ms: config.fetch.browser.post_load_wait_ms,
                verification_wait_ms: config.fetch.browser.verification_wait_ms,
                max_requests: config.fetch.browser.max_requests,
                max_dom_bytes: config.fetch.browser.max_dom_bytes,
                global_concurrency: config.fetch.browser.global_concurrency,
                per_origin_concurrency: config.fetch.browser.per_origin_concurrency,
                block_media: config.fetch.browser.block_media,
                persistent_profiles: config.fetch.browser.persistent_profiles.clone(),
            };
            Some(Arc::new(BrowserLifecycle::new(discovery, browser_config)))
        } else {
            None
        };
        #[cfg(feature = "browser")]
        let browser_global_semaphore = if config.fetch.browser.enabled {
            Some(Arc::new(tokio::sync::Semaphore::new(
                config.fetch.browser.global_concurrency.max(1),
            )))
        } else {
            None
        };

        // Build local workspace backend
        let local_backend = match LocalWorkspaceBackend::new(config.local.clone()) {
            Ok(backend) if backend.is_enabled() => {
                tracing::info!(
                    roots = ?config.local.roots.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
                    "local workspace search enabled"
                );
                Some(Arc::new(backend))
            }
            Ok(_) => None,
            Err(e) => {
                tracing::warn!(error = %e, "failed to build local workspace backend; local search will be unavailable");
                None
            }
        };

        Ok(Self {
            config,
            adapter: Arc::new(adapter),
            fetch_client,
            origin_controller,
            fetch_cache,
            kev_client,
            local_backend,
            local_inventory_cache: Arc::new(Mutex::new(None)),
            host_scoped_local_backends: Arc::new(
                Mutex::new(HostScopedLocalBackendCache::default()),
            ),
            #[cfg(feature = "browser")]
            profile_manager,
            #[cfg(feature = "browser")]
            browser_lifecycle,
            #[cfg(feature = "browser")]
            browser_global_semaphore,
            #[cfg(feature = "browser")]
            browser_discovery_state,
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
        let origin_controller = if config.fetch.enabled {
            let policy = OriginPolicy {
                http_concurrency: config.fetch.origin_http_concurrency,
                browser_concurrency: Self::effective_browser_concurrency(&config),
                retry_max_attempts: config.fetch.retry_max_attempts,
                retry_base_delay_ms: config.fetch.retry_base_delay_ms,
                retry_max_delay_ms: config.fetch.retry_max_delay_ms,
                circuit_failure_threshold: config.fetch.origin_circuit_failure_threshold,
                circuit_duration_ms: config.fetch.origin_circuit_duration_ms,
            };
            Some(Arc::new(OriginController::new(policy, 1024)))
        } else {
            None
        };
        let fetch_cache = if config.fetch.enabled && config.fetch.cache.enabled {
            Some(Arc::new(FetchCache::new(
                config.fetch.cache.memory_max_entries,
                config.fetch.cache.derived_max_entries,
                config.fetch.cache.memory_max_bytes,
                config.fetch.cache.derived_max_bytes,
            )))
        } else {
            None
        };
        let kev_client = Arc::new(KevClient::new(
            crate::meta::engines::build_http_client(None).expect("kev HTTP client builds"),
        ));
        Self {
            config,
            adapter,
            fetch_client,
            origin_controller,
            fetch_cache,
            kev_client,
            local_backend: None,
            local_inventory_cache: Arc::new(Mutex::new(None)),
            host_scoped_local_backends: Arc::new(
                Mutex::new(HostScopedLocalBackendCache::default()),
            ),
            #[cfg(feature = "browser")]
            profile_manager: None,
            #[cfg(feature = "browser")]
            browser_lifecycle: None,
            #[cfg(feature = "browser")]
            browser_global_semaphore: None,
            #[cfg(feature = "browser")]
            browser_discovery_state: crate::fetch::browser::types::BrowserDiscoveryState::NotFound,
        }
    }

    /// Returns the shared fetch client, if fetch is enabled. Callers
    /// should already have run the `fetch_allowed` policy check; this
    /// helper exists for clean error reporting when the client is
    /// unexpectedly absent.
    pub fn fetch_client(&self) -> Option<Arc<FetchClient>> {
        self.fetch_client.clone()
    }

    /// Returns the shared browser lifecycle, if browser rendering is
    /// enabled and a Chrome/Chromium executable was discovered. The
    /// lifecycle holds a warm browser process for reuse across requests.
    #[cfg(feature = "browser")]
    pub fn browser_lifecycle(&self) -> Option<Arc<BrowserLifecycle>> {
        self.browser_lifecycle.clone()
    }

    /// Returns the semaphore capping concurrent headless renders
    /// process-wide, sized by `[fetch.browser].global_concurrency`.
    #[cfg(feature = "browser")]
    pub fn browser_global_semaphore(&self) -> Option<Arc<tokio::sync::Semaphore>> {
        self.browser_global_semaphore.clone()
    }

    /// Returns the cached local repository inventory, re-running the
    /// discovery walk when the cached snapshot is older than
    /// `LOCAL_INVENTORY_CACHE_TTL`. Returns an empty vector when no
    /// local backend is configured. Discovery is performed on a blocking
    /// task to keep the async runtime responsive; the cache keeps the
    /// cost bounded across repeated tool calls.
    pub async fn local_inventory(&self) -> Vec<LocalRepoIdentity> {
        match self.local_workspace_view(None) {
            Ok(view) => view.inventory().await,
            Err(_) => Vec::new(),
        }
    }

    /// Resolve an authenticated request-local local-workspace view. `None`
    /// preserves standalone Eggsearch's startup-configured roots. A present
    /// but invalid host scope fails closed and never falls back to those roots.
    pub fn local_workspace_view(
        &self,
        host_scope: Option<&serde_json::Value>,
    ) -> Result<Arc<LocalWorkspaceView>, String> {
        let Some(host_scope_value) = host_scope else {
            return Ok(Arc::new(LocalWorkspaceView {
                backend: self.local_backend.clone(),
                inventory_cache: self.local_inventory_cache.clone(),
            }));
        };

        let host_scope: HostLocalScope = serde_json::from_value(host_scope_value.clone())
            .map_err(|_| "host-scoped local retrieval metadata is malformed".to_string())?;

        let expected = std::env::var(HOST_LOCAL_SCOPE_TOKEN_ENV).map_err(|_| {
            "host-scoped local retrieval is unavailable on this Eggsearch process".to_string()
        })?;
        if !constant_time_eq(expected.as_bytes(), host_scope.token.as_bytes()) {
            return Err("host-scoped local retrieval request authentication failed".to_string());
        }
        let Some(root) = host_scope.root.as_ref() else {
            return Ok(Arc::new(LocalWorkspaceView {
                backend: None,
                inventory_cache: Arc::new(Mutex::new(None)),
            }));
        };
        if !self.config.local.enabled {
            return Ok(Arc::new(LocalWorkspaceView {
                backend: None,
                inventory_cache: Arc::new(Mutex::new(None)),
            }));
        }
        let canonical_root = root
            .canonicalize()
            .map_err(|error| format!("host-scoped local workspace root is unavailable: {error}"))?;
        if !canonical_root.is_dir() {
            return Err("host-scoped local workspace root is not a directory".to_string());
        }
        let mut config = self.config.local.clone();
        config.enabled = true;
        config.roots = vec![canonical_root];
        let fingerprint = serde_json::to_vec(&config)
            .map_err(|error| format!("cannot fingerprint local workspace config: {error}"))?;
        use sha2::{Digest, Sha256};
        let key = format!("{:x}", Sha256::digest(fingerprint));
        self.host_scoped_local_backends
            .lock()
            .map_err(|_| "host-scoped local backend cache is unavailable".to_string())?
            .get_or_insert(key, config)
    }

    /// Invalidate the cached local inventory. Useful for tests and for
    /// future endpoints that mutate workspace roots.
    pub fn invalidate_local_inventory_cache(&self) {
        if let Ok(mut cache) = self.local_inventory_cache.lock() {
            *cache = None;
        }
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let max_len = left.len().max(right.len());
    let mut difference = left.len() ^ right.len();
    for index in 0..max_len {
        difference |= usize::from(
            left.get(index).copied().unwrap_or_default()
                ^ right.get(index).copied().unwrap_or_default(),
        );
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::ApiProviderConfig;

    #[test]
    fn build_includes_configured_api_provider() {
        let env = "EGGSEARCH_TEST_STATE_BRAVE_API_KEY";
        std::env::set_var(env, "test_key");
        let mut config = AppConfig::default();
        config.search.default_providers = vec!["brave_api".to_string()];
        config.search.api.insert(
            "brave_api".to_string(),
            ApiProviderConfig {
                enabled: true,
                api_key_env: Some(env.to_string()),
                base_url: None,
            },
        );

        let state = ServerState::build(config).expect("state builds");
        std::env::remove_var(env);
        assert!(state
            .adapter
            .provider_ids()
            .iter()
            .any(|id| id == "brave_api"));
    }

    #[test]
    fn host_scopes_authenticate_and_cache_distinct_roots() {
        let first = tempfile::tempdir().expect("first root");
        let second = tempfile::tempdir().expect("second root");
        let first_canonical = first.path().canonicalize().expect("canonical first root");
        let second_canonical = second.path().canonicalize().expect("canonical second root");
        let token = format!("test-{}", std::process::id());
        let previous_token = std::env::var_os(HOST_LOCAL_SCOPE_TOKEN_ENV);
        std::env::set_var(HOST_LOCAL_SCOPE_TOKEN_ENV, &token);
        let mut config = AppConfig::default();
        config.local.enabled = true;
        config.local.roots = vec![first.path().to_path_buf()];
        let state = ServerState::build(config).expect("state builds");

        let scope =
            |root: &std::path::Path, token: &str| serde_json::json!({"root": root, "token": token});
        let first_view = state
            .local_workspace_view(Some(&scope(first.path(), &token)))
            .expect("authorized first root");
        let second_view = state
            .local_workspace_view(Some(&scope(second.path(), &token)))
            .expect("authorized second root");
        let first_warm = state
            .local_workspace_view(Some(&scope(first.path(), &token)))
            .expect("warm authorized first root");
        assert_eq!(first_view.backend().unwrap().roots()[0].1, first_canonical);
        assert_eq!(
            second_view.backend().unwrap().roots()[0].1,
            second_canonical
        );
        assert!(!Arc::ptr_eq(&first_view, &second_view));
        assert!(Arc::ptr_eq(&first_view, &first_warm));

        let disabled = state
            .local_workspace_view(Some(&serde_json::json!({"root": null, "token": token})))
            .expect("authenticated explicit disabled scope");
        assert!(
            !disabled.is_enabled(),
            "null scope must not fall back to configured roots"
        );

        let forged = state.local_workspace_view(Some(&scope(second.path(), "forged")));
        assert!(forged.is_err());
        let malformed =
            state.local_workspace_view(Some(&serde_json::json!({"root": second.path()})));
        assert!(malformed.is_err());
        std::fs::remove_dir_all(second.path()).expect("remove host-scoped workspace");
        assert!(
            state
                .local_workspace_view(Some(&scope(second.path(), &token)))
                .is_err(),
            "removed workspace must fail rather than use a cached or static root"
        );
        std::env::remove_var(HOST_LOCAL_SCOPE_TOKEN_ENV);
        assert!(
            state
                .local_workspace_view(Some(&scope(first.path(), &token)))
                .is_err(),
            "missing process token must reject host metadata"
        );
        if let Some(previous_token) = previous_token {
            std::env::set_var(HOST_LOCAL_SCOPE_TOKEN_ENV, previous_token);
        }
    }

    #[test]
    fn host_scope_token_comparison_checks_length_and_contents() {
        assert!(constant_time_eq(b"same-token", b"same-token"));
        assert!(!constant_time_eq(b"same-token", b"same-token-x"));
        assert!(!constant_time_eq(b"same-token", b"other-token"));
    }
}
