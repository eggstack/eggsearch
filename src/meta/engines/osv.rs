use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::{ResultMetadata, SearchResult};
use crate::core::security::{
    SeverityLevel, VulnerabilityMetadata, VulnerabilityReference, VulnerabilitySource,
};

const ENGINE: &str = "osv";
const DEFAULT_BASE_URL: &str = "https://api.osv.dev/v1";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const SNIPPET_MAX_CHARS: usize = 500;

#[derive(Debug, Deserialize)]
struct OsvQueryResponse {
    #[serde(default)]
    vulns: Vec<OsvVulnerability>,
}

#[derive(Debug, Deserialize)]
struct OsvVulnerability {
    #[serde(default)]
    id: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    details: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    severity: Vec<OsvSeverity>,
    #[serde(default)]
    affected: Vec<OsvAffected>,
    #[serde(default)]
    published: Option<String>,
    #[serde(default)]
    modified: Option<String>,
    #[serde(default)]
    withdrawn: Option<String>,
    #[serde(default)]
    references: Vec<OsvReference>,
}

#[derive(Debug, Deserialize)]
struct OsvSeverity {
    #[serde(default, rename = "type")]
    #[allow(dead_code)]
    severity_type: Option<String>,
    #[serde(default)]
    score: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OsvAffected {
    #[serde(default)]
    package: Option<OsvPackage>,
    #[serde(default)]
    ranges: Vec<OsvRange>,
    #[serde(default)]
    versions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct OsvPackage {
    #[serde(default)]
    name: String,
    #[serde(default)]
    ecosystem: String,
}

#[derive(Debug, Deserialize)]
struct OsvRange {
    #[serde(default, rename = "type")]
    #[allow(dead_code)]
    range_type: Option<String>,
    #[serde(default)]
    events: Vec<OsvEvent>,
}

#[derive(Debug, Deserialize)]
struct OsvEvent {
    #[serde(default)]
    introduced: Option<String>,
    #[serde(default)]
    fixed: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OsvReference {
    #[serde(default, rename = "type")]
    ref_type: Option<String>,
    #[serde(default)]
    url: String,
}

pub async fn search(
    client: &Client,
    query: &str,
    max_results: usize,
    timeout: Duration,
) -> Result<Vec<SearchResult>, EngineError> {
    if max_results == 0 {
        return Ok(Vec::new());
    }

    let url = format!("{DEFAULT_BASE_URL}/query");

    let body = serde_json::json!({
        "package": {
            "name": query,
        }
    });

    let response = tokio::time::timeout(
        timeout,
        client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send(),
    )
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

    let parsed: OsvQueryResponse =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;

    Ok(convert(parsed.vulns, max_results))
}

pub async fn lookup_by_id(
    client: &Client,
    vuln_id: &str,
    timeout: Duration,
) -> Result<Option<VulnerabilityMetadata>, EngineError> {
    let url = format!("{DEFAULT_BASE_URL}/vulns/{vuln_id}");

    let response = tokio::time::timeout(timeout, client.get(&url).send())
        .await
        .map_err(|_| EngineError::Timeout { engine: ENGINE })?
        .map_err(|e| EngineError::Http {
            engine: ENGINE,
            source: e,
        })?;

    let status = response.status();
    if status.as_u16() == 404 {
        return Ok(None);
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

    let vuln: OsvVulnerability =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;

    Ok(Some(convert_vuln_metadata(&vuln)))
}

/// Query OSV by package name, ecosystem, and optional version.
///
/// Builds a proper OSV `/v1/query` request body with explicit
/// `package.ecosystem`, `package.name`, and optional `version`.
/// Returns parsed `VulnerabilityMetadata` for each matching vulnerability.
pub async fn query_package(
    client: &Client,
    ecosystem: &str,
    package: &str,
    version: Option<&str>,
    max_results: usize,
    timeout: Duration,
) -> Result<Vec<VulnerabilityMetadata>, EngineError> {
    if max_results == 0 {
        return Ok(Vec::new());
    }

    let url = format!("{DEFAULT_BASE_URL}/query");

    let mut body = serde_json::json!({
        "package": {
            "ecosystem": ecosystem,
            "name": package,
        }
    });

    if let Some(v) = version {
        body["version"] = serde_json::Value::String(v.to_string());
    }

    let response = tokio::time::timeout(
        timeout,
        client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send(),
    )
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

    let parsed: OsvQueryResponse =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;

    let mut out = Vec::with_capacity(max_results.min(parsed.vulns.len()));
    for vuln in parsed.vulns {
        if out.len() >= max_results {
            break;
        }
        let id = vuln.id.clone();
        if id.is_empty() {
            continue;
        }
        out.push(convert_vuln_metadata(&vuln));
    }

    Ok(out)
}

fn convert(vulns: Vec<OsvVulnerability>, max_results: usize) -> Vec<SearchResult> {
    let mut out = Vec::with_capacity(max_results.min(vulns.len()));
    for vuln in vulns {
        if out.len() >= max_results {
            break;
        }
        let id = vuln.id.clone();
        if id.is_empty() {
            continue;
        }

        let summary = vuln.summary.as_deref().unwrap_or("OSV vulnerability entry");
        let title = format!("{id}: {summary}");

        let snippet = vuln
            .details
            .as_deref()
            .map(|d| truncate_body(d, SNIPPET_MAX_CHARS))
            .filter(|s| !s.is_empty());

        let url = format!("https://osv.dev/vulnerability/{id}");

        let metadata = ResultMetadata::Advisory(Box::new(convert_vuln_metadata(&vuln)));

        out.push(SearchResult {
            title,
            url,
            snippet,
            source_engine: ENGINE.to_string(),
            metadata,
        });
    }
    out
}

fn convert_vuln_metadata(vuln: &OsvVulnerability) -> VulnerabilityMetadata {
    let cve_ids: Vec<String> = vuln
        .aliases
        .iter()
        .filter(|a| a.starts_with("CVE-"))
        .cloned()
        .collect();
    let ghsa_ids: Vec<String> = vuln
        .aliases
        .iter()
        .filter(|a| a.starts_with("GHSA-"))
        .map(|a| a.to_uppercase())
        .collect();

    let severity = vuln
        .severity
        .first()
        .and_then(|s| s.score.as_deref())
        .and_then(|score| {
            // Try to parse CVSS score from the score string
            score
                .split('/')
                .find(|part| part.starts_with("CVSS:"))
                .map(|_| SeverityLevel::Unknown)
                .or_else(|| {
                    // Parse severity from score
                    let s = score.to_ascii_lowercase();
                    if s.contains("critical") {
                        Some(SeverityLevel::Critical)
                    } else if s.contains("high") {
                        Some(SeverityLevel::High)
                    } else if s.contains("medium") || s.contains("moderate") {
                        Some(SeverityLevel::Medium)
                    } else if s.contains("low") {
                        Some(SeverityLevel::Low)
                    } else {
                        None
                    }
                })
        });

    let mut affected_ranges = Vec::new();
    let mut patched_ranges = Vec::new();
    let mut vulnerable_versions = Vec::new();
    let mut patched_versions = Vec::new();
    let mut package_name = None;
    let mut ecosystem = None;

    for affected in &vuln.affected {
        if let Some(ref pkg) = affected.package {
            if package_name.is_none() {
                package_name = Some(pkg.name.clone());
            }
            if ecosystem.is_none() {
                ecosystem = Some(pkg.ecosystem.clone());
            }
        }
        for version in &affected.versions {
            if !vulnerable_versions.contains(version) {
                vulnerable_versions.push(version.clone());
            }
        }
        for range in &affected.ranges {
            for event in &range.events {
                if let Some(ref introduced) = event.introduced {
                    if introduced != "0" {
                        affected_ranges.push(format!(">={introduced}"));
                    }
                }
                if let Some(ref fixed) = event.fixed {
                    patched_ranges.push(format!("<{fixed}"));
                    if !vulnerable_versions.contains(fixed) {
                        patched_versions.push(fixed.clone());
                    }
                }
            }
        }
    }

    let references: Vec<VulnerabilityReference> = vuln
        .references
        .iter()
        .map(|r| VulnerabilityReference {
            url: r.url.clone(),
            kind: r.ref_type.clone(),
        })
        .collect();

    VulnerabilityMetadata {
        cve_ids,
        ghsa_ids,
        osv_ids: vec![vuln.id.clone()],
        rustsec_ids: Vec::new(),
        ecosystem,
        package: package_name,
        affected_ranges,
        patched_ranges,
        vulnerable_versions,
        patched_versions,
        severity,
        cvss_score: None,
        cvss_vector: None,
        epss_score: None,
        kev: None,
        published_at: vuln.published.clone(),
        modified_at: vuln.modified.clone(),
        withdrawn_at: vuln.withdrawn.clone(),
        references,
        source: VulnerabilitySource::Osv,
    }
}

fn truncate_body(body: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let body_char_len = body.chars().count();
    if body_char_len <= max_chars {
        return body.to_string();
    }
    let truncated: String = body.chars().take(max_chars).collect();
    match truncated.rfind(char::is_whitespace) {
        Some(pos) if pos > 0 => truncated[..pos].to_string(),
        _ => truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_extracts_results() {
        let vulns = vec![
            OsvVulnerability {
                id: "GHSA-test-1234-abcd".to_string(),
                summary: Some("Test vulnerability".to_string()),
                details: Some("Details about the vulnerability".to_string()),
                aliases: vec!["CVE-2024-0001".to_string()],
                severity: vec![OsvSeverity {
                    severity_type: Some("CVSS_V3".to_string()),
                    score: Some("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H".to_string()),
                }],
                affected: vec![OsvAffected {
                    package: Some(OsvPackage {
                        name: "test-package".to_string(),
                        ecosystem: "npm".to_string(),
                    }),
                    ranges: vec![OsvRange {
                        range_type: Some("SEMVER".to_string()),
                        events: vec![
                            OsvEvent {
                                introduced: Some("1.0.0".to_string()),
                                fixed: None,
                            },
                            OsvEvent {
                                introduced: None,
                                fixed: Some("1.2.3".to_string()),
                            },
                        ],
                    }],
                    versions: vec!["1.0.0".to_string(), "1.1.0".to_string()],
                }],
                published: Some("2024-01-15T10:00:00Z".to_string()),
                modified: Some("2024-01-20T12:00:00Z".to_string()),
                withdrawn: None,
                references: vec![OsvReference {
                    ref_type: Some("WEB".to_string()),
                    url: "https://example.com/advisory".to_string(),
                }],
            },
            OsvVulnerability {
                id: "GHSA-test-5678-efgh".to_string(),
                summary: None,
                details: None,
                aliases: vec![],
                severity: vec![],
                affected: vec![],
                published: None,
                modified: None,
                withdrawn: None,
                references: vec![],
            },
        ];
        let out = convert(vulns, 10);
        assert_eq!(out.len(), 2);
        assert!(out[0].title.contains("GHSA-test-1234-abcd"));
        assert!(out[0].title.contains("Test vulnerability"));
        assert_eq!(
            out[0].url,
            "https://osv.dev/vulnerability/GHSA-test-1234-abcd"
        );
        assert!(out[0].snippet.is_some());
        assert_eq!(out[0].source_engine, "osv");

        match &out[0].metadata {
            ResultMetadata::Advisory(m) => {
                assert_eq!(m.cve_ids, vec!["CVE-2024-0001"]);
                assert_eq!(m.osv_ids, vec!["GHSA-test-1234-abcd"]);
                assert_eq!(m.package.as_deref(), Some("test-package"));
                assert_eq!(m.ecosystem.as_deref(), Some("npm"));
                assert!(!m.affected_ranges.is_empty());
                assert!(!m.patched_ranges.is_empty());
                assert_eq!(m.vulnerable_versions, vec!["1.0.0", "1.1.0"]);
                assert_eq!(m.patched_versions, vec!["1.2.3"]);
                assert!(m.published_at.is_some());
                assert_eq!(m.references.len(), 1);
                assert_eq!(m.source, VulnerabilitySource::Osv);
            }
            other => panic!("expected Advisory metadata, got: {other:?}"),
        }
    }

    #[test]
    fn test_convert_respects_max_results() {
        let vulns: Vec<OsvVulnerability> = (0..5)
            .map(|i| OsvVulnerability {
                id: format!("GHSA-{i:04}-test"),
                summary: None,
                details: None,
                aliases: vec![],
                severity: vec![],
                affected: vec![],
                published: None,
                modified: None,
                withdrawn: None,
                references: vec![],
            })
            .collect();
        let out = convert(vulns, 2);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn test_convert_skips_empty_id() {
        let vulns = vec![OsvVulnerability {
            id: String::new(),
            summary: None,
            details: None,
            aliases: vec![],
            severity: vec![],
            affected: vec![],
            published: None,
            modified: None,
            withdrawn: None,
            references: vec![],
        }];
        let out = convert(vulns, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_truncate_body_short() {
        assert_eq!(truncate_body("hello", 100), "hello");
    }

    #[test]
    fn test_truncate_body_at_word_boundary() {
        assert_eq!(truncate_body("hello world foo bar", 11), "hello");
    }

    #[test]
    fn test_truncate_body_zero_max() {
        assert_eq!(truncate_body("anything", 0), "");
    }

    #[test]
    fn test_convert_vuln_metadata_extracts_aliases() {
        let vuln = OsvVulnerability {
            id: "GHSA-test-1234-abcd".to_string(),
            summary: None,
            details: None,
            aliases: vec![
                "CVE-2024-0001".to_string(),
                "CVE-2024-0002".to_string(),
                "GHSA-other-1234-abcd".to_string(),
            ],
            severity: vec![],
            affected: vec![],
            published: None,
            modified: None,
            withdrawn: None,
            references: vec![],
        };
        let m = convert_vuln_metadata(&vuln);
        assert_eq!(m.cve_ids, vec!["CVE-2024-0001", "CVE-2024-0002"]);
        assert_eq!(m.ghsa_ids, vec!["GHSA-OTHER-1234-ABCD"]);
        assert_eq!(m.osv_ids, vec!["GHSA-test-1234-abcd"]);
    }

    #[test]
    fn test_convert_vuln_metadata_severity_parsing() {
        let vuln = OsvVulnerability {
            id: "test".to_string(),
            summary: None,
            details: None,
            aliases: vec![],
            severity: vec![OsvSeverity {
                severity_type: Some("CVSS_V3".to_string()),
                score: Some("HIGH".to_string()),
            }],
            affected: vec![],
            published: None,
            modified: None,
            withdrawn: None,
            references: vec![],
        };
        let m = convert_vuln_metadata(&vuln);
        assert_eq!(m.severity, Some(SeverityLevel::High));
    }

    #[test]
    fn test_convert_vuln_metadata_critical_severity() {
        let vuln = OsvVulnerability {
            id: "test".to_string(),
            summary: None,
            details: None,
            aliases: vec![],
            severity: vec![OsvSeverity {
                severity_type: Some("CVSS_V3".to_string()),
                score: Some("CRITICAL".to_string()),
            }],
            affected: vec![],
            published: None,
            modified: None,
            withdrawn: None,
            references: vec![],
        };
        let m = convert_vuln_metadata(&vuln);
        assert_eq!(m.severity, Some(SeverityLevel::Critical));
    }

    #[tokio::test]
    async fn test_osv_provider_descriptor() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc = built_in_provider_descriptor("osv", true, false, true).unwrap();
        assert_eq!(desc.id, "osv");
        assert_eq!(desc.display_name, "OSV (Open Source Vulnerabilities)");
        assert_eq!(desc.kind, crate::core::provider::ProviderKind::JsonApi);
        assert!(!desc.requires_api_key);
        assert!(desc.configured);
        assert!(desc.enabled);
        assert!(!desc.default);
        assert!(desc.capabilities.supports_security_search);
        assert!(!desc.capabilities.supports_code_search);
        assert!(!desc.capabilities.supports_issue_search);
    }
}
