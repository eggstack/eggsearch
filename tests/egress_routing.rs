use std::net::SocketAddr;
use std::time::Duration;

use eggsearch::core::config::{AppConfig, EgressHopConfig, EgressSection};
use eggsearch::fetch::egress::{apply_route, route_summary};

fn loopback_hop(port: u16, scheme: &str) -> EgressHopConfig {
    EgressHopConfig {
        scheme: scheme.to_string(),
        host: "127.0.0.1".to_string(),
        port,
        username: None,
        password_env: None,
    }
}

#[test]
fn egress_disabled_by_default_and_loads_without_feature() {
    let cfg = AppConfig::default();
    assert!(!cfg.egress.enabled);
    assert!(cfg.egress.hops.is_empty());
    assert!(!cfg.egress.is_routed());
    assert_eq!(route_summary(&cfg.egress), "direct");
    assert!(cfg.validate().is_ok());
}

#[test]
fn egress_explicit_route_without_feature_fails_closed() {
    let section = EgressSection {
        enabled: true,
        hops: vec![loopback_hop(18080, "http")],
    };
    assert!(section.is_routed());
    let builder = eggfetch_core::Client::builder();
    let result = apply_route(builder, &section);
    #[cfg(not(feature = "egress"))]
    assert!(result.is_err());
    #[cfg(feature = "egress")]
    assert!(result.is_ok());
}

#[test]
fn egress_rejects_unsupported_schemes() {
    for scheme in ["shadowsocks", "trojan", "ssh", "quic", "direct"] {
        let cfg = AppConfig {
            egress: EgressSection {
                enabled: true,
                hops: vec![loopback_hop(1080, scheme)],
            },
            ..Default::default()
        };
        assert!(cfg.validate().is_err(), "scheme {scheme} must be rejected");
    }
}

#[test]
fn egress_redacts_credentials_in_diagnostics() {
    let section = EgressSection {
        enabled: true,
        hops: vec![EgressHopConfig {
            scheme: "http".to_string(),
            host: "127.0.0.1".to_string(),
            port: 8080,
            username: Some("alice".to_string()),
            password_env: Some("EGRESS_ROUTING_TEST_PASS".to_string()),
        }],
    };
    let summary = route_summary(&section);
    assert!(summary.contains("127.0.0.1:8080"));
    assert!(!summary.contains("alice"));
    assert!(!summary.contains("EGRESS_ROUTING_TEST_PASS"));
}

#[test]
fn egress_save_never_writes_raw_secrets() {
    let cfg = AppConfig {
        egress: EgressSection {
            enabled: true,
            hops: vec![EgressHopConfig {
                scheme: "socks5".to_string(),
                host: "127.0.0.1".to_string(),
                port: 1080,
                username: Some("bob".to_string()),
                password_env: Some("EGRESS_ROUTING_TEST_PASS_ENV".to_string()),
            }],
        },
        ..Default::default()
    };
    let text = toml::to_string_pretty(&cfg).expect("serializes");
    assert!(text.contains("EGRESS_ROUTING_TEST_PASS_ENV"));
    assert!(!text.contains("s3cret-payload"));
}

#[tokio::test]
async fn custom_dialer_rejects_resolved_addresses() {
    struct NoopDialer;
    impl eggfetch_core::Dialer for NoopDialer {
        fn dial(&self, _target: eggfetch_core::DialTarget) -> eggfetch_core::DialFuture<'_> {
            Box::pin(async {
                Err(eggfetch_core::DialError::new(
                    eggfetch_core::DialErrorKind::Connection,
                    "noop",
                ))
            })
        }
    }
    let client = eggfetch_core::Client::builder().dialer(NoopDialer).build();
    let result = client
        .get("http://127.0.0.1:9/")
        .expect("request builds")
        .resolved_addresses(["127.0.0.1:80".parse::<SocketAddr>().unwrap()])
        .send()
        .await;
    assert!(
        result.is_err(),
        "custom dialer with resolved_addresses must fail instead of silently dropping pinning"
    );
    let message = format!("{:?}", result.unwrap_err());
    assert!(
        message.contains("resolved") || message.contains("custom dial"),
        "failure must name the incompatibility, got: {message}"
    );
}

#[tokio::test]
async fn fetch_client_keeps_resolved_pinning_without_dialer() {
    let limits = eggsearch::fetch::limits::FetchLimits::default();
    let client = eggsearch::fetch::FetchClient::new(limits, "eggsearch/test".to_string(), false)
        .expect("fetch client builds");
    let result = client
        .fetch(
            "http://127.0.0.1:9/definitely-closed",
            Some(64),
            eggsearch::core::fetch::ExtractMode::Text,
            false,
            None,
        )
        .await;
    assert!(result.is_err());
}

#[cfg(feature = "egress")]
mod egress_feature {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    async fn read_http_head(stream: &mut TcpStream) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut tmp = [0u8; 1024];
        loop {
            let n = stream.read(&mut tmp).await.expect("reads");
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
            if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
            if buf.len() > 65536 {
                break;
            }
        }
        buf
    }

    async fn start_http_origin(body: Vec<u8>) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let body = body.clone();
                tokio::spawn(async move {
                    loop {
                        let mut buf = vec![0u8; 4096];
                        let n = match stream.read(&mut buf).await {
                            Ok(0) => break,
                            Ok(n) => n,
                            Err(_) => break,
                        };
                        let request = &buf[..n];
                        if !request.windows(4).any(|w| w == b"\r\n\r\n") {
                            continue;
                        }
                        let response = format!(
                            "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: keep-alive\r\n\r\n",
                            body.len()
                        );
                        if stream.write_all(response.as_bytes()).await.is_err() {
                            break;
                        }
                        if stream.write_all(&body).await.is_err() {
                            break;
                        }
                    }
                });
            }
        });
        addr
    }

    async fn start_gzip_origin(raw: Vec<u8>) -> SocketAddr {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&raw).expect("compresses");
        let compressed = encoder.finish().expect("finishes");
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let compressed = compressed.clone();
                tokio::spawn(async move {
                    loop {
                        let mut buf = vec![0u8; 4096];
                        let n = match stream.read(&mut buf).await {
                            Ok(0) => break,
                            Ok(n) => n,
                            Err(_) => break,
                        };
                        if !buf[..n].windows(4).any(|w| w == b"\r\n\r\n") {
                            continue;
                        }
                        let head = format!(
                            "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-encoding: gzip\r\ncontent-length: {}\r\nconnection: keep-alive\r\n\r\n",
                            compressed.len()
                        );
                        if stream.write_all(head.as_bytes()).await.is_err() {
                            break;
                        }
                        if stream.write_all(&compressed).await.is_err() {
                            break;
                        }
                    }
                });
            }
        });
        addr
    }

    async fn start_connect_proxy(require_auth: bool) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            loop {
                let Ok((mut inbound, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(async move {
                    let head = read_http_head(&mut inbound).await;
                    let text = String::from_utf8_lossy(&head).to_string();
                    let mut lines = text.lines();
                    let request_line = lines.next().unwrap_or_default().to_string();
                    if require_auth && !text.to_lowercase().contains("proxy-authorization:") {
                        let _ = inbound
                            .write_all(
                                b"HTTP/1.1 407 Proxy Authentication Required\r\nproxy-authenticate: Basic realm=\"egress\"\r\ncontent-length: 0\r\n\r\n",
                            )
                            .await;
                        return;
                    }
                    let target = request_line
                        .strip_prefix("CONNECT ")
                        .and_then(|rest| rest.split_whitespace().next())
                        .unwrap_or_default()
                        .to_string();
                    if target.is_empty() {
                        let _ = inbound
                            .write_all(b"HTTP/1.1 400 Bad Request\r\ncontent-length: 0\r\n\r\n")
                            .await;
                        return;
                    }
                    let upstream = match TcpStream::connect(&target).await {
                        Ok(s) => s,
                        Err(_) => {
                            let _ = inbound
                                .write_all(b"HTTP/1.1 502 Bad Gateway\r\ncontent-length: 0\r\n\r\n")
                                .await;
                            return;
                        }
                    };
                    if inbound
                        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                        .await
                        .is_err()
                    {
                        return;
                    }
                    let mut upstream = upstream;
                    let _ = tokio::io::copy_bidirectional(&mut inbound, &mut upstream).await;
                });
            }
        });
        addr
    }

    async fn start_socks5_proxy(
        require_auth: bool,
        expected_user: Vec<u8>,
        expected_pass: Vec<u8>,
    ) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            loop {
                let Ok((mut inbound, _)) = listener.accept().await else {
                    break;
                };
                let expected_user = expected_user.clone();
                let expected_pass = expected_pass.clone();
                tokio::spawn(async move {
                    let mut greeting = [0u8; 2];
                    if inbound.read_exact(&mut greeting).await.is_err() {
                        return;
                    }
                    let nmethods = greeting[1] as usize;
                    let mut methods = vec![0u8; nmethods];
                    if inbound.read_exact(&mut methods).await.is_err() {
                        return;
                    }
                    if require_auth {
                        if !methods.contains(&0x02) {
                            let _ = inbound.write_all(&[0x05, 0xFF]).await;
                            return;
                        }
                        if inbound.write_all(&[0x05, 0x02]).await.is_err() {
                            return;
                        }
                        let mut auth_head = [0u8; 2];
                        if inbound.read_exact(&mut auth_head).await.is_err() {
                            return;
                        }
                        let ulen = auth_head[1] as usize;
                        let mut uname = vec![0u8; ulen];
                        if inbound.read_exact(&mut uname).await.is_err() {
                            return;
                        }
                        let mut plen_buf = [0u8; 1];
                        if inbound.read_exact(&mut plen_buf).await.is_err() {
                            return;
                        }
                        let mut passwd = vec![0u8; plen_buf[0] as usize];
                        if inbound.read_exact(&mut passwd).await.is_err() {
                            return;
                        }
                        if uname != expected_user || passwd != expected_pass {
                            let _ = inbound.write_all(&[0x01, 0x01]).await;
                            return;
                        }
                        if inbound.write_all(&[0x01, 0x00]).await.is_err() {
                            return;
                        }
                    } else if inbound.write_all(&[0x05, 0x00]).await.is_err() {
                        return;
                    }
                    let mut req_head = [0u8; 4];
                    if inbound.read_exact(&mut req_head).await.is_err() {
                        return;
                    }
                    if req_head[1] != 0x01 {
                        let _ = inbound
                            .write_all(&[0x05, 0x07, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                            .await;
                        return;
                    }
                    let target: String = match req_head[3] {
                        0x01 => {
                            let mut addr = [0u8; 4];
                            if inbound.read_exact(&mut addr).await.is_err() {
                                return;
                            }
                            let mut port = [0u8; 2];
                            if inbound.read_exact(&mut port).await.is_err() {
                                return;
                            }
                            let port = u16::from_be_bytes(port);
                            format!("{}.{}.{}.{}:{port}", addr[0], addr[1], addr[2], addr[3])
                        }
                        0x03 => {
                            let mut len = [0u8; 1];
                            if inbound.read_exact(&mut len).await.is_err() {
                                return;
                            }
                            let mut host = vec![0u8; len[0] as usize];
                            if inbound.read_exact(&mut host).await.is_err() {
                                return;
                            }
                            let mut port = [0u8; 2];
                            if inbound.read_exact(&mut port).await.is_err() {
                                return;
                            }
                            let port = u16::from_be_bytes(port);
                            format!("{}:{port}", String::from_utf8_lossy(&host))
                        }
                        _ => {
                            let _ = inbound
                                .write_all(&[0x05, 0x08, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                                .await;
                            return;
                        }
                    };
                    let upstream = match TcpStream::connect(&target).await {
                        Ok(s) => s,
                        Err(_) => {
                            let _ = inbound
                                .write_all(&[0x05, 0x05, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                                .await;
                            return;
                        }
                    };
                    if inbound
                        .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                        .await
                        .is_err()
                    {
                        return;
                    }
                    let mut upstream = upstream;
                    let _ = tokio::io::copy_bidirectional(&mut inbound, &mut upstream).await;
                });
            }
        });
        addr
    }

    fn egress_section(scheme: &str, port: u16) -> EgressSection {
        EgressSection {
            enabled: true,
            hops: vec![EgressHopConfig {
                scheme: scheme.to_string(),
                host: "127.0.0.1".to_string(),
                port,
                username: None,
                password_env: None,
            }],
        }
    }

    async fn get_via_route(origin: SocketAddr, section: &EgressSection) -> Vec<u8> {
        let builder = eggfetch_core::Client::builder()
            .timeout(eggfetch_core::Timeout {
                pool: Some(Duration::from_secs(5)),
                connect: Some(Duration::from_secs(5)),
                write: Some(Duration::from_secs(5)),
                read: Some(Duration::from_secs(5)),
                total: Some(Duration::from_secs(5)),
            })
            .follow_redirects(false);
        let builder = apply_route(builder, section).expect("route applies");
        let client = builder.build();
        let url = format!("http://127.0.0.1:{}/", origin.port());
        let mut response = client
            .get(&url)
            .expect("builds")
            .send()
            .await
            .expect("fetches through proxy");
        assert!(response.status().is_success());
        response.bytes().await.expect("reads").to_vec()
    }

    #[tokio::test]
    async fn direct_connector_smoke() {
        let origin = start_http_origin(b"direct-ok".to_vec()).await;
        let connector = eggress_outbound::OutboundConnector::direct();
        let (mut stream, info) = tokio::time::timeout(
            Duration::from_secs(5),
            connector.connect_tcp_detailed("127.0.0.1", origin.port()),
        )
        .await
        .expect("connects within deadline")
        .expect("direct connects");
        assert_eq!(info.hop_count, 0);
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        stream
            .write_all(b"GET / HTTP/1.1\r\nhost: x\r\nconnection: close\r\n\r\n")
            .await
            .expect("writes");
        let mut out = Vec::new();
        let mut tmp = [0u8; 1024];
        let found = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let n = stream.read(&mut tmp).await.expect("reads");
                if n == 0 {
                    break false;
                }
                out.extend_from_slice(&tmp[..n]);
                if out.windows(9).any(|w| w == b"direct-ok") {
                    break true;
                }
                if out.len() > 65536 {
                    break false;
                }
            }
        })
        .await
        .expect("reads within deadline");
        assert!(found);
    }

    #[tokio::test]
    async fn http_proxy_chain_carries_http() {
        let origin = start_http_origin(b"via-http-proxy".to_vec()).await;
        let proxy = start_connect_proxy(false).await;
        let body = get_via_route(origin, &egress_section("http", proxy.port())).await;
        assert!(String::from_utf8_lossy(&body).contains("via-http-proxy"));
    }

    #[tokio::test]
    async fn socks5_proxy_chain_carries_http() {
        let origin = start_http_origin(b"via-socks5".to_vec()).await;
        let proxy = start_socks5_proxy(false, Vec::new(), Vec::new()).await;
        let body = get_via_route(origin, &egress_section("socks5", proxy.port())).await;
        assert!(String::from_utf8_lossy(&body).contains("via-socks5"));
    }

    #[tokio::test]
    async fn mixed_two_hop_chain_carries_http() {
        let origin = start_http_origin(b"via-mixed-chain".to_vec()).await;
        let second = start_connect_proxy(false).await;
        let first = start_socks5_proxy(false, Vec::new(), Vec::new()).await;
        let section = EgressSection {
            enabled: true,
            hops: vec![
                EgressHopConfig {
                    scheme: "socks5".to_string(),
                    host: "127.0.0.1".to_string(),
                    port: first.port(),
                    username: None,
                    password_env: None,
                },
                EgressHopConfig {
                    scheme: "http".to_string(),
                    host: "127.0.0.1".to_string(),
                    port: second.port(),
                    username: None,
                    password_env: None,
                },
            ],
        };
        let body = get_via_route(origin, &section).await;
        assert!(String::from_utf8_lossy(&body).contains("via-mixed-chain"));
    }

    #[tokio::test]
    async fn proxy_authentication_rejection_fails_closed() {
        let origin = start_http_origin(b"must-not-leak".to_vec()).await;
        let proxy = start_connect_proxy(true).await;
        let section = egress_section("http", proxy.port());
        let builder = eggfetch_core::Client::builder().follow_redirects(false);
        let builder = apply_route(builder, &section).expect("applies");
        let client = builder.build();
        let url = format!("http://127.0.0.1:{}/", origin.port());
        let result = client.get(&url).expect("builds").send().await;
        assert!(result.is_err(), "auth rejection must fail, never bypass");
    }

    #[tokio::test]
    async fn unavailable_first_hop_fails_without_direct_fallback() {
        let origin = start_http_origin(b"must-not-leak".to_vec()).await;
        let section = egress_section("http", 9);
        let builder = eggfetch_core::Client::builder()
            .timeout(eggfetch_core::Timeout {
                pool: Some(Duration::from_secs(2)),
                connect: Some(Duration::from_secs(2)),
                write: Some(Duration::from_secs(2)),
                read: Some(Duration::from_secs(2)),
                total: Some(Duration::from_secs(2)),
            })
            .follow_redirects(false);
        let builder = apply_route(builder, &section).expect("applies");
        let client = builder.build();
        let url = format!("http://127.0.0.1:{}/", origin.port());
        let result = client.get(&url).expect("builds").send().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn gzip_decodes_through_proxy() {
        let origin = start_gzip_origin(b"compress-via-proxy".to_vec()).await;
        let proxy = start_connect_proxy(false).await;
        let body = get_via_route(origin, &egress_section("http", proxy.port())).await;
        assert!(String::from_utf8_lossy(&body).contains("compress-via-proxy"));
    }

    #[tokio::test]
    async fn malformed_chain_config_fails_fast() {
        let section = EgressSection {
            enabled: true,
            hops: vec![],
        };
        assert!(!section.is_routed());
        let builder = eggfetch_core::Client::builder();
        let builder = apply_route(builder, &section).expect("empty disabled route is direct");
        let _ = builder.build();
        let bad = EgressSection {
            enabled: true,
            hops: vec![EgressHopConfig {
                scheme: "trojan".to_string(),
                host: "127.0.0.1".to_string(),
                port: 1080,
                username: None,
                password_env: None,
            }],
        };
        let builder = eggfetch_core::Client::builder();
        assert!(apply_route(builder, &bad).is_err());
    }

    #[tokio::test]
    async fn missing_credential_env_fails_closed() {
        let section = EgressSection {
            enabled: true,
            hops: vec![EgressHopConfig {
                scheme: "http".to_string(),
                host: "127.0.0.1".to_string(),
                port: 8080,
                username: Some("alice".to_string()),
                password_env: Some("EGRESS_DEFINITELY_MISSING_ENV_12345".to_string()),
            }],
        };
        let builder = eggfetch_core::Client::builder();
        assert!(apply_route(builder, &section).is_err());
    }

    #[tokio::test]
    async fn typed_failures_map_without_string_parsing() {
        use eggfetch_core::DialErrorKind;
        use eggress_outbound::OutboundConnectErrorKind as Kind;
        use eggsearch::fetch::egress::map_egress_failure;
        assert_eq!(map_egress_failure(Kind::Timeout), DialErrorKind::Timeout);
        assert_eq!(
            map_egress_failure(Kind::Authentication),
            DialErrorKind::Authentication
        );
        assert_eq!(map_egress_failure(Kind::Policy), DialErrorKind::Rejected);
        assert_eq!(map_egress_failure(Kind::Dns), DialErrorKind::Connection);
        assert_eq!(map_egress_failure(Kind::Tls), DialErrorKind::Other);
    }
}
