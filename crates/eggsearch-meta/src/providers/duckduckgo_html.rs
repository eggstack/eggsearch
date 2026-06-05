//! DuckDuckGo HTML provider.
//!
//! Uses the no-JS HTML endpoint (POST) and parses the result list. This is
//! best-effort: layout changes are common, so the parser is defensive and
//! emits warnings when the parsed result count looks suspicious.

use async_trait::async_trait;
use eggsearch_core::{
    error::CoreResult,
    normalize::canonicalize,
    provider::{SearchContext, SearchProvider, SearchProviderResponse},
    query::SearchQuery,
    result::{SearchResult, SearchWarning, SourceKind, TrustLevel},
};
use reqwest::Client;
use scraper::{Html, Selector};
use sha2::{Digest, Sha256};
use std::time::Instant;
use tracing::warn;
use url::Url;

pub const DUCKDUCKGO_HTML_URL: &str = "https://html.duckduckgo.com/html/";

#[derive(Clone, Debug)]
pub struct DuckDuckGoHtmlProvider {
    client: Client,
    endpoint: String,
    /// `user_agent` override; if None, uses `ctx.user_agent`.
    pub user_agent: Option<String>,
}

impl Default for DuckDuckGoHtmlProvider {
    fn default() -> Self {
        Self {
            client: Client::builder()
                .user_agent("eggsearch/0.1 (+https://github.com/anomalyco/eggsearch)")
                .build()
                .expect("reqwest client"),
            endpoint: DUCKDUCKGO_HTML_URL.to_string(),
            user_agent: None,
        }
    }
}

impl DuckDuckGoHtmlProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_client(client: Client) -> Self {
        Self {
            client,
            ..Self::default()
        }
    }

    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    /// Parse a DuckDuckGo HTML response into search results.
    ///
    /// Exposed for fixture testing.
    pub fn parse_html(&self, html: &str) -> (Vec<SearchResult>, Vec<SearchWarning>) {
        let document = Html::parse_document(html);
        // The HTML endpoint renders results in `.result` divs.
        let result_sel = Selector::parse(".result").ok();
        let title_link_sel = Selector::parse("a.result__a").ok();
        let snippet_sel = Selector::parse(".result__snippet").ok();
        let url_sel = Selector::parse("a.result__url").ok();

        let mut results = Vec::new();
        let mut warnings = Vec::new();

        if let (Some(rs), Some(tl), Some(ss)) = (result_sel.clone(), title_link_sel, snippet_sel) {
            for (i, r) in document.select(&rs).enumerate() {
                let title = r
                    .select(&tl)
                    .next()
                    .map(|n| n.text().collect::<String>().trim().to_string())
                    .unwrap_or_default();
                // DuckDuckGo wraps real URLs in a `/l/?uddg=...` redirect.
                // We try to extract the real URL by reading the `uddg` query
                // parameter; if absent, we fall back to the href.
                let href = r
                    .select(&tl)
                    .next()
                    .and_then(|n| n.value().attr("href"))
                    .map(String::from)
                    .unwrap_or_default();
                let url = extract_uddg(&href).unwrap_or(href);

                let snippet = r
                    .select(&ss)
                    .next()
                    .map(|n| n.text().collect::<String>().trim().to_string())
                    .filter(|s| !s.is_empty());

                if title.is_empty() || url.is_empty() {
                    continue;
                }
                let url = match Url::parse(&url) {
                    Ok(u) => u,
                    Err(_) => continue,
                };
                let url = canonicalize(url.as_str()).unwrap_or(url);
                results.push(SearchResult {
                    title,
                    url,
                    snippet,
                    published_at: None,
                    rank: i,
                    score: None,
                    provider_id: "duckduckgo_html".to_string(),
                    source_kind: SourceKind::Web,
                    trust_level: TrustLevel::ExternalUntrusted,
                });
            }
        }

        if results.is_empty() {
            warnings.push(SearchWarning {
                provider_id: "duckduckgo_html".to_string(),
                message: "no results parsed from response (layout change?)".to_string(),
            });
        }
        let _ = (url_sel, result_sel); // silence unused
        (results, warnings)
    }
}

fn extract_uddg(href: &str) -> Option<String> {
    // DuckDuckGo wraps URLs as protocol-relative `//duckduckgo.com/l/?uddg=...`.
    // We can't parse that directly, so add a dummy scheme first.
    let with_scheme = if href.starts_with("//") {
        format!("https:{href}")
    } else {
        href.to_string()
    };
    let url = Url::parse(&with_scheme).ok()?;
    for (k, v) in url.query_pairs() {
        if k == "uddg" {
            return Some(v.into_owned());
        }
    }
    None
}

#[async_trait]
impl SearchProvider for DuckDuckGoHtmlProvider {
    fn id(&self) -> &'static str {
        "duckduckgo_html"
    }

    async fn search(
        &self,
        query: SearchQuery,
        ctx: SearchContext,
    ) -> CoreResult<SearchProviderResponse> {
        let started = Instant::now();
        let mut resp = SearchProviderResponse::empty(self.id(), query.clone());

        let ua = self.user_agent.clone().unwrap_or(ctx.user_agent.clone());
        let body = match self
            .client
            .post(&self.endpoint)
            .header("User-Agent", ua)
            .header("Accept", "text/html,application/xhtml+xml")
            .form(&[("q", query.query.as_str()), ("kl", "us-en")])
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
            let s = body.status();
            resp.warnings.push(SearchWarning {
                provider_id: self.id().to_string(),
                message: format!("upstream returned status {s}"),
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

        let hash = hex::encode(Sha256::digest(text.as_bytes()));
        let (mut results, mut warnings) = self.parse_html(&text);
        results.truncate(query.max_results.max(1));
        resp.results = results;
        resp.warnings.append(&mut warnings);
        resp.raw_response_hash = Some(hash);
        resp.elapsed_ms = started.elapsed().as_millis() as u64;
        if resp.results.is_empty() {
            warn!(provider = "duckduckgo_html", "no results parsed");
        }
        Ok(resp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_BASIC: &str = include_str!("../../tests/fixtures/duckduckgo/basic.html");
    const FIXTURE_EMPTY: &str = include_str!("../../tests/fixtures/duckduckgo/no_results.html");

    #[test]
    fn parse_basic_fixture() {
        let p = DuckDuckGoHtmlProvider::new();
        let (results, warnings) = p.parse_html(FIXTURE_BASIC);
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].provider_id, "duckduckgo_html");
        assert!(results[0].url.as_str().contains("rust-lang.org"));
        assert!(results[0].snippet.is_some());
    }

    #[test]
    fn parse_empty_fixture_emits_warning() {
        let p = DuckDuckGoHtmlProvider::new();
        let (results, warnings) = p.parse_html(FIXTURE_EMPTY);
        assert!(results.is_empty());
        assert!(!warnings.is_empty());
    }

    #[test]
    fn parse_garbage_does_not_panic() {
        let p = DuckDuckGoHtmlProvider::new();
        let (results, warnings) = p.parse_html("<html><body>random</body></html>");
        assert!(results.is_empty());
        assert!(!warnings.is_empty());
    }
}
