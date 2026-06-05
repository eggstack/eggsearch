//! docs.rs provider.
//!
//! docs.rs has no public full-text search endpoint, so this provider
//! implements a conservative lookup: it queries the public crate page
//! summary and returns the canonical docs.rs URL plus a short description.

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

pub const DOCS_RS_CRATE_SUMMARY: &str = "https://docs.rs/crate/{crate}";

#[derive(Clone, Debug)]
pub struct DocsRsProvider {
    client: Client,
}

impl Default for DocsRsProvider {
    fn default() -> Self {
        Self {
            client: Client::builder()
                .user_agent(format!("eggsearch/{}", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("reqwest client"),
        }
    }
}

impl DocsRsProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_client(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl SearchProvider for DocsRsProvider {
    fn id(&self) -> &'static str {
        "docs_rs"
    }

    fn categories(&self) -> &[SearchCategory] {
        &[SearchCategory::Documentation, SearchCategory::PackageRegistry]
    }

    async fn search(
        &self,
        query: SearchQuery,
        ctx: SearchContext,
    ) -> CoreResult<SearchProviderResponse> {
        let started = Instant::now();
        let mut resp = SearchProviderResponse::empty(self.id(), query.clone());

        // docs.rs lookup accepts crate names; take the first whitespace-separated
        // token as the candidate crate name.
        let crate_name = query.query.split_whitespace().next().unwrap_or("").trim();
        if crate_name.is_empty() {
            resp.warnings.push(SearchWarning {
                provider_id: self.id().to_string(),
                message: "empty crate name".to_string(),
            });
            return Ok(resp);
        }
        if !is_plausible_crate_name(crate_name) {
            resp.warnings.push(SearchWarning {
                provider_id: self.id().to_string(),
                message: format!("'{crate_name}' does not look like a valid crate name"),
            });
            return Ok(resp);
        }

        let url = DOCS_RS_CRATE_SUMMARY.replace("{crate}", crate_name);
        let parsed = match Url::parse(&url) {
            Ok(u) => u,
            Err(e) => {
                resp.warnings.push(SearchWarning {
                    provider_id: self.id().to_string(),
                    message: format!("invalid url: {e}"),
                });
                return Ok(resp);
            }
        };

        let body = match self
            .client
            .get(parsed.clone())
            .header("User-Agent", ctx.user_agent.clone())
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

        if body.status() == reqwest::StatusCode::NOT_FOUND {
            resp.warnings.push(SearchWarning {
                provider_id: self.id().to_string(),
                message: format!("crate '{crate_name}' not found on docs.rs"),
            });
            resp.elapsed_ms = started.elapsed().as_millis() as u64;
            return Ok(resp);
        }
        if !body.status().is_success() {
            resp.warnings.push(SearchWarning {
                provider_id: self.id().to_string(),
                message: format!("upstream status {}", body.status()),
            });
            resp.elapsed_ms = started.elapsed().as_millis() as u64;
            return Ok(resp);
        }

        // We don't fully parse the HTML; we just confirm reachability and return
        // the canonical docs URL as a documentation source card.
        resp.results.push(SearchResult {
            title: format!("{crate_name} - Rust"),
            url: parsed,
            snippet: Some(format!("Documentation for the '{crate_name}' crate on docs.rs.")),
            published_at: None,
            rank: 0,
            score: None,
            provider_id: "docs_rs".to_string(),
            source_kind: SourceKind::Documentation,
            trust_level: TrustLevel::ExternalUntrusted,
        });
        resp.elapsed_ms = started.elapsed().as_millis() as u64;
        Ok(resp)
    }
}

fn is_plausible_crate_name(s: &str) -> bool {
    if s.is_empty() || s.len() > 64 {
        return false;
    }
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_validation() {
        assert!(is_plausible_crate_name("tokio"));
        assert!(is_plausible_crate_name("serde_json"));
        assert!(is_plausible_crate_name("egg-search"));
        assert!(!is_plausible_crate_name(""));
        assert!(!is_plausible_crate_name("not a name"));
        assert!(!is_plausible_crate_name("with.dot"));
    }
}
