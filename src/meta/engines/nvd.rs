use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::{ResultMetadata, SearchResult};
use crate::core::security::{
    SeverityLevel, VulnerabilityMetadata, VulnerabilityReference, VulnerabilitySource,
};

const ENGINE: &str = "nvd";
const DEFAULT_BASE_URL: &str = "https://services.nvd.nist.gov/rest/json/cves/2.0";
const MAX_BODY_BYTES: usize = 5 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct NvdResponse {
    #[serde(default)]
    vulnerabilities: Vec<NvdVulnerability>,
}

#[derive(Debug, Deserialize)]
struct NvdVulnerability {
    #[serde(default)]
    cve: Option<NvdCve>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct NvdCve {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    published: Option<String>,
    #[serde(default)]
    lastModified: Option<String>,
    #[serde(default)]
    descriptions: Vec<NvdDescription>,
    #[serde(default)]
    metrics: Option<NvdMetrics>,
    #[serde(default)]
    configurations: Vec<NvdConfiguration>,
    #[serde(default)]
    references: Vec<NvdReference>,
}

#[derive(Debug, Deserialize)]
struct NvdDescription {
    #[serde(default)]
    lang: Option<String>,
    #[serde(default)]
    value: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct NvdMetrics {
    #[serde(default)]
    cvssMetricV31: Vec<NvdCvssMetric>,
    #[serde(default)]
    cvssMetricV30: Vec<NvdCvssMetric>,
    #[serde(default)]
    cvssMetricV2: Vec<NvdCvssMetric>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct NvdCvssMetric {
    #[serde(default)]
    cvssData: Option<NvdCvssData>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct NvdCvssData {
    #[serde(default)]
    baseScore: Option<f64>,
    #[serde(default)]
    vectorString: Option<String>,
    #[serde(default)]
    baseSeverity: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NvdConfiguration {
    #[serde(default)]
    nodes: Vec<NvdNode>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct NvdNode {
    #[serde(default)]
    cpeMatch: Vec<NvdCpeMatch>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct NvdCpeMatch {
    #[serde(default)]
    criteria: Option<String>,
    #[serde(default)]
    versionStartIncluding: Option<String>,
    #[serde(default)]
    versionEndExcluding: Option<String>,
    #[serde(default)]
    vulnerable: bool,
}

#[derive(Debug, Deserialize)]
struct NvdReference {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    source: Option<String>,
}

pub struct NvdEngine {
    pub client: Client,
    pub api_key: Option<String>,
}

impl super::SearchEngine for NvdEngine {
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
            let results =
                keyword_search(&self.client, self.api_key.as_deref(), query, timeout).await?;
            Ok(results.into_iter().take(max_results).collect())
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
            if !upper.starts_with("CVE-") {
                return Ok(None);
            }
            let results =
                lookup_by_cve(&self.client, self.api_key.as_deref(), &upper, timeout).await?;
            Ok(results.into_iter().next())
        })
    }
}

async fn fetch_json(
    client: &Client,
    api_key: Option<&str>,
    url: &str,
    timeout: Duration,
) -> Result<Vec<u8>, EngineError> {
    let mut builder = client.get(url);
    if let Some(key) = api_key {
        if !key.is_empty() {
            builder = builder.header("apiKey", key);
        }
    }

    let response = tokio::time::timeout(timeout, builder.send())
        .await
        .map_err(|_| EngineError::Timeout { engine: ENGINE })?
        .map_err(|e| EngineError::Http {
            engine: ENGINE,
            source: e,
        })?;

    let status = response.status();
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

async fn lookup_by_cve(
    client: &Client,
    api_key: Option<&str>,
    cve_id: &str,
    timeout: Duration,
) -> Result<Vec<VulnerabilityMetadata>, EngineError> {
    let url = format!("{DEFAULT_BASE_URL}?cveId={cve_id}");
    let bytes = fetch_json(client, api_key, &url, timeout).await?;
    let parsed: NvdResponse =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;
    Ok(parsed
        .vulnerabilities
        .into_iter()
        .filter_map(|v| v.cve.map(|cve| convert_cve(&cve)))
        .collect())
}

async fn keyword_search(
    client: &Client,
    api_key: Option<&str>,
    query: &str,
    timeout: Duration,
) -> Result<Vec<SearchResult>, EngineError> {
    let encoded = urlencoding::encode(query);
    let url = format!("{DEFAULT_BASE_URL}?keywordSearch={encoded}");
    let bytes = fetch_json(client, api_key, &url, timeout).await?;
    let parsed: NvdResponse =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;
    Ok(parsed
        .vulnerabilities
        .into_iter()
        .filter_map(|v| v.cve.map(|cve| convert_to_result(&cve)))
        .collect())
}

fn convert_to_result(cve: &NvdCve) -> SearchResult {
    let metadata = convert_cve(cve);
    let id_display = cve.id.as_deref().unwrap_or("unknown");
    let description = cve
        .descriptions
        .iter()
        .find(|d| d.lang.as_deref() == Some("en"))
        .and_then(|d| d.value.as_deref())
        .unwrap_or("NVD CVE entry");
    let title = format!("{id_display}: {description}");
    let url = format!("https://nvd.nist.gov/vuln/detail/{id_display}");
    let snippet = cve
        .descriptions
        .iter()
        .find(|d| d.lang.as_deref() == Some("en"))
        .and_then(|d| d.value.as_deref())
        .map(|d| truncate(d, 500));

    SearchResult {
        title,
        url,
        snippet,
        source_engine: ENGINE.to_string(),
        metadata: ResultMetadata::Advisory(Box::new(metadata)),
    }
}

fn convert_cve(cve: &NvdCve) -> VulnerabilityMetadata {
    let cve_ids: Vec<String> = cve.id.iter().filter(|id| !id.is_empty()).cloned().collect();

    let (cvss_score, cvss_vector, severity) = extract_cvss(cve);

    let mut affected_ranges = Vec::new();
    let mut package_name = None;
    let mut ecosystem = None;

    for config in &cve.configurations {
        for node in &config.nodes {
            for cpe in &node.cpeMatch {
                if !cpe.vulnerable {
                    continue;
                }
                if let Some(ref criteria) = cpe.criteria {
                    if let Some((eco, pkg)) = parse_cpe(criteria) {
                        if ecosystem.is_none() {
                            ecosystem = Some(eco);
                        }
                        if package_name.is_none() {
                            package_name = Some(pkg);
                        }
                    }
                }
                let range = build_version_range(cpe);
                if !range.is_empty() && !affected_ranges.contains(&range) {
                    affected_ranges.push(range);
                }
            }
        }
    }

    let references: Vec<VulnerabilityReference> = cve
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
        ghsa_ids: Vec::new(),
        osv_ids: Vec::new(),
        rustsec_ids: Vec::new(),
        ecosystem,
        package: package_name,
        affected_ranges,
        patched_ranges: Vec::new(),
        vulnerable_versions: Vec::new(),
        patched_versions: Vec::new(),
        severity,
        cvss_score,
        cvss_vector,
        epss_score: None,
        kev: None,
        published_at: cve.published.clone(),
        modified_at: cve.lastModified.clone(),
        withdrawn_at: None,
        references,
        source: VulnerabilitySource::Nvd,
    }
}

fn extract_cvss(cve: &NvdCve) -> (Option<f64>, Option<String>, Option<SeverityLevel>) {
    let metrics = match &cve.metrics {
        Some(m) => m,
        None => return (None, None, None),
    };

    let metric = metrics
        .cvssMetricV31
        .iter()
        .chain(metrics.cvssMetricV30.iter())
        .chain(metrics.cvssMetricV2.iter())
        .next();

    let data = match metric.and_then(|m| m.cvssData.as_ref()) {
        Some(d) => d,
        None => return (None, None, None),
    };

    let severity = data.baseSeverity.as_ref().and_then(|s| {
        let s_lower = s.to_ascii_lowercase();
        match s_lower.as_str() {
            "critical" => Some(SeverityLevel::Critical),
            "high" => Some(SeverityLevel::High),
            "medium" => Some(SeverityLevel::Medium),
            "low" => Some(SeverityLevel::Low),
            _ => None,
        }
    });

    let severity = severity.or_else(|| {
        data.baseScore.map(|score| match score {
            9.0..=10.0 => SeverityLevel::Critical,
            7.0..=8.99 => SeverityLevel::High,
            4.0..=6.99 => SeverityLevel::Medium,
            0.1..=3.99 => SeverityLevel::Low,
            _ => SeverityLevel::Unknown,
        })
    });

    (data.baseScore, data.vectorString.clone(), severity)
}

fn parse_cpe(criteria: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = criteria.split(':').collect();
    if parts.len() >= 5 {
        let vendor = parts[3];
        let ecosystem = match vendor {
            "microsoft" => "microsoft",
            "linux" => "linux",
            "php" => "php",
            "python" => "pypi",
            "nodejs" => "npm",
            other => other,
        };
        let package = parts[4];
        if !package.is_empty() && package != "*" {
            return Some((ecosystem.to_string(), package.to_string()));
        }
    }
    None
}

fn build_version_range(cpe: &NvdCpeMatch) -> String {
    match (
        cpe.versionStartIncluding.as_ref(),
        cpe.versionEndExcluding.as_ref(),
    ) {
        (Some(start), Some(end)) => format!(">= {start}, < {end}"),
        (Some(start), None) => format!(">= {start}"),
        (None, Some(end)) => format!("< {end}"),
        _ => String::new(),
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
    fn test_parse_cpe() {
        assert_eq!(
            parse_cpe("cpe:2.3:a:nginx:nginx:1.0.0:*:*:*:*:*:*:*"),
            Some(("nginx".to_string(), "nginx".to_string()))
        );
    }

    #[test]
    fn test_parse_cpe_python() {
        assert_eq!(
            parse_cpe("cpe:2.3:a:python:requests:2.0.0:*:*:*:*:*:*:*"),
            Some(("pypi".to_string(), "requests".to_string()))
        );
    }

    #[test]
    fn test_build_version_range() {
        let cpe = NvdCpeMatch {
            criteria: None,
            versionStartIncluding: Some("1.0.0".to_string()),
            versionEndExcluding: Some("2.0.0".to_string()),
            vulnerable: true,
        };
        assert_eq!(build_version_range(&cpe), ">= 1.0.0, < 2.0.0");
    }

    #[test]
    fn test_build_version_range_start_only() {
        let cpe = NvdCpeMatch {
            criteria: None,
            versionStartIncluding: Some("1.0.0".to_string()),
            versionEndExcluding: None,
            vulnerable: true,
        };
        assert_eq!(build_version_range(&cpe), ">= 1.0.0");
    }

    #[test]
    fn test_build_version_range_end_only() {
        let cpe = NvdCpeMatch {
            criteria: None,
            versionStartIncluding: None,
            versionEndExcluding: Some("2.0.0".to_string()),
            vulnerable: true,
        };
        assert_eq!(build_version_range(&cpe), "< 2.0.0");
    }

    #[test]
    fn test_provider_descriptor() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc = built_in_provider_descriptor("nvd", true, false, true, false, None).unwrap();
        assert_eq!(desc.id, "nvd");
        assert!(desc.capabilities.supports_security_search);
        assert!(desc.capabilities.supports_advisory_lookup_by_id);
        assert!(desc.capabilities.supports_freshness);
        assert!(!desc.capabilities.supports_advisory_lookup_by_package);
    }

    #[test]
    fn test_convert_cve_metadata() {
        let cve = NvdCve {
            id: Some("CVE-2024-12345".to_string()),
            published: Some("2024-01-15T10:00:00.000".to_string()),
            lastModified: Some("2024-01-20T12:00:00.000".to_string()),
            descriptions: vec![NvdDescription {
                lang: Some("en".to_string()),
                value: Some("A test vulnerability".to_string()),
            }],
            metrics: Some(NvdMetrics {
                cvssMetricV31: vec![NvdCvssMetric {
                    cvssData: Some(NvdCvssData {
                        baseScore: Some(7.5),
                        vectorString: Some(
                            "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:N/A:N".to_string(),
                        ),
                        baseSeverity: Some("HIGH".to_string()),
                    }),
                }],
                cvssMetricV30: vec![],
                cvssMetricV2: vec![],
            }),
            configurations: vec![],
            references: vec![],
        };
        let m = convert_cve(&cve);
        assert_eq!(m.cve_ids, vec!["CVE-2024-12345"]);
        assert_eq!(m.severity, Some(SeverityLevel::High));
        assert_eq!(m.cvss_score, Some(7.5));
        assert!(m.cvss_vector.is_some());
        assert_eq!(m.source, VulnerabilitySource::Nvd);
    }
}
