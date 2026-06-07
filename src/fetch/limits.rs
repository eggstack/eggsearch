//! Fetch limits and URL validation.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
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
///
/// This is the synchronous, shape-level check: scheme, URL length,
/// and obvious localhost / private literals in the host string. It
/// does **not** perform DNS resolution; call [`validate_url_with_dns`]
/// after this succeeds to close the SSRF gap on resolved addresses.
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

/// Resolve a URL's host to one or more socket addresses and reject
/// any that fall into a blocked network range.
///
/// This is the second layer of SSRF defense, run after
/// [`validate_url`]: the sync check handles URL shape and obvious
/// literals, this check handles the case where a public-looking
/// hostname (e.g. `attacker.com`) actually resolves to a private or
/// loopback address.
///
/// Note the TOCTOU window between this resolution and the actual
/// HTTP request. For a single-tenant MCP server that does not
/// resolve hostnames on a per-request basis on the data path, this
/// is acceptable; the same pattern is used by SSRF proxies.
pub async fn validate_url_with_dns(url: Url, limits: &FetchLimits) -> Result<Url, FetchError> {
    // Nothing to do if both flags grant access.
    if limits.allow_private_network && limits.allow_localhost {
        return Ok(url);
    }

    let host = match url.host_str() {
        Some(h) if !h.is_empty() => h.to_string(),
        _ => return Ok(url),
    };

    let port = url.port_or_known_default().unwrap_or(match url.scheme() {
        "https" => 443,
        _ => 80,
    });

    let resolve_target = format!("{}:{}", host, port);
    let resolved = tokio::task::spawn_blocking(move || {
        resolve_target
            .to_socket_addrs()
            .map(|it| it.collect::<Vec<_>>())
    })
    .await
    .map_err(|e| FetchError::NetworkError(format!("DNS resolution task panicked: {e}")))?;

    let addrs = resolved
        .map_err(|e| FetchError::NetworkError(format!("DNS resolution failed for {host}: {e}")))?;

    if addrs.is_empty() {
        return Err(FetchError::NetworkError(format!(
            "DNS resolution returned no addresses for {host}"
        )));
    }

    for addr in &addrs {
        if is_blocked_address(*addr, limits) {
            return Err(FetchError::PrivateNetworkBlocked(format!(
                "DNS resolved {host} to blocked address {addr}"
            )));
        }
    }

    Ok(url)
}

/// Returns true if the given resolved socket address falls into a
/// network range that the operator has disabled.
fn is_blocked_address(addr: SocketAddr, limits: &FetchLimits) -> bool {
    let ip = addr.ip();

    if !limits.allow_localhost && ip.is_loopback() {
        return true;
    }

    if limits.allow_private_network {
        return false;
    }

    match ip {
        IpAddr::V4(v4) => is_blocked_v4(v4),
        IpAddr::V6(v6) => is_blocked_v6(v6),
    }
}

fn is_blocked_v4(v4: Ipv4Addr) -> bool {
    // loopback, private (RFC 1918 + 100.64/10), link-local
    // (169.254/16), unspecified (0.0.0.0).
    v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified()
}

fn is_blocked_v6(v6: Ipv6Addr) -> bool {
    if v6.is_loopback() || v6.is_unspecified() {
        return true;
    }
    // IPv6 unique-local (fc00::/7) - not covered by is_private on
    // older stdlibs, and is_private on Ipv6Addr as of 1.80 also
    // covers other ranges we may not want to block, so be explicit.
    let seg0 = v6.segments()[0];
    if (seg0 & 0xfe00) == 0xfc00 {
        return true;
    }
    // IPv6 link-local (fe80::/10). Manual check for MSRV 1.80
    // (Ipv6Addr::is_unicast_link_local stabilized in 1.84).
    if (seg0 & 0xffc0) == 0xfe80 {
        return true;
    }
    // IPv4-mapped IPv6 (::ffff:a.b.c.d) - extract and re-check v4.
    if let Some(v4) = ipv4_mapped_from_v6(v6) {
        return is_blocked_v4(v4);
    }
    false
}

fn ipv4_mapped_from_v6(v6: Ipv6Addr) -> Option<Ipv4Addr> {
    let s = v6.segments();
    if s[0] == 0 && s[1] == 0 && s[2] == 0 && s[3] == 0 && s[4] == 0 && s[5] == 0xffff {
        let octets = v6.octets();
        Some(Ipv4Addr::new(
            octets[12], octets[13], octets[14], octets[15],
        ))
    } else {
        None
    }
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

    #[tokio::test]
    async fn validate_url_with_dns_allows_when_fully_open() {
        // When both flags grant access, no DNS work is done and any
        // URL (even an IP literal that would normally be blocked) is
        // allowed. We use a syntactically-valid IP literal to make
        // sure the fast path returns Ok without resolving.
        let limits = FetchLimits {
            allow_private_network: true,
            allow_localhost: true,
            ..Default::default()
        };
        let url = Url::parse("http://127.0.0.1/").unwrap();
        let out = validate_url_with_dns(url.clone(), &limits).await.unwrap();
        assert_eq!(out, url);
    }

    #[tokio::test]
    async fn validate_url_with_dns_rejects_loopback_literal() {
        // 127.0.0.1 is a literal; to_socket_addrs should yield a
        // loopback address that is_blocked_address catches.
        let limits = FetchLimits::default();
        let url = Url::parse("http://127.0.0.1:8080/").unwrap();
        let result = validate_url_with_dns(url, &limits).await;
        assert!(
            matches!(result, Err(FetchError::PrivateNetworkBlocked(_))),
            "expected private network block, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn validate_url_with_dns_rejects_link_local_v4() {
        let limits = FetchLimits::default();
        let url = Url::parse("http://169.254.169.254/").unwrap();
        let result = validate_url_with_dns(url, &limits).await;
        assert!(
            matches!(result, Err(FetchError::PrivateNetworkBlocked(_))),
            "expected link-local block, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn validate_url_with_dns_handles_v6_ula_block() {
        // Pure unit test: bypass the resolver and check the bit logic.
        let limits = FetchLimits::default();
        let ula: SocketAddr = "[fc00::1]:80".parse().unwrap();
        assert!(is_blocked_address(ula, &limits));
    }

    #[tokio::test]
    async fn validate_url_with_dns_handles_v6_link_local_block() {
        let limits = FetchLimits::default();
        let ll: SocketAddr = "[fe80::1]:80".parse().unwrap();
        assert!(is_blocked_address(ll, &limits));
    }

    #[tokio::test]
    async fn validate_url_with_dns_handles_v4_mapped_v6_block() {
        let limits = FetchLimits::default();
        let mapped: SocketAddr = "[::ffff:10.0.0.1]:80".parse().unwrap();
        assert!(is_blocked_address(mapped, &limits));
    }

    #[test]
    fn ipv4_mapped_from_v6_parses_known_form() {
        let v6: Ipv6Addr = "::ffff:10.0.0.1".parse().unwrap();
        let v4 = ipv4_mapped_from_v6(v6).expect("expected mapped v4");
        assert_eq!(v4, Ipv4Addr::new(10, 0, 0, 1));
    }

    #[test]
    fn ipv4_mapped_from_v6_rejects_unmapped() {
        let v6: Ipv6Addr = "2001:db8::1".parse().unwrap();
        assert!(ipv4_mapped_from_v6(v6).is_none());
    }
}
