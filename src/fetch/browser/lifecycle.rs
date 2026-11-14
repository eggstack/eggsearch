use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use tokio::sync::{Mutex, RwLock};

use super::types::{BrowserConfig, BrowserDiscovery, DEFAULT_IDLE_TIMEOUT_MS};

#[derive(Clone, Debug)]
enum BrowserExecutionMode {
    AnonymousEphemeral,
    PersistentProfile { user_data_dir: PathBuf },
}

#[derive(Clone, Copy, Debug)]
enum CloseReason {
    Explicit,
    Idle,
    SessionCap,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BrowserLifecycleStatus {
    pub resident: bool,
    pub session_launches: u64,
    pub renders: u64,
    pub idle_teardowns: u64,
    pub session_cap_teardowns: u64,
}

pub struct BrowserLifecycle {
    discovery: Option<BrowserDiscovery>,
    config: BrowserConfig,
    mode: BrowserExecutionMode,
    browser: Mutex<Option<Arc<chromiumoxide::Browser>>>,
    handler_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    restart_state: Mutex<RestartState>,
    user_data_dir: Mutex<Option<PathBuf>>,
    idle_timeout: Duration,
    last_activity: StdMutex<Instant>,
    render_gate: RwLock<()>,
    teardown_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    resident: AtomicBool,
    requests_this_session: AtomicUsize,
    session_launches: AtomicU64,
    renders: AtomicU64,
    idle_teardowns: AtomicU64,
    session_cap_teardowns: AtomicU64,
}

#[derive(Default)]
struct RestartState {
    ever_launched: bool,
    restarts: u32,
}

const MAX_RESTARTS: u32 = 1;

impl BrowserLifecycle {
    pub fn new(discovery: Option<BrowserDiscovery>, config: BrowserConfig) -> Self {
        Self {
            discovery,
            config,
            mode: BrowserExecutionMode::AnonymousEphemeral,
            browser: Mutex::new(None),
            handler_handle: Mutex::new(None),
            restart_state: Mutex::new(RestartState::default()),
            user_data_dir: Mutex::new(None),
            idle_timeout: Duration::from_millis(DEFAULT_IDLE_TIMEOUT_MS),
            last_activity: StdMutex::new(Instant::now()),
            render_gate: RwLock::new(()),
            teardown_handle: Mutex::new(None),
            resident: AtomicBool::new(false),
            requests_this_session: AtomicUsize::new(0),
            session_launches: AtomicU64::new(0),
            renders: AtomicU64::new(0),
            idle_teardowns: AtomicU64::new(0),
            session_cap_teardowns: AtomicU64::new(0),
        }
    }

    pub fn for_persistent_profile(
        discovery: Option<BrowserDiscovery>,
        config: BrowserConfig,
        user_data_dir: PathBuf,
    ) -> Self {
        Self {
            discovery,
            config,
            mode: BrowserExecutionMode::PersistentProfile { user_data_dir },
            browser: Mutex::new(None),
            handler_handle: Mutex::new(None),
            restart_state: Mutex::new(RestartState::default()),
            user_data_dir: Mutex::new(None),
            idle_timeout: Duration::from_millis(DEFAULT_IDLE_TIMEOUT_MS),
            last_activity: StdMutex::new(Instant::now()),
            render_gate: RwLock::new(()),
            teardown_handle: Mutex::new(None),
            resident: AtomicBool::new(false),
            requests_this_session: AtomicUsize::new(0),
            session_launches: AtomicU64::new(0),
            renders: AtomicU64::new(0),
            idle_teardowns: AtomicU64::new(0),
            session_cap_teardowns: AtomicU64::new(0),
        }
    }

    pub fn with_idle_timeout(mut self, idle_timeout: Duration) -> Self {
        self.idle_timeout = idle_timeout;
        self
    }

    pub fn status(&self) -> BrowserLifecycleStatus {
        BrowserLifecycleStatus {
            resident: self.resident.load(Ordering::Relaxed),
            session_launches: self.session_launches.load(Ordering::Relaxed),
            renders: self.renders.load(Ordering::Relaxed),
            idle_teardowns: self.idle_teardowns.load(Ordering::Relaxed),
            session_cap_teardowns: self.session_cap_teardowns.load(Ordering::Relaxed),
        }
    }

    pub async fn begin_render(&self) -> tokio::sync::RwLockReadGuard<'_, ()> {
        let guard = self.render_gate.read().await;
        self.renders.fetch_add(1, Ordering::Relaxed);
        self.requests_this_session.fetch_add(1, Ordering::Relaxed);
        guard
    }

    pub async fn finish_render(&self, guard: tokio::sync::RwLockReadGuard<'_, ()>) {
        drop(guard);
        self.touch();
        if self.requests_this_session.load(Ordering::Relaxed) >= self.config.max_requests {
            let _write = self.render_gate.write().await;
            self.close_inner(CloseReason::SessionCap).await;
        }
    }

    fn touch(&self) {
        if let Ok(mut last) = self.last_activity.lock() {
            *last = Instant::now();
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

        let mut state = self.restart_state.lock().await;

        // Re-check under the lock: a concurrent cold start may have
        // finished launching while this task waited.
        {
            let browser = self.browser.lock().await;
            if let Some(b) = browser.as_ref() {
                return Ok(Arc::clone(b));
            }
        }

        if state.ever_launched {
            if state.restarts >= MAX_RESTARTS {
                return Err(BrowserLaunchError::RestartLimitReached);
            }
            state.restarts += 1;
        }
        state.ever_launched = true;

        self.launch().await
    }

    async fn launch(self: &Arc<Self>) -> Result<Arc<chromiumoxide::Browser>, BrowserLaunchError> {
        let disc = self
            .discovery
            .as_ref()
            .ok_or(BrowserLaunchError::NoBrowser)?;

        let (user_data_dir, anonymous) = match &self.mode {
            BrowserExecutionMode::AnonymousEphemeral => (self.create_user_data_dir().await?, true),
            BrowserExecutionMode::PersistentProfile { user_data_dir } => {
                if !user_data_dir.is_dir() {
                    return Err(BrowserLaunchError::LaunchFailed(format!(
                        "persistent browser profile directory is unavailable: {}",
                        user_data_dir.display()
                    )));
                }
                (user_data_dir.clone(), false)
            }
        };

        let mut bc = chromiumoxide::BrowserConfig::builder()
            .no_sandbox()
            .disable_default_args();

        if anonymous {
            bc = bc.incognito();
        }

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

        if self.config.block_media {
            bc = bc.arg("--autoplay-policy=user-gesture-required");
        } else {
            bc = bc.arg("--autoplay-policy=no-user-gesture-required");
        }

        bc = bc.arg(format!("--user-data-dir={}", user_data_dir.display()));

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
                lifecycle.resident.store(false, Ordering::Relaxed);
            }
        });

        {
            let mut h = self.handler_handle.lock().await;
            *h = Some(handle);
        }
        let mut b = self.browser.lock().await;
        let browser = Arc::new(browser);
        *b = Some(Arc::clone(&browser));
        drop(b);

        self.requests_this_session.store(0, Ordering::Relaxed);
        self.session_launches.fetch_add(1, Ordering::Relaxed);
        self.resident.store(true, Ordering::Relaxed);
        self.touch();
        self.start_watchdog().await;

        Ok(browser)
    }

    async fn start_watchdog(self: &Arc<Self>) {
        let mut slot = self.teardown_handle.lock().await;
        if slot.as_ref().is_some_and(|handle| !handle.is_finished()) {
            return;
        }
        let weak = Arc::downgrade(self);
        let idle = self.idle_timeout;
        let tick = Duration::from_millis(
            u64::try_from(idle.as_millis() / 4)
                .unwrap_or(1)
                .clamp(10, 5_000),
        );
        *slot = Some(tokio::spawn(async move {
            loop {
                tokio::time::sleep(tick).await;
                let Some(lifecycle) = weak.upgrade() else {
                    return;
                };
                let write = lifecycle.render_gate.write().await;
                let idle_elapsed = lifecycle
                    .last_activity
                    .lock()
                    .map(|last| last.elapsed())
                    .unwrap_or_default();
                if lifecycle.resident.load(Ordering::Relaxed) && idle_elapsed >= idle {
                    lifecycle.close_inner(CloseReason::Idle).await;
                }
                drop(write);
            }
        }));
    }

    async fn stop_watchdog(&self) {
        let mut slot = self.teardown_handle.lock().await;
        if let Some(handle) = Option::take(&mut *slot) {
            handle.abort();
        }
    }

    async fn create_user_data_dir(&self) -> Result<PathBuf, BrowserLaunchError> {
        let dir = tokio::task::spawn_blocking(|| -> Result<PathBuf, BrowserLaunchError> {
            static COUNTER: AtomicU64 = AtomicU64::new(0);
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

            let pid = std::process::id() as u64;
            for _ in 0..10 {
                let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                let suffix = nanos ^ ((pid as u128) << 32) ^ (counter as u128);
                let dir = base.join(format!("ctx-{suffix}"));
                match std::fs::create_dir(&dir) {
                    Ok(()) => {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            let perms = std::fs::Permissions::from_mode(0o700);
                            std::fs::set_permissions(&dir, perms).map_err(|e| {
                                BrowserLaunchError::LaunchFailed(format!(
                                    "failed to set browser context dir permissions: {e}"
                                ))
                            })?;
                        }
                        return Ok(dir);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(e) => {
                        return Err(BrowserLaunchError::LaunchFailed(format!(
                            "failed to create browser context dir: {e}"
                        )))
                    }
                }
            }
            Err(BrowserLaunchError::LaunchFailed(
                "failed to create browser context dir after retries".into(),
            ))
        })
        .await
        .map_err(|e| {
            BrowserLaunchError::LaunchFailed(format!("browser temp dir task failed: {e}"))
        })??;

        {
            let mut ud = self.user_data_dir.lock().await;
            *ud = Some(dir.clone());
        }

        Ok(dir)
    }

    pub async fn close(&self) {
        let _write = self.render_gate.write().await;
        self.stop_watchdog().await;
        self.close_inner(CloseReason::Explicit).await;
    }

    async fn close_inner(&self, reason: CloseReason) {
        let taken = {
            let mut browser = self.browser.lock().await;
            browser.take()
        };
        if let Some(browser) = taken {
            if let Ok(mut inner) = Arc::try_unwrap(browser) {
                let _ = inner.close().await;
            }
        }
        self.resident.store(false, Ordering::Relaxed);
        self.requests_this_session.store(0, Ordering::Relaxed);
        {
            let mut handle = self.handler_handle.lock().await;
            if let Some(h) = Option::take(&mut *handle) {
                h.abort();
            }
        }
        self.cleanup_user_data_dir().await;
        match reason {
            CloseReason::Explicit => {}
            CloseReason::Idle => {
                self.idle_teardowns.fetch_add(1, Ordering::Relaxed);
            }
            CloseReason::SessionCap => {
                self.session_cap_teardowns.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    async fn cleanup_user_data_dir(&self) {
        if !matches!(&self.mode, BrowserExecutionMode::AnonymousEphemeral) {
            return;
        }
        let mut dir = self.user_data_dir.lock().await;
        if let Some(path) = dir.take() {
            let _ = tokio::task::spawn_blocking(move || std::fs::remove_dir_all(&path)).await;
        }
    }

    pub fn is_available(&self) -> bool {
        self.discovery.is_some()
    }

    pub fn config(&self) -> BrowserConfig {
        self.config.clone()
    }

    pub fn is_persistent_profile(&self) -> bool {
        matches!(&self.mode, BrowserExecutionMode::PersistentProfile { .. })
    }

    pub fn discovery(&self) -> Option<&BrowserDiscovery> {
        self.discovery.as_ref()
    }
}

impl Drop for BrowserLifecycle {
    fn drop(&mut self) {
        match self.handler_handle.try_lock() {
            Ok(mut handle) => {
                if let Some(h) = handle.take() {
                    h.abort();
                }
            }
            Err(_) => {
                tracing::warn!("browser lifecycle drop: handler_handle mutex already locked");
            }
        }
        match self.teardown_handle.try_lock() {
            Ok(mut handle) => {
                if let Some(h) = handle.take() {
                    h.abort();
                }
            }
            Err(_) => {
                tracing::warn!("browser lifecycle drop: teardown_handle mutex already locked");
            }
        }
        if matches!(&self.mode, BrowserExecutionMode::AnonymousEphemeral) {
            match self.user_data_dir.try_lock() {
                Ok(mut dir) => {
                    if let Some(path) = dir.take() {
                        let _ = std::fs::remove_dir_all(&path);
                    }
                }
                Err(_) => {
                    tracing::warn!(
                        "browser lifecycle drop: user_data_dir mutex locked, leaking ephemeral dir"
                    );
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn persistent_profile_close_does_not_remove_profile_directory(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let profile_dir = TempDir::new()?;
        let path = profile_dir.path().to_path_buf();
        let lifecycle =
            BrowserLifecycle::for_persistent_profile(None, BrowserConfig::default(), path.clone());

        assert!(lifecycle.is_persistent_profile());
        lifecycle.close().await;
        assert!(path.exists());
        Ok(())
    }

    #[tokio::test]
    async fn anonymous_close_removes_ephemeral_directory() {
        let lifecycle = BrowserLifecycle::new(None, BrowserConfig::default());
        let path = lifecycle
            .create_user_data_dir()
            .await
            .expect("temp directory");

        assert!(!lifecycle.is_persistent_profile());
        assert!(path.exists());
        lifecycle.close().await;
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn concurrent_cold_starts_do_not_fail_with_restart_limit() {
        let lifecycle = Arc::new(BrowserLifecycle::new(None, BrowserConfig::default()));
        let (a, b) = tokio::join!(lifecycle.ensure_browser(), lifecycle.ensure_browser());
        assert!(matches!(a, Err(BrowserLaunchError::NoBrowser)));
        assert!(matches!(b, Err(BrowserLaunchError::NoBrowser)));
    }

    #[tokio::test]
    async fn restart_budget_allows_one_retry_then_limits() {
        let lifecycle = Arc::new(BrowserLifecycle::new(None, BrowserConfig::default()));
        assert!(matches!(
            lifecycle.ensure_browser().await,
            Err(BrowserLaunchError::NoBrowser)
        ));
        assert!(matches!(
            lifecycle.ensure_browser().await,
            Err(BrowserLaunchError::NoBrowser)
        ));
        assert!(matches!(
            lifecycle.ensure_browser().await,
            Err(BrowserLaunchError::RestartLimitReached)
        ));
    }

    #[tokio::test]
    async fn status_starts_zeroed_and_explicit_close_does_not_count_as_teardown() {
        let lifecycle = BrowserLifecycle::new(None, BrowserConfig::default());
        assert_eq!(lifecycle.status(), BrowserLifecycleStatus::default());
        lifecycle.close().await;
        let status = lifecycle.status();
        assert!(!status.resident);
        assert_eq!(status.idle_teardowns, 0);
        assert_eq!(status.session_cap_teardowns, 0);
    }

    #[tokio::test]
    async fn teardown_gate_waits_for_an_in_flight_render() {
        let lifecycle = Arc::new(BrowserLifecycle::new(None, BrowserConfig::default()));
        let render = lifecycle.begin_render().await;
        let waiter = {
            let lifecycle = Arc::clone(&lifecycle);
            tokio::spawn(async move {
                let _write = lifecycle.render_gate.write().await;
                true
            })
        };
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            !waiter.is_finished(),
            "teardown must wait for the in-flight render"
        );
        drop(render);
        assert!(waiter.await.expect("waiter task"));
    }

    #[tokio::test]
    async fn idle_watchdog_closes_a_resident_session() {
        let lifecycle = Arc::new(
            BrowserLifecycle::new(None, BrowserConfig::default())
                .with_idle_timeout(Duration::from_millis(40)),
        );
        lifecycle.resident.store(true, Ordering::Relaxed);
        if let Ok(mut last) = lifecycle.last_activity.lock() {
            *last = Instant::now() - Duration::from_secs(1);
        }
        lifecycle.start_watchdog().await;
        tokio::time::sleep(Duration::from_millis(200)).await;
        let status = lifecycle.status();
        assert!(!status.resident, "watchdog must close the session");
        assert_eq!(status.idle_teardowns, 1);
        lifecycle.close().await;
    }

    #[tokio::test]
    async fn idle_watchdog_leaves_recent_activity_alone() {
        let lifecycle = Arc::new(
            BrowserLifecycle::new(None, BrowserConfig::default())
                .with_idle_timeout(Duration::from_millis(400)),
        );
        lifecycle.resident.store(true, Ordering::Relaxed);
        lifecycle.start_watchdog().await;
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(lifecycle.status().resident);
        assert_eq!(lifecycle.status().idle_teardowns, 0);
        lifecycle.close().await;
    }

    #[tokio::test]
    async fn session_cap_closes_the_session_after_the_render() {
        let config = BrowserConfig {
            max_requests: 1,
            ..BrowserConfig::default()
        };
        let lifecycle = BrowserLifecycle::new(None, config);
        let gate = lifecycle.begin_render().await;
        lifecycle.finish_render(gate).await;
        let status = lifecycle.status();
        assert_eq!(status.renders, 1);
        assert_eq!(status.session_cap_teardowns, 1);
        assert!(!status.resident);
    }

    #[tokio::test]
    async fn teardown_is_idempotent() {
        let lifecycle = BrowserLifecycle::new(None, BrowserConfig::default());
        lifecycle.close().await;
        lifecycle.close().await;
        assert_eq!(lifecycle.status().idle_teardowns, 0);
    }
}
