//! Wikipedia search provider using the public REST + Action API.

use async_trait::async_trait;
use eggsearch_core::{
    error::CoreResult,
    provider::{SearchContext, SearchProvider, SearchProviderResponse},
    query::{SearchCategory, SearchQuery},
    result::{SearchResult, SearchWarning, SourceKind, TrustLevel},
};
use reqwest::Client;
use std::time::Instant;
use url::Url;

pub const WIKIPEDIA_REST_SUMMARY: &str = "https://{lang}.wikipedia.org/api/rest_v1/page/summary/";
pub const WIKIPEDIA_ACTION_SEARCH: &str = "https://{lang}.wikipedia.org/w/api.php";

#[derive(Clone, Debug)]
pub struct WikipediaProvider {
    client: Client,
    lang: String,
}

impl Default for WikipediaProvider {
    fn default() -> Self {
        Self {
            client: Client::builder()
                .user_agent(format!("eggsearch/{}", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("reqwest client"),
            lang: "en".to_string(),
        }
    }
}

impl WikipediaProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_lang(mut self, lang: impl Into<String>) -> Self {
        self.lang = lang.into();
        self
    }

    pub fn with_client(client: Client) -> Self {
        Self { client, lang: "en".into() }
    }

    /// Parse a Wikipedia action API search response.
    pub fn parse_search_json(
        &self,
        json: &serde_json::Value,
    ) -> (Vec<SearchResult>, Vec<SearchWarning>) {
        let mut results = Vec::new();
        let mut warnings = Vec::new();
        let pages = json.get("query").and_then(|q| q.get("search")).and_then(|s| s.as_array());
        let Some(pages) = pages else {
            warnings.push(SearchWarning {
                provider_id: "wikipedia".to_string(),
                message: "missing query.search in response".to_string(),
            });
            return (results, warnings);
        };
        for (i, p) in pages.iter().enumerate() {
            let title = p.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let snippet = p
                .get("snippet")
                .and_then(|v| v.as_str())
                .map(|s| strip_html(s));
            if title.is_empty() {
                continue;
            }
            let url = format!("https://{}.wikipedia.org/wiki/{}", self.lang, urlencode(&title.replace(' ', "_")));
            let url = match Url::parse(&url) {
                Ok(u) => u,
                Err(_) => continue,
            };
            results.push(SearchResult {
                title,
                url,
                snippet,
                published_at: None,
                rank: i,
                score: None,
                provider_id: "wikipedia".to_string(),
                source_kind: SourceKind::Reference,
                trust_level: TrustLevel::ExternalUntrusted,
            });
        }
        (results, warnings)
    }
}

fn urlencode(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

fn strip_html(s: &str) -> String {
    // Cheap HTML stripper for snippet fields that may contain <span> tags.
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}

#[derive(serde::Deserialize)]
struct _Unused;

#[async_trait]
impl SearchProvider for WikipediaProvider {
    fn id(&self) -> &'static str {
        "wikipedia"
    }

    fn categories(&self) -> &[SearchCategory] {
        &[SearchCategory::Reference, SearchCategory::General]
    }

    async fn search(
        &self,
        query: SearchQuery,
        ctx: SearchContext,
    ) -> CoreResult<SearchProviderResponse> {
        let started = Instant::now();
        let mut resp = SearchProviderResponse::empty(self.id(), query.clone());
        let lang = query
            .language
            .clone()
            .unwrap_or_else(|| self.lang.clone());
        let url = WIKIPEDIA_ACTION_SEARCH.replace("{lang}", &lang);
        let limit = query.max_results.clamp(1, 50).to_string();

        let body = match self
            .client
            .get(&url)
            .header("User-Agent", ctx.user_agent.clone())
            .query(&[
                ("action", "query"),
                ("list", "search"),
                ("format", "json"),
                ("srsearch", query.query.as_str()),
                ("srlimit", limit.as_str()),
                ("utf8", "1"),
                ("origin", "*"),
            ])
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

        let (mut results, mut warnings) = self.parse_search_json(&json);
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

    #[test]
    fn parses_search_response() {
        let p = WikipediaProvider::new();
        let body = json!({
            "query": {
                "search": [
                    {"title": "Rust (programming language)", "snippet": "Rust is a <span>systems</span> language"},
                    {"title": "Rust", "snippet": "Disambiguation page"}
                ]
            }
        });
        let (results, warnings) = p.parse_search_json(&body);
        assert!(warnings.is_empty());
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Rust (programming language)");
        assert!(results[0].snippet.as_deref().unwrap().contains("systems"));
        assert_eq!(results[0].url.as_str(), "https://en.wikipedia.org/wiki/Rust_%28programming_language%29");
    }

    #[test]
    fn missing_search_warns() {
        let p = WikipediaProvider::new();
        let (results, warnings) = p.parse_search_json(&json!({}));
        assert!(results.is_empty());
        assert!(!warnings.is_empty());
    }
}
