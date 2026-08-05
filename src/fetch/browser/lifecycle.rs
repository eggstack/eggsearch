use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use super::types::{BrowserConfig, BrowserDiscovery};

pub struct BrowserLifecycle {
    discovery: Option<BrowserDiscovery>,
    config: BrowserConfig,
    browser: Mutex<Option<Arc<chromiumoxide::Browser>>>,
    handler_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    restart_count: Mutex<u32>,
    user_data_dir: Mutex<Option<PathBuf>>,
}

const MAX_RESTARTS: u32 = 1;

impl BrowserLifecycle {
    pub fn new(discovery: Option<BrowserDiscovery>, config: BrowserConfig) -> Self {
        Self {
            discovery,
            config,
            browser: Mutex::new(None),
            handler_handle: Mutex::new(None),
            restart_count: Mutex::new(0),
            user_data_dir: Mutex::new(None),
        }
    }

    pub async fn ensure_browser(
        self: &Arc<Self>,
    ) -> Result<Arc<chromiumoxide::Browser>, BrowserLaunchError> {
        {
            let browser = self.browser.lock().await;
            if let Some(b) = browser.as_ref() {
                return Ok(Arc::clone(b));
            }
        }

        let mut restarts = self.restart_count.lock().await;
        if *restarts >= MAX_RESTARTS {
            return Err(BrowserLaunchError::RestartLimitReached);
        }
        *restarts += 1;

        self.launch().await
    }

    async fn launch(self: &Arc<Self>) -> Result<Arc<chromiumoxide::Browser>, BrowserLaunchError> {
        let disc = self
            .discovery
            .as_ref()
            .ok_or(BrowserLaunchError::NoBrowser)?;

        let temp_dir = self.create_user_data_dir().await?;

        let mut bc = chromiumoxide::BrowserConfig::builder()
            .no_sandbox()
            .incognito()
            .disable_default_args();

        bc = bc.chrome_executable(&disc.path);

        bc = bc.arg("--headless=new");
        bc = bc.arg("--disable-extensions");
        bc = bc.arg("--disable-component-extensions-with-background-pages");
        bc = bc.arg("--disable-default-apps");
        bc = bc.arg("--disable-dev-shm-usage");
        bc = bc.arg("--disable-gpu");
        bc = bc.arg("--no-first-run");
        bc = bc.arg("--no-default-browser-check");
        bc = bc.arg("--disable-background-networking");
        bc = bc.arg("--disable-background-timer-throttling");
        bc = bc.arg("--disable-backgrounding-occluded-windows");
        bc = bc.arg("--disable-renderer-backgrounding");
        bc = bc.arg("--disable-hang-monitor");
        bc = bc.arg("--disable-prompt-on-repost");
        bc = bc.arg("--disable-sync");
        bc = bc.arg("--metrics-recording-only");
        bc = bc.arg("--password-store=basic");
        bc = bc.arg("--use-mock-keychain");
        bc = bc.arg("--disable-features=TranslateUI");
        bc = bc.arg("--disable-ipc-flooding-protection");
        bc = bc.arg("--disable-extensions-http-auth-schemes");
        bc = bc.arg("--disable-backgrounding-occluded-windows");
        bc = bc.arg("--disable-renderer-backgrounding");

        if self.config.block_media {
            bc = bc.arg("--autoplay-policy=no-user-gesture-required");
        }

        bc = bc.arg(format!("--user-data-dir={}", temp_dir.display()));

        let startup_timeout = Duration::from_millis(self.config.startup_timeout_ms);

        let config = bc
            .launch_timeout(startup_timeout)
            .build()
            .map_err(BrowserLaunchError::ConfigError)?;

        let (browser, mut handler) = chromiumoxide::Browser::launch(config)
            .await
            .map_err(|e| BrowserLaunchError::LaunchFailed(e.to_string()))?;

        let self_weak = Arc::downgrade(self);
        let handle = tokio::spawn(async move {
            use futures::StreamExt;
            while let Some(_event) = handler.next().await {}
            if let Some(lifecycle) = self_weak.upgrade() {
                let mut browser = lifecycle.browser.lock().await;
                *browser = None;
            }
        });

        {
            let mut h = self.handler_handle.lock().await;
            *h = Some(handle);
        }
        {
            let mut b = self.browser.lock().await;
            *b = Some(Arc::new(browser));
        }

        Ok(Arc::clone(self.browser.lock().await.as_ref().unwrap()))
    }

    async fn create_user_data_dir(&self) -> Result<PathBuf, BrowserLaunchError> {
        let base = std::env::temp_dir().join("eggsearch-browser");
        std::fs::create_dir_all(&base).map_err(|e| {
            BrowserLaunchError::LaunchFailed(format!("failed to create browser temp dir: {e}"))
        })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o700);
            std::fs::set_permissions(&base, perms).map_err(|e| {
                BrowserLaunchError::LaunchFailed(format!(
                    "failed to set browser temp dir permissions: {e}"
                ))
            })?;
        }

        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();

        let dir = base.join(format!("ctx-{suffix}"));
        std::fs::create_dir(&dir).map_err(|e| {
            BrowserLaunchError::LaunchFailed(format!("failed to create browser context dir: {e}"))
        })?;

        {
            let mut ud = self.user_data_dir.lock().await;
            *ud = Some(dir.clone());
        }

        Ok(dir)
    }

    pub async fn close(&self) {
        let mut browser = self.browser.lock().await;
        if let Some(b) = browser.take() {
            if let Ok(mut inner) = Arc::try_unwrap(b) {
                let _ = inner.close().await;
            }
        }
        let mut handle = self.handler_handle.lock().await;
        if let Some(h) = Option::take(&mut *handle) {
            h.abort();
        }
        self.cleanup_user_data_dir().await;
    }

    async fn cleanup_user_data_dir(&self) {
        let mut dir = self.user_data_dir.lock().await;
        if let Some(path) = dir.take() {
            let _ = std::fs::remove_dir_all(&path);
        }
    }

    pub fn is_available(&self) -> bool {
        self.discovery.is_some()
    }

    pub fn discovery(&self) -> Option<&BrowserDiscovery> {
        self.discovery.as_ref()
    }
}

impl Drop for BrowserLifecycle {
    fn drop(&mut self) {
        if let Ok(mut handle) = self.handler_handle.try_lock() {
            if handle.is_some() {
                let h = handle.take().unwrap();
                h.abort();
            }
        }
        if let Ok(mut dir) = self.user_data_dir.try_lock() {
            if let Some(path) = dir.take() {
                let _ = std::fs::remove_dir_all(&path);
            }
        }
    }
}

#[derive(Debug)]
pub enum BrowserLaunchError {
    NoBrowser,
    ConfigError(String),
    LaunchFailed(String),
    RestartLimitReached,
}

impl std::fmt::Display for BrowserLaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoBrowser => write!(f, "no browser executable discovered"),
            Self::ConfigError(e) => write!(f, "browser config error: {e}"),
            Self::LaunchFailed(e) => write!(f, "browser launch failed: {e}"),
            Self::RestartLimitReached => write!(f, "browser restart limit reached"),
        }
    }
}

impl std::error::Error for BrowserLaunchError {}
