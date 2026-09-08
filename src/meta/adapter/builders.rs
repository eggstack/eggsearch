use crate::core::config::ApiProviderConfig;
use crate::core::provider::ProviderSkipCode;
use crate::meta::engines::build_http_client;
use std::sync::Arc;
use tracing::warn;

use super::{EngineList, SkippedProvider};

/// Build the default engine set used by the server.
///
/// `searxng_base_url`, when `Some`, is the base URL of a self-hosted
/// SearXNG instance. The `searxng` provider id is included in the engine
/// list only when the operator has both enabled it (in
/// `[search].providers`) and supplied a non-empty base URL. A missing or
/// empty base URL causes the `searxng` id to be reported as skipped and
/// a warning to be logged at startup by the caller.
///
/// `api_providers` contains the API-key backed provider configurations.
/// Each enabled entry with a resolvable API key env var produces a live
/// engine instance.
pub fn build_default_engines(
    enabled_providers: &[String],
    user_agent: Option<String>,
    searxng_base_url: Option<String>,
    api_providers: &std::collections::BTreeMap<String, ApiProviderConfig>,
) -> anyhow::Result<(EngineList, Vec<SkippedProvider>)> {
    use crate::meta::engines::{
        BraveApiEngine, BraveEngine, CisaKevEngine, CratesIoRegistryEngine, CrossRefEngine,
        DuckDuckGoEngine, ExaEngine, FirecrawlDeveloperEngine, GiteaCodeEngine, GiteaIssuesEngine,
        GiteaReleasesEngine, GithubAdvisoryEngine, GithubCodeEngine, GithubIssuesEngine,
        GithubReleasesEngine, GitlabCodeEngine, GitlabIssuesEngine, GitlabReleasesEngine,
        GoPkgRegistryEngine, MavenCentralRegistryEngine, MojeekEngine, NpmRegistryEngine,
        NugetRegistryEngine, NvdEngine, OpenAlexEngine, OsvEngine, PackagistRegistryEngine,
        PypiRegistryEngine, RubygemsRegistryEngine, RustSecEngine, SearxngEngine,
        SemanticScholarEngine, SourcegraphCodeEngine, StartpageEngine, TavilyEngine, YahooEngine,
    };

    let client = Arc::new(build_http_client(user_agent.as_deref())?);
    let mut engines: EngineList = Vec::new();
    let mut skipped: Vec<SkippedProvider> = Vec::new();

    for id in enabled_providers {
        match id.as_str() {
            "duckduckgo" => engines.push(Arc::new(DuckDuckGoEngine {
                client: client.clone(),
            })),
            "brave" => engines.push(Arc::new(BraveEngine {
                client: client.clone(),
            })),
            "startpage" => engines.push(Arc::new(StartpageEngine {
                client: client.clone(),
            })),
            "yahoo" => engines.push(Arc::new(YahooEngine {
                client: client.clone(),
            })),
            "mojeek" => engines.push(Arc::new(MojeekEngine {
                client: client.clone(),
            })),
            "osv" => engines.push(Arc::new(OsvEngine {
                client: client.clone(),
            })),
            "cisa_kev" => {
                engines.push(Arc::new(CisaKevEngine::new((*client).clone())));
            }
            "rustsec" => engines.push(Arc::new(RustSecEngine {
                client: (*client).clone(),
            })),
            "crates_io" => engines.push(Arc::new(CratesIoRegistryEngine {
                client: client.clone(),
            })),
            "pypi" => engines.push(Arc::new(PypiRegistryEngine {
                client: client.clone(),
            })),
            "npm_registry" => engines.push(Arc::new(NpmRegistryEngine {
                client: client.clone(),
            })),
            "go_pkg" => engines.push(Arc::new(GoPkgRegistryEngine {
                client: client.clone(),
            })),
            "maven_central" => engines.push(Arc::new(MavenCentralRegistryEngine {
                client: client.clone(),
            })),
            "nuget" => engines.push(Arc::new(NugetRegistryEngine {
                client: client.clone(),
            })),
            "rubygems" => engines.push(Arc::new(RubygemsRegistryEngine {
                client: client.clone(),
            })),
            "packagist" => engines.push(Arc::new(PackagistRegistryEngine {
                client: client.clone(),
            })),
            "openalex" => engines.push(Arc::new(OpenAlexEngine {
                client: client.clone(),
            })),
            "crossref" => engines.push(Arc::new(CrossRefEngine {
                client: client.clone(),
            })),
            "semantic_scholar" => {
                let api_key = std::env::var("SEMANTIC_SCHOLAR_API_KEY")
                    .ok()
                    .filter(|k| !k.is_empty());
                engines.push(Arc::new(SemanticScholarEngine {
                    client: client.clone(),
                    api_key,
                }));
            }
            "sourcegraph" => {
                if !api_providers.contains_key("sourcegraph") {
                    let api_key = std::env::var("SOURCEGRAPH_API_KEY")
                        .ok()
                        .filter(|k| !k.is_empty());
                    engines.push(Arc::new(SourcegraphCodeEngine {
                        client: client.clone(),
                        api_key,
                        base_url: None,
                    }));
                }
            }
            "nvd" => {
                let api_key = std::env::var("NVD_API_KEY").ok().filter(|k| !k.is_empty());
                engines.push(Arc::new(NvdEngine {
                    client: (*client).clone(),
                    api_key,
                }));
            }
            "firecrawl_developer" => {
                let api_key = crate::core::config::optional_api_key(id, api_providers);
                if crate::core::config::optional_api_key_misconfigured(id, api_providers) {
                    warn!(
                        provider_id = %id,
                        "optional API provider enabled without usable credential; continuing keyless"
                    );
                }
                let base_url = api_providers
                    .get(id)
                    .and_then(|cfg| cfg.base_url.clone())
                    .filter(|u| !u.is_empty());
                engines.push(Arc::new(FirecrawlDeveloperEngine {
                    client: client.clone(),
                    api_key,
                    base_url,
                }));
            }
            "searxng" => match searxng_base_url.as_deref().filter(|s| !s.is_empty()) {
                Some(base) => engines.push(Arc::new(SearxngEngine {
                    client: client.clone(),
                    base_url: base.to_string(),
                })),
                None => skipped.push(SkippedProvider {
                    id: id.clone(),
                    reason: format!(
                        "[{}] {}",
                        ProviderSkipCode::MissingSearxngConfig.as_str(),
                        ProviderSkipCode::MissingSearxngConfig.display_name()
                    ),
                }),
            },
            _ if api_providers.contains_key(id) => {}
            other => skipped.push(SkippedProvider {
                id: other.to_string(),
                reason: format!(
                    "[{}] {}",
                    ProviderSkipCode::UnknownProvider.as_str(),
                    ProviderSkipCode::UnknownProvider.display_name()
                ),
            }),
        }
    }

    for (id, api_cfg) in api_providers {
        if crate::core::provider::is_optional_api_provider(id) {
            continue;
        }
        if !api_cfg.enabled {
            continue;
        }
        if !enabled_providers.iter().any(|p| p == id) {
            continue;
        }
        if id == "sourcegraph" {
            let api_key = api_cfg
                .api_key_env
                .as_deref()
                .and_then(|env| std::env::var(env).ok())
                .filter(|k| !k.is_empty());
            engines.push(Arc::new(SourcegraphCodeEngine {
                client: client.clone(),
                api_key,
                base_url: api_cfg.base_url.clone(),
            }));
            continue;
        }
        if id == "semantic_scholar" {
            let api_key = api_cfg
                .api_key_env
                .as_deref()
                .and_then(|env| std::env::var(env).ok())
                .filter(|k| !k.is_empty());
            engines.push(Arc::new(SemanticScholarEngine {
                client: client.clone(),
                api_key,
            }));
            continue;
        }
        let api_key = match api_cfg
            .api_key_env
            .as_deref()
            .and_then(|env| std::env::var(env).ok())
        {
            Some(key) if !key.is_empty() => key,
            _ => {
                skipped.push(SkippedProvider {
                    id: id.clone(),
                    reason: format!(
                        "[{}] {}",
                        ProviderSkipCode::MissingApiKey.as_str(),
                        ProviderSkipCode::MissingApiKey.display_name()
                    ),
                });
                continue;
            }
        };
        match id.as_str() {
            "brave_api" => {
                engines.push(Arc::new(BraveApiEngine {
                    client: client.clone(),
                    api_key,
                    base_url: api_cfg.base_url.clone(),
                }));
            }
            "exa" => {
                engines.push(Arc::new(ExaEngine {
                    client: client.clone(),
                    api_key,
                    base_url: api_cfg.base_url.clone(),
                }));
            }
            "tavily" => {
                engines.push(Arc::new(TavilyEngine {
                    client: client.clone(),
                    api_key,
                    base_url: api_cfg.base_url.clone(),
                }));
            }
            "github_code" => {
                engines.push(Arc::new(GithubCodeEngine {
                    client: client.clone(),
                    api_key,
                    base_url: api_cfg.base_url.clone(),
                }));
            }
            "github_issues" => {
                engines.push(Arc::new(GithubIssuesEngine {
                    client: client.clone(),
                    api_key,
                    base_url: api_cfg.base_url.clone(),
                }));
            }
            "github_releases" => {
                engines.push(Arc::new(GithubReleasesEngine {
                    client: client.clone(),
                    api_key,
                    base_url: api_cfg.base_url.clone(),
                }));
            }
            "gitlab_code" => {
                engines.push(Arc::new(GitlabCodeEngine {
                    client: client.clone(),
                    api_key,
                    base_url: api_cfg.base_url.clone(),
                }));
            }
            "gitlab_issues" => {
                engines.push(Arc::new(GitlabIssuesEngine {
                    client: client.clone(),
                    api_key,
                    base_url: api_cfg.base_url.clone(),
                }));
            }
            "gitlab_releases" => {
                engines.push(Arc::new(GitlabReleasesEngine {
                    client: client.clone(),
                    api_key,
                    base_url: api_cfg.base_url.clone(),
                }));
            }
            "gitea_code" => {
                let base = api_cfg.base_url.clone().unwrap_or_default();
                if base.is_empty() {
                    skipped.push(SkippedProvider {
                        id: id.clone(),
                        reason: format!(
                            "[{}] {}",
                            ProviderSkipCode::MissingBaseUrl.as_str(),
                            ProviderSkipCode::MissingBaseUrl.display_name()
                        ),
                    });
                    continue;
                }
                engines.push(Arc::new(GiteaCodeEngine {
                    client: client.clone(),
                    api_key,
                    base_url: base,
                }));
            }
            "gitea_issues" => {
                let base = api_cfg.base_url.clone().unwrap_or_default();
                if base.is_empty() {
                    skipped.push(SkippedProvider {
                        id: id.clone(),
                        reason: format!(
                            "[{}] {}",
                            ProviderSkipCode::MissingBaseUrl.as_str(),
                            ProviderSkipCode::MissingBaseUrl.display_name()
                        ),
                    });
                    continue;
                }
                engines.push(Arc::new(GiteaIssuesEngine {
                    client: client.clone(),
                    api_key,
                    base_url: base,
                }));
            }
            "gitea_releases" => {
                let base = api_cfg.base_url.clone().unwrap_or_default();
                if base.is_empty() {
                    skipped.push(SkippedProvider {
                        id: id.clone(),
                        reason: format!(
                            "[{}] {}",
                            ProviderSkipCode::MissingBaseUrl.as_str(),
                            ProviderSkipCode::MissingBaseUrl.display_name()
                        ),
                    });
                    continue;
                }
                engines.push(Arc::new(GiteaReleasesEngine {
                    client: client.clone(),
                    api_key,
                    base_url: base,
                }));
            }
            "github_advisory" => {
                engines.push(Arc::new(GithubAdvisoryEngine {
                    client: (*client).clone(),
                    api_key,
                }));
            }
            other => {
                skipped.push(SkippedProvider {
                    id: other.to_string(),
                    reason: format!(
                        "[{}] {}",
                        ProviderSkipCode::UnknownProvider.as_str(),
                        ProviderSkipCode::UnknownProvider.display_name()
                    ),
                });
            }
        }
    }

    Ok((engines, skipped))
}
