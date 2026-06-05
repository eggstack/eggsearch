//! SearXNG metasearch provider.
//!
//! Speaks SearXNG's documented JSON output format
//! (`GET /search?q=...&format=json&categories=general`).
//!
//! Configured via `[search.providers.searxng]`:
//!
//! ```toml
//! [search.providers.searxng]
//! enabled = true
//! base_url = "http://127.0.0.1:8080"
//! ```
//!
//! `base_url` is required. If the upstream returns HTML instead of JSON
//! (which is what happens when SearXNG's `search.formats` does not include
//! `json`), the provider emits a clear warning and returns no results.

use async_trait::async_trait;
use eggsearch_core::{
    config::ProviderConfig,
    error::CoreResult,
    provider::{SearchContext, SearchProvider, SearchProviderResponse},
    query::{SearchCategory, SearchQuery},
    result::{SearchResult, SearchWarning, SourceKind, TrustLevel},
};
use reqwest::Client;
use std::time::Instant;
use url::Url;

pub const SEARXNG_DEFAULT_CATEGORIES: &str = "general";

#[derive(Clone, Debug)]
pub struct SearxngProvider {
    client: Client,
    base_url: String,
    categories: String,
}

impl SearxngProvider {
    /// Build a SearXNG provider. Returns an error if `base_url` is empty
    /// or not a valid URL.
    pub fn new(base_url: impl Into<String>) -> CoreResult<Self> {
        let base_url = base_url.into();
        if base_url.trim().is_empty() {
            return Err(eggsearch_core::error::CoreError::Config(
                "searxng base_url is required".to_string(),
            ));
        }
        let parsed = Url::parse(&base_url).map_err(|e| {
            eggsearch_core::error::CoreError::Config(format!("searxng base_url invalid: {e}"))
        })?;
        Ok(Self {
            client: Client::builder()
                .user_agent(format!("eggsearch/{}", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("reqwest client"),
            base_url: parsed.as_str().trim_end_matches('/').to_string(),
            categories: SEARXNG_DEFAULT_CATEGORIES.to_string(),
        })
    }

    pub fn with_categories(mut self, categories: impl Into<String>) -> Self {
        self.categories = categories.into();
        self
    }

    pub fn with_client(mut self, client: Client) -> Self {
        self.client = client;
        self
    }

    /// Build from a `[search.providers.searxng]` config block.
    pub fn from_config(cfg: &ProviderConfig) -> CoreResult<Self> {
        let base_url = cfg
            .base_url
            .clone()
            .or_else(|| {
                cfg.extra
                    .get("base_url")
                    .cloned()
            })
            .ok_or_else(|| {
                eggsearch_core::error::CoreError::Config(
                    "searxng provider enabled but no base_url configured".to_string(),
                )
            })?;
        let mut p = Self::new(base_url)?;
        if let Some(cat) = cfg.extra.get("categories") {
            p = p.with_categories(cat.clone());
        }
        if let Some(cat) = cfg.extra.get("category") {
            p = p.with_categories(cat.clone());
        }
        Ok(p)
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Parse a SearXNG JSON response.
    pub fn parse_json(
        &self,
        json: &serde_json::Value,
    ) -> (Vec<SearchResult>, Vec<SearchWarning>) {
        let mut results = Vec::new();
        let mut warnings = Vec::new();
        let arr = match json.get("results").and_then(|v| v.as_array()) {
            Some(a) => a,
            None => {
                warnings.push(SearchWarning {
                    provider_id: "searxng".to_string(),
                    message: "missing 'results' array in SearXNG response".to_string(),
                });
                return (results, warnings);
            }
        };
        for (i, r) in arr.iter().enumerate() {
            let url = match r.get("url").and_then(|v| v.as_str()) {
                Some(u) => u.to_string(),
                None => continue,
            };
            let url = match Url::parse(&url) {
                Ok(u) => eggsearch_core::normalize::canonicalize(u.as_str()).unwrap_or(u),
                Err(_) => continue,
            };
            let title = r
                .get("title")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .unwrap_or("")
                .to_string();
            if title.is_empty() {
                continue;
            }
            let snippet = r
                .get("content")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from);
            let published_at = r
                .get("publishedDate")
                .and_then(|v| v.as_str())
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.with_timezone(&chrono::Utc));
            let category = r
                .get("category")
                .and_then(|v| v.as_str())
                .unwrap_or("general");
            let source_kind = match category {
                "general" => SourceKind::Web,
                "news" => SourceKind::News,
                "science" | "it" | "files" | "images" | "videos" | "map" | "music"
                | "social media" => SourceKind::Web,
                // Default to web for any other / empty category. Future
                // work can map additional SearXNG categories to more
                // specific `SourceKind` variants.
                _ => SourceKind::Web,
            };
            results.push(SearchResult {
                title,
                url,
                snippet,
                published_at,
                rank: i,
                score: r.get("score").and_then(|v| v.as_f64()).map(|f| f as f32),
                provider_id: "searxng".to_string(),
                source_kind,
                trust_level: TrustLevel::ExternalUntrusted,
            });
        }
        (results, warnings)
    }
}

#[async_trait]
impl SearchProvider for SearxngProvider {
    fn id(&self) -> &'static str {
        "searxng"
    }

    fn categories(&self) -> &[SearchCategory] {
        &[SearchCategory::General, SearchCategory::News]
    }

    async fn search(
        &self,
        query: SearchQuery,
        ctx: SearchContext,
    ) -> CoreResult<SearchProviderResponse> {
        let started = Instant::now();
        let mut resp = SearchProviderResponse::empty(self.id(), query.clone());
        let limit = query.max_results.clamp(1, 50).to_string();
        let url = format!("{}/search", self.base_url);

        let body = match self
            .client
            .get(&url)
            .header("User-Agent", ctx.user_agent.clone())
            .header("Accept", "application/json")
            .query(&[
                ("q", query.query.as_str()),
                ("format", "json"),
                ("categories", self.categories.as_str()),
                ("count", limit.as_str()),
            ])
            .timeout(ctx.timeout)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                resp.warnings.push(SearchWarning {
                    provider_id: self.id().to_string(),
                    message: format!(
                        "request to SearXNG at {} failed: {e}",
                        self.base_url
                    ),
                });
                resp.elapsed_ms = started.elapsed().as_millis() as u64;
                return Ok(resp);
            }
        };

        if !body.status().is_success() {
            let s = body.status();
            // Try to peek at the body to give a useful error.
            let body_text = body.text().await.unwrap_or_default();
            let hint = if body_text.contains("search.formats")
                || body_text.contains("JSON")
                || body_text.contains("application/json")
            {
                " (SearXNG JSON output may be disabled: set `search.formats: [\"json\"]` in settings.yml)"
            } else {
                ""
            };
            resp.warnings.push(SearchWarning {
                provider_id: self.id().to_string(),
                message: format!("upstream returned status {s}{hint}"),
            });
            resp.elapsed_ms = started.elapsed().as_millis() as u64;
            return Ok(resp);
        }

        // Detect non-JSON response (SearXNG with JSON disabled).
        let content_type = body
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();
        let text = match body.text().await {
            Ok(t) => t,
            Err(e) => {
                resp.warnings.push(SearchWarning {
                    provider_id: self.id().to_string(),
                    message: format!("failed reading body: {e}"),
                });
                resp.elapsed_ms = started.elapsed().as_millis() as u64;
                return Ok(resp);
            }
        };
        if !content_type.contains("json") {
            let looks_like_html = text.trim_start().starts_with('<');
            if looks_like_html {
                resp.warnings.push(SearchWarning {
                    provider_id: self.id().to_string(),
                    message: format!(
                        "SearXNG returned HTML (Content-Type: '{content_type}') instead of JSON. \
                         JSON output is disabled on the upstream. Enable it by setting \
                         `search.formats: [\"json\"]` in SearXNG settings.yml."
                    ),
                });
            } else {
                resp.warnings.push(SearchWarning {
                    provider_id: self.id().to_string(),
                    message: format!(
                        "SearXNG returned non-JSON Content-Type: '{content_type}'"
                    ),
                });
            }
            resp.elapsed_ms = started.elapsed().as_millis() as u64;
            return Ok(resp);
        }

        let json: serde_json::Value = match serde_json::from_str(&text) {
            Ok(j) => j,
            Err(e) => {
                let preview = &text.chars().take(200).collect::<String>();
                resp.warnings.push(SearchWarning {
                    provider_id: self.id().to_string(),
                    message: format!(
                        "SearXNG JSON parse failed: {e} (body starts with: {preview:?})"
                    ),
                });
                resp.elapsed_ms = started.elapsed().as_millis() as u64;
                return Ok(resp);
            }
        };

        let (mut results, mut warnings) = self.parse_json(&json);
        if results.is_empty() && warnings.is_empty() {
            warnings.push(SearchWarning {
                provider_id: self.id().to_string(),
                message: "SearXNG returned an empty result set".to_string(),
            });
        }
        results.truncate(query.max_results.max(1));
        resp.results = results;
        resp.warnings.append(&mut warnings);
        resp.elapsed_ms = started.elapsed().as_millis() as u64;
        Ok(resp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const FIXTURE_BASIC: &str = include_str!("../../tests/fixtures/searxng/basic.json");
    const FIXTURE_EMPTY: &str = include_str!("../../tests/fixtures/searxng/empty.json");
    const FIXTURE_HTML: &str = include_str!("../../tests/fixtures/searxng/json_disabled.html");

    #[test]
    fn requires_base_url() {
        let r = SearxngProvider::new("");
        assert!(r.is_err());
    }

    #[test]
    fn rejects_invalid_base_url() {
        let r = SearxngProvider::new("not a url");
        assert!(r.is_err());
    }

    #[test]
    fn trims_trailing_slash() {
        let p = SearxngProvider::new("http://127.0.0.1:8080/").unwrap();
        assert_eq!(p.base_url(), "http://127.0.0.1:8080");
    }

    #[test]
    fn parses_basic_fixture() {
        let p = SearxngProvider::new("http://127.0.0.1:8080").unwrap();
        let v: serde_json::Value = serde_json::from_str(FIXTURE_BASIC).unwrap();
        let (results, warnings) = p.parse_json(&v);
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].provider_id, "searxng");
        assert_eq!(results[0].url.as_str(), "https://tokio.rs/");
        assert!(results[0].snippet.is_some());
        assert_eq!(results[0].source_kind, SourceKind::Web);
        assert_eq!(results[0].trust_level, TrustLevel::ExternalUntrusted);
    }

    #[test]
    fn parses_empty_fixture() {
        let p = SearxngProvider::new("http://127.0.0.1:8080").unwrap();
        let v: serde_json::Value = serde_json::from_str(FIXTURE_EMPTY).unwrap();
        let (results, warnings) = p.parse_json(&v);
        assert!(results.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn missing_results_warns() {
        let p = SearxngProvider::new("http://127.0.0.1:8080").unwrap();
        let (results, warnings) = p.parse_json(&json!({}));
        assert!(results.is_empty());
        assert!(!warnings.is_empty());
    }

    #[test]
    fn from_config_reads_base_url() {
        let cfg = ProviderConfig {
            enabled: true,
            base_url: Some("http://localhost:8080/".to_string()),
            ..Default::default()
        };
        let p = SearxngProvider::from_config(&cfg).unwrap();
        assert_eq!(p.base_url(), "http://localhost:8080");
    }

    #[test]
    fn from_config_missing_base_url_errors() {
        let cfg = ProviderConfig {
            enabled: true,
            base_url: None,
            ..Default::default()
        };
        let r = SearxngProvider::from_config(&cfg);
        assert!(r.is_err());
    }

    #[test]
    fn from_config_reads_extra_base_url() {
        let mut cfg = ProviderConfig::default();
        cfg.enabled = true;
        cfg.extra.insert("base_url".to_string(), "http://extra:9000".to_string());
        let p = SearxngProvider::from_config(&cfg).unwrap();
        assert_eq!(p.base_url(), "http://extra:9000");
    }

    #[test]
    fn json_disabled_html_recognized() {
        // Sanity: the fixture is what we'd see when JSON is disabled.
        assert!(FIXTURE_HTML.trim_start().starts_with('<'));
    }
}
