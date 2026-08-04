use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use super::types::{BrowserConfig, BrowserDiscovery};

pub struct BrowserLifecycle {
    discovery: Option<BrowserDiscovery>,
    config: BrowserConfig,
    browser: Mutex<Option<Arc<chromiumoxide::Browser>>>,
    handler_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl BrowserLifecycle {
    pub fn new(discovery: Option<BrowserDiscovery>, config: BrowserConfig) -> Self {
        Self {
            discovery,
            config,
            browser: Mutex::new(None),
            handler_handle: Mutex::new(None),
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

        self.launch().await
    }

    async fn launch(self: &Arc<Self>) -> Result<Arc<chromiumoxide::Browser>, BrowserLaunchError> {
        let disc = self
            .discovery
            .as_ref()
            .ok_or(BrowserLaunchError::NoBrowser)?;

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

        if self.config.block_media {
            bc = bc.arg("--autoplay-policy=no-user-gesture-required");
        }

        let startup_timeout = Duration::from_millis(self.config.startup_timeout_ms);

        let config = bc
            .launch_timeout(startup_timeout)
            .build()
            .map_err(BrowserLaunchError::ConfigError)?;

        let (browser, mut handler) = chromiumoxide::Browser::launch(config)
            .await
            .map_err(|e| BrowserLaunchError::LaunchFailed(e.to_string()))?;

        let handle = tokio::spawn(async move {
            use futures::StreamExt;
            while let Some(_event) = handler.next().await {}
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
    }
}

#[derive(Debug)]
pub enum BrowserLaunchError {
    NoBrowser,
    ConfigError(String),
    LaunchFailed(String),
}

impl std::fmt::Display for BrowserLaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoBrowser => write!(f, "no browser executable discovered"),
            Self::ConfigError(e) => write!(f, "browser config error: {e}"),
            Self::LaunchFailed(e) => write!(f, "browser launch failed: {e}"),
        }
    }
}

impl std::error::Error for BrowserLaunchError {}
