//! Reqwest-based implementation of `FetchProvider` with timeout, byte limits,
//! optional robots policy, content-type aware extraction, and an artifact
//! store write-through.

use async_trait::async_trait;
use chrono::Utc;
use eggsearch_core::{
    normalize::canonicalize,
    result::TrustLevel,
};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use reqwest::{Client, Response, StatusCode};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tracing::warn;
use url::Url;

use crate::artifact::ArtifactStore;
use crate::cache::FetchCache;
use crate::error::{FetchError, FetchResult};
use crate::extract::{extract_html, extract_text, ExtractMode};
use crate::fetch::{FetchProvider, FetchRequest, FetchedDocument};
use crate::robots::RobotsCache;

#[derive(Clone)]
pub struct ReqwestFetchProvider {
    client: Client,
    cache: Arc<FetchCache>,
    artifacts: Arc<ArtifactStore>,
    robots: Arc<RobotsCache>,
    user_agent: String,
}

impl std::fmt::Debug for ReqwestFetchProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReqwestFetchProvider")
            .field("user_agent", &self.user_agent)
            .finish()
    }
}

impl ReqwestFetchProvider {
    pub fn new(artifacts: Arc<ArtifactStore>, cache: Arc<FetchCache>, robots: Arc<RobotsCache>) -> FetchResult<Self> {
        let user_agent = format!("eggsearch/{}", env!("CARGO_PKG_VERSION"));
        let client = Client::builder()
            .user_agent(&user_agent)
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| FetchError::Other(e.to_string()))?;
        Ok(Self {
            client,
            cache,
            artifacts,
            robots,
            user_agent,
        })
    }

    pub fn with_user_agent(mut self, ua: impl Into<String>) -> Self {
        let ua = ua.into();
        self.user_agent = ua.clone();
        // Rebuild client with the new UA.
        if let Ok(client) = Client::builder()
            .user_agent(&ua)
            .timeout(std::time::Duration::from_secs(15))
            .build()
        {
            self.client = client;
        }
        self
    }

    async fn send(&self, request: &FetchRequest) -> FetchResult<Response> {
        if request.respect_robots_txt {
            if !self.robots.allowed(&request.url).await {
                return Err(FetchError::RobotsDenied(request.url.to_string()));
            }
        }
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_str(&self.user_agent).unwrap());
        headers.insert(ACCEPT, HeaderValue::from_static("text/html,application/xhtml+xml,text/plain,application/json"));
        let timeout = std::time::Duration::from_millis(request.timeout_ms.max(100));

        let resp = tokio::time::timeout(
            timeout,
            self.client
                .get(request.url.as_str())
                .headers(headers)
                .send(),
        )
        .await
        .map_err(|_| FetchError::Timeout(request.timeout_ms))?
        .map_err(|e| FetchError::Network(e.to_string()))?;

        Ok(resp)
    }

    async fn read_capped(&self, mut resp: Response, max_bytes: usize) -> FetchResult<(Vec<u8>, String)> {
        let status = resp.status();
        if !status.is_success() {
            return Err(FetchError::BadStatus(status.as_u16()));
        }
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();

        // Check Content-Length header up-front if present.
        if let Some(cl) = resp
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<usize>().ok())
        {
            if cl > max_bytes {
                return Err(FetchError::TooLarge { limit: max_bytes });
            }
        }

        let mut buf: Vec<u8> = Vec::new();
        let mut total = 0usize;
        while let Some(chunk) = resp.chunk().await.map_err(|e| FetchError::Network(e.to_string()))? {
            total += chunk.len();
            if total > max_bytes {
                return Err(FetchError::TooLarge { limit: max_bytes });
            }
            buf.extend_from_slice(&chunk);
        }
        Ok((buf, content_type))
    }
}

#[async_trait]
impl FetchProvider for ReqwestFetchProvider {
    async fn fetch(&self, request: FetchRequest) -> FetchResult<FetchedDocument> {
        let canonical = canonicalize(request.url.as_str()).unwrap_or(request.url.clone());
        let cache_key = format!("{}|{}|{:?}", canonical, request.max_bytes, request.extract_mode);

        if let Some(mut cached) = self.cache.get(&cache_key).await {
            cached.from_cache = true;
            return Ok(cached);
        }

        let mut warnings: Vec<String> = Vec::new();
        let started = Utc::now();
        let resp = match self.send(&request).await {
            Ok(r) => r,
            Err(FetchError::BadStatus(s)) => {
                return Err(FetchError::BadStatus(s));
            }
            Err(e) => return Err(e),
        };

        let status = resp.status();
        if status == StatusCode::NOT_FOUND {
            return Err(FetchError::BadStatus(404));
        }
        if status.is_redirection() {
            warnings.push(format!("upstream redirected to {}", resp.headers().get(reqwest::header::LOCATION).and_then(|v| v.to_str().ok()).unwrap_or("?")));
        }
        let (body, content_type) = self.read_capped(resp, request.max_bytes).await?;

        let body_text = String::from_utf8_lossy(&body).to_string();
        let extractor_input = body_text.clone();

        // Try to extract title from HTML
        let mut title: Option<String> = None;
        if content_type.contains("html") || extractor_input.trim_start().starts_with("<") {
            if let Some(t) = crate::html::extract_title(&extractor_input) {
                title = Some(t);
            }
        }

        let text = match request.extract_mode {
            ExtractMode::Raw => body_text,
            ExtractMode::Text => extract_text(&extractor_input),
            ExtractMode::Readability => {
                if content_type.contains("html") || extractor_input.trim_start().starts_with("<") {
                    extract_html(&extractor_input)
                } else {
                    extract_text(&extractor_input)
                }
            }
            ExtractMode::Markdown => {
                if content_type.contains("html") || extractor_input.trim_start().starts_with("<") {
                    crate::markdown::html_to_markdown(&extractor_input)
                } else {
                    extract_text(&extractor_input)
                }
            }
        };

        let excerpt = make_excerpt(&text, 1200);
        let content_hash = hex::encode(Sha256::digest(body.as_slice()));

        let artifact_id = match self
            .artifacts
            .put(
                &content_hash,
                &serde_json::json!({
                    "url": request.url.to_string(),
                    "canonical_url": canonical.to_string(),
                    "title": title,
                    "content_type": content_type,
                    "fetched_at": started,
                    "trust_level": TrustLevel::ExternalUntrusted,
                    "text": text,
                    "raw_length": body.len(),
                }),
            )
            .await
        {
            Ok(id) => id,
            Err(e) => {
                warn!("artifact write failed: {e}");
                warnings.push(format!("artifact write failed: {e}"));
                content_hash.clone()
            }
        };

        let doc = FetchedDocument {
            url: request.url.to_string(),
            canonical_url: canonical.to_string(),
            title,
            text: text.clone(),
            excerpt,
            content_type,
            content_hash: content_hash.clone(),
            artifact_id,
            fetched_at: started,
            status: status.as_u16(),
            trust_level: TrustLevel::ExternalUntrusted,
            warnings,
            from_cache: false,
        };

        self.cache.put(cache_key, doc.clone()).await;
        Ok(doc)
    }
}

fn make_excerpt(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let mut out = String::new();
    for (i, c) in trimmed.chars().enumerate() {
        if i >= max_chars {
            break;
        }
        out.push(c);
    }
    out.push('…');
    out
}

#[allow(dead_code)]
fn _silence_url_unused(_: &Url) {}
