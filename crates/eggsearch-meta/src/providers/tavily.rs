//! Tavily Search API provider.
//!
//! Uses `https://api.tavily.com/search` via POST with a JSON body. Requires
//! an API key. The API key is sent in the request body (not a header).
//!
//! Configured via `[search.providers.tavily]`:
//!
//! ```toml
//! [search.providers.tavily]
//! enabled = true
//! api_key_env = "TAVILY_API_KEY"
//! ```
//!
//! If `api_key_env` is unset or empty, the provider fails with a clear
//! diagnostic naming the env var.

use async_trait::async_trait;
use eggsearch_core::{
    config::ProviderConfig,
    error::{CoreError, CoreResult},
    provider::{SearchContext, SearchProvider, SearchProviderResponse},
    query::{SearchCategory, SearchQuery},
    result::{SearchResult, SearchWarning, SourceKind, TrustLevel},
};
use reqwest::Client;
use serde_json::json;
use std::time::Instant;
use url::Url;

use chrono::TimeZone;

pub const TAVILY_API_URL: &str = "https://api.tavily.com/search";
pub const TAVILY_DEFAULT_KEY_ENV: &str = "TAVILY_API_KEY";

#[derive(Clone, Debug)]
pub struct TavilyProvider {
    client: Client,
    api_key: String,
    api_key_env: String,
}

impl TavilyProvider {
    pub fn with_api_key(api_key: impl Into<String>) -> CoreResult<Self> {
        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            return Err(CoreError::Config(
                "tavily api_key is empty".to_string(),
            ));
        }
        Ok(Self {
            client: Client::builder()
                .user_agent(format!("eggsearch/{}", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("reqwest client"),
            api_key,
            api_key_env: "<inline>".to_string(),
        })
    }

    pub fn from_env(var: impl Into<String>) -> CoreResult<Self> {
        let var = var.into();
        if var.trim().is_empty() {
            return Err(CoreError::Config(
                "tavily api_key_env is empty".to_string(),
            ));
        }
        let key = std::env::var(&var).map_err(|_| {
            CoreError::Config(format!(
                "tavily provider enabled but environment variable '{var}' is not set or unreadable"
            ))
        })?;
        if key.trim().is_empty() {
            return Err(CoreError::Config(format!(
                "tavily provider enabled but environment variable '{var}' is empty"
            )));
        }
        Ok(Self {
            client: Client::builder()
                .user_agent(format!("eggsearch/{}", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("reqwest client"),
            api_key: key,
            api_key_env: var,
        })
    }

    pub fn from_config(cfg: &ProviderConfig) -> CoreResult<Self> {
        if !cfg.enabled {
            return Err(CoreError::Config(
                "tavily provider not enabled in config".to_string(),
            ));
        }
        let var = cfg
            .api_key_env
            .clone()
            .unwrap_or_else(|| TAVILY_DEFAULT_KEY_ENV.to_string());
        Self::from_env(var)
    }

    pub fn api_key_env(&self) -> &str {
        &self.api_key_env
    }

    pub fn masked_key(&self) -> String {
        if self.api_key.len() <= 4 {
            return "****".to_string();
        }
        let tail = &self.api_key[self.api_key.len() - 4..];
        format!("***{tail}")
    }

    /// Parse a Tavily search response.
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
                    provider_id: "tavily".to_string(),
                    message: "missing 'results' array in Tavily response".to_string(),
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
                .get("published_date")
                .and_then(|v| v.as_str())
                .and_then(parse_tavily_date);
            results.push(SearchResult {
                title,
                url,
                snippet,
                published_at,
                rank: i,
                score: r.get("score").and_then(|v| v.as_f64()).map(|f| f as f32),
                provider_id: "tavily".to_string(),
                source_kind: SourceKind::Web,
                trust_level: TrustLevel::ExternalUntrusted,
            });
        }
        (results, warnings)
    }
}

fn parse_tavily_date(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    // Tavily returns YYYY-MM-DD for published_date. If it returns an RFC3339
    // timestamp, that is also accepted.
    if let Ok(d) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(d.with_timezone(&chrono::Utc));
    }
    if let Ok(d) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        if let Some(dt) = d.and_hms_opt(0, 0, 0) {
            return Some(chrono::Utc.from_utc_datetime(&dt));
        }
    }
    None
}

#[async_trait]
impl SearchProvider for TavilyProvider {
    fn id(&self) -> &'static str {
        "tavily"
    }

    fn categories(&self) -> &[SearchCategory] {
        &[SearchCategory::General, SearchCategory::Reference, SearchCategory::News]
    }

    async fn search(
        &self,
        query: SearchQuery,
        ctx: SearchContext,
    ) -> CoreResult<SearchProviderResponse> {
        let started = Instant::now();
        let mut resp = SearchProviderResponse::empty(self.id(), query.clone());
        let max_results = query.max_results.clamp(1, 20);

        let body_json = json!({
            "api_key": self.api_key,
            "query": query.query,
            "max_results": max_results,
            "include_raw_content": false,
        });

        let body = match self
            .client
            .post(TAVILY_API_URL)
            .header("User-Agent", ctx.user_agent.clone())
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .timeout(ctx.timeout)
            .json(&body_json)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                resp.warnings.push(SearchWarning {
                    provider_id: self.id().to_string(),
                    message: format!("Tavily request failed: {e}"),
                });
                resp.elapsed_ms = started.elapsed().as_millis() as u64;
                return Ok(resp);
            }
        };

        let status = body.status();
        if !status.is_success() {
            let body_text = body.text().await.unwrap_or_default();
            let hint = match status.as_u16() {
                401 | 403 => format!(
                    " (check the {env} env var; masked={masked})",
                    env = self.api_key_env,
                    masked = self.masked_key()
                ),
                432 | 429 => " (rate limited or quota exceeded)".to_string(),
                _ => String::new(),
            };
            let preview = body_text.chars().take(200).collect::<String>();
            resp.warnings.push(SearchWarning {
                provider_id: self.id().to_string(),
                message: format!("Tavily returned status {status}{hint}; body preview: {preview:?}"),
            });
            resp.elapsed_ms = started.elapsed().as_millis() as u64;
            return Ok(resp);
        }

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

        let json: serde_json::Value = match serde_json::from_str(&text) {
            Ok(j) => j,
            Err(e) => {
                let preview = text.chars().take(200).collect::<String>();
                resp.warnings.push(SearchWarning {
                    provider_id: self.id().to_string(),
                    message: format!("Tavily JSON parse failed: {e} (body preview: {preview:?})"),
                });
                resp.elapsed_ms = started.elapsed().as_millis() as u64;
                return Ok(resp);
            }
        };

        let (mut results, mut warnings) = self.parse_json(&json);
        if results.is_empty() && warnings.is_empty() {
            warnings.push(SearchWarning {
                provider_id: self.id().to_string(),
                message: "Tavily returned no results".to_string(),
            });
        }
        results.truncate(max_results);
        resp.results = results;
        resp.warnings.append(&mut warnings);
        resp.elapsed_ms = started.elapsed().as_millis() as u64;
        Ok(resp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    const FIXTURE_BASIC: &str = include_str!("../../tests/fixtures/tavily/basic.json");
    const FIXTURE_EMPTY: &str = include_str!("../../tests/fixtures/tavily/empty.json");

    #[test]
    fn rejects_empty_key() {
        assert!(TavilyProvider::with_api_key("").is_err());
    }

    #[test]
    fn from_env_errors_when_missing() {
        std::env::remove_var("EGGSEARCH_TEST_TAVILY_MISSING");
        let r = TavilyProvider::from_env("EGGSEARCH_TEST_TAVILY_MISSING");
        assert!(r.is_err());
        let msg = r.unwrap_err().to_string();
        assert!(msg.contains("EGGSEARCH_TEST_TAVILY_MISSING"), "msg: {msg}");
    }

    #[test]
    fn from_env_reads_set_variable() {
        std::env::set_var("EGGSEARCH_TEST_TAVILY_SET", "tvly-abcdef1234");
        let p = TavilyProvider::from_env("EGGSEARCH_TEST_TAVILY_SET").unwrap();
        assert_eq!(p.api_key_env(), "EGGSEARCH_TEST_TAVILY_SET");
        assert_eq!(p.masked_key(), "***1234");
        std::env::remove_var("EGGSEARCH_TEST_TAVILY_SET");
    }

    #[test]
    fn from_config_uses_default_env_var() {
        std::env::set_var("TAVILY_API_KEY", "tvly-zzz9999");
        let cfg = ProviderConfig {
            enabled: true,
            ..Default::default()
        };
        let p = TavilyProvider::from_config(&cfg).unwrap();
        assert_eq!(p.api_key_env(), "TAVILY_API_KEY");
        std::env::remove_var("TAVILY_API_KEY");
    }

    #[test]
    fn from_config_disabled_errors() {
        let cfg = ProviderConfig::default();
        let r = TavilyProvider::from_config(&cfg);
        assert!(r.is_err());
    }

    #[test]
    fn parses_basic_fixture() {
        let p = TavilyProvider::with_api_key("dummy").unwrap();
        let v: serde_json::Value = serde_json::from_str(FIXTURE_BASIC).unwrap();
        let (results, warnings) = p.parse_json(&v);
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].provider_id, "tavily");
        assert_eq!(results[0].url.as_str(), "https://tokio.rs/");
        assert!(results[0].snippet.as_deref().unwrap().contains("asynchronous"));
        assert_eq!(
            results[0].published_at,
            Some(chrono::Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap())
        );
    }

    #[test]
    fn parses_empty_fixture() {
        let p = TavilyProvider::with_api_key("dummy").unwrap();
        let v: serde_json::Value = serde_json::from_str(FIXTURE_EMPTY).unwrap();
        let (results, warnings) = p.parse_json(&v);
        assert!(results.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn missing_results_warns() {
        let p = TavilyProvider::with_api_key("dummy").unwrap();
        let (results, warnings) = p.parse_json(&json!({}));
        assert!(results.is_empty());
        assert!(!warnings.is_empty());
    }

    #[test]
    fn date_parser_handles_rfc3339() {
        let d = parse_tavily_date("2024-05-01T12:00:00Z").unwrap();
        assert_eq!(d.to_rfc3339(), "2024-05-01T12:00:00+00:00");
    }

    #[test]
    fn date_parser_handles_date_only() {
        let d = parse_tavily_date("2024-05-01").unwrap();
        assert_eq!(d.to_rfc3339(), "2024-05-01T00:00:00+00:00");
    }

    #[test]
    fn date_parser_rejects_garbage() {
        assert!(parse_tavily_date("yesterday").is_none());
    }
}
