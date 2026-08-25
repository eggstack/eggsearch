//! Vendored HTML search engines for the metasearch adapter. These are
//! internal implementation details; the public types are re-exported
//! from [`crate::meta`].

#![allow(missing_docs)]

pub mod brave;
pub mod brave_api;
pub mod cisa_kev;
pub mod crates_io;
pub mod crossref;
pub mod duckduckgo;
pub mod error;
pub mod gitea_code;
pub mod gitea_issues;
pub mod gitea_releases;
pub mod github_advisory;
pub mod github_code;
pub mod github_issues;
pub mod github_releases;
pub mod gitlab_code;
pub mod gitlab_issues;
pub mod gitlab_releases;
pub mod go_pkg;
pub mod kev;
pub mod maven_central;
pub mod models;
pub mod mojeek;
pub mod normalizer;
pub mod npm_registry;
pub mod nuget;
pub mod nvd;
pub mod openalex;
pub mod osv;
pub mod packagist;
pub mod pypi;
pub mod rubygems;
pub mod rustsec;
pub mod searxng;
pub mod semantic_scholar;
pub mod sourcegraph;
pub mod startpage;
pub mod yahoo;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use reqwest::Client;

use self::error::EngineError;
use self::models::SearchResult;

// A heap-allocated future that is Send — required for dyn trait + tokio multi-thread.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AdvisoryCapabilities {
    pub lookup_by_id: bool,
    pub query_by_package: bool,
}

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

    /// Whether this engine can serve the given evidence role.
    /// Returns `true` by default (conservative: assume all roles are
    /// reachable via generic search). Override to return `false` for
    /// roles this engine provably cannot serve.
    fn supports_role(&self, _role: &crate::core::evidence_role::EvidenceRole) -> bool {
        true
    }

    fn advisory_capabilities(&self) -> AdvisoryCapabilities {
        AdvisoryCapabilities::default()
    }

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

pub struct CratesIoRegistryEngine {
    pub client: Arc<Client>,
}

pub struct PypiRegistryEngine {
    pub client: Arc<Client>,
}

pub struct NpmRegistryEngine {
    pub client: Arc<Client>,
}

pub struct GoPkgRegistryEngine {
    pub client: Arc<Client>,
}

pub struct MavenCentralRegistryEngine {
    pub client: Arc<Client>,
}

pub struct NugetRegistryEngine {
    pub client: Arc<Client>,
}

pub struct RubygemsRegistryEngine {
    pub client: Arc<Client>,
}

pub struct PackagistRegistryEngine {
    pub client: Arc<Client>,
}

pub struct OpenAlexEngine {
    pub client: Arc<Client>,
}

pub struct CrossRefEngine {
    pub client: Arc<Client>,
}

pub struct SemanticScholarEngine {
    pub client: Arc<Client>,
    pub api_key: Option<String>,
}

pub struct SourcegraphCodeEngine {
    pub client: Arc<Client>,
    pub api_key: Option<String>,
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

    fn advisory_capabilities(&self) -> AdvisoryCapabilities {
        AdvisoryCapabilities {
            lookup_by_id: true,
            query_by_package: true,
        }
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

impl SearchEngine for CratesIoRegistryEngine {
    fn name(&self) -> &'static str {
        "crates_io"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(crates_io::search(&self.client, query, max_results, timeout))
    }
}

impl SearchEngine for PypiRegistryEngine {
    fn name(&self) -> &'static str {
        "pypi"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(pypi::search(&self.client, query, max_results, timeout))
    }
}

impl SearchEngine for NpmRegistryEngine {
    fn name(&self) -> &'static str {
        "npm_registry"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(npm_registry::search(
            &self.client,
            query,
            max_results,
            timeout,
        ))
    }
}

impl SearchEngine for GoPkgRegistryEngine {
    fn name(&self) -> &'static str {
        "go_pkg"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(go_pkg::search(&self.client, query, max_results, timeout))
    }
}

impl SearchEngine for MavenCentralRegistryEngine {
    fn name(&self) -> &'static str {
        "maven_central"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(maven_central::search(
            &self.client,
            query,
            max_results,
            timeout,
        ))
    }
}

impl SearchEngine for NugetRegistryEngine {
    fn name(&self) -> &'static str {
        "nuget"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(nuget::search(&self.client, query, max_results, timeout))
    }
}

impl SearchEngine for RubygemsRegistryEngine {
    fn name(&self) -> &'static str {
        "rubygems"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(rubygems::search(&self.client, query, max_results, timeout))
    }
}

impl SearchEngine for PackagistRegistryEngine {
    fn name(&self) -> &'static str {
        "packagist"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(packagist::search(&self.client, query, max_results, timeout))
    }
}

impl SearchEngine for OpenAlexEngine {
    fn name(&self) -> &'static str {
        "openalex"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(openalex::search(&self.client, query, max_results, timeout))
    }
}

impl SearchEngine for CrossRefEngine {
    fn name(&self) -> &'static str {
        "crossref"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(crossref::search(&self.client, query, max_results, timeout))
    }
}

impl SearchEngine for SemanticScholarEngine {
    fn name(&self) -> &'static str {
        "semantic_scholar"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(async move {
            semantic_scholar::search(
                &self.client,
                query,
                max_results,
                timeout,
                self.api_key.as_deref(),
            )
            .await
        })
    }
}

impl SearchEngine for SourcegraphCodeEngine {
    fn name(&self) -> &'static str {
        "sourcegraph"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(async move {
            sourcegraph::search(
                &self.client,
                self.api_key.as_deref(),
                query,
                max_results,
                timeout,
            )
            .await
        })
    }
}

pub use cisa_kev::CisaKevEngine;
pub use github_advisory::GithubAdvisoryEngine;
pub use nvd::NvdEngine;
pub use rustsec::RustSecEngine;

// Browser-like UA used as the fallback when no operator-supplied UA is provided.
// Mimic a real browser as closely as possible to avoid bot-detection rejections
// from HTML providers — but only when the operator has not configured their own.
const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
     Chrome/124.0.0.0 Safari/537.36";

/// Read an HTTP response body with a streaming byte cap.
///
/// Checks `Content-Length` upfront and streams at most `max_bytes + 1`
/// bytes, aborting when the cap is crossed. Returns the buffered body
/// on success or `EngineError::ParseFailed` on overflow.
pub async fn read_bounded_body(
    response: reqwest::Response,
    engine: &'static str,
    max_bytes: usize,
) -> Result<Vec<u8>, EngineError> {
    if let Some(content_length) = response.content_length() {
        if content_length as usize > max_bytes {
            return Err(EngineError::ParseFailed {
                engine,
                reason: format!(
                    "response body too large (Content-Length: {content_length} bytes, limit: {max_bytes} bytes)",
                ),
            });
        }
    }
    let mut body = Vec::with_capacity(max_bytes.min(64 * 1024));
    let mut stream = response.bytes_stream();
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| EngineError::Http { engine, source: e })?;
        push_bounded_chunk(&mut body, &chunk, max_bytes, engine)?;
    }
    Ok(body)
}

/// Append one streamed chunk to `body` under a hard byte cap.
///
/// Returns `Ok(())` when the chunk fits within `max_bytes`, or an
/// `EngineError::ParseFailed` overflow error otherwise. This is the
/// single implementation of the streaming cap semantics shared by
/// `read_bounded_body` (and exercised by the `bounded_response_reader`
/// fuzz target).
pub fn push_bounded_chunk(
    body: &mut Vec<u8>,
    chunk: &[u8],
    max_bytes: usize,
    engine: &'static str,
) -> Result<(), EngineError> {
    if chunk.len() > max_bytes.saturating_sub(body.len()) {
        return Err(EngineError::ParseFailed {
            engine,
            reason: format!(
                "response body too large: read {} bytes, limit is {max_bytes} bytes",
                body.len().max(chunk.len())
            ),
        });
    }
    body.extend_from_slice(chunk);
    Ok(())
}

/// Returns `true` if `url` parses as an HTTP or HTTPS URL.
pub fn is_http_url(url: &str) -> bool {
    url::Url::parse(url)
        .ok()
        .is_some_and(|u| matches!(u.scheme(), "http" | "https"))
}

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
