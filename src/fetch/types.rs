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

    /// PDF support is not compiled in (the `pdf` Cargo feature is disabled).
    #[error(
        "PDF support is not compiled in; build with `cargo build --features pdf` (see docs/features.md#pdf-extraction for the `pdf` Cargo feature)"
    )]
    PdfNotCompiledIn,

    /// PDF extraction is disabled by configuration.
    #[error("PDF extraction is disabled; set [fetch].pdf_enabled = true in config")]
    PdfDisabled,

    /// PDF parse failure.
    #[error("PDF parse error: {0}")]
    PdfParseError(String),

    /// PDF decompressed content would exceed the configured bound.
    ///
    /// Raised before `lopdf` sees the document when measured Flate expansion
    /// crosses the budget, when a declared xref `/W` geometry would allocate
    /// beyond it, or when a stream uses `LZWDecode` or a width beyond eight
    /// bytes, which cannot be measured or accepted. `measured` is the accounted
    /// bytes for a measurement breach and `budget + 1` for a structural
    /// refusal.
    #[error(
        "PDF decompressed content exceeds the bound: measured {measured} bytes (budget {budget})"
    )]
    PdfDecompressionBoundExceeded {
        /// Bytes accounted for the document, or the declared allocation.
        measured: usize,
        /// Configured per-document decompressed-byte budget.
        budget: usize,
    },

    /// PDF is encrypted/password-protected.
    #[error("PDF is encrypted or password-protected; extraction is not supported")]
    PdfEncrypted,

    /// PDF has no extractable text (scanned or image-only).
    #[error("PDF has little or no extractable text; OCR is not supported")]
    PdfNoExtractableText,

    /// PDF page specification is invalid.
    #[error("invalid PDF page specification: {0}")]
    PdfPageSpecInvalid(String),

    /// Requested page numbers exceed the document's page count.
    #[error("requested pages {requested:?} exceed document page count ({total_pages})")]
    PdfPageOutOfRange {
        /// Pages that were requested but exceed the document length.
        requested: Vec<u32>,
        /// Total pages in the document.
        total_pages: usize,
    },

    /// Selected page count exceeds the configured maximum.
    #[error("selected page count ({selected}) exceeds configured maximum ({max_pages})")]
    PdfPageCapExceeded {
        /// Number of pages selected.
        selected: usize,
        /// Configured maximum.
        max_pages: usize,
    },

    /// OCR was requested but is not available in this build.
    #[error("OCR was requested but is not available; enable an OCR provider or set pdf_ocr to \"never\"")]
    PdfOcrUnavailable,

    /// Browser support is not compiled in (the `browser` Cargo feature is disabled).
    #[error("browser support is not compiled in; enable the `browser` Cargo feature")]
    BrowserNotCompiledIn,

    /// Browser rendering is disabled by configuration.
    #[error("browser rendering is disabled; set [fetch].browser.enabled = true in config")]
    BrowserDisabled,

    /// Browser executable was not found.
    #[error("no Chrome/Chromium executable found on this system")]
    BrowserNotFound,

    /// Browser launch failed.
    #[error("browser launch failed: {0}")]
    BrowserLaunchFailed(String),

    /// Browser navigation failed.
    #[error("browser navigation failed: {0}")]
    BrowserNavigationFailed(String),

    /// Browser policy violation.
    #[error("browser policy violation: {0}")]
    BrowserPolicyViolation(String),

    /// Interactive challenge detected in browser.
    #[error("interactive challenge detected; manual interaction required at {0}")]
    BrowserInteractiveChallenge(String),

    /// Browser DOM size exceeded limit.
    #[error("browser DOM size {0} exceeds limit {1}")]
    BrowserDomTooLarge(usize, usize),

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
    /// PDF not compiled in.
    PdfNotCompiledIn,
    /// PDF disabled by config.
    PdfDisabled,
    /// PDF parse error.
    PdfParseError,
    /// PDF decompressed content exceeds the bound.
    PdfDecompressionBoundExceeded,
    /// PDF encrypted.
    PdfEncrypted,
    /// PDF no extractable text.
    PdfNoExtractableText,
    /// PDF page specification invalid.
    PdfPageSpecInvalid,
    /// PDF page out of range.
    PdfPageOutOfRange,
    /// PDF page cap exceeded.
    PdfPageCapExceeded,
    /// PDF OCR unavailable.
    PdfOcrUnavailable,
    /// Browser not compiled in.
    BrowserNotCompiledIn,
    /// Browser disabled by config.
    BrowserDisabled,
    /// Browser not found.
    BrowserNotFound,
    /// Browser launch failed.
    BrowserLaunchFailed,
    /// Browser navigation failed.
    BrowserNavigationFailed,
    /// Browser policy violation.
    BrowserPolicyViolation,
    /// Browser interactive challenge.
    BrowserInteractiveChallenge,
    /// Browser DOM too large.
    BrowserDomTooLarge,
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
            FetchError::PdfNotCompiledIn => FetchErrorKind::PdfNotCompiledIn,
            FetchError::PdfDisabled => FetchErrorKind::PdfDisabled,
            FetchError::PdfParseError(_) => FetchErrorKind::PdfParseError,
            FetchError::PdfDecompressionBoundExceeded { .. } => {
                FetchErrorKind::PdfDecompressionBoundExceeded
            }
            FetchError::PdfEncrypted => FetchErrorKind::PdfEncrypted,
            FetchError::PdfNoExtractableText => FetchErrorKind::PdfNoExtractableText,
            FetchError::PdfPageSpecInvalid(_) => FetchErrorKind::PdfPageSpecInvalid,
            FetchError::PdfPageOutOfRange { .. } => FetchErrorKind::PdfPageOutOfRange,
            FetchError::PdfPageCapExceeded { .. } => FetchErrorKind::PdfPageCapExceeded,
            FetchError::PdfOcrUnavailable => FetchErrorKind::PdfOcrUnavailable,
            FetchError::BrowserNotCompiledIn => FetchErrorKind::BrowserNotCompiledIn,
            FetchError::BrowserDisabled => FetchErrorKind::BrowserDisabled,
            FetchError::BrowserNotFound => FetchErrorKind::BrowserNotFound,
            FetchError::BrowserLaunchFailed(_) => FetchErrorKind::BrowserLaunchFailed,
            FetchError::BrowserNavigationFailed(_) => FetchErrorKind::BrowserNavigationFailed,
            FetchError::BrowserPolicyViolation(_) => FetchErrorKind::BrowserPolicyViolation,
            FetchError::BrowserInteractiveChallenge(_) => {
                FetchErrorKind::BrowserInteractiveChallenge
            }
            FetchError::BrowserDomTooLarge(..) => FetchErrorKind::BrowserDomTooLarge,
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
            FetchErrorKind::PdfNotCompiledIn => "pdf_not_compiled_in",
            FetchErrorKind::PdfDisabled => "pdf_disabled",
            FetchErrorKind::PdfParseError => "pdf_parse_error",
            FetchErrorKind::PdfDecompressionBoundExceeded => "pdf_decompression_bound_exceeded",
            FetchErrorKind::PdfEncrypted => "pdf_encrypted",
            FetchErrorKind::PdfNoExtractableText => "pdf_no_extractable_text",
            FetchErrorKind::PdfPageSpecInvalid => "pdf_page_spec_invalid",
            FetchErrorKind::PdfPageOutOfRange => "pdf_page_out_of_range",
            FetchErrorKind::PdfPageCapExceeded => "pdf_page_cap_exceeded",
            FetchErrorKind::PdfOcrUnavailable => "pdf_ocr_unavailable",
            FetchErrorKind::BrowserNotCompiledIn => "browser_not_compiled_in",
            FetchErrorKind::BrowserDisabled => "browser_disabled",
            FetchErrorKind::BrowserNotFound => "browser_not_found",
            FetchErrorKind::BrowserLaunchFailed => "browser_launch_failed",
            FetchErrorKind::BrowserNavigationFailed => "browser_navigation_failed",
            FetchErrorKind::BrowserPolicyViolation => "browser_policy_violation",
            FetchErrorKind::BrowserInteractiveChallenge => "browser_interactive_challenge",
            FetchErrorKind::BrowserDomTooLarge => "browser_dom_too_large",
            FetchErrorKind::Unknown => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_decompression_bound_maps_to_its_own_kind() {
        let error = FetchError::PdfDecompressionBoundExceeded {
            measured: 4096,
            budget: 1024,
        };
        assert_eq!(
            error.error_code(),
            "pdf_decompression_bound_exceeded",
            "the breach must not be repurposed as a parse error"
        );
        assert!(matches!(
            error.kind(),
            FetchErrorKind::PdfDecompressionBoundExceeded
        ));
        assert!(error.to_string().contains("4096"));
        assert!(error.to_string().contains("1024"));
        assert_ne!(
            FetchError::PdfParseError("x".into()).error_code(),
            error.error_code()
        );
    }

    #[test]
    fn pdf_not_compiled_in_message_is_actionable() {
        let message = FetchError::PdfNotCompiledIn.to_string();
        assert!(
            message.contains("`pdf` Cargo feature"),
            "message must name the feature: {message}"
        );
        assert!(
            message.contains("cargo build --features pdf"),
            "message must name the enabling source-build command: {message}"
        );
        assert!(
            message.contains("docs/features.md#pdf-extraction"),
            "message must point at the documentation anchor: {message}"
        );
        assert_eq!(
            FetchError::PdfNotCompiledIn.error_code(),
            "pdf_not_compiled_in"
        );
    }
}
