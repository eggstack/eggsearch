//! Fetch-related error types.

use thiserror::Error;

/// Errors that can occur during a fetch operation.
#[derive(Error, Debug, Clone)]
pub enum FetchError {
    /// Invalid URL.
    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    /// Unsupported URL scheme.
    #[error("blocked URL scheme: {0} (only http/https allowed)")]
    UnsupportedScheme(String),

    /// Private network access blocked.
    #[error("private network access blocked: {0}")]
    PrivateNetworkBlocked(String),

    /// Redirect limit exceeded.
    #[error("redirect limit exceeded: {0} redirects followed")]
    RedirectLimitExceeded(usize),

    /// Redirect target blocked.
    #[error("redirect target blocked: {0}")]
    RedirectTargetBlocked(String),

    /// Invalid redirect location header.
    #[error("invalid redirect location: {0}")]
    InvalidRedirectLocation(String),

    /// Embedded credentials in URL.
    #[error("embedded credentials blocked: {0}")]
    EmbeddedCredentialsBlocked(String),

    /// URL too long.
    #[error("URL too long: {0} bytes (max {1})")]
    UrlTooLong(usize, usize),

    /// Request timeout.
    #[error("timeout after {0}ms")]
    Timeout(u64),

    /// HTTP error status.
    #[error("HTTP error: {0} {1}")]
    HttpStatus(u16, String),

    /// Content too large.
    #[error("content too large: {0} bytes (max {1})")]
    ContentTooLarge(usize, usize),

    /// Unsupported content type.
    #[error("unsupported content type: {0}")]
    UnsupportedContentType(String),

    /// Network error.
    #[error("network error: {0}")]
    NetworkError(String),

    /// Extraction failed.
    #[error("extraction failed: {0}")]
    ExtractError(String),

    /// Unknown error.
    #[error("{0}")]
    Unknown(String),
}

/// Kind of fetch error for MCP error mapping.
#[derive(Clone, Copy, Debug)]
pub enum FetchErrorKind {
    /// Invalid URL error.
    InvalidUrl,
    /// Unsupported scheme error.
    UnsupportedScheme,
    /// Private network blocked error.
    PrivateNetworkBlocked,
    /// Redirect limit exceeded error.
    RedirectLimitExceeded,
    /// Redirect target blocked error.
    RedirectTargetBlocked,
    /// Invalid redirect location error.
    InvalidRedirectLocation,
    /// Embedded credentials blocked error.
    EmbeddedCredentialsBlocked,
    /// Timeout error.
    Timeout,
    /// HTTP status error.
    HttpStatus,
    /// Content too large error.
    ContentTooLarge,
    /// Unsupported content type error.
    UnsupportedContentType,
    /// Network error.
    NetworkError,
    /// Extraction error.
    ExtractError,
    /// Unknown error.
    Unknown,
}

impl FetchError {
    /// Returns the kind of fetch error.
    pub fn kind(&self) -> FetchErrorKind {
        match self {
            FetchError::InvalidUrl(_) => FetchErrorKind::InvalidUrl,
            FetchError::UnsupportedScheme(_) => FetchErrorKind::UnsupportedScheme,
            FetchError::PrivateNetworkBlocked(_) => FetchErrorKind::PrivateNetworkBlocked,
            FetchError::RedirectLimitExceeded(_) => FetchErrorKind::RedirectLimitExceeded,
            FetchError::RedirectTargetBlocked(_) => FetchErrorKind::RedirectTargetBlocked,
            FetchError::InvalidRedirectLocation(_) => FetchErrorKind::InvalidRedirectLocation,
            FetchError::EmbeddedCredentialsBlocked(_) => FetchErrorKind::EmbeddedCredentialsBlocked,
            FetchError::UrlTooLong(..) => FetchErrorKind::InvalidUrl,
            FetchError::Timeout(_) => FetchErrorKind::Timeout,
            FetchError::HttpStatus(..) => FetchErrorKind::HttpStatus,
            FetchError::ContentTooLarge(..) => FetchErrorKind::ContentTooLarge,
            FetchError::UnsupportedContentType(_) => FetchErrorKind::UnsupportedContentType,
            FetchError::NetworkError(_) => FetchErrorKind::NetworkError,
            FetchError::ExtractError(_) => FetchErrorKind::ExtractError,
            FetchError::Unknown(_) => FetchErrorKind::Unknown,
        }
    }

    /// Returns a machine-readable error code for MCP error mapping.
    pub fn error_code(&self) -> &'static str {
        match self.kind() {
            FetchErrorKind::InvalidUrl => "invalid_url",
            FetchErrorKind::UnsupportedScheme => "unsupported_scheme",
            FetchErrorKind::PrivateNetworkBlocked => "private_network_blocked",
            FetchErrorKind::RedirectLimitExceeded => "redirect_limit_exceeded",
            FetchErrorKind::RedirectTargetBlocked => "redirect_target_blocked",
            FetchErrorKind::InvalidRedirectLocation => "invalid_redirect_location",
            FetchErrorKind::EmbeddedCredentialsBlocked => "embedded_credentials_blocked",
            FetchErrorKind::Timeout => "timeout",
            FetchErrorKind::HttpStatus => "http_status",
            FetchErrorKind::ContentTooLarge => "content_too_large",
            FetchErrorKind::UnsupportedContentType => "unsupported_content_type",
            FetchErrorKind::NetworkError => "network_error",
            FetchErrorKind::ExtractError => "extract_error",
            FetchErrorKind::Unknown => "unknown",
        }
    }
}
