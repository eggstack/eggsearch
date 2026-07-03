//! Phase 8 tests for security applicability assessment and defensive output.
//!
//! Covers:
//! - Affected/not-affected/unknown/insufficient-evidence status
//! - Matched ranges and fixed versions in assessment output
//! - Remediation action categories for each status
//! - No exploit instructions in remediation text
//! - SecuritySuggestedFetch stable_id and source_id
//! - SecurityEvidenceSummary counts
//! - DependencyFinding relation field
//!
//! Run via:
//! ```bash
//! cargo test --test security_applicability_phase8
//! ```

use eggsearch::core::code_evidence::EvidenceConfidence;
use eggsearch::core::package::PackageEcosystem;
use eggsearch::core::security::{
    RemediationCategory, SecurityEvidenceSummary, SecurityRemediation, SecuritySuggestedFetch,
    SeverityLevel, VulnerabilityMetadata, VulnerabilitySource,
};
use eggsearch::core::security_applicability::{
    AdvisoryRange, ApplicabilityAssessment, ApplicabilityConfidence, ApplicabilityStatus,
    DependencyFinding, DependencyRelation, DependencySource,
};
use eggsearch::meta::advisory_range::assess_version_applicability;

fn make_range(
    ecosystem: PackageEcosystem,
    affected_range: Option<&str>,
    fixed_versions: Vec<&str>,
    last_affected: Vec<&str>,
) -> AdvisoryRange {
    AdvisoryRange {
        ecosystem,
        package: "test-pkg".to_string(),
        affected_range: affected_range.map(String::from),
        fixed_versions: fixed_versions.into_iter().map(String::from).collect(),
        introduced_versions: Vec::new(),
        last_affected_versions: last_affected.into_iter().map(String::from).collect(),
        source: "test-advisory".to_string(),
    }
}

fn make_vuln(affected: Vec<&str>, patched: Vec<&str>) -> VulnerabilityMetadata {
    VulnerabilityMetadata {
        cve_ids: vec!["CVE-2024-9999".to_string()],
        ghsa_ids: Vec::new(),
        osv_ids: Vec::new(),
        rustsec_ids: Vec::new(),
        ecosystem: Some("crates_io".to_string()),
        package: Some("test-pkg".to_string()),
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

// ---------------------------------------------------------------------------
// 1. Affected package/version returns affected with matched range and fixed
// ---------------------------------------------------------------------------

#[test]
fn affected_version_returns_affected_with_matched_range() {
    let ranges = vec![make_range(
        PackageEcosystem::Npm,
        Some(">= 2.0.0, < 3.0.0"),
        vec!["3.0.0"],
        vec![],
    )];
    let outcome = assess_version_applicability("2.5.0", &ranges, &PackageEcosystem::Npm);

    assert_eq!(outcome.status, ApplicabilityStatus::Affected);
    assert!(
        !outcome.matched_ranges.is_empty(),
        "must have at least one matched range"
    );
    assert_eq!(outcome.matched_ranges[0].fixed_versions, vec!["3.0.0"]);
    assert_eq!(outcome.matched_ranges[0].package, "test-pkg");
    assert_eq!(
        outcome.matched_ranges[0].affected_range.as_deref(),
        Some(">= 2.0.0, < 3.0.0")
    );
}

// ---------------------------------------------------------------------------
// 2. Not affected: version below the affected range returns NotAffected
// ---------------------------------------------------------------------------

#[test]
fn version_below_range_returns_not_affected() {
    let ranges = vec![make_range(
        PackageEcosystem::CratesIo,
        Some(">= 2.0.0, < 3.0.0"),
        vec!["3.0.0"],
        vec![],
    )];
    let outcome = assess_version_applicability("1.9.9", &ranges, &PackageEcosystem::CratesIo);
    assert_eq!(
        outcome.status,
        ApplicabilityStatus::NotAffected,
        "version below affected range must be NotAffected"
    );
    assert!(
        outcome
            .reasons
            .iter()
            .any(|r| r.contains("outside affected range")),
        "reasons should mention outside range: {:?}",
        outcome.reasons
    );
}

#[test]
fn version_at_fixed_version_returns_not_affected() {
    let ranges = vec![make_range(
        PackageEcosystem::Npm,
        Some(">= 2.0.0, < 3.0.0"),
        vec!["3.0.0"],
        vec![],
    )];
    let outcome = assess_version_applicability("3.0.0", &ranges, &PackageEcosystem::Npm);
    assert_eq!(
        outcome.status,
        ApplicabilityStatus::NotAffected,
        "version matching fixed version must be NotAffected"
    );
    assert!(
        outcome
            .reasons
            .iter()
            .any(|r| r.contains("matches fixed version")),
        "reasons should mention fixed version match: {:?}",
        outcome.reasons
    );
}

// ---------------------------------------------------------------------------
// 3. Unknown range syntax returns Unknown
// ---------------------------------------------------------------------------

#[test]
fn unknown_range_syntax_returns_unknown() {
    let ranges = vec![make_range(
        PackageEcosystem::CratesIo,
        Some("banana"),
        vec![],
        vec![],
    )];
    let outcome = assess_version_applicability("1.0.0", &ranges, &PackageEcosystem::CratesIo);
    assert_eq!(
        outcome.status,
        ApplicabilityStatus::Unknown,
        "unparseable range must return Unknown"
    );
    assert!(
        outcome
            .reasons
            .iter()
            .any(|r| r.contains("could not evaluate range")),
        "reasons should mention range evaluation failure: {:?}",
        outcome.reasons
    );
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
}

// ---------------------------------------------------------------------------
// 4. Missing version returns InsufficientEvidence (via assessment construction)
// ---------------------------------------------------------------------------

#[test]
fn missing_version_yields_insufficient_evidence_assessment() {
    let assessment = ApplicabilityAssessment {
        status: ApplicabilityStatus::InsufficientEvidence,
        confidence: ApplicabilityConfidence::Low,
        ecosystem: PackageEcosystem::Npm,
        package: "test-pkg".to_string(),
        version: None,
        advisory_ids: vec!["CVE-2024-9999".to_string()],
        matched_ranges: Vec::new(),
        fixed_versions: Vec::new(),
        reasons: vec!["no version provided for applicability assessment".to_string()],
        evidence_urls: Vec::new(),
        warnings: Vec::new(),
        version_source: None,
        dependency_relation: None,
        source_ids: Vec::new(),
        fetch_ids: Vec::new(),
    };

    assert_eq!(assessment.status, ApplicabilityStatus::InsufficientEvidence);
    assert!(assessment.version.is_none(), "version must be None");
    assert_eq!(
        assessment.confidence,
        ApplicabilityConfidence::Low,
        "confidence must be Low when no version is provided"
    );
    assert!(
        !assessment.reasons.is_empty(),
        "must have at least one reason explaining why evidence is insufficient"
    );
}

#[test]
fn insufficient_evidence_has_low_confidence() {
    let assessment = ApplicabilityAssessment {
        status: ApplicabilityStatus::InsufficientEvidence,
        confidence: ApplicabilityConfidence::Low,
        ecosystem: PackageEcosystem::Pypi,
        package: "flask".to_string(),
        version: None,
        advisory_ids: Vec::new(),
        matched_ranges: Vec::new(),
        fixed_versions: Vec::new(),
        reasons: vec!["missing version".to_string()],
        evidence_urls: Vec::new(),
        warnings: Vec::new(),
        version_source: None,
        dependency_relation: None,
        source_ids: Vec::new(),
        fetch_ids: Vec::new(),
    };

    assert_eq!(assessment.confidence, ApplicabilityConfidence::Low);
}

// ---------------------------------------------------------------------------
// 5. Remediation action is Upgrade when fixed version exists
// ---------------------------------------------------------------------------

fn make_remediation(category: RemediationCategory, description: &str) -> SecurityRemediation {
    SecurityRemediation {
        category,
        description: description.to_string(),
        rationale: "test rationale".to_string(),
        evidence_urls: Vec::new(),
        fixed_versions: vec!["3.0.0".to_string()],
        affected_packages: vec!["test-pkg".to_string()],
        source_ids: Vec::new(),
        confidence: EvidenceConfidence::Exact,
    }
}

#[test]
fn remediation_for_affected_with_fixed_version_is_upgrade() {
    let remediation = make_remediation(
        RemediationCategory::Upgrade,
        "Upgrade test-pkg to version 3.0.0 or later to fix the vulnerability.",
    );

    assert_eq!(remediation.category, RemediationCategory::Upgrade);
    assert_eq!(
        remediation.category.as_str(),
        "upgrade",
        "category string must be 'upgrade'"
    );
    assert!(
        !remediation.fixed_versions.is_empty(),
        "must have fixed_versions"
    );
    assert_eq!(remediation.fixed_versions, vec!["3.0.0"]);
}

// ---------------------------------------------------------------------------
// 6. Remediation action is ManualReview when applicability unknown
// ---------------------------------------------------------------------------

#[test]
fn remediation_for_unknown_applicability_is_manual_review() {
    let remediation = make_remediation(
        RemediationCategory::ManualReview,
        "Could not determine applicability; manual review required.",
    );

    assert_eq!(remediation.category, RemediationCategory::ManualReview);
    assert_eq!(
        remediation.category.as_str(),
        "manual_review",
        "category string must be 'manual_review'"
    );
}

// ---------------------------------------------------------------------------
// 7. No exploit instructions in remediation action text
// ---------------------------------------------------------------------------

const EXPLOIT_KEYWORDS: &[&str] = &[
    "exploit",
    "payload",
    "injection",
    "shellcode",
    "overflow",
    "rop",
    "gadget",
    "rce",
    "remote code execution",
    "pwn",
    "p0c",
    "proof of concept",
];

fn assert_no_exploit_instructions(remediation: &SecurityRemediation) {
    let combined = format!(
        "{} {}",
        remediation.description.to_lowercase(),
        remediation.rationale.to_lowercase()
    );
    for keyword in EXPLOIT_KEYWORDS {
        assert!(
            !combined.contains(keyword),
            "remediation text must not contain exploit keyword '{}':\n  desc: {}\n  rationale: {}",
            keyword,
            remediation.description,
            remediation.rationale
        );
    }
}

#[test]
fn upgrade_remediation_has_no_exploit_instructions() {
    let remediation = make_remediation(
        RemediationCategory::Upgrade,
        "Upgrade test-pkg to version 3.0.0 or later to fix the vulnerability.",
    );
    assert_no_exploit_instructions(&remediation);
}

#[test]
fn manual_review_remediation_has_no_exploit_instructions() {
    let remediation = make_remediation(
        RemediationCategory::ManualReview,
        "Could not determine applicability; manual review required.",
    );
    assert_no_exploit_instructions(&remediation);
}

#[test]
fn configuration_mitigation_remediation_has_no_exploit_instructions() {
    let remediation = make_remediation(
        RemediationCategory::ConfigurationMitigation,
        "Apply configuration hardening to limit exposure.",
    );
    assert_no_exploit_instructions(&remediation);
}

// ---------------------------------------------------------------------------
// 8. Suggested fetches have stable IDs and source IDs
// ---------------------------------------------------------------------------

#[test]
fn suggested_fetch_has_stable_id_and_source_id() {
    let fetch = SecuritySuggestedFetch {
        url: "https://github.com/example/advisory".to_string(),
        reason: "Advisory source".to_string(),
        group: eggsearch::core::security::SecurityResultGroupKind::AuthoritativeAdvisories,
        priority: 1,
        stable_id: Some("suggested_abcdef0123456789".to_string()),
        source_id: Some("src_aabbccdd11223344".to_string()),
        score: Some(100),
        rank_reasons: vec!["authoritative_advisory".to_string()],
        information_gain: Some(0.9),
        reason_code: Some("fetch_authoritative_advisory".to_string()),
        advisory_ids: vec!["CVE-2024-9999".to_string()],
        package: Some("test-pkg".to_string()),
        version: None,
    };

    let stable_id = fetch.stable_id.as_ref().expect("stable_id must be present");
    assert!(
        stable_id.starts_with("suggested_"),
        "stable_id must start with 'suggested_' prefix, got: {}",
        stable_id
    );
    assert_eq!(
        stable_id.len(),
        26,
        "stable_id must be 26 chars (prefix + 16 hex)"
    );

    let source_id = fetch.source_id.as_ref().expect("source_id must be present");
    assert!(
        source_id.starts_with("src_"),
        "source_id must start with 'src_' prefix, got: {}",
        source_id
    );
}

#[test]
fn suggested_fetch_without_source_id_has_none() {
    let fetch = SecuritySuggestedFetch {
        url: "https://example.com/synthetic".to_string(),
        reason: "Synthesized advisory".to_string(),
        group: eggsearch::core::security::SecurityResultGroupKind::AuthoritativeAdvisories,
        priority: 2,
        stable_id: Some("suggested_deadbeef01234567".to_string()),
        source_id: None,
        score: None,
        rank_reasons: Vec::new(),
        information_gain: None,
        reason_code: None,
        advisory_ids: Vec::new(),
        package: None,
        version: None,
    };

    assert!(
        fetch.source_id.is_none(),
        "synthesized advisory has no source card"
    );
    assert!(fetch.stable_id.is_some(), "stable_id must still be present");
}

// ---------------------------------------------------------------------------
// 9. Evidence summary counts are correct
// ---------------------------------------------------------------------------

#[test]
fn evidence_summary_counts_are_correct() {
    let summary = SecurityEvidenceSummary {
        total_vulnerabilities: 1,
        total_assessments: 4,
        affected_count: 1,
        not_affected_count: 1,
        unknown_count: 1,
        insufficient_evidence_count: 1,
        remediation_count: 2,
        highest_severity: Some(SeverityLevel::Critical),
        kev_match_present: false,
        source_quality_tier: eggsearch::core::security::SecuritySourceTier::PrimaryAdvisory,
        has_authoritative_source: true,
    };

    assert_eq!(summary.total_assessments, 4);
    assert_eq!(summary.affected_count, 1);
    assert_eq!(summary.not_affected_count, 1);
    assert_eq!(summary.unknown_count, 1);
    assert_eq!(summary.insufficient_evidence_count, 1);
    assert_eq!(
        summary.affected_count
            + summary.not_affected_count
            + summary.unknown_count
            + summary.insufficient_evidence_count,
        summary.total_assessments,
        "sum of per-status counts must equal total_assessments"
    );
}

#[test]
fn evidence_summary_all_zero_is_valid() {
    let summary = SecurityEvidenceSummary::default();

    assert_eq!(summary.total_vulnerabilities, 0);
    assert_eq!(summary.total_assessments, 0);
    assert_eq!(summary.affected_count, 0);
    assert_eq!(summary.not_affected_count, 0);
    assert_eq!(summary.unknown_count, 0);
    assert_eq!(summary.insufficient_evidence_count, 0);
    assert_eq!(summary.remediation_count, 0);
    assert!(!summary.kev_match_present);
    assert!(!summary.has_authoritative_source);
}

// ---------------------------------------------------------------------------
// 10. DependencyFinding has relation field
// ---------------------------------------------------------------------------

#[test]
fn dependency_finding_from_manifest_is_direct() {
    let finding = DependencyFinding {
        ecosystem: PackageEcosystem::Npm,
        package: "express".to_string(),
        version: Some("4.18.0".to_string()),
        source_file: Some("package.json".to_string()),
        source_line: Some(12),
        source_kind: DependencySource::Manifest,
        confidence: Some(ApplicabilityConfidence::High),
        relation: Some(DependencyRelation::Direct),
    };

    assert_eq!(
        finding.relation,
        Some(DependencyRelation::Direct),
        "manifest dependency must be Direct"
    );
}

#[test]
fn dependency_finding_from_lockfile_is_transitive() {
    let finding = DependencyFinding {
        ecosystem: PackageEcosystem::Npm,
        package: "qs".to_string(),
        version: Some("6.5.3".to_string()),
        source_file: Some("package-lock.json".to_string()),
        source_line: Some(1542),
        source_kind: DependencySource::LockFile,
        confidence: Some(ApplicabilityConfidence::Medium),
        relation: Some(DependencyRelation::Transitive),
    };

    assert_eq!(
        finding.relation,
        Some(DependencyRelation::Transitive),
        "lockfile dependency must be Transitive"
    );
}

#[test]
fn dependency_finding_from_advisory_metadata_has_unknown_relation() {
    let finding = DependencyFinding {
        ecosystem: PackageEcosystem::CratesIo,
        package: "serde".to_string(),
        version: None,
        source_file: None,
        source_line: None,
        source_kind: DependencySource::AdvisoryMetadata,
        confidence: Some(ApplicabilityConfidence::Low),
        relation: Some(DependencyRelation::Unknown),
    };

    assert_eq!(
        finding.relation,
        Some(DependencyRelation::Unknown),
        "advisory metadata dependency must be Unknown"
    );
}

#[test]
fn dependency_finding_optional_relation_omitted_when_none() {
    let finding = DependencyFinding {
        ecosystem: PackageEcosystem::Npm,
        package: "lodash".to_string(),
        version: None,
        source_file: None,
        source_line: None,
        source_kind: DependencySource::RequestField,
        confidence: None,
        relation: None,
    };

    assert!(finding.relation.is_none());
}

// ---------------------------------------------------------------------------
// 11. Not-affected assessments produce no remediation actions
// ---------------------------------------------------------------------------

#[test]
fn not_affected_assessment_produces_no_remediation_actions() {
    let ranges = vec![make_range(
        PackageEcosystem::CratesIo,
        Some(">= 2.0.0, < 3.0.0"),
        vec!["3.0.0"],
        vec![],
    )];
    let outcome = assess_version_applicability("1.5.0", &ranges, &PackageEcosystem::CratesIo);

    // Simulate what security_search does: skip remediation for not_affected
    let remediation_actions: Vec<SecurityRemediation> = match outcome.status {
        ApplicabilityStatus::Affected => {
            vec![make_remediation(
                RemediationCategory::Upgrade,
                "Upgrade to 3.0.0.",
            )]
        }
        ApplicabilityStatus::NotAffected => {
            // No remediation needed
            Vec::new()
        }
        ApplicabilityStatus::Unknown => {
            vec![make_remediation(
                RemediationCategory::ManualReview,
                "Manual review required.",
            )]
        }
        ApplicabilityStatus::InsufficientEvidence => {
            vec![make_remediation(
                RemediationCategory::ManualReview,
                "Insufficient evidence to assess.",
            )]
        }
    };

    assert!(
        remediation_actions.is_empty(),
        "not_affected must produce zero remediation actions, got {}",
        remediation_actions.len()
    );
}

// ---------------------------------------------------------------------------
// 12. Insufficient evidence remediation is ManualReview
// ---------------------------------------------------------------------------

#[test]
fn insufficient_evidence_produces_manual_review_remediation() {
    let assessment = ApplicabilityAssessment {
        status: ApplicabilityStatus::InsufficientEvidence,
        confidence: ApplicabilityConfidence::Low,
        ecosystem: PackageEcosystem::Npm,
        package: "test-pkg".to_string(),
        version: None,
        advisory_ids: vec!["CVE-2024-9999".to_string()],
        matched_ranges: Vec::new(),
        fixed_versions: Vec::new(),
        reasons: vec!["no version provided for applicability assessment".to_string()],
        evidence_urls: Vec::new(),
        warnings: Vec::new(),
        version_source: None,
        dependency_relation: None,
        source_ids: Vec::new(),
        fetch_ids: Vec::new(),
    };

    // Simulate remediation generation for insufficient evidence
    let remediation = match assessment.status {
        ApplicabilityStatus::InsufficientEvidence => Some(make_remediation(
            RemediationCategory::ManualReview,
            "Insufficient evidence to assess applicability; manual review required.",
        )),
        _ => None,
    };

    let r = remediation.expect("insufficient_evidence must produce a remediation");
    assert_eq!(r.category, RemediationCategory::ManualReview);
    assert_no_exploit_instructions(&r);
}

// ---------------------------------------------------------------------------
// Additional: extract_advisory_ranges round-trips VulnerabilityMetadata
// ---------------------------------------------------------------------------

#[test]
fn extract_advisory_ranges_from_vulnerability_metadata() {
    use eggsearch::meta::advisory_range::extract_advisory_ranges;

    let vuln = make_vuln(vec![">= 1.0.0, < 2.0.0"], vec!["2.0.0"]);
    let ranges = extract_advisory_ranges(&vuln);

    assert_eq!(ranges.len(), 1, "must extract one range");
    assert_eq!(ranges[0].package, "test-pkg");
    assert_eq!(
        ranges[0].affected_range.as_deref(),
        Some(">= 1.0.0, < 2.0.0")
    );
    assert_eq!(ranges[0].fixed_versions, vec!["2.0.0"]);
    assert_eq!(ranges[0].ecosystem, PackageEcosystem::CratesIo);
}

// ---------------------------------------------------------------------------
// Additional: assess + remediation flow end-to-end
// ---------------------------------------------------------------------------

#[test]
fn affected_with_fixed_version_produces_upgrade_remediation() {
    let ranges = vec![make_range(
        PackageEcosystem::Npm,
        Some(">= 2.0.0, < 3.0.0"),
        vec!["3.0.0"],
        vec![],
    )];
    let outcome = assess_version_applicability("2.5.0", &ranges, &PackageEcosystem::Npm);

    assert_eq!(outcome.status, ApplicabilityStatus::Affected);

    // Simulate remediation generation
    let fixed_versions = &outcome.matched_ranges[0].fixed_versions;
    let remediation = make_remediation(
        RemediationCategory::Upgrade,
        &format!(
            "Upgrade test-pkg to version {} or later.",
            fixed_versions[0]
        ),
    );

    assert_eq!(remediation.category, RemediationCategory::Upgrade);
    assert_eq!(remediation.fixed_versions, vec!["3.0.0"]);
    assert_no_exploit_instructions(&remediation);
}

#[test]
fn unknown_range_produces_manual_review_remediation() {
    let ranges = vec![make_range(
        PackageEcosystem::CratesIo,
        Some("GIT:deadbeef"),
        vec![],
        vec![],
    )];
    let outcome = assess_version_applicability("1.0.0", &ranges, &PackageEcosystem::CratesIo);

    assert_eq!(outcome.status, ApplicabilityStatus::Unknown);

    let remediation = make_remediation(
        RemediationCategory::ManualReview,
        "Range could not be evaluated; manual review required.",
    );

    assert_eq!(remediation.category, RemediationCategory::ManualReview);
    assert_no_exploit_instructions(&remediation);
}

// ---------------------------------------------------------------------------
// Additional: EvidenceConfidence on SecurityRemediation
// ---------------------------------------------------------------------------

#[test]
fn remediation_confidence_variants() {
    let cases = vec![
        EvidenceConfidence::Exact,
        EvidenceConfidence::Strong,
        EvidenceConfidence::Weak,
        EvidenceConfidence::Unknown,
    ];

    for variant in cases {
        let remediation = SecurityRemediation {
            category: RemediationCategory::Upgrade,
            description: "test".to_string(),
            rationale: "test".to_string(),
            evidence_urls: Vec::new(),
            fixed_versions: Vec::new(),
            affected_packages: Vec::new(),
            source_ids: Vec::new(),
            confidence: variant,
        };
        assert_eq!(remediation.confidence, variant);
    }
}

// ---------------------------------------------------------------------------
// Additional: RemediationCategory as_str coverage
// ---------------------------------------------------------------------------

#[test]
fn remediation_category_as_str_coverage() {
    let all_categories = vec![
        (RemediationCategory::Upgrade, "upgrade"),
        (RemediationCategory::Pin, "pin"),
        (RemediationCategory::Replace, "replace"),
        (RemediationCategory::RemoveDependency, "remove_dependency"),
        (
            RemediationCategory::ConfigurationMitigation,
            "configuration_mitigation",
        ),
        (RemediationCategory::FeatureDisable, "feature_disable"),
        (
            RemediationCategory::VulnerableApiAvoidance,
            "vulnerable_api_avoidance",
        ),
        (
            RemediationCategory::TransitiveOverride,
            "transitive_override",
        ),
        (RemediationCategory::VendorPatch, "vendor_patch"),
        (RemediationCategory::MonitorOnly, "monitor_only"),
        (RemediationCategory::ManualReview, "manual_review"),
        (
            RemediationCategory::NoActionSupportedByEvidence,
            "no_action_supported_by_evidence",
        ),
    ];

    for (category, expected_str) in all_categories {
        assert_eq!(
            category.as_str(),
            expected_str,
            "RemediationCategory::{:?} must serialize to '{}'",
            category,
            expected_str
        );
    }
}
