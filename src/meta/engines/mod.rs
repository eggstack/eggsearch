//! Vendored HTML search engines for the metasearch adapter. These are
//! internal implementation details; the public types are re-exported
//! from [`crate::meta`].

#![allow(missing_docs)]

mod api;
pub(crate) use api::{get_json_bounded, truncate_body};

pub mod arxiv;
pub mod brave;
pub mod brave_api;
pub mod cisa_kev;
pub mod crates_io;
pub mod crossref;
pub mod duckduckgo;
pub mod error;
pub mod exa;
pub mod firecrawl_developer;
pub mod gitea_code;
pub mod gitea_issues;
pub mod gitea_releases;
pub mod github_advisory;
pub mod github_code;
pub mod github_issues;
pub mod github_releases;
pub mod github_repositories;
pub mod gitlab_code;
pub mod gitlab_issues;
pub mod gitlab_releases;
pub mod go_pkg;
pub mod hn_algolia;
pub mod kagi;
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
pub mod pubmed;
pub mod pypi;
pub mod render;
pub mod request;
pub mod rubygems;
pub mod rustsec;
pub mod searxng;
pub mod semantic_scholar;
pub mod serpapi;
pub mod sourcegraph;
pub mod startpage;
mod startpage_payload;
pub mod tavily;
#[cfg(feature = "mock")]
pub mod test_harness;
pub mod wikipedia;
pub mod yahoo;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use eggfetch_core::Client;
use futures::StreamExt;

use self::error::EngineError;
use self::models::SearchResult;

pub use self::request::EngineSearchRequest;

// A heap-allocated future that is Send — required for dyn trait + tokio multi-thread.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AdvisoryCapabilities {
    pub lookup_by_id: bool,
    pub query_by_package: bool,
}

pub trait SearchEngine: Send + Sync {
    fn name(&self) -> &'static str;

    /// Run a single structured search request. Engines read
    /// query/max-results/timeout from the request and ignore optional
    /// constraints they do not natively support.
    fn search<'a>(
        &'a self,
        request: &'a EngineSearchRequest,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>>;

    /// Run a search returning results plus provider-neutral retrieval
    /// metadata (e.g. scope-index evidence). Defaults to `search` with
    /// empty metadata so existing engines need no changes; specialist
    /// engines override to preserve retrieval-state evidence.
    fn search_batch<'a>(
        &'a self,
        request: &'a EngineSearchRequest,
    ) -> BoxFuture<'a, Result<models::EngineSearchBatch, EngineError>> {
        Box::pin(async move {
            self.search(request)
                .await
                .map(models::EngineSearchBatch::from_results)
        })
    }

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

/// Defines the forwarding methods of a `SearchEngine` implementation.
///
/// ```ignore
/// impl_engine!(
///     DuckDuckGoEngine,
///     "duckduckgo",
///     |engine, request| duckduckgo::search(
///         &engine.client,
///         &request.query,
///         request.max_results,
///         request.timeout,
///     ),
/// );
/// ```
macro_rules! impl_engine {
    ($engine_type:ty, $name:literal, |$engine:ident, $request:ident| $call:expr $(,)?) => {
        impl SearchEngine for $engine_type {
            fn name(&self) -> &'static str {
                $name
            }

            fn search<'a>(
                &'a self,
                request: &'a EngineSearchRequest,
            ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
                let $engine = self;
                let $request = request;
                Box::pin($call)
            }
        }
    };
}

pub struct DuckDuckGoEngine {
    pub client: Arc<Client>,
}

pub struct BraveEngine {
    pub client: Arc<Client>,
    pub renderer: Option<Arc<dyn render::SerpRenderer>>,
}

pub struct StartpageEngine {
    pub client: Arc<Client>,
    pub challenge: Arc<crate::core::config::ChallengeSection>,
}

pub struct YahooEngine {
    pub client: Arc<Client>,
    pub renderer: Option<Arc<dyn render::SerpRenderer>>,
}

pub struct MojeekEngine {
    pub client: Arc<Client>,
    pub challenge: Arc<crate::core::config::ChallengeSection>,
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
    pub base_url: Option<String>,
}

pub struct FirecrawlDeveloperEngine {
    pub client: Arc<Client>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

pub struct ExaEngine {
    pub client: Arc<Client>,
    pub api_key: String,
    pub base_url: Option<String>,
}

pub struct TavilyEngine {
    pub client: Arc<Client>,
    pub api_key: String,
    pub base_url: Option<String>,
}

/// SerpAPI Google Search engine. The credential is a query parameter by
/// upstream contract, so it is attached only after the endpoint is accepted.
pub struct SerpapiEngine {
    pub client: Arc<Client>,
    pub api_key: String,
    pub base_url: Option<String>,
}

/// Kagi Search API v1 engine. Kagi's API terms forbid circumventing rate
/// limits, so quota responses are terminal provider-scoped failures.
pub struct KagiEngine {
    pub client: Arc<Client>,
    pub api_key: String,
    pub base_url: Option<String>,
}

/// Keyless MediaWiki Action API engine. No operator base URL override in
/// the shipped surface.
pub struct WikipediaEngine {
    pub client: Arc<Client>,
}

/// arXiv Atom API engine. The pacing gate is shared process-wide so
/// concurrent calls cannot bypass the upstream request spacing.
pub struct ArxivEngine {
    pub client: Arc<Client>,
    pub gate: Arc<arxiv::RequestGate>,
}

/// NCBI E-utilities engine. The API key and contact identity are optional
/// and never gate routing.
pub struct PubmedEngine {
    pub client: Arc<Client>,
    pub api_key: Option<String>,
    pub email: Option<String>,
    pub base_url: Option<String>,
}

/// Keyless GitHub repository discovery engine. An optional operator token
/// only raises upstream limits.
pub struct GithubRepositoriesEngine {
    pub client: Arc<Client>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

/// Keyless HN Search API (Algolia) engine.
pub struct HnAlgoliaEngine {
    pub client: Arc<Client>,
}

impl_engine!(DuckDuckGoEngine, "duckduckgo", |engine, request| {
    duckduckgo::search(
        &engine.client,
        &request.query,
        request.max_results,
        request.timeout,
    )
});

impl_engine!(BraveEngine, "brave", |engine, request| brave::search(
    &engine.client,
    &request.query,
    request.max_results,
    request.timeout,
    engine.renderer.as_ref()
));

impl_engine!(StartpageEngine, "startpage", |engine, request| {
    startpage::search(
        &engine.client,
        &request.query,
        request.max_results,
        request.timeout,
        &engine.challenge,
    )
});

impl_engine!(YahooEngine, "yahoo", |engine, request| yahoo::search(
    &engine.client,
    &request.query,
    request.max_results,
    request.timeout,
    engine.renderer.as_ref()
));

impl_engine!(MojeekEngine, "mojeek", |engine, request| mojeek::search(
    &engine.client,
    &request.query,
    request.max_results,
    request.timeout,
    &engine.challenge
));

impl_engine!(SearxngEngine, "searxng", |engine, request| async move {
    searxng::search(
        &engine.client,
        engine.base_url.as_str(),
        &request.query,
        request.max_results,
        request.timeout,
    )
    .await
});

impl_engine!(BraveApiEngine, "brave_api", |engine, request| async move {
    brave_api::search(
        &engine.client,
        &engine.api_key,
        engine.base_url.as_deref(),
        request,
    )
    .await
});

impl_engine!(
    GithubCodeEngine,
    "github_code",
    |engine, request| async move {
        github_code::search(
            &engine.client,
            &engine.api_key,
            engine.base_url.as_deref(),
            &request.query,
            request.max_results,
            request.timeout,
        )
        .await
    }
);

impl_engine!(
    GithubIssuesEngine,
    "github_issues",
    |engine, request| async move {
        github_issues::search(
            &engine.client,
            &engine.api_key,
            engine.base_url.as_deref(),
            &request.query,
            request.max_results,
            request.timeout,
        )
        .await
    }
);

impl_engine!(
    GithubReleasesEngine,
    "github_releases",
    |engine, request| async move {
        github_releases::search(
            &engine.client,
            &engine.api_key,
            engine.base_url.as_deref(),
            &request.query,
            request.max_results,
            request.timeout,
        )
        .await
    }
);

impl_engine!(
    GitlabCodeEngine,
    "gitlab_code",
    |engine, request| async move {
        gitlab_code::search(
            &engine.client,
            &engine.api_key,
            engine.base_url.as_deref(),
            &request.query,
            request.max_results,
            request.timeout,
        )
        .await
    }
);

impl_engine!(
    GitlabIssuesEngine,
    "gitlab_issues",
    |engine, request| async move {
        gitlab_issues::search(
            &engine.client,
            &engine.api_key,
            engine.base_url.as_deref(),
            &request.query,
            request.max_results,
            request.timeout,
        )
        .await
    }
);

impl_engine!(
    GitlabReleasesEngine,
    "gitlab_releases",
    |engine, request| async move {
        gitlab_releases::search(
            &engine.client,
            &engine.api_key,
            engine.base_url.as_deref(),
            &request.query,
            request.max_results,
            request.timeout,
        )
        .await
    }
);

impl_engine!(
    GiteaCodeEngine,
    "gitea_code",
    |engine, request| async move {
        gitea_code::search(
            &engine.client,
            &engine.api_key,
            Some(engine.base_url.as_str()),
            &request.query,
            request.max_results,
            request.timeout,
        )
        .await
    }
);

impl_engine!(
    GiteaIssuesEngine,
    "gitea_issues",
    |engine, request| async move {
        gitea_issues::search(
            &engine.client,
            &engine.api_key,
            Some(engine.base_url.as_str()),
            &request.query,
            request.max_results,
            request.timeout,
        )
        .await
    }
);

impl_engine!(
    GiteaReleasesEngine,
    "gitea_releases",
    |engine, request| async move {
        gitea_releases::search(
            &engine.client,
            &engine.api_key,
            Some(engine.base_url.as_str()),
            &request.query,
            request.max_results,
            request.timeout,
        )
        .await
    }
);

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
        request: &'a EngineSearchRequest,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(osv::search(
            &self.client,
            &request.query,
            request.max_results,
            request.timeout,
        ))
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

impl_engine!(CratesIoRegistryEngine, "crates_io", |engine, request| {
    crates_io::search(
        &engine.client,
        &request.query,
        request.max_results,
        request.timeout,
    )
});

impl_engine!(PypiRegistryEngine, "pypi", |engine, request| pypi::search(
    &engine.client,
    &request.query,
    request.max_results,
    request.timeout
));

impl_engine!(NpmRegistryEngine, "npm_registry", |engine, request| {
    npm_registry::search(
        &engine.client,
        &request.query,
        request.max_results,
        request.timeout,
    )
});

impl_engine!(GoPkgRegistryEngine, "go_pkg", |engine, request| {
    go_pkg::search(
        &engine.client,
        &request.query,
        request.max_results,
        request.timeout,
    )
});

impl_engine!(
    MavenCentralRegistryEngine,
    "maven_central",
    |engine, request| maven_central::search(
        &engine.client,
        &request.query,
        request.max_results,
        request.timeout
    )
);

impl_engine!(NugetRegistryEngine, "nuget", |engine, request| {
    nuget::search(
        &engine.client,
        &request.query,
        request.max_results,
        request.timeout,
    )
});

impl_engine!(RubygemsRegistryEngine, "rubygems", |engine, request| {
    rubygems::search(
        &engine.client,
        &request.query,
        request.max_results,
        request.timeout,
    )
});

impl_engine!(PackagistRegistryEngine, "packagist", |engine, request| {
    packagist::search(
        &engine.client,
        &request.query,
        request.max_results,
        request.timeout,
    )
});

impl_engine!(OpenAlexEngine, "openalex", |engine, request| {
    openalex::search(
        &engine.client,
        &request.query,
        request.max_results,
        request.timeout,
    )
});

impl_engine!(CrossRefEngine, "crossref", |engine, request| {
    crossref::search(
        &engine.client,
        &request.query,
        request.max_results,
        request.timeout,
    )
});

impl_engine!(
    SemanticScholarEngine,
    "semantic_scholar",
    |engine, request| async move {
        semantic_scholar::search(
            &engine.client,
            &request.query,
            request.max_results,
            request.timeout,
            engine.api_key.as_deref(),
        )
        .await
    }
);

impl_engine!(
    SourcegraphCodeEngine,
    "sourcegraph",
    |engine, request| async move {
        sourcegraph::search(
            &engine.client,
            engine.api_key.as_deref(),
            engine.base_url.as_deref(),
            &request.query,
            request.max_results,
            request.timeout,
        )
        .await
    }
);

impl SearchEngine for FirecrawlDeveloperEngine {
    fn name(&self) -> &'static str {
        "firecrawl_developer"
    }

    fn search<'a>(
        &'a self,
        request: &'a EngineSearchRequest,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(async move {
            firecrawl_developer::search(
                &self.client,
                self.api_key.as_deref(),
                self.base_url.as_deref(),
                request,
            )
            .await
            .map(|batch| batch.results)
        })
    }

    fn search_batch<'a>(
        &'a self,
        request: &'a EngineSearchRequest,
    ) -> BoxFuture<'a, Result<models::EngineSearchBatch, EngineError>> {
        Box::pin(async move {
            firecrawl_developer::search(
                &self.client,
                self.api_key.as_deref(),
                self.base_url.as_deref(),
                request,
            )
            .await
        })
    }

    fn supports_role(&self, role: &crate::core::evidence_role::EvidenceRole) -> bool {
        use crate::core::evidence_role::EvidenceRole;
        matches!(
            role,
            EvidenceRole::OfficialDocumentation
                | EvidenceRole::IssueOrIncidentDiscussion
                | EvidenceRole::PullRequestOrDesignReview
        )
    }
}

impl_engine!(ExaEngine, "exa", |engine, request| async move {
    exa::search(
        &engine.client,
        &engine.api_key,
        engine.base_url.as_deref(),
        request,
    )
    .await
});

impl_engine!(TavilyEngine, "tavily", |engine, request| async move {
    tavily::search(
        &engine.client,
        &engine.api_key,
        engine.base_url.as_deref(),
        request,
    )
    .await
});

impl_engine!(SerpapiEngine, "serpapi", |engine, request| async move {
    serpapi::search(
        &engine.client,
        &engine.api_key,
        engine.base_url.as_deref(),
        request,
    )
    .await
});

impl_engine!(KagiEngine, "kagi", |engine, request| async move {
    kagi::search(
        &engine.client,
        &engine.api_key,
        engine.base_url.as_deref(),
        request,
    )
    .await
});

impl_engine!(WikipediaEngine, "wikipedia", |engine, request| {
    wikipedia::search(&engine.client, None, request)
});

impl_engine!(ArxivEngine, "arxiv", |engine, request| arxiv::search(
    &engine.client,
    None,
    &engine.gate,
    request
));

impl_engine!(PubmedEngine, "pubmed", |engine, request| pubmed::search(
    &engine.client,
    engine.base_url.as_deref(),
    engine.api_key.as_deref(),
    engine.email.as_deref(),
    request
));

impl_engine!(HnAlgoliaEngine, "hn_algolia", |engine, request| {
    hn_algolia::search(&engine.client, None, request)
});

impl_engine!(
    GithubRepositoriesEngine,
    "github_repositories",
    |engine, request| github_repositories::search(
        &engine.client,
        engine.base_url.as_deref(),
        engine.api_key.as_deref(),
        request
    )
);

pub use cisa_kev::CisaKevEngine;
pub use github_advisory::GithubAdvisoryEngine;
pub use nvd::NvdEngine;
pub use rustsec::RustSecEngine;
// Browser-like default UA unless the operator configured one.
const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
     Chrome/124.0.0.0 Safari/537.36";

/// Stream response bytes under a hard cap, returning a parse error on overflow.
pub async fn read_bounded_body(
    mut response: eggfetch_core::Response,
    engine: &'static str,
    max_bytes: usize,
) -> Result<Vec<u8>, EngineError> {
    if let Some(content_length) = response.content_length() {
        if content_length > max_bytes as u64 {
            return Err(EngineError::ParseFailed {
                engine,
                reason: format!(
                    "response body too large (Content-Length: {content_length} bytes, limit: {max_bytes} bytes)",
                ),
            });
        }
    }
    let mut body = Vec::with_capacity(max_bytes.min(64 * 1024));
    let mut stream = response
        .bytes_stream()
        .map_err(|e| EngineError::Http { engine, source: e })?;
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| EngineError::Http { engine, source: e })?;
        push_bounded_chunk(&mut body, &chunk, max_bytes, engine)?;
    }
    Ok(body)
}

/// Append a streamed chunk unless it would exceed `max_bytes`.
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

/// Build the shared eggfetch client for vendored search engines.
/// Cookies are not persisted across requests or operator sessions.
pub fn build_http_client(user_agent: Option<&str>) -> anyhow::Result<Client> {
    build_http_client_with_egress(user_agent, &crate::core::config::EgressSection::default())
}

/// Build the shared provider HTTP client, optionally routed through egress.
pub fn build_http_client_with_egress(
    user_agent: Option<&str>,
    egress: &crate::core::config::EgressSection,
) -> anyhow::Result<Client> {
    let ua = resolve_user_agent(user_agent).to_string();
    let timeout = eggfetch_core::Timeout {
        pool: Some(Duration::from_secs(20)),
        connect: Some(Duration::from_secs(20)),
        write: Some(Duration::from_secs(20)),
        read: Some(Duration::from_secs(20)),
        total: Some(Duration::from_secs(20)),
    };
    let builder = Client::builder()
        .user_agent(&ua)
        .timeout(timeout)
        .follow_redirects(true)
        .max_redirects(10)
        .redirect_policy(eggfetch_core::RedirectPolicy::strict(10));
    let builder = crate::fetch::egress::apply_route(builder, egress)?;
    Ok(builder.build())
}

pub fn engine_timeout(timeout: Duration) -> eggfetch_core::Timeout {
    eggfetch_core::Timeout {
        pool: Some(timeout),
        connect: Some(timeout),
        write: Some(timeout),
        read: Some(timeout),
        total: Some(timeout),
    }
}

pub fn is_timeout_error(error: &eggfetch_core::Error) -> bool {
    matches!(
        error,
        eggfetch_core::Error::Timeout { .. } | eggfetch_core::Error::TransportIoTimeout { .. }
    )
}

pub fn map_request_error(engine: &'static str, error: eggfetch_core::Error) -> EngineError {
    if is_timeout_error(&error) {
        EngineError::Timeout { engine }
    } else {
        EngineError::Http {
            engine,
            source: error,
        }
    }
}

// Prefer the operator's UA; otherwise use the browser-like fallback.
fn resolve_user_agent(user_agent: Option<&str>) -> &str {
    user_agent.unwrap_or(DEFAULT_USER_AGENT)
}

mod response;

pub(crate) use response::read_search_body;

/// A marker that only appears on a documented non-results page.
#[derive(Clone, Copy, Debug)]
pub(crate) enum PageMarker {
    /// A CSS selector that only matches on the marked page.
    Selector(&'static str),
    /// A literal run of rendered page text that only appears on the marked page.
    Text(&'static str),
}

impl PageMarker {
    fn label(self) -> &'static str {
        match self {
            Self::Selector(marker) | Self::Text(marker) => marker,
        }
    }
}

/// Declared result container and non-results page markers for one HTML scraper.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ScrapeShape {
    pub(crate) result_container: &'static str,
    pub(crate) empty_markers: &'static [PageMarker],
    pub(crate) blocked_markers: &'static [PageMarker],
}

/// Return results, explicit empty, blocked, or structural-parse-failure outcomes for a scraper.
pub(crate) fn finalize_scrape(
    engine: &'static str,
    document: &scraper::Html,
    shape: &ScrapeShape,
    results: Vec<SearchResult>,
) -> Result<Vec<SearchResult>, EngineError> {
    if !results.is_empty() {
        return Ok(results);
    }

    let mut rendered: Option<String> = None;
    for marker in shape.blocked_markers {
        if marker_matches(engine, *marker, document, &mut rendered)? {
            return Err(EngineError::Blocked {
                engine,
                reason: format!("challenge page detected ({})", marker.label()),
            });
        }
    }
    for marker in shape.empty_markers {
        if marker_matches(engine, *marker, document, &mut rendered)? {
            return Ok(results);
        }
    }

    let container = sel(engine, shape.result_container)?;
    let reason = if document.select(&container).next().is_some() {
        format!(
            "result container '{}' produced no parseable results",
            shape.result_container
        )
    } else {
        format!("result container '{}' missing", shape.result_container)
    };
    Err(EngineError::ParseFailed { engine, reason })
}

/// Parse a CSS selector, mapping invalid syntax to
/// [`EngineError::ParseFailed`].
pub(crate) fn sel(engine: &'static str, selector: &str) -> Result<scraper::Selector, EngineError> {
    scraper::Selector::parse(selector).map_err(|e| EngineError::ParseFailed {
        engine,
        reason: format!("invalid selector '{selector}': {e:?}"),
    })
}

pub(super) fn marker_matches(
    engine: &'static str,
    marker: PageMarker,
    document: &scraper::Html,
    rendered: &mut Option<String>,
) -> Result<bool, EngineError> {
    match marker {
        PageMarker::Selector(selector) => {
            Ok(document.select(&sel(engine, selector)?).next().is_some())
        }
        PageMarker::Text(text) => {
            let rendered = rendered.get_or_insert_with(|| rendered_text(document));
            Ok(rendered.contains(text))
        }
    }
}

fn rendered_text(document: &scraper::Html) -> String {
    let mut text = String::new();
    for node in document.root_element().descendants() {
        let Some(value) = node.value().as_text() else {
            continue;
        };
        if node.ancestors().any(|ancestor| {
            ancestor
                .value()
                .as_element()
                .is_some_and(|element| matches!(element.name(), "script" | "style"))
        }) {
            continue;
        }
        text.push_str(value);
        text.push(' ');
    }
    text
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

    fn row(title: &str, url: &str) -> SearchResult {
        SearchResult {
            title: title.to_string(),
            url: url.to_string(),
            snippet: None,
            source_engine: "test".to_string(),
            excerpts: Vec::new(),
            published_at: None,
            metadata: Default::default(),
        }
    }

    const SHAPE: ScrapeShape = ScrapeShape {
        result_container: "div.result",
        empty_markers: &[
            PageMarker::Selector("div.no-results"),
            PageMarker::Text("No results."),
        ],
        blocked_markers: &[
            PageMarker::Selector("#challenge-form"),
            PageMarker::Text("Please verify you are a human"),
        ],
    };

    #[test]
    fn finalize_scrape_returns_parsed_rows() {
        let document = scraper::Html::parse_document(
            r#"<html><body><div class="result">ok</div></body></html>"#,
        );
        let results = finalize_scrape(
            "test",
            &document,
            &SHAPE,
            vec![row("Title", "https://example.com")],
        )
        .expect("parsed rows");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Title");
    }

    #[test]
    fn finalize_scrape_returns_empty_for_text_marker() {
        let document =
            scraper::Html::parse_document(r#"<html><body><p>No results.</p></body></html>"#);
        let results = finalize_scrape("test", &document, &SHAPE, Vec::new()).expect("empty state");
        assert!(results.is_empty());
    }

    #[test]
    fn finalize_scrape_returns_empty_for_selector_marker() {
        let document = scraper::Html::parse_document(
            r#"<html><body><div class="no-results"></div></body></html>"#,
        );
        let results = finalize_scrape("test", &document, &SHAPE, Vec::new()).expect("empty state");
        assert!(results.is_empty());
    }

    #[test]
    fn finalize_scrape_ignores_script_text_markers() {
        let document = scraper::Html::parse_document(
            r#"<html><body><script>const t = "No results.";</script><div id="page"></div></body></html>"#,
        );
        let err = finalize_scrape("test", &document, &SHAPE, Vec::new()).expect_err("drift");
        assert!(matches!(
            err,
            EngineError::ParseFailed { engine: "test", .. }
        ));
    }

    #[test]
    fn finalize_scrape_flags_missing_container() {
        let document = scraper::Html::parse_document(r#"<html><body><p>changed</p></body></html>"#);
        let err = finalize_scrape("test", &document, &SHAPE, Vec::new()).expect_err("drift");
        match err {
            EngineError::ParseFailed { engine, reason } => {
                assert_eq!(engine, "test");
                assert!(reason.contains("div.result"), "reason: {reason}");
                assert!(reason.contains("missing"), "reason: {reason}");
            }
            other => panic!("expected ParseFailed, got {other:?}"),
        }
    }

    #[test]
    fn finalize_scrape_flags_container_without_rows() {
        let document = scraper::Html::parse_document(
            r#"<html><body><div class="result"><span>no title</span></div></body></html>"#,
        );
        let err = finalize_scrape("test", &document, &SHAPE, Vec::new()).expect_err("drift");
        match err {
            EngineError::ParseFailed { reason, .. } => {
                assert!(
                    reason.contains("produced no parseable results"),
                    "reason: {reason}"
                );
            }
            other => panic!("expected ParseFailed, got {other:?}"),
        }
    }

    #[test]
    fn finalize_scrape_flags_blocked_page() {
        for html in [
            r#"<html><body><form id="challenge-form"></form></body></html>"#,
            r#"<html><body><h1>Please verify you are a human</h1></body></html>"#,
        ] {
            let document = scraper::Html::parse_document(html);
            let err = finalize_scrape("test", &document, &SHAPE, Vec::new()).expect_err("blocked");
            assert!(matches!(err, EngineError::Blocked { engine: "test", .. }));
        }
    }

    #[test]
    fn sel_rejects_invalid_selector_syntax() {
        let err = sel("test", "div[").expect_err("invalid selector");
        assert!(matches!(
            err,
            EngineError::ParseFailed { engine: "test", .. }
        ));
    }
}
