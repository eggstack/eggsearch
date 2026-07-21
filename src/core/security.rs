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

    /// Ordinal rank for severity comparison. Higher = more severe.
    /// `Unknown` maps to 0 so unknown severities never meet any
    /// `severity_min` threshold.
    pub fn rank(self) -> u8 {
        match self {
            Self::Critical => 4,
            Self::High => 3,
            Self::Medium => 2,
            Self::Low => 1,
            Self::Unknown => 0,
        }
    }

    /// Returns true when this severity is at least as severe as
    /// `min`. `Unknown` is never considered severe enough.
    pub fn meets_minimum(self, min: SeverityLevel) -> bool {
        self.rank() >= min.rank() && self != SeverityLevel::Unknown
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
        let mut seen_cve: std::collections::HashSet<String> = cve_ids.iter().cloned().collect();
        for id in &other.cve_ids {
            if seen_cve.insert(id.clone()) {
                cve_ids.push(id.clone());
            }
        }
        let mut ghsa_ids = self.ghsa_ids;
        let mut seen_ghsa: std::collections::HashSet<String> = ghsa_ids.iter().cloned().collect();
        for id in &other.ghsa_ids {
            if seen_ghsa.insert(id.clone()) {
                ghsa_ids.push(id.clone());
            }
        }
        let mut osv_ids = self.osv_ids;
        let mut seen_osv: std::collections::HashSet<String> = osv_ids.iter().cloned().collect();
        for id in &other.osv_ids {
            if seen_osv.insert(id.clone()) {
                osv_ids.push(id.clone());
            }
        }
        let mut rustsec_ids = self.rustsec_ids;
        let mut seen_rustsec: std::collections::HashSet<String> =
            rustsec_ids.iter().cloned().collect();
        for id in &other.rustsec_ids {
            if seen_rustsec.insert(id.clone()) {
                rustsec_ids.push(id.clone());
            }
        }
        let mut affected_ranges = self.affected_ranges;
        let mut seen_affected: std::collections::HashSet<String> =
            affected_ranges.iter().cloned().collect();
        for r in &other.affected_ranges {
            if seen_affected.insert(r.clone()) {
                affected_ranges.push(r.clone());
            }
        }
        let mut patched_ranges = self.patched_ranges;
        let mut seen_patched: std::collections::HashSet<String> =
            patched_ranges.iter().cloned().collect();
        for r in &other.patched_ranges {
            if seen_patched.insert(r.clone()) {
                patched_ranges.push(r.clone());
            }
        }
        let mut vulnerable_versions = self.vulnerable_versions;
        let mut seen_vv: std::collections::HashSet<String> =
            vulnerable_versions.iter().cloned().collect();
        for v in &other.vulnerable_versions {
            if seen_vv.insert(v.clone()) {
                vulnerable_versions.push(v.clone());
            }
        }
        let mut patched_versions = self.patched_versions;
        let mut seen_pv: std::collections::HashSet<String> =
            patched_versions.iter().cloned().collect();
        for v in &other.patched_versions {
            if seen_pv.insert(v.clone()) {
                patched_versions.push(v.clone());
            }
        }
        let mut references = self.references;
        let mut seen_refs: std::collections::HashSet<String> =
            references.iter().map(|r| r.url.clone()).collect();
        for r in &other.references {
            if seen_refs.insert(r.url.clone()) {
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
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema,
)]
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cwe_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ecosystem: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Optional function or API name extracted from `symbol:` hints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function_or_api: Option<String>,
    pub residual_query: String,
}

impl SecurityIdentifiers {
    pub fn has_strong_identifier(&self) -> bool {
        !self.cve_ids.is_empty()
            || !self.ghsa_ids.is_empty()
            || !self.osv_ids.is_empty()
            || !self.rustsec_ids.is_empty()
            || !self.cwe_ids.is_empty()
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
        // CWE IDs: only parse from query if not already provided explicitly
        if result.cwe_ids.is_empty() {
            for cap in CWE_RE.find_iter(query) {
                let id = normalize_cwe(cap.as_str());
                if !id.is_empty() && !result.cwe_ids.contains(&id) {
                    result.cwe_ids.push(id);
                }
            }
        }
        // Function/API names from symbol: hints
        if result.function_or_api.is_none() {
            for cap in SYMBOL_RE.find_iter(query) {
                let m = cap.as_str();
                if let Some(pos) = m.find(':') {
                    let name = &m[pos + 1..];
                    if !name.is_empty() {
                        result.function_or_api = Some(name.to_string());
                    }
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
static GHSA_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?i)\b(GHSA-[a-z0-9]{4}-[a-z0-9]{4}-[a-z0-9]{4})\b").unwrap()
});
static RUSTSEC_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?i)\b(RUSTSEC-\d{4}-\d{4,})\b").unwrap());
static PACKAGE_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?i)\b(package|crate|pypi|npm):([a-zA-Z0-9_\-\.]+)\b").unwrap()
});
static ECOSYSTEM_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?i)\b(ecosystem):([a-zA-Z0-9_\-\.]+)\b").unwrap());
static CWE_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?i)\b(CWE-\d{2,4})\b").unwrap());
static SYMBOL_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?i)\b(symbol):([a-zA-Z0-9_\-\.:\[\]<>,]+)\b").unwrap());
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

fn normalize_cwe(raw: &str) -> String {
    let upper = raw.to_uppercase();
    if CWE_RE.is_match(&upper) {
        upper
    } else {
        String::new()
    }
}

/// Classify the query intent based on parsed identifiers.
pub fn classify_query_kind(ids: &SecurityIdentifiers) -> SecurityQueryKind {
    if !ids.cve_ids.is_empty() {
        return SecurityQueryKind::Cve;
    }
    if !ids.cwe_ids.is_empty() {
        return SecurityQueryKind::Cwe;
    }
    if ids.package.is_some() && ids.ecosystem.is_some() {
        return SecurityQueryKind::Package;
    }
    if ids.function_or_api.is_some() {
        return SecurityQueryKind::Api;
    }
    if !ids.ghsa_ids.is_empty() || !ids.osv_ids.is_empty() || !ids.rustsec_ids.is_empty() {
        return SecurityQueryKind::Cve;
    }
    // Heuristic: check residual query for concept-like patterns
    let residual = ids.residual_query.to_lowercase();
    if residual.contains("vulnerability")
        || residual.contains("exploit")
        || residual.contains("attack")
        || residual.contains("security")
    {
        return SecurityQueryKind::Concept;
    }
    SecurityQueryKind::Unknown
}

/// Convert parsed `SecurityIdentifiers` into the normalized
/// `SecurityIdentifier` list format.
pub fn build_identifier_list(ids: &SecurityIdentifiers) -> Vec<SecurityIdentifier> {
    use crate::core::code_evidence::EvidenceConfidence;

    let mut result = Vec::new();
    for id in &ids.cve_ids {
        result.push(SecurityIdentifier {
            kind: SecurityIdentifierKind::CVE,
            value: id.clone(),
            confidence: EvidenceConfidence::Exact,
        });
    }
    for id in &ids.ghsa_ids {
        result.push(SecurityIdentifier {
            kind: SecurityIdentifierKind::GHSA,
            value: id.clone(),
            confidence: EvidenceConfidence::Exact,
        });
    }
    for id in &ids.osv_ids {
        result.push(SecurityIdentifier {
            kind: SecurityIdentifierKind::OSV,
            value: id.clone(),
            confidence: EvidenceConfidence::Exact,
        });
    }
    for id in &ids.rustsec_ids {
        result.push(SecurityIdentifier {
            kind: SecurityIdentifierKind::RustSec,
            value: id.clone(),
            confidence: EvidenceConfidence::Exact,
        });
    }
    for id in &ids.cwe_ids {
        result.push(SecurityIdentifier {
            kind: SecurityIdentifierKind::CWE,
            value: id.clone(),
            confidence: EvidenceConfidence::Exact,
        });
    }
    if let Some(ref pkg) = ids.package {
        result.push(SecurityIdentifier {
            kind: SecurityIdentifierKind::Package,
            value: pkg.clone(),
            confidence: EvidenceConfidence::Strong,
        });
    }
    if let Some(ref eco) = ids.ecosystem {
        result.push(SecurityIdentifier {
            kind: SecurityIdentifierKind::Ecosystem,
            value: eco.clone(),
            confidence: EvidenceConfidence::Strong,
        });
    }
    if let Some(ref ver) = ids.version {
        result.push(SecurityIdentifier {
            kind: SecurityIdentifierKind::Version,
            value: ver.clone(),
            confidence: EvidenceConfidence::Strong,
        });
    }
    if let Some(ref api) = ids.function_or_api {
        result.push(SecurityIdentifier {
            kind: SecurityIdentifierKind::FunctionOrApi,
            value: api.clone(),
            confidence: EvidenceConfidence::Weak,
        });
    }
    result
}

/// Classify a URL's source tier for security context.
pub fn classify_source_tier(url: &str) -> SecuritySourceTier {
    let url_lower = url.to_ascii_lowercase();

    // Primary advisory databases
    if url_lower.contains("nvd.nist.gov")
        || url_lower.contains("osv.dev")
        || url_lower.contains("rustsec.org")
        || url_lower.contains("cve.mitre.org")
        || url_lower.contains("cwe.mitre.org")
    {
        return SecuritySourceTier::PrimaryAdvisory;
    }

    // Package registry advisories
    if url_lower.contains("github.com/advisories")
        || url_lower.contains("ghsa")
        || url_lower.contains("security.snyk.io")
    {
        return SecuritySourceTier::PackageRegistryAdvisory;
    }

    // Vendor advisories
    if url_lower.contains("advisory") || url_lower.contains("/security/advisories") {
        return SecuritySourceTier::VendorAdvisory;
    }

    // Release notes
    if url_lower.contains("release") || url_lower.contains("changelog") {
        return SecuritySourceTier::ReleaseNotes;
    }

    // Maintainer discussion
    if (url_lower.contains("github.com") || url_lower.contains("gitlab.com"))
        && (url_lower.contains("/issues/") || url_lower.contains("/pull/"))
    {
        return SecuritySourceTier::MaintainerDiscussion;
    }

    // Security research
    if url_lower.contains("exploit")
        || url_lower.contains("poc")
        || url_lower.contains("proof-of-concept")
        || url_lower.contains("research")
        || url_lower.contains("analysis")
    {
        return SecuritySourceTier::SecurityResearch;
    }

    // Community discussion
    if url_lower.contains("stackoverflow.com")
        || url_lower.contains("stackexchange.com")
        || url_lower.contains("forum")
        || url_lower.contains("discourse")
        || url_lower.contains("reddit.com")
    {
        return SecuritySourceTier::CommunityDiscussion;
    }

    // News/blogs
    if url_lower.contains("blog")
        || url_lower.contains("news")
        || url_lower.contains("medium.com")
        || url_lower.contains("dev.to")
    {
        return SecuritySourceTier::NewsOrBlog;
    }

    SecuritySourceTier::Unknown
}

/// Build a `SecuritySourceQuality` assessment for a set of source cards.
pub fn assess_source_quality(results: &[crate::core::SourceCard]) -> SecuritySourceQuality {
    use std::collections::HashSet;

    let mut tiers: HashSet<SecuritySourceTier> = HashSet::new();
    let mut reasons = Vec::new();

    for card in results {
        let tier = classify_source_tier(&card.url);
        tiers.insert(tier);
    }

    // Determine the overall tier: prefer the highest-quality tier found
    let overall_tier = if tiers.contains(&SecuritySourceTier::PrimaryAdvisory) {
        reasons.push("results include primary advisory sources".to_string());
        SecuritySourceTier::PrimaryAdvisory
    } else if tiers.contains(&SecuritySourceTier::PackageRegistryAdvisory) {
        reasons.push("results include package registry advisory sources".to_string());
        SecuritySourceTier::PackageRegistryAdvisory
    } else if tiers.contains(&SecuritySourceTier::VendorAdvisory) {
        reasons.push("results include vendor advisory sources".to_string());
        SecuritySourceTier::VendorAdvisory
    } else if tiers.contains(&SecuritySourceTier::MaintainerDiscussion) {
        reasons.push("results include maintainer discussion sources".to_string());
        SecuritySourceTier::MaintainerDiscussion
    } else if tiers.contains(&SecuritySourceTier::ReleaseNotes) {
        reasons.push("results include release notes".to_string());
        SecuritySourceTier::ReleaseNotes
    } else if tiers.contains(&SecuritySourceTier::SecurityResearch) {
        reasons.push("results include security research sources".to_string());
        SecuritySourceTier::SecurityResearch
    } else if tiers.contains(&SecuritySourceTier::CommunityDiscussion) {
        reasons.push("results include community discussion sources".to_string());
        SecuritySourceTier::CommunityDiscussion
    } else if tiers.contains(&SecuritySourceTier::NewsOrBlog) {
        reasons.push("results include news or blog sources only".to_string());
        SecuritySourceTier::NewsOrBlog
    } else {
        SecuritySourceTier::Unknown
    };

    // Emit warning-level reasons for low-quality tiers
    if matches!(
        overall_tier,
        SecuritySourceTier::NewsOrBlog
            | SecuritySourceTier::CommunityDiscussion
            | SecuritySourceTier::Unknown
    ) {
        reasons
            .push("only low-tier sources found; results may lack advisory authority".to_string());
    }

    SecuritySourceQuality {
        tier: overall_tier,
        tier_reasons: reasons,
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
    for cap in CWE_RE.find_iter(&result.clone()) {
        result = result.replace(cap.as_str(), "");
    }
    for cap in SYMBOL_RE.find_iter(&result.clone()) {
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

/// Classification of the security query's intent.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SecurityQueryKind {
    /// Query targets a specific package or dependency.
    Package,
    /// Query targets a specific CVE identifier.
    Cve,
    /// Query targets a CWE weakness class.
    Cwe,
    /// Query targets an API or function name.
    Api,
    /// Query is about an error message or symptom.
    ErrorMessage,
    /// Query is about a general security concept.
    Concept,
    #[default]
    /// Query intent could not be determined.
    Unknown,
}

impl SecurityQueryKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Package => "package",
            Self::Cve => "cve",
            Self::Cwe => "cwe",
            Self::Api => "api",
            Self::ErrorMessage => "error_message",
            Self::Concept => "concept",
            Self::Unknown => "unknown",
        }
    }
}

/// A parsed security identifier with its kind and confidence.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SecurityIdentifier {
    /// The type of identifier.
    pub kind: SecurityIdentifierKind,
    /// The normalized value (e.g. `CVE-2024-0001`, `CWE-79`).
    pub value: String,
    /// How confidently this identifier was parsed.
    pub confidence: crate::core::code_evidence::EvidenceConfidence,
}

/// Type of security identifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SecurityIdentifierKind {
    /// Common Vulnerabilities and Exposures identifier.
    CVE,
    /// Common Weakness Enumeration identifier.
    CWE,
    /// GitHub Security Advisory identifier.
    GHSA,
    /// Open Source Vulnerabilities identifier.
    OSV,
    /// RustSec advisory identifier.
    RustSec,
    /// Package or dependency name.
    Package,
    /// Ecosystem (crates.io, npm, pypi, etc.).
    Ecosystem,
    /// Specific version or version range.
    Version,
    /// Function, method, or API name.
    FunctionOrApi,
}

impl SecurityIdentifierKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CVE => "cve",
            Self::CWE => "cwe",
            Self::GHSA => "ghsa",
            Self::OSV => "osv",
            Self::RustSec => "rustsec",
            Self::Package => "package",
            Self::Ecosystem => "ecosystem",
            Self::Version => "version",
            Self::FunctionOrApi => "function_or_api",
        }
    }
}

/// Deterministic source quality tier for security results.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SecuritySourceTier {
    /// Primary advisory databases (NVD, OSV, RustSec).
    PrimaryAdvisory,
    /// Vendor or project security pages.
    VendorAdvisory,
    /// Package registry advisory data (GitHub Advisories).
    PackageRegistryAdvisory,
    /// Maintainer discussion (issues, PRs).
    MaintainerDiscussion,
    /// Release notes and changelogs.
    ReleaseNotes,
    /// Security research and analysis.
    SecurityResearch,
    /// News articles or blog posts.
    NewsOrBlog,
    /// Community discussion (forums, StackOverflow).
    CommunityDiscussion,
    #[default]
    /// Source tier could not be determined.
    Unknown,
}

impl SecuritySourceTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PrimaryAdvisory => "primary_advisory",
            Self::VendorAdvisory => "vendor_advisory",
            Self::PackageRegistryAdvisory => "package_registry_advisory",
            Self::MaintainerDiscussion => "maintainer_discussion",
            Self::ReleaseNotes => "release_notes",
            Self::SecurityResearch => "security_research",
            Self::NewsOrBlog => "news_or_blog",
            Self::CommunityDiscussion => "community_discussion",
            Self::Unknown => "unknown",
        }
    }
}

/// Deterministic source quality metadata for a security result.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SecuritySourceQuality {
    /// The determined source tier.
    pub tier: SecuritySourceTier,
    /// Deterministic reasons for the tier classification.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tier_reasons: Vec<String>,
}

/// A deterministic defensive guidance entry derived from source evidence.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DefensiveGuidance {
    /// The category of defensive guidance.
    pub category: DefensiveGuidanceCategory,
    /// Short summary derived from source evidence.
    pub summary: String,
    /// Source URLs that support this guidance.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_urls: Vec<String>,
    /// Confidence in the guidance classification.
    pub confidence: crate::core::code_evidence::EvidenceConfidence,
}

/// Category of defensive guidance.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DefensiveGuidanceCategory {
    /// Upgrade or pin to a fixed version.
    UpgradeOrPin,
    /// Input validation hardening.
    InputValidation,
    /// Output encoding or escaping.
    OutputEncoding,
    /// Authentication or authorization hardening.
    AuthenticationOrAuthorization,
    /// Deserialization hardening.
    DeserializationHardening,
    /// Path traversal prevention.
    PathTraversalHardening,
    /// SSRF prevention.
    SsrFHardening,
    /// SQL injection prevention.
    SqlInjectionHardening,
    /// XSS prevention.
    XssHardening,
    /// Cryptographic configuration.
    CryptoConfiguration,
    /// Resource limit enforcement.
    ResourceLimit,
    /// Safe API usage patterns.
    SafeApiUsage,
    #[default]
    /// Category could not be determined.
    Unknown,
}

impl DefensiveGuidanceCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UpgradeOrPin => "upgrade_or_pin",
            Self::InputValidation => "input_validation",
            Self::OutputEncoding => "output_encoding",
            Self::AuthenticationOrAuthorization => "authentication_or_authorization",
            Self::DeserializationHardening => "deserialization_hardening",
            Self::PathTraversalHardening => "path_traversal_hardening",
            Self::SsrFHardening => "ssrf_hardening",
            Self::SqlInjectionHardening => "sql_injection_hardening",
            Self::XssHardening => "xss_hardening",
            Self::CryptoConfiguration => "crypto_configuration",
            Self::ResourceLimit => "resource_limit",
            Self::SafeApiUsage => "safe_api_usage",
            Self::Unknown => "unknown",
        }
    }
}

/// Defensive remediation action category.
///
/// Each category maps to a concrete action a coding agent can take.
/// The action is evidence-linked and based on advisory metadata, not
/// runtime exploitability assessment.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RemediationCategory {
    /// Upgrade to a fixed version.
    Upgrade,
    /// Pin to a specific safe version.
    Pin,
    /// Replace the vulnerable dependency with an alternative.
    Replace,
    /// Remove the dependency entirely.
    RemoveDependency,
    /// Apply a configuration-level mitigation.
    ConfigurationMitigation,
    /// Disable the vulnerable feature or code path.
    FeatureDisable,
    /// Avoid calling the vulnerable API surface.
    VulnerableApiAvoidance,
    /// Override a transitive dependency version.
    TransitiveOverride,
    /// Apply a vendor-provided patch.
    VendorPatch,
    /// Monitor for upstream fixes; no immediate action possible.
    MonitorOnly,
    /// Manual review required; insufficient evidence for automated action.
    ManualReview,
    /// No actionable remediation supported by available evidence.
    #[default]
    NoActionSupportedByEvidence,
}

impl RemediationCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Upgrade => "upgrade",
            Self::Pin => "pin",
            Self::Replace => "replace",
            Self::RemoveDependency => "remove_dependency",
            Self::ConfigurationMitigation => "configuration_mitigation",
            Self::FeatureDisable => "feature_disable",
            Self::VulnerableApiAvoidance => "vulnerable_api_avoidance",
            Self::TransitiveOverride => "transitive_override",
            Self::VendorPatch => "vendor_patch",
            Self::MonitorOnly => "monitor_only",
            Self::ManualReview => "manual_review",
            Self::NoActionSupportedByEvidence => "no_action_supported_by_evidence",
        }
    }
}

/// A defensive remediation action derived from advisory evidence.
///
/// Remediation actions are category-based, evidence-linked, and
/// defensive. They do NOT include exploit instructions or runtime
/// exploitability determinations.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SecurityRemediation {
    /// The remediation category.
    pub category: RemediationCategory,
    /// Short description of the recommended action.
    pub description: String,
    /// Rationale explaining why this action is recommended.
    pub rationale: String,
    /// Advisory or evidence URLs supporting this action.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_urls: Vec<String>,
    /// Fixed versions recommended by advisory metadata, if available.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixed_versions: Vec<String>,
    /// Packages affected by this remediation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_packages: Vec<String>,
    /// Source card IDs linking to evidence.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_ids: Vec<String>,
    /// Confidence in the remediation recommendation.
    pub confidence: crate::core::code_evidence::EvidenceConfidence,
}

/// Offensive-instruction keyword blocklist for remediation text safety.
///
/// These terms indicate active exploit techniques or offensive security
/// guidance that must never appear in remediation text.
const OFFENSIVE_INSTRUCTION_KEYWORDS: &[&str] = &[
    "exploit",
    "exploit code",
    "payload",
    "shellcode",
    "rop",
    "gadget",
    "pwn",
    "p0c",
    "proof of concept",
    "heap spray",
    "nop sled",
    "buffer overflow exploit",
    "bypass authentication",
    "deserialization attack",
    "zero-day",
    "0day",
];

/// Vulnerability-class keyword blocklist for remediation text safety.
///
/// These terms are vulnerability class names that may legitimately
/// appear in security advisories but should be flagged in remediation
/// action text where they suggest the text was copied from advisory
/// prose rather than written as defensive guidance.
const VULNERABILITY_CLASS_KEYWORDS: &[&str] = &[
    "injection",
    "overflow",
    "rce",
    "remote code execution",
    "buffer overflow",
    "use after free",
    "double free",
    "format string",
    "privilege escalation",
    "sql injection",
    "xss",
    "cross-site scripting",
    "command injection",
    "csrf",
    "xxe",
    "ssrf",
];

/// Category of a text-safety warning.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TextSafetyCategory {
    /// Active exploit technique or offensive security instruction.
    OffensiveInstruction,
    /// Vulnerability class name that may appear in advisory prose.
    VulnerabilityClass,
}

impl TextSafetyCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OffensiveInstruction => "offensive_instruction",
            Self::VulnerabilityClass => "vulnerability_class",
        }
    }
}

/// A text-safety warning with the matched keyword and its category.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TextSafetyWarning {
    /// The keyword that triggered the warning.
    pub keyword: String,
    /// Category of the matched keyword.
    pub category: TextSafetyCategory,
}

impl SecurityRemediation {
    /// Validate that remediation text does not contain exploit instructions.
    ///
    /// Checks `description` and `rationale` against blocklists of known
    /// exploit-related keywords. Returns `Ok(())` if the text is safe,
    /// or `Err(TextSafetyWarning)` with the matched keyword and its
    /// category if a flagged term is found.
    pub fn validate_text_safety(&self) -> Result<(), TextSafetyWarning> {
        let combined = format!(
            "{} {}",
            self.description.to_lowercase(),
            self.rationale.to_lowercase()
        );
        for &keyword in OFFENSIVE_INSTRUCTION_KEYWORDS {
            if combined.contains(keyword) {
                return Err(TextSafetyWarning {
                    keyword: keyword.to_string(),
                    category: TextSafetyCategory::OffensiveInstruction,
                });
            }
        }
        for &keyword in VULNERABILITY_CLASS_KEYWORDS {
            if combined.contains(keyword) {
                return Err(TextSafetyWarning {
                    keyword: keyword.to_string(),
                    category: TextSafetyCategory::VulnerabilityClass,
                });
            }
        }
        Ok(())
    }
}

/// Classification of a security source's role in advisory evidence.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SecuritySourceClass {
    /// Official CVE/NVD/OSV/RustSec advisory.
    PrimaryAdvisory,
    /// Vendor-published security advisory.
    VendorAdvisory,
    /// Maintainer-published security notice.
    MaintainerAdvisory,
    /// Database record (NVD, OSV, GitHub Advisory).
    DatabaseRecord,
    /// CISA KEV catalog entry.
    KevRecord,
    /// Release notes or changelog with security content.
    ReleaseNote,
    /// Patch commit or PR.
    PatchCommit,
    /// Issue thread discussing a vulnerability.
    IssueThread,
    /// Exploit discussion or proof-of-concept.
    ExploitDiscussion,
    /// Defensive guidance or mitigation advice.
    DefensiveGuidance,
    /// Secondary article or blog post.
    SecondaryArticle,
    #[default]
    /// Source class could not be determined.
    Unknown,
}

impl SecuritySourceClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PrimaryAdvisory => "primary_advisory",
            Self::VendorAdvisory => "vendor_advisory",
            Self::MaintainerAdvisory => "maintainer_advisory",
            Self::DatabaseRecord => "database_record",
            Self::KevRecord => "kev_record",
            Self::ReleaseNote => "release_note",
            Self::PatchCommit => "patch_commit",
            Self::IssueThread => "issue_thread",
            Self::ExploitDiscussion => "exploit_discussion",
            Self::DefensiveGuidance => "defensive_guidance",
            Self::SecondaryArticle => "secondary_article",
            Self::Unknown => "unknown",
        }
    }
}

/// Rank reason for security source quality assessment.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SecurityRankReason {
    /// Source is an official advisory database.
    OfficialDatabase,
    /// Source is vendor-maintained.
    VendorMaintained,
    /// Source is from the package maintainer.
    MaintainerSource,
    /// Advisory contains version range information.
    VersionRangePresent,
    /// Advisory contains a fixed version.
    FixedVersionPresent,
    /// CVE is in the CISA KEV catalog.
    KevMatch,
    /// Source includes patch commit evidence.
    PatchEvidence,
    /// Source includes release note evidence.
    ReleaseNoteEvidence,
    /// Source is low authority or secondary only.
    LowAuthority,
    #[default]
    /// Rank reason could not be determined.
    Unknown,
}

impl SecurityRankReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OfficialDatabase => "official_database",
            Self::VendorMaintained => "vendor_maintained",
            Self::MaintainerSource => "maintainer_source",
            Self::VersionRangePresent => "version_range_present",
            Self::FixedVersionPresent => "fixed_version_present",
            Self::KevMatch => "kev_match",
            Self::PatchEvidence => "patch_evidence",
            Self::ReleaseNoteEvidence => "release_note_evidence",
            Self::LowAuthority => "low_authority",
            Self::Unknown => "unknown",
        }
    }
}

/// Aggregate security evidence summary for the response.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SecurityEvidenceSummary {
    /// Total number of vulnerability records found.
    pub total_vulnerabilities: usize,
    /// Total number of applicability assessments made.
    pub total_assessments: usize,
    /// Count of assessments with `affected` status.
    pub affected_count: usize,
    /// Count of assessments with `not_affected` status.
    pub not_affected_count: usize,
    /// Count of assessments with `unknown` status.
    pub unknown_count: usize,
    /// Count of assessments with `insufficient_evidence` status.
    pub insufficient_evidence_count: usize,
    /// Number of remediation actions generated.
    pub remediation_count: usize,
    /// Highest severity among found vulnerabilities.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highest_severity: Option<SeverityLevel>,
    /// Whether any CVE is in the KEV catalog.
    pub kev_match_present: bool,
    /// Aggregate source quality tier.
    pub source_quality_tier: SecuritySourceTier,
    /// Whether any authoritative advisory source was found.
    pub has_authoritative_source: bool,
}

/// A suggested URL for follow-up reading.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SecurityContext {
    /// Classified query intent.
    pub query_kind: SecurityQueryKind,
    /// All parsed identifiers from the query.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifiers: Vec<SecurityIdentifier>,
    /// Summary of affected packages.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_packages: Vec<AffectedPackageSummary>,
    /// Summary of known vulnerabilities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vulnerability_summaries: Vec<VulnerabilitySummary>,
    /// Defensive guidance derived from source evidence.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub defensive_guidance: Vec<DefensiveGuidance>,
    /// Aggregate source quality assessment.
    pub source_quality: SecuritySourceQuality,
    /// Context-specific warnings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// Compact security context for `repo_search` responses.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CompactSecurityContext {
    /// Classified query intent.
    pub query_kind: SecurityQueryKind,
    /// Parsed identifiers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifiers: Vec<SecurityIdentifier>,
    /// Number of known vulnerabilities found.
    pub vulnerability_count: usize,
    /// Highest severity among known vulnerabilities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highest_severity: Option<SeverityLevel>,
    /// Aggregate source quality assessment.
    pub source_quality: SecuritySourceQuality,
    /// Context-specific warnings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// Summary of an affected package from advisory data.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AffectedPackageSummary {
    /// Package name.
    pub package: String,
    /// Ecosystem.
    pub ecosystem: String,
    /// Affected version ranges.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_ranges: Vec<String>,
    /// Patched versions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub patched_versions: Vec<String>,
}

/// Summary of a known vulnerability.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct VulnerabilitySummary {
    /// Primary identifier (CVE, GHSA, etc.).
    pub id: String,
    /// Severity level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<SeverityLevel>,
    /// Brief description or title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Source that provided this vulnerability.
    pub source: VulnerabilitySource,
    /// Whether this is in the CISA KEV catalog.
    pub kev: bool,
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
    /// Aggregate quality summary for this group's results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality_summary: Option<crate::core::quality::GroupQualitySummary>,
}

/// A suggested URL for follow-up reading.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SecuritySuggestedFetch {
    pub url: String,
    pub reason: String,
    pub group: SecurityResultGroupKind,
    pub priority: u8,
    /// Deterministic, content-derived identifier stable across runs.
    /// Format: `suggested_<16hex>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stable_id: Option<String>,
    /// Deterministic source card ID linking this suggested fetch back
    /// to the source card that produced it. `None` for synthesized
    /// advisory URLs without a source card.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    /// Deterministic score for this suggestion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<i32>,
    /// Rank reasons explaining why this fetch was scored as it was.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rank_reasons: Vec<String>,
    /// Information gain estimate (0.0 to 1.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub information_gain: Option<f32>,
    /// Stable machine-readable reason code for this suggested fetch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
    /// Advisory IDs this suggested fetch relates to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub advisory_ids: Vec<String>,
    /// Package name this suggested fetch relates to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    /// Version this suggested fetch relates to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assess_applicability: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependency_files: Vec<String>,
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
        if let Some(0) = self.timeout_ms {
            return Err("timeout_ms must be > 0".to_string());
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
    /// Aggregated security context with source quality, defensive
    /// guidance, and normalized identifiers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security_context: Option<SecurityContext>,
    pub groups: Vec<SecurityResultGroup>,
    pub suggested_fetches: Vec<SecuritySuggestedFetch>,
    pub providers_queried: Vec<String>,
    pub providers_failed: Vec<ProviderFailure>,
    pub warnings: Vec<SearchWarning>,
    pub trust_markers: TrustMarkers,
    /// Capability enforcement telemetry for this request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_enforcement:
        Option<crate::meta::provider_diagnostics::CapabilityEnforcementTelemetry>,
    /// Provider routing decision for this request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routing_decision: Option<crate::meta::provider_diagnostics::ProviderRoutingDecision>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub applicability: Vec<crate::core::security_applicability::ApplicabilityAssessment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependency_findings: Vec<crate::core::security_applicability::DependencyFinding>,
    /// Structured warnings with stable machine-readable codes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub structured_warnings: Vec<crate::core::warning::AgentWarning>,
    /// Next-action hints for chaining tool calls.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub next_actions: Vec<crate::core::workflow::AgentNextAction>,
    /// Defensive remediation actions derived from advisory evidence.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remediation_actions: Vec<SecurityRemediation>,
    /// Aggregate security evidence summary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security_evidence_summary: Option<SecurityEvidenceSummary>,
    /// Retrieval status summary across providers and evidence dimensions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval_summary: Option<crate::core::retrieval_status::ResponseRetrievalSummary>,
    /// Role-based evidence summary with counts and coverage status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_role_summary: Option<crate::core::evidence_postprocess::EvidenceRoleSummary>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_level_from_str_loose() {
        assert_eq!(
            SeverityLevel::from_str_loose("CRITICAL"),
            SeverityLevel::Critical
        );
        assert_eq!(
            SeverityLevel::from_str_loose("crit"),
            SeverityLevel::Critical
        );
        assert_eq!(SeverityLevel::from_str_loose("High"), SeverityLevel::High);
        assert_eq!(
            SeverityLevel::from_str_loose("important"),
            SeverityLevel::High
        );
        assert_eq!(
            SeverityLevel::from_str_loose("MODERATE"),
            SeverityLevel::Medium
        );
        assert_eq!(SeverityLevel::from_str_loose("med"), SeverityLevel::Medium);
        assert_eq!(SeverityLevel::from_str_loose("low"), SeverityLevel::Low);
        assert_eq!(SeverityLevel::from_str_loose("minor"), SeverityLevel::Low);
        assert_eq!(
            SeverityLevel::from_str_loose("banana"),
            SeverityLevel::Unknown
        );
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
    fn severity_level_meets_minimum() {
        assert!(SeverityLevel::Critical.meets_minimum(SeverityLevel::High));
        assert!(SeverityLevel::High.meets_minimum(SeverityLevel::High));
        assert!(SeverityLevel::Medium.meets_minimum(SeverityLevel::Low));
        assert!(!SeverityLevel::Low.meets_minimum(SeverityLevel::Medium));
        assert!(!SeverityLevel::Unknown.meets_minimum(SeverityLevel::Low));
        assert!(!SeverityLevel::Unknown.meets_minimum(SeverityLevel::Critical));
        assert!(SeverityLevel::Critical.meets_minimum(SeverityLevel::Critical));
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
        assert_eq!(normalize_ghsa("GHSA-xxxx-xxxx-xxxx"), "GHSA-XXXX-XXXX-XXXX");
        assert_eq!(normalize_ghsa("ghsa-abcd-1234-efgh"), "GHSA-ABCD-1234-EFGH");
    }

    #[test]
    fn normalize_ghsa_invalid() {
        assert_eq!(normalize_ghsa("GHSA-xxx"), "");
        assert_eq!(normalize_ghsa("not a ghsa"), "");
    }

    #[test]
    fn normalize_rustsec_valid() {
        assert_eq!(normalize_rustsec("RUSTSEC-2024-0001"), "RUSTSEC-2024-0001");
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
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(ids.cve_ids, vec!["CVE-2024-0001"]);
        assert!(ids.residual_query.contains("openssl"));
        assert!(!ids.residual_query.contains("CVE-2024-0001"));
    }

    #[test]
    fn identifiers_parse_ghsa_from_query() {
        let ids = SecurityIdentifiers::parse(
            "GHSA-abcd-1234-efgh is a vulnerability",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(ids.ghsa_ids, vec!["GHSA-ABCD-1234-EFGH"]);
        assert!(ids.residual_query.contains("vulnerability"));
    }

    #[test]
    fn identifiers_parse_multiple_ids() {
        let ids = SecurityIdentifiers::parse(
            "CVE-2024-0001 and GHSA-abcd-1234-efgh",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(ids.cve_ids, vec!["CVE-2024-0001"]);
        assert_eq!(ids.ghsa_ids, vec!["GHSA-ABCD-1234-EFGH"]);
    }

    #[test]
    fn identifiers_parse_package_hint() {
        let ids = SecurityIdentifiers::parse(
            "package:openssl vulnerability",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(ids.package.as_deref(), Some("openssl"));
    }

    #[test]
    fn identifiers_parse_crate_hint() {
        let ids = SecurityIdentifiers::parse(
            "crate:serde-rs serde vulnerability",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(ids.package.as_deref(), Some("serde-rs"));
    }

    #[test]
    fn identifiers_parse_ecosystem_hint() {
        let ids = SecurityIdentifiers::parse(
            "ecosystem:crates.io vulnerability",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(ids.ecosystem.as_deref(), Some("crates.io"));
    }

    #[test]
    fn identifiers_parse_version_hint() {
        let ids = SecurityIdentifiers::parse(
            "version:1.2.3 vulnerability",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
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
    fn validate_rejects_zero_timeout_ms() {
        let req = SecuritySearchRequest {
            query: "CVE-2024-0001".to_string(),
            timeout_ms: Some(0),
            ..Default::default()
        };
        let err = req.validate(512).unwrap_err();
        assert!(err.contains("timeout_ms"));
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
        assert_eq!(
            VulnerabilitySource::GithubAdvisory.as_str(),
            "github_advisory"
        );
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

    #[test]
    fn normalize_cwe_valid() {
        assert_eq!(normalize_cwe("CWE-79"), "CWE-79");
        assert_eq!(normalize_cwe("cwe-89"), "CWE-89");
        assert_eq!(normalize_cwe("CWE-1234"), "CWE-1234");
    }

    #[test]
    fn normalize_cwe_invalid() {
        assert_eq!(normalize_cwe("CWE-0"), "");
        assert_eq!(normalize_cwe("CWE-"), "");
        assert_eq!(normalize_cwe("not a cwe"), "");
    }

    #[test]
    fn identifiers_parse_cwe_from_query() {
        let ids = SecurityIdentifiers::parse(
            "CWE-79 cross-site scripting vulnerability",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(ids.cwe_ids, vec!["CWE-79"]);
        assert!(ids.residual_query.contains("cross-site scripting"));
        assert!(!ids.residual_query.contains("CWE-79"));
    }

    #[test]
    fn identifiers_parse_symbol_hint() {
        let ids = SecurityIdentifiers::parse(
            "symbol:Router::layer vulnerability",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(ids.function_or_api.as_deref(), Some("Router::layer"));
    }

    #[test]
    fn has_strong_identifier_true_for_cwe() {
        let ids = SecurityIdentifiers {
            cwe_ids: vec!["CWE-79".to_string()],
            ..Default::default()
        };
        assert!(ids.has_strong_identifier());
    }

    #[test]
    fn classify_query_kind_cve() {
        let ids = SecurityIdentifiers {
            cve_ids: vec!["CVE-2024-0001".to_string()],
            ..Default::default()
        };
        assert_eq!(classify_query_kind(&ids), SecurityQueryKind::Cve);
    }

    #[test]
    fn classify_query_kind_cwe() {
        let ids = SecurityIdentifiers {
            cwe_ids: vec!["CWE-79".to_string()],
            ..Default::default()
        };
        assert_eq!(classify_query_kind(&ids), SecurityQueryKind::Cwe);
    }

    #[test]
    fn classify_query_kind_package() {
        let ids = SecurityIdentifiers {
            package: Some("openssl".to_string()),
            ecosystem: Some("crates.io".to_string()),
            ..Default::default()
        };
        assert_eq!(classify_query_kind(&ids), SecurityQueryKind::Package);
    }

    #[test]
    fn classify_query_kind_api() {
        let ids = SecurityIdentifiers {
            function_or_api: Some("Router::layer".to_string()),
            ..Default::default()
        };
        assert_eq!(classify_query_kind(&ids), SecurityQueryKind::Api);
    }

    #[test]
    fn classify_query_kind_concept() {
        let ids = SecurityIdentifiers {
            residual_query: "security vulnerability in authentication".to_string(),
            ..Default::default()
        };
        assert_eq!(classify_query_kind(&ids), SecurityQueryKind::Concept);
    }

    #[test]
    fn classify_query_kind_unknown() {
        let ids = SecurityIdentifiers::default();
        assert_eq!(classify_query_kind(&ids), SecurityQueryKind::Unknown);
    }

    #[test]
    fn build_identifier_list_all_types() {
        let ids = SecurityIdentifiers {
            cve_ids: vec!["CVE-2024-0001".to_string()],
            ghsa_ids: vec!["GHSA-test-1234-abcd".to_string()],
            osv_ids: vec!["GHSA-osv-1234-efgh".to_string()],
            rustsec_ids: vec!["RUSTSEC-2024-0001".to_string()],
            cwe_ids: vec!["CWE-79".to_string()],
            package: Some("openssl".to_string()),
            ecosystem: Some("crates.io".to_string()),
            version: Some("1.0.0".to_string()),
            function_or_api: Some("connect".to_string()),
            ..Default::default()
        };
        let list = build_identifier_list(&ids);
        assert_eq!(list.len(), 9);
        assert!(list.iter().any(|i| i.kind == SecurityIdentifierKind::CVE));
        assert!(list.iter().any(|i| i.kind == SecurityIdentifierKind::GHSA));
        assert!(list.iter().any(|i| i.kind == SecurityIdentifierKind::OSV));
        assert!(list
            .iter()
            .any(|i| i.kind == SecurityIdentifierKind::RustSec));
        assert!(list.iter().any(|i| i.kind == SecurityIdentifierKind::CWE));
        assert!(list
            .iter()
            .any(|i| i.kind == SecurityIdentifierKind::Package));
        assert!(list
            .iter()
            .any(|i| i.kind == SecurityIdentifierKind::Ecosystem));
        assert!(list
            .iter()
            .any(|i| i.kind == SecurityIdentifierKind::Version));
        assert!(list
            .iter()
            .any(|i| i.kind == SecurityIdentifierKind::FunctionOrApi));
    }

    #[test]
    fn classify_source_tier_nvd() {
        assert_eq!(
            classify_source_tier("https://nvd.nist.gov/vuln/detail/CVE-2024-0001"),
            SecuritySourceTier::PrimaryAdvisory
        );
    }

    #[test]
    fn classify_source_tier_osv() {
        assert_eq!(
            classify_source_tier("https://osv.dev/vulnerability/GHSA-test"),
            SecuritySourceTier::PrimaryAdvisory
        );
    }

    #[test]
    fn classify_source_tier_github_advisory() {
        assert_eq!(
            classify_source_tier("https://github.com/advisories/GHSA-test"),
            SecuritySourceTier::PackageRegistryAdvisory
        );
    }

    #[test]
    fn classify_source_tier_vendor_advisory() {
        assert_eq!(
            classify_source_tier("https://example.com/security/advisory"),
            SecuritySourceTier::VendorAdvisory
        );
    }

    #[test]
    fn classify_source_tier_release_notes() {
        assert_eq!(
            classify_source_tier("https://github.com/foo/bar/releases/tag/v1.0"),
            SecuritySourceTier::ReleaseNotes
        );
    }

    #[test]
    fn classify_source_tier_maintainer_discussion() {
        assert_eq!(
            classify_source_tier("https://github.com/foo/bar/issues/123"),
            SecuritySourceTier::MaintainerDiscussion
        );
    }

    #[test]
    fn classify_source_tier_exploit_research() {
        assert_eq!(
            classify_source_tier("https://example.com/exploit/poc"),
            SecuritySourceTier::SecurityResearch
        );
    }

    #[test]
    fn classify_source_tier_community() {
        assert_eq!(
            classify_source_tier("https://stackoverflow.com/questions/123"),
            SecuritySourceTier::CommunityDiscussion
        );
    }

    #[test]
    fn classify_source_tier_blog() {
        assert_eq!(
            classify_source_tier("https://blog.example.com/security"),
            SecuritySourceTier::NewsOrBlog
        );
    }

    #[test]
    fn classify_source_tier_unknown() {
        assert_eq!(
            classify_source_tier("https://example.com/some/page"),
            SecuritySourceTier::Unknown
        );
    }

    #[test]
    fn security_query_kind_as_str() {
        assert_eq!(SecurityQueryKind::Package.as_str(), "package");
        assert_eq!(SecurityQueryKind::Cve.as_str(), "cve");
        assert_eq!(SecurityQueryKind::Cwe.as_str(), "cwe");
        assert_eq!(SecurityQueryKind::Api.as_str(), "api");
        assert_eq!(SecurityQueryKind::Unknown.as_str(), "unknown");
    }

    #[test]
    fn security_identifier_kind_as_str() {
        assert_eq!(SecurityIdentifierKind::CVE.as_str(), "cve");
        assert_eq!(SecurityIdentifierKind::CWE.as_str(), "cwe");
        assert_eq!(SecurityIdentifierKind::Package.as_str(), "package");
        assert_eq!(
            SecurityIdentifierKind::FunctionOrApi.as_str(),
            "function_or_api"
        );
    }

    #[test]
    fn security_source_tier_as_str() {
        assert_eq!(
            SecuritySourceTier::PrimaryAdvisory.as_str(),
            "primary_advisory"
        );
        assert_eq!(
            SecuritySourceTier::VendorAdvisory.as_str(),
            "vendor_advisory"
        );
        assert_eq!(SecuritySourceTier::Unknown.as_str(), "unknown");
    }

    #[test]
    fn defensive_guidance_category_as_str() {
        assert_eq!(
            DefensiveGuidanceCategory::UpgradeOrPin.as_str(),
            "upgrade_or_pin"
        );
        assert_eq!(
            DefensiveGuidanceCategory::XssHardening.as_str(),
            "xss_hardening"
        );
        assert_eq!(DefensiveGuidanceCategory::Unknown.as_str(), "unknown");
    }

    #[test]
    fn security_context_serde_roundtrip() {
        let ctx = SecurityContext {
            query_kind: SecurityQueryKind::Package,
            identifiers: vec![SecurityIdentifier {
                kind: SecurityIdentifierKind::CVE,
                value: "CVE-2024-0001".to_string(),
                confidence: crate::core::code_evidence::EvidenceConfidence::Exact,
            }],
            affected_packages: vec![AffectedPackageSummary {
                package: "openssl".to_string(),
                ecosystem: "crates.io".to_string(),
                affected_ranges: vec!["< 1.0.0".to_string()],
                patched_versions: vec!["1.0.0".to_string()],
            }],
            vulnerability_summaries: vec![VulnerabilitySummary {
                id: "CVE-2024-0001".to_string(),
                severity: Some(SeverityLevel::High),
                description: None,
                source: VulnerabilitySource::Osv,
                kev: false,
            }],
            defensive_guidance: vec![],
            source_quality: SecuritySourceQuality {
                tier: SecuritySourceTier::PrimaryAdvisory,
                tier_reasons: vec!["results include primary advisory sources".to_string()],
            },
            warnings: vec![],
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let parsed: SecurityContext = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.query_kind, SecurityQueryKind::Package);
        assert_eq!(parsed.identifiers.len(), 1);
        assert_eq!(parsed.vulnerability_summaries.len(), 1);
    }

    #[test]
    fn compact_security_context_serde_roundtrip() {
        let ctx = CompactSecurityContext {
            query_kind: SecurityQueryKind::Package,
            identifiers: vec![],
            vulnerability_count: 3,
            highest_severity: Some(SeverityLevel::High),
            source_quality: SecuritySourceQuality {
                tier: SecuritySourceTier::PackageRegistryAdvisory,
                tier_reasons: vec![],
            },
            warnings: vec![],
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let parsed: CompactSecurityContext = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.vulnerability_count, 3);
        assert_eq!(parsed.highest_severity, Some(SeverityLevel::High));
    }

    #[test]
    fn assess_source_quality_with_authoritative() {
        use crate::core::result::TrustLevel;
        use crate::core::SourceCard;

        let cards = vec![SourceCard::new(
            "NVD",
            "https://nvd.nist.gov/vuln/detail/CVE-2024-0001",
            vec!["osv".to_string()],
            None,
            TrustLevel::ExternalUntrusted,
        )];
        let quality = assess_source_quality(&cards);
        assert_eq!(quality.tier, SecuritySourceTier::PrimaryAdvisory);
        assert!(!quality.tier_reasons.is_empty());
    }

    #[test]
    fn assess_source_quality_with_blog_only() {
        use crate::core::result::TrustLevel;
        use crate::core::SourceCard;

        let cards = vec![SourceCard::new(
            "Blog Post",
            "https://blog.example.com/security",
            vec!["duckduckgo".to_string()],
            None,
            TrustLevel::ExternalUntrusted,
        )];
        let quality = assess_source_quality(&cards);
        assert_eq!(quality.tier, SecuritySourceTier::NewsOrBlog);
        assert!(quality.tier_reasons.iter().any(|r| r.contains("low-tier")));
    }

    // ---------------------------------------------------------------
    // Task 6: Security-context safety and source-quality tests
    // ---------------------------------------------------------------

    #[test]
    fn cve_query_produces_exact_identifier_context() {
        let ids = SecurityIdentifiers::parse(
            "CVE-2024-1234 is a critical vulnerability",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(ids.cve_ids, vec!["CVE-2024-1234"]);
        assert!(
            ids.has_strong_identifier(),
            "CVE should be a strong identifier"
        );
        assert_eq!(
            classify_query_kind(&ids),
            SecurityQueryKind::Cve,
            "CVE query should classify as Cve"
        );
    }

    #[test]
    fn ghsa_query_produces_exact_identifier_context() {
        let ids = SecurityIdentifiers::parse(
            "GHSA-abcd-1234-efgh affects multiple packages",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(ids.ghsa_ids, vec!["GHSA-ABCD-1234-EFGH"]);
        assert!(
            ids.has_strong_identifier(),
            "GHSA should be a strong identifier"
        );
        assert_eq!(
            classify_query_kind(&ids),
            SecurityQueryKind::Cve,
            "GHSA-only query should classify as Cve (via fallback)"
        );
    }

    #[test]
    fn cwe_query_produces_weakness_class_context() {
        let ids = SecurityIdentifiers::parse(
            "CWE-79 cross-site scripting in web apps",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(ids.cwe_ids, vec!["CWE-79"]);
        assert!(
            ids.has_strong_identifier(),
            "CWE should be a strong identifier"
        );
        assert_eq!(
            classify_query_kind(&ids),
            SecurityQueryKind::Cwe,
            "CWE query should classify as Cwe"
        );
        let list = build_identifier_list(&ids);
        assert!(
            list.iter().any(|i| i.kind == SecurityIdentifierKind::CWE),
            "build_identifier_list should include CWE"
        );
    }

    #[test]
    fn package_version_query_no_advisory_match_has_no_false_vulnerability_claim() {
        let ids = SecurityIdentifiers {
            package: Some("nonexistent-crate".to_string()),
            ecosystem: Some("crates.io".to_string()),
            version: Some("9.9.9".to_string()),
            ..Default::default()
        };
        let ctx = SecurityContext {
            query_kind: classify_query_kind(&ids),
            identifiers: build_identifier_list(&ids),
            affected_packages: vec![],
            vulnerability_summaries: vec![],
            defensive_guidance: vec![],
            source_quality: SecuritySourceQuality {
                tier: SecuritySourceTier::Unknown,
                tier_reasons: vec![],
            },
            warnings: vec![],
        };
        assert!(
            ctx.vulnerability_summaries.is_empty(),
            "no vulnerabilities should be claimed when advisory data is absent"
        );
        assert!(
            ctx.affected_packages.is_empty(),
            "no affected packages should be claimed when advisory data is absent"
        );
        assert_eq!(ctx.query_kind, SecurityQueryKind::Package);
        let pkg_id = ctx
            .identifiers
            .iter()
            .find(|i| i.kind == SecurityIdentifierKind::Package);
        assert!(
            pkg_id.is_some(),
            "package identifier should be present in the list"
        );
    }

    #[test]
    fn exploit_context_flag_does_not_produce_executable_payload_fields() {
        let req = SecuritySearchRequest {
            query: "CVE-2024-0001 exploit".to_string(),
            include_exploit_context: Some(true),
            ..Default::default()
        };
        let resp = SecuritySearchResponse {
            query: req.query.clone(),
            mode: "security_metasearch".to_string(),
            resolved_identifiers: SecurityIdentifiers::parse(
                &req.query,
                req.cve_id.as_deref(),
                req.ghsa_id.as_deref(),
                req.osv_id.as_deref(),
                req.rustsec_id.as_deref(),
                req.package.as_deref(),
                req.ecosystem.as_deref(),
                req.version.as_deref(),
            ),
            vulnerabilities: vec![],
            security_context: None,
            groups: vec![SecurityResultGroup {
                kind: SecurityResultGroupKind::ExploitDiscussion,
                label: "Exploit Discussion".to_string(),
                results: vec![],
                truncated: false,
                quality_summary: None,
            }],
            suggested_fetches: vec![],
            providers_queried: vec!["mock".to_string()],
            providers_failed: vec![],
            warnings: vec![],
            trust_markers: TrustMarkers::default(),
            capability_enforcement: None,
            routing_decision: None,
            applicability: vec![],
            dependency_findings: vec![],
            structured_warnings: vec![],
            next_actions: vec![],
            remediation_actions: vec![],
            security_evidence_summary: None,
            retrieval_summary: None,
            evidence_role_summary: None,
        };
        let json = serde_json::to_value(&resp).unwrap();
        let groups = json["groups"].as_array().expect("groups");
        let exploit_group = groups
            .iter()
            .find(|g| g["kind"].as_str() == Some("exploit_discussion"));
        assert!(
            exploit_group.is_some(),
            "exploit_discussion group should be present"
        );
        let results = exploit_group.unwrap()["results"]
            .as_array()
            .expect("results");
        for card in results {
            let text = serde_json::to_string(card).unwrap();
            assert!(
                !text.contains("payload"),
                "exploit card must not contain 'payload': {text}"
            );
            assert!(
                !text.contains("exploit_code"),
                "exploit card must not contain 'exploit_code': {text}"
            );
        }
    }

    #[test]
    fn defensive_guidance_categories_are_mitigation_oriented() {
        let defensive_categories: Vec<DefensiveGuidanceCategory> = vec![
            DefensiveGuidanceCategory::UpgradeOrPin,
            DefensiveGuidanceCategory::InputValidation,
            DefensiveGuidanceCategory::OutputEncoding,
            DefensiveGuidanceCategory::AuthenticationOrAuthorization,
            DefensiveGuidanceCategory::DeserializationHardening,
            DefensiveGuidanceCategory::PathTraversalHardening,
            DefensiveGuidanceCategory::SsrFHardening,
            DefensiveGuidanceCategory::SqlInjectionHardening,
            DefensiveGuidanceCategory::XssHardening,
            DefensiveGuidanceCategory::CryptoConfiguration,
            DefensiveGuidanceCategory::ResourceLimit,
            DefensiveGuidanceCategory::SafeApiUsage,
        ];
        for cat in &defensive_categories {
            let name = cat.as_str();
            assert!(
                !name.is_empty(),
                "DefensiveGuidanceCategory variant must have a non-empty as_str()"
            );
            let json = serde_json::to_string(cat).unwrap();
            let parsed: DefensiveGuidanceCategory = serde_json::from_str(&json).unwrap();
            assert_eq!(&parsed, cat);
        }
        let all_variants: Vec<DefensiveGuidanceCategory> = vec![
            DefensiveGuidanceCategory::UpgradeOrPin,
            DefensiveGuidanceCategory::InputValidation,
            DefensiveGuidanceCategory::OutputEncoding,
            DefensiveGuidanceCategory::AuthenticationOrAuthorization,
            DefensiveGuidanceCategory::DeserializationHardening,
            DefensiveGuidanceCategory::PathTraversalHardening,
            DefensiveGuidanceCategory::SsrFHardening,
            DefensiveGuidanceCategory::SqlInjectionHardening,
            DefensiveGuidanceCategory::XssHardening,
            DefensiveGuidanceCategory::CryptoConfiguration,
            DefensiveGuidanceCategory::ResourceLimit,
            DefensiveGuidanceCategory::SafeApiUsage,
            DefensiveGuidanceCategory::Unknown,
        ];
        assert_eq!(
            all_variants.len(),
            13,
            "exhaustive DefensiveGuidanceCategory variant count"
        );
    }

    #[test]
    fn classify_source_tier_nvd_url() {
        assert_eq!(
            classify_source_tier("https://nvd.nist.gov/vuln/detail/CVE-2024-1234"),
            SecuritySourceTier::PrimaryAdvisory
        );
    }

    #[test]
    fn classify_source_tier_github_advisory_url() {
        assert_eq!(
            classify_source_tier("https://github.com/advisories/GHSA-test-1234-abcd"),
            SecuritySourceTier::PackageRegistryAdvisory
        );
    }

    #[test]
    fn classify_source_tier_blog_url() {
        assert_eq!(
            classify_source_tier("https://blog.example.com/2024/security-post"),
            SecuritySourceTier::NewsOrBlog
        );
    }

    #[test]
    fn classify_source_tier_stackoverflow_url() {
        assert_eq!(
            classify_source_tier("https://stackoverflow.com/questions/12345/how-to-fix"),
            SecuritySourceTier::CommunityDiscussion
        );
    }

    #[test]
    fn classify_source_tier_unknown_url() {
        assert_eq!(
            classify_source_tier("https://example.com/some/random/page"),
            SecuritySourceTier::Unknown
        );
    }

    #[test]
    fn assess_source_quality_picks_highest_tier() {
        use crate::core::result::TrustLevel;
        use crate::core::SourceCard;

        let cards = vec![
            SourceCard::new(
                "Blog Post",
                "https://blog.example.com/security",
                vec!["duckduckgo".to_string()],
                None,
                TrustLevel::ExternalUntrusted,
            ),
            SourceCard::new(
                "NVD Entry",
                "https://nvd.nist.gov/vuln/detail/CVE-2024-0001",
                vec!["duckduckgo".to_string()],
                None,
                TrustLevel::ExternalUntrusted,
            ),
            SourceCard::new(
                "StackOverflow Answer",
                "https://stackoverflow.com/questions/99999",
                vec!["duckduckgo".to_string()],
                None,
                TrustLevel::ExternalUntrusted,
            ),
        ];
        let quality = assess_source_quality(&cards);
        assert_eq!(
            quality.tier,
            SecuritySourceTier::PrimaryAdvisory,
            "should pick the highest tier (PrimaryAdvisory) among mixed sources"
        );
        assert!(
            quality
                .tier_reasons
                .iter()
                .any(|r| r.contains("primary advisory")),
            "reason should mention primary advisory: {:?}",
            quality.tier_reasons
        );
    }

    #[test]
    fn assess_source_quality_mixed_tiers_with_maintainer_discussion() {
        use crate::core::result::TrustLevel;
        use crate::core::SourceCard;

        let cards = vec![
            SourceCard::new(
                "Issue Discussion",
                "https://github.com/foo/bar/issues/123",
                vec!["mock".to_string()],
                None,
                TrustLevel::ExternalUntrusted,
            ),
            SourceCard::new(
                "Blog Post",
                "https://blog.example.com/security",
                vec!["mock".to_string()],
                None,
                TrustLevel::ExternalUntrusted,
            ),
        ];
        let quality = assess_source_quality(&cards);
        assert_eq!(
            quality.tier,
            SecuritySourceTier::MaintainerDiscussion,
            "should pick MaintainerDiscussion over NewsOrBlog"
        );
    }

    #[test]
    fn assess_source_quality_empty_input() {
        use crate::core::SourceCard;

        let cards: Vec<SourceCard> = vec![];
        let quality = assess_source_quality(&cards);
        assert_eq!(
            quality.tier,
            SecuritySourceTier::Unknown,
            "empty input should produce Unknown tier"
        );
    }

    #[test]
    fn security_source_quality_serde_roundtrip() {
        let sq = SecuritySourceQuality {
            tier: SecuritySourceTier::PrimaryAdvisory,
            tier_reasons: vec!["results include primary advisory sources".to_string()],
        };
        let json = serde_json::to_string(&sq).unwrap();
        let parsed: SecuritySourceQuality = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.tier, SecuritySourceTier::PrimaryAdvisory);
        assert_eq!(parsed.tier_reasons.len(), 1);
    }

    #[test]
    fn security_context_no_vulnerabilities_when_empty() {
        let ctx = SecurityContext {
            query_kind: SecurityQueryKind::Package,
            identifiers: vec![],
            affected_packages: vec![],
            vulnerability_summaries: vec![],
            defensive_guidance: vec![],
            source_quality: SecuritySourceQuality {
                tier: SecuritySourceTier::Unknown,
                tier_reasons: vec![],
            },
            warnings: vec![],
        };
        assert!(
            ctx.vulnerability_summaries.is_empty(),
            "vulnerability_summaries should be empty"
        );
        assert!(
            ctx.affected_packages.is_empty(),
            "affected_packages should be empty"
        );
        let json = serde_json::to_value(&ctx).unwrap();
        // Vulnerability summaries is serialized as an empty array (not
        // skipped when empty because the default Vec serializes to [])
        // but may be omitted by skip_serializing_if. Either way the
        // JSON must not claim any vulnerabilities exist.
        if let Some(arr) = json
            .get("vulnerability_summaries")
            .and_then(|v| v.as_array())
        {
            assert!(
                arr.is_empty(),
                "vulnerability_summaries must be empty when no data: {arr:?}"
            );
        }
        if let Some(arr) = json.get("affected_packages").and_then(|v| v.as_array()) {
            assert!(
                arr.is_empty(),
                "affected_packages must be empty when no data: {arr:?}"
            );
        }
    }

    #[test]
    fn security_query_kind_all_variants_as_str() {
        let cases = vec![
            (SecurityQueryKind::Package, "package"),
            (SecurityQueryKind::Cve, "cve"),
            (SecurityQueryKind::Cwe, "cwe"),
            (SecurityQueryKind::Api, "api"),
            (SecurityQueryKind::ErrorMessage, "error_message"),
            (SecurityQueryKind::Concept, "concept"),
            (SecurityQueryKind::Unknown, "unknown"),
        ];
        for (kind, expected) in cases {
            assert_eq!(kind.as_str(), expected);
        }
    }

    #[test]
    fn security_result_group_kind_default_is_other() {
        assert_eq!(
            SecurityResultGroupKind::default(),
            SecurityResultGroupKind::Other
        );
    }
}
