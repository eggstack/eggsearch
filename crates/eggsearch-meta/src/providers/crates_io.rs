//! crates.io search provider (no key required).

use async_trait::async_trait;
use eggsearch_core::{
    error::CoreResult,
    provider::{SearchContext, SearchProvider, SearchProviderResponse},
    query::{SearchCategory, SearchQuery},
    result::{SearchResult, SearchWarning, SourceKind, TrustLevel},
};
use reqwest::Client;
use serde::Deserialize;
use std::time::Instant;
use url::Url;

pub const CRATES_IO_API: &str = "https://crates.io/api/v1/crates";

#[derive(Clone, Debug)]
pub struct CratesIoProvider {
    client: Client,
}

impl Default for CratesIoProvider {
    fn default() -> Self {
        Self {
            client: Client::builder()
                .user_agent(format!("eggsearch/{}", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("reqwest client"),
        }
    }
}

impl CratesIoProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_client(client: Client) -> Self {
        Self { client }
    }

    pub fn parse_response(
        &self,
        json: &serde_json::Value,
        max: usize,
    ) -> (Vec<SearchResult>, Vec<SearchWarning>) {
        let mut results = Vec::new();
        let mut warnings = Vec::new();
        let crates = json.get("crates").and_then(|c| c.as_array());
        let Some(crates) = crates else {
            warnings.push(SearchWarning {
                provider_id: "crates_io".to_string(),
                message: "missing crates in response".to_string(),
            });
            return (results, warnings);
        };
        for (i, c) in crates.iter().enumerate() {
            if i >= max {
                break;
            }
            let name = c.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if name.is_empty() {
                continue;
            }
            let description = c
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            let version = c
                .get("max_stable_version")
                .and_then(|v| v.as_str())
                .or_else(|| c.get("max_version").and_then(|v| v.as_str()))
                .map(String::from);
            let title = match &version {
                Some(v) => format!("{name} {v}"),
                None => name.to_string(),
            };
            let url = Url::parse(&format!("https://crates.io/crates/{name}")).unwrap();
            let snippet = match (&description, &version) {
                (Some(d), Some(v)) => Some(format!("{d} (latest: {v})")),
                (Some(d), None) => Some(d.clone()),
                (None, Some(v)) => Some(format!("latest: {v}")),
                (None, None) => None,
            };
            results.push(SearchResult {
                title,
                url,
                snippet,
                published_at: c
                    .get("updated_at")
                    .and_then(|v| v.as_str())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|d| d.with_timezone(&chrono::Utc)),
                rank: i,
                score: None,
                provider_id: "crates_io".to_string(),
                source_kind: SourceKind::PackageRegistry,
                trust_level: TrustLevel::ExternalUntrusted,
            });
        }
        (results, warnings)
    }
}

#[derive(Deserialize)]
struct _Unused {
    #[allow(dead_code)]
    placeholder: Option<String>,
}

#[async_trait]
impl SearchProvider for CratesIoProvider {
    fn id(&self) -> &'static str {
        "crates_io"
    }

    fn categories(&self) -> &[SearchCategory] {
        &[SearchCategory::PackageRegistry, SearchCategory::Documentation]
    }

    async fn search(
        &self,
        query: SearchQuery,
        ctx: SearchContext,
    ) -> CoreResult<SearchProviderResponse> {
        let started = Instant::now();
        let mut resp = SearchProviderResponse::empty(self.id(), query.clone());
        let limit = query.max_results.clamp(1, 100).to_string();

        let body = match self
            .client
            .get(CRATES_IO_API)
            .header("User-Agent", ctx.user_agent.clone())
            .query(&[("q", query.query.as_str()), ("per_page", limit.as_str())])
            .timeout(ctx.timeout)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                resp.warnings.push(SearchWarning {
                    provider_id: self.id().to_string(),
                    message: format!("request failed: {e}"),
                });
                resp.elapsed_ms = started.elapsed().as_millis() as u64;
                return Ok(resp);
            }
        };

        if !body.status().is_success() {
            resp.warnings.push(SearchWarning {
                provider_id: self.id().to_string(),
                message: format!("upstream status {}", body.status()),
            });
            resp.elapsed_ms = started.elapsed().as_millis() as u64;
            return Ok(resp);
        }

        let json: serde_json::Value = match body.json().await {
            Ok(j) => j,
            Err(e) => {
                resp.warnings.push(SearchWarning {
                    provider_id: self.id().to_string(),
                    message: format!("json parse failed: {e}"),
                });
                resp.elapsed_ms = started.elapsed().as_millis() as u64;
                return Ok(resp);
            }
        };

        let max = query.max_results.max(1);
        let (mut results, mut warnings) = self.parse_response(&json, max);
        results.truncate(max);
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

    #[test]
    fn parses_response() {
        let p = CratesIoProvider::new();
        let body = json!({
            "crates": [
                {
                    "name": "tokio",
                    "max_stable_version": "1.40.0",
                    "description": "An async runtime",
                    "updated_at": "2024-01-01T00:00:00Z"
                },
                {
                    "name": "axum",
                    "max_stable_version": "0.7.0",
                    "description": "Web framework"
                }
            ]
        });
        let (results, warnings) = p.parse_response(&body, 5);
        assert!(warnings.is_empty());
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "tokio 1.40.0");
        assert_eq!(results[0].source_kind, SourceKind::PackageRegistry);
        assert_eq!(results[1].title, "axum 0.7.0");
        assert_eq!(results[1].url.as_str(), "https://crates.io/crates/axum");
    }

    #[test]
    fn missing_crates_warns() {
        let p = CratesIoProvider::new();
        let (results, warnings) = p.parse_response(&json!({}), 5);
        assert!(results.is_empty());
        assert!(!warnings.is_empty());
    }
}
