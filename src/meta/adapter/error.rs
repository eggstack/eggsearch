use crate::core::provider::ProviderSkipCode;
use crate::meta::engines::error::EngineError;

/// Coarse error class for provider failures. Exposed via `provider_status`
/// and the `web_search` tool's `providers_failed` field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorClass {
    /// The engine did not respond within the per-engine timeout.
    Timeout,
    /// The engine responded with a non-2xx HTTP status.
    HttpStatus,
    /// The engine responded but the HTML could not be parsed.
    ParseError,
    /// A network-level error (DNS, TLS, connection reset, etc.).
    NetworkError,
    /// The engine returned HTTP 429 (rate-limited).
    RateLimited,
    /// The provider task panicked during dispatch.
    Panic,
    /// Unclassified failure.
    Unknown,
}

impl ErrorClass {
    /// Stable snake-case string form.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::HttpStatus => "http_status",
            Self::ParseError => "parse_error",
            Self::NetworkError => "network_error",
            Self::RateLimited => "rate_limited",
            Self::Panic => "panic",
            Self::Unknown => "unknown",
        }
    }
}

pub(crate) fn classify(err: &EngineError) -> ErrorClass {
    use EngineError::*;
    match err {
        Timeout { .. } => ErrorClass::Timeout,
        BadStatus { status, .. } if *status == 429 => ErrorClass::RateLimited,
        BadStatus { .. } => ErrorClass::HttpStatus,
        ParseFailed { .. } => ErrorClass::ParseError,
        NetworkError { reason, .. } if reason.contains("panicked during dispatch") => {
            ErrorClass::Panic
        }
        Http { .. } | NetworkError { .. } => ErrorClass::NetworkError,
        Unsupported { .. } => ErrorClass::Unknown,
    }
}

pub(crate) fn provider_skip_reason(skip_code: Option<ProviderSkipCode>) -> Option<String> {
    skip_code.map(|code| format!("[{}] {}", code.as_str(), code.display_name()))
}

pub(crate) fn known_provider_skip_code(
    id: &str,
    is_enabled: bool,
    configured: bool,
    searxng_configured: bool,
) -> Option<ProviderSkipCode> {
    if is_enabled && configured {
        return None;
    }
    Some(if !is_enabled {
        ProviderSkipCode::DisabledByUser
    } else if id == "local_workspace" {
        ProviderSkipCode::MissingLocalBackend
    } else if crate::core::provider::is_api_provider(id) {
        ProviderSkipCode::MissingApiKey
    } else if id.contains("searxng") && !searxng_configured {
        ProviderSkipCode::MissingSearxngConfig
    } else {
        ProviderSkipCode::Unknown
    })
}
