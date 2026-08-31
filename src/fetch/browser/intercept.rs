use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};
use std::time::Duration;

use tokio::time::timeout;
use url::Url;

use crate::fetch::limits::{classify_ip, is_allowed_by_policy};

const DNS_RESOLUTION_TIMEOUT: Duration = Duration::from_secs(3);

pub fn is_request_allowed(url_str: &str) -> Result<(), PolicyViolation> {
    let url = Url::parse(url_str).map_err(|_| PolicyViolation::InvalidUrl)?;

    match url.scheme() {
        "http" | "https" => {}
        _ => return Err(PolicyViolation::UnsupportedScheme(url.scheme().to_string())),
    }

    if url.username() != "" || url.password().is_some() {
        return Err(PolicyViolation::EmbeddedCredentials);
    }

    let host_str = url.host_str().ok_or(PolicyViolation::NoHost)?;

    if is_private_host(host_str) {
        return Err(PolicyViolation::PrivateNetworkTarget);
    }

    if let Ok(ip) = host_str.parse::<Ipv4Addr>() {
        if is_private_ip(IpAddr::V4(ip)) {
            return Err(PolicyViolation::PrivateNetworkTarget);
        }
    }

    let v6_candidate = host_str.strip_prefix('[').unwrap_or(host_str);
    let v6_candidate = v6_candidate.strip_suffix(']').unwrap_or(v6_candidate);
    if let Ok(ip) = v6_candidate.parse::<Ipv6Addr>() {
        if is_private_ip(IpAddr::V6(ip)) {
            return Err(PolicyViolation::PrivateNetworkTarget);
        }
    }

    Ok(())
}

pub async fn is_request_allowed_with_dns(url_str: &str) -> Result<(), PolicyViolation> {
    is_request_allowed(url_str)?;

    let url = Url::parse(url_str).map_err(|_| PolicyViolation::InvalidUrl)?;
    let host_str = url.host_str().ok_or(PolicyViolation::NoHost)?;

    if host_str.parse::<Ipv4Addr>().is_ok() || host_str.parse::<Ipv6Addr>().is_ok() {
        return Ok(());
    }

    let port = url.port().unwrap_or(match url.scheme() {
        "https" => 443,
        _ => 80,
    });

    let addr_str = format!("{host_str}:{port}");

    let addrs = timeout(
        DNS_RESOLUTION_TIMEOUT,
        tokio::task::spawn_blocking(move || {
            addr_str
                .to_socket_addrs()
                .map(|iter| iter.collect::<Vec<_>>())
        }),
    )
    .await
    .map_err(|_| PolicyViolation::DnsResolutionTimeout)?
    .map_err(|_| PolicyViolation::DnsResolutionFailed)?
    .map_err(|_| PolicyViolation::DnsResolutionFailed)?;

    for addr in addrs {
        if is_private_ip(addr.ip()) {
            return Err(PolicyViolation::ResolvedToPrivateNetwork(
                addr.ip().to_string(),
            ));
        }
    }

    Ok(())
}

fn is_private_host(host: &str) -> bool {
    let lower = host.to_lowercase();
    let stripped = lower.trim_start_matches('[').trim_end_matches(']');
    if stripped == "localhost"
        || stripped.ends_with(".localhost")
        || stripped == "127.0.0.1"
        || stripped == "::1"
        || stripped == "0.0.0.0"
        || stripped == "metadata.google.internal"
        || stripped == "169.254.169.254"
        || crate::fetch::limits::is_private_hostname(stripped)
    {
        return true;
    }

    false
}

fn is_private_ip(ip: IpAddr) -> bool {
    !is_allowed_by_policy(classify_ip(ip), false, false)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyViolation {
    InvalidUrl,
    UnsupportedScheme(String),
    EmbeddedCredentials,
    NoHost,
    PrivateNetworkTarget,
    DnsResolutionTimeout,
    DnsResolutionFailed,
    ResolvedToPrivateNetwork(String),
}

impl std::fmt::Display for PolicyViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidUrl => write!(f, "invalid URL"),
            Self::UnsupportedScheme(s) => write!(f, "unsupported scheme: {s}"),
            Self::EmbeddedCredentials => write!(f, "embedded credentials blocked"),
            Self::NoHost => write!(f, "no host in URL"),
            Self::PrivateNetworkTarget => write!(f, "private/local network target blocked"),
            Self::DnsResolutionTimeout => write!(f, "DNS resolution timed out"),
            Self::DnsResolutionFailed => write!(f, "DNS resolution failed"),
            Self::ResolvedToPrivateNetwork(addr) => {
                write!(f, "resolved to private network address: {addr}")
            }
        }
    }
}

impl std::error::Error for PolicyViolation {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_public_https() {
        assert!(is_request_allowed("https://example.com").is_ok());
    }

    #[test]
    fn allows_public_http() {
        assert!(is_request_allowed("http://example.com").is_ok());
    }

    #[test]
    fn blocks_ftp() {
        assert_eq!(
            is_request_allowed("ftp://example.com/file"),
            Err(PolicyViolation::UnsupportedScheme("ftp".into()))
        );
    }

    #[test]
    fn blocks_file_scheme() {
        assert_eq!(
            is_request_allowed("file:///etc/passwd"),
            Err(PolicyViolation::UnsupportedScheme("file".into()))
        );
    }

    #[test]
    fn blocks_localhost() {
        assert_eq!(
            is_request_allowed("http://localhost/"),
            Err(PolicyViolation::PrivateNetworkTarget)
        );
    }

    #[test]
    fn blocks_private_hostname_suffixes() {
        for host in ["service.internal", "service.private", "service.local"] {
            assert_eq!(
                is_request_allowed(&format!("http://{host}/")),
                Err(PolicyViolation::PrivateNetworkTarget)
            );
        }
    }

    #[test]
    fn blocks_loopback() {
        assert_eq!(
            is_request_allowed("http://127.0.0.1/"),
            Err(PolicyViolation::PrivateNetworkTarget)
        );
    }

    #[test]
    fn blocks_private_10() {
        assert_eq!(
            is_request_allowed("http://10.0.0.1/"),
            Err(PolicyViolation::PrivateNetworkTarget)
        );
    }

    #[test]
    fn blocks_private_192_168() {
        assert_eq!(
            is_request_allowed("http://192.168.1.1/"),
            Err(PolicyViolation::PrivateNetworkTarget)
        );
    }

    #[test]
    fn blocks_link_local() {
        assert_eq!(
            is_request_allowed("http://169.254.1.1/"),
            Err(PolicyViolation::PrivateNetworkTarget)
        );
    }

    #[test]
    fn blocks_embedded_credentials() {
        assert_eq!(
            is_request_allowed("https://user:pass@example.com/"),
            Err(PolicyViolation::EmbeddedCredentials)
        );
    }

    #[test]
    fn blocks_ipv6_loopback() {
        assert_eq!(
            is_request_allowed("http://[::1]/"),
            Err(PolicyViolation::PrivateNetworkTarget)
        );
    }

    #[test]
    fn is_private_ipv6_unwraps_ipv4_mapped() {
        let mapped_private = "::ffff:10.0.0.1".parse::<Ipv6Addr>().unwrap();
        let mapped_loopback = "::ffff:127.0.0.1".parse::<Ipv6Addr>().unwrap();
        let mapped_link_local = "::ffff:169.254.169.254".parse::<Ipv6Addr>().unwrap();
        let mapped_public = "::ffff:93.184.216.34".parse::<Ipv6Addr>().unwrap();
        assert!(is_private_ip(IpAddr::V6(mapped_private)));
        assert!(is_private_ip(IpAddr::V6(mapped_loopback)));
        assert!(is_private_ip(IpAddr::V6(mapped_link_local)));
        assert!(!is_private_ip(IpAddr::V6(mapped_public)));
    }

    #[test]
    fn blocks_ipv4_mapped_ipv6_literal() {
        assert_eq!(
            is_request_allowed("http://[::ffff:10.0.0.1]/"),
            Err(PolicyViolation::PrivateNetworkTarget)
        );
    }

    #[test]
    fn blocks_ipv4_mapped_loopback_literal() {
        assert_eq!(
            is_request_allowed("http://[::ffff:127.0.0.1]/"),
            Err(PolicyViolation::PrivateNetworkTarget)
        );
    }

    #[test]
    fn blocks_ipv6_ula_fc00_literal() {
        assert_eq!(
            is_request_allowed("http://[fc00::1]/"),
            Err(PolicyViolation::PrivateNetworkTarget)
        );
    }

    #[test]
    fn blocks_ipv6_ula_fd00_literal() {
        assert_eq!(
            is_request_allowed("http://[fd00::1]/"),
            Err(PolicyViolation::PrivateNetworkTarget)
        );
    }

    #[test]
    fn allows_public_ipv6_literal() {
        assert!(is_request_allowed("http://[2607:f8b0:4004:800::200e]/").is_ok());
    }

    #[test]
    fn blocks_ipv4_compatible_ipv6_literal() {
        assert_eq!(
            is_request_allowed("http://[::10.0.0.1]/"),
            Err(PolicyViolation::PrivateNetworkTarget)
        );
    }

    #[test]
    fn allows_public_ipv4_compatible_ipv6_literal() {
        assert!(is_request_allowed("http://[::93.184.216.34]/").is_ok());
    }

    #[test]
    fn allows_public_mapped_ipv6_literal() {
        assert!(is_request_allowed("http://[::ffff:93.184.216.34]/").is_ok());
    }

    #[test]
    fn blocks_metadata_host() {
        assert_eq!(
            is_request_allowed("http://169.254.169.254/metadata"),
            Err(PolicyViolation::PrivateNetworkTarget)
        );
    }

    #[test]
    fn blocks_cgnat_range() {
        assert_eq!(
            is_request_allowed("http://100.64.0.1/"),
            Err(PolicyViolation::PrivateNetworkTarget)
        );
    }

    #[test]
    fn blocks_documentation_range() {
        assert_eq!(
            is_request_allowed("http://192.0.2.1/"),
            Err(PolicyViolation::PrivateNetworkTarget)
        );
    }

    #[test]
    fn blocks_zero_literal_range() {
        for host in ["0.0.0.0", "0.1.2.3"] {
            assert_eq!(
                is_request_allowed(&format!("http://{host}/")),
                Err(PolicyViolation::PrivateNetworkTarget),
                "{host} should be denied"
            );
        }
    }

    #[test]
    fn blocks_reserved_v4_range() {
        for host in ["240.0.0.1", "250.1.2.3", "255.255.255.254"] {
            assert_eq!(
                is_request_allowed(&format!("http://{host}/")),
                Err(PolicyViolation::PrivateNetworkTarget),
                "{host} should be denied"
            );
        }
    }

    #[test]
    fn blocks_full_192_0_0_reserved_range() {
        for host in ["192.0.0.0", "192.0.0.1", "192.0.0.255"] {
            assert_eq!(
                is_request_allowed(&format!("http://{host}/")),
                Err(PolicyViolation::PrivateNetworkTarget),
                "{host} should be denied"
            );
        }
    }

    #[tokio::test]
    async fn dns_resolution_succeeds_for_public_host() {
        let result = is_request_allowed_with_dns("https://example.com").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn dns_blocks_localhost_resolution() {
        let result = is_request_allowed_with_dns("http://localhost/").await;
        assert!(result.is_err());
    }
}
