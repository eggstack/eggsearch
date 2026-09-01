use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;
use reqwest::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::{ResultMetadata, SearchResult};
use crate::core::security::{
    SeverityLevel, VulnerabilityMetadata, VulnerabilityReference, VulnerabilitySource,
};

static CVE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(CVE-\d{4}-\d{4,})\b").unwrap());
static GHSA_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(GHSA-[a-z0-9]{4}-[a-z0-9]{4}-[a-z0-9]{4})\b").unwrap());

const ENGINE: &str = "github_advisory";
const DEFAULT_BASE_URL: &str = "https://api.github.com";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const KEYWORD_SEARCH_MAX_PAGES: usize = 3;
const KEYWORD_SEARCH_PER_PAGE: usize = 100;

#[derive(Debug, Deserialize)]
struct GhAdvisoryResponse {
    #[serde(default)]
    advisories: Vec<GhAdvisory>,
}

#[derive(Debug, Deserialize)]
struct GhAdvisory {
    #[serde(default)]
    ghsa_id: Option<String>,
    #[serde(default)]
    cve_id: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    severity: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    withdrawn_at: Option<String>,
    #[serde(default)]
    ecosystem: Option<String>,
    #[serde(default)]
    package: Option<String>,
    #[serde(default)]
    vulnerabilities: Vec<GhVulnerability>,
    #[serde(default)]
    references: Vec<GhReference>,
    #[serde(default)]
    cvss: Option<GhCvss>,
}

#[derive(Debug, Deserialize)]
struct GhVulnerability {
    #[serde(default)]
    package: Option<GhPackage>,
    #[serde(default)]
    vulnerable_version_range: Option<String>,
    #[serde(default)]
    first_patched_version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhPackage {
    #[serde(default)]
    ecosystem: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhReference {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    source: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhCvss {
    #[serde(default)]
    score: Option<f64>,
    #[serde(default)]
    vector_string: Option<String>,
}

fn parse_query_type(query: &str) -> QueryType {
    if let Some(cap) = CVE_RE.captures(query) {
        return QueryType::CveId(cap[1].to_uppercase());
    }
    if let Some(cap) = GHSA_RE.captures(query) {
        return QueryType::GhsaId(cap[1].to_uppercase());
    }
    QueryType::Keyword(query.to_string())
}

enum QueryType {
    CveId(String),
    GhsaId(String),
    Keyword(String),
}

pub struct GithubAdvisoryEngine {
    pub client: Client,
    pub api_key: String,
}

impl super::SearchEngine for GithubAdvisoryEngine {
    fn name(&self) -> &'static str {
        ENGINE
    }

    fn advisory_capabilities(&self) -> super::AdvisoryCapabilities {
        super::AdvisoryCapabilities {
            lookup_by_id: true,
            query_by_package: true,
        }
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> super::BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(async move {
            if max_results == 0 {
                return Ok(Vec::new());
            }
            match parse_query_type(query) {
                QueryType::CveId(cve_id) => {
                    let results =
                        search_by_cve(&self.client, &self.api_key, &cve_id, timeout).await?;
                    Ok(results.into_iter().take(max_results).collect())
                }
                QueryType::GhsaId(ghsa_id) => {
                    let results =
                        search_by_ghsa(&self.client, &self.api_key, &ghsa_id, timeout).await?;
                    Ok(results.into_iter().take(max_results).collect())
                }
                QueryType::Keyword(keyword) => {
                    let results =
                        search_by_keyword(&self.client, &self.api_key, &keyword, timeout).await?;
                    Ok(results.into_iter().take(max_results).collect())
                }
            }
        })
    }

    fn lookup_advisory<'a>(
        &'a self,
        vuln_id: &'a str,
        timeout: Duration,
    ) -> super::BoxFuture<
        'a,
        Result<Option<crate::core::security::VulnerabilityMetadata>, EngineError>,
    > {
        Box::pin(async move {
            if vuln_id.to_uppercase().starts_with("CVE-") {
                let results = search_by_cve(
                    &self.client,
                    &self.api_key,
                    &vuln_id.to_uppercase(),
                    timeout,
                )
                .await?;
                if let Some(first) = results.first() {
                    if let ResultMetadata::Advisory(m) = &first.metadata {
                        return Ok(Some(*m.clone()));
                    }
                }
            } else if vuln_id.to_uppercase().starts_with("GHSA-") {
                let results = search_by_ghsa(
                    &self.client,
                    &self.api_key,
                    &vuln_id.to_uppercase(),
                    timeout,
                )
                .await?;
                if let Some(first) = results.first() {
                    if let ResultMetadata::Advisory(m) = &first.metadata {
                        return Ok(Some(*m.clone()));
                    }
                }
            }
            Ok(None)
        })
    }

    fn query_advisories_by_package<'a>(
        &'a self,
        ecosystem: &'a str,
        package: &'a str,
        _version: Option<&'a str>,
        max_results: usize,
        timeout: Duration,
    ) -> super::BoxFuture<'a, Result<Vec<crate::core::security::VulnerabilityMetadata>, EngineError>>
    {
        Box::pin(async move {
            let results =
                search_by_package(&self.client, &self.api_key, ecosystem, package, timeout).await?;
            Ok(results
                .into_iter()
                .take(max_results)
                .filter_map(|sr| match sr.metadata {
                    ResultMetadata::Advisory(m) => Some(*m),
                    _ => None,
                })
                .collect())
        })
    }
}

async fn search_by_cve(
    client: &Client,
    api_key: &str,
    cve_id: &str,
    timeout: Duration,
) -> Result<Vec<SearchResult>, EngineError> {
    let url = format!("{DEFAULT_BASE_URL}/advisories?cve_id={cve_id}");
    let bytes = match fetch_json(client, api_key, &url, timeout).await? {
        Some(b) => b,
        None => return Ok(Vec::new()),
    };
    let parsed: GhAdvisoryResponse =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;
    Ok(parsed
        .advisories
        .into_iter()
        .map(convert_to_result)
        .collect())
}

async fn search_by_ghsa(
    client: &Client,
    api_key: &str,
    ghsa_id: &str,
    timeout: Duration,
) -> Result<Vec<SearchResult>, EngineError> {
    let url = format!("{DEFAULT_BASE_URL}/advisories/{ghsa_id}");
    let bytes = match fetch_json(client, api_key, &url, timeout).await? {
        Some(b) => b,
        None => return Ok(Vec::new()),
    };
    let advisory: GhAdvisory =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;
    Ok(vec![convert_to_result(advisory)])
}

async fn search_by_keyword(
    client: &Client,
    api_key: &str,
    keyword: &str,
    timeout: Duration,
) -> Result<Vec<SearchResult>, EngineError> {
    let encoded = urlencoding::encode(keyword);
    let mut results = Vec::new();
    for page in 1..=KEYWORD_SEARCH_MAX_PAGES {
        let url = format!(
            "{DEFAULT_BASE_URL}/advisories?affects={encoded}&per_page={KEYWORD_SEARCH_PER_PAGE}&page={page}"
        );
        let bytes = match fetch_json(client, api_key, &url, timeout).await? {
            Some(b) => b,
            None => break,
        };
        let parsed: GhAdvisoryResponse =
            serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
                engine: ENGINE,
                reason: format!("invalid JSON: {e}"),
            })?;
        let page_len = parsed.advisories.len();
        results.extend(parsed.advisories.into_iter().map(convert_to_result));
        if page_len < KEYWORD_SEARCH_PER_PAGE {
            break;
        }
    }
    Ok(results)
}

async fn search_by_package(
    client: &Client,
    api_key: &str,
    ecosystem: &str,
    package: &str,
    timeout: Duration,
) -> Result<Vec<SearchResult>, EngineError> {
    let url = format!("{DEFAULT_BASE_URL}/advisories?affects={ecosystem}:{package}");
    let bytes = match fetch_json(client, api_key, &url, timeout).await? {
        Some(b) => b,
        None => return Ok(Vec::new()),
    };
    let parsed: GhAdvisoryResponse =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;
    Ok(parsed
        .advisories
        .into_iter()
        .map(convert_to_result)
        .collect())
}

async fn fetch_json(
    client: &Client,
    api_key: &str,
    url: &str,
    timeout: Duration,
) -> Result<Option<Vec<u8>>, EngineError> {
    let bytes = tokio::time::timeout(timeout, async {
        let resp = client
            .get(url)
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(|e| EngineError::Http {
                engine: ENGINE,
                source: e,
            })?;
        let status = resp.status();
        if status.as_u16() == 404 {
            return Ok(None);
        }
        if !status.is_success() {
            return Err(EngineError::BadStatus {
                engine: ENGINE,
                status: status.as_u16(),
            });
        }
        Ok(Some(
            super::read_bounded_body(resp, ENGINE, MAX_BODY_BYTES).await?,
        ))
    })
    .await
    .map_err(|_| EngineError::Timeout { engine: ENGINE })??;

    Ok(bytes)
}

fn convert_to_result(advisory: GhAdvisory) -> SearchResult {
    let metadata = convert_to_metadata(&advisory);
    let id_display = advisory
        .ghsa_id
        .as_deref()
        .or(advisory.cve_id.as_deref())
        .unwrap_or("unknown");
    let summary = advisory
        .summary
        .as_deref()
        .unwrap_or("GitHub Security Advisory");
    let title = format!("{id_display}: {summary}");
    let url = format!(
        "https://github.com/advisories/{}",
        advisory.ghsa_id.as_deref().unwrap_or(id_display)
    );
    let snippet = advisory.description.as_deref().map(|d| truncate(d, 500));

    SearchResult {
        title,
        url,
        snippet,
        source_engine: ENGINE.to_string(),
        metadata: ResultMetadata::Advisory(Box::new(metadata)),
    }
}

fn convert_to_metadata(advisory: &GhAdvisory) -> VulnerabilityMetadata {
    let cve_ids = advisory
        .cve_id
        .iter()
        .filter(|id| !id.is_empty())
        .cloned()
        .collect();
    let ghsa_ids = advisory
        .ghsa_id
        .iter()
        .filter(|id| !id.is_empty())
        .cloned()
        .collect();

    let severity = advisory.severity.as_ref().and_then(|s| {
        let s_lower = s.to_ascii_lowercase();
        match s_lower.as_str() {
            "critical" => Some(SeverityLevel::Critical),
            "high" => Some(SeverityLevel::High),
            "medium" => Some(SeverityLevel::Medium),
            "low" => Some(SeverityLevel::Low),
            _ => None,
        }
    });

    let cvss_score = advisory.cvss.as_ref().and_then(|c| c.score);
    let cvss_vector = advisory.cvss.as_ref().and_then(|c| c.vector_string.clone());

    let mut affected_ranges = Vec::new();
    let mut patched_versions = Vec::new();
    let mut package_name = None;
    let mut ecosystem = None;

    for vuln in &advisory.vulnerabilities {
        if let Some(ref pkg) = vuln.package {
            if package_name.is_none() {
                package_name = pkg.name.clone();
            }
            if ecosystem.is_none() {
                ecosystem = pkg.ecosystem.clone();
            }
        }
        if let Some(ref range) = vuln.vulnerable_version_range {
            if !range.is_empty() {
                affected_ranges.push(range.clone());
            }
        }
        if let Some(ref fixed) = vuln.first_patched_version {
            if !fixed.is_empty() {
                patched_versions.push(fixed.clone());
            }
        }
    }

    let references: Vec<VulnerabilityReference> = advisory
        .references
        .iter()
        .filter_map(|r| {
            r.url.as_ref().map(|url| VulnerabilityReference {
                url: url.clone(),
                kind: r.source.clone(),
            })
        })
        .collect();

    VulnerabilityMetadata {
        cve_ids,
        ghsa_ids,
        osv_ids: Vec::new(),
        rustsec_ids: Vec::new(),
        ecosystem: ecosystem.or_else(|| advisory.ecosystem.clone()),
        package: package_name.or_else(|| advisory.package.clone()),
        affected_ranges,
        patched_ranges: Vec::new(),
        vulnerable_versions: Vec::new(),
        patched_versions,
        severity,
        cvss_score,
        cvss_vector,
        epss_score: None,
        kev: None,
        published_at: advisory.published_at.clone(),
        modified_at: advisory.updated_at.clone(),
        withdrawn_at: advisory.withdrawn_at.clone(),
        references,
        source: VulnerabilitySource::GithubAdvisory,
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    crate::core::sanitize::truncate_at_word(s, max_chars)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_query_type_cve() {
        assert!(matches!(
            parse_query_type("CVE-2024-12345"),
            QueryType::CveId(_)
        ));
    }

    #[test]
    fn test_parse_query_type_ghsa() {
        assert!(matches!(
            parse_query_type("GHSA-xxxx-xxxx-xxxx"),
            QueryType::GhsaId(_)
        ));
    }

    #[test]
    fn test_parse_query_type_keyword() {
        assert!(matches!(
            parse_query_type("sql injection"),
            QueryType::Keyword(_)
        ));
    }

    #[test]
    fn test_convert_to_metadata_severity() {
        let advisory = GhAdvisory {
            ghsa_id: Some("GHSA-test-1234-abcd".to_string()),
            cve_id: Some("CVE-2024-0001".to_string()),
            summary: Some("Test".to_string()),
            description: None,
            severity: Some("CRITICAL".to_string()),
            published_at: Some("2024-01-15T10:00:00Z".to_string()),
            updated_at: None,
            withdrawn_at: None,
            ecosystem: Some("npm".to_string()),
            package: Some("test-pkg".to_string()),
            vulnerabilities: vec![GhVulnerability {
                package: None,
                vulnerable_version_range: Some("< 1.2.3".to_string()),
                first_patched_version: Some("1.2.3".to_string()),
            }],
            references: vec![],
            cvss: Some(GhCvss {
                score: Some(9.8),
                vector_string: Some("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H".to_string()),
            }),
        };
        let m = convert_to_metadata(&advisory);
        assert_eq!(m.severity, Some(SeverityLevel::Critical));
        assert_eq!(m.cvss_score, Some(9.8));
        assert_eq!(m.cve_ids, vec!["CVE-2024-0001"]);
        assert_eq!(m.ghsa_ids, vec!["GHSA-test-1234-abcd"]);
        assert_eq!(m.ecosystem.as_deref(), Some("npm"));
        assert_eq!(m.package.as_deref(), Some("test-pkg"));
        assert!(!m.affected_ranges.is_empty());
        assert!(!m.patched_versions.is_empty());
        assert_eq!(m.source, VulnerabilitySource::GithubAdvisory);
    }

    #[test]
    fn test_provider_descriptor() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc =
            built_in_provider_descriptor("github_advisory", true, false, true, false, None, None)
                .unwrap();
        assert_eq!(desc.id, "github_advisory");
        assert!(desc.capabilities.supports_security_search);
        assert!(desc.capabilities.supports_advisory_lookup_by_id);
        assert!(desc.capabilities.supports_advisory_lookup_by_package);
    }
}
