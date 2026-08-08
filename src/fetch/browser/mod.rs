pub mod classify;
pub mod discover;
pub mod intercept;
pub mod lifecycle;
pub mod navigate;
pub mod profiles;
pub mod types;

pub use classify::{classify_response, FetchDisposition};
pub use discover::{browser_capability_report, discover_browser};
pub use intercept::{is_request_allowed, is_request_allowed_with_dns, PolicyViolation};
pub use lifecycle::{BrowserLaunchError, BrowserLifecycle};
pub use navigate::{
    browser_fetch, browser_fetch_with_policy, browser_result_to_response, BrowserFetchError,
    BrowserFetchResult,
};
pub use profiles::{
    parse_browser_major_version, BrowserProfileMetadata, ProfileError, ProfileLock, ProfileManager,
    ProfileResult, PROFILE_SCHEMA_VERSION,
};
pub use types::{
    BrowserAvailability, BrowserConfig, BrowserDiscovery, BrowserDiscoveryState, BrowserFamily,
    BrowserSource, FetchTransportKind, ManualInteractionReason, ManualInteractionRequired,
    RenderPolicy, TransportResponse, TransportTiming, DEFAULT_GLOBAL_CONCURRENCY,
    DEFAULT_MAX_DOM_BYTES, DEFAULT_MAX_REQUESTS, DEFAULT_NAVIGATION_TIMEOUT_MS,
    DEFAULT_PER_ORIGIN_CONCURRENCY, DEFAULT_POST_LOAD_WAIT_MS, DEFAULT_PROFILE_PROCESS_TIMEOUT_MS,
    DEFAULT_STARTUP_TIMEOUT_MS, DEFAULT_VERIFICATION_WAIT_MS, MAX_GLOBAL_CONCURRENCY,
    MAX_MAX_DOM_BYTES, MAX_MAX_REQUESTS, MAX_NAVIGATION_TIMEOUT_MS, MAX_PER_ORIGIN_CONCURRENCY,
    MAX_POST_LOAD_WAIT_MS, MAX_PROFILE_PROCESS_TIMEOUT_MS, MAX_STARTUP_TIMEOUT_MS,
    MAX_VERIFICATION_WAIT_MS,
};
