//! Fetch limits and URL validation.

use std::net::IpAddr;
use std::str::FromStr;

use url::Url;

use super::types::FetchError;

/// Limits for a fetch operation.
#[derive(Clone, Debug)]
pub struct FetchLimits {
    /// Maximum URL length in bytes.
    pub max_url_len: usize,
    /// Maximum content size in bytes.
    pub max_bytes: usize,
    /// Maximum character count for extracted text.
    pub max_chars_default: usize,
    /// Maximum character count cap.
    pub max_chars_cap: usize,
    /// Request timeout in milliseconds.
    pub timeout_ms: u64,
    /// Maximum redirect count.
    pub redirect_limit: usize,
    /// Whether to allow private network access.
    pub allow_private_network: bool,
    /// Whether to allow localhost.
    pub allow_localhost: bool,
}

impl Default for FetchLimits {
    fn default() -> Self {
        Self {
            max_url_len: 8192,
            max_bytes: 2_000_000,
            max_chars_default: 12000,
            max_chars_cap: 50000,
            timeout_ms: 8000,
            redirect_limit: 5,
            allow_private_network: false,
            allow_localhost: false,
        }
    }
}

/// Validates a URL for fetching.
pub fn validate_url(url_str: &str, limits: &FetchLimits) -> Result<Url, FetchError> {
    if url_str.trim().is_empty() {
        return Err(FetchError::InvalidUrl("URL must not be empty".into()));
    }

    let url =
        Url::parse(url_str).map_err(|e| FetchError::InvalidUrl(format!("invalid URL: {e}")))?;

    match url.scheme() {
        "http" | "https" => {}
        "file" => {
            return Err(FetchError::UnsupportedScheme(
                "file:// URLs are not supported".into(),
            ));
        }
        other => {
            return Err(FetchError::UnsupportedScheme(format!(
                "scheme '{}' is not supported (only http/https allowed)",
                other
            )));
        }
    }

    if url_str.len() > limits.max_url_len {
        return Err(FetchError::UrlTooLong(url_str.len(), limits.max_url_len));
    }

    if !limits.allow_localhost {
        if let Some(host) = url.host_str() {
            let host_lower = host.to_lowercase();
            if host_lower == "localhost"
                || host_lower == "127.0.0.1"
                || host_lower == "::1"
                || host_lower.starts_with("0.0.0.0")
            {
                return Err(FetchError::PrivateNetworkBlocked(format!(
                    "localhost access is disabled: {}",
                    host
                )));
            }
        }
    }

    if !limits.allow_private_network {
        if let Some(host_str) = url.host_str() {
            if let Ok(ip) = IpAddr::from_str(host_str) {
                if ip.is_loopback() {
                    return Err(FetchError::PrivateNetworkBlocked(format!(
                        "private IP access is disabled: {}",
                        ip
                    )));
                }
                if let std::net::IpAddr::V4(ipv4) = ip {
                    if ipv4.is_private() {
                        return Err(FetchError::PrivateNetworkBlocked(format!(
                            "private IP access is disabled: {}",
                            ip
                        )));
                    }
                }
            }
            if host_str.ends_with(".internal")
                || host_str.ends_with(".private")
                || host_str.ends_with(".local")
                || host_str.contains(".lan.")
                || host_str.starts_with("192.168.")
                || host_str.starts_with("10.")
            {
                return Err(FetchError::PrivateNetworkBlocked(format!(
                    "private network access is disabled: {}",
                    host_str
                )));
            }
        }
    }

    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_url_rejects_empty() {
        let limits = FetchLimits::default();
        let result = validate_url("", &limits);
        assert!(result.is_err());
    }

    #[test]
    fn validate_url_rejects_non_http() {
        let limits = FetchLimits::default();
        assert!(validate_url("file:///etc/passwd", &limits).is_err());
        assert!(validate_url("ftp://example.com", &limits).is_err());
    }

    #[test]
    fn validate_url_rejects_localhost_by_default() {
        let limits = FetchLimits::default();
        assert!(validate_url("http://localhost:8080", &limits).is_err());
        assert!(validate_url("http://127.0.0.1:8080", &limits).is_err());
    }

    #[test]
    fn validate_url_accepts_localhost_when_allowed() {
        let limits = FetchLimits {
            allow_localhost: true,
            ..Default::default()
        };
        assert!(validate_url("http://localhost:8080", &limits).is_ok());
    }

    #[test]
    fn validate_url_rejects_private_network_by_default() {
        let limits = FetchLimits::default();
        assert!(validate_url("http://192.168.1.1/", &limits).is_err());
        assert!(validate_url("http://10.0.0.1/", &limits).is_err());
    }

    #[test]
    fn validate_url_accepts_valid_https() {
        let limits = FetchLimits::default();
        assert!(validate_url("https://example.com/path?query=1", &limits).is_ok());
    }
}
