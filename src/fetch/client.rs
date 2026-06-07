//! HTTP client for fetching URLs.

use std::time::Duration;

use futures::StreamExt;
use reqwest::Client;

use super::extract::extract_content;
use super::limits::{validate_url, validate_url_with_dns, FetchLimits};
use super::types::FetchError;
use crate::core::fetch::{ExtractMode, FetchTrust, WebFetchResponse};

/// HTTP client for fetching URLs.
pub struct FetchClient {
    client: Client,
    limits: FetchLimits,
    #[allow(dead_code)]
    user_agent: String,
}

impl FetchClient {
    /// Creates a new FetchClient with the given limits and user agent.
    pub fn new(limits: FetchLimits, user_agent: String) -> anyhow::Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_millis(limits.timeout_ms))
            .redirect(reqwest::redirect::Policy::limited(limits.redirect_limit))
            .user_agent(&user_agent)
            .build()?;
        Ok(Self {
            client,
            limits,
            user_agent,
        })
    }

    /// Fetches a URL and extracts content.
    ///
    /// # Arguments
    ///
    /// * `url_str` - The URL to fetch
    /// * `max_chars` - Maximum characters to extract (None for default)
    /// * `extract_mode` - The extraction mode to use
    /// * `include_links` - Whether to include extracted links
    #[allow(clippy::too_many_arguments)]
    pub async fn fetch(
        &self,
        url_str: &str,
        max_chars: Option<usize>,
        extract_mode: ExtractMode,
        include_links: bool,
    ) -> Result<WebFetchResponse, FetchError> {
        let url = validate_url(url_str, &self.limits)?;

        // Defense-in-depth DNS check. The sync `validate_url` already
        // blocked obvious local/private literals; this resolves the
        // host and rejects any address in a blocked range. Note the
        // TOCTOU window: between this check and the actual HTTP
        // request, DNS could return a different address. For a single
        // tenant MCP server with no real-time-critical attack surface
        // this is acceptable; the same pattern is used by SSRF proxies
        // that re-resolve per request.
        let url = validate_url_with_dns(url, &self.limits).await?;

        let max_chars = max_chars
            .unwrap_or(self.limits.max_chars_default)
            .min(self.limits.max_chars_cap);

        let response = self.client.get(url.clone()).send().await.map_err(|e| {
            if e.is_timeout() {
                FetchError::Timeout(self.limits.timeout_ms)
            } else {
                FetchError::NetworkError(e.to_string())
            }
        })?;

        let final_url = response.url().to_string();
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        // Pre-check Content-Length for honest servers. The streaming
        // body cap below remains the authoritative upper bound for
        // chunked/encoded responses; this is an early bailout.
        if let Some(cl_header) = response.headers().get("content-length") {
            if let Some(content_length) = cl_header.to_str().ok().and_then(|s| s.parse::<usize>().ok()) {
                if content_length > self.limits.max_bytes {
                    return Err(FetchError::ContentTooLarge(content_length, self.limits.max_bytes));
                }
            }
        }

        if !(200..300).contains(&status) {
            return Err(FetchError::HttpStatus(status, format!("HTTP {}", status)));
        }

        let is_html = content_type
            .as_ref()
            .map(|ct| ct.starts_with("text/html") || ct.starts_with("application/xhtml"))
            .unwrap_or(false);
        let is_text = content_type
            .as_ref()
            .map(|ct| ct.starts_with("text/plain"))
            .unwrap_or(false);

        if !is_html && !is_text {
            return Err(FetchError::UnsupportedContentType(
                content_type.unwrap_or_else(|| "unknown".into()),
            ));
        }

        let mut body = Vec::new();
        let mut stream = response.bytes_stream();
        let mut truncated = false;

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| FetchError::NetworkError(e.to_string()))?;
            if body.len() + chunk.len() > self.limits.max_bytes {
                let remaining = self.limits.max_bytes.saturating_sub(body.len());
                if remaining > 0 {
                    body.extend_from_slice(&chunk[..remaining]);
                }
                truncated = true;
                break;
            }
            body.extend_from_slice(&chunk);
        }

        let (title, description, text, links, extract_warnings) =
            if extract_mode == ExtractMode::MetadataOnly {
                if is_html {
                    let extractor = super::extract::HtmlExtractor::new(&body, &final_url);
                    let (t, d, _, l, w) = extractor.extract(max_chars, include_links);
                    (t, d, None, l, w)
                } else {
                    (None, None, None, Vec::new(), Vec::new())
                }
            } else if is_html {
                let (t, d, txt, l, w) = extract_content(&body, &final_url, max_chars, include_links);
                (t, d, Some(txt), l, w)
            } else {
                let text = String::from_utf8_lossy(&body)
                    .chars()
                    .take(max_chars)
                    .collect::<String>();
                (None, None, Some(text), Vec::new(), Vec::new())
            };

        let mut warnings = extract_warnings;
        warnings.push(WebFetchResponse::untrusted_warning());

        Ok(WebFetchResponse {
            url: url_str.to_string(),
            final_url,
            title,
            description,
            content_type,
            status,
            fetched: true,
            truncated,
            trust: FetchTrust::ExternalUntrusted,
            text,
            links,
            warnings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::fetch::ExtractMode;
    use httpmock::prelude::*;
    use std::time::Duration;

    fn test_limits() -> FetchLimits {
        FetchLimits {
            max_url_len: 8192,
            max_bytes: 2_000_000,
            max_chars_default: 12_000,
            max_chars_cap: 50_000,
            timeout_ms: 5_000,
            redirect_limit: 5,
            allow_private_network: true,
            allow_localhost: true,
        }
    }

    fn test_client() -> FetchClient {
        FetchClient::new(test_limits(), "eggsearch/test".to_string()).expect("client builds")
    }

    #[tokio::test]
    async fn fetch_200_text_html_happy_path() {
        let server = MockServer::start();
        let body = b"<!DOCTYPE html><html><head><title>Hi</title></head><body><p>hello world</p></body></html>";
        let mock = server.mock(|when, then| {
            when.method(GET).path("/page");
            then.status(200)
                .header("content-type", "text/html; charset=utf-8")
                .body(body);
        });

        let client = test_client();
        let resp = client
            .fetch(&server.url("/page"), None, ExtractMode::Text, false)
            .await
            .expect("ok");

        assert_eq!(resp.status, 200);
        assert!(resp.fetched);
        assert!(!resp.truncated);
        assert_eq!(resp.title.as_deref(), Some("Hi"));
        assert!(resp.text.as_deref().unwrap_or("").contains("hello world"));
        mock.assert();
    }

    #[tokio::test]
    async fn fetch_200_text_plain_happy_path() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/note");
            then.status(200)
                .header("content-type", "text/plain")
                .body("just plain text here\n");
        });

        let client = test_client();
        let resp = client
            .fetch(&server.url("/note"), None, ExtractMode::Text, false)
            .await
            .expect("ok");

        assert_eq!(resp.status, 200);
        assert!(resp.fetched);
        assert!(resp.text.as_deref().unwrap_or("").contains("just plain text"));
    }

    #[tokio::test]
    async fn fetch_301_redirect_within_limit() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/start");
            then.status(301).header("location", "/end");
        });
        server.mock(|when, then| {
            when.method(GET).path("/end");
            then.status(200)
                .header("content-type", "text/plain")
                .body("redirected");
        });

        let client = test_client();
        let resp = client
            .fetch(&server.url("/start"), None, ExtractMode::Text, false)
            .await
            .expect("ok");
        assert_eq!(resp.status, 200);
        assert!(resp.text.as_deref().unwrap_or("").contains("redirected"));
        assert_ne!(
            resp.url, resp.final_url,
            "final_url should differ from url after redirect"
        );
    }

    #[tokio::test]
    async fn fetch_redirect_loop_exceeds_limit() {
        let server = MockServer::start();
        // Build a chain of 10 redirects; the client is configured with redirect_limit = 5.
        for i in 0..10 {
            let next = format!("/r/{}", i + 1);
            server.mock(|when, then| {
                let path = format!("/r/{}", i);
                when.method(GET).path(path);
                then.status(302).header("location", next);
            });
        }

        let client = test_client();
        let result = client
            .fetch(&server.url("/r/0"), None, ExtractMode::Text, false)
            .await;
        assert!(
            result.is_err(),
            "expected redirect loop error, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn fetch_404_returns_http_status_error() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/missing");
            then.status(404);
        });

        let client = test_client();
        let err = client
            .fetch(&server.url("/missing"), None, ExtractMode::Text, false)
            .await
            .expect_err("expected error");
        assert!(
            matches!(err.kind(), crate::fetch::FetchErrorKind::HttpStatus),
            "got: {err:?}"
        );
    }

    #[tokio::test]
    async fn fetch_content_length_above_max_bytes_errors() {
        let server = MockServer::start();
        let big = vec![b'x'; 5_000];
        server.mock(|when, then| {
            when.method(GET).path("/big");
            then.status(200)
                .header("content-type", "text/plain")
                .header("content-length", &big.len().to_string())
                .body(&big);
        });

        let mut limits = test_limits();
        limits.max_bytes = 1_000; // smaller than the body
        let client = FetchClient::new(limits, "eggsearch/test".to_string()).expect("client");
        let result = client
            .fetch(&server.url("/big"), None, ExtractMode::Text, false)
            .await;

        // The implementation streams chunks; an oversize body should either
        // produce a ContentTooLarge error or come back with truncated=true
        // (the body is truncated to max_bytes rather than errored out).
        // We accept either behavior; what we must NOT see is a successful
        // untruncated fetch of the full body.
        match result {
            Err(e) => assert!(
                matches!(
                    e.kind(),
                    crate::fetch::FetchErrorKind::ContentTooLarge
                        | crate::fetch::FetchErrorKind::NetworkError
                ),
                "unexpected error: {e:?}"
            ),
            Ok(resp) => {
                assert!(resp.truncated, "expected truncated=true, got: {resp:?}");
                let len = resp.text.as_deref().unwrap_or("").len();
                assert!(len <= 1_000, "got text len {len} > max_bytes 1000");
            }
        }
    }

    #[tokio::test]
    async fn fetch_content_length_precheck_short_circuits() {
        // content-length > max_bytes should produce ContentTooLarge
        // *without* reading the body. Body length must match the
        // declared content-length, otherwise hyper rejects the
        // response at the protocol level.
        let server = MockServer::start();
        let body = vec![b'x'; 5_000];
        server.mock(|when, then| {
            when.method(GET).path("/declared-huge");
            then.status(200)
                .header("content-type", "text/plain")
                .header("content-length", &body.len().to_string())
                .body(&body);
        });

        let mut limits = test_limits();
        limits.max_bytes = 1_000;
        let client = FetchClient::new(limits, "eggsearch/test".to_string()).expect("client");
        let result = client
            .fetch(
                &server.url("/declared-huge"),
                None,
                ExtractMode::Text,
                false,
            )
            .await;
        let err = result.expect_err("expected content-too-large error from pre-check");
        assert!(
            matches!(err.kind(), crate::fetch::FetchErrorKind::ContentTooLarge),
            "got: {err:?}"
        );
    }

    #[tokio::test]
    async fn fetch_unsupported_pdf_errors() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/doc.pdf");
            then.status(200)
                .header("content-type", "application/pdf")
                .body("%PDF-1.4 fake");
        });

        let client = test_client();
        let err = client
            .fetch(&server.url("/doc.pdf"), None, ExtractMode::Text, false)
            .await
            .expect_err("expected unsupported content type error");
        assert!(
            matches!(err.kind(), crate::fetch::FetchErrorKind::UnsupportedContentType),
            "got: {err:?}"
        );
    }

    #[tokio::test]
    async fn fetch_slow_response_times_out() {
        // Use a server that delays 3 seconds; the client is configured with
        // timeout_ms = 500. We expect a Timeout error.
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/slow");
            then.status(200)
                .header("content-type", "text/plain")
                .delay(Duration::from_secs(3))
                .body("too late");
        });

        let mut limits = test_limits();
        limits.timeout_ms = 500;
        let client = FetchClient::new(limits, "eggsearch/test".to_string()).expect("client");
        let result = client
            .fetch(&server.url("/slow"), None, ExtractMode::Text, false)
            .await;
        let err = result.expect_err("expected timeout");
        assert!(
            matches!(err.kind(), crate::fetch::FetchErrorKind::Timeout),
            "got: {err:?}"
        );
    }
}
