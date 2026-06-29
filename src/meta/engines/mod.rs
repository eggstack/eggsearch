//! Vendored HTML search engines for the metasearch adapter. These are
//! internal implementation details; the public types are re-exported
//! from [`crate::meta`].

#![allow(missing_docs)]

pub mod brave;
pub mod brave_api;
pub mod duckduckgo;
pub mod error;
pub mod gitea_code;
pub mod gitea_issues;
pub mod gitea_releases;
pub mod github_code;
pub mod github_issues;
pub mod github_releases;
pub mod gitlab_code;
pub mod gitlab_issues;
pub mod gitlab_releases;
pub mod kev;
pub mod models;
pub mod mojeek;
pub mod normalizer;
pub mod osv;
pub mod searxng;
pub mod startpage;
pub mod yahoo;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;

use self::error::EngineError;
use self::models::SearchResult;

// A heap-allocated future that is Send — required for dyn trait + tokio multi-thread.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait SearchEngine: Send + Sync {
    fn name(&self) -> &'static str;

    /// Run a single search query. `timeout` is the per-engine request
    /// timeout, supplied by the adapter (bounded above by the
    /// configured global timeout).
    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>>;

    /// Look up a vulnerability by ID (CVE, GHSA, OSV, etc.).
    /// Returns `Ok(None)` if not found or not supported by this engine.
    fn lookup_advisory<'a>(
        &'a self,
        _vuln_id: &'a str,
        _timeout: Duration,
    ) -> BoxFuture<'a, Result<Option<crate::core::security::VulnerabilityMetadata>, EngineError>>
    {
        Box::pin(async { Ok(None) })
    }

    /// Query vulnerabilities by package name, ecosystem, and optional version.
    /// Returns `Ok(Vec::new())` if not supported by this engine.
    fn query_advisories_by_package<'a>(
        &'a self,
        _ecosystem: &'a str,
        _package: &'a str,
        _version: Option<&'a str>,
        _max_results: usize,
        _timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<crate::core::security::VulnerabilityMetadata>, EngineError>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

pub struct DuckDuckGoEngine {
    pub client: Arc<Client>,
}

pub struct BraveEngine {
    pub client: Arc<Client>,
}

pub struct StartpageEngine {
    pub client: Arc<Client>,
}

pub struct YahooEngine {
    pub client: Arc<Client>,
}

pub struct MojeekEngine {
    pub client: Arc<Client>,
}

pub struct SearxngEngine {
    pub client: Arc<Client>,
    pub base_url: String,
}

pub struct BraveApiEngine {
    pub client: Arc<Client>,
    pub api_key: String,
    pub base_url: Option<String>,
}

pub struct GithubCodeEngine {
    pub client: Arc<Client>,
    pub api_key: String,
    pub base_url: Option<String>,
}

pub struct GithubIssuesEngine {
    pub client: Arc<Client>,
    pub api_key: String,
    pub base_url: Option<String>,
}

pub struct GithubReleasesEngine {
    pub client: Arc<Client>,
    pub api_key: String,
    pub base_url: Option<String>,
}

pub struct GitlabCodeEngine {
    pub client: Arc<Client>,
    pub api_key: String,
    pub base_url: Option<String>,
}

pub struct GitlabIssuesEngine {
    pub client: Arc<Client>,
    pub api_key: String,
    pub base_url: Option<String>,
}

pub struct GitlabReleasesEngine {
    pub client: Arc<Client>,
    pub api_key: String,
    pub base_url: Option<String>,
}

pub struct GiteaCodeEngine {
    pub client: Arc<Client>,
    pub api_key: String,
    pub base_url: String,
}

pub struct GiteaIssuesEngine {
    pub client: Arc<Client>,
    pub api_key: String,
    pub base_url: String,
}

pub struct GiteaReleasesEngine {
    pub client: Arc<Client>,
    pub api_key: String,
    pub base_url: String,
}

pub struct OsvEngine {
    pub client: Arc<Client>,
}

impl SearchEngine for DuckDuckGoEngine {
    fn name(&self) -> &'static str {
        "duckduckgo"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(duckduckgo::search(
            &self.client,
            query,
            max_results,
            timeout,
        ))
    }
}

impl SearchEngine for BraveEngine {
    fn name(&self) -> &'static str {
        "brave"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(brave::search(&self.client, query, max_results, timeout))
    }
}

impl SearchEngine for StartpageEngine {
    fn name(&self) -> &'static str {
        "startpage"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(startpage::search(&self.client, query, max_results, timeout))
    }
}

impl SearchEngine for YahooEngine {
    fn name(&self) -> &'static str {
        "yahoo"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(yahoo::search(&self.client, query, max_results, timeout))
    }
}

impl SearchEngine for MojeekEngine {
    fn name(&self) -> &'static str {
        "mojeek"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(mojeek::search(&self.client, query, max_results, timeout))
    }
}

impl SearchEngine for SearxngEngine {
    fn name(&self) -> &'static str {
        "searxng"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(async move {
            searxng::search(
                &self.client,
                self.base_url.as_str(),
                query,
                max_results,
                timeout,
            )
            .await
        })
    }
}

impl SearchEngine for BraveApiEngine {
    fn name(&self) -> &'static str {
        "brave_api"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(async move {
            brave_api::search(
                &self.client,
                &self.api_key,
                self.base_url.as_deref(),
                query,
                max_results,
                timeout,
            )
            .await
        })
    }
}

impl SearchEngine for GithubCodeEngine {
    fn name(&self) -> &'static str {
        "github_code"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(async move {
            github_code::search(
                &self.client,
                &self.api_key,
                self.base_url.as_deref(),
                query,
                max_results,
                timeout,
            )
            .await
        })
    }
}

impl SearchEngine for GithubIssuesEngine {
    fn name(&self) -> &'static str {
        "github_issues"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(async move {
            github_issues::search(
                &self.client,
                &self.api_key,
                self.base_url.as_deref(),
                query,
                max_results,
                timeout,
            )
            .await
        })
    }
}

impl SearchEngine for GithubReleasesEngine {
    fn name(&self) -> &'static str {
        "github_releases"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(async move {
            github_releases::search(
                &self.client,
                &self.api_key,
                self.base_url.as_deref(),
                query,
                max_results,
                timeout,
            )
            .await
        })
    }
}

impl SearchEngine for GitlabCodeEngine {
    fn name(&self) -> &'static str {
        "gitlab_code"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(async move {
            gitlab_code::search(
                &self.client,
                &self.api_key,
                self.base_url.as_deref(),
                query,
                max_results,
                timeout,
            )
            .await
        })
    }
}

impl SearchEngine for GitlabIssuesEngine {
    fn name(&self) -> &'static str {
        "gitlab_issues"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(async move {
            gitlab_issues::search(
                &self.client,
                &self.api_key,
                self.base_url.as_deref(),
                query,
                max_results,
                timeout,
            )
            .await
        })
    }
}

impl SearchEngine for GitlabReleasesEngine {
    fn name(&self) -> &'static str {
        "gitlab_releases"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(async move {
            gitlab_releases::search(
                &self.client,
                &self.api_key,
                self.base_url.as_deref(),
                query,
                max_results,
                timeout,
            )
            .await
        })
    }
}

impl SearchEngine for GiteaCodeEngine {
    fn name(&self) -> &'static str {
        "gitea_code"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(async move {
            gitea_code::search(
                &self.client,
                &self.api_key,
                Some(self.base_url.as_str()),
                query,
                max_results,
                timeout,
            )
            .await
        })
    }
}

impl SearchEngine for GiteaIssuesEngine {
    fn name(&self) -> &'static str {
        "gitea_issues"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(async move {
            gitea_issues::search(
                &self.client,
                &self.api_key,
                Some(self.base_url.as_str()),
                query,
                max_results,
                timeout,
            )
            .await
        })
    }
}

impl SearchEngine for GiteaReleasesEngine {
    fn name(&self) -> &'static str {
        "gitea_releases"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(async move {
            gitea_releases::search(
                &self.client,
                &self.api_key,
                Some(self.base_url.as_str()),
                query,
                max_results,
                timeout,
            )
            .await
        })
    }
}

impl SearchEngine for OsvEngine {
    fn name(&self) -> &'static str {
        "osv"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(osv::search(&self.client, query, max_results, timeout))
    }

    fn lookup_advisory<'a>(
        &'a self,
        vuln_id: &'a str,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Option<crate::core::security::VulnerabilityMetadata>, EngineError>>
    {
        Box::pin(osv::lookup_by_id(&self.client, vuln_id, timeout))
    }

    fn query_advisories_by_package<'a>(
        &'a self,
        ecosystem: &'a str,
        package: &'a str,
        version: Option<&'a str>,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<crate::core::security::VulnerabilityMetadata>, EngineError>> {
        Box::pin(osv::query_package(
            &self.client,
            ecosystem,
            package,
            version,
            max_results,
            timeout,
        ))
    }
}

// Browser-like UA used as the fallback when no operator-supplied UA is provided.
// Mimic a real browser as closely as possible to avoid bot-detection rejections
// from HTML providers — but only when the operator has not configured their own.
const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
     Chrome/124.0.0.0 Safari/537.36";

/// Build the reqwest client used by the vendored search engines.
///
/// We intentionally do **not** enable a cookie store on this client:
/// a long-lived MCP server should not persist cookies across requests
/// or across operator sessions. Cookies were historically needed for
/// certain HTML providers but are no longer required for any of the
/// vendored engines.
pub fn build_http_client(user_agent: Option<&str>) -> anyhow::Result<Client> {
    let ua = resolve_user_agent(user_agent);

    let builder = Client::builder()
        .user_agent(ua)
        .gzip(true)
        .brotli(true)
        .timeout(Duration::from_secs(20));

    let client = builder.build()?;

    Ok(client)
}

// Pick the UA the client will actually send: the operator's configured value
// if present, otherwise the browser-like fallback.
fn resolve_user_agent(user_agent: Option<&str>) -> &str {
    user_agent.unwrap_or(DEFAULT_USER_AGENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_user_agent_uses_configured_value() {
        assert_eq!(
            resolve_user_agent(Some("eggsearch/test-ua")),
            "eggsearch/test-ua"
        );
    }

    #[test]
    fn resolve_user_agent_uses_default_when_none() {
        let ua = resolve_user_agent(None);
        assert!(
            ua.contains("Mozilla"),
            "default UA should be Mozilla-like, got: {ua}"
        );
    }

    #[test]
    fn build_http_client_succeeds_with_configured_ua() {
        let client = build_http_client(Some("eggsearch/test-ua")).expect("build");
        drop(client);
    }

    #[test]
    fn build_http_client_succeeds_with_default_ua() {
        let client = build_http_client(None).expect("build");
        drop(client);
    }
}
