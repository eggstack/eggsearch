//! HTTP client for fetching URLs.

use std::time::Duration;

use futures::StreamExt;
use reqwest::Client;

use super::extract::extract_content;
use super::limits::{validate_url, FetchLimits};
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

        let (title, description, text, links) = if extract_mode == ExtractMode::MetadataOnly {
            if is_html {
                let extractor = super::extract::HtmlExtractor::new(&body, &final_url);
                let (t, d, _, l) = extractor.extract(max_chars, include_links);
                (t, d, None, l)
            } else {
                (None, None, None, Vec::new())
            }
        } else if is_html {
            let (t, d, txt, l) = extract_content(&body, &final_url, max_chars, include_links);
            (t, d, Some(txt), l)
        } else {
            let text = String::from_utf8_lossy(&body)
                .chars()
                .take(max_chars)
                .collect::<String>();
            (None, None, Some(text), Vec::new())
        };

        let warnings = vec![WebFetchResponse::untrusted_warning()];

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
