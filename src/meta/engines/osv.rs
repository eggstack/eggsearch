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
static RUSTSEC_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(RUSTSEC-\d{4}-\d{4,})\b").unwrap());
static PACKAGE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(package|crate|pypi|npm):([a-zA-Z0-9_\-\.]+)\b").unwrap());
static ECOSYSTEM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(ecosystem):([a-zA-Z0-9_\-\.]+)\b").unwrap());
static VERSION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(version):([0-9]+[a-zA-Z0-9_\-\.]*)\b").unwrap());

#[derive(Debug, Clone, PartialEq, Eq)]
enum OsvQuery {
    ById(String),
    ByPackage {
        ecosystem: String,
        package: String,
        version: Option<String>,
    },
    Unstructured,
}

fn parse_osv_query(query: &str) -> OsvQuery {
    if let Some(cap) = CVE_RE.captures(query) {
        return OsvQuery::ById(cap[1].to_uppercase());
    }
    if let Some(cap) = GHSA_RE.captures(query) {
        return OsvQuery::ById(cap[1].to_uppercase());
    }
    if let Some(cap) = RUSTSEC_RE.captures(query) {
        return OsvQuery::ById(cap[1].to_uppercase());
    }

    if let Some(cap) = PACKAGE_RE.captures(query) {
        let prefix = cap[1].to_ascii_lowercase();
        let name = cap[2].to_string();
        let mut ecosystem = match prefix.as_str() {
            "crate" => Some("crates.io".to_string()),
            "npm" => Some("npm".to_string()),
            "pypi" => Some("PyPI".to_string()),
            _ => None,
        };
        let mut version = None;

        if let Some(eco_cap) = ECOSYSTEM_RE.captures(query) {
            ecosystem = Some(eco_cap[2].to_string());
        }
        if let Some(ver_cap) = VERSION_RE.captures(query) {
            version = Some(ver_cap[2].to_string());
        }

        if let Some(eco) = ecosystem {
            return OsvQuery::ByPackage {
                ecosystem: eco,
                package: name,
                version,
            };
        }
    }

    OsvQuery::Unstructured
}

const ENGINE: &str = "osv";
const DEFAULT_BASE_URL: &str = "https://api.osv.dev/v1";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
#[allow(dead_code)]
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
    #[allow(dead_code)]
    summary: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
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
    #[serde(default)]
    last_affected: Option<String>,
    #[serde(default)]
    limit: Option<String>,
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

    match parse_osv_query(query) {
        OsvQuery::ById(id) => {
            let metadata = lookup_by_id(client, &id, timeout).await?;
            match metadata {
                Some(m) => {
                    let id_display = m
                        .osv_ids
                        .first()
                        .or(m.cve_ids.first())
                        .or(m.ghsa_ids.first())
                        .cloned()
                        .unwrap_or_else(|| id.clone());
                    let summary = m
                        .references
                        .first()
                        .map(|r| r.url.as_str())
                        .unwrap_or("OSV vulnerability entry");
                    let title = format!("{id_display}: {summary}");
                    let url = format!("https://osv.dev/vulnerability/{id_display}");
                    let snippet = m
                        .affected_ranges
                        .first()
                        .map(|r| format!("Affected: {r}"))
                        .filter(|s| !s.is_empty());
                    Ok(vec![SearchResult {
                        title,
                        url,
                        snippet,
                        source_engine: ENGINE.to_string(),
                        metadata: ResultMetadata::Advisory(Box::new(m)),
                    }])
                }
                None => Ok(Vec::new()),
            }
        }
        OsvQuery::ByPackage {
            ecosystem,
            package,
            version,
        } => {
            let vulns = query_package(
                client,
                &ecosystem,
                &package,
                version.as_deref(),
                max_results,
                timeout,
            )
            .await?;
            let mut out = Vec::with_capacity(max_results.min(vulns.len()));
            for m in vulns {
                if out.len() >= max_results {
                    break;
                }
                let id_display = m
                    .osv_ids
                    .first()
                    .or(m.cve_ids.first())
                    .or(m.ghsa_ids.first())
                    .cloned()
                    .unwrap_or_else(|| package.clone());
                let summary = m
                    .references
                    .first()
                    .map(|r| r.url.as_str())
                    .unwrap_or("OSV vulnerability entry");
                let title = format!("{id_display}: {summary}");
                let url = format!("https://osv.dev/vulnerability/{id_display}");
                let snippet = m
                    .affected_ranges
                    .first()
                    .map(|r| format!("Affected: {r}"))
                    .filter(|s| !s.is_empty());
                out.push(SearchResult {
                    title,
                    url,
                    snippet,
                    source_engine: ENGINE.to_string(),
                    metadata: ResultMetadata::Advisory(Box::new(m)),
                });
            }
            Ok(out)
        }
        OsvQuery::Unstructured => Ok(Vec::new()),
    }
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

#[allow(dead_code)]
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

    let first_score = vuln.severity.iter().find_map(|s| s.score.as_deref());

    let cvss_vector = first_score.and_then(|score| {
        if score.contains("CVSS:") {
            Some(score.to_string())
        } else {
            None
        }
    });

    let cvss_score = first_score.and_then(|score| {
        score.parse::<f64>().ok().or_else(|| {
            score
                .split_whitespace()
                .find_map(|part| part.parse::<f64>().ok())
        })
    });

    let severity = first_score
        .and_then(|score| {
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
        .or_else(|| {
            cvss_score.map(|s| match s {
                9.0..=10.0 => SeverityLevel::Critical,
                7.0..=8.99 => SeverityLevel::High,
                4.0..=6.99 => SeverityLevel::Medium,
                0.1..=3.99 => SeverityLevel::Low,
                _ => SeverityLevel::Unknown,
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
                if let Some(ref last_affected) = event.last_affected {
                    if !vulnerable_versions.contains(last_affected) {
                        vulnerable_versions.push(last_affected.clone());
                    }
                }
                if let Some(ref limit) = event.limit {
                    // limit events mark the end of the affected range
                    // but the version itself is not affected
                    patched_ranges.push(format!("<={limit}"));
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
        cvss_score,
        cvss_vector,
        epss_score: None,
        kev: None,
        published_at: vuln.published.clone(),
        modified_at: vuln.modified.clone(),
        withdrawn_at: vuln.withdrawn.clone(),
        references,
        source: VulnerabilitySource::Osv,
    }
}

#[allow(dead_code)]
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
                                last_affected: None,
                                limit: None,
                            },
                            OsvEvent {
                                introduced: None,
                                fixed: Some("1.2.3".to_string()),
                                last_affected: None,
                                limit: None,
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

    #[test]
    fn test_cvss_vector_preserved() {
        let vuln = OsvVulnerability {
            id: "test".to_string(),
            summary: None,
            details: None,
            aliases: vec![],
            severity: vec![OsvSeverity {
                severity_type: Some("CVSS_V3".to_string()),
                score: Some("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H".to_string()),
            }],
            affected: vec![],
            published: None,
            modified: None,
            withdrawn: None,
            references: vec![],
        };
        let m = convert_vuln_metadata(&vuln);
        assert_eq!(
            m.cvss_vector.as_deref(),
            Some("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H")
        );
    }

    #[test]
    fn test_numeric_score_maps_to_cvss_score_and_severity() {
        let vuln = OsvVulnerability {
            id: "test".to_string(),
            summary: None,
            details: None,
            aliases: vec![],
            severity: vec![OsvSeverity {
                severity_type: Some("CVSS_V3".to_string()),
                score: Some("7.5".to_string()),
            }],
            affected: vec![],
            published: None,
            modified: None,
            withdrawn: None,
            references: vec![],
        };
        let m = convert_vuln_metadata(&vuln);
        assert_eq!(m.cvss_score, Some(7.5));
        assert_eq!(m.severity, Some(SeverityLevel::High));
    }

    #[test]
    fn test_empty_severity_no_fabricated_data() {
        let vuln = OsvVulnerability {
            id: "test".to_string(),
            summary: None,
            details: None,
            aliases: vec![],
            severity: vec![],
            affected: vec![],
            published: None,
            modified: None,
            withdrawn: None,
            references: vec![],
        };
        let m = convert_vuln_metadata(&vuln);
        assert_eq!(m.severity, None);
        assert_eq!(m.cvss_score, None);
        assert_eq!(m.cvss_vector, None);
    }

    #[test]
    fn test_text_severity_still_works() {
        let vuln = OsvVulnerability {
            id: "test".to_string(),
            summary: None,
            details: None,
            aliases: vec![],
            severity: vec![OsvSeverity {
                severity_type: Some("CVSS_V3".to_string()),
                score: Some("MEDIUM".to_string()),
            }],
            affected: vec![],
            published: None,
            modified: None,
            withdrawn: None,
            references: vec![],
        };
        let m = convert_vuln_metadata(&vuln);
        assert_eq!(m.severity, Some(SeverityLevel::Medium));
        assert_eq!(m.cvss_score, None);
        assert_eq!(m.cvss_vector, None);
    }

    #[test]
    fn test_cvss_score_with_vector_and_numeric() {
        let vuln = OsvVulnerability {
            id: "test".to_string(),
            summary: None,
            details: None,
            aliases: vec![],
            severity: vec![OsvSeverity {
                severity_type: Some("CVSS_V3".to_string()),
                score: Some("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H 9.8".to_string()),
            }],
            affected: vec![],
            published: None,
            modified: None,
            withdrawn: None,
            references: vec![],
        };
        let m = convert_vuln_metadata(&vuln);
        assert_eq!(
            m.cvss_vector.as_deref(),
            Some("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H 9.8")
        );
        assert_eq!(m.cvss_score, Some(9.8));
        assert_eq!(m.severity, Some(SeverityLevel::Critical));
    }

    #[tokio::test]
    async fn test_osv_provider_descriptor() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc = built_in_provider_descriptor("osv", true, false, true, false, None).unwrap();
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

    #[test]
    fn test_parse_osv_query_unstructured_prose() {
        assert_eq!(
            parse_osv_query("how to fix sql injection"),
            OsvQuery::Unstructured
        );
    }

    #[test]
    fn test_parse_osv_query_unstructured_plain_words() {
        assert_eq!(
            parse_osv_query("serde deserialization vulnerability"),
            OsvQuery::Unstructured
        );
    }

    #[test]
    fn test_parse_osv_query_cve_id() {
        assert_eq!(
            parse_osv_query("CVE-2024-12345"),
            OsvQuery::ById("CVE-2024-12345".to_string())
        );
    }

    #[test]
    fn test_parse_osv_query_cve_id_case_insensitive() {
        assert_eq!(
            parse_osv_query("cve-2024-12345"),
            OsvQuery::ById("CVE-2024-12345".to_string())
        );
    }

    #[test]
    fn test_parse_osv_query_ghsa_id() {
        assert_eq!(
            parse_osv_query("GHSA-xxxx-xxxx-xxxx"),
            OsvQuery::ById("GHSA-XXXX-XXXX-XXXX".to_string())
        );
    }

    #[test]
    fn test_parse_osv_query_rustsec_id() {
        assert_eq!(
            parse_osv_query("RUSTSEC-2024-0001"),
            OsvQuery::ById("RUSTSEC-2024-0001".to_string())
        );
    }

    #[test]
    fn test_parse_osv_query_crate_hint_infers_ecosystem() {
        assert_eq!(
            parse_osv_query("crate:serde"),
            OsvQuery::ByPackage {
                ecosystem: "crates.io".to_string(),
                package: "serde".to_string(),
                version: None,
            }
        );
    }

    #[test]
    fn test_parse_osv_query_npm_hint_infers_ecosystem() {
        assert_eq!(
            parse_osv_query("npm:express"),
            OsvQuery::ByPackage {
                ecosystem: "npm".to_string(),
                package: "express".to_string(),
                version: None,
            }
        );
    }

    #[test]
    fn test_parse_osv_query_pypi_hint_infers_ecosystem() {
        assert_eq!(
            parse_osv_query("pypi:requests"),
            OsvQuery::ByPackage {
                ecosystem: "PyPI".to_string(),
                package: "requests".to_string(),
                version: None,
            }
        );
    }

    #[test]
    fn test_parse_osv_query_package_with_explicit_ecosystem() {
        assert_eq!(
            parse_osv_query("package:serde ecosystem:crates.io"),
            OsvQuery::ByPackage {
                ecosystem: "crates.io".to_string(),
                package: "serde".to_string(),
                version: None,
            }
        );
    }

    #[test]
    fn test_parse_osv_query_package_with_version() {
        assert_eq!(
            parse_osv_query("package:serde ecosystem:crates.io version:1.0.0"),
            OsvQuery::ByPackage {
                ecosystem: "crates.io".to_string(),
                package: "serde".to_string(),
                version: Some("1.0.0".to_string()),
            }
        );
    }

    #[test]
    fn test_parse_osv_query_crate_with_version() {
        assert_eq!(
            parse_osv_query("crate:tokio version:1.0"),
            OsvQuery::ByPackage {
                ecosystem: "crates.io".to_string(),
                package: "tokio".to_string(),
                version: Some("1.0".to_string()),
            }
        );
    }

    #[test]
    fn test_parse_osv_query_cve_in_prose() {
        assert_eq!(
            parse_osv_query("details about CVE-2024-12345 and its impact"),
            OsvQuery::ById("CVE-2024-12345".to_string())
        );
    }

    #[test]
    fn test_parse_osv_query_explicit_ecosystem_overrides_prefix() {
        assert_eq!(
            parse_osv_query("crate:serde ecosystem:npm"),
            OsvQuery::ByPackage {
                ecosystem: "npm".to_string(),
                package: "serde".to_string(),
                version: None,
            }
        );
    }

    #[test]
    fn test_parse_osv_query_empty_string() {
        assert_eq!(parse_osv_query(""), OsvQuery::Unstructured);
    }
}
