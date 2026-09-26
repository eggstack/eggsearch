#![allow(missing_docs)]

use crate::core::package::PackageEcosystem;
use crate::core::security::VulnerabilityMetadata;
use crate::core::security_applicability::{
    advisory_confidence_for_ranges, compose_confidence, AdvisoryRange, ApplicabilityAssessment,
    ApplicabilityConfidence, ApplicabilityStatus, DependencyFinding, DependencyParserBudget,
    DependencySource, ParseDiagnostic, RangeMatch,
};
use crate::meta::version_compare::{compare_versions_for_ecosystem, version_satisfies_range};

/// Extract advisory ranges from vulnerability metadata.
/// Returns empty vec if no structured ranges can be extracted.
pub fn extract_advisory_ranges(vuln: &VulnerabilityMetadata) -> Vec<AdvisoryRange> {
    let mut ranges = Vec::new();

    let Some(ecosystem) = vuln.ecosystem.as_deref().and_then(PackageEcosystem::parse) else {
        return ranges;
    };
    let package = vuln.package.clone().unwrap_or_default();

    if package.is_empty() {
        return ranges;
    }

    let affected = vuln.affected_ranges.clone();
    let patched = vuln.patched_ranges.clone();
    let vulnerable = vuln.vulnerable_versions.clone();

    if !affected.is_empty() || !patched.is_empty() || !vulnerable.is_empty() {
        ranges.push(AdvisoryRange {
            ecosystem,
            package,
            affected_range: if affected.is_empty() {
                None
            } else {
                Some(affected.join(", "))
            },
            fixed_versions: patched,
            introduced_versions: Vec::new(),
            last_affected_versions: vulnerable,
            source: vuln.source.as_str().to_string(),
        });
    }

    ranges
}

/// Tri-state applicability result for a (version, ranges) pair.
///
/// Use this instead of the legacy `(bool, Vec<String>)` shape when
/// `Unknown` must be distinguished from `NotAffected`.
#[derive(Debug, Clone, PartialEq)]
pub struct ApplicabilityOutcome {
    pub status: ApplicabilityStatus,
    pub reasons: Vec<String>,
    pub matched_ranges: Vec<AdvisoryRange>,
}

impl ApplicabilityOutcome {
    pub fn unknown(reason: impl Into<String>) -> Self {
        Self {
            status: ApplicabilityStatus::Unknown,
            reasons: vec![reason.into()],
            matched_ranges: Vec::new(),
        }
    }
}

/// Check if a version string falls within advisory ranges.
///
/// Returns a tri-state `ApplicabilityOutcome`. Legacy callers that
/// only need a boolean receive `is_affected` here for backward
/// compatibility, but should prefer the tri-state form when
/// `Unknown` matters.
///
/// Rules:
/// - If a version exactly matches a fixed version, status is
///   `NotAffected` (high confidence).
/// - If version matches an explicit last-affected list, status is
///   `Affected`.
/// - If version satisfies an affected range expression, status is
///   `Affected`.
/// - If version is outside a successfully evaluated affected range,
///   status is `NotAffected` for that range.
/// - If no structured range exists, status is `Unknown`.
/// - If range syntax cannot be evaluated, status is `Unknown`.
/// - Across multiple ranges/advisories, any `Affected` dominates
///   `NotAffected`; `Unknown` plus some `NotAffected` collapses to
///   `Unknown` unless every relevant range was evaluated
///   successfully.
pub fn assess_version_applicability(
    version: &str,
    ranges: &[AdvisoryRange],
    ecosystem: &PackageEcosystem,
) -> ApplicabilityOutcome {
    if ranges.is_empty() {
        return ApplicabilityOutcome::unknown(
            "no structured advisory ranges available for comparison",
        );
    }

    let mut combined: Option<RangeMatch> = None;
    let mut reasons = Vec::new();
    let mut matched_ranges = Vec::new();

    for range in ranges {
        let outcome = evaluate_single_range(version, range, ecosystem);
        if let Some(matched) = outcome.matched_range.clone() {
            matched_ranges.push(matched);
        }
        for reason in outcome.reasons {
            reasons.push(reason);
        }
        combined = Some(match combined {
            None => outcome.match_status,
            Some(prev) => prev.combine(outcome.match_status),
        });
    }

    let status = match combined.unwrap_or(RangeMatch::Unknown) {
        RangeMatch::Affected => ApplicabilityStatus::Affected,
        RangeMatch::NotAffected => ApplicabilityStatus::NotAffected,
        RangeMatch::Unknown => ApplicabilityStatus::Unknown,
    };

    ApplicabilityOutcome {
        status,
        reasons,
        matched_ranges,
    }
}

struct SingleRangeOutcome {
    match_status: RangeMatch,
    reasons: Vec<String>,
    matched_range: Option<AdvisoryRange>,
}

fn evaluate_single_range(
    version: &str,
    range: &AdvisoryRange,
    ecosystem: &PackageEcosystem,
) -> SingleRangeOutcome {
    let mut reasons = Vec::new();

    if range.fixed_versions.iter().any(|fixed| fixed == version) {
        reasons.push(format!(
            "version {version} matches fixed version in advisory from {}",
            range.source
        ));
        return SingleRangeOutcome {
            match_status: RangeMatch::NotAffected,
            reasons,
            matched_range: Some(range.clone()),
        };
    }

    if !range.last_affected_versions.is_empty() {
        if range.last_affected_versions.iter().any(|la| la == version) {
            reasons.push(format!(
                "version {version} matches last affected version in advisory from {}",
                range.source
            ));
            return SingleRangeOutcome {
                match_status: RangeMatch::Affected,
                reasons,
                matched_range: Some(range.clone()),
            };
        }
        reasons.push(format!(
            "version {version} not in affected version list from {}",
            range.source
        ));
        return SingleRangeOutcome {
            match_status: RangeMatch::NotAffected,
            reasons,
            matched_range: None,
        };
    }

    if let Some(ref affected_range) = range.affected_range {
        match evaluate_range_expression(version, affected_range, ecosystem) {
            Some(true) => {
                reasons.push(format!(
                    "version {version} matches affected range '{affected_range}' from {}",
                    range.source
                ));
                return SingleRangeOutcome {
                    match_status: RangeMatch::Affected,
                    reasons,
                    matched_range: Some(range.clone()),
                };
            }
            Some(false) => {
                reasons.push(format!(
                    "version {version} outside affected range '{affected_range}' from {}",
                    range.source
                ));
                return SingleRangeOutcome {
                    match_status: RangeMatch::NotAffected,
                    reasons,
                    matched_range: None,
                };
            }
            None => {
                reasons.push(format!(
                    "could not evaluate range '{affected_range}' for version {version} from {}",
                    range.source
                ));
                return SingleRangeOutcome {
                    match_status: RangeMatch::Unknown,
                    reasons,
                    matched_range: None,
                };
            }
        }
    }

    reasons.push(format!(
        "no structured range expression for advisory from {}",
        range.source
    ));
    SingleRangeOutcome {
        match_status: RangeMatch::Unknown,
        reasons,
        matched_range: None,
    }
}

/// Backward-compatible boolean form.
///
/// Returns `(is_affected, reasons)`. Note: this collapses the tri-state
/// result to a boolean. Callers that need to distinguish
/// `NotAffected` from `Unknown` must use
/// [`assess_version_applicability`] instead.
pub fn version_in_ranges(
    version: &str,
    ranges: &[AdvisoryRange],
    ecosystem: &PackageEcosystem,
) -> (bool, Vec<String>) {
    let outcome = assess_version_applicability(version, ranges, ecosystem);
    (
        outcome.status == ApplicabilityStatus::Affected,
        outcome.reasons,
    )
}

/// Evaluate a single comparison clause against a target version.
///
/// Returns:
/// - `Some(true)` if the clause is satisfied.
/// - `Some(false)` if the clause is not satisfied.
/// - `None` if the clause cannot be evaluated (unknown operator, version
///   could not be parsed, etc.).
pub fn evaluate_clause(version: &str, clause: &str, ecosystem: &PackageEcosystem) -> Option<bool> {
    let clause = clause.trim();
    if clause.is_empty() {
        return None;
    }

    if let Some(ver) = clause.strip_prefix(">=") {
        let target = ver.trim();
        let ordering = compare_versions_for_ecosystem(ecosystem, version, target)?;
        return Some(ordering != std::cmp::Ordering::Less);
    }
    if let Some(ver) = clause.strip_prefix('>') {
        let target = ver.trim();
        let ordering = compare_versions_for_ecosystem(ecosystem, version, target)?;
        return Some(ordering == std::cmp::Ordering::Greater);
    }
    if let Some(ver) = clause.strip_prefix("<=") {
        let target = ver.trim();
        let ordering = compare_versions_for_ecosystem(ecosystem, version, target)?;
        return Some(ordering != std::cmp::Ordering::Greater);
    }
    if let Some(ver) = clause.strip_prefix('<') {
        let target = ver.trim();
        let ordering = compare_versions_for_ecosystem(ecosystem, version, target)?;
        return Some(ordering == std::cmp::Ordering::Less);
    }
    if let Some(ver) = clause.strip_prefix("!=") {
        let target = ver.trim();
        let ordering = compare_versions_for_ecosystem(ecosystem, version, target)?;
        return Some(ordering != std::cmp::Ordering::Equal);
    }
    if let Some(ver) = clause.strip_prefix('=') {
        let target = ver.trim();
        let ordering = compare_versions_for_ecosystem(ecosystem, version, target)?;
        return Some(ordering == std::cmp::Ordering::Equal);
    }

    None
}

/// Evaluate a comma-separated range expression.
///
/// A range with no clauses returns `None`. Any unknown clause that
/// cannot be evaluated yields `None` for the whole range unless an
/// earlier clause already produced a definitive `false`.
///
/// This prefers the ecosystem-aware range parser (which knows about
/// Maven qualifiers, OCI exact-match semantics, etc.) and falls back
/// to the clause-by-clause evaluator when the ecosystem parser
/// declines to evaluate the range.
pub fn evaluate_range_expression(
    version: &str,
    range: &str,
    ecosystem: &PackageEcosystem,
) -> Option<bool> {
    if let Some(result) = version_satisfies_range(ecosystem, version, range) {
        return Some(result);
    }

    let mut saw_clause = false;

    for raw in range.split(',') {
        let clause = raw.trim();
        if clause.is_empty() {
            continue;
        }
        saw_clause = true;
        match evaluate_clause(version, clause, ecosystem)? {
            true => continue,
            false => return Some(false),
        }
    }

    if saw_clause {
        Some(true)
    } else {
        None
    }
}

pub fn primary_advisory_id(vuln: &VulnerabilityMetadata) -> String {
    vuln.cve_ids
        .first()
        .or(vuln.ghsa_ids.first())
        .or(vuln.osv_ids.first())
        .or(vuln.rustsec_ids.first())
        .cloned()
        .unwrap_or_default()
}

pub fn request_version_assessment(
    package: &str,
    version: &str,
    ecosystem: PackageEcosystem,
    advisory_id: String,
    outcome: &ApplicabilityOutcome,
    range_count: usize,
    evidence_urls: Vec<String>,
) -> ApplicabilityAssessment {
    let confidence = compose_confidence(
        Some(ApplicabilityConfidence::High),
        advisory_confidence_for_ranges(range_count),
    );
    let mut reasons = outcome.reasons.clone();
    match outcome.status {
        ApplicabilityStatus::Affected => reasons.push(format!(
            "version {version} appears affected by advisory {advisory_id}"
        )),
        ApplicabilityStatus::NotAffected => reasons.push(format!(
            "version {version} does not appear affected by advisory {advisory_id}"
        )),
        ApplicabilityStatus::Unknown => reasons.push(format!(
            "could not determine applicability of version {version} for advisory {advisory_id}"
        )),
        ApplicabilityStatus::InsufficientEvidence => {
            reasons.push("insufficient package/version data to assess applicability".to_string());
        }
    }
    ApplicabilityAssessment {
        status: outcome.status,
        confidence,
        ecosystem,
        package: package.to_string(),
        version: Some(version.to_string()),
        advisory_ids: vec![advisory_id],
        matched_ranges: outcome.matched_ranges.clone(),
        fixed_versions: outcome
            .matched_ranges
            .iter()
            .flat_map(|r| r.fixed_versions.iter().cloned())
            .collect(),
        reasons,
        evidence_urls,
        warnings: Vec::new(),
        version_source: Some(DependencySource::RequestField),
        dependency_relation: None,
        source_ids: Vec::new(),
        fetch_ids: Vec::new(),
    }
}

pub fn resolved_finding_assessment(
    finding: &DependencyFinding,
    version: &str,
    advisory_id: String,
    outcome: &ApplicabilityOutcome,
    range_count: usize,
    evidence_urls: Vec<String>,
) -> ApplicabilityAssessment {
    let confidence = compose_confidence(
        finding.confidence,
        advisory_confidence_for_ranges(range_count),
    );
    let mut reasons = outcome.reasons.clone();
    reasons.push(format!(
        "dependency '{}' resolved version '{}' found in {}",
        finding.package,
        version,
        finding.source_file.as_deref().unwrap_or("unknown")
    ));
    ApplicabilityAssessment {
        status: outcome.status,
        confidence,
        ecosystem: finding.ecosystem.clone(),
        package: finding.package.clone(),
        version: Some(version.to_string()),
        advisory_ids: vec![advisory_id],
        matched_ranges: outcome.matched_ranges.clone(),
        fixed_versions: outcome
            .matched_ranges
            .iter()
            .flat_map(|r| r.fixed_versions.iter().cloned())
            .collect(),
        reasons,
        evidence_urls,
        warnings: Vec::new(),
        version_source: Some(finding.source_kind),
        dependency_relation: finding.relation,
        source_ids: Vec::new(),
        fetch_ids: Vec::new(),
    }
}

pub fn weak_evidence_reason(finding: &DependencyFinding) -> String {
    let location = finding.source_file.as_deref().unwrap_or("unknown");
    if finding.integrity_hash.is_some() {
        format!(
            "dependency '{}' has only integrity/checksum evidence in {}; checksums do not prove an installed version",
            finding.package, location
        )
    } else if let Some(req) = finding.version_requirement.as_deref() {
        format!(
            "dependency '{}' has only a version requirement ({}) in {}; requirements do not prove an installed version",
            finding.package, req, location
        )
    } else if finding.reference_value.is_some() {
        format!(
            "dependency '{}' has only a source reference in {}; references do not prove a resolved package version",
            finding.package, location
        )
    } else {
        format!(
            "dependency '{}' in {} has no resolved version evidence",
            finding.package, location
        )
    }
}

pub fn weak_evidence_assessment(
    finding: &DependencyFinding,
    advisory_id: String,
    evidence_urls: Vec<String>,
) -> ApplicabilityAssessment {
    ApplicabilityAssessment {
        status: ApplicabilityStatus::Unknown,
        confidence: compose_confidence(finding.confidence, ApplicabilityConfidence::Low),
        ecosystem: finding.ecosystem.clone(),
        package: finding.package.clone(),
        version: finding.version.clone(),
        advisory_ids: vec![advisory_id],
        matched_ranges: Vec::new(),
        fixed_versions: Vec::new(),
        reasons: vec![weak_evidence_reason(finding)],
        evidence_urls,
        warnings: vec!["weak_evidence_ignored_for_exact_applicability".to_string()],
        version_source: Some(finding.source_kind),
        dependency_relation: finding.relation,
        source_ids: Vec::new(),
        fetch_ids: Vec::new(),
    }
}

pub fn cap_file_list(
    files: &[String],
    max: usize,
) -> (&[String], Option<crate::core::result::SearchWarning>) {
    if files.len() <= max {
        return (files, None);
    }
    let skipped = &files[max..];
    let first_skipped = skipped.first().map(String::as_str).unwrap_or("unknown");
    (
        &files[..max],
        Some(crate::core::result::SearchWarning::new(
            "_system",
            format!(
                "dependency_file_budget_exceeded: kept {max} files in request order; {first_skipped} and later files were not read"
            ),
        )),
    )
}

pub fn assemble_dependency_evidence(
    entries: Vec<(String, String)>,
    budget: DependencyParserBudget,
) -> (
    Vec<DependencyFinding>,
    Vec<crate::core::result::SearchWarning>,
) {
    let mut findings = Vec::new();
    let mut warnings = Vec::new();
    for (path, content) in &entries {
        let report = crate::meta::dependency_parse::parse_dependency_file_report(path, content);
        for diagnostic in &report.diagnostics {
            warnings.push(parse_diagnostic_warning(path, diagnostic));
        }
        findings.extend(report.findings);
    }
    let (findings, aggregate_note) = crate::core::security_applicability::truncate_aggregate(
        findings,
        budget.max_findings_per_request,
    );
    if let Some(note) = aggregate_note {
        warnings.push(parse_diagnostic_warning("aggregate", &note));
    }
    (findings, warnings)
}

pub fn parse_diagnostic_warning(
    path: &str,
    diagnostic: &ParseDiagnostic,
) -> crate::core::result::SearchWarning {
    let code = match diagnostic.code.as_str() {
        "dependency_parse_malformed" => "dependency_parse_malformed",
        "dependency_format_unsupported" => "dependency_format_unsupported",
        "dependency_parse_partial" => "dependency_parse_partial",
        "dependency_finding_budget_exceeded" => "dependency_finding_budget_exceeded",
        _ => "dependency_parse_partial",
    };
    let location = short_path(path);
    let mut message = format!("{code}: {location}: {}", diagnostic.message);
    if let Some(line) = diagnostic.line {
        message.push_str(&format!(" (line {line})"));
    }
    crate::core::result::SearchWarning::new("_system", message)
}

fn short_path(path: &str) -> String {
    const MAX_PATH_CHARS: usize = 200;
    if path.chars().count() <= MAX_PATH_CHARS {
        path.to_string()
    } else {
        path.chars().take(MAX_PATH_CHARS).collect()
    }
}

pub fn weak_evidence_summary(weak_count: usize) -> Option<crate::core::result::SearchWarning> {
    if weak_count == 0 {
        return None;
    }
    Some(crate::core::result::SearchWarning::new(
        "_system",
        format!(
            "weak_evidence_ignored_for_exact_applicability: {weak_count} requirement, reference, or integrity observations were assessed as Unknown rather than Affected or NotAffected"
        ),
    ))
}

pub fn build_finding_index(
    findings: &[DependencyFinding],
) -> std::collections::HashMap<(String, String), Vec<usize>> {
    let mut index: std::collections::HashMap<(String, String), Vec<usize>> =
        std::collections::HashMap::new();
    for (position, finding) in findings.iter().enumerate() {
        index
            .entry(index_key(&finding.ecosystem, &finding.package))
            .or_default()
            .push(position);
    }
    index
}

fn index_key(ecosystem: &PackageEcosystem, package: &str) -> (String, String) {
    (
        ecosystem.as_str().to_string(),
        crate::core::security_applicability::canonical_package_name(ecosystem, package),
    )
}

pub fn finding_assessment_fingerprint(finding: &DependencyFinding) -> String {
    format!(
        "{}|{}|{:?}|{}|{}|{:?}",
        finding.provenance.as_deref().unwrap_or(""),
        finding.target_context.as_deref().unwrap_or(""),
        finding.reference_kind,
        finding.reference_value.as_deref().unwrap_or(""),
        finding.source_file.as_deref().unwrap_or(""),
        finding.source_kind
    )
}

pub fn matching_positions(
    vuln: &VulnerabilityMetadata,
    index: &std::collections::HashMap<(String, String), Vec<usize>>,
) -> Vec<usize> {
    let vuln_package = vuln.package.as_deref().unwrap_or("");
    let vuln_ecosystem = vuln.ecosystem.as_deref().unwrap_or("");
    let Some(ecosystem) = PackageEcosystem::parse(vuln_ecosystem) else {
        return Vec::new();
    };
    index
        .get(&index_key(&ecosystem, vuln_package))
        .cloned()
        .unwrap_or_default()
}

pub fn explicit_request_identity_ecosystem(
    request_package: &str,
    request_ecosystem: Option<&str>,
    vuln_package: Option<&str>,
    vuln_ecosystem: Option<&str>,
) -> Option<PackageEcosystem> {
    let advisory_ecosystem = vuln_ecosystem.and_then(PackageEcosystem::parse)?;
    if let Some(request_ecosystem) = request_ecosystem {
        let parsed_request = PackageEcosystem::parse(request_ecosystem)?;
        if parsed_request != advisory_ecosystem {
            return None;
        }
    }
    let advisory_package = vuln_package.unwrap_or_default();
    if advisory_package.is_empty() {
        return None;
    }
    if !crate::core::security_applicability::packages_match(
        &advisory_ecosystem,
        advisory_package,
        request_package,
    ) {
        return None;
    }
    Some(advisory_ecosystem)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::security::{SeverityLevel, VulnerabilitySource};

    fn make_vuln(affected: Vec<&str>, patched: Vec<&str>) -> VulnerabilityMetadata {
        VulnerabilityMetadata {
            cve_ids: vec!["CVE-2024-0001".to_string()],
            ghsa_ids: Vec::new(),
            osv_ids: Vec::new(),
            rustsec_ids: Vec::new(),
            ecosystem: Some("crates_io".to_string()),
            package: Some("test-crate".to_string()),
            affected_ranges: affected.into_iter().map(String::from).collect(),
            patched_ranges: patched.into_iter().map(String::from).collect(),
            vulnerable_versions: Vec::new(),
            patched_versions: Vec::new(),
            severity: Some(SeverityLevel::High),
            cvss_score: None,
            cvss_vector: None,
            epss_score: None,
            kev: None,
            published_at: None,
            modified_at: None,
            withdrawn_at: None,
            references: Vec::new(),
            source: VulnerabilitySource::Osv,
        }
    }

    fn make_range(
        ecosystem: PackageEcosystem,
        affected_range: Option<&str>,
        fixed_versions: Vec<&str>,
        last_affected: Vec<&str>,
    ) -> AdvisoryRange {
        AdvisoryRange {
            ecosystem,
            package: "test".to_string(),
            affected_range: affected_range.map(String::from),
            fixed_versions: fixed_versions.into_iter().map(String::from).collect(),
            introduced_versions: Vec::new(),
            last_affected_versions: last_affected.into_iter().map(String::from).collect(),
            source: "test".to_string(),
        }
    }

    #[test]
    fn extract_ranges_from_metadata() {
        let vuln = make_vuln(vec![" >= 1.0.0"], vec!["1.2.3"]);
        let ranges = extract_advisory_ranges(&vuln);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].fixed_versions, vec!["1.2.3"]);
    }

    #[test]
    fn version_below_introduced_not_affected() {
        let ranges = vec![make_range(
            PackageEcosystem::CratesIo,
            Some(">= 2.0.0, < 3.0.0"),
            vec!["3.0.0"],
            vec![],
        )];
        let outcome = assess_version_applicability("1.5.0", &ranges, &PackageEcosystem::CratesIo);
        assert_eq!(outcome.status, ApplicabilityStatus::NotAffected);
    }

    #[test]
    fn version_in_affected_range() {
        let ranges = vec![make_range(
            PackageEcosystem::CratesIo,
            Some(">= 1.0.0, < 2.0.0"),
            vec!["2.0.0"],
            vec![],
        )];
        let outcome = assess_version_applicability("1.5.0", &ranges, &PackageEcosystem::CratesIo);
        assert_eq!(outcome.status, ApplicabilityStatus::Affected);
    }

    #[test]
    fn version_equal_to_fixed_not_affected() {
        let ranges = vec![make_range(
            PackageEcosystem::CratesIo,
            None,
            vec!["2.0.0"],
            vec![],
        )];
        let outcome = assess_version_applicability("2.0.0", &ranges, &PackageEcosystem::CratesIo);
        assert_eq!(outcome.status, ApplicabilityStatus::NotAffected);
    }

    #[test]
    fn empty_ranges_return_unknown() {
        let outcome = assess_version_applicability("1.0.0", &[], &PackageEcosystem::CratesIo);
        assert_eq!(outcome.status, ApplicabilityStatus::Unknown);
        assert!(outcome
            .reasons
            .iter()
            .any(|r| r.contains("no structured advisory ranges")));
    }

    #[test]
    fn last_affected_version_is_affected() {
        let ranges = vec![make_range(
            PackageEcosystem::Npm,
            None,
            vec![],
            vec!["1.2.3"],
        )];
        let outcome = assess_version_applicability("1.2.3", &ranges, &PackageEcosystem::Npm);
        assert_eq!(outcome.status, ApplicabilityStatus::Affected);
    }

    #[test]
    fn last_affected_version_not_matching() {
        let ranges = vec![make_range(
            PackageEcosystem::Npm,
            None,
            vec![],
            vec!["1.2.3"],
        )];
        let outcome = assess_version_applicability("1.2.4", &ranges, &PackageEcosystem::Npm);
        assert_eq!(outcome.status, ApplicabilityStatus::NotAffected);
    }

    #[test]
    fn maven_range_evaluation() {
        let ranges = vec![make_range(
            PackageEcosystem::Maven,
            Some(">= 2.0.0"),
            vec!["2.5.0"],
            vec![],
        )];
        let outcome = assess_version_applicability("2.3.0", &ranges, &PackageEcosystem::Maven);
        assert_eq!(outcome.status, ApplicabilityStatus::Affected);
    }

    // ===== WS1: operator-by-operator table-driven tests =====

    fn assert_clause(
        range: &str,
        version: &str,
        expected: Option<bool>,
        ecosystem: &PackageEcosystem,
    ) {
        let actual = evaluate_clause(version, range, ecosystem);
        assert_eq!(
            actual, expected,
            "evaluate_clause(version={version}, range=\"{range}\") returned {actual:?}, expected {expected:?}"
        );
    }

    #[test]
    fn operator_ge_correctness() {
        let eco = PackageEcosystem::CratesIo;
        assert_clause(">= 2.0.0", "1.9.9", Some(false), &eco);
        assert_clause(">= 2.0.0", "2.0.0", Some(true), &eco);
        assert_clause(">= 2.0.0", "2.1.0", Some(true), &eco);
    }

    #[test]
    fn operator_gt_correctness() {
        let eco = PackageEcosystem::CratesIo;
        assert_clause("> 2.0.0", "2.0.0", Some(false), &eco);
        assert_clause("> 2.0.0", "2.0.1", Some(true), &eco);
    }

    #[test]
    fn operator_le_correctness() {
        let eco = PackageEcosystem::CratesIo;
        assert_clause("<= 2.0.0", "1.9.9", Some(true), &eco);
        assert_clause("<= 2.0.0", "2.0.0", Some(true), &eco);
        assert_clause("<= 2.0.0", "2.0.1", Some(false), &eco);
    }

    #[test]
    fn operator_lt_correctness() {
        let eco = PackageEcosystem::CratesIo;
        assert_clause("< 2.0.0", "1.9.9", Some(true), &eco);
        assert_clause("< 2.0.0", "2.0.0", Some(false), &eco);
    }

    #[test]
    fn operator_eq_correctness() {
        let eco = PackageEcosystem::CratesIo;
        assert_clause("= 2.0.0", "2.0.0", Some(true), &eco);
        assert_clause("= 2.0.0", "2.0.1", Some(false), &eco);
    }

    #[test]
    fn range_expression_intersection() {
        let eco = PackageEcosystem::CratesIo;
        assert_eq!(
            evaluate_range_expression("1.9.9", ">= 2.0.0, < 3.0.0", &eco),
            Some(false)
        );
        assert_eq!(
            evaluate_range_expression("2.5.0", ">= 2.0.0, < 3.0.0", &eco),
            Some(true)
        );
        assert_eq!(
            evaluate_range_expression("3.0.0", ">= 2.0.0, < 3.0.0", &eco),
            Some(false)
        );
    }

    #[test]
    fn unknown_range_syntax_returns_unknown_status() {
        let ranges = vec![make_range(
            PackageEcosystem::CratesIo,
            Some("banana"),
            vec![],
            vec![],
        )];
        let outcome = assess_version_applicability("1.0.0", &ranges, &PackageEcosystem::CratesIo);
        assert_eq!(outcome.status, ApplicabilityStatus::Unknown);
    }

    #[test]
    fn mixed_known_unknown_clauses_yield_unknown() {
        let eco = PackageEcosystem::CratesIo;
        assert_eq!(
            evaluate_range_expression("1.5.0", ">= 1.0.0, banana", &eco),
            None
        );
    }

    #[test]
    fn unknown_clause_after_definitive_false_stays_not_affected() {
        // If a known clause already fails, subsequent unknown clauses
        // do not promote the result to Unknown — we already know the
        // version is outside the range.
        let eco = PackageEcosystem::CratesIo;
        assert_eq!(
            evaluate_range_expression("0.5.0", ">= 1.0.0, banana", &eco),
            Some(false)
        );
    }

    #[test]
    fn empty_range_returns_none() {
        assert_eq!(
            evaluate_range_expression("1.0.0", "", &PackageEcosystem::CratesIo),
            None
        );
        assert_eq!(
            evaluate_range_expression("1.0.0", "   ", &PackageEcosystem::CratesIo),
            None
        );
    }

    #[test]
    fn multiple_ranges_any_affected_dominates() {
        let ranges = vec![
            make_range(PackageEcosystem::CratesIo, Some(">= 5.0.0"), vec![], vec![]),
            make_range(
                PackageEcosystem::CratesIo,
                Some(">= 1.0.0, < 2.0.0"),
                vec![],
                vec![],
            ),
        ];
        let outcome = assess_version_applicability("1.5.0", &ranges, &PackageEcosystem::CratesIo);
        assert_eq!(outcome.status, ApplicabilityStatus::Affected);
    }

    #[test]
    fn multiple_ranges_one_unknown_collapses_to_unknown() {
        let ranges = vec![
            make_range(PackageEcosystem::CratesIo, Some(">= 5.0.0"), vec![], vec![]),
            make_range(PackageEcosystem::CratesIo, Some("banana"), vec![], vec![]),
        ];
        let outcome = assess_version_applicability("1.5.0", &ranges, &PackageEcosystem::CratesIo);
        assert_eq!(outcome.status, ApplicabilityStatus::Unknown);
    }

    #[test]
    fn multiple_ranges_all_not_affected_stays_not_affected() {
        let ranges = vec![
            make_range(PackageEcosystem::CratesIo, Some(">= 5.0.0"), vec![], vec![]),
            make_range(
                PackageEcosystem::CratesIo,
                Some(">= 3.0.0, < 4.0.0"),
                vec![],
                vec![],
            ),
        ];
        let outcome = assess_version_applicability("1.5.0", &ranges, &PackageEcosystem::CratesIo);
        assert_eq!(outcome.status, ApplicabilityStatus::NotAffected);
    }

    #[test]
    fn range_match_combine_rules() {
        assert_eq!(
            RangeMatch::Affected.combine(RangeMatch::NotAffected),
            RangeMatch::Affected
        );
        assert_eq!(
            RangeMatch::NotAffected.combine(RangeMatch::Affected),
            RangeMatch::Affected
        );
        assert_eq!(
            RangeMatch::NotAffected.combine(RangeMatch::NotAffected),
            RangeMatch::NotAffected
        );
        assert_eq!(
            RangeMatch::Unknown.combine(RangeMatch::NotAffected),
            RangeMatch::Unknown
        );
        assert_eq!(
            RangeMatch::Unknown.combine(RangeMatch::Affected),
            RangeMatch::Affected
        );
    }

    // ===== WS1 regression: the original inverted bug =====

    #[test]
    fn regression_ge_does_not_invert() {
        // Before the fix, the legacy `evaluate_range` returned true for
        // versions LESS THAN the floor. This is the exact regression
        // we are guarding against.
        let ranges = vec![make_range(
            PackageEcosystem::CratesIo,
            Some(">= 2.0.0"),
            vec![],
            vec![],
        )];
        let outcome = assess_version_applicability("1.0.0", &ranges, &PackageEcosystem::CratesIo);
        assert_eq!(outcome.status, ApplicabilityStatus::NotAffected);
        assert_eq!(outcome.status, ApplicabilityStatus::NotAffected);
    }

    // ===== WS3: OSV/RustSec affected/fixed range fixture hardening =====

    #[test]
    fn osv_introduced_fixed_range_vulnerable() {
        // OSV introduced/fixed events: >= 2.0.0, < 3.0.0 with patched 3.0.0
        let ranges = vec![make_range(
            PackageEcosystem::CratesIo,
            Some(">= 2.0.0, < 3.0.0"),
            vec!["3.0.0"],
            vec![],
        )];
        let outcome = assess_version_applicability("2.5.0", &ranges, &PackageEcosystem::CratesIo);
        assert_eq!(outcome.status, ApplicabilityStatus::Affected);
    }

    #[test]
    fn osv_introduced_fixed_range_fixed() {
        let ranges = vec![make_range(
            PackageEcosystem::CratesIo,
            Some(">= 2.0.0, < 3.0.0"),
            vec!["3.0.0"],
            vec![],
        )];
        let outcome = assess_version_applicability("3.0.0", &ranges, &PackageEcosystem::CratesIo);
        assert_eq!(outcome.status, ApplicabilityStatus::NotAffected);
    }

    #[test]
    fn osv_introduced_fixed_range_below() {
        let ranges = vec![make_range(
            PackageEcosystem::CratesIo,
            Some(">= 2.0.0, < 3.0.0"),
            vec!["3.0.0"],
            vec![],
        )];
        let outcome = assess_version_applicability("1.5.0", &ranges, &PackageEcosystem::CratesIo);
        assert_eq!(outcome.status, ApplicabilityStatus::NotAffected);
    }

    #[test]
    fn osv_explicit_affected_version_list_match() {
        let ranges = vec![make_range(
            PackageEcosystem::CratesIo,
            None,
            vec![],
            vec!["1.2.3", "1.2.4"],
        )];
        let outcome = assess_version_applicability("1.2.3", &ranges, &PackageEcosystem::CratesIo);
        assert_eq!(outcome.status, ApplicabilityStatus::Affected);
    }

    #[test]
    fn osv_explicit_affected_version_list_match_second() {
        let ranges = vec![make_range(
            PackageEcosystem::CratesIo,
            None,
            vec![],
            vec!["1.2.3", "1.2.4"],
        )];
        let outcome = assess_version_applicability("1.2.4", &ranges, &PackageEcosystem::CratesIo);
        assert_eq!(outcome.status, ApplicabilityStatus::Affected);
    }

    #[test]
    fn osv_explicit_affected_version_list_not_match_above() {
        let ranges = vec![make_range(
            PackageEcosystem::CratesIo,
            None,
            vec![],
            vec!["1.2.3", "1.2.4"],
        )];
        let outcome = assess_version_applicability("1.2.5", &ranges, &PackageEcosystem::CratesIo);
        assert_eq!(outcome.status, ApplicabilityStatus::NotAffected);
    }

    #[test]
    fn osv_explicit_affected_version_list_not_match_below() {
        let ranges = vec![make_range(
            PackageEcosystem::CratesIo,
            None,
            vec![],
            vec!["1.2.3", "1.2.4"],
        )];
        let outcome = assess_version_applicability("1.2.2", &ranges, &PackageEcosystem::CratesIo);
        assert_eq!(outcome.status, ApplicabilityStatus::NotAffected);
    }

    #[test]
    fn rustsec_patched_range_affected() {
        // RustSec-style: affected < 0.7.4, >= 0.5.0; patched at 0.7.4
        let ranges = vec![make_range(
            PackageEcosystem::CratesIo,
            Some(">= 0.5.0, < 0.7.4"),
            vec!["0.7.4"],
            vec![],
        )];
        let outcome = assess_version_applicability("0.6.0", &ranges, &PackageEcosystem::CratesIo);
        assert_eq!(outcome.status, ApplicabilityStatus::Affected);
    }

    #[test]
    fn rustsec_patched_range_fixed() {
        let ranges = vec![make_range(
            PackageEcosystem::CratesIo,
            Some(">= 0.5.0, < 0.7.4"),
            vec!["0.7.4"],
            vec![],
        )];
        let outcome = assess_version_applicability("0.7.4", &ranges, &PackageEcosystem::CratesIo);
        assert_eq!(outcome.status, ApplicabilityStatus::NotAffected);
    }

    #[test]
    fn rustsec_patched_range_above() {
        let ranges = vec![make_range(
            PackageEcosystem::CratesIo,
            Some(">= 0.5.0, < 0.7.4"),
            vec!["0.7.4"],
            vec![],
        )];
        let outcome = assess_version_applicability("0.8.0", &ranges, &PackageEcosystem::CratesIo);
        assert_eq!(outcome.status, ApplicabilityStatus::NotAffected);
    }

    #[test]
    fn unsupported_osv_git_range_returns_unknown() {
        let ranges = vec![make_range(
            PackageEcosystem::CratesIo,
            Some("GIT:abc123def"),
            vec![],
            vec![],
        )];
        let outcome = assess_version_applicability("1.0.0", &ranges, &PackageEcosystem::CratesIo);
        assert_eq!(outcome.status, ApplicabilityStatus::Unknown);
        assert!(outcome
            .reasons
            .iter()
            .any(|r| r.contains("could not evaluate range")));
    }

    // ===== WS3: OSV multiple affected packages =====

    #[test]
    fn osv_multiple_affected_packages_one_matches() {
        // Advisory affects both "serde" and "serde_derive" with different ranges.
        // Version 1.5.0 is inside serde's range but outside serde_derive's range.
        let serde_range = AdvisoryRange {
            ecosystem: PackageEcosystem::CratesIo,
            package: "serde".to_string(),
            affected_range: Some(">= 1.0.0, < 2.0.0".to_string()),
            fixed_versions: vec!["2.0.0".to_string()],
            introduced_versions: Vec::new(),
            last_affected_versions: Vec::new(),
            source: "GHSA-test-multi".to_string(),
        };
        let derive_range = AdvisoryRange {
            ecosystem: PackageEcosystem::CratesIo,
            package: "serde_derive".to_string(),
            affected_range: Some(">= 3.0.0, < 4.0.0".to_string()),
            fixed_versions: vec!["4.0.0".to_string()],
            introduced_versions: Vec::new(),
            last_affected_versions: Vec::new(),
            source: "GHSA-test-multi".to_string(),
        };
        // 1.5.0 is inside serde's >= 1.0.0, < 2.0.0 → Affected
        let outcome = assess_version_applicability(
            "1.5.0",
            &[serde_range, derive_range],
            &PackageEcosystem::CratesIo,
        );
        assert_eq!(outcome.status, ApplicabilityStatus::Affected);
    }

    #[test]
    fn osv_multiple_affected_packages_neither_matches() {
        // Version 2.5.0 is outside both ranges → NotAffected
        let serde_range = AdvisoryRange {
            ecosystem: PackageEcosystem::CratesIo,
            package: "serde".to_string(),
            affected_range: Some(">= 1.0.0, < 2.0.0".to_string()),
            fixed_versions: vec!["2.0.0".to_string()],
            introduced_versions: Vec::new(),
            last_affected_versions: Vec::new(),
            source: "GHSA-test-multi".to_string(),
        };
        let derive_range = AdvisoryRange {
            ecosystem: PackageEcosystem::CratesIo,
            package: "serde_derive".to_string(),
            affected_range: Some(">= 3.0.0, < 4.0.0".to_string()),
            fixed_versions: vec!["4.0.0".to_string()],
            introduced_versions: Vec::new(),
            last_affected_versions: Vec::new(),
            source: "GHSA-test-multi".to_string(),
        };
        let outcome = assess_version_applicability(
            "2.5.0",
            &[serde_range, derive_range],
            &PackageEcosystem::CratesIo,
        );
        assert_eq!(outcome.status, ApplicabilityStatus::NotAffected);
    }

    #[test]
    fn osv_multiple_affected_packages_both_match() {
        // Version 1.5.0 is inside both ranges → Affected
        let serde_range = AdvisoryRange {
            ecosystem: PackageEcosystem::CratesIo,
            package: "serde".to_string(),
            affected_range: Some(">= 1.0.0, < 2.0.0".to_string()),
            fixed_versions: vec!["2.0.0".to_string()],
            introduced_versions: Vec::new(),
            last_affected_versions: Vec::new(),
            source: "GHSA-test-multi-a".to_string(),
        };
        let derive_range = AdvisoryRange {
            ecosystem: PackageEcosystem::CratesIo,
            package: "serde_derive".to_string(),
            affected_range: Some(">= 1.0.0, < 2.0.0".to_string()),
            fixed_versions: vec!["2.0.0".to_string()],
            introduced_versions: Vec::new(),
            last_affected_versions: Vec::new(),
            source: "GHSA-test-multi-b".to_string(),
        };
        let outcome = assess_version_applicability(
            "1.5.0",
            &[serde_range, derive_range],
            &PackageEcosystem::CratesIo,
        );
        assert_eq!(outcome.status, ApplicabilityStatus::Affected);
    }

    #[test]
    fn explicit_request_go_case_variant_does_not_match() {
        let matched = explicit_request_identity_ecosystem(
            "example.com/mod",
            Some("go"),
            Some("Example.com/Mod"),
            Some("go"),
        );
        assert_eq!(matched, None);
    }

    #[test]
    fn explicit_request_go_exact_spelling_matches() {
        let matched = explicit_request_identity_ecosystem(
            "Example.com/Mod",
            Some("go"),
            Some("Example.com/Mod"),
            Some("go"),
        );
        assert_eq!(matched, Some(PackageEcosystem::Go));
    }

    #[test]
    fn explicit_request_pypi_normalization_matches() {
        assert_eq!(
            explicit_request_identity_ecosystem(
                "my-package",
                Some("pypi"),
                Some("my_package"),
                Some("pypi"),
            ),
            Some(PackageEcosystem::Pypi)
        );
        assert_eq!(
            explicit_request_identity_ecosystem(
                "MY-PACKAGE",
                Some("pypi"),
                Some("my_package"),
                Some("pypi"),
            ),
            Some(PackageEcosystem::Pypi)
        );
        assert_eq!(
            explicit_request_identity_ecosystem(
                "my.package",
                Some("pypi"),
                Some("my-package"),
                Some("pypi"),
            ),
            Some(PackageEcosystem::Pypi)
        );
    }

    #[test]
    fn explicit_request_nuget_case_variant_matches() {
        let matched = explicit_request_identity_ecosystem(
            "newtonsoft.json",
            Some("nuget"),
            Some("Newtonsoft.Json"),
            Some("nuget"),
        );
        assert_eq!(matched, Some(PackageEcosystem::Nuget));
    }

    #[test]
    fn explicit_request_exact_identity_ecosystems_reject_case_variants() {
        assert_eq!(
            explicit_request_identity_ecosystem("Lodash", Some("npm"), Some("lodash"), Some("npm"),),
            None
        );
        assert_eq!(
            explicit_request_identity_ecosystem(
                "example-crate",
                Some("crates_io"),
                Some("Example_Crate"),
                Some("crates_io"),
            ),
            None
        );
    }

    #[test]
    fn explicit_request_mismatched_ecosystem_does_not_match() {
        let matched = explicit_request_identity_ecosystem(
            "lodash",
            Some("npm"),
            Some("lodash"),
            Some("pypi"),
        );
        assert_eq!(matched, None);
    }

    #[test]
    fn explicit_request_unknown_advisory_ecosystem_does_not_match() {
        assert_eq!(
            explicit_request_identity_ecosystem(
                "serde",
                Some("crates_io"),
                Some("serde"),
                Some("terraform"),
            ),
            None
        );
        assert_eq!(
            explicit_request_identity_ecosystem("serde", None, Some("serde"), Some("terraform"),),
            None
        );
        assert_eq!(
            explicit_request_identity_ecosystem("serde", Some("crates_io"), Some("serde"), None,),
            None
        );
    }

    #[test]
    fn explicit_request_invalid_caller_ecosystem_does_not_match() {
        let matched = explicit_request_identity_ecosystem(
            "serde",
            Some("not-an-ecosystem"),
            Some("serde"),
            Some("crates_io"),
        );
        assert_eq!(matched, None);
    }

    #[test]
    fn explicit_request_omitted_caller_ecosystem_uses_advisory_ecosystem() {
        let matched =
            explicit_request_identity_ecosystem("serde", None, Some("serde"), Some("crates_io"));
        assert_eq!(matched, Some(PackageEcosystem::CratesIo));
    }
}
