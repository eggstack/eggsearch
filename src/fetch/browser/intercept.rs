use std::net::{Ipv4Addr, Ipv6Addr};

use url::Url;

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
        if is_private_ipv4(ip) {
            return Err(PolicyViolation::PrivateNetworkTarget);
        }
    }

    if let Ok(ip) = host_str.parse::<Ipv6Addr>() {
        if is_private_ipv6(ip) {
            return Err(PolicyViolation::PrivateNetworkTarget);
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
    {
        return true;
    }

    false
}

fn is_private_ipv4(ip: Ipv4Addr) -> bool {
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (ip.octets()[0] == 100 && (ip.octets()[1] & 0xC0) == 64)
        || (ip.octets()[0] == 192
            && ip.octets()[1] == 0
            && ip.octets()[2] == 0
            && ip.octets()[3] == 1)
        || (ip.octets()[0] == 192 && ip.octets()[1] == 88 && ip.octets()[2] == 99)
        || (ip.octets()[0] == 198 && (ip.octets()[1] == 18 || ip.octets()[1] == 19))
        || (ip.octets()[0] == 198 && ip.octets()[1] == 51 && ip.octets()[2] == 100)
        || (ip.octets()[0] == 203 && ip.octets()[1] == 0 && ip.octets()[2] == 113)
        || (ip.octets()[0] == 192 && ip.octets()[1] == 0 && ip.octets()[2] == 2)
}

fn is_private_ipv6(ip: Ipv6Addr) -> bool {
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_unicast_link_local()
        || ip.octets()[0] == 0xfe && (ip.octets()[1] & 0xC0) == 0xC0
        || ip.segments()[0] == 0x2001 && ip.segments()[1] == 0xdb8
        || ip.is_multicast()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyViolation {
    InvalidUrl,
    UnsupportedScheme(String),
    EmbeddedCredentials,
    NoHost,
    PrivateNetworkTarget,
}

impl std::fmt::Display for PolicyViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidUrl => write!(f, "invalid URL"),
            Self::UnsupportedScheme(s) => write!(f, "unsupported scheme: {s}"),
            Self::EmbeddedCredentials => write!(f, "embedded credentials blocked"),
            Self::NoHost => write!(f, "no host in URL"),
            Self::PrivateNetworkTarget => write!(f, "private/local network target blocked"),
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
}
