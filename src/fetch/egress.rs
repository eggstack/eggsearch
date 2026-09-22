//! Optional listener-free outbound proxy-chain route beneath eggfetch.
//!
//! Eggfetch remains the HTTP, pooling, destination-TLS, decompression,
//! redirect-mechanics, and deadline owner. Eggress owns only physical TCP
//! proxy-hop establishment through [`EggressDialer`]. Dynamic SSRF-pinned
//! fetch paths never use this module.

use crate::core::config::EgressSection;

#[cfg(feature = "egress")]
use std::sync::Arc;

#[cfg(feature = "egress")]
use eggfetch_core::{DialError, DialErrorKind, DialFuture, DialStream, DialTarget, Dialer};

#[cfg(feature = "egress")]
struct EgressStream {
    inner: eggress_core::BoxStream,
}

#[cfg(feature = "egress")]
impl tokio::io::AsyncRead for EgressStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

#[cfg(feature = "egress")]
impl tokio::io::AsyncWrite for EgressStream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// Eggfetch [`Dialer`] that establishes TCP through an Eggress proxy chain.
#[cfg(feature = "egress")]
#[derive(Clone)]
pub struct EggressDialer {
    connector: Arc<eggress_outbound::OutboundConnector>,
    redacted: String,
}

#[cfg(feature = "egress")]
impl std::fmt::Debug for EggressDialer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EggressDialer")
            .field("route", &self.redacted)
            .finish()
    }
}

#[cfg(feature = "egress")]
impl EggressDialer {
    /// Build a dialer over an already-validated connector.
    pub fn new(connector: Arc<eggress_outbound::OutboundConnector>, redacted: String) -> Self {
        Self {
            connector,
            redacted,
        }
    }

    /// Redacted route description without credentials.
    pub fn redacted_route(&self) -> &str {
        &self.redacted
    }
}

/// Map a typed Eggress failure kind into an eggfetch dial error kind.
#[cfg(feature = "egress")]
pub fn map_egress_failure(kind: eggress_outbound::OutboundConnectErrorKind) -> DialErrorKind {
    use eggress_outbound::OutboundConnectErrorKind as Kind;
    match kind {
        Kind::Timeout => DialErrorKind::Timeout,
        Kind::Authentication => DialErrorKind::Authentication,
        Kind::Policy => DialErrorKind::Rejected,
        Kind::Dns | Kind::ConnectionRefused | Kind::NetworkUnreachable | Kind::HostUnreachable => {
            DialErrorKind::Connection
        }
        Kind::Tls | Kind::Protocol | Kind::Other => DialErrorKind::Other,
        _ => DialErrorKind::Other,
    }
}

#[cfg(feature = "egress")]
impl Dialer for EggressDialer {
    fn dial(&self, target: DialTarget) -> DialFuture<'_> {
        let connector = self.connector.clone();
        Box::pin(async move {
            match connector
                .connect_tcp_detailed(target.host(), target.port())
                .await
            {
                Ok((stream, _)) => {
                    let wrapped = EgressStream { inner: stream };
                    let boxed: DialStream = Box::new(wrapped);
                    Ok(boxed)
                }
                Err(error) => {
                    let kind = map_egress_failure(error.kind());
                    Err(DialError::with_source(kind, error.to_string(), error))
                }
            }
        })
    }
}

#[cfg(feature = "egress")]
fn protocol_for_scheme(scheme: &str) -> Result<eggress_uri::ProtocolSpec, String> {
    match scheme {
        "http" => Ok(eggress_uri::ProtocolSpec::Http),
        "httponly" => Ok(eggress_uri::ProtocolSpec::HttpOnly),
        "socks4" => Ok(eggress_uri::ProtocolSpec::Socks4),
        "socks5" => Ok(eggress_uri::ProtocolSpec::Socks5),
        other => Err(format!(
            "unsupported egress scheme '{other}'; expected one of: http, socks4, socks5"
        )),
    }
}

/// Build a validated Eggress connector from operator configuration.
#[cfg(feature = "egress")]
pub fn build_connector(
    section: &EgressSection,
) -> anyhow::Result<Arc<eggress_outbound::OutboundConnector>> {
    if !section.is_routed() {
        anyhow::bail!("egress route is not enabled; no proxy hops configured");
    }
    let mut hops = Vec::with_capacity(section.hops.len());
    for hop in &section.hops {
        let protocol = protocol_for_scheme(hop.scheme.as_str()).map_err(|e| anyhow::anyhow!(e))?;
        let credentials = match (&hop.username, &hop.password_env) {
            (Some(username), Some(env)) => {
                let password = std::env::var(env).map_err(|_| {
                    anyhow::anyhow!("egress proxy credential env '{env}' is missing or unreadable")
                })?;
                if password.is_empty() {
                    anyhow::bail!("egress proxy credential env '{env}' is empty");
                }
                Some(eggress_uri::CredentialSpec {
                    username: username.clone(),
                    password,
                })
            }
            (Some(_), None) => {
                anyhow::bail!(
                    "egress hop '{}:{}' sets username without password_env; refusing to use incomplete credentials",
                    hop.host,
                    hop.port
                )
            }
            (None, Some(_)) => {
                anyhow::bail!("egress hop password_env requires a username");
            }
            (None, None) => None,
        };
        hops.push(eggress_uri::ProxyHopSpec {
            protocols: vec![protocol],
            endpoint: eggress_uri::EndpointSpec {
                host: hop.host.clone(),
                port: hop.port,
            },
            credentials,
            rule: None,
            local_bind: None,
            tls: false,
            server_name: None,
            insecure: false,
            plugins: Vec::new(),
            auth_prefix: None,
        });
    }
    let chain = eggress_uri::ProxyChainSpec { hops };
    let connector = eggress_outbound::OutboundConnector::from_chain(chain)
        .map_err(|e| anyhow::anyhow!("invalid egress chain: {e}"))?;
    Ok(Arc::new(connector))
}

/// Apply the configured egress route to an eggfetch client builder.
pub fn apply_route(
    builder: eggfetch_core::ClientBuilder,
    section: &EgressSection,
) -> anyhow::Result<eggfetch_core::ClientBuilder> {
    if !section.is_routed() {
        return Ok(builder);
    }
    #[cfg(feature = "egress")]
    {
        let connector = build_connector(section)?;
        let redacted = section.redacted_route();
        let dialer = EggressDialer::new(connector, redacted);
        Ok(builder.dialer(dialer))
    }
    #[cfg(not(feature = "egress"))]
    {
        anyhow::bail!(
            "egress route is configured ({}) but this binary was built without the `egress` feature; rebuild with --features egress or set [egress].enabled = false",
            section.redacted_route()
        );
    }
}

/// Redacted one-line route summary without credentials.
pub fn route_summary(section: &EgressSection) -> String {
    section.redacted_route()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::{EgressHopConfig, EgressSection};

    fn routed_section() -> EgressSection {
        EgressSection {
            enabled: true,
            hops: vec![EgressHopConfig {
                scheme: "http".to_string(),
                host: "127.0.0.1".to_string(),
                port: 18080,
                username: None,
                password_env: None,
            }],
        }
    }

    #[test]
    fn disabled_route_leaves_builder_direct() {
        let section = EgressSection::default();
        assert!(!section.is_routed());
        assert_eq!(route_summary(&section), "direct");
        let builder = eggfetch_core::Client::builder();
        let builder = apply_route(builder, &section).expect("direct route applies");
        let _ = builder.build();
    }

    #[test]
    fn redacted_route_hides_credentials() {
        let section = EgressSection {
            enabled: true,
            hops: vec![EgressHopConfig {
                scheme: "socks5".to_string(),
                host: "proxy.example".to_string(),
                port: 1080,
                username: Some("alice".to_string()),
                password_env: Some("EGRESS_TEST_PASS".to_string()),
            }],
        };
        let redacted = section.redacted_route();
        assert!(redacted.contains("socks5://"));
        assert!(redacted.contains("proxy.example:1080"));
        assert!(!redacted.contains("alice"));
        assert!(!redacted.contains("EGRESS_TEST_PASS"));
        assert!(redacted.contains("****"));
    }

    #[test]
    fn unsupported_scheme_fails_validation() {
        let section = EgressSection {
            enabled: true,
            hops: vec![EgressHopConfig {
                scheme: "shadowsocks".to_string(),
                host: "127.0.0.1".to_string(),
                port: 8388,
                username: None,
                password_env: None,
            }],
        };
        let cfg = crate::core::config::AppConfig {
            egress: section,
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn save_never_persists_raw_passwords() {
        let section = EgressSection {
            enabled: true,
            hops: vec![EgressHopConfig {
                scheme: "http".to_string(),
                host: "127.0.0.1".to_string(),
                port: 8080,
                username: Some("bob".to_string()),
                password_env: Some("EGRESS_PROXY_PASS".to_string()),
            }],
        };
        let cfg = crate::core::config::AppConfig {
            egress: section,
            ..Default::default()
        };
        let text = toml::to_string_pretty(&cfg).expect("serializes");
        assert!(!text.contains("s3cret"));
        assert!(text.contains("EGRESS_PROXY_PASS"));
    }

    #[test]
    fn routed_without_feature_fails_closed() {
        let section = routed_section();
        let builder = eggfetch_core::Client::builder();
        let result = apply_route(builder, &section);
        #[cfg(not(feature = "egress"))]
        assert!(result.is_err());
        #[cfg(feature = "egress")]
        assert!(result.is_ok());
    }

    #[cfg(feature = "egress")]
    #[test]
    fn egress_error_mapping_covers_typed_kinds() {
        use eggress_outbound::OutboundConnectErrorKind as Kind;
        assert_eq!(
            map_egress_failure(Kind::Timeout),
            eggfetch_core::DialErrorKind::Timeout
        );
        assert_eq!(
            map_egress_failure(Kind::Authentication),
            eggfetch_core::DialErrorKind::Authentication
        );
        assert_eq!(
            map_egress_failure(Kind::Policy),
            eggfetch_core::DialErrorKind::Rejected
        );
        for kind in [
            Kind::Dns,
            Kind::ConnectionRefused,
            Kind::NetworkUnreachable,
            Kind::HostUnreachable,
        ] {
            assert_eq!(
                map_egress_failure(kind),
                eggfetch_core::DialErrorKind::Connection
            );
        }
        for kind in [Kind::Tls, Kind::Protocol, Kind::Other] {
            assert_eq!(
                map_egress_failure(kind),
                eggfetch_core::DialErrorKind::Other
            );
        }
    }
}
