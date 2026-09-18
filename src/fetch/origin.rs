use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tokio::sync::{Mutex, Semaphore};

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct OriginKey {
    pub scheme: String,
    pub host: String,
    pub port: u16,
}

impl OriginKey {
    pub fn from_url(url: &url::Url) -> Option<Self> {
        let scheme = url.scheme().to_lowercase();
        if !matches!(scheme.as_str(), "http" | "https") {
            return None;
        }
        let host = url.host_str()?.to_lowercase();
        let port = url.port_or_known_default()?;
        Some(Self { scheme, host, port })
    }
}

#[derive(Clone, Debug)]
pub struct OriginPolicy {
    pub http_concurrency: usize,
    pub browser_concurrency: usize,
    pub retry_max_attempts: usize,
    pub retry_base_delay_ms: u64,
    pub retry_max_delay_ms: u64,
    pub circuit_failure_threshold: u8,
    pub circuit_duration_ms: u64,
}

impl Default for OriginPolicy {
    fn default() -> Self {
        Self {
            http_concurrency: 2,
            browser_concurrency: 1,
            retry_max_attempts: 2,
            retry_base_delay_ms: 250,
            retry_max_delay_ms: 4000,
            circuit_failure_threshold: 3,
            circuit_duration_ms: 60_000,
        }
    }
}

pub struct OriginState {
    pub semaphore: Arc<Semaphore>,
    pub failures: Mutex<FailureState>,
    pub last_access_ms: AtomicU64,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(1)
}

pub struct FailureState {
    pub consecutive_retryable_failures: u8,
    pub next_allowed_at: Option<Instant>,
    pub circuit_open_until: Option<Instant>,
    pub last_failure_class: Option<OriginFailureClass>,
}

impl FailureState {
    fn new() -> Self {
        Self {
            consecutive_retryable_failures: 0,
            next_allowed_at: None,
            circuit_open_until: None,
            last_failure_class: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OriginFailureClass {
    Retryable,
    RateLimited,
    NonRetryable,
}

pub struct OriginController {
    states: Mutex<HashMap<OriginKey, Arc<OriginState>>>,
    defaults: OriginPolicy,
    max_entries: usize,
}

impl OriginController {
    pub fn new(defaults: OriginPolicy, max_entries: usize) -> Self {
        Self {
            states: Mutex::new(HashMap::new()),
            defaults,
            max_entries,
        }
    }

    pub async fn acquire(
        &self,
        key: &OriginKey,
    ) -> Result<tokio::sync::OwnedSemaphorePermit, OriginBackoffError> {
        let state = self.get_or_create_state(key).await;

        if let Some(open_until) = state.failures.lock().await.circuit_open_until {
            if Instant::now() < open_until {
                let remaining_ms = open_until
                    .duration_since(Instant::now())
                    .as_millis()
                    .min(u128::from(u64::MAX)) as u64;
                return Err(OriginBackoffError::CircuitOpen { remaining_ms });
            }
        }

        let sem = Arc::clone(&state.semaphore);
        sem.acquire_owned()
            .await
            .map_err(|_| OriginBackoffError::LimiterClosed)
    }

    pub async fn record_success(&self, key: &OriginKey) {
        if let Some(state) = self.get_state(key).await {
            let mut failures = state.failures.lock().await;
            failures.consecutive_retryable_failures = 0;
            failures.next_allowed_at = None;
            failures.circuit_open_until = None;
            failures.last_failure_class = None;
        }
    }

    pub async fn record_failure(
        &self,
        key: &OriginKey,
        class: OriginFailureClass,
    ) -> OriginBackoffDecision {
        let state = self.get_or_create_state(key).await;
        let mut failures = state.failures.lock().await;

        match class {
            OriginFailureClass::NonRetryable => {
                failures.consecutive_retryable_failures = 0;
                failures.next_allowed_at = None;
                failures.last_failure_class = Some(class);
                return OriginBackoffDecision::NoBackoff;
            }
            OriginFailureClass::Retryable | OriginFailureClass::RateLimited => {
                failures.consecutive_retryable_failures =
                    failures.consecutive_retryable_failures.saturating_add(1);
                failures.last_failure_class = Some(class);
            }
        }

        let count = failures.consecutive_retryable_failures;
        let base = self.defaults.retry_base_delay_ms;
        let max = self.defaults.retry_max_delay_ms;
        let cap = base.saturating_mul(1u64 << count.min(6)).min(max);
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        count.hash(&mut hasher);
        let jitter_seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
            ^ hasher.finish();
        let delay_ms = (jitter_seed % (cap + 1)).max(1);

        if count >= self.defaults.circuit_failure_threshold {
            let circuit_dur = Duration::from_millis(self.defaults.circuit_duration_ms)
                .min(Duration::from_secs(120));
            failures.circuit_open_until = Some(Instant::now() + circuit_dur);
            return OriginBackoffDecision::CircuitOpened {
                delay_ms,
                circuit_duration_ms: circuit_dur.as_millis().min(u128::from(u64::MAX)) as u64,
            };
        }

        let retry_after = match class {
            OriginFailureClass::RateLimited => Some(delay_ms),
            _ => None,
        };

        failures.next_allowed_at = Some(Instant::now() + Duration::from_millis(delay_ms));
        OriginBackoffDecision::Backoff {
            delay_ms,
            retry_after_ms: retry_after,
        }
    }

    pub async fn reset_circuit(&self, key: &OriginKey) {
        if let Some(state) = self.get_state(key).await {
            let mut failures = state.failures.lock().await;
            failures.circuit_open_until = None;
            failures.consecutive_retryable_failures = 0;
            failures.next_allowed_at = None;
        }
    }

    async fn get_or_create_state(&self, key: &OriginKey) -> Arc<OriginState> {
        let mut states = self.states.lock().await;
        if let Some(state) = states.get(key) {
            state.last_access_ms.store(now_ms(), Ordering::Relaxed);
            return Arc::clone(state);
        }

        if states.len() >= self.max_entries {
            let oldest_key = states
                .iter()
                .min_by_key(|(_, s)| s.last_access_ms.load(Ordering::Relaxed))
                .map(|(k, _)| k.clone());
            if let Some(oldest) = oldest_key {
                states.remove(&oldest);
            }
        }

        let state = Arc::new(OriginState {
            semaphore: Arc::new(Semaphore::new(self.defaults.http_concurrency)),
            failures: Mutex::new(FailureState::new()),
            last_access_ms: AtomicU64::new(now_ms()),
        });
        states.insert(key.clone(), Arc::clone(&state));
        state
    }

    async fn get_state(&self, key: &OriginKey) -> Option<Arc<OriginState>> {
        let states = self.states.lock().await;
        states.get(key).map(Arc::clone)
    }

    pub async fn circuit_is_open(&self, key: &OriginKey) -> Option<Duration> {
        let state = self.get_state(key).await?;
        let failures = state.failures.lock().await;
        let open_until = failures.circuit_open_until?;
        let now = Instant::now();
        if now < open_until {
            Some(open_until.duration_since(now))
        } else {
            None
        }
    }

    pub async fn entry_count(&self) -> usize {
        self.states.lock().await.len()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OriginBackoffError {
    CircuitOpen { remaining_ms: u64 },
    LimiterClosed,
}

impl std::fmt::Display for OriginBackoffError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CircuitOpen { remaining_ms } => {
                write!(f, "origin circuit breaker open, retry in {remaining_ms}ms")
            }
            Self::LimiterClosed => write!(f, "origin limiter closed"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OriginBackoffDecision {
    NoBackoff,
    Backoff {
        delay_ms: u64,
        retry_after_ms: Option<u64>,
    },
    CircuitOpened {
        delay_ms: u64,
        circuit_duration_ms: u64,
    },
}

pub fn parse_retry_after(header_value: &str) -> Option<Duration> {
    let trimmed = header_value.trim();
    if let Ok(secs) = trimmed.parse::<u64>() {
        return Some(Duration::from_secs(secs.min(300)));
    }

    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(trimmed) {
        let now = SystemTime::now();
        let header_time: SystemTime = dt.into();
        if let Ok(dur) = header_time.duration_since(now) {
            return Some(dur.min(Duration::from_secs(300)));
        }
        return Some(Duration::ZERO);
    }

    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(trimmed) {
        let now = SystemTime::now();
        let header_time: SystemTime = dt.into();
        if let Ok(dur) = header_time.duration_since(now) {
            return Some(dur.min(Duration::from_secs(300)));
        }
        return Some(Duration::ZERO);
    }

    None
}

pub fn should_retry(class: OriginFailureClass, attempt: usize, max_attempts: usize) -> bool {
    if attempt >= max_attempts {
        return false;
    }
    matches!(
        class,
        OriginFailureClass::Retryable | OriginFailureClass::RateLimited
    )
}

pub fn classify_http_status(status: u16) -> OriginFailureClass {
    match status {
        429 => OriginFailureClass::RateLimited,
        502..=504 => OriginFailureClass::Retryable,
        _ => OriginFailureClass::NonRetryable,
    }
}

pub fn classify_network_error(err: &str) -> OriginFailureClass {
    let lower = err.to_lowercase();
    if lower.contains("connection reset")
        || lower.contains("connection refused")
        || lower.contains("dns")
        || lower.contains("broken pipe")
        || lower.contains("eof")
    {
        OriginFailureClass::Retryable
    } else {
        OriginFailureClass::NonRetryable
    }
}

pub fn classify_eggfetch_error(error: &eggfetch_core::Error) -> OriginFailureClass {
    if matches!(
        error,
        eggfetch_core::Error::Timeout { .. } | eggfetch_core::Error::TransportIoTimeout { .. }
    ) {
        OriginFailureClass::Retryable
    } else {
        classify_network_error(&error.to_string())
    }
}

pub fn classify_request_failure(failure: &eggfetch_core::RequestFailure) -> OriginFailureClass {
    use eggfetch_core::NetworkFailureKind::*;
    match failure.network_failure_kind() {
        Some(Dns) | Some(ConnectionRefused) | Some(Connect) => OriginFailureClass::Retryable,
        _ => {
            if failure.is_timeout() {
                OriginFailureClass::Retryable
            } else {
                classify_network_error(&failure.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_key_from_url() {
        let url = url::Url::parse("https://example.com:8443/path").unwrap();
        let key = OriginKey::from_url(&url).unwrap();
        assert_eq!(key.scheme, "https");
        assert_eq!(key.host, "example.com");
        assert_eq!(key.port, 8443);
    }

    #[test]
    fn origin_key_from_url_default_port() {
        let url = url::Url::parse("https://example.com/path").unwrap();
        let key = OriginKey::from_url(&url).unwrap();
        assert_eq!(key.port, 443);
    }

    #[test]
    fn origin_key_from_url_http_default_port() {
        let url = url::Url::parse("http://example.com/path").unwrap();
        let key = OriginKey::from_url(&url).unwrap();
        assert_eq!(key.port, 80);
    }

    #[test]
    fn origin_key_rejects_non_http() {
        let url = url::Url::parse("ftp://example.com/file").unwrap();
        assert!(OriginKey::from_url(&url).is_none());
    }

    #[test]
    fn parse_retry_after_delta_seconds() {
        let d = parse_retry_after("30").unwrap();
        assert_eq!(d, Duration::from_secs(30));
    }

    #[test]
    fn parse_retry_after_clamps_to_300() {
        let d = parse_retry_after("9999").unwrap();
        assert_eq!(d, Duration::from_secs(300));
    }

    #[test]
    fn parse_retry_after_rfc2822() {
        let now = SystemTime::now() + Duration::from_secs(10);
        let dt: chrono::DateTime<chrono::Utc> = now.into();
        let header = dt.to_rfc2822();
        let d = parse_retry_after(&header).unwrap();
        assert!(d.as_secs() <= 15);
        assert!(d.as_secs() >= 5);
    }

    #[test]
    fn parse_retry_after_invalid() {
        assert!(parse_retry_after("not-a-date").is_none());
    }

    #[test]
    fn classify_status_retryable() {
        assert_eq!(classify_http_status(429), OriginFailureClass::RateLimited);
        assert_eq!(classify_http_status(502), OriginFailureClass::Retryable);
        assert_eq!(classify_http_status(503), OriginFailureClass::Retryable);
        assert_eq!(classify_http_status(504), OriginFailureClass::Retryable);
    }

    #[test]
    fn classify_status_non_retryable() {
        assert_eq!(classify_http_status(400), OriginFailureClass::NonRetryable);
        assert_eq!(classify_http_status(403), OriginFailureClass::NonRetryable);
        assert_eq!(classify_http_status(404), OriginFailureClass::NonRetryable);
    }

    #[tokio::test]
    async fn retry_delay_is_bounded() {
        let controller = OriginController::new(OriginPolicy::default(), 100);
        let key = OriginKey {
            scheme: "https".into(),
            host: "example.com".into(),
            port: 443,
        };
        let decision = controller
            .record_failure(&key, OriginFailureClass::Retryable)
            .await;
        assert!(matches!(
            decision,
            OriginBackoffDecision::Backoff { delay_ms, .. }
                if (1..=500).contains(&delay_ms)
        ));
    }

    #[test]
    fn should_retry_respects_attempt_cap() {
        assert!(should_retry(OriginFailureClass::Retryable, 0, 2));
        assert!(should_retry(OriginFailureClass::Retryable, 1, 2));
        assert!(!should_retry(OriginFailureClass::Retryable, 2, 2));
    }

    #[test]
    fn should_retry_rejects_non_retryable() {
        assert!(!should_retry(OriginFailureClass::NonRetryable, 0, 2));
    }

    #[test]
    fn classify_network_errors() {
        assert_eq!(
            classify_network_error("connection reset by peer"),
            OriginFailureClass::Retryable
        );
        assert_eq!(
            classify_network_error("dns error: failed to lookup"),
            OriginFailureClass::Retryable
        );
        assert_eq!(
            classify_network_error("tls handshake failed"),
            OriginFailureClass::NonRetryable
        );
    }

    #[tokio::test]
    async fn origin_controller_basic() {
        let policy = OriginPolicy {
            http_concurrency: 1,
            circuit_failure_threshold: 2,
            ..Default::default()
        };
        let controller = OriginController::new(policy, 100);
        let key = OriginKey {
            scheme: "https".into(),
            host: "example.com".into(),
            port: 443,
        };

        let permit = controller.acquire(&key).await.unwrap();
        drop(permit);

        controller.record_success(&key).await;
        assert!(controller.circuit_is_open(&key).await.is_none());
    }

    #[tokio::test]
    async fn circuit_opens_after_threshold() {
        let policy = OriginPolicy {
            http_concurrency: 2,
            circuit_failure_threshold: 2,
            circuit_duration_ms: 60_000,
            ..Default::default()
        };
        let controller = OriginController::new(policy, 100);
        let key = OriginKey {
            scheme: "https".into(),
            host: "example.com".into(),
            port: 443,
        };

        let _p1 = controller.acquire(&key).await.unwrap();
        let _p2 = controller.acquire(&key).await.unwrap();

        let d1 = controller
            .record_failure(&key, OriginFailureClass::Retryable)
            .await;
        assert!(matches!(d1, OriginBackoffDecision::Backoff { .. }));

        let d2 = controller
            .record_failure(&key, OriginFailureClass::Retryable)
            .await;
        assert!(matches!(d2, OriginBackoffDecision::CircuitOpened { .. }));

        let result = controller.acquire(&key).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn success_resets_circuit() {
        let policy = OriginPolicy {
            http_concurrency: 1,
            circuit_failure_threshold: 1,
            circuit_duration_ms: 60_000,
            ..Default::default()
        };
        let controller = OriginController::new(policy, 100);
        let key = OriginKey {
            scheme: "https".into(),
            host: "a.com".into(),
            port: 443,
        };

        controller
            .record_failure(&key, OriginFailureClass::Retryable)
            .await;
        controller.record_success(&key).await;
        assert!(controller.circuit_is_open(&key).await.is_none());
    }

    #[tokio::test]
    async fn non_retryable_failure_does_not_increment() {
        let policy = OriginPolicy {
            circuit_failure_threshold: 2,
            ..Default::default()
        };
        let controller = OriginController::new(policy, 100);
        let key = OriginKey {
            scheme: "https".into(),
            host: "b.com".into(),
            port: 443,
        };

        let d = controller
            .record_failure(&key, OriginFailureClass::NonRetryable)
            .await;
        assert_eq!(d, OriginBackoffDecision::NoBackoff);

        let d = controller
            .record_failure(&key, OriginFailureClass::NonRetryable)
            .await;
        assert_eq!(d, OriginBackoffDecision::NoBackoff);

        assert!(controller.circuit_is_open(&key).await.is_none());
    }

    #[tokio::test]
    async fn typed_timeout_drives_retryable_backoff() {
        let error = eggfetch_core::Error::Timeout {
            phase: eggfetch_core::TimeoutPhase::Connect,
            elapsed: Duration::from_millis(500),
        };
        let typed = classify_eggfetch_error(&error);
        assert_eq!(typed, OriginFailureClass::Retryable);
        let controller = OriginController::new(OriginPolicy::default(), 100);
        let key = OriginKey {
            scheme: "https".into(),
            host: "timeout.example".into(),
            port: 443,
        };
        let decision = controller.record_failure(&key, typed).await;
        assert!(matches!(decision, OriginBackoffDecision::Backoff { .. }));
    }

    #[test]
    fn typed_transport_io_timeout_is_retryable() {
        let error = eggfetch_core::Error::TransportIoTimeout {
            direction: eggfetch_core::TransportIoDirection::Read,
            elapsed: Duration::from_millis(500),
        };
        assert_eq!(
            classify_eggfetch_error(&error),
            OriginFailureClass::Retryable
        );
    }

    #[test]
    fn typed_refused_matches_string_classification() {
        let error = eggfetch_core::Error::Connect("connection refused".into());
        let typed = classify_eggfetch_error(&error);
        assert_eq!(typed, OriginFailureClass::Retryable);
        assert_eq!(typed, classify_network_error(&error.to_string()));
    }

    #[test]
    fn typed_tls_matches_string_classification() {
        let error = eggfetch_core::Error::Tls("tls handshake failed".into());
        let typed = classify_eggfetch_error(&error);
        assert_eq!(typed, OriginFailureClass::NonRetryable);
        assert_eq!(typed, classify_network_error(&error.to_string()));
    }

    #[tokio::test]
    async fn typed_refused_drives_retryable_backoff() {
        let controller = OriginController::new(OriginPolicy::default(), 100);
        let key = OriginKey {
            scheme: "https".into(),
            host: "refused.example".into(),
            port: 443,
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let client = eggfetch_core::Client::builder().build();
        let failure = client
            .get(&format!("http://{addr}/"))
            .unwrap()
            .send_detailed()
            .await
            .expect_err("expected connection refused");
        let typed = classify_request_failure(&failure);
        assert_eq!(typed, OriginFailureClass::Retryable);
        assert!(failure.network_failure_kind().is_some());
        let decision = controller.record_failure(&key, typed).await;
        assert!(matches!(decision, OriginBackoffDecision::Backoff { .. }));
    }

    #[tokio::test]
    async fn typed_dns_failure_is_retryable() {
        let client = eggfetch_core::Client::builder().build();
        let timeout = eggfetch_core::Timeout {
            pool: Some(Duration::from_secs(2)),
            connect: Some(Duration::from_secs(2)),
            write: Some(Duration::from_secs(2)),
            read: Some(Duration::from_secs(2)),
            total: Some(Duration::from_secs(5)),
        };
        let failure = client
            .get("http://eggsearch-nonexistent.invalid/")
            .unwrap()
            .timeout(timeout)
            .send_detailed()
            .await
            .expect_err("expected DNS failure");
        assert_eq!(
            classify_request_failure(&failure),
            OriginFailureClass::Retryable
        );
    }

    #[tokio::test]
    async fn typed_timeout_failure_is_retryable() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            if let Ok((socket, _)) = listener.accept().await {
                tokio::time::sleep(Duration::from_secs(30)).await;
                drop(socket);
            }
        });
        let client = eggfetch_core::Client::builder().build();
        let stall = Duration::from_millis(300);
        let timeout = eggfetch_core::Timeout {
            pool: Some(stall),
            connect: Some(stall),
            write: Some(stall),
            read: Some(stall),
            total: Some(stall),
        };
        let failure = client
            .get(&format!("http://{addr}/"))
            .unwrap()
            .timeout(timeout)
            .send_detailed()
            .await
            .expect_err("expected timeout");
        assert!(failure.is_timeout());
        assert_eq!(
            classify_request_failure(&failure),
            OriginFailureClass::Retryable
        );
    }
}
