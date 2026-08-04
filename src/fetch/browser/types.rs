use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::classify::FetchDisposition;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
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
}

fn default_render_policy() -> RenderPolicy {
    RenderPolicy::HttpOnly
}
fn default_startup_timeout_ms() -> u64 {
    10_000
}
fn default_navigation_timeout_ms() -> u64 {
    20_000
}
fn default_post_load_wait_ms() -> u64 {
    1_500
}
fn default_verification_wait_ms() -> u64 {
    10_000
}
fn default_max_requests() -> usize {
    100
}
fn default_max_dom_bytes() -> usize {
    4_000_000
}
fn default_global_concurrency() -> usize {
    1
}
fn default_per_origin_concurrency() -> usize {
    1
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
            startup_timeout_ms: default_startup_timeout_ms(),
            navigation_timeout_ms: default_navigation_timeout_ms(),
            post_load_wait_ms: default_post_load_wait_ms(),
            verification_wait_ms: default_verification_wait_ms(),
            max_requests: default_max_requests(),
            max_dom_bytes: default_max_dom_bytes(),
            global_concurrency: default_global_concurrency(),
            per_origin_concurrency: default_per_origin_concurrency(),
            block_media: default_block_media(),
        }
    }
}

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
