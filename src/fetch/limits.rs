//! Fetch limits and URL validation.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use std::str::FromStr;

use url::Url;

use super::types::FetchError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AddressClass {
    Loopback,
    Private,
    LinkLocal,
    CarrierGradeNat,
    Documentation,
    Multicast,
    Reserved,
    Public,
}

fn classify_ipv4(v4: Ipv4Addr) -> AddressClass {
    if v4.is_loopback() {
        return AddressClass::Loopback;
    }
    if v4.is_link_local() {
        return AddressClass::LinkLocal;
    }
    if v4.is_unspecified() {
        return AddressClass::Reserved;
    }
    let o = v4.octets();
    let octet0 = o[0];
    if octet0 == 0 {
        return AddressClass::Reserved;
    }
    if octet0 == 10 {
        return AddressClass::Private;
    }
    if octet0 == 100 && (o[1] & 0b1100_0000) == 0b0100_0000 {
        return AddressClass::CarrierGradeNat;
    }
    if octet0 == 127 {
        return AddressClass::Loopback;
    }
    if octet0 == 169 && o[1] == 254 {
        return AddressClass::LinkLocal;
    }
    if octet0 == 172 && (o[1] & 0b1111_0000) == 16 {
        return AddressClass::Private;
    }
    if octet0 == 192 && o[1] == 0 && o[2] == 0 {
        return AddressClass::Reserved;
    }
    if octet0 == 192 && o[1] == 0 && o[2] == 2 {
        return AddressClass::Documentation;
    }
    if octet0 == 192 && o[1] == 88 && o[2] == 99 {
        return AddressClass::Reserved;
    }
    if octet0 == 192 && o[1] == 168 {
        return AddressClass::Private;
    }
    if octet0 == 198 && (o[1] & 0b1111_1110) == 18 {
        return AddressClass::Reserved;
    }
    if octet0 == 198 && o[1] == 51 && o[2] == 100 {
        return AddressClass::Documentation;
    }
    if octet0 == 203 && o[1] == 0 && o[2] == 113 {
        return AddressClass::Documentation;
    }
    if (224..=239).contains(&octet0) {
        return AddressClass::Multicast;
    }
    if octet0 >= 240 {
        return AddressClass::Reserved;
    }
    AddressClass::Public
}

fn classify_ipv6(v6: Ipv6Addr) -> AddressClass {
    if v6.is_loopback() {
        return AddressClass::Loopback;
    }
    if v6.is_unspecified() {
        return AddressClass::Reserved;
    }
    if v6.is_multicast() {
        return AddressClass::Multicast;
    }
    let seg0 = v6.segments()[0];
    if (seg0 & 0xfe00) == 0xfc00 {
        return AddressClass::Private;
    }
    if (seg0 & 0xffc0) == 0xfe80 {
        return AddressClass::LinkLocal;
    }
    if let Some(v4) = ipv4_mapped_from_v6(v6) {
        return classify_ipv4(v4);
    }
    if let Some(v4) = ipv4_compatible_from_v6(v6) {
        return classify_ipv4(v4);
    }
    let seg1 = v6.segments()[1];
    if seg0 == 0x2001 && seg1 == 0x0db8 {
        return AddressClass::Documentation;
    }
    if seg0 == 0x2001 && seg1 == 0x0002 {
        return AddressClass::Reserved;
    }
    if seg0 == 0x2001 && seg1 == 0x0000 {
        return AddressClass::Reserved;
    }
    if seg0 == 0x2002 {
        return AddressClass::Reserved;
    }
    AddressClass::Public
}

pub(crate) fn classify_ip(ip: IpAddr) -> AddressClass {
    match ip {
        IpAddr::V4(v4) => classify_ipv4(v4),
        IpAddr::V6(v6) => classify_ipv6(v6),
    }
}

pub(crate) fn is_allowed_by_policy(
    class: AddressClass,
    allow_localhost: bool,
    allow_private_network: bool,
) -> bool {
    match class {
        AddressClass::Public => true,
        AddressClass::Loopback => allow_localhost,
        AddressClass::Private
        | AddressClass::LinkLocal
        | AddressClass::CarrierGradeNat
        | AddressClass::Documentation
        | AddressClass::Multicast
        | AddressClass::Reserved => allow_private_network,
    }
}

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
    ///
    /// The DNS resolution phase consumes at most `timeout_ms / 2`
    /// (with a 1500 ms floor), so a fetch with `timeout_ms = 1000`
    /// has only ~500 ms left for the HTTP round-trip after DNS.
    /// Increase `timeout_ms` if cold DNS resolvers cause spurious
    /// `DNS resolution timed out` errors under short budgets.
    pub timeout_ms: u64,
    /// Maximum redirect count.
    pub redirect_limit: usize,
    /// Whether to allow private network access.
    pub allow_private_network: bool,
    /// Whether to allow localhost.
    pub allow_localhost: bool,
    /// Whether PDF text extraction is enabled.
    pub pdf_enabled: bool,
    /// Maximum number of PDF pages to attempt extracting.
    pub pdf_max_pages: usize,
    /// Maximum characters to extract per PDF page.
    pub pdf_max_chars_per_page: usize,
    /// Maximum total characters to extract from PDF.
    pub pdf_max_total_chars: usize,
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
            pdf_enabled: false,
            pdf_max_pages: 25,
            pdf_max_chars_per_page: 12000,
            pdf_max_total_chars: 50000,
        }
    }
}

/// Validates a URL for fetching (sync, shape-level only).
///
/// Performs scheme, URL length, credential, localhost literal, and obvious
/// private-network literal checks. Does **not** perform DNS resolution — use
/// [`validate_fetch_target`] for the full validation pipeline.
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
                "scheme '{other}' is not supported (only http/https allowed)"
            )));
        }
    }

    if url_str.len() > limits.max_url_len {
        return Err(FetchError::UrlTooLong(url_str.len(), limits.max_url_len));
    }

    if !url.username().is_empty() || url.password().is_some() {
        return Err(FetchError::EmbeddedCredentialsBlocked(format!(
            "URL contains embedded credentials: {}",
            url.host_str().unwrap_or("unknown")
        )));
    }

    if let Some(host_str) = url.host_str() {
        let host_lower = host_str.to_lowercase();
        if host_lower == "localhost" {
            if !limits.allow_localhost {
                return Err(FetchError::PrivateNetworkBlocked(format!(
                    "localhost access is disabled: {host_str}"
                )));
            }
        } else {
            let ip_str = host_str
                .strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
                .unwrap_or(host_str)
                .split('%')
                .next()
                .unwrap_or(host_str);
            if let Ok(ip) = IpAddr::from_str(ip_str) {
                let class = classify_ip(ip);
                if !is_allowed_by_policy(
                    class,
                    limits.allow_localhost,
                    limits.allow_private_network,
                ) {
                    return Err(FetchError::PrivateNetworkBlocked(format!(
                        "address not allowed by policy: {ip}"
                    )));
                }
            } else if is_private_hostname(&host_lower) && !limits.allow_private_network {
                return Err(FetchError::PrivateNetworkBlocked(format!(
                    "private network access is disabled: {host_str}"
                )));
            }
        }
    }

    Ok(url)
}

/// Full validation of a fetch target: scheme, credentials, localhost,
/// DNS resolution, and IP-range checks.
///
/// This is the single canonical validation function used for both the
/// initial URL and every redirect target. It performs:
///
/// 1. Scheme check (http/https only)
/// 2. Embedded credentials rejection
/// 3. Localhost/literal private-IP rejection (unless `allow_localhost`)
/// 4. DNS resolution and IP-range validation (unless `allow_private_network`)
///
/// When DNS validation runs, the fetch client reuses the validated
/// address set to pin the outbound request to the same resolution
/// result for that attempt.
pub async fn validate_fetch_target(url: &Url, limits: &FetchLimits) -> Result<(), FetchError> {
    validate_fetch_target_with_resolved_addrs(url, limits)
        .await
        .map(|_| ())
}

pub(crate) async fn validate_fetch_target_with_resolved_addrs(
    url: &Url,
    limits: &FetchLimits,
) -> Result<Option<Vec<SocketAddr>>, FetchError> {
    // 1. Scheme check
    match url.scheme() {
        "http" | "https" => {}
        other => {
            return Err(FetchError::UnsupportedScheme(format!(
                "scheme '{other}' is not supported (only http/https allowed)"
            )));
        }
    }

    // 2. Embedded credentials
    if !url.username().is_empty() || url.password().is_some() {
        return Err(FetchError::EmbeddedCredentialsBlocked(format!(
            "URL contains embedded credentials: {}",
            url.host_str().unwrap_or("unknown")
        )));
    }

    // 3. Localhost / literal private-IP checks
    if let Some(host_str) = url.host_str() {
        let host_lower = host_str.to_lowercase();
        if host_lower == "localhost" {
            if !limits.allow_localhost {
                return Err(FetchError::PrivateNetworkBlocked(format!(
                    "localhost access is disabled: {host_str}"
                )));
            }
        } else {
            let ip_str = host_str
                .strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
                .unwrap_or(host_str)
                .split('%')
                .next()
                .unwrap_or(host_str);
            if let Ok(ip) = IpAddr::from_str(ip_str) {
                let class = classify_ip(ip);
                if !is_allowed_by_policy(
                    class,
                    limits.allow_localhost,
                    limits.allow_private_network,
                ) {
                    return Err(FetchError::PrivateNetworkBlocked(format!(
                        "address not allowed by policy: {ip}"
                    )));
                }
            } else if is_private_hostname(&host_lower) && !limits.allow_private_network {
                return Err(FetchError::PrivateNetworkBlocked(format!(
                    "private network access is disabled: {host_str}"
                )));
            }
        }
    }

    // 4. DNS resolution + IP-range validation
    let host = match url.host_str() {
        Some(h) if !h.is_empty() => h.to_string(),
        _ => return Ok(None),
    };

    let port = url.port_or_known_default().unwrap_or(match url.scheme() {
        "https" => 443,
        _ => 80,
    });

    let resolve_target = match IpAddr::from_str(&host) {
        Ok(IpAddr::V6(_)) => format!("[{host}]:{port}"),
        _ => format!("{host}:{port}"),
    };
    let dns_timeout = std::time::Duration::from_millis(limits.timeout_ms / 2)
        .max(std::time::Duration::from_millis(1500));
    let resolved = tokio::time::timeout(
        dns_timeout,
        tokio::task::spawn_blocking(move || {
            resolve_target
                .to_socket_addrs()
                .map(|it| it.collect::<Vec<_>>())
        }),
    )
    .await
    .map_err(|_| FetchError::NetworkError(format!("DNS resolution timed out for {host}")))?
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

    Ok(Some(addrs))
}

fn is_blocked_address(addr: SocketAddr, limits: &FetchLimits) -> bool {
    let class = classify_ip(addr.ip());
    !is_allowed_by_policy(class, limits.allow_localhost, limits.allow_private_network)
}

pub(crate) fn is_private_hostname(host: &str) -> bool {
    host.ends_with(".internal") || host.ends_with(".private") || host.ends_with(".local")
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

fn ipv4_compatible_from_v6(v6: Ipv6Addr) -> Option<Ipv4Addr> {
    let s = v6.segments();
    if s[0] == 0
        && s[1] == 0
        && s[2] == 0
        && s[3] == 0
        && s[4] == 0
        && s[5] == 0
        && (s[6] != 0 || s[7] != 0)
    {
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
    fn validate_url_rejects_embedded_credentials() {
        let limits = FetchLimits::default();
        let result = validate_url("http://user:pass@example.com/", &limits);
        assert!(matches!(
            result,
            Err(FetchError::EmbeddedCredentialsBlocked(_))
        ));
    }

    #[test]
    fn validate_url_rejects_ipv6_zone_id_loopback() {
        let limits = FetchLimits::default();
        let result = validate_url("http://[::1%25en0]:8080/", &limits);
        assert!(
            matches!(
                result,
                Err(FetchError::PrivateNetworkBlocked(_)) | Err(FetchError::InvalidUrl(_))
            ),
            "zone-ID loopback URL must be rejected (either as blocked or invalid URL), got: {result:?}"
        );
    }

    #[test]
    fn validate_url_accepts_valid_https() {
        let limits = FetchLimits::default();
        assert!(validate_url("https://example.com/path?query=1", &limits).is_ok());
    }

    #[test]
    fn validate_url_does_not_treat_public_hostname_prefixes_as_private_ips() {
        let limits = FetchLimits::default();
        assert!(validate_url("https://10.example.com/", &limits).is_ok());
        assert!(validate_url("https://192.168.example.com/", &limits).is_ok());
        assert!(validate_url("https://slack.lan.example.com/", &limits).is_ok());
    }

    #[tokio::test]
    async fn validate_fetch_target_allows_when_fully_open() {
        let limits = FetchLimits {
            allow_private_network: true,
            allow_localhost: true,
            ..Default::default()
        };
        let url = Url::parse("http://127.0.0.1/").unwrap();
        validate_fetch_target(&url, &limits).await.unwrap();
    }

    #[tokio::test]
    async fn validate_fetch_target_rejects_loopback_literal() {
        let limits = FetchLimits::default();
        let url = Url::parse("http://127.0.0.1:8080/").unwrap();
        let result = validate_fetch_target(&url, &limits).await;
        assert!(
            matches!(result, Err(FetchError::PrivateNetworkBlocked(_))),
            "expected private network block, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn validate_fetch_target_rejects_link_local_v4() {
        let limits = FetchLimits::default();
        let url = Url::parse("http://169.254.169.254/").unwrap();
        let result = validate_fetch_target(&url, &limits).await;
        assert!(
            matches!(result, Err(FetchError::PrivateNetworkBlocked(_))),
            "expected link-local block, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn validate_fetch_target_rejects_embedded_credentials() {
        let limits = FetchLimits::default();
        let url = Url::parse("http://user:pass@example.com/").unwrap();
        let result = validate_fetch_target(&url, &limits).await;
        assert!(
            matches!(result, Err(FetchError::EmbeddedCredentialsBlocked(_))),
            "expected embedded credentials block, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn validate_fetch_target_allows_embedded_credentials_when_no_password() {
        let limits = FetchLimits::default();
        let url = Url::parse("http://user@example.com/").unwrap();
        let result = validate_fetch_target(&url, &limits).await;
        assert!(
            matches!(result, Err(FetchError::EmbeddedCredentialsBlocked(_))),
            "expected embedded credentials block, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn validate_fetch_target_handles_v6_ula_block() {
        let limits = FetchLimits::default();
        let ula: SocketAddr = "[fc00::1]:80".parse().unwrap();
        assert!(is_blocked_address(ula, &limits));
    }

    #[tokio::test]
    async fn validate_fetch_target_handles_v6_link_local_block() {
        let limits = FetchLimits::default();
        let ll: SocketAddr = "[fe80::1]:80".parse().unwrap();
        assert!(is_blocked_address(ll, &limits));
    }

    #[tokio::test]
    async fn validate_fetch_target_handles_v4_mapped_v6_block() {
        let limits = FetchLimits::default();
        let mapped: SocketAddr = "[::ffff:10.0.0.1]:80".parse().unwrap();
        assert!(is_blocked_address(mapped, &limits));
    }

    #[tokio::test]
    async fn validate_fetch_target_handles_v4_compatible_v6_block() {
        let limits = FetchLimits::default();
        let compat: SocketAddr = "[::10.0.0.1]:80".parse().unwrap();
        assert!(is_blocked_address(compat, &limits));
    }

    #[tokio::test]
    async fn validate_fetch_target_resolves_ipv6_literal_with_port() {
        let limits = FetchLimits {
            allow_localhost: true,
            ..Default::default()
        };
        let url = Url::parse("http://[::1]:8080/").unwrap();
        let result = validate_fetch_target_with_resolved_addrs(&url, &limits).await;
        assert!(result.is_ok(), "IPv6 literal resolution failed: {result:?}");
        assert_eq!(
            result.unwrap().unwrap(),
            vec!["[::1]:8080".parse().unwrap()]
        );
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

    #[test]
    fn classify_ip_loopback_v4() {
        assert_eq!(
            classify_ip(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
            AddressClass::Loopback
        );
        assert_eq!(
            classify_ip(IpAddr::V4(Ipv4Addr::new(127, 255, 255, 255))),
            AddressClass::Loopback
        );
    }

    #[test]
    fn classify_ip_loopback_v6() {
        assert_eq!(
            classify_ip(IpAddr::V6("::1".parse().unwrap())),
            AddressClass::Loopback
        );
    }

    #[test]
    fn classify_ip_private_v4() {
        assert_eq!(
            classify_ip(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))),
            AddressClass::Private
        );
        assert_eq!(
            classify_ip(IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))),
            AddressClass::Private
        );
        assert_eq!(
            classify_ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))),
            AddressClass::Private
        );
    }

    #[test]
    fn classify_ip_link_local_v4() {
        assert_eq!(
            classify_ip(IpAddr::V4(Ipv4Addr::new(169, 254, 0, 1))),
            AddressClass::LinkLocal
        );
        assert_eq!(
            classify_ip(IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254))),
            AddressClass::LinkLocal
        );
    }

    #[test]
    fn classify_ip_cgnat() {
        assert_eq!(
            classify_ip(IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1))),
            AddressClass::CarrierGradeNat
        );
        assert_eq!(
            classify_ip(IpAddr::V4(Ipv4Addr::new(100, 127, 255, 255))),
            AddressClass::CarrierGradeNat
        );
    }

    #[test]
    fn classify_ip_documentation_v4() {
        assert_eq!(
            classify_ip(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1))),
            AddressClass::Documentation
        );
        assert_eq!(
            classify_ip(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1))),
            AddressClass::Documentation
        );
        assert_eq!(
            classify_ip(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1))),
            AddressClass::Documentation
        );
    }

    #[test]
    fn classify_ip_public_v4() {
        assert_eq!(
            classify_ip(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))),
            AddressClass::Public
        );
        assert_eq!(
            classify_ip(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))),
            AddressClass::Public
        );
    }

    #[test]
    fn classify_ip_multicast_v4() {
        assert_eq!(
            classify_ip(IpAddr::V4(Ipv4Addr::new(224, 0, 0, 1))),
            AddressClass::Multicast
        );
    }

    #[test]
    fn classify_ip_v6_ula() {
        assert_eq!(
            classify_ip(IpAddr::V6("fc00::1".parse().unwrap())),
            AddressClass::Private
        );
        assert_eq!(
            classify_ip(IpAddr::V6("fd00::1".parse().unwrap())),
            AddressClass::Private
        );
    }

    #[test]
    fn classify_ip_v4_compatible_v6_uses_embedded_v4() {
        assert_eq!(
            classify_ip(IpAddr::V6("::10.0.0.1".parse().unwrap())),
            AddressClass::Private
        );
        assert_eq!(
            classify_ip(IpAddr::V6("::93.184.216.34".parse().unwrap())),
            AddressClass::Public
        );
    }

    #[test]
    fn classify_ip_v6_link_local() {
        assert_eq!(
            classify_ip(IpAddr::V6("fe80::1".parse().unwrap())),
            AddressClass::LinkLocal
        );
    }

    #[test]
    fn classify_ip_v6_documentation() {
        assert_eq!(
            classify_ip(IpAddr::V6("2001:db8::1".parse().unwrap())),
            AddressClass::Documentation
        );
    }

    #[test]
    fn classify_ip_v6_multicast() {
        assert_eq!(
            classify_ip(IpAddr::V6("ff02::1".parse().unwrap())),
            AddressClass::Multicast
        );
    }

    #[test]
    fn classify_ip_v6_public() {
        assert_eq!(
            classify_ip(IpAddr::V6("2607:f8b0:4004:800::200e".parse().unwrap())),
            AddressClass::Public
        );
    }

    #[test]
    fn classify_ip_v4_mapped_v6() {
        let v6: Ipv6Addr = "::ffff:10.0.0.1".parse().unwrap();
        assert_eq!(classify_ip(IpAddr::V6(v6)), AddressClass::Private);
    }

    #[test]
    fn classify_ip_reserved_v4() {
        assert_eq!(
            classify_ip(IpAddr::V4(Ipv4Addr::UNSPECIFIED)),
            AddressClass::Reserved
        );
        assert_eq!(
            classify_ip(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))),
            AddressClass::Reserved
        );
        assert_eq!(
            classify_ip(IpAddr::V4(Ipv4Addr::new(192, 0, 0, 1))),
            AddressClass::Reserved
        );
    }

    #[test]
    fn is_allowed_by_policy_public_always_allowed() {
        assert!(is_allowed_by_policy(AddressClass::Public, false, false));
        assert!(is_allowed_by_policy(AddressClass::Public, true, true));
    }

    #[test]
    fn is_allowed_by_policy_loopback_only_by_localhost_flag() {
        assert!(is_allowed_by_policy(AddressClass::Loopback, true, false));
        assert!(!is_allowed_by_policy(AddressClass::Loopback, false, false));
        assert!(is_allowed_by_policy(AddressClass::Loopback, true, true));
        assert!(!is_allowed_by_policy(AddressClass::Loopback, false, true));
    }

    #[test]
    fn is_allowed_by_policy_private_only_by_private_network_flag() {
        assert!(is_allowed_by_policy(AddressClass::Private, false, true));
        assert!(!is_allowed_by_policy(AddressClass::Private, false, false));
        assert!(is_allowed_by_policy(AddressClass::Private, true, true));
        assert!(!is_allowed_by_policy(AddressClass::Private, true, false));
    }

    #[test]
    fn validate_url_loopback_all_four_combinations() {
        let url = "http://127.0.0.1:8080/";

        let r00 = validate_url(
            url,
            &FetchLimits {
                allow_localhost: false,
                allow_private_network: false,
                ..Default::default()
            },
        );
        assert!(
            r00.is_err(),
            "localhost=false,private=false should block loopback"
        );

        let r01 = validate_url(
            url,
            &FetchLimits {
                allow_localhost: false,
                allow_private_network: true,
                ..Default::default()
            },
        );
        assert!(
            r01.is_err(),
            "localhost=false,private=true should still block loopback"
        );

        let r10 = validate_url(
            url,
            &FetchLimits {
                allow_localhost: true,
                allow_private_network: false,
                ..Default::default()
            },
        );
        assert!(
            r10.is_ok(),
            "localhost=true,private=false should allow loopback"
        );

        let r11 = validate_url(
            url,
            &FetchLimits {
                allow_localhost: true,
                allow_private_network: true,
                ..Default::default()
            },
        );
        assert!(
            r11.is_ok(),
            "localhost=true,private=true should allow loopback"
        );
    }

    #[test]
    fn validate_url_private_all_four_combinations() {
        let url = "http://10.0.0.1:8080/";

        let r00 = validate_url(
            url,
            &FetchLimits {
                allow_localhost: false,
                allow_private_network: false,
                ..Default::default()
            },
        );
        assert!(
            r00.is_err(),
            "localhost=false,private=false should block private"
        );

        let r01 = validate_url(
            url,
            &FetchLimits {
                allow_localhost: false,
                allow_private_network: true,
                ..Default::default()
            },
        );
        assert!(
            r01.is_ok(),
            "localhost=false,private=true should allow private"
        );

        let r10 = validate_url(
            url,
            &FetchLimits {
                allow_localhost: true,
                allow_private_network: false,
                ..Default::default()
            },
        );
        assert!(
            r10.is_err(),
            "localhost=true,private=false should block private"
        );

        let r11 = validate_url(
            url,
            &FetchLimits {
                allow_localhost: true,
                allow_private_network: true,
                ..Default::default()
            },
        );
        assert!(
            r11.is_ok(),
            "localhost=true,private=true should allow private"
        );
    }

    #[test]
    fn validate_url_192_168_all_four_combinations() {
        let url = "http://192.168.1.1:8080/";

        let r00 = validate_url(
            url,
            &FetchLimits {
                allow_localhost: false,
                allow_private_network: false,
                ..Default::default()
            },
        );
        assert!(r00.is_err());

        let r01 = validate_url(
            url,
            &FetchLimits {
                allow_localhost: false,
                allow_private_network: true,
                ..Default::default()
            },
        );
        assert!(r01.is_ok());

        let r10 = validate_url(
            url,
            &FetchLimits {
                allow_localhost: true,
                allow_private_network: false,
                ..Default::default()
            },
        );
        assert!(r10.is_err());

        let r11 = validate_url(
            url,
            &FetchLimits {
                allow_localhost: true,
                allow_private_network: true,
                ..Default::default()
            },
        );
        assert!(r11.is_ok());
    }

    #[test]
    fn validate_url_link_local_all_four_combinations() {
        let url = "http://169.254.169.254:80/";

        let r00 = validate_url(
            url,
            &FetchLimits {
                allow_localhost: false,
                allow_private_network: false,
                ..Default::default()
            },
        );
        assert!(r00.is_err());

        let r01 = validate_url(
            url,
            &FetchLimits {
                allow_localhost: false,
                allow_private_network: true,
                ..Default::default()
            },
        );
        assert!(r01.is_ok());

        let r10 = validate_url(
            url,
            &FetchLimits {
                allow_localhost: true,
                allow_private_network: false,
                ..Default::default()
            },
        );
        assert!(r10.is_err());

        let r11 = validate_url(
            url,
            &FetchLimits {
                allow_localhost: true,
                allow_private_network: true,
                ..Default::default()
            },
        );
        assert!(r11.is_ok());
    }

    #[test]
    fn validate_url_public_always_allowed() {
        let url = "http://8.8.8.8:80/";

        let r00 = validate_url(
            url,
            &FetchLimits {
                allow_localhost: false,
                allow_private_network: false,
                ..Default::default()
            },
        );
        assert!(r00.is_ok(), "public IP should always be allowed");

        let r11 = validate_url(
            url,
            &FetchLimits {
                allow_localhost: true,
                allow_private_network: true,
                ..Default::default()
            },
        );
        assert!(r11.is_ok(), "public IP should always be allowed");
    }

    #[test]
    fn validate_url_v6_loopback_all_four_combinations() {
        let url = "http://[::1]:8080/";

        let r00 = validate_url(
            url,
            &FetchLimits {
                allow_localhost: false,
                allow_private_network: false,
                ..Default::default()
            },
        );
        assert!(r00.is_err());

        let r01 = validate_url(
            url,
            &FetchLimits {
                allow_localhost: false,
                allow_private_network: true,
                ..Default::default()
            },
        );
        assert!(r01.is_err());

        let r10 = validate_url(
            url,
            &FetchLimits {
                allow_localhost: true,
                allow_private_network: false,
                ..Default::default()
            },
        );
        assert!(r10.is_ok());

        let r11 = validate_url(
            url,
            &FetchLimits {
                allow_localhost: true,
                allow_private_network: true,
                ..Default::default()
            },
        );
        assert!(r11.is_ok());
    }

    #[test]
    fn is_blocked_address_all_four_combinations() {
        let loopback: SocketAddr = "127.0.0.1:80".parse().unwrap();
        let private: SocketAddr = "10.0.0.1:80".parse().unwrap();
        let link_local: SocketAddr = "169.254.169.254:80".parse().unwrap();
        let public: SocketAddr = "8.8.8.8:80".parse().unwrap();

        let r00 = FetchLimits {
            allow_localhost: false,
            allow_private_network: false,
            ..Default::default()
        };
        assert!(is_blocked_address(loopback, &r00));
        assert!(is_blocked_address(private, &r00));
        assert!(is_blocked_address(link_local, &r00));
        assert!(!is_blocked_address(public, &r00));

        let r01 = FetchLimits {
            allow_localhost: false,
            allow_private_network: true,
            ..Default::default()
        };
        assert!(is_blocked_address(loopback, &r01));
        assert!(!is_blocked_address(private, &r01));
        assert!(!is_blocked_address(link_local, &r01));
        assert!(!is_blocked_address(public, &r01));

        let r10 = FetchLimits {
            allow_localhost: true,
            allow_private_network: false,
            ..Default::default()
        };
        assert!(!is_blocked_address(loopback, &r10));
        assert!(is_blocked_address(private, &r10));
        assert!(is_blocked_address(link_local, &r10));
        assert!(!is_blocked_address(public, &r10));

        let r11 = FetchLimits {
            allow_localhost: true,
            allow_private_network: true,
            ..Default::default()
        };
        assert!(!is_blocked_address(loopback, &r11));
        assert!(!is_blocked_address(private, &r11));
        assert!(!is_blocked_address(link_local, &r11));
        assert!(!is_blocked_address(public, &r11));
    }

    #[test]
    fn is_blocked_address_v6_all_four_combinations() {
        let loopback: SocketAddr = "[::1]:80".parse().unwrap();
        let ula: SocketAddr = "[fc00::1]:80".parse().unwrap();
        let link_local: SocketAddr = "[fe80::1]:80".parse().unwrap();
        let public: SocketAddr = "[2607:f8b0:4004:800::200e]:80".parse().unwrap();

        let r00 = FetchLimits {
            allow_localhost: false,
            allow_private_network: false,
            ..Default::default()
        };
        assert!(is_blocked_address(loopback, &r00));
        assert!(is_blocked_address(ula, &r00));
        assert!(is_blocked_address(link_local, &r00));
        assert!(!is_blocked_address(public, &r00));

        let r01 = FetchLimits {
            allow_localhost: false,
            allow_private_network: true,
            ..Default::default()
        };
        assert!(is_blocked_address(loopback, &r01));
        assert!(!is_blocked_address(ula, &r01));
        assert!(!is_blocked_address(link_local, &r01));
        assert!(!is_blocked_address(public, &r01));

        let r10 = FetchLimits {
            allow_localhost: true,
            allow_private_network: false,
            ..Default::default()
        };
        assert!(!is_blocked_address(loopback, &r10));
        assert!(is_blocked_address(ula, &r10));
        assert!(is_blocked_address(link_local, &r10));
        assert!(!is_blocked_address(public, &r10));

        let r11 = FetchLimits {
            allow_localhost: true,
            allow_private_network: true,
            ..Default::default()
        };
        assert!(!is_blocked_address(loopback, &r11));
        assert!(!is_blocked_address(ula, &r11));
        assert!(!is_blocked_address(link_local, &r11));
        assert!(!is_blocked_address(public, &r11));
    }

    #[test]
    fn is_blocked_address_v4_mapped_v6_private() {
        let mapped: SocketAddr = "[::ffff:10.0.0.1]:80".parse().unwrap();
        let r00 = FetchLimits {
            allow_localhost: false,
            allow_private_network: false,
            ..Default::default()
        };
        assert!(is_blocked_address(mapped, &r00));

        let r01 = FetchLimits {
            allow_localhost: false,
            allow_private_network: true,
            ..Default::default()
        };
        assert!(!is_blocked_address(mapped, &r01));
    }
}
