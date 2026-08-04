pub mod classify;
pub mod discover;
pub mod intercept;
pub mod lifecycle;
pub mod navigate;
pub mod types;

pub use classify::{classify_response, FetchDisposition};
pub use discover::{browser_capability_report, discover_browser};
pub use intercept::{is_request_allowed, PolicyViolation};
pub use lifecycle::{BrowserLaunchError, BrowserLifecycle};
pub use navigate::{browser_fetch, BrowserFetchError};
pub use types::{
    BrowserConfig, BrowserDiscovery, BrowserFamily, BrowserSource, FetchTransportKind,
    ManualInteractionReason, ManualInteractionRequired, RenderPolicy, TransportResponse,
    TransportTiming,
};
