//! Brave Search API provider.
//!
//! Uses the `https://api.search.brave.com/res/v1/web/search` endpoint with
//! the `X-Subscription-Token` header. Requires an API key.
//!
//! Configured via `[search.providers.brave]`:
//!
//! ```toml
//! [search.providers.brave]
//! enabled = true
//! api_key_env = "BRAVE_SEARCH_API_KEY"
//! ```
//!
//! If `api_key_env` is unset, the provider fails with a clear diagnostic
//! naming the environment variable that must be populated.

use async_trait::async_trait;
use eggsearch_core::{
    config::ProviderConfig,
    error::{CoreError, CoreResult},
    provider::{SearchContext, SearchProvider, SearchProviderResponse},
    query::{SearchCategory, SearchQuery},
    result::{SearchResult, SearchWarning, SourceKind, TrustLevel},
};
use reqwest::Client;
use std::time::Instant;
use url::Url;

pub const BRAVE_API_BASE: &str = "https://api.search.brave.com/res/v1/web/search";
pub const BRAVE_DEFAULT_KEY_ENV: &str = "BRAVE_SEARCH_API_KEY";

#[derive(Clone, Debug)]
pub struct BraveProvider {
    client: Client,
    api_key: String,
    api_key_env: String,
}

impl BraveProvider {
    /// Build a Brave provider with a literal API key (e.g. for tests).
    pub fn with_api_key(api_key: impl Into<String>) -> CoreResult<Self> {
        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            return Err(CoreError::Config(
                "brave api_key is empty".to_string(),
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

    /// Build a Brave provider that reads its key from the named env var.
    pub fn from_env(var: impl Into<String>) -> CoreResult<Self> {
        let var = var.into();
        if var.trim().is_empty() {
            return Err(CoreError::Config(
                "brave api_key_env is empty".to_string(),
            ));
        }
        let key = std::env::var(&var).map_err(|_| {
            CoreError::Config(format!(
                "brave provider enabled but environment variable '{var}' is not set or unreadable"
            ))
        })?;
        if key.trim().is_empty() {
            return Err(CoreError::Config(format!(
                "brave provider enabled but environment variable '{var}' is empty"
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

    /// Build from a `[search.providers.brave]` config block.
    pub fn from_config(cfg: &ProviderConfig) -> CoreResult<Self> {
        if !cfg.enabled {
            return Err(CoreError::Config(
                "brave provider not enabled in config".to_string(),
            ));
        }
        let var = cfg
            .api_key_env
            .clone()
            .unwrap_or_else(|| BRAVE_DEFAULT_KEY_ENV.to_string());
        Self::from_env(var)
    }

    pub fn api_key_env(&self) -> &str {
        &self.api_key_env
    }

    /// Mask the key for diagnostics. Show only the last 4 chars.
    pub fn masked_key(&self) -> String {
        if self.api_key.len() <= 4 {
            return "****".to_string();
        }
        let tail = &self.api_key[self.api_key.len() - 4..];
        format!("***{tail}")
    }

    /// Parse a Brave Search API JSON response.
    pub fn parse_json(
        &self,
        json: &serde_json::Value,
    ) -> (Vec<SearchResult>, Vec<SearchWarning>) {
        let mut results = Vec::new();
        let mut warnings = Vec::new();
        let arr = json
            .get("web")
            .and_then(|w| w.get("results"))
            .and_then(|r| r.as_array());
        let Some(arr) = arr else {
            warnings.push(SearchWarning {
                provider_id: "brave".to_string(),
                message: "missing web.results in Brave response".to_string(),
            });
            return (results, warnings);
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
                .get("description")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from);
            let published_at = r
                .get("age")
                .and_then(|v| v.as_str())
                .and_then(parse_brave_age);
            results.push(SearchResult {
                title,
                url,
                snippet,
                published_at,
                rank: i,
                score: None,
                provider_id: "brave".to_string(),
                source_kind: SourceKind::Web,
                trust_level: TrustLevel::ExternalUntrusted,
            });
        }
        (results, warnings)
    }
}

fn parse_brave_age(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    // Brave returns relative ages like "2 days ago". If it returns a
    // machine-readable timestamp we'd parse that instead. We refuse to
    // fabricate dates, so on relative strings we return None.
    if chrono::DateTime::parse_from_rfc3339(s).is_ok() {
        return chrono::DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|d| d.with_timezone(&chrono::Utc));
    }
    None
}

#[async_trait]
impl SearchProvider for BraveProvider {
    fn id(&self) -> &'static str {
        "brave"
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
        let limit = query.max_results.clamp(1, 20).to_string();

        let body = match self
            .client
            .get(BRAVE_API_BASE)
            .header("User-Agent", ctx.user_agent.clone())
            .header("Accept", "application/json")
            .header("Accept-Encoding", "gzip")
            .header("X-Subscription-Token", self.api_key.as_str())
            .query(&[("q", query.query.as_str()), ("count", limit.as_str())])
            .timeout(ctx.timeout)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                resp.warnings.push(SearchWarning {
                    provider_id: self.id().to_string(),
                    message: format!("Brave API request failed: {e}"),
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
                429 => " (rate limited by Brave)".to_string(),
                _ => String::new(),
            };
            let preview = body_text.chars().take(200).collect::<String>();
            resp.warnings.push(SearchWarning {
                provider_id: self.id().to_string(),
                message: format!("Brave API returned status {status}{hint}; body preview: {preview:?}"),
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
                    message: format!("Brave JSON parse failed: {e} (body preview: {preview:?})"),
                });
                resp.elapsed_ms = started.elapsed().as_millis() as u64;
                return Ok(resp);
            }
        };

        let (mut results, mut warnings) = self.parse_json(&json);
        if results.is_empty() && warnings.is_empty() {
            warnings.push(SearchWarning {
                provider_id: self.id().to_string(),
                message: "Brave returned no web results".to_string(),
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

    const FIXTURE_BASIC: &str = include_str!("../../tests/fixtures/brave/basic.json");
    const FIXTURE_EMPTY: &str = include_str!("../../tests/fixtures/brave/empty.json");

    #[test]
    fn rejects_empty_key() {
        assert!(BraveProvider::with_api_key("").is_err());
    }

    #[test]
    fn from_env_rejects_empty_var_name() {
        assert!(BraveProvider::from_env("").is_err());
    }

    #[test]
    fn from_env_errors_when_missing() {
        // Unset the variable (best-effort); if it was set, the test still
        // exercises the not-set-or-unreadable code path.
        std::env::remove_var("EGGSEARCH_TEST_BRAVE_MISSING");
        let r = BraveProvider::from_env("EGGSEARCH_TEST_BRAVE_MISSING");
        assert!(r.is_err());
        let msg = r.unwrap_err().to_string();
        assert!(msg.contains("EGGSEARCH_TEST_BRAVE_MISSING"), "msg: {msg}");
    }

    #[test]
    fn from_env_reads_set_variable() {
        std::env::set_var("EGGSEARCH_TEST_BRAVE_SET", "test-secret-1234");
        let p = BraveProvider::from_env("EGGSEARCH_TEST_BRAVE_SET").unwrap();
        assert_eq!(p.api_key_env(), "EGGSEARCH_TEST_BRAVE_SET");
        assert_eq!(p.masked_key(), "***1234");
        std::env::remove_var("EGGSEARCH_TEST_BRAVE_SET");
    }

    #[test]
    fn from_env_errors_on_empty_value() {
        std::env::set_var("EGGSEARCH_TEST_BRAVE_EMPTY", "");
        let r = BraveProvider::from_env("EGGSEARCH_TEST_BRAVE_EMPTY");
        assert!(r.is_err());
        std::env::remove_var("EGGSEARCH_TEST_BRAVE_EMPTY");
    }

    #[test]
    fn from_config_uses_default_env_var() {
        std::env::set_var("BRAVE_SEARCH_API_KEY", "abc123def9");
        let cfg = ProviderConfig {
            enabled: true,
            ..Default::default()
        };
        let p = BraveProvider::from_config(&cfg).unwrap();
        assert_eq!(p.api_key_env(), "BRAVE_SEARCH_API_KEY");
        assert_eq!(p.masked_key(), "***def9");
        std::env::remove_var("BRAVE_SEARCH_API_KEY");
    }

    #[test]
    fn from_config_honors_explicit_env_var() {
        std::env::set_var("EGGSEARCH_BRAVE_KEY", "xyz987654");
        let cfg = ProviderConfig {
            enabled: true,
            api_key_env: Some("EGGSEARCH_BRAVE_KEY".to_string()),
            ..Default::default()
        };
        let p = BraveProvider::from_config(&cfg).unwrap();
        assert_eq!(p.api_key_env(), "EGGSEARCH_BRAVE_KEY");
        std::env::remove_var("EGGSEARCH_BRAVE_KEY");
    }

    #[test]
    fn from_config_disabled_errors() {
        let cfg = ProviderConfig::default();
        let r = BraveProvider::from_config(&cfg);
        assert!(r.is_err());
    }

    #[test]
    fn parses_basic_fixture() {
        let p = BraveProvider::with_api_key("dummy").unwrap();
        let v: serde_json::Value = serde_json::from_str(FIXTURE_BASIC).unwrap();
        let (results, warnings) = p.parse_json(&v);
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].provider_id, "brave");
        assert_eq!(results[0].url.as_str(), "https://tokio.rs/");
        assert!(results[0].snippet.as_deref().unwrap().contains("asynchronous"));
    }

    #[test]
    fn parses_empty_fixture() {
        let p = BraveProvider::with_api_key("dummy").unwrap();
        let v: serde_json::Value = serde_json::from_str(FIXTURE_EMPTY).unwrap();
        let (results, warnings) = p.parse_json(&v);
        assert!(results.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn missing_results_warns() {
        let p = BraveProvider::with_api_key("dummy").unwrap();
        let (results, warnings) = p.parse_json(&json!({}));
        assert!(results.is_empty());
        assert!(!warnings.is_empty());
    }
}
