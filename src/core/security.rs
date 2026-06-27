//! Security advisory types for the `security_search` tool.
//!
//! This module defines the request/response types, vulnerability
//! metadata model, identifier parser, and grouping kinds for the
//! security-oriented retrieval layer.

#![allow(missing_docs)]

use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::core::query::Freshness;
use crate::core::result::SearchWarning;
use crate::core::sanitize::TrustMarkers;
use crate::core::source_card::SourceCard;
use crate::meta::response::ProviderFailure;

/// Severity level from advisory databases.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SeverityLevel {
    Critical,
    High,
    Medium,
    Low,
    #[default]
    Unknown,
}

impl SeverityLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_str_loose(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "critical" | "crit" => Self::Critical,
            "high" | "important" => Self::High,
            "medium" | "moderate" | "med" => Self::Medium,
            "low" | "minor" => Self::Low,
            _ => Self::Unknown,
        }
    }
}

/// Reference to an external vulnerability resource.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct VulnerabilityReference {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

/// Normalized vulnerability metadata from advisory databases.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct VulnerabilityMetadata {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cve_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ghsa_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub osv_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rustsec_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ecosystem: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_ranges: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub patched_ranges: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vulnerable_versions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub patched_versions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<SeverityLevel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cvss_score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cvss_vector: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epss_score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kev: Option<KevMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub withdrawn_at: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<VulnerabilityReference>,
    pub source: VulnerabilitySource,
}

impl VulnerabilityMetadata {
    pub fn merge(self, other: VulnerabilityMetadata) -> VulnerabilityMetadata {
        let mut cve_ids = self.cve_ids;
        for id in &other.cve_ids {
            if !cve_ids.contains(id) {
                cve_ids.push(id.clone());
            }
        }
        let mut ghsa_ids = self.ghsa_ids;
        for id in &other.ghsa_ids {
            if !ghsa_ids.contains(id) {
                ghsa_ids.push(id.clone());
            }
        }
        let mut osv_ids = self.osv_ids;
        for id in &other.osv_ids {
            if !osv_ids.contains(id) {
                osv_ids.push(id.clone());
            }
        }
        let mut rustsec_ids = self.rustsec_ids;
        for id in &other.rustsec_ids {
            if !rustsec_ids.contains(id) {
                rustsec_ids.push(id.clone());
            }
        }
        let mut affected_ranges = self.affected_ranges;
        for r in &other.affected_ranges {
            if !affected_ranges.contains(r) {
                affected_ranges.push(r.clone());
            }
        }
        let mut patched_ranges = self.patched_ranges;
        for r in &other.patched_ranges {
            if !patched_ranges.contains(r) {
                patched_ranges.push(r.clone());
            }
        }
        let mut vulnerable_versions = self.vulnerable_versions;
        for v in &other.vulnerable_versions {
            if !vulnerable_versions.contains(v) {
                vulnerable_versions.push(v.clone());
            }
        }
        let mut patched_versions = self.patched_versions;
        for v in &other.patched_versions {
            if !patched_versions.contains(v) {
                patched_versions.push(v.clone());
            }
        }
        let mut references = self.references;
        for r in &other.references {
            if !references.iter().any(|existing| existing.url == r.url) {
                references.push(r.clone());
            }
        }
        VulnerabilityMetadata {
            cve_ids,
            ghsa_ids,
            osv_ids,
            rustsec_ids,
            ecosystem: self.ecosystem.or(other.ecosystem),
            package: self.package.or(other.package),
            affected_ranges,
            patched_ranges,
            vulnerable_versions,
            patched_versions,
            severity: self.severity.or(other.severity),
            cvss_score: self.cvss_score.or(other.cvss_score),
            cvss_vector: self.cvss_vector.or(other.cvss_vector),
            epss_score: self.epss_score.or(other.epss_score),
            kev: self.kev.or(other.kev),
            published_at: self.published_at.or(other.published_at),
            modified_at: self.modified_at.or(other.modified_at),
            withdrawn_at: self.withdrawn_at.or(other.withdrawn_at),
            references,
            source: self.source,
        }
    }
}

/// CISA Known Exploited Vulnerabilities metadata.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct KevMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    pub known_ransomware_usage: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_date: Option<String>,
}

/// Which advisory source produced the vulnerability metadata.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VulnerabilitySource {
    Osv,
    GithubAdvisory,
    Nvd,
    Rustsec,
    CisaKev,
    #[default]
    Generic,
}

impl VulnerabilitySource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Osv => "osv",
            Self::GithubAdvisory => "github_advisory",
            Self::Nvd => "nvd",
            Self::Rustsec => "rustsec",
            Self::CisaKev => "cisa_kev",
            Self::Generic => "generic",
        }
    }
}

/// Parsed security identifiers extracted from request fields and
/// free-text query.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SecurityIdentifiers {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cve_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ghsa_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub osv_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rustsec_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ecosystem: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub residual_query: String,
}

impl SecurityIdentifiers {
    pub fn has_strong_identifier(&self) -> bool {
        !self.cve_ids.is_empty()
            || !self.ghsa_ids.is_empty()
            || !self.osv_ids.is_empty()
            || !self.rustsec_ids.is_empty()
            || (self.package.is_some() && self.ecosystem.is_some())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn parse(
        query: &str,
        cve_id: Option<&str>,
        ghsa_id: Option<&str>,
        osv_id: Option<&str>,
        rustsec_id: Option<&str>,
        package: Option<&str>,
        ecosystem: Option<&str>,
        version: Option<&str>,
    ) -> Self {
        let mut result = Self::default();

        if let Some(id) = cve_id {
            let normalized = normalize_cve(id);
            if !normalized.is_empty() {
                result.cve_ids.push(normalized);
            }
        }
        if let Some(id) = ghsa_id {
            let normalized = normalize_ghsa(id);
            if !normalized.is_empty() {
                result.ghsa_ids.push(normalized);
            }
        }
        if let Some(id) = osv_id {
            result.osv_ids.push(id.to_string());
        }
        if let Some(id) = rustsec_id {
            let normalized = normalize_rustsec(id);
            if !normalized.is_empty() {
                result.rustsec_ids.push(normalized);
            }
        }
        if let Some(p) = package {
            result.package = Some(p.to_string());
        }
        if let Some(e) = ecosystem {
            result.ecosystem = Some(normalize_ecosystem(e));
        }
        if let Some(v) = version {
            result.version = Some(v.to_string());
        }

        let mut residual = query.to_string();

        // Only parse IDs from query text if not already provided explicitly
        if result.cve_ids.is_empty() {
            for cap in CVE_RE.find_iter(query) {
                let id = normalize_cve(cap.as_str());
                if !id.is_empty() && !result.cve_ids.contains(&id) {
                    result.cve_ids.push(id);
                }
            }
        }
        if result.ghsa_ids.is_empty() {
            for cap in GHSA_RE.find_iter(query) {
                let id = normalize_ghsa(cap.as_str());
                if !id.is_empty() && !result.ghsa_ids.contains(&id) {
                    result.ghsa_ids.push(id);
                }
            }
        }
        if result.rustsec_ids.is_empty() {
            for cap in RUSTSEC_RE.find_iter(query) {
                let id = normalize_rustsec(cap.as_str());
                if !id.is_empty() && !result.rustsec_ids.contains(&id) {
                    result.rustsec_ids.push(id);
                }
            }
        }
        // Package/ecosystem/version: only parse from query if not explicit
        for cap in PACKAGE_RE.find_iter(query) {
            if result.package.is_none() {
                let m = cap.as_str();
                if let Some(pos) = m.find(':') {
                    let name = &m[pos + 1..];
                    if !name.is_empty() {
                        result.package = Some(name.to_string());
                    }
                }
            }
        }
        for cap in ECOSYSTEM_RE.find_iter(query) {
            if result.ecosystem.is_none() {
                let m = cap.as_str();
                if let Some(pos) = m.find(':') {
                    let eco = &m[pos + 1..];
                    if !eco.is_empty() {
                        result.ecosystem = Some(normalize_ecosystem(eco));
                    }
                }
            }
        }
        for cap in VERSION_RE.find_iter(query) {
            if result.version.is_none() {
                let m = cap.as_str();
                if let Some(pos) = m.find(':') {
                    let ver = &m[pos + 1..];
                    if !ver.is_empty() {
                        result.version = Some(ver.to_string());
                    }
                }
            }
        }

        residual = remove_identifier_tokens(&residual);
        result.residual_query = residual.trim().to_string();

        result
    }
}

static CVE_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?i)\b(CVE-\d{4}-\d{4,})\b").unwrap());
static GHSA_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?i)\b(GHSA-[a-z0-9]{4}-[a-z0-9]{4}-[a-z0-9]{4})\b").unwrap());
static RUSTSEC_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?i)\b(RUSTSEC-\d{4}-\d{4,})\b").unwrap());
static PACKAGE_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?i)\b(package|crate|pypi|npm):([a-zA-Z0-9_\-\.]+)\b").unwrap());
static ECOSYSTEM_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?i)\b(ecosystem):([a-zA-Z0-9_\-\.]+)\b").unwrap());
static VERSION_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?i)\b(version):([0-9]+[a-zA-Z0-9_\-\.]*)\b").unwrap());

fn normalize_cve(raw: &str) -> String {
    let upper = raw.to_uppercase();
    if CVE_RE.is_match(&upper) {
        upper
    } else {
        String::new()
    }
}

fn normalize_ghsa(raw: &str) -> String {
    let upper = raw.to_uppercase();
    if GHSA_RE.is_match(&upper) {
        upper
    } else {
        String::new()
    }
}

fn normalize_rustsec(raw: &str) -> String {
    let upper = raw.to_uppercase();
    if RUSTSEC_RE.is_match(&upper) {
        upper
    } else {
        String::new()
    }
}

fn normalize_ecosystem(raw: &str) -> String {
    match raw.to_ascii_lowercase().as_str() {
        "crates.io" | "cratesio" | "crate" | "crates" => "crates.io".to_string(),
        "npm" | "npmjs" | "npmjs.com" => "npm".to_string(),
        "pypi" | "pip" | "python" => "pypi".to_string(),
        "go" | "golang" | "pkg.go.dev" => "go".to_string(),
        "rubygems" | "ruby" | "gem" => "rubygems".to_string(),
        "maven" | "java" | "gradle" => "maven".to_string(),
        "nuget" | ".net" => "nuget".to_string(),
        other => other.to_string(),
    }
}

fn remove_identifier_tokens(text: &str) -> String {
    let mut result = text.to_string();
    for cap in CVE_RE.find_iter(text) {
        result = result.replace(cap.as_str(), "");
    }
    for cap in GHSA_RE.find_iter(&result.clone()) {
        result = result.replace(cap.as_str(), "");
    }
    for cap in RUSTSEC_RE.find_iter(&result.clone()) {
        result = result.replace(cap.as_str(), "");
    }
    for cap in PACKAGE_RE.find_iter(&result.clone()) {
        result = result.replace(cap.as_str(), "");
    }
    for cap in ECOSYSTEM_RE.find_iter(&result.clone()) {
        result = result.replace(cap.as_str(), "");
    }
    for cap in VERSION_RE.find_iter(&result.clone()) {
        result = result.replace(cap.as_str(), "");
    }
    // Collapse whitespace.
    let mut out = String::with_capacity(result.len());
    let mut prev_space = false;
    for ch in result.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out
}

/// Classification for security result groups.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SecurityResultGroupKind {
    AuthoritativeAdvisories,
    VendorAdvisories,
    PackageAdvisories,
    KevEntries,
    PatchCommitsOrReleases,
    ExploitDiscussion,
    DefensiveGuidance,
    GeneralContext,
    #[default]
    Other,
}

/// A group of source cards sharing a security classification.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SecurityResultGroup {
    pub kind: SecurityResultGroupKind,
    pub label: String,
    pub results: Vec<SourceCard>,
    pub truncated: bool,
}

/// A suggested URL for follow-up reading.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SecuritySuggestedFetch {
    pub url: String,
    pub reason: String,
    pub group: SecurityResultGroupKind,
    pub priority: u8,
}

/// Input shape for the MCP `security_search` tool.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SecuritySearchRequest {
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ecosystem: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cve_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ghsa_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub osv_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rustsec_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity_min: Option<SeverityLevel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_kev: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_exploit_context: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_defensive_guidance: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_vendor_advisories: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_results: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_per_group: Option<usize>,
    #[serde(default)]
    pub freshness: Freshness,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers: Vec<String>,
}

impl SecuritySearchRequest {
    pub fn validate(&self, max_query_chars: usize) -> Result<(), String> {
        let ids = SecurityIdentifiers::parse(
            &self.query,
            self.cve_id.as_deref(),
            self.ghsa_id.as_deref(),
            self.osv_id.as_deref(),
            self.rustsec_id.as_deref(),
            self.package.as_deref(),
            self.ecosystem.as_deref(),
            self.version.as_deref(),
        );
        if self.query.trim().is_empty() && !ids.has_strong_identifier() {
            return Err(
                "query must not be empty unless at least one strong identifier is provided (cve_id, ghsa_id, osv_id, rustsec_id, or package+ecosystem)"
                    .to_string(),
            );
        }
        if self.query.chars().count() > max_query_chars {
            return Err(format!("query must be <= {max_query_chars} characters"));
        }
        if let Some(0) = self.max_results {
            return Err("max_results must be > 0".to_string());
        }
        if let Some(0) = self.max_per_group {
            return Err("max_per_group must be > 0".to_string());
        }
        Ok(())
    }

    pub fn effective_max_results(&self, default: usize, cap: usize) -> usize {
        crate::core::query::resolve_max_results(self.max_results, default, cap).effective
    }
}

/// Response from `security_search`.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SecuritySearchResponse {
    pub query: String,
    pub mode: String,
    pub resolved_identifiers: SecurityIdentifiers,
    pub vulnerabilities: Vec<VulnerabilityMetadata>,
    pub groups: Vec<SecurityResultGroup>,
    pub suggested_fetches: Vec<SecuritySuggestedFetch>,
    pub providers_queried: Vec<String>,
    pub providers_failed: Vec<ProviderFailure>,
    pub warnings: Vec<SearchWarning>,
    pub trust_markers: TrustMarkers,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_level_from_str_loose() {
        assert_eq!(SeverityLevel::from_str_loose("CRITICAL"), SeverityLevel::Critical);
        assert_eq!(SeverityLevel::from_str_loose("crit"), SeverityLevel::Critical);
        assert_eq!(SeverityLevel::from_str_loose("High"), SeverityLevel::High);
        assert_eq!(SeverityLevel::from_str_loose("important"), SeverityLevel::High);
        assert_eq!(SeverityLevel::from_str_loose("MODERATE"), SeverityLevel::Medium);
        assert_eq!(SeverityLevel::from_str_loose("med"), SeverityLevel::Medium);
        assert_eq!(SeverityLevel::from_str_loose("low"), SeverityLevel::Low);
        assert_eq!(SeverityLevel::from_str_loose("minor"), SeverityLevel::Low);
        assert_eq!(SeverityLevel::from_str_loose("banana"), SeverityLevel::Unknown);
    }

    #[test]
    fn severity_level_as_str() {
        assert_eq!(SeverityLevel::Critical.as_str(), "critical");
        assert_eq!(SeverityLevel::High.as_str(), "high");
        assert_eq!(SeverityLevel::Medium.as_str(), "medium");
        assert_eq!(SeverityLevel::Low.as_str(), "low");
        assert_eq!(SeverityLevel::Unknown.as_str(), "unknown");
    }

    #[test]
    fn normalize_cve_valid() {
        assert_eq!(normalize_cve("CVE-2024-0001"), "CVE-2024-0001");
        assert_eq!(normalize_cve("cve-2024-12345"), "CVE-2024-12345");
        assert_eq!(normalize_cve("CVE-2024-12345678"), "CVE-2024-12345678");
    }

    #[test]
    fn normalize_cve_invalid() {
        assert_eq!(normalize_cve("CVE-24-0001"), "");
        assert_eq!(normalize_cve("CVE-2024-001"), "");
        assert_eq!(normalize_cve("not a cve"), "");
    }

    #[test]
    fn normalize_ghsa_valid() {
        assert_eq!(
            normalize_ghsa("GHSA-xxxx-xxxx-xxxx"),
            "GHSA-XXXX-XXXX-XXXX"
        );
        assert_eq!(
            normalize_ghsa("ghsa-abcd-1234-efgh"),
            "GHSA-ABCD-1234-EFGH"
        );
    }

    #[test]
    fn normalize_ghsa_invalid() {
        assert_eq!(normalize_ghsa("GHSA-xxx"), "");
        assert_eq!(normalize_ghsa("not a ghsa"), "");
    }

    #[test]
    fn normalize_rustsec_valid() {
        assert_eq!(
            normalize_rustsec("RUSTSEC-2024-0001"),
            "RUSTSEC-2024-0001"
        );
        assert_eq!(
            normalize_rustsec("rustsec-2024-12345"),
            "RUSTSEC-2024-12345"
        );
    }

    #[test]
    fn normalize_ecosystem_variants() {
        assert_eq!(normalize_ecosystem("crates.io"), "crates.io");
        assert_eq!(normalize_ecosystem("cratesio"), "crates.io");
        assert_eq!(normalize_ecosystem("crate"), "crates.io");
        assert_eq!(normalize_ecosystem("npm"), "npm");
        assert_eq!(normalize_ecosystem("npmjs.com"), "npm");
        assert_eq!(normalize_ecosystem("pypi"), "pypi");
        assert_eq!(normalize_ecosystem("pip"), "pypi");
        assert_eq!(normalize_ecosystem("python"), "pypi");
        assert_eq!(normalize_ecosystem("go"), "go");
        assert_eq!(normalize_ecosystem("golang"), "go");
    }

    #[test]
    fn identifiers_parse_cve_from_query() {
        let ids = SecurityIdentifiers::parse(
            "CVE-2024-0001 openssl vulnerability",
            None, None, None, None, None, None, None,
        );
        assert_eq!(ids.cve_ids, vec!["CVE-2024-0001"]);
        assert!(ids.residual_query.contains("openssl"));
        assert!(!ids.residual_query.contains("CVE-2024-0001"));
    }

    #[test]
    fn identifiers_parse_ghsa_from_query() {
        let ids = SecurityIdentifiers::parse(
            "GHSA-abcd-1234-efgh is a vulnerability",
            None, None, None, None, None, None, None,
        );
        assert_eq!(ids.ghsa_ids, vec!["GHSA-ABCD-1234-EFGH"]);
        assert!(ids.residual_query.contains("vulnerability"));
    }

    #[test]
    fn identifiers_parse_multiple_ids() {
        let ids = SecurityIdentifiers::parse(
            "CVE-2024-0001 and GHSA-abcd-1234-efgh",
            None, None, None, None, None, None, None,
        );
        assert_eq!(ids.cve_ids, vec!["CVE-2024-0001"]);
        assert_eq!(ids.ghsa_ids, vec!["GHSA-ABCD-1234-EFGH"]);
    }

    #[test]
    fn identifiers_parse_package_hint() {
        let ids = SecurityIdentifiers::parse(
            "package:openssl vulnerability",
            None, None, None, None, None, None, None,
        );
        assert_eq!(ids.package.as_deref(), Some("openssl"));
    }

    #[test]
    fn identifiers_parse_crate_hint() {
        let ids = SecurityIdentifiers::parse(
            "crate:serde-rs serde vulnerability",
            None, None, None, None, None, None, None,
        );
        assert_eq!(ids.package.as_deref(), Some("serde-rs"));
    }

    #[test]
    fn identifiers_parse_ecosystem_hint() {
        let ids = SecurityIdentifiers::parse(
            "ecosystem:crates.io vulnerability",
            None, None, None, None, None, None, None,
        );
        assert_eq!(ids.ecosystem.as_deref(), Some("crates.io"));
    }

    #[test]
    fn identifiers_parse_version_hint() {
        let ids = SecurityIdentifiers::parse(
            "version:1.2.3 vulnerability",
            None, None, None, None, None, None, None,
        );
        assert_eq!(ids.version.as_deref(), Some("1.2.3"));
    }

    #[test]
    fn identifiers_explicit_fields_take_priority() {
        let ids = SecurityIdentifiers::parse(
            "CVE-2024-9999 openssl",
            Some("CVE-2024-0001"),
            None,
            None,
            None,
            Some("mylib"),
            Some("npm"),
            Some("2.0.0"),
        );
        assert_eq!(ids.cve_ids, vec!["CVE-2024-0001"]);
        assert_eq!(ids.package.as_deref(), Some("mylib"));
        assert_eq!(ids.ecosystem.as_deref(), Some("npm"));
        assert_eq!(ids.version.as_deref(), Some("2.0.0"));
    }

    #[test]
    fn identifiers_empty_query_no_identifiers_fails_validation() {
        let req = SecuritySearchRequest {
            query: "   ".to_string(),
            ..Default::default()
        };
        assert!(req.validate(512).is_err());
    }

    #[test]
    fn identifiers_empty_query_with_cve_passes_validation() {
        let req = SecuritySearchRequest {
            query: String::new(),
            cve_id: Some("CVE-2024-0001".to_string()),
            ..Default::default()
        };
        assert!(req.validate(512).is_ok());
    }

    #[test]
    fn identifiers_empty_query_with_package_ecosystem_passes_validation() {
        let req = SecuritySearchRequest {
            query: String::new(),
            package: Some("openssl".to_string()),
            ecosystem: Some("crates.io".to_string()),
            ..Default::default()
        };
        assert!(req.validate(512).is_ok());
    }

    #[test]
    fn has_strong_identifier_true_for_cve() {
        let ids = SecurityIdentifiers {
            cve_ids: vec!["CVE-2024-0001".to_string()],
            ..Default::default()
        };
        assert!(ids.has_strong_identifier());
    }

    #[test]
    fn has_strong_identifier_true_for_package_ecosystem() {
        let ids = SecurityIdentifiers {
            package: Some("openssl".to_string()),
            ecosystem: Some("crates.io".to_string()),
            ..Default::default()
        };
        assert!(ids.has_strong_identifier());
    }

    #[test]
    fn has_strong_identifier_false_without_package_ecosystem() {
        let ids = SecurityIdentifiers {
            package: Some("openssl".to_string()),
            ..Default::default()
        };
        assert!(!ids.has_strong_identifier());
    }

    #[test]
    fn vulnerability_metadata_merge_preserves_self() {
        let a = VulnerabilityMetadata {
            cve_ids: vec!["CVE-2024-0001".to_string()],
            severity: Some(SeverityLevel::High),
            source: VulnerabilitySource::Osv,
            ..Default::default()
        };
        let b = VulnerabilityMetadata {
            cve_ids: vec!["CVE-2024-0002".to_string()],
            severity: Some(SeverityLevel::Critical),
            source: VulnerabilitySource::GithubAdvisory,
            ..Default::default()
        };
        let merged = a.clone().merge(b);
        assert_eq!(merged.cve_ids, vec!["CVE-2024-0001", "CVE-2024-0002"]);
        assert_eq!(merged.severity, Some(SeverityLevel::High));
        assert_eq!(merged.source, VulnerabilitySource::Osv);
    }

    #[test]
    fn vulnerability_metadata_merge_deduplicates_ids() {
        let a = VulnerabilityMetadata {
            cve_ids: vec!["CVE-2024-0001".to_string()],
            ..Default::default()
        };
        let b = VulnerabilityMetadata {
            cve_ids: vec!["CVE-2024-0001".to_string(), "CVE-2024-0002".to_string()],
            ..Default::default()
        };
        let merged = a.merge(b);
        assert_eq!(merged.cve_ids, vec!["CVE-2024-0001", "CVE-2024-0002"]);
    }

    #[test]
    fn security_result_group_kind_default() {
        assert_eq!(
            SecurityResultGroupKind::default(),
            SecurityResultGroupKind::Other
        );
    }

    #[test]
    fn vulnerability_source_as_str() {
        assert_eq!(VulnerabilitySource::Osv.as_str(), "osv");
        assert_eq!(VulnerabilitySource::GithubAdvisory.as_str(), "github_advisory");
        assert_eq!(VulnerabilitySource::Nvd.as_str(), "nvd");
        assert_eq!(VulnerabilitySource::Rustsec.as_str(), "rustsec");
        assert_eq!(VulnerabilitySource::CisaKev.as_str(), "cisa_kev");
        assert_eq!(VulnerabilitySource::Generic.as_str(), "generic");
    }

    #[test]
    fn serde_roundtrip_severity() {
        let s = SeverityLevel::Critical;
        let json = serde_json::to_string(&s).unwrap();
        let parsed: SeverityLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, s);
    }

    #[test]
    fn serde_roundtrip_vulnerability_metadata() {
        let m = VulnerabilityMetadata {
            cve_ids: vec!["CVE-2024-0001".to_string()],
            severity: Some(SeverityLevel::High),
            source: VulnerabilitySource::Osv,
            ..Default::default()
        };
        let json = serde_json::to_string(&m).unwrap();
        let parsed: VulnerabilityMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.cve_ids, m.cve_ids);
        assert_eq!(parsed.severity, m.severity);
        assert_eq!(parsed.source, m.source);
    }

    #[test]
    fn remove_identifier_tokens_cleans_query() {
        let cleaned = remove_identifier_tokens("CVE-2024-0001 openssl vulnerability");
        assert!(!cleaned.contains("CVE-2024-0001"));
        assert!(cleaned.contains("openssl"));
        assert!(cleaned.contains("vulnerability"));
    }
}
