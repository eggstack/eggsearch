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

#[test]
fn egress_host_validation_accepts_dns_ipv4_and_ipv6_literals() {
    for host in ["127.0.0.1", "proxy.example", "::1", "2001:db8::1"] {
        let cfg = AppConfig {
            egress: EgressSection {
                enabled: false,
                hops: vec![EgressHopConfig {
                    scheme: "http".to_string(),
                    host: host.to_string(),
                    port: 8080,
                    username: None,
                    password_env: None,
                }],
            },
            ..Default::default()
        };
        assert!(
            cfg.validate().is_ok(),
            "host '{host}' must be accepted as a bare hostname or IP literal"
        );
    }
}

#[test]
fn egress_host_validation_rejects_scheme_userinfo_path_and_port_suffix() {
    for host in [
        "proxy.example:8080",
        "http://proxy.example",
        "user:pw@proxy.example",
        "proxy.example/path",
        "proxy.example/",
        "/proxy.example",
        "[::1]",
    ] {
        let cfg = AppConfig {
            egress: EgressSection {
                enabled: false,
                hops: vec![EgressHopConfig {
                    scheme: "http".to_string(),
                    host: host.to_string(),
                    port: 8080,
                    username: None,
                    password_env: None,
                }],
            },
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "host '{host}' must be rejected (userinfo / path / port / bracket syntax)"
        );
    }
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

    async fn start_brotli_origin(raw: Vec<u8>) -> SocketAddr {
        let mut compressed = Vec::new();
        {
            use brotli::CompressorWriter;
            use std::io::Write;
            let mut writer = CompressorWriter::new(&mut compressed, 4096, 6, 22);
            writer.write_all(&raw).expect("compresses");
            writer.flush().expect("flushes");
        }
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
                            "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-encoding: br\r\ncontent-length: {}\r\nconnection: keep-alive\r\n\r\n",
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

    async fn start_counting_connect_proxy(
        require_auth: bool,
        accept_counter: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            loop {
                let Ok((mut inbound, _)) = listener.accept().await else {
                    break;
                };
                accept_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
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
    async fn brotli_decodes_through_proxy() {
        let raw = b"brotli-via-proxy-abcdefghijklmnopqrstuvwxyz".repeat(64);
        let origin = start_brotli_origin(raw.clone()).await;
        let proxy = start_connect_proxy(false).await;
        let body = get_via_route(origin, &egress_section("http", proxy.port())).await;
        assert_eq!(
            String::from_utf8_lossy(&body).as_ref(),
            std::str::from_utf8(&raw).unwrap(),
            "Brotli body must decode to original payload through the route"
        );
    }

    #[tokio::test]
    async fn gzip_decoded_body_limit_enforced_through_proxy() {
        let raw = vec![b'a'; 4096];
        let origin = start_gzip_origin(raw).await;
        let proxy = start_connect_proxy(false).await;
        let builder = eggfetch_core::Client::builder()
            .max_decoded_body_size(64)
            .max_decompression_ratio(100.0)
            .timeout(eggfetch_core::Timeout {
                pool: Some(Duration::from_secs(5)),
                connect: Some(Duration::from_secs(5)),
                write: Some(Duration::from_secs(5)),
                read: Some(Duration::from_secs(5)),
                total: Some(Duration::from_secs(5)),
            })
            .follow_redirects(false);
        let builder =
            apply_route(builder, &egress_section("http", proxy.port())).expect("route applies");
        let client = builder.build();
        let url = format!("http://127.0.0.1:{}/", origin.port());
        let mut response = client
            .get(&url)
            .expect("builds")
            .send()
            .await
            .expect("response succeeds; limit is checked on body consumption");
        let err = response
            .bytes()
            .await
            .expect_err("oversized decoded gzip body must be rejected on read");
        let message = format!("{err:?}");
        assert!(
            message.to_lowercase().contains("body")
                || message.to_lowercase().contains("decompression")
                || message.to_lowercase().contains("too large")
                || message.to_lowercase().contains("limit"),
            "failure must mention a body/limit/decompression reason, got: {message}"
        );
    }

    #[tokio::test]
    async fn brotli_decoded_body_limit_enforced_through_proxy() {
        let raw = vec![b'b'; 4096];
        let origin = start_brotli_origin(raw).await;
        let proxy = start_connect_proxy(false).await;
        let builder = eggfetch_core::Client::builder()
            .max_decoded_body_size(64)
            .max_decompression_ratio(100.0)
            .timeout(eggfetch_core::Timeout {
                pool: Some(Duration::from_secs(5)),
                connect: Some(Duration::from_secs(5)),
                write: Some(Duration::from_secs(5)),
                read: Some(Duration::from_secs(5)),
                total: Some(Duration::from_secs(5)),
            })
            .follow_redirects(false);
        let builder =
            apply_route(builder, &egress_section("http", proxy.port())).expect("route applies");
        let client = builder.build();
        let url = format!("http://127.0.0.1:{}/", origin.port());
        let mut response = client
            .get(&url)
            .expect("builds")
            .send()
            .await
            .expect("response succeeds; limit is checked on body consumption");
        let err = response
            .bytes()
            .await
            .expect_err("oversized decoded brotli body must be rejected on read");
        let message = format!("{err:?}");
        assert!(
            message.to_lowercase().contains("body")
                || message.to_lowercase().contains("decompression")
                || message.to_lowercase().contains("too large")
                || message.to_lowercase().contains("limit"),
            "failure must mention a body/limit/decompression reason, got: {message}"
        );
    }

    async fn start_authenticated_connect_proxy(
        expected_user: Vec<u8>,
        expected_pass: Vec<u8>,
        received_proxy_auth: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
        received_by_destination: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
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
                let received_proxy_auth = received_proxy_auth.clone();
                let received_by_destination = received_by_destination.clone();
                tokio::spawn(async move {
                    let head = read_http_head(&mut inbound).await;
                    let head_text = String::from_utf8_lossy(&head).to_string();
                    if let Some(idx) = head_text.to_lowercase().find("proxy-authorization:") {
                        let after = &head_text[idx..];
                        if let Some(end) = after.find("\r\n") {
                            *received_proxy_auth.lock().unwrap() = after.as_bytes()[..end].to_vec();
                        }
                    }
                    let expected = format!(
                        "Basic {}",
                        base64_encode(
                            format!(
                                "{}:{}",
                                String::from_utf8_lossy(&expected_user),
                                String::from_utf8_lossy(&expected_pass)
                            )
                            .as_bytes()
                        )
                    );
                    if !head_text.to_lowercase().contains("proxy-authorization:") {
                        let _ = inbound
                            .write_all(
                                b"HTTP/1.1 407 Proxy Authentication Required\r\nproxy-authenticate: Basic realm=\"egress\"\r\ncontent-length: 0\r\n\r\n",
                            )
                            .await;
                        return;
                    }
                    if !head_text.contains(&expected) {
                        let _ = inbound
                            .write_all(b"HTTP/1.1 403 Forbidden\r\ncontent-length: 0\r\n\r\n")
                            .await;
                        return;
                    }
                    let request_line = head_text.lines().next().unwrap_or_default().to_string();
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
                    let (mut a, mut b) = (inbound, upstream);
                    let _ = received_by_destination.clone();
                    let _ = tokio::io::copy_bidirectional(&mut a, &mut b).await;
                });
            }
        });
        addr
    }

    async fn start_origin_recording_headers(
        recorded: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    ) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let recorded = recorded.clone();
                tokio::spawn(async move {
                    let head = read_http_head(&mut stream).await;
                    *recorded.lock().unwrap() = head;
                    let response = b"HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: 2\r\nconnection: close\r\n\r\nOK";
                    let _ = stream.write_all(response).await;
                });
            }
        });
        addr
    }

    #[tokio::test]
    async fn authenticated_http_connect_succeeds_and_does_not_forward_creds() {
        let auth_env = format!("EGRESS_PHASE26_HTTP_PASS_{}", std::process::id());
        let unique_user = format!("phase26-user-{}", std::process::id());
        let unique_pass = format!("phase26-pass-{}", std::process::id());
        std::env::set_var(&auth_env, &unique_pass);
        let received_proxy_auth = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
        let received_by_destination = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
        let origin = start_origin_recording_headers(received_by_destination.clone()).await;
        let proxy = start_authenticated_connect_proxy(
            unique_user.as_bytes().to_vec(),
            unique_pass.as_bytes().to_vec(),
            received_proxy_auth.clone(),
            received_by_destination.clone(),
        )
        .await;
        let section = EgressSection {
            enabled: true,
            hops: vec![EgressHopConfig {
                scheme: "http".to_string(),
                host: "127.0.0.1".to_string(),
                port: proxy.port(),
                username: Some(unique_user.clone()),
                password_env: Some(auth_env.clone()),
            }],
        };
        let body = get_via_route(origin, &section).await;
        assert!(String::from_utf8_lossy(&body).contains("OK"));
        std::env::remove_var(&auth_env);

        let auth_seen = received_proxy_auth.lock().unwrap().clone();
        assert!(
            !auth_seen.is_empty(),
            "CONNECT proxy must observe a Proxy-Authorization header"
        );
        let auth_text = String::from_utf8_lossy(&auth_seen).to_string();
        assert!(
            auth_text.contains("Basic "),
            "CONNECT proxy must observe Basic-encoded Proxy-Authorization, saw: {auth_text:?}"
        );
        let token = auth_text
            .split("Basic ")
            .nth(1)
            .unwrap_or_default()
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .as_bytes()
            .to_vec();
        let decoded = base64_decode(&token);
        let decoded_str = String::from_utf8_lossy(&decoded).to_string();
        assert_eq!(decoded_str, format!("{unique_user}:{unique_pass}"));
        let dest_seen = received_by_destination.lock().unwrap().clone();
        let dest_text = String::from_utf8_lossy(&dest_seen).to_lowercase();
        assert!(
            !dest_text.contains("proxy-authorization"),
            "Proxy-Authorization must never reach the destination, saw: {dest_text}"
        );
        assert!(
            !dest_seen
                .windows(unique_pass.len())
                .any(|w| w == unique_pass.as_bytes()),
            "raw proxy password must not appear in the destination request bytes"
        );
    }

    fn base64_decode(input: &[u8]) -> Vec<u8> {
        let cleaned: Vec<u8> = input
            .iter()
            .copied()
            .filter(|c| !c.is_ascii_whitespace())
            .collect();
        let mut out = Vec::with_capacity(cleaned.len() * 3 / 4);
        let mut buf: [u8; 4] = [0; 4];
        let mut i = 0;
        for &c in &cleaned {
            if c == b'=' {
                break;
            }
            let v = match c {
                b'A'..=b'Z' => c - b'A',
                b'a'..=b'z' => c - b'a' + 26,
                b'0'..=b'9' => c - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                _ => continue,
            };
            buf[i] = v;
            i += 1;
            if i == 4 {
                out.push((buf[0] << 2) | (buf[1] >> 4));
                out.push((buf[1] << 4) | (buf[2] >> 2));
                out.push((buf[2] << 6) | buf[3]);
                i = 0;
            }
        }
        match i {
            2 => out.push((buf[0] << 2) | (buf[1] >> 4)),
            3 => {
                out.push((buf[0] << 2) | (buf[1] >> 4));
                out.push((buf[1] << 4) | (buf[2] >> 2));
            }
            _ => {}
        }
        out
    }

    #[tokio::test]
    async fn authenticated_socks5_succeeds_and_does_not_forward_creds() {
        let auth_env = format!("EGRESS_PHASE26_SOCKS5_PASS_{}", std::process::id());
        let unique_user = format!("socks5-user-{}", std::process::id());
        let unique_pass = format!("socks5-pass-{}", std::process::id());
        std::env::set_var(&auth_env, &unique_pass);
        let origin = start_http_origin(b"socks5-auth-ok".to_vec()).await;
        let proxy = start_socks5_proxy(
            true,
            unique_user.as_bytes().to_vec(),
            unique_pass.as_bytes().to_vec(),
        )
        .await;
        let section = EgressSection {
            enabled: true,
            hops: vec![EgressHopConfig {
                scheme: "socks5".to_string(),
                host: "127.0.0.1".to_string(),
                port: proxy.port(),
                username: Some(unique_user.clone()),
                password_env: Some(auth_env.clone()),
            }],
        };
        let body = get_via_route(origin, &section).await;
        assert!(String::from_utf8_lossy(&body).contains("socks5-auth-ok"));
        std::env::remove_var(&auth_env);
    }

    #[tokio::test]
    async fn wrong_proxy_credentials_fail_closed() {
        let auth_env = format!("EGRESS_PHASE26_WRONG_PASS_{}", std::process::id());
        let user = format!("phase26-bad-{}", std::process::id());
        let pass = format!("phase26-bad-{}", std::process::id());
        std::env::set_var(&auth_env, &pass);
        let origin = start_http_origin(b"must-not-leak".to_vec()).await;
        let proxy = start_socks5_proxy(
            true,
            user.as_bytes().to_vec(),
            b"different-credential-bytes".to_vec(),
        )
        .await;
        let section = EgressSection {
            enabled: true,
            hops: vec![EgressHopConfig {
                scheme: "socks5".to_string(),
                host: "127.0.0.1".to_string(),
                port: proxy.port(),
                username: Some(user.clone()),
                password_env: Some(auth_env.clone()),
            }],
        };
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
        assert!(
            result.is_err(),
            "wrong proxy credentials must fail closed, never bypass to direct"
        );
        let message = format!("{:?}", result.unwrap_err());
        assert!(
            !message.contains(&pass),
            "credential value must never appear in the failure message: {message}"
        );
        std::env::remove_var(&auth_env);
    }

    #[tokio::test]
    async fn route_summary_and_debug_redact_password() {
        let auth_env = format!("EGRESS_PHASE26_REDACT_PASS_{}", std::process::id());
        let pass = format!("super-secret-{}", std::process::id());
        std::env::set_var(&auth_env, &pass);
        let section = EgressSection {
            enabled: true,
            hops: vec![EgressHopConfig {
                scheme: "http".to_string(),
                host: "127.0.0.1".to_string(),
                port: 8080,
                username: Some("phase26-redact".to_string()),
                password_env: Some(auth_env.clone()),
            }],
        };
        let summary = route_summary(&section);
        assert!(
            !summary.contains(&pass),
            "route_summary must not include the raw password: {summary}"
        );
        let debug = format!("{:?}", section);
        assert!(
            !debug.contains(&pass),
            "Debug of EgressSection must not include the raw password: {debug}"
        );
        std::env::remove_var(&auth_env);
    }

    async fn start_garbage_connect_proxy() -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            loop {
                let Ok((mut inbound, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(async move {
                    let _ = read_http_head(&mut inbound).await;
                    let _ = inbound
                        .write_all(b"THIS IS NOT HTTP AT ALL \x00\xff\xfe garbage!!!\r\n\r\n")
                        .await;
                    let _ = inbound.shutdown().await;
                });
            }
        });
        addr
    }

    async fn start_eof_connect_proxy() -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            loop {
                let Ok((mut inbound, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(async move {
                    let _ = read_http_head(&mut inbound).await;
                    let _ = inbound.write_all(b"HTTP/1.1 200 Connection Est").await;
                    let _ = inbound.shutdown().await;
                });
            }
        });
        addr
    }

    async fn start_malformed_socks5_proxy() -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            loop {
                let Ok((mut inbound, _)) = listener.accept().await else {
                    break;
                };
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
                    let _ = inbound.write_all(&[0x05, 0xFE]).await;
                    let _ = inbound.shutdown().await;
                });
            }
        });
        addr
    }

    async fn assert_malformed_route_fails_closed(proxy: SocketAddr, scheme: &str, label: &str) {
        let origin = start_http_origin(b"must-not-leak".to_vec()).await;
        let section = egress_section(scheme, proxy.port());
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
        let started = std::time::Instant::now();
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            client.get(&url).expect("builds").send(),
        )
        .await;
        let elapsed = started.elapsed();
        assert!(elapsed < Duration::from_secs(5), "{label} must be bounded");
        match result {
            Ok(Ok(_)) => panic!("{label} must fail, never succeed through a malformed proxy"),
            Ok(Err(e)) => {
                let message = format!("{e:?}");
                assert!(
                    !message.contains("s3cret") && !message.contains("EGRESS"),
                    "{label} error must not leak credential material: {message}"
                );
            }
            Err(_) => {}
        }
    }

    #[tokio::test]
    async fn malformed_connect_status_fails_closed() {
        let proxy = start_garbage_connect_proxy().await;
        assert_malformed_route_fails_closed(proxy, "http", "garbage CONNECT framing").await;
    }

    #[tokio::test]
    async fn truncated_connect_response_fails_closed() {
        let proxy = start_eof_connect_proxy().await;
        assert_malformed_route_fails_closed(proxy, "http", "truncated CONNECT response").await;
    }

    #[tokio::test]
    async fn malformed_socks5_greeting_fails_closed() {
        let proxy = start_malformed_socks5_proxy().await;
        assert_malformed_route_fails_closed(proxy, "socks5", "malformed SOCKS5 greeting").await;
    }

    async fn start_redirect_origin(target: SocketAddr) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(async move {
                    let _ = read_http_head(&mut stream).await;
                    let location = format!("http://127.0.0.1:{}/", target.port());
                    let response = format!(
                        "HTTP/1.1 302 Found\r\nlocation: {location}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                });
            }
        });
        addr
    }

    #[tokio::test]
    async fn routed_redirect_stays_on_proxy_route() {
        let final_origin = start_http_origin(b"redirect-final".to_vec()).await;
        let redirect_origin = start_redirect_origin(final_origin).await;
        let accepts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let proxy = start_counting_connect_proxy(false, accepts.clone()).await;
        let builder = eggfetch_core::Client::builder()
            .timeout(eggfetch_core::Timeout {
                pool: Some(Duration::from_secs(5)),
                connect: Some(Duration::from_secs(5)),
                write: Some(Duration::from_secs(5)),
                read: Some(Duration::from_secs(5)),
                total: Some(Duration::from_secs(5)),
            })
            .follow_redirects(true)
            .max_redirects(5);
        let builder =
            apply_route(builder, &egress_section("http", proxy.port())).expect("route applies");
        let client = builder.build();
        let url = format!("http://127.0.0.1:{}/", redirect_origin.port());
        let mut response = client
            .get(&url)
            .expect("builds")
            .send()
            .await
            .expect("redirect through proxy succeeds");
        assert!(response.status().is_success());
        let bytes = response.bytes().await.expect("reads").to_vec();
        assert!(String::from_utf8_lossy(&bytes).contains("redirect-final"));
        let accepts_after = accepts.load(std::sync::atomic::Ordering::SeqCst);
        assert!(
            (1..=2).contains(&accepts_after),
            "redirect must traverse the proxy chain without direct bypass (saw {accepts_after} accepts)"
        );
    }

    #[tokio::test]
    async fn routed_client_reuses_destination_connection() {
        let origin = start_http_origin(b"reuse-target".to_vec()).await;
        let accepts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let proxy = start_counting_connect_proxy(false, accepts.clone()).await;
        let builder = eggfetch_core::Client::builder()
            .timeout(eggfetch_core::Timeout {
                pool: Some(Duration::from_secs(5)),
                connect: Some(Duration::from_secs(5)),
                write: Some(Duration::from_secs(5)),
                read: Some(Duration::from_secs(5)),
                total: Some(Duration::from_secs(5)),
            })
            .follow_redirects(false);
        let builder =
            apply_route(builder, &egress_section("http", proxy.port())).expect("route applies");
        let client = builder.build();
        let url = format!("http://127.0.0.1:{}/", origin.port());

        for expected_marker in ["reuse-a", "reuse-b"] {
            let request = client
                .get(&url)
                .expect("builds")
                .header("x-test-marker", expected_marker);
            let mut response = request.send().await.expect("fetches through proxy");
            assert!(response.status().is_success());
            let bytes = response.bytes().await.expect("reads").to_vec();
            assert!(String::from_utf8_lossy(&bytes).contains("reuse-target"));
        }

        let accepts_after = accepts.load(std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            accepts_after, 1,
            "one shared routed client must pool the destination connection above EggressDialer (saw {accepts_after} accepts)"
        );
    }

    async fn start_stalling_connect_proxy() -> (SocketAddr, tokio::sync::oneshot::Sender<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
        let addr = listener.local_addr().expect("addr");
        let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();
        let release_rx = std::sync::Arc::new(tokio::sync::Mutex::new(Some(release_rx)));
        tokio::spawn(async move {
            loop {
                let Ok((mut inbound, _)) = listener.accept().await else {
                    break;
                };
                let release_rx = release_rx.clone();
                tokio::spawn(async move {
                    let _ = read_http_head(&mut inbound).await;
                    let mut guard = release_rx.lock().await;
                    if let Some(rx) = guard.take() {
                        let _ = tokio::time::timeout(Duration::from_secs(15), rx).await;
                    }
                    let _ = inbound.shutdown().await;
                });
            }
        });
        (addr, release_tx)
    }

    #[tokio::test]
    async fn stalled_proxy_handshake_is_bounded_by_timeout() {
        let (proxy, release_tx) = start_stalling_connect_proxy().await;
        let _ = release_tx;
        let section = egress_section("http", proxy.port());
        let builder = eggfetch_core::Client::builder()
            .timeout(eggfetch_core::Timeout {
                pool: Some(Duration::from_millis(500)),
                connect: Some(Duration::from_millis(500)),
                write: Some(Duration::from_millis(500)),
                read: Some(Duration::from_millis(500)),
                total: Some(Duration::from_millis(500)),
            })
            .follow_redirects(false);
        let builder = apply_route(builder, &section).expect("route applies");
        let client = builder.build();
        let url = "http://example.invalid/".to_string();
        let started = std::time::Instant::now();
        let result = client.get(&url).expect("builds").send().await;
        let elapsed = started.elapsed();
        assert!(result.is_err(), "stalled handshake must surface as error");
        assert!(
            elapsed < Duration::from_secs(3),
            "stalled handshake must be bounded within 3s (was {elapsed:?})"
        );
    }

    #[tokio::test]
    async fn dropped_routed_future_cancels_proxy_handshake() {
        let (proxy, release_tx) = start_stalling_connect_proxy().await;
        let section = egress_section("http", proxy.port());
        let builder = eggfetch_core::Client::builder()
            .timeout(eggfetch_core::Timeout {
                pool: Some(Duration::from_secs(5)),
                connect: Some(Duration::from_secs(5)),
                write: Some(Duration::from_secs(5)),
                read: Some(Duration::from_secs(5)),
                total: Some(Duration::from_secs(5)),
            })
            .follow_redirects(false);
        let builder = apply_route(builder, &section).expect("route applies");
        let client = builder.build();
        let url = "http://example.invalid/".to_string();
        let request = client.get(&url).expect("builds").send();
        drop(request);
        let started = std::time::Instant::now();
        let bound = tokio::time::timeout(Duration::from_secs(3), async {
            release_tx.send(()).expect("release channel sends");
        })
        .await;
        let elapsed = started.elapsed();
        assert!(
            bound.is_ok(),
            "stalled handshake must observe cancellation within 3s (was {elapsed:?})"
        );
    }

    fn generate_self_signed_cert(
        sans: Vec<rcgen::SanType>,
    ) -> (
        Vec<rustls::pki_types::CertificateDer<'static>>,
        rustls::pki_types::PrivateKeyDer<'static>,
    ) {
        let key_pair = rcgen::KeyPair::generate().expect("key pair");
        let mut params = rcgen::CertificateParams::default();
        params.subject_alt_names = sans;
        let cert = params.self_signed(&key_pair).expect("cert signs");
        let cert_der = rustls::pki_types::CertificateDer::from(cert.der().to_vec());
        let key_der = rustls::pki_types::PrivateKeyDer::Pkcs8(key_pair.serialize_der().into());
        (vec![cert_der], key_der)
    }

    fn rustls_server_config(
        certs: Vec<rustls::pki_types::CertificateDer<'static>>,
        key: rustls::pki_types::PrivateKeyDer<'static>,
    ) -> std::sync::Arc<rustls::ServerConfig> {
        std::sync::Arc::new(
            rustls::ServerConfig::builder_with_provider(
                rustls::crypto::ring::default_provider().into(),
            )
            .with_safe_default_protocol_versions()
            .expect("protocol versions")
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .expect("server config"),
        )
    }

    async fn start_https_origin(
        body: Vec<u8>,
        certs: Vec<rustls::pki_types::CertificateDer<'static>>,
        key: rustls::pki_types::PrivateKeyDer<'static>,
    ) -> SocketAddr {
        let cfg = rustls_server_config(certs, key);
        let acceptor = tokio_rustls::TlsAcceptor::from(cfg);
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    break;
                };
                let acceptor = acceptor.clone();
                let body = body.clone();
                tokio::spawn(async move {
                    let mut tls = match acceptor.accept(stream).await {
                        Ok(s) => s,
                        Err(_) => return,
                    };
                    let mut buf = vec![0u8; 4096];
                    let n = match tls.read(&mut buf).await {
                        Ok(0) => return,
                        Ok(n) => n,
                        Err(_) => return,
                    };
                    if !buf[..n].windows(4).any(|w| w == b"\r\n\r\n") {
                        return;
                    }
                    let head = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                        body.len()
                    );
                    use tokio::io::AsyncWriteExt;
                    if tls.write_all(head.as_bytes()).await.is_err() {
                        return;
                    }
                    let _ = tls.write_all(&body).await;
                });
            }
        });
        addr
    }

    fn certs_to_pem(certs: &[rustls::pki_types::CertificateDer<'static>]) -> Vec<u8> {
        let mut pem_buf = Vec::new();
        for cert in certs {
            pem_buf.extend_from_slice(b"-----BEGIN CERTIFICATE-----\n");
            let b64 = base64_encode(cert.as_ref());
            for chunk in b64.as_bytes().chunks(64) {
                pem_buf.extend_from_slice(chunk);
                pem_buf.push(b'\n');
            }
            pem_buf.extend_from_slice(b"-----END CERTIFICATE-----\n");
        }
        pem_buf
    }

    fn base64_encode(data: &[u8]) -> String {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
        let mut i = 0;
        while i + 3 <= data.len() {
            let b0 = data[i];
            let b1 = data[i + 1];
            let b2 = data[i + 2];
            out.push(TABLE[(b0 >> 2) as usize] as char);
            out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
            out.push(TABLE[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
            out.push(TABLE[(b2 & 0x3f) as usize] as char);
            i += 3;
        }
        match data.len() - i {
            1 => {
                let b0 = data[i];
                out.push(TABLE[(b0 >> 2) as usize] as char);
                out.push(TABLE[((b0 & 0x03) << 4) as usize] as char);
                out.push('=');
                out.push('=');
            }
            2 => {
                let b0 = data[i];
                let b1 = data[i + 1];
                out.push(TABLE[(b0 >> 2) as usize] as char);
                out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
                out.push(TABLE[((b1 & 0x0f) << 2) as usize] as char);
                out.push('=');
            }
            _ => {}
        }
        out
    }

    #[tokio::test]
    async fn https_destination_tls_works_through_http_connect() {
        let sans = vec![rcgen::SanType::IpAddress(
            "127.0.0.1".parse::<std::net::IpAddr>().unwrap(),
        )];
        let (certs, key) = generate_self_signed_cert(sans);
        let origin = start_https_origin(b"https-via-connect".to_vec(), certs.clone(), key).await;
        let proxy = start_connect_proxy(false).await;
        let pem_buf = certs_to_pem(&certs);
        let builder = eggfetch_core::Client::builder()
            .timeout(eggfetch_core::Timeout {
                pool: Some(Duration::from_secs(5)),
                connect: Some(Duration::from_secs(5)),
                write: Some(Duration::from_secs(5)),
                read: Some(Duration::from_secs(5)),
                total: Some(Duration::from_secs(5)),
            })
            .follow_redirects(false)
            .tls_config(
                eggfetch_core::tls::TlsConfig::builder()
                    .ca_certificate_pem(&pem_buf)
                    .expect("test ca is valid pem")
                    .build(),
            );
        let builder =
            apply_route(builder, &egress_section("http", proxy.port())).expect("route applies");
        let client = builder.build();
        let url = format!("https://127.0.0.1:{}/", origin.port());
        let mut response = client
            .get(&url)
            .expect("builds")
            .send()
            .await
            .expect("fetches via https through CONNECT");
        assert!(response.status().is_success());
        let bytes = response.bytes().await.expect("reads").to_vec();
        assert!(String::from_utf8_lossy(&bytes).contains("https-via-connect"));
    }

    #[tokio::test]
    async fn https_hostname_mismatch_fails_through_connect() {
        let sans = vec![rcgen::SanType::IpAddress(
            "10.0.0.1".parse::<std::net::IpAddr>().unwrap(),
        )];
        let (certs, key) = generate_self_signed_cert(sans);
        let origin = start_https_origin(b"must-not-leak".to_vec(), certs.clone(), key).await;
        let proxy = start_connect_proxy(false).await;
        let pem_buf = certs_to_pem(&certs);
        let builder = eggfetch_core::Client::builder()
            .timeout(eggfetch_core::Timeout {
                pool: Some(Duration::from_secs(5)),
                connect: Some(Duration::from_secs(5)),
                write: Some(Duration::from_secs(5)),
                read: Some(Duration::from_secs(5)),
                total: Some(Duration::from_secs(5)),
            })
            .follow_redirects(false)
            .tls_config(
                eggfetch_core::tls::TlsConfig::builder()
                    .ca_certificate_pem(&pem_buf)
                    .expect("test ca is valid pem")
                    .build(),
            );
        let builder =
            apply_route(builder, &egress_section("http", proxy.port())).expect("route applies");
        let client = builder.build();
        let url = format!("https://127.0.0.1:{}/", origin.port());
        let result = client.get(&url).expect("builds").send().await;
        assert!(
            result.is_err(),
            "hostname/certificate mismatch must fail above the proxy tunnel, never silently succeed"
        );
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
