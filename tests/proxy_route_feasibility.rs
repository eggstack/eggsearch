use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use eggfetch_core::{Client, Error, NoProxy, Proxy, ProxyAuth, Timeout};
use eggsearch::core::config::{AppConfig, FetchProxyMode};
use eggsearch::mcp::state::ServerState;
use eggsearch::mcp::tools::{run_web_fetch, WebFetchArgs};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;

const HEADER_LIMIT: usize = 65_536;
const IO_DEADLINE: Duration = Duration::from_secs(5);
const LOGICAL_HOST: &str = "route-probe.invalid";

fn handler_proxy_state(
    route: FetchProxyMode,
    proxy_url: String,
    no_proxy: Vec<String>,
) -> Arc<ServerState> {
    let mut config = AppConfig::default();
    config.fetch.allow_localhost = true;
    config.fetch.allow_private_network = true;
    config.fetch.browser.enabled = false;
    config.fetch.proxy.route = route;
    config.fetch.proxy.url = Some(proxy_url);
    config.fetch.proxy.no_proxy = no_proxy;
    Arc::new(ServerState::build(config).expect("proxy handler state"))
}

fn handler_error_code(error: &eggsearch::mcp::tools::ToolError) -> String {
    error.error_payload()["data"]["code"]
        .as_str()
        .expect("typed handler error code")
        .to_string()
}

#[derive(Default)]
struct HttpCapture {
    requests: Mutex<Vec<Vec<u8>>>,
    auth: Mutex<Vec<Option<String>>>,
    accepted: AtomicUsize,
}

impl HttpCapture {
    fn request_lines(&self) -> Vec<Vec<u8>> {
        self.requests
            .lock()
            .expect("capture lock")
            .iter()
            .map(|head| {
                head.split(|byte| *byte == b'\n')
                    .next()
                    .unwrap_or_default()
                    .strip_suffix(b"\r")
                    .unwrap_or_else(|| head.split(|byte| *byte == b'\n').next().unwrap_or_default())
                    .to_vec()
            })
            .collect()
    }

    fn auth_headers(&self) -> Vec<Option<String>> {
        self.auth.lock().expect("auth lock").clone()
    }

    fn request_count(&self) -> usize {
        self.accepted.load(Ordering::SeqCst)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SocksTarget {
    atyp: u8,
    host: String,
    port: u16,
}

#[derive(Default)]
struct SocksCapture {
    targets: Mutex<Vec<SocksTarget>>,
    accepted: AtomicUsize,
}

async fn read_http_head<R: AsyncRead + Unpin>(reader: &mut R) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    loop {
        let mut buffer = [0u8; 1024];
        let size = match tokio::time::timeout(IO_DEADLINE, reader.read(&mut buffer)).await {
            Ok(Ok(size)) if size > 0 => size,
            _ => return None,
        };
        if output.len().saturating_add(size) > HEADER_LIMIT {
            return None;
        }
        output.extend_from_slice(&buffer[..size]);
        if output.windows(4).any(|window| window == b"\r\n\r\n") {
            return Some(output);
        }
    }
}

async fn read_exact_bounded<R: AsyncRead + Unpin>(reader: &mut R, output: &mut [u8]) -> bool {
    matches!(
        tokio::time::timeout(IO_DEADLINE, reader.read_exact(output)).await,
        Ok(Ok(_))
    )
}

fn request_line_and_auth(head: &[u8]) -> (Vec<u8>, Option<String>) {
    let line = head
        .split(|byte| *byte == b'\n')
        .next()
        .unwrap_or_default()
        .strip_suffix(b"\r")
        .unwrap_or_else(|| head.split(|byte| *byte == b'\n').next().unwrap_or_default())
        .to_vec();
    let text = String::from_utf8_lossy(head);
    let auth = text.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("proxy-authorization")
            .then(|| value.trim().to_string())
    });
    (line, auth)
}

async fn write_proxy_error<S: AsyncWrite + Unpin>(stream: &mut S, status: &[u8]) {
    let _ = stream.write_all(status).await;
}

async fn handle_connect<S: AsyncRead + AsyncWrite + Unpin>(
    mut inbound: S,
    capture: Arc<HttpCapture>,
    expected_auth: Option<String>,
) {
    let Some(head) = read_http_head(&mut inbound).await else {
        return;
    };
    let (line, auth) = request_line_and_auth(&head);
    capture.requests.lock().expect("capture lock").push(head);
    capture.auth.lock().expect("auth lock").push(auth.clone());
    if expected_auth
        .as_deref()
        .is_some_and(|expected| auth.as_deref() != Some(expected))
    {
        write_proxy_error(
            &mut inbound,
            b"HTTP/1.1 407 Proxy Authentication Required\r\nproxy-authenticate: Basic realm=\"route-probe\"\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
        )
        .await;
        return;
    }
    let authority = line
        .strip_prefix(b"CONNECT ")
        .and_then(|rest| rest.split(|byte| byte.is_ascii_whitespace()).next())
        .and_then(|bytes| std::str::from_utf8(bytes).ok());
    let Some(destination) = authority.and_then(|value| value.parse::<SocketAddr>().ok()) else {
        write_proxy_error(
            &mut inbound,
            b"HTTP/1.1 403 Forbidden\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
        )
        .await;
        return;
    };
    if !destination.ip().is_loopback() {
        write_proxy_error(
            &mut inbound,
            b"HTTP/1.1 403 Forbidden\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
        )
        .await;
        return;
    }
    let Ok(Ok(mut upstream)) =
        tokio::time::timeout(Duration::from_secs(1), TcpStream::connect(destination)).await
    else {
        write_proxy_error(
            &mut inbound,
            b"HTTP/1.1 502 Bad Gateway\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
        )
        .await;
        return;
    };
    if inbound
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .await
        .is_ok()
    {
        let _ = tokio::io::copy_bidirectional(&mut inbound, &mut upstream).await;
    }
}

async fn start_connect_proxy(expected_auth: Option<String>) -> (SocketAddr, Arc<HttpCapture>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("proxy bind");
    let address = listener.local_addr().expect("proxy address");
    let capture = Arc::new(HttpCapture::default());
    let task_capture = capture.clone();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                break;
            };
            task_capture.accepted.fetch_add(1, Ordering::SeqCst);
            let capture = task_capture.clone();
            let expected_auth = expected_auth.clone();
            tokio::spawn(async move {
                handle_connect(stream, capture, expected_auth).await;
            });
        }
    });
    (address, capture)
}

async fn start_stalling_proxy() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("proxy bind");
    let address = listener.local_addr().expect("proxy address");
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                if read_http_head(&mut stream).await.is_some() {
                    let mut byte = [0u8; 1];
                    let _ =
                        tokio::time::timeout(Duration::from_secs(1), stream.read(&mut byte)).await;
                }
            });
        }
    });
    address
}

async fn forward_http_request(mut inbound: TcpStream, capture: Arc<HttpCapture>) {
    let Some(head) = read_http_head(&mut inbound).await else {
        return;
    };
    let (line, auth) = request_line_and_auth(&head);
    capture
        .requests
        .lock()
        .expect("capture lock")
        .push(head.clone());
    capture.auth.lock().expect("auth lock").push(auth);
    let text = String::from_utf8_lossy(&line);
    let mut parts = text.split_ascii_whitespace();
    let method = parts.next().unwrap_or_default();
    let Some(uri) = parts.next().and_then(|value| url::Url::parse(value).ok()) else {
        write_proxy_error(
            &mut inbound,
            b"HTTP/1.1 400 Bad Request\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
        )
        .await;
        return;
    };
    let Some(host) = uri.host_str() else {
        return;
    };
    let port = uri.port_or_known_default().unwrap_or(80);
    let Ok(destination) = format!("{host}:{port}").parse::<SocketAddr>() else {
        write_proxy_error(
            &mut inbound,
            b"HTTP/1.1 403 Forbidden\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
        )
        .await;
        return;
    };
    if !destination.ip().is_loopback() {
        write_proxy_error(
            &mut inbound,
            b"HTTP/1.1 403 Forbidden\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
        )
        .await;
        return;
    }
    let Ok(Ok(mut upstream)) =
        tokio::time::timeout(Duration::from_secs(1), TcpStream::connect(destination)).await
    else {
        return;
    };
    let path = if uri.path().is_empty() {
        "/"
    } else {
        uri.path()
    };
    let target = match uri.query() {
        Some(query) => format!("{path}?{query}"),
        None => path.to_string(),
    };
    let mut request = format!("{method} {target} HTTP/1.1\r\n").into_bytes();
    if let Some(end) = head.windows(4).position(|window| window == b"\r\n\r\n") {
        let headers = String::from_utf8_lossy(&head[line.len() + 2..end]);
        for header in headers.lines() {
            if !header
                .to_ascii_lowercase()
                .starts_with("proxy-authorization:")
            {
                request.extend_from_slice(header.as_bytes());
                request.extend_from_slice(b"\r\n");
            }
        }
    }
    request.extend_from_slice(b"\r\n");
    if upstream.write_all(&request).await.is_ok() {
        let _ = tokio::io::copy_bidirectional(&mut inbound, &mut upstream).await;
    }
}

async fn start_forward_proxy() -> (SocketAddr, Arc<HttpCapture>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("proxy bind");
    let address = listener.local_addr().expect("proxy address");
    let capture = Arc::new(HttpCapture::default());
    let task_capture = capture.clone();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                break;
            };
            task_capture.accepted.fetch_add(1, Ordering::SeqCst);
            let capture = task_capture.clone();
            tokio::spawn(async move {
                forward_http_request(stream, capture).await;
            });
        }
    });
    (address, capture)
}

async fn handle_socks(mut inbound: TcpStream, capture: Arc<SocksCapture>) {
    let mut greeting = [0u8; 2];
    if !read_exact_bounded(&mut inbound, &mut greeting).await || greeting[0] != 5 {
        return;
    }
    let method_count = usize::from(greeting[1]);
    if method_count == 0 || method_count > 32 {
        return;
    }
    let mut methods = vec![0u8; method_count];
    if !read_exact_bounded(&mut inbound, &mut methods).await {
        return;
    }
    if !methods.contains(&0) {
        let _ = inbound.write_all(&[5, 255]).await;
        return;
    }
    if inbound.write_all(&[5, 0]).await.is_err() {
        return;
    }
    let mut request = [0u8; 4];
    if !read_exact_bounded(&mut inbound, &mut request).await || request[0] != 5 || request[1] != 1 {
        return;
    }
    let (host, ip) = match request[3] {
        1 => {
            let mut octets = [0u8; 4];
            if !read_exact_bounded(&mut inbound, &mut octets).await {
                return;
            }
            let address = Ipv4Addr::from(octets);
            (address.to_string(), Some(IpAddr::V4(address)))
        }
        3 => {
            let mut length = [0u8; 1];
            if !read_exact_bounded(&mut inbound, &mut length).await {
                return;
            }
            let mut domain = vec![0u8; usize::from(length[0])];
            if !read_exact_bounded(&mut inbound, &mut domain).await {
                return;
            }
            (String::from_utf8_lossy(&domain).into_owned(), None)
        }
        4 => {
            let mut octets = [0u8; 16];
            if !read_exact_bounded(&mut inbound, &mut octets).await {
                return;
            }
            let address = Ipv6Addr::from(octets);
            (address.to_string(), Some(IpAddr::V6(address)))
        }
        _ => return,
    };
    let mut port = [0u8; 2];
    if !read_exact_bounded(&mut inbound, &mut port).await {
        return;
    }
    let port = u16::from_be_bytes(port);
    capture
        .targets
        .lock()
        .expect("SOCKS capture lock")
        .push(SocksTarget {
            atyp: request[3],
            host,
            port,
        });
    let Some(ip) = ip.filter(IpAddr::is_loopback) else {
        let _ = inbound.write_all(&[5, 4, 0, 1, 0, 0, 0, 0, 0, 0]).await;
        return;
    };
    let Ok(Ok(mut upstream)) = tokio::time::timeout(
        Duration::from_secs(1),
        TcpStream::connect(SocketAddr::new(ip, port)),
    )
    .await
    else {
        let _ = inbound.write_all(&[5, 5, 0, 1, 0, 0, 0, 0, 0, 0]).await;
        return;
    };
    if inbound
        .write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0])
        .await
        .is_ok()
    {
        let _ = tokio::io::copy_bidirectional(&mut inbound, &mut upstream).await;
    }
}

async fn start_socks_proxy() -> (SocketAddr, Arc<SocksCapture>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("SOCKS bind");
    let address = listener.local_addr().expect("SOCKS address");
    let capture = Arc::new(SocksCapture::default());
    let task_capture = capture.clone();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                break;
            };
            task_capture.accepted.fetch_add(1, Ordering::SeqCst);
            let capture = task_capture.clone();
            tokio::spawn(async move {
                handle_socks(stream, capture).await;
            });
        }
    });
    (address, capture)
}

async fn answer_origin<S: AsyncRead + AsyncWrite + Unpin>(
    mut stream: S,
    response: Vec<u8>,
    capture: Arc<Mutex<Vec<Vec<u8>>>>,
) {
    let Some(head) = read_http_head(&mut stream).await else {
        return;
    };
    capture.lock().expect("origin capture lock").push(head);
    let _ = stream.write_all(&response).await;
}

async fn start_http_origin(response: Vec<u8>) -> (SocketAddr, Arc<Mutex<Vec<Vec<u8>>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("origin bind");
    let address = listener.local_addr().expect("origin address");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let task_requests = requests.clone();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                break;
            };
            let requests = task_requests.clone();
            let response = response.clone();
            tokio::spawn(async move {
                answer_origin(stream, response, requests).await;
            });
        }
    });
    (address, requests)
}

fn response(status: &str, headers: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
    let mut output = format!(
        "HTTP/1.1 {status}\r\ncontent-length: {}\r\nconnection: close\r\n",
        body.len()
    )
    .into_bytes();
    for (name, value) in headers {
        output.extend_from_slice(format!("{name}: {value}\r\n").as_bytes());
    }
    output.extend_from_slice(b"\r\n");
    output.extend_from_slice(body);
    output
}

fn self_signed_certificate(
    subject_alt_names: Vec<rcgen::SanType>,
) -> (
    Vec<rustls::pki_types::CertificateDer<'static>>,
    rustls::pki_types::PrivateKeyDer<'static>,
    Vec<u8>,
) {
    let key_pair = rcgen::KeyPair::generate().expect("key pair");
    let mut params = rcgen::CertificateParams::default();
    params.subject_alt_names = subject_alt_names;
    let certificate = params
        .self_signed(&key_pair)
        .expect("self-signed certificate");
    let der = rustls::pki_types::CertificateDer::from(certificate.der().to_vec());
    let pem_body = base64::engine::general_purpose::STANDARD.encode(der.as_ref());
    let mut pem = String::from("-----BEGIN CERTIFICATE-----\n");
    for chunk in pem_body.as_bytes().chunks(64) {
        pem.push_str(std::str::from_utf8(chunk).expect("base64 is ASCII"));
        pem.push('\n');
    }
    pem.push_str("-----END CERTIFICATE-----\n");
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(key_pair.serialize_der().into());
    (vec![der], key, pem.into_bytes())
}

fn rustls_server_config(
    certs: Vec<rustls::pki_types::CertificateDer<'static>>,
    key: rustls::pki_types::PrivateKeyDer<'static>,
) -> Arc<rustls::ServerConfig> {
    Arc::new(
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

fn client_tls_config(pem: &[u8]) -> eggfetch_core::tls::TlsConfig {
    eggfetch_core::tls::TlsConfig::builder()
        .ca_certificate_pem(pem)
        .expect("test CA PEM")
        .build()
}

async fn start_tls_origin(
    response: Vec<u8>,
    certs: Vec<rustls::pki_types::CertificateDer<'static>>,
    key: rustls::pki_types::PrivateKeyDer<'static>,
) -> (SocketAddr, Arc<Mutex<Vec<Vec<u8>>>>) {
    let acceptor = TlsAcceptor::from(rustls_server_config(certs, key));
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("TLS origin bind");
    let address = listener.local_addr().expect("TLS origin address");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let task_requests = requests.clone();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                break;
            };
            let acceptor = acceptor.clone();
            let requests = task_requests.clone();
            let response = response.clone();
            tokio::spawn(async move {
                if let Ok(Ok(stream)) =
                    tokio::time::timeout(IO_DEADLINE, acceptor.accept(stream)).await
                {
                    answer_origin(stream, response, requests).await;
                }
            });
        }
    });
    (address, requests)
}

async fn start_tls_connect_proxy(
    certs: Vec<rustls::pki_types::CertificateDer<'static>>,
    key: rustls::pki_types::PrivateKeyDer<'static>,
) -> (SocketAddr, Arc<HttpCapture>) {
    let acceptor = TlsAcceptor::from(rustls_server_config(certs, key));
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("TLS proxy bind");
    let address = listener.local_addr().expect("TLS proxy address");
    let capture = Arc::new(HttpCapture::default());
    let task_capture = capture.clone();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                break;
            };
            task_capture.accepted.fetch_add(1, Ordering::SeqCst);
            let acceptor = acceptor.clone();
            let capture = task_capture.clone();
            tokio::spawn(async move {
                if let Ok(Ok(stream)) =
                    tokio::time::timeout(IO_DEADLINE, acceptor.accept(stream)).await
                {
                    handle_connect(stream, capture, None).await;
                }
            });
        }
    });
    (address, capture)
}

fn timeout_policy(duration: Duration) -> Timeout {
    Timeout {
        pool: Some(duration),
        connect: Some(duration),
        write: Some(duration),
        read: Some(duration),
        total: Some(duration),
    }
}

fn client_for(proxy: Proxy) -> Client {
    Client::builder()
        .proxy(proxy)
        .timeout(timeout_policy(Duration::from_secs(4)))
        .max_decoded_body_size(4096)
        .follow_redirects(false)
        .build()
}

fn origin_request_text(requests: &Mutex<Vec<Vec<u8>>>) -> String {
    String::from_utf8_lossy(
        requests
            .lock()
            .expect("origin capture lock")
            .first()
            .expect("origin received request"),
    )
    .to_ascii_lowercase()
}

#[tokio::test]
async fn http_connect_pins_ip_and_preserves_logical_host_and_tls_identity() {
    let (certs, key, pem) = self_signed_certificate(vec![rcgen::SanType::DnsName(
        "route-probe.invalid".try_into().expect("DNS SAN"),
    )]);
    let (origin, origin_requests) =
        start_tls_origin(response("200 OK", &[], b"connect-pinned"), certs, key).await;
    let (proxy, proxy_capture) = start_connect_proxy(None).await;
    let client = Client::builder()
        .proxy(Proxy::all(&format!("http://{proxy}")).expect("HTTP proxy"))
        .tls_config(client_tls_config(&pem))
        .timeout(timeout_policy(Duration::from_secs(4)))
        .max_decoded_body_size(4096)
        .follow_redirects(false)
        .build();
    let url = format!("https://{LOGICAL_HOST}:{}/", origin.port());
    let mut result = client
        .get(&url)
        .expect("request")
        .proxy_target_addresses([origin])
        .send()
        .await
        .expect("pinned HTTPS CONNECT succeeds");
    assert_eq!(result.status(), 200);
    assert_eq!(
        result.bytes().await.expect("bounded body"),
        b"connect-pinned"[..]
    );
    let expected_line = format!("CONNECT {origin} HTTP/1.1");
    assert_eq!(
        proxy_capture
            .request_lines()
            .first()
            .expect("CONNECT captured"),
        expected_line.as_bytes()
    );
    let origin_text = origin_request_text(&origin_requests);
    assert!(origin_text.contains(&format!("host: {LOGICAL_HOST}")));
    assert!(!origin_text.contains("127.0.0.1"));
    println!("CONNECT request line bytes: {:?}", expected_line.as_bytes());
    println!("origin Host retained: {LOGICAL_HOST} (transport omits the non-default port)");
    println!("DNS observation: .invalid host succeeded with pinned loopback IP; proxy received only the IP authority");
}

#[tokio::test]
async fn plaintext_forward_works_and_rejects_a_target_pin() {
    let (origin, _) = start_http_origin(response("200 OK", &[], b"forward-ok")).await;
    let (proxy, capture) = start_forward_proxy().await;
    let proxy_config = Proxy::all(&format!("http://{proxy}")).expect("HTTP proxy");
    let client = client_for(proxy_config);
    let url = format!("http://127.0.0.1:{}/", origin.port());
    let mut result = client
        .get(&url)
        .expect("forward request")
        .send()
        .await
        .expect("plaintext forward proxy succeeds");
    assert_eq!(
        result.bytes().await.expect("bounded body"),
        b"forward-ok"[..]
    );
    let before_pin = capture.request_count();
    let error = client
        .get(&url)
        .expect("pinned forward request")
        .proxy_target_addresses([origin])
        .send()
        .await
        .expect_err("plaintext forward proxy must reject target pin");
    assert_eq!(capture.request_count(), before_pin);
    println!(
        "HTTP forward over plaintext: routed; pinned variant rejected before proxy I/O: {error:?}"
    );
}

#[tokio::test]
async fn socks5_uses_pinned_ip_and_socks5h_keeps_remote_hostname_unpinnable() {
    let (origin, origin_requests) =
        start_http_origin(response("200 OK", &[], b"socks-pinned")).await;
    let (socks5, socks5_capture) = start_socks_proxy().await;
    let client = client_for(Proxy::all(&format!("socks5://{socks5}")).expect("SOCKS5 proxy"));
    let url = format!("http://{LOGICAL_HOST}:{}/", origin.port());
    let mut result = client
        .get(&url)
        .expect("SOCKS5 request")
        .proxy_target_addresses([origin])
        .send()
        .await
        .expect("pinned SOCKS5 request succeeds");
    assert_eq!(
        result.bytes().await.expect("bounded body"),
        b"socks-pinned"[..]
    );
    let target = socks5_capture
        .targets
        .lock()
        .expect("SOCKS capture lock")
        .first()
        .cloned()
        .expect("SOCKS target captured");
    assert_eq!(target.atyp, 1);
    assert_eq!(target.host, origin.ip().to_string());
    assert_eq!(target.port, origin.port());
    assert!(origin_request_text(&origin_requests)
        .contains(&format!("host: {LOGICAL_HOST}:{}", origin.port())));

    let (socks5h, socks5h_capture) = start_socks_proxy().await;
    let socks5h_client =
        client_for(Proxy::all(&format!("socks5h://{socks5h}")).expect("SOCKS5H proxy"));
    let pinned_error = socks5h_client
        .get(&url)
        .expect("pinned SOCKS5H request")
        .proxy_target_addresses([origin])
        .send()
        .await
        .expect_err("SOCKS5H must reject target pin");
    assert_eq!(socks5h_capture.accepted.load(Ordering::SeqCst), 0);
    let remote_error = socks5h_client
        .get(&url)
        .expect("remote-DNS request")
        .send()
        .await
        .expect_err("loopback fixture refuses domain resolution");
    let remote_target = socks5h_capture
        .targets
        .lock()
        .expect("SOCKS capture lock")
        .first()
        .cloned()
        .expect("SOCKS5H target captured");
    assert_eq!(remote_target.atyp, 3);
    assert_eq!(remote_target.host, LOGICAL_HOST);
    println!("SOCKS5 pinned target: {target:?}; logical Host retained");
    println!("SOCKS5H pin error: {pinned_error:?}; remote-name error: {remote_error:?}; target: {remote_target:?}");
}

#[tokio::test]
async fn proxy_authentication_succeeds_and_bad_credentials_fail_closed() {
    let (certs, key, pem) = self_signed_certificate(vec![rcgen::SanType::DnsName(
        "route-probe.invalid".try_into().expect("DNS SAN"),
    )]);
    let (origin, _) = start_tls_origin(response("200 OK", &[], b"auth-ok"), certs, key).await;
    let expected = format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode("route-user:route-pass")
    );
    let (proxy, capture) = start_connect_proxy(Some(expected.clone())).await;
    let client = Client::builder()
        .proxy(
            Proxy::all(&format!("http://{proxy}"))
                .expect("HTTP proxy")
                .auth(ProxyAuth::basic("route-user", "route-pass").expect("test credentials")),
        )
        .tls_config(client_tls_config(&pem))
        .timeout(timeout_policy(Duration::from_secs(4)))
        .max_decoded_body_size(4096)
        .follow_redirects(false)
        .build();
    let url = format!("https://{LOGICAL_HOST}:{}/", origin.port());
    let mut result = client
        .get(&url)
        .expect("authenticated request")
        .proxy_target_addresses([origin])
        .send()
        .await
        .expect("proxy auth accepted");
    assert_eq!(result.bytes().await.expect("bounded body"), b"auth-ok"[..]);
    assert_eq!(
        capture.auth_headers().first().and_then(Option::as_deref),
        Some(expected.as_str())
    );

    let (bad_proxy, bad_capture) = start_connect_proxy(Some(expected)).await;
    let bad_client = Client::builder()
        .proxy(
            Proxy::all(&format!("http://{bad_proxy}"))
                .expect("HTTP proxy")
                .auth(ProxyAuth::basic("route-user", "wrong-pass").expect("test credentials")),
        )
        .tls_config(client_tls_config(&pem))
        .timeout(timeout_policy(Duration::from_secs(4)))
        .follow_redirects(false)
        .build();
    let error = bad_client
        .get(&url)
        .expect("bad-auth request")
        .proxy_target_addresses([origin])
        .send()
        .await
        .expect_err("proxy auth rejection must fail closed");
    assert_eq!(bad_capture.request_count(), 1);
    println!("proxy authentication: accepted correct Basic credentials; wrong credentials returned error {error:?}");
}

#[tokio::test]
async fn https_proxy_endpoint_uses_separate_proxy_tls_trust() {
    let (origin_certs, origin_key, origin_pem) =
        self_signed_certificate(vec![rcgen::SanType::DnsName(
            "route-probe.invalid".try_into().expect("origin DNS SAN"),
        )]);
    let (origin, _) = start_tls_origin(
        response("200 OK", &[], b"https-proxy-ok"),
        origin_certs,
        origin_key,
    )
    .await;
    let (proxy_certs, proxy_key, proxy_pem) =
        self_signed_certificate(vec![rcgen::SanType::IpAddress(IpAddr::V4(
            Ipv4Addr::LOCALHOST,
        ))]);
    let (proxy, capture) = start_tls_connect_proxy(proxy_certs, proxy_key).await;
    let proxy = Proxy::all(&format!("https://{proxy}"))
        .expect("HTTPS proxy")
        .with_proxy_tls_config(client_tls_config(&proxy_pem))
        .resolved_addresses([proxy])
        .expect("pinned proxy endpoint");
    let client = Client::builder()
        .proxy(proxy)
        .tls_config(client_tls_config(&origin_pem))
        .timeout(timeout_policy(Duration::from_secs(4)))
        .max_decoded_body_size(4096)
        .follow_redirects(false)
        .build();
    let url = format!("https://{LOGICAL_HOST}:{}/", origin.port());
    let mut result = client
        .get(&url)
        .expect("request")
        .proxy_target_addresses([origin])
        .send()
        .await
        .expect("HTTPS proxy and pinned origin succeed");
    assert_eq!(
        result.bytes().await.expect("bounded body"),
        b"https-proxy-ok"[..]
    );
    assert_eq!(
        capture.request_lines().first().expect("CONNECT captured"),
        format!("CONNECT {origin} HTTP/1.1").as_bytes()
    );
    println!("HTTPS proxy TLS: success with distinct proxy and origin trust roots");
}

#[tokio::test]
async fn malformed_proxy_is_rejected_and_stalled_connect_times_out() {
    let malformed = Proxy::all("not-a-proxy-url").expect_err("malformed route rejected");
    assert!(matches!(malformed, Error::InvalidProxyUrl(_)));
    let proxy = start_stalling_proxy().await;
    let client = Client::builder()
        .proxy(Proxy::all(&format!("http://{proxy}")).expect("HTTP proxy"))
        .timeout(timeout_policy(Duration::from_millis(150)))
        .follow_redirects(false)
        .build();
    let result = client
        .get("https://route-probe.invalid:443/")
        .expect("request")
        .send()
        .await;
    let error = result.expect_err("stalled proxy must time out");
    assert!(matches!(error, Error::Timeout { .. }));
    println!("malformed proxy error: {malformed:?}; stalled CONNECT error: {error:?}");
}

#[tokio::test]
async fn cross_origin_redirect_rejects_a_pinned_proxy_target() {
    let (certs, key, pem) = self_signed_certificate(vec![rcgen::SanType::DnsName(
        "route-probe.invalid".try_into().expect("DNS SAN"),
    )]);
    let (origin, _) = start_tls_origin(
        response(
            "302 Found",
            &[("location", "https://second.invalid/next")],
            b"",
        ),
        certs,
        key,
    )
    .await;
    let (proxy, _) = start_connect_proxy(None).await;
    let client = Client::builder()
        .proxy(Proxy::all(&format!("http://{proxy}")).expect("HTTP proxy"))
        .tls_config(client_tls_config(&pem))
        .timeout(timeout_policy(Duration::from_secs(4)))
        .follow_redirects(true)
        .build();
    let url = format!("https://{LOGICAL_HOST}:{}/", origin.port());
    let error = client
        .get(&url)
        .expect("request")
        .proxy_target_addresses([origin])
        .send()
        .await
        .expect_err("cross-origin redirect cannot reuse pinned address");
    assert!(matches!(error, Error::ResolvedTargetRedirect));
    println!("cross-origin pinned redirect error: {error:?}");
}

#[tokio::test]
async fn no_proxy_bypasses_route_and_fetch_policy_refuses_loopback() {
    let (origin, _) = start_http_origin(response("200 OK", &[], b"direct-bypass")).await;
    let (proxy, capture) = start_connect_proxy(None).await;
    let client = client_for(
        Proxy::all(&format!("http://{proxy}"))
            .expect("HTTP proxy")
            .no_proxy(NoProxy::parse("127.0.0.1").expect("NO_PROXY rule")),
    );
    let url = format!("http://127.0.0.1:{}/", origin.port());
    let mut result = client
        .get(&url)
        .expect("bypass request")
        .send()
        .await
        .expect("NO_PROXY direct route succeeds");
    assert_eq!(
        result.bytes().await.expect("bounded body"),
        b"direct-bypass"[..]
    );
    assert_eq!(capture.request_count(), 0);
    let limits = eggsearch::fetch::limits::FetchLimits::default();
    let target = url::Url::parse(&url).expect("loopback URL");
    let refusal = eggsearch::fetch::limits::validate_fetch_target(&target, &limits)
        .await
        .expect_err("fetch policy refuses loopback");
    assert!(matches!(
        refusal,
        eggsearch::fetch::FetchError::PrivateNetworkBlocked(_)
    ));
    println!("NO_PROXY routed direct with zero proxy accepts; fetch loopback refusal: {refusal:?}");
}

#[cfg(feature = "egress")]
#[tokio::test]
async fn eggress_dialer_receives_logical_host_and_refuses_direct_address_pin() {
    use eggsearch::core::config::{EgressHopConfig, EgressSection};
    use eggsearch::fetch::egress::{build_connector, EggressDialer};

    let (origin, _) = start_http_origin(response("200 OK", &[], b"not-reached")).await;
    let (proxy, capture) = start_connect_proxy(None).await;
    let section = EgressSection {
        enabled: true,
        hops: vec![EgressHopConfig {
            scheme: "http".to_string(),
            host: "127.0.0.1".to_string(),
            port: proxy.port(),
            username: None,
            password_env: None,
        }],
    };
    let connector = build_connector(&section).expect("single-hop Eggress connector");
    let dialer = EggressDialer::new(connector, "loopback HTTP hop".to_string());
    let client = Client::builder()
        .dialer(dialer)
        .timeout(timeout_policy(Duration::from_secs(2)))
        .follow_redirects(false)
        .build();
    let url = format!("http://{LOGICAL_HOST}:{}/", origin.port());
    let routed_error = client
        .get(&url)
        .expect("un-pinned Eggress request")
        .send()
        .await
        .expect_err("fixture refuses proxy DNS for the logical host");
    let expected_line = format!("CONNECT {LOGICAL_HOST}:{} HTTP/1.1", origin.port());
    assert_eq!(
        capture
            .request_lines()
            .first()
            .expect("Eggress CONNECT captured"),
        expected_line.as_bytes()
    );
    let before_pin = capture.request_count();
    let pin_error = client
        .get(&url)
        .expect("pinned Eggress request")
        .resolved_addresses([origin])
        .send()
        .await
        .expect_err("Eggress custom dialer cannot consume resolved target");
    assert_eq!(capture.request_count(), before_pin);
    println!("Eggress CONNECT authority: {expected_line}; unpinned error: {routed_error:?}; resolved pin refused before dial: {pin_error:?}");
}

#[tokio::test]
async fn eggsearch_handler_pins_connect_authority_and_maps_proxy_authentication_failure() {
    let (certs, key, _) = self_signed_certificate(vec![rcgen::SanType::IpAddress(IpAddr::V4(
        Ipv4Addr::LOCALHOST,
    ))]);
    let (origin, origin_requests) =
        start_tls_origin(response("200 OK", &[], b"not-reached"), certs, key).await;
    let (proxy, capture) = start_connect_proxy(Some("Basic rejected".to_string())).await;
    let state = handler_proxy_state(
        FetchProxyMode::ProxyRequired,
        format!("http://{proxy}"),
        Vec::new(),
    );
    let error = run_web_fetch(
        state,
        WebFetchArgs {
            include_selectors: None,
            exclude_selectors: None,
            attributes: None,
            url: format!("https://127.0.0.1:{}/", origin.port()),
            ..Default::default()
        },
    )
    .await
    .expect_err("proxy authentication rejection must fail closed");
    assert_eq!(handler_error_code(&error), "proxy_authentication_failed");
    let expected_line = format!("CONNECT {origin} HTTP/1.1");
    assert_eq!(
        capture.request_lines().first().expect("CONNECT captured"),
        expected_line.as_bytes()
    );
    assert_eq!(capture.request_count(), 1);
    assert!(origin_requests.lock().expect("origin requests").is_empty());
}

#[tokio::test]
async fn eggsearch_handler_socks5_uses_the_pinned_ip_target() {
    let (certs, key, _) = self_signed_certificate(vec![rcgen::SanType::IpAddress(IpAddr::V4(
        Ipv4Addr::LOCALHOST,
    ))]);
    let (origin, origin_requests) =
        start_tls_origin(response("200 OK", &[], b"socks-not-reached"), certs, key).await;
    let (proxy, capture) = start_socks_proxy().await;
    let state = handler_proxy_state(
        FetchProxyMode::ProxyRequired,
        format!("socks5://{proxy}"),
        Vec::new(),
    );
    let error = run_web_fetch(
        state,
        WebFetchArgs {
            include_selectors: None,
            exclude_selectors: None,
            attributes: None,
            url: format!("https://127.0.0.1:{}/", origin.port()),
            ..Default::default()
        },
    )
    .await
    .expect_err("test TLS certificate is intentionally untrusted");
    assert_eq!(handler_error_code(&error), "proxy_connect_failed");
    let target = capture
        .targets
        .lock()
        .expect("SOCKS capture lock")
        .first()
        .cloned()
        .expect("SOCKS target captured");
    assert_eq!(target.atyp, 1);
    assert_eq!(target.host, origin.ip().to_string());
    assert_eq!(target.port, origin.port());
    assert!(origin_requests.lock().expect("origin requests").is_empty());
}

#[tokio::test]
async fn eggsearch_handler_rejects_redirects_that_change_no_proxy_route() {
    let location = "https://localhost:443/next";
    let redirect = response("302 Found", &[("location", location)], b"");
    let (origin, requests) = start_http_origin(redirect).await;
    let (proxy, capture) = start_connect_proxy(None).await;
    let state = handler_proxy_state(
        FetchProxyMode::Proxy,
        format!("http://{proxy}"),
        vec!["127.0.0.1".to_string()],
    );
    let error = run_web_fetch(
        state,
        WebFetchArgs {
            include_selectors: None,
            exclude_selectors: None,
            attributes: None,
            url: format!("http://127.0.0.1:{}/", origin.port()),
            ..Default::default()
        },
    )
    .await
    .expect_err("redirect from no_proxy to proxy route must be refused");
    assert_eq!(
        handler_error_code(&error),
        "proxy_route_changed_by_redirect"
    );
    assert_eq!(requests.lock().expect("origin requests").len(), 1);
    assert_eq!(capture.request_count(), 0);
}
