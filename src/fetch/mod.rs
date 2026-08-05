//! URL fetch and content extraction module.
//!
//! Fetches a single HTTP(S) URL, enforces size/time/content limits,
//! extracts readable text/metadata, and returns bounded structured output.

#[cfg(feature = "browser")]
#[allow(missing_docs)]
pub mod browser;
#[allow(missing_docs)]
pub mod cache;
pub mod client;
pub mod detect;
pub mod extract;
pub mod limits;
#[allow(missing_docs)]
pub mod origin;
/// PDF text extraction (requires the `pdf` Cargo feature).
#[cfg(feature = "pdf")]
pub mod pdf;
/// HTML structural rendering (blocks, text, markdown).
pub mod render;
/// Symbol/span-aware block expansion for `repo_fetch`.
pub mod span;
pub mod types;

#[cfg(feature = "browser")]
pub use browser::{
    browser_capability_report, browser_fetch, browser_fetch_with_policy,
    browser_result_to_response, classify_response, discover_browser, is_request_allowed,
    is_request_allowed_with_dns, parse_browser_major_version, BrowserAvailability, BrowserConfig,
    BrowserDiscovery, BrowserFamily, BrowserFetchResult, BrowserLaunchError, BrowserLifecycle,
    BrowserProfileMetadata, BrowserSource, FetchDisposition, FetchTransportKind,
    ManualInteractionReason, ManualInteractionRequired, PolicyViolation, ProfileError, ProfileLock,
    ProfileManager, ProfileResult, RenderPolicy, TransportResponse, TransportTiming,
    DEFAULT_GLOBAL_CONCURRENCY, DEFAULT_MAX_DOM_BYTES, DEFAULT_MAX_REQUESTS,
    DEFAULT_NAVIGATION_TIMEOUT_MS, DEFAULT_PER_ORIGIN_CONCURRENCY, DEFAULT_POST_LOAD_WAIT_MS,
    DEFAULT_PROFILE_PROCESS_TIMEOUT_MS, DEFAULT_STARTUP_TIMEOUT_MS, DEFAULT_VERIFICATION_WAIT_MS,
    MAX_GLOBAL_CONCURRENCY, MAX_MAX_DOM_BYTES, MAX_MAX_REQUESTS, MAX_NAVIGATION_TIMEOUT_MS,
    MAX_PER_ORIGIN_CONCURRENCY, MAX_POST_LOAD_WAIT_MS, MAX_PROFILE_PROCESS_TIMEOUT_MS,
    MAX_STARTUP_TIMEOUT_MS, MAX_VERIFICATION_WAIT_MS, PROFILE_SCHEMA_VERSION,
};
pub use cache::{
    CacheScope, CacheStatus, DerivedCacheKey, DerivedDocumentCacheEntry, FetchCache,
    FetchCacheMetadata, RawCacheKey,
};
pub use client::FetchClient;
pub use extract::{extract_content, HtmlExtractor, LinkExtractionResult};
pub use limits::{validate_fetch_target, FetchLimits};
pub use origin::{
    classify_http_status, classify_network_error, parse_retry_after, OriginController,
    OriginFailureClass, OriginKey, OriginPolicy,
};
pub use span::SelectedSpan;
pub use types::{FetchError, FetchErrorKind};
