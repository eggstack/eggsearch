use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Policy controlling which forge endpoint addresses and schemes are permitted.
#[derive(Debug, Clone)]
pub struct ForgeEndpointPolicy {
    /// Whether to allow loopback addresses (localhost, 127.0.0.1, ::1).
    pub allow_loopback: bool,
    /// Whether to allow private network addresses (RFC 1918, ULA, etc.).
    pub allow_private_network: bool,
    /// Whether to require HTTPS for all endpoints.
    pub require_https: bool,
}

impl Default for ForgeEndpointPolicy {
    fn default() -> Self {
        Self {
            allow_loopback: false,
            allow_private_network: false,
            require_https: true,
        }
    }
}

/// Identifies the type of forge HTTP request for budget tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForgeRequestKind {
    /// Commit SHA resolution request.
    CommitResolution,
    /// Tree page retrieval request.
    TreePage,
    /// Contents API fallback request.
    ContentsFallback,
    /// Repository metadata request (default branch, project info).
    RepositoryMetadata,
    /// Error response body preview.
    ErrorBody,
}

/// Validate a user-supplied base URL for safety.
///
/// Ensures the URL uses HTTPS, does not point to localhost, loopback,
/// or private IP ranges, does not contain embedded credentials, and
/// (when `api_key` is provided) rejects plain HTTP.
/// Returns `Ok(())` if valid, or an error message.
pub fn validate_base_url(
    url: &str,
    api_key: Option<&str>,
    policy: &ForgeEndpointPolicy,
) -> Result<(), String> {
    let host_to_resolve = validate_base_url_common(url, api_key, policy)?;
    if let Some(host) = host_to_resolve {
        let addrs = std::net::ToSocketAddrs::to_socket_addrs(&host)
            .map_err(|e| format!("DNS resolution failed for {host}: {e}"))?;
        validate_resolved_addresses(addrs, policy)?;
    }
    Ok(())
}

pub(super) async fn validate_base_url_async(
    url: &str,
    api_key: Option<&str>,
    policy: &ForgeEndpointPolicy,
) -> Result<(), String> {
    let host_to_resolve = validate_base_url_common(url, api_key, policy)?;
    if let Some(host) = host_to_resolve {
        let addrs = tokio::net::lookup_host(&host)
            .await
            .map_err(|e| format!("DNS resolution failed for {host}: {e}"))?;
        validate_resolved_addresses(addrs, policy)?;
    }
    Ok(())
}

fn validate_base_url_common(
    url: &str,
    api_key: Option<&str>,
    policy: &ForgeEndpointPolicy,
) -> Result<Option<String>, String> {
    let parsed = url
        .parse::<url::Url>()
        .map_err(|e| format!("invalid base URL: {e}"))?;
    if parsed.scheme() != "https" && parsed.scheme() != "http" {
        return Err(format!(
            "base URL must use http or https, got: {}",
            parsed.scheme()
        ));
    }
    if parsed.username() != "" || parsed.password().is_some() {
        return Err("base URL must not contain embedded credentials".into());
    }
    if let Some(host) = parsed.host_str() {
        let is_loopback = is_loopback_addr(host);

        if parsed.scheme() == "http" {
            if is_loopback {
                if !policy.allow_loopback {
                    return Err(format!("base URL must not point to localhost: {host}"));
                }
                if api_key.is_some() {
                    return Err("credential-bearing endpoint must use HTTPS".into());
                }
            } else {
                if api_key.is_some() {
                    return Err("credential-bearing endpoint must use HTTPS".into());
                }
                if policy.require_https {
                    return Err("base URL must use HTTPS per policy".into());
                }
                if let Some(ip) = parse_literal_ip(host) {
                    classify_and_reject_address(ip, policy)?;
                } else {
                    let port = parsed.port().unwrap_or(80);
                    return Ok(Some(format!("{host}:{port}")));
                }
            }
        } else {
            if is_loopback && !policy.allow_loopback {
                return Err(format!(
                    "HTTPS base URL must not point to localhost: {host}"
                ));
            }
            if !is_loopback {
                if let Some(ip) = parse_literal_ip(host) {
                    classify_and_reject_address(ip, policy)?;
                } else {
                    return Ok(Some(format!("{host}:443")));
                }
            }
        }
    }
    Ok(None)
}

fn validate_resolved_addresses(
    addrs: impl IntoIterator<Item = std::net::SocketAddr>,
    policy: &ForgeEndpointPolicy,
) -> Result<(), String> {
    for addr in addrs {
        match addr {
            std::net::SocketAddr::V4(v4) => {
                classify_and_reject_address(IpAddr::V4(*v4.ip()), policy)?;
            }
            std::net::SocketAddr::V6(v6) => {
                classify_and_reject_address(IpAddr::V6(*v6.ip()), policy)?;
            }
        }
    }
    Ok(())
}

fn parse_literal_ip(host: &str) -> Option<IpAddr> {
    if let Ok(ip) = host.parse::<Ipv4Addr>() {
        return Some(IpAddr::V4(ip));
    }
    let inner = if host.starts_with('[') && host.ends_with(']') {
        &host[1..host.len() - 1]
    } else {
        host
    };
    if let Ok(ip) = inner.parse::<Ipv6Addr>() {
        return Some(IpAddr::V6(ip));
    }
    None
}

fn classify_and_reject_address(ip: IpAddr, policy: &ForgeEndpointPolicy) -> Result<(), String> {
    let class = match ip {
        IpAddr::V4(v4) => classify_ipv4_forge(v4),
        IpAddr::V6(v6) => classify_ipv6_forge(v6),
    };
    match class {
        ForgeAddressClass::Loopback if !policy.allow_loopback => Err(format!(
            "resolved address {ip} is loopback, rejected by policy"
        )),
        ForgeAddressClass::Private | ForgeAddressClass::LinkLocal
            if !policy.allow_private_network =>
        {
            Err(format!(
                "resolved address {ip} is private/link-local, rejected by policy"
            ))
        }
        ForgeAddressClass::Documentation | ForgeAddressClass::Reserved => Err(format!(
            "resolved address {ip} is reserved/documentation, rejected"
        )),
        _ => Ok(()),
    }
}

/// Classification of an IP address for forge endpoint safety.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForgeAddressClass {
    /// Loopback address (127.0.0.0/8, ::1).
    Loopback,
    /// Private network address (RFC 1918, ULA).
    Private,
    /// Link-local address (169.254.0.0/16, fe80::/10).
    LinkLocal,
    /// Documentation address (192.0.2.0/24, 198.51.100.0/24, 203.0.113.0/24, 2001:db8::/32).
    Documentation,
    /// Reserved address (multicast, unspecified, etc.).
    Reserved,
    /// Public routable address.
    Public,
}

impl ForgeAddressClass {
    /// Stable string representation for telemetry.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Loopback => "loopback",
            Self::Private => "private",
            Self::LinkLocal => "link_local",
            Self::Documentation => "documentation",
            Self::Reserved => "reserved",
            Self::Public => "public",
        }
    }
}

/// Classify an IPv6 address for forge endpoint safety.
pub fn classify_ipv6_forge(v6: Ipv6Addr) -> ForgeAddressClass {
    if v6.is_loopback() {
        return ForgeAddressClass::Loopback;
    }
    if v6.is_unspecified() {
        return ForgeAddressClass::Reserved;
    }
    if v6.is_multicast() {
        return ForgeAddressClass::Reserved;
    }
    let seg0 = v6.segments()[0];
    if (seg0 & 0xfe00) == 0xfc00 {
        return ForgeAddressClass::Private;
    }
    if (seg0 & 0xffc0) == 0xfe80 {
        return ForgeAddressClass::LinkLocal;
    }
    if let Some(v4) = ipv4_mapped_from_v6_forge(v6) {
        return classify_ipv4_forge(v4);
    }
    let seg1 = v6.segments()[1];
    if seg0 == 0x2001 && seg1 == 0x0db8 {
        return ForgeAddressClass::Documentation;
    }
    if seg0 == 0x2001 && (seg1 == 0x0002 || seg1 == 0x0000) {
        return ForgeAddressClass::Reserved;
    }
    if seg0 == 0x2002 {
        return ForgeAddressClass::Reserved;
    }
    ForgeAddressClass::Public
}

/// Classify an IPv4 address for forge endpoint safety.
pub fn classify_ipv4_forge(v4: Ipv4Addr) -> ForgeAddressClass {
    if v4.is_loopback() {
        return ForgeAddressClass::Loopback;
    }
    if v4.is_link_local() {
        return ForgeAddressClass::LinkLocal;
    }
    if v4.is_unspecified() {
        return ForgeAddressClass::Reserved;
    }
    let o = v4.octets();
    let octet0 = o[0];
    if octet0 == 0 || octet0 == 127 {
        return ForgeAddressClass::Loopback;
    }
    if octet0 == 10 {
        return ForgeAddressClass::Private;
    }
    if octet0 == 100 && (o[1] & 0b1100_0000) == 0b0100_0000 {
        return ForgeAddressClass::Reserved;
    }
    if octet0 == 169 && o[1] == 254 {
        return ForgeAddressClass::LinkLocal;
    }
    if octet0 == 172 && (o[1] & 0b1111_0000) == 16 {
        return ForgeAddressClass::Private;
    }
    if octet0 == 192 && o[1] == 168 {
        return ForgeAddressClass::Private;
    }
    if octet0 == 192 && o[1] == 0 && o[2] == 2 {
        return ForgeAddressClass::Documentation;
    }
    if octet0 == 198 && o[1] == 51 && o[2] == 100 {
        return ForgeAddressClass::Documentation;
    }
    if octet0 == 203 && o[1] == 0 && o[2] == 113 {
        return ForgeAddressClass::Documentation;
    }
    if (224..=239).contains(&octet0) {
        return ForgeAddressClass::Reserved;
    }
    if octet0 >= 240 {
        return ForgeAddressClass::Reserved;
    }
    ForgeAddressClass::Public
}

fn ipv4_mapped_from_v6_forge(v6: Ipv6Addr) -> Option<Ipv4Addr> {
    match v6.to_ipv4_mapped() {
        Some(v4) if !v4.is_unspecified() => Some(v4),
        _ => None,
    }
}

pub(super) fn extract_host(url: &str) -> Option<String> {
    url.parse::<url::Url>()
        .ok()
        .and_then(|u| u.host_str().map(String::from))
}

pub(super) async fn classify_host_from_url(url: &str) -> Option<String> {
    let parsed = url.parse::<url::Url>().ok()?;
    let host = parsed.host_str()?;
    if let Some(ip) = parse_literal_ip(host) {
        let class = match ip {
            IpAddr::V4(v4) => classify_ipv4_forge(v4),
            IpAddr::V6(v6) => classify_ipv6_forge(v6),
        };
        return Some(class.as_str().to_string());
    }
    let addrs = tokio::net::lookup_host(format!("{host}:443")).await.ok()?;
    for addr in addrs {
        let class = match addr {
            std::net::SocketAddr::V4(v4) => classify_ipv4_forge(*v4.ip()),
            std::net::SocketAddr::V6(v6) => classify_ipv6_forge(*v6.ip()),
        };
        if class != ForgeAddressClass::Public {
            return Some(class.as_str().to_string());
        }
    }
    Some(ForgeAddressClass::Public.as_str().to_string())
}

pub(super) fn encode_url_component(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

pub(super) fn is_loopback_addr(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    let Some(ip) = parse_literal_ip(host) else {
        return false;
    };
    match ip {
        IpAddr::V4(v4) => {
            matches!(classify_ipv4_forge(v4), ForgeAddressClass::Loopback) || v4.is_unspecified()
        }
        IpAddr::V6(v6) => {
            matches!(classify_ipv6_forge(v6), ForgeAddressClass::Loopback) || v6.is_unspecified()
        }
    }
}
