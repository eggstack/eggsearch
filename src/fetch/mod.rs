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
    browser_capability_report, browser_fetch, classify_response, discover_browser,
    is_request_allowed, BrowserConfig, BrowserDiscovery, BrowserFamily, BrowserLaunchError,
    BrowserLifecycle, BrowserSource, FetchDisposition, FetchTransportKind, ManualInteractionReason,
    ManualInteractionRequired, PolicyViolation, RenderPolicy, TransportResponse, TransportTiming,
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
