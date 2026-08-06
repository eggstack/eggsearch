use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::classify::FetchDisposition;

pub const MAX_STARTUP_TIMEOUT_MS: u64 = 60_000;
pub const MAX_NAVIGATION_TIMEOUT_MS: u64 = 120_000;
pub const MAX_POST_LOAD_WAIT_MS: u64 = 30_000;
pub const MAX_VERIFICATION_WAIT_MS: u64 = 60_000;
pub const MAX_MAX_REQUESTS: usize = 1000;
pub const MAX_MAX_DOM_BYTES: usize = 50_000_000;
pub const MAX_GLOBAL_CONCURRENCY: usize = 4;
pub const MAX_PER_ORIGIN_CONCURRENCY: usize = 4;

pub const DEFAULT_STARTUP_TIMEOUT_MS: u64 = 10_000;
pub const DEFAULT_NAVIGATION_TIMEOUT_MS: u64 = 20_000;
pub const DEFAULT_POST_LOAD_WAIT_MS: u64 = 1_500;
pub const DEFAULT_VERIFICATION_WAIT_MS: u64 = 10_000;
pub const DEFAULT_MAX_REQUESTS: usize = 100;
pub const DEFAULT_MAX_DOM_BYTES: usize = 4_000_000;
pub const DEFAULT_GLOBAL_CONCURRENCY: usize = 1;
pub const DEFAULT_PER_ORIGIN_CONCURRENCY: usize = 1;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RenderPolicy {
    #[default]
    HttpOnly,
    Auto,
    Browser,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FetchTransportKind {
    Http,
    Browser,
}

#[derive(Clone, Debug, Default)]
pub struct TransportTiming {
    pub dns_ms: u64,
    pub connect_ms: u64,
    pub tls_ms: u64,
    pub server_ms: u64,
    pub total_ms: u64,
}

#[derive(Clone, Debug)]
pub struct TransportResponse {
    pub transport: FetchTransportKind,
    pub requested_url: String,
    pub final_url: String,
    pub status: Option<u16>,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub content_type: Option<String>,
    pub redirects: Vec<String>,
    pub timing: TransportTiming,
    pub classification: Option<FetchDisposition>,
}

#[derive(Clone, Debug)]
pub struct BrowserDiscovery {
    pub path: PathBuf,
    pub family: BrowserFamily,
    pub version: String,
    pub source: BrowserSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowserFamily {
    Chrome,
    Chromium,
    Unknown(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowserSource {
    Configured,
    AutoDiscovered,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrowserConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_render_policy")]
    pub policy: RenderPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable: Option<String>,
    #[serde(default = "default_startup_timeout_ms")]
    pub startup_timeout_ms: u64,
    #[serde(default = "default_navigation_timeout_ms")]
    pub navigation_timeout_ms: u64,
    #[serde(default = "default_post_load_wait_ms")]
    pub post_load_wait_ms: u64,
    #[serde(default = "default_verification_wait_ms")]
    pub verification_wait_ms: u64,
    #[serde(default = "default_max_requests")]
    pub max_requests: usize,
    #[serde(default = "default_max_dom_bytes")]
    pub max_dom_bytes: usize,
    #[serde(default = "default_global_concurrency")]
    pub global_concurrency: usize,
    #[serde(default = "default_per_origin_concurrency")]
    pub per_origin_concurrency: usize,
    #[serde(default = "default_block_media")]
    pub block_media: bool,
    #[serde(default)]
    pub persistent_profiles: PersistentBrowserProfilesConfig,
}

fn default_render_policy() -> RenderPolicy {
    RenderPolicy::HttpOnly
}
fn default_startup_timeout_ms() -> u64 {
    DEFAULT_STARTUP_TIMEOUT_MS
}
fn default_navigation_timeout_ms() -> u64 {
    DEFAULT_NAVIGATION_TIMEOUT_MS
}
fn default_post_load_wait_ms() -> u64 {
    DEFAULT_POST_LOAD_WAIT_MS
}
fn default_verification_wait_ms() -> u64 {
    DEFAULT_VERIFICATION_WAIT_MS
}
fn default_max_requests() -> usize {
    DEFAULT_MAX_REQUESTS
}
fn default_max_dom_bytes() -> usize {
    DEFAULT_MAX_DOM_BYTES
}
fn default_global_concurrency() -> usize {
    DEFAULT_GLOBAL_CONCURRENCY
}
fn default_per_origin_concurrency() -> usize {
    DEFAULT_PER_ORIGIN_CONCURRENCY
}
fn default_block_media() -> bool {
    true
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            policy: RenderPolicy::default(),
            executable: None,
            startup_timeout_ms: DEFAULT_STARTUP_TIMEOUT_MS,
            navigation_timeout_ms: DEFAULT_NAVIGATION_TIMEOUT_MS,
            post_load_wait_ms: DEFAULT_POST_LOAD_WAIT_MS,
            verification_wait_ms: DEFAULT_VERIFICATION_WAIT_MS,
            max_requests: DEFAULT_MAX_REQUESTS,
            max_dom_bytes: DEFAULT_MAX_DOM_BYTES,
            global_concurrency: DEFAULT_GLOBAL_CONCURRENCY,
            per_origin_concurrency: DEFAULT_PER_ORIGIN_CONCURRENCY,
            block_media: true,
            persistent_profiles: PersistentBrowserProfilesConfig::default(),
        }
    }
}

impl BrowserConfig {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.startup_timeout_ms > MAX_STARTUP_TIMEOUT_MS {
            errors.push(format!(
                "startup_timeout_ms {} exceeds maximum {}",
                self.startup_timeout_ms, MAX_STARTUP_TIMEOUT_MS
            ));
        }
        if self.navigation_timeout_ms > MAX_NAVIGATION_TIMEOUT_MS {
            errors.push(format!(
                "navigation_timeout_ms {} exceeds maximum {}",
                self.navigation_timeout_ms, MAX_NAVIGATION_TIMEOUT_MS
            ));
        }
        if self.post_load_wait_ms > MAX_POST_LOAD_WAIT_MS {
            errors.push(format!(
                "post_load_wait_ms {} exceeds maximum {}",
                self.post_load_wait_ms, MAX_POST_LOAD_WAIT_MS
            ));
        }
        if self.verification_wait_ms > MAX_VERIFICATION_WAIT_MS {
            errors.push(format!(
                "verification_wait_ms {} exceeds maximum {}",
                self.verification_wait_ms, MAX_VERIFICATION_WAIT_MS
            ));
        }
        if self.max_requests > MAX_MAX_REQUESTS {
            errors.push(format!(
                "max_requests {} exceeds maximum {}",
                self.max_requests, MAX_MAX_REQUESTS
            ));
        }
        if self.max_dom_bytes > MAX_MAX_DOM_BYTES {
            errors.push(format!(
                "max_dom_bytes {} exceeds maximum {}",
                self.max_dom_bytes, MAX_MAX_DOM_BYTES
            ));
        }
        if self.global_concurrency > MAX_GLOBAL_CONCURRENCY {
            errors.push(format!(
                "global_concurrency {} exceeds maximum {}",
                self.global_concurrency, MAX_GLOBAL_CONCURRENCY
            ));
        }
        if self.per_origin_concurrency > MAX_PER_ORIGIN_CONCURRENCY {
            errors.push(format!(
                "per_origin_concurrency {} exceeds maximum {}",
                self.per_origin_concurrency, MAX_PER_ORIGIN_CONCURRENCY
            ));
        }
        if self.persistent_profiles.profile_process_timeout_ms > MAX_PROFILE_PROCESS_TIMEOUT_MS {
            errors.push(format!(
                "persistent_profiles.profile_process_timeout_ms {} exceeds maximum {}",
                self.persistent_profiles.profile_process_timeout_ms, MAX_PROFILE_PROCESS_TIMEOUT_MS
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn with_clamped_values(self) -> Self {
        Self {
            startup_timeout_ms: self.startup_timeout_ms.min(MAX_STARTUP_TIMEOUT_MS),
            navigation_timeout_ms: self.navigation_timeout_ms.min(MAX_NAVIGATION_TIMEOUT_MS),
            post_load_wait_ms: self.post_load_wait_ms.min(MAX_POST_LOAD_WAIT_MS),
            verification_wait_ms: self.verification_wait_ms.min(MAX_VERIFICATION_WAIT_MS),
            max_requests: self.max_requests.min(MAX_MAX_REQUESTS),
            max_dom_bytes: self.max_dom_bytes.min(MAX_MAX_DOM_BYTES),
            global_concurrency: self.global_concurrency.min(MAX_GLOBAL_CONCURRENCY),
            per_origin_concurrency: self.per_origin_concurrency.min(MAX_PER_ORIGIN_CONCURRENCY),
            persistent_profiles: PersistentBrowserProfilesConfig {
                profile_process_timeout_ms: self
                    .persistent_profiles
                    .profile_process_timeout_ms
                    .min(MAX_PROFILE_PROCESS_TIMEOUT_MS),
                ..self.persistent_profiles
            },
            ..self
        }
    }

    pub fn check_availability(&self, discovery: Option<&BrowserDiscovery>) -> BrowserAvailability {
        if !self.enabled {
            return BrowserAvailability::BrowserDisabled;
        }
        match discovery {
            None => BrowserAvailability::AutoDiscoveryFailed,
            Some(disc) => BrowserAvailability::Available {
                discovery: disc.clone(),
            },
        }
    }
}

pub const DEFAULT_PROFILE_PROCESS_TIMEOUT_MS: u64 = 30_000;
pub const MAX_PROFILE_PROCESS_TIMEOUT_MS: u64 = 120_000;

pub use crate::core::config::PersistentBrowserProfilesConfig;

#[derive(Clone, Debug)]
pub struct ManualInteractionRequired {
    pub origin: String,
    pub reason: ManualInteractionReason,
    pub browser_profile_supported: bool,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManualInteractionReason {
    InteractiveChallenge,
    TurnstileCaptcha,
    OtherVerificationRequired,
}

#[derive(Clone, Debug)]
pub enum BrowserAvailability {
    FeatureNotCompiled,
    BrowserDisabled,
    ExplicitExecutableInvalid,
    AutoDiscoveryFailed,
    Available { discovery: BrowserDiscovery },
    StartupFailed(String),
}

impl BrowserAvailability {
    pub fn is_available(&self) -> bool {
        matches!(self, BrowserAvailability::Available { .. })
    }
}

#[derive(Clone, Debug)]
pub enum BrowserDiscoveryState {
    Available(BrowserDiscovery),
    NotConfigured,
    ExplicitPathInvalid { path: String },
    NotFound,
    VersionUnsupported { version: String },
}

impl BrowserDiscoveryState {
    pub fn is_available(&self) -> bool {
        matches!(self, BrowserDiscoveryState::Available { .. })
    }

    pub fn discovery(&self) -> Option<&BrowserDiscovery> {
        match self {
            BrowserDiscoveryState::Available(d) => Some(d),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_passes_validation() {
        let cfg = BrowserConfig::default();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn config_with_valid_values_passes() {
        let cfg = BrowserConfig {
            startup_timeout_ms: 5_000,
            navigation_timeout_ms: 10_000,
            post_load_wait_ms: 500,
            verification_wait_ms: 5_000,
            max_requests: 50,
            max_dom_bytes: 1_000_000,
            global_concurrency: 2,
            per_origin_concurrency: 2,
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn config_with_excessive_values_fails() {
        let cfg = BrowserConfig {
            startup_timeout_ms: MAX_STARTUP_TIMEOUT_MS + 1,
            navigation_timeout_ms: MAX_NAVIGATION_TIMEOUT_MS + 1,
            post_load_wait_ms: MAX_POST_LOAD_WAIT_MS + 1,
            verification_wait_ms: MAX_VERIFICATION_WAIT_MS + 1,
            max_requests: MAX_MAX_REQUESTS + 1,
            max_dom_bytes: MAX_MAX_DOM_BYTES + 1,
            global_concurrency: MAX_GLOBAL_CONCURRENCY + 1,
            per_origin_concurrency: MAX_PER_ORIGIN_CONCURRENCY + 1,
            ..Default::default()
        };
        let errors = cfg.validate().unwrap_err();
        assert_eq!(errors.len(), 8);
    }

    #[test]
    fn with_clamped_values_caps_excessive_config() {
        let cfg = BrowserConfig {
            startup_timeout_ms: u64::MAX,
            navigation_timeout_ms: u64::MAX,
            post_load_wait_ms: u64::MAX,
            verification_wait_ms: u64::MAX,
            max_requests: usize::MAX,
            max_dom_bytes: usize::MAX,
            global_concurrency: usize::MAX,
            per_origin_concurrency: usize::MAX,
            ..Default::default()
        };
        let clamped = cfg.with_clamped_values();
        assert_eq!(clamped.startup_timeout_ms, MAX_STARTUP_TIMEOUT_MS);
        assert_eq!(clamped.navigation_timeout_ms, MAX_NAVIGATION_TIMEOUT_MS);
        assert_eq!(clamped.post_load_wait_ms, MAX_POST_LOAD_WAIT_MS);
        assert_eq!(clamped.verification_wait_ms, MAX_VERIFICATION_WAIT_MS);
        assert_eq!(clamped.max_requests, MAX_MAX_REQUESTS);
        assert_eq!(clamped.max_dom_bytes, MAX_MAX_DOM_BYTES);
        assert_eq!(clamped.global_concurrency, MAX_GLOBAL_CONCURRENCY);
        assert_eq!(clamped.per_origin_concurrency, MAX_PER_ORIGIN_CONCURRENCY);
        assert!(clamped.validate().is_ok());
    }
}
