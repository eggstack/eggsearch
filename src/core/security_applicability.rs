#![allow(missing_docs)]

use serde::{Deserialize, Serialize};

use crate::core::package::PackageEcosystem;

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ApplicabilityStatus {
    Affected,
    NotAffected,
    /// Advisory range syntax, ecosystem mapping, package aliasing, or
    /// version parsing prevents a firm answer.
    #[default]
    Unknown,
    /// The query lacks package/version/dependency data needed to assess
    /// applicability.
    InsufficientEvidence,
}

/// Internal tri-state result for advisory range evaluation.
///
/// This preserves the distinction between `NotAffected` (the advisory
/// explicitly excludes this version) and `Unknown` (the advisory could
/// not be evaluated for this version, e.g. unparseable range syntax
/// or unsupported range type). Collapsing unknown into not-affected
/// produces dangerous false-negatives for security triage.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RangeMatch {
    Affected,
    NotAffected,
    #[default]
    Unknown,
}

impl RangeMatch {
    pub fn is_affected(self) -> bool {
        matches!(self, RangeMatch::Affected)
    }

    /// Combine two range-match results using the rules:
    /// - `Affected` dominates everything.
    /// - All `NotAffected` resolves to `NotAffected` (every range was
    ///   evaluated and excluded this version).
    /// - Any `Unknown` mixed with `NotAffected` resolves to `Unknown`
    ///   (some range could not be evaluated, so we cannot conclude
    ///   the version is safe).
    /// - All `Unknown` stays `Unknown`.
    pub fn combine(self, other: RangeMatch) -> RangeMatch {
        match (self, other) {
            (RangeMatch::Affected, _) | (_, RangeMatch::Affected) => RangeMatch::Affected,
            (RangeMatch::NotAffected, RangeMatch::NotAffected) => RangeMatch::NotAffected,
            (RangeMatch::NotAffected, RangeMatch::Unknown)
            | (RangeMatch::Unknown, RangeMatch::NotAffected) => RangeMatch::Unknown,
            (RangeMatch::Unknown, RangeMatch::Unknown) => RangeMatch::Unknown,
        }
    }

    pub fn from_satisfied(satisfied: Option<bool>) -> RangeMatch {
        match satisfied {
            Some(true) => RangeMatch::Affected,
            Some(false) => RangeMatch::NotAffected,
            None => RangeMatch::Unknown,
        }
    }
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ApplicabilityConfidence {
    High,
    Medium,
    #[default]
    Low,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DependencySource {
    LockFile,
    Manifest,
    Dockerfile,
    WorkflowFile,
    AdvisoryMetadata,
    #[default]
    RequestField,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AdvisoryRange {
    pub ecosystem: PackageEcosystem,
    pub package: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affected_range: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixed_versions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub introduced_versions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub last_affected_versions: Vec<String>,
    pub source: String,
}

/// Whether a dependency is direct or transitive.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DependencyRelation {
    Direct,
    Transitive,
    #[default]
    Unknown,
}

/// Kind of a dependency source reference when the finding records a
/// reference (VCS ref, tag, digest, local path, expression) rather than a
/// resolved package version.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DependencyReferenceKind {
    Git,
    Path,
    Url,
    Tag,
    Branch,
    Commit,
    Digest,
    Local,
    Workspace,
    Registry,
    Expression,
    #[default]
    Unknown,
}

/// Completeness of a single dependency-file parse.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ParseStatus {
    Complete,
    Partial,
    Unsupported,
    #[default]
    Malformed,
}

/// Machine-readable diagnostic attached to a dependency parse report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ParseDiagnostic {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
}

/// Findings plus completeness information for one dependency file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DependencyParseReport {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<DependencyFinding>,
    pub status: ParseStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ParseDiagnostic>,
}

impl DependencyParseReport {
    pub fn complete(findings: Vec<DependencyFinding>) -> Self {
        Self {
            findings,
            status: ParseStatus::Complete,
            diagnostics: Vec::new(),
        }
    }

    pub fn partial(findings: Vec<DependencyFinding>, diagnostics: Vec<ParseDiagnostic>) -> Self {
        Self {
            findings,
            status: ParseStatus::Partial,
            diagnostics,
        }
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self {
            findings: Vec::new(),
            status: ParseStatus::Unsupported,
            diagnostics: vec![ParseDiagnostic {
                code: "dependency_format_unsupported".to_string(),
                message: message.into(),
                line: None,
            }],
        }
    }

    pub fn malformed(message: impl Into<String>) -> Self {
        Self {
            findings: Vec::new(),
            status: ParseStatus::Malformed,
            diagnostics: vec![ParseDiagnostic {
                code: "dependency_parse_malformed".to_string(),
                message: message.into(),
                line: None,
            }],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DependencyFinding {
    pub ecosystem: PackageEcosystem,
    pub package: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_line: Option<u32>,
    pub source_kind: DependencySource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<ApplicabilityConfidence>,
    /// Whether this is a direct or transitive dependency.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation: Option<DependencyRelation>,
    /// Exact resolved/selected version. Only this field (never the legacy
    /// `version` projection) authorizes range-based applicability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_version: Option<String>,
    /// Declared version requirement/request text (manifest constraint).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_requirement: Option<String>,
    /// Kind of source reference when the finding records a reference
    /// rather than a version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_kind: Option<DependencyReferenceKind>,
    /// Reference value (commit SHA, tag, digest, URL, path, expression).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_value: Option<String>,
    /// Provenance/source locator or source class (registry name, VCS URL,
    /// path scope, custom source marker).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<String>,
    /// Target/environment context (framework, runtime identifier, marker).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_context: Option<String>,
    /// Integrity/checksum observation. Never resolved-version evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrity_hash: Option<String>,
}

impl DependencyFinding {
    /// Exact resolved version eligible for range-based applicability.
    pub fn exact_version(&self) -> Option<&str> {
        self.resolved_version.as_deref()
    }

    /// Whether this finding carries evidence that actually proves a
    /// resolved version (as opposed to a requirement, reference, or
    /// integrity observation).
    pub fn has_resolved_evidence(&self) -> bool {
        self.resolved_version
            .as_deref()
            .is_some_and(|v| !v.is_empty())
    }
}

/// Combine dependency-evidence confidence with advisory/range confidence.
///
/// The result never exceeds either side: High requires High on both.
pub fn compose_confidence(
    dependency: Option<ApplicabilityConfidence>,
    advisory: ApplicabilityConfidence,
) -> ApplicabilityConfidence {
    fn rank(c: ApplicabilityConfidence) -> u8 {
        match c {
            ApplicabilityConfidence::High => 3,
            ApplicabilityConfidence::Medium => 2,
            ApplicabilityConfidence::Low => 1,
        }
    }
    let dep = dependency.unwrap_or(ApplicabilityConfidence::Low);
    if rank(dep) <= rank(advisory) {
        dep
    } else {
        advisory
    }
}

/// Advisory-side confidence for a set of extracted ranges.
pub fn advisory_confidence_for_ranges(range_count: usize) -> ApplicabilityConfidence {
    if range_count > 0 {
        ApplicabilityConfidence::High
    } else {
        ApplicabilityConfidence::Low
    }
}

/// Canonicalize a package name for comparison using ecosystem rules.
///
/// PyPI names are lowercased with runs of `-`, `_`, `.` collapsed to `-`.
/// All other ecosystems use a conservative case-insensitive comparison
/// with the display name preserved in findings.
pub fn canonical_package_name(ecosystem: &PackageEcosystem, name: &str) -> String {
    if *ecosystem == PackageEcosystem::Pypi {
        let mut out = String::with_capacity(name.len());
        let mut prev_dash = false;
        for c in name.chars() {
            if c == '-' || c == '_' || c == '.' {
                if !prev_dash {
                    out.push('-');
                    prev_dash = true;
                }
            } else {
                prev_dash = false;
                out.extend(c.to_lowercase());
            }
        }
        out
    } else {
        name.to_string()
    }
}

/// Conservative ecosystem-aware package identity comparison.
pub fn packages_match(ecosystem: &PackageEcosystem, a: &str, b: &str) -> bool {
    if *ecosystem == PackageEcosystem::Pypi {
        canonical_package_name(ecosystem, a) == canonical_package_name(ecosystem, b)
    } else {
        a.eq_ignore_ascii_case(b)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ApplicabilityAssessment {
    pub status: ApplicabilityStatus,
    pub confidence: ApplicabilityConfidence,
    pub ecosystem: PackageEcosystem,
    pub package: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub advisory_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub matched_ranges: Vec<AdvisoryRange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixed_versions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_urls: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// Source of the dependency version (lockfile, manifest, request field, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_source: Option<DependencySource>,
    /// Whether this is a direct or transitive dependency.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependency_relation: Option<DependencyRelation>,
    /// Source card IDs this assessment is linked to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_ids: Vec<String>,
    /// Fetch item IDs this assessment is linked to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fetch_ids: Vec<String>,
}

/// Aggregate resource policy for dependency evidence collection.
///
/// Limits are constants so truncation boundaries are testable and
/// stable. The per-file finding cap sits above the maximum finding
/// density of a 1 MiB input at realistic entry sizes; the aggregate
/// cap bounds the applicability cross-product.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DependencyParserBudget {
    pub max_files_per_request: usize,
    pub max_findings_per_file: usize,
    pub max_findings_per_request: usize,
    pub max_diagnostics: usize,
    pub max_diagnostic_chars: usize,
    pub max_nesting_depth: usize,
}

impl DependencyParserBudget {
    pub const fn standard() -> Self {
        Self {
            max_files_per_request: 32,
            max_findings_per_file: 10_000,
            max_findings_per_request: 20_000,
            max_diagnostics: 32,
            max_diagnostic_chars: 300,
            max_nesting_depth: 64,
        }
    }
}

/// Truncate a finding list to the per-file cap, preserving stable source
/// order. Returns the kept findings plus a budget diagnostic when the
/// cap applied.
pub fn truncate_findings(
    findings: Vec<DependencyFinding>,
    max: usize,
) -> (Vec<DependencyFinding>, Option<ParseDiagnostic>) {
    if findings.len() <= max {
        return (findings, None);
    }
    let mut kept = findings;
    kept.truncate(max);
    (
        kept,
        Some(ParseDiagnostic {
            code: "dependency_finding_budget_exceeded".to_string(),
            message: format!("finding budget exceeded: kept {max} findings in stable source order"),
            line: None,
        }),
    )
}

/// Truncate a finding list to the aggregate request cap, preserving
/// stable collection order.
pub fn truncate_aggregate(
    findings: Vec<DependencyFinding>,
    max: usize,
) -> (Vec<DependencyFinding>, Option<ParseDiagnostic>) {
    if findings.len() <= max {
        return (findings, None);
    }
    let mut kept = findings;
    kept.truncate(max);
    (
        kept,
        Some(ParseDiagnostic {
            code: "dependency_finding_budget_exceeded".to_string(),
            message: format!(
                "aggregate finding budget exceeded: kept {max} findings in stable collection order"
            ),
            line: None,
        }),
    )
}

/// Bound a parse report: findings to the per-file cap, diagnostics to
/// the count cap, and diagnostic text to the character cap.
pub fn truncate_report(
    mut report: DependencyParseReport,
    budget: DependencyParserBudget,
) -> DependencyParseReport {
    let (findings, budget_note) = truncate_findings(report.findings, budget.max_findings_per_file);
    report.findings = findings;
    if let Some(note) = budget_note {
        report.diagnostics.push(note);
        report.status = ParseStatus::Partial;
    }
    if report.diagnostics.len() > budget.max_diagnostics {
        report.diagnostics.truncate(budget.max_diagnostics);
        report.status = ParseStatus::Partial;
    }
    for diagnostic in &mut report.diagnostics {
        if diagnostic.message.chars().count() > budget.max_diagnostic_chars {
            let truncated: String = diagnostic
                .message
                .chars()
                .take(budget.max_diagnostic_chars)
                .collect();
            diagnostic.message = truncated;
        }
        if let Some(line) = diagnostic.line {
            if line == 0 {
                diagnostic.line = None;
            }
        }
    }
    report
}
