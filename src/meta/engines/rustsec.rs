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

static RUSTSEC_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(RUSTSEC-\d{4}-\d{4,})\b").unwrap());

const ENGINE: &str = "rustsec";
const DEFAULT_BASE_URL: &str = "https://rustsec.org";
const MAX_BODY_BYTES: usize = 5 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct RustSecAdvisory {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    package: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    date: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    cvss: Option<RustSecCvss>,
    #[serde(default)]
    patched_versions: Option<String>,
    #[serde(default)]
    unaffected_versions: Option<String>,
    #[serde(default)]
    references: Vec<RustSecReference>,
}

#[derive(Debug, Deserialize)]
struct RustSecCvss {
    #[serde(default)]
    score: Option<f64>,
    #[serde(default)]
    vector_string: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RustSecReference {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

pub struct RustSecEngine {
    pub client: Client,
}

fn parse_query_type(query: &str) -> QueryType {
    if let Some(cap) = RUSTSEC_RE.captures(query) {
        return QueryType::ById(cap[1].to_uppercase());
    }
    QueryType::ByKeyword(query.to_string())
}

enum QueryType {
    ById(String),
    ByKeyword(String),
}

impl super::SearchEngine for RustSecEngine {
    fn name(&self) -> &'static str {
        ENGINE
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
                QueryType::ById(id) => {
                    let results = lookup_by_id(&self.client, &id, timeout).await?;
                    Ok(results.into_iter().take(max_results).collect())
                }
                QueryType::ByKeyword(keyword) => {
                    let results = search_by_keyword(&self.client, &keyword, timeout).await?;
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
            let upper = vuln_id.to_uppercase();
            if !upper.starts_with("RUSTSEC-") {
                return Ok(None);
            }
            let results = lookup_by_id(&self.client, &upper, timeout).await?;
            Ok(results.into_iter().next().map(|sr| match sr.metadata {
                ResultMetadata::Advisory(m) => *m,
                _ => VulnerabilityMetadata {
                    rustsec_ids: vec![upper],
                    source: VulnerabilitySource::Rustsec,
                    ..Default::default()
                },
            }))
        })
    }

    fn query_advisories_by_package<'a>(
        &'a self,
        _ecosystem: &'a str,
        package: &'a str,
        _version: Option<&'a str>,
        max_results: usize,
        timeout: Duration,
    ) -> super::BoxFuture<'a, Result<Vec<crate::core::security::VulnerabilityMetadata>, EngineError>>
    {
        Box::pin(async move {
            let results = search_by_keyword(&self.client, package, timeout).await?;
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

async fn fetch_json(client: &Client, url: &str, timeout: Duration) -> Result<Vec<u8>, EngineError> {
    let response = tokio::time::timeout(timeout, client.get(url).send())
        .await
        .map_err(|_| EngineError::Timeout { engine: ENGINE })?
        .map_err(|e| EngineError::Http {
            engine: ENGINE,
            source: e,
        })?;

    let status = response.status();
    if status.as_u16() == 404 {
        return Err(EngineError::ParseFailed {
            engine: ENGINE,
            reason: "not found".to_string(),
        });
    }
    if !status.is_success() {
        return Err(EngineError::BadStatus {
            engine: ENGINE,
            status: status.as_u16(),
        });
    }

    let bytes = response.bytes().await.map_err(|e| EngineError::Http {
        engine: ENGINE,
        source: e,
    })?;
    if bytes.len() > MAX_BODY_BYTES {
        return Err(EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("response body too large: {} bytes", bytes.len()),
        });
    }

    Ok(bytes.to_vec())
}

async fn lookup_by_id(
    client: &Client,
    id: &str,
    timeout: Duration,
) -> Result<Vec<SearchResult>, EngineError> {
    let url = format!("{DEFAULT_BASE_URL}/advisories/{id}.json");
    match fetch_json(client, &url, timeout).await {
        Ok(bytes) => {
            let advisory: RustSecAdvisory =
                serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
                    engine: ENGINE,
                    reason: format!("invalid JSON: {e}"),
                })?;
            Ok(vec![convert_to_result(&advisory)])
        }
        Err(EngineError::ParseFailed { reason, .. }) if reason == "not found" => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

async fn search_by_keyword(
    client: &Client,
    keyword: &str,
    timeout: Duration,
) -> Result<Vec<SearchResult>, EngineError> {
    let _ = keyword;
    let _ = (client, timeout);
    Ok(Vec::new())
}

fn convert_to_result(advisory: &RustSecAdvisory) -> SearchResult {
    let metadata = convert_to_metadata(advisory);
    let id_display = advisory.id.as_deref().unwrap_or("unknown");
    let title_text = advisory.title.as_deref().unwrap_or("RustSec Advisory");
    let title = format!("{id_display}: {title_text}");
    let url = advisory
        .url
        .as_deref()
        .unwrap_or("https://rustsec.org/advisories/")
        .to_string();
    let snippet = advisory.description.as_deref().map(|d| truncate(d, 500));

    SearchResult {
        title,
        url,
        snippet,
        source_engine: ENGINE.to_string(),
        metadata: ResultMetadata::Advisory(Box::new(metadata)),
    }
}

fn convert_to_metadata(advisory: &RustSecAdvisory) -> VulnerabilityMetadata {
    let rustsec_ids: Vec<String> = advisory
        .id
        .iter()
        .filter(|id| !id.is_empty())
        .cloned()
        .collect();

    let cve_ids: Vec<String> = advisory
        .aliases
        .iter()
        .filter(|a| a.starts_with("CVE-"))
        .cloned()
        .collect();
    let ghsa_ids: Vec<String> = advisory
        .aliases
        .iter()
        .filter(|a| a.starts_with("GHSA-"))
        .map(|a| a.to_uppercase())
        .collect();

    let cvss_score = advisory.cvss.as_ref().and_then(|c| c.score);
    let cvss_vector = advisory.cvss.as_ref().and_then(|c| c.vector_string.clone());

    let severity = cvss_score.map(|score| match score {
        9.0..=10.0 => SeverityLevel::Critical,
        7.0..=8.99 => SeverityLevel::High,
        4.0..=6.99 => SeverityLevel::Medium,
        0.1..=3.99 => SeverityLevel::Low,
        _ => SeverityLevel::Unknown,
    });

    let mut affected_ranges = Vec::new();
    let mut patched_versions = Vec::new();

    if let Some(ref pv) = advisory.patched_versions {
        if !pv.is_empty() {
            patched_versions.push(pv.clone());
        }
    }
    if let Some(ref uv) = advisory.unaffected_versions {
        if !uv.is_empty() {
            affected_ranges.push(format!("unaffected: {uv}"));
        }
    }

    let references: Vec<VulnerabilityReference> = advisory
        .references
        .iter()
        .filter_map(|r| {
            r.url.as_ref().map(|url| VulnerabilityReference {
                url: url.clone(),
                kind: r.name.clone(),
            })
        })
        .collect();

    VulnerabilityMetadata {
        cve_ids,
        ghsa_ids,
        osv_ids: Vec::new(),
        rustsec_ids,
        ecosystem: Some("crates.io".to_string()),
        package: advisory.package.clone(),
        affected_ranges,
        patched_ranges: Vec::new(),
        vulnerable_versions: Vec::new(),
        patched_versions,
        severity,
        cvss_score,
        cvss_vector,
        epss_score: None,
        kev: None,
        published_at: advisory.date.clone(),
        modified_at: None,
        withdrawn_at: None,
        references,
        source: VulnerabilitySource::Rustsec,
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let char_len = s.chars().count();
    if char_len <= max_chars {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max_chars).collect();
    match truncated.rfind(char::is_whitespace) {
        Some(pos) if pos > 0 => truncated[..pos].to_string(),
        _ => truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_query_type_rustsec() {
        assert!(matches!(
            parse_query_type("RUSTSEC-2024-0001"),
            QueryType::ById(_)
        ));
    }

    #[test]
    fn test_parse_query_type_keyword() {
        assert!(matches!(
            parse_query_type("serde deserialization"),
            QueryType::ByKeyword(_)
        ));
    }

    #[test]
    fn test_convert_to_metadata() {
        let advisory = RustSecAdvisory {
            id: Some("RUSTSEC-2024-0001".to_string()),
            package: Some("serde".to_string()),
            title: Some("Deserialization vulnerability".to_string()),
            description: Some("A deserialization issue".to_string()),
            date: Some("2024-01-15".to_string()),
            url: Some("https://rustsec.org/advisories/RUSTSEC-2024-0001.html".to_string()),
            aliases: vec![
                "CVE-2024-0001".to_string(),
                "GHSA-test-1234-abcd".to_string(),
            ],
            cvss: Some(RustSecCvss {
                score: Some(8.5),
                vector_string: Some("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:N/A:N".to_string()),
            }),
            patched_versions: Some(">= 1.2.3".to_string()),
            unaffected_versions: None,
            references: vec![RustSecReference {
                url: Some("https://example.com/advisory".to_string()),
                name: Some("advisory".to_string()),
            }],
        };
        let m = convert_to_metadata(&advisory);
        assert_eq!(m.rustsec_ids, vec!["RUSTSEC-2024-0001"]);
        assert_eq!(m.cve_ids, vec!["CVE-2024-0001"]);
        assert_eq!(m.ghsa_ids, vec!["GHSA-TEST-1234-ABCD"]);
        assert_eq!(m.cvss_score, Some(8.5));
        assert_eq!(m.severity, Some(SeverityLevel::High));
        assert_eq!(m.ecosystem.as_deref(), Some("crates.io"));
        assert_eq!(m.package.as_deref(), Some("serde"));
        assert_eq!(m.source, VulnerabilitySource::Rustsec);
    }

    #[test]
    fn test_provider_descriptor() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc = built_in_provider_descriptor("rustsec", true, false, true, false, None).unwrap();
        assert_eq!(desc.id, "rustsec");
        assert!(desc.capabilities.supports_security_search);
        assert!(desc.capabilities.supports_advisory_lookup_by_id);
        assert!(desc.capabilities.supports_advisory_lookup_by_package);
    }
}
