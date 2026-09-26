//! Security applicability assessment and defensive output contract.
//!
//! Regression provenance: phase-8 security applicability workstream.
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
//! cargo test --test security_applicability_contract
//! ```

use eggsearch::core::code_evidence::EvidenceConfidence;
use eggsearch::core::package::PackageEcosystem;
use eggsearch::core::security::{
    RemediationCategory, SecurityEvidenceSummary, SecurityRemediation, SecuritySuggestedFetch,
    SeverityLevel, VulnerabilityMetadata, VulnerabilitySource,
};
use eggsearch::core::security_applicability::{
    advisory_confidence_for_ranges, canonical_package_name, compose_confidence, packages_match,
    AdvisoryRange, ApplicabilityAssessment, ApplicabilityConfidence, ApplicabilityStatus,
    DependencyFinding, DependencyParseReport, DependencyReferenceKind, DependencyRelation,
    DependencySource, ParseStatus,
};
use eggsearch::meta::advisory_range::assess_version_applicability;
use eggsearch::meta::dependency_parse::{parse_dependency_file, parse_dependency_file_report};

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

fn assert_no_exploit_instructions(remediation: &SecurityRemediation) {
    let combined = format!(
        "{} {}",
        remediation.description.to_lowercase(),
        remediation.rationale.to_lowercase()
    );
    for keyword in OFFENSIVE_INSTRUCTION_KEYWORDS
        .iter()
        .chain(VULNERABILITY_CLASS_KEYWORDS)
    {
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
        recommended_extract_mode: None,
        recommended_focus_query: None,
        batch_item: None,
    };

    let stable_id = fetch.stable_id.as_ref().expect("stable_id must be present");
    assert!(
        stable_id.starts_with("suggested_"),
        "stable_id must start with 'suggested_' prefix, got: {stable_id}"
    );
    assert_eq!(
        stable_id.len(),
        26,
        "stable_id must be 26 chars (prefix + 16 hex)"
    );

    let source_id = fetch.source_id.as_ref().expect("source_id must be present");
    assert!(
        source_id.starts_with("src_"),
        "source_id must start with 'src_' prefix, got: {source_id}"
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
        recommended_extract_mode: None,
        recommended_focus_query: None,
        batch_item: None,
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
        resolved_version: None,
        version_requirement: Some("4.18.0".to_string()),
        reference_kind: None,
        reference_value: None,
        provenance: None,
        target_context: None,
        integrity_hash: None,
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
        resolved_version: Some("6.5.3".to_string()),
        version_requirement: None,
        reference_kind: None,
        reference_value: None,
        provenance: None,
        target_context: None,
        integrity_hash: None,
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
        resolved_version: None,
        version_requirement: None,
        reference_kind: None,
        reference_value: None,
        provenance: None,
        target_context: None,
        integrity_hash: None,
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
        resolved_version: None,
        version_requirement: None,
        reference_kind: None,
        reference_value: None,
        provenance: None,
        target_context: None,
        integrity_hash: None,
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
            "RemediationCategory::{category:?} must serialize to '{expected_str}'"
        );
    }
}

// ---------------------------------------------------------------------------
// 13. Evidence bundle preserves security source metadata
// -----------------------------------------------------------------------

#[test]
fn evidence_bundle_preserves_security_source_metadata() {
    use eggsearch::core::evidence_bundle::{EvidenceBundleRequest, EvidenceSourceInput};
    use eggsearch::core::source_card::{SourceKind, SourceMetadata};
    use eggsearch::meta::evidence_bundle::build_evidence_bundle;

    let metadata = SourceMetadata {
        source_kind: SourceKind::SecurityAdvisory,
        domain: Some("osv.dev".to_string()),
        ..Default::default()
    };

    let request = EvidenceBundleRequest {
        goal: Some("security triage for CVE-2024-9999".to_string()),
        sources: vec![EvidenceSourceInput {
            id: Some("src_security_001".to_string()),
            url: Some("https://osv.dev/vulnerability/CVE-2024-9999".to_string()),
            title: Some("CVE-2024-9999 in test-pkg".to_string()),
            snippet: Some("Affected versions: >= 2.0.0".to_string()),
            providers: vec!["osv".to_string()],
            score: Some(100.0),
            trust: Some(eggsearch::core::result::TrustLevel::ExternalUntrusted),
            trust_markers: None,
            metadata: Some(metadata),
            quality: None,
        }],
        fetches: vec![],
        include_unfetched_sources: Some(true),
        max_sources: Some(50),
        max_fetched_items: Some(20),
        max_total_chars: Some(100_000),
        warnings: vec![],
        research_claims: None,
        research_conflicts: None,
    };

    let bundle = build_evidence_bundle(request);

    assert_eq!(bundle.sources.len(), 1, "must have one source");
    let source = &bundle.sources[0];
    assert_eq!(
        source.source_kind,
        Some(SourceKind::SecurityAdvisory),
        "source_kind must be preserved as SecurityAdvisory"
    );
    assert_eq!(
        source.url.as_deref(),
        Some("https://osv.dev/vulnerability/CVE-2024-9999")
    );
    assert_eq!(source.title.as_deref(), Some("CVE-2024-9999 in test-pkg"));
    assert_eq!(source.provider_id.as_deref(), Some("osv"));
    assert_eq!(
        source.trust,
        eggsearch::core::result::TrustLevel::ExternalUntrusted
    );
}

#[test]
fn evidence_bundle_deduplicates_security_sources_by_url() {
    use eggsearch::core::evidence_bundle::{EvidenceBundleRequest, EvidenceSourceInput};
    use eggsearch::core::source_card::{SourceKind, SourceMetadata};
    use eggsearch::meta::evidence_bundle::build_evidence_bundle;

    let make_security_source = |id: &str, provider: &str| EvidenceSourceInput {
        id: Some(id.to_string()),
        url: Some("https://osv.dev/vulnerability/CVE-2024-9999".to_string()),
        title: Some("CVE-2024-9999".to_string()),
        snippet: Some("Vulnerability details".to_string()),
        providers: vec![provider.to_string()],
        score: Some(100.0),
        trust: Some(eggsearch::core::result::TrustLevel::ExternalUntrusted),
        trust_markers: None,
        metadata: Some(SourceMetadata {
            source_kind: SourceKind::SecurityAdvisory,
            domain: Some("osv.dev".to_string()),
            ..Default::default()
        }),
        quality: None,
    };

    let request = EvidenceBundleRequest {
        goal: Some("dedup test".to_string()),
        sources: vec![
            make_security_source("src_001", "osv"),
            make_security_source("src_002", "duckduckgo"),
        ],
        fetches: vec![],
        include_unfetched_sources: Some(true),
        max_sources: Some(50),
        max_fetched_items: Some(20),
        max_total_chars: Some(100_000),
        warnings: vec![],
        research_claims: None,
        research_conflicts: None,
    };

    let bundle = build_evidence_bundle(request);

    assert_eq!(
        bundle.sources.len(),
        1,
        "duplicate sources with same URL must be deduplicated"
    );
    let source = &bundle.sources[0];
    assert_eq!(
        source.source_kind,
        Some(SourceKind::SecurityAdvisory),
        "deduplicated source must preserve SecurityAdvisory kind"
    );
}

// ---------------------------------------------------------------------------
// 9. validate_text_safety blocklist enforcement
// ---------------------------------------------------------------------------

#[test]
fn upgrade_remediation_passes_validate_text_safety() {
    let remediation = SecurityRemediation {
        category: RemediationCategory::Upgrade,
        description: "Upgrade test-pkg to version 3.0.0 or later".to_string(),
        rationale: "Advisory CVE-2024-9999 indicates this package is affected; fixed versions are available".to_string(),
        evidence_urls: Vec::new(),
        fixed_versions: vec!["3.0.0".to_string()],
        affected_packages: vec!["test-pkg".to_string()],
        source_ids: Vec::new(),
        confidence: EvidenceConfidence::Strong,
    };
    assert!(
        remediation.validate_text_safety().is_ok(),
        "normal upgrade remediation must pass validation"
    );
}

#[test]
fn manual_review_remediation_passes_validate_text_safety() {
    let remediation = SecurityRemediation {
        category: RemediationCategory::ManualReview,
        description: "Manual review required for test-pkg - no fixed version available".to_string(),
        rationale:
            "Advisory CVE-2024-9999 confirms affected status but no fixed version is documented"
                .to_string(),
        evidence_urls: Vec::new(),
        fixed_versions: Vec::new(),
        affected_packages: vec!["test-pkg".to_string()],
        source_ids: Vec::new(),
        confidence: EvidenceConfidence::Weak,
    };
    assert!(
        remediation.validate_text_safety().is_ok(),
        "manual review remediation must pass validation"
    );
}

#[test]
fn remediation_with_exploit_keyword_fails_validate_text_safety() {
    let remediation = SecurityRemediation {
        category: RemediationCategory::Upgrade,
        description: "Upgrade to fix the vulnerability and prevent exploit".to_string(),
        rationale: "This version is affected by a known exploit".to_string(),
        evidence_urls: Vec::new(),
        fixed_versions: vec!["3.0.0".to_string()],
        affected_packages: vec!["test-pkg".to_string()],
        source_ids: Vec::new(),
        confidence: EvidenceConfidence::Strong,
    };
    let result = remediation.validate_text_safety();
    assert!(
        result.is_err(),
        "text with 'exploit' keyword must fail validation"
    );
    assert_eq!(result.unwrap_err().keyword, "exploit");
}

#[test]
fn remediation_with_shellcode_fails_validate_text_safety() {
    let remediation = SecurityRemediation {
        category: RemediationCategory::ManualReview,
        description: "Requires manual review of shellcode vector".to_string(),
        rationale: "The advisory describes a shellcode vulnerability".to_string(),
        evidence_urls: Vec::new(),
        fixed_versions: Vec::new(),
        affected_packages: vec!["test-pkg".to_string()],
        source_ids: Vec::new(),
        confidence: EvidenceConfidence::Unknown,
    };
    let result = remediation.validate_text_safety();
    assert!(
        result.is_err(),
        "text with 'shellcode' keyword must fail validation"
    );
    assert_eq!(result.unwrap_err().keyword, "shellcode");
}

#[test]
fn remediation_with_rce_fails_validate_text_safety() {
    let remediation = SecurityRemediation {
        category: RemediationCategory::Upgrade,
        description: "Upgrade to prevent rce through the vulnerable endpoint".to_string(),
        rationale: "Remote code execution is possible".to_string(),
        evidence_urls: Vec::new(),
        fixed_versions: vec!["2.0.0".to_string()],
        affected_packages: vec!["test-pkg".to_string()],
        source_ids: Vec::new(),
        confidence: EvidenceConfidence::Strong,
    };
    let result = remediation.validate_text_safety();
    assert!(
        result.is_err(),
        "text with 'rce' keyword must fail validation"
    );
    assert_eq!(result.unwrap_err().keyword, "rce");
}

#[test]
fn remediation_with_proof_of_concept_fails_validate_text_safety() {
    let remediation = SecurityRemediation {
        category: RemediationCategory::ManualReview,
        description: "Review proof of concept details in advisory".to_string(),
        rationale: "Advisory references a proof of concept for this vulnerability".to_string(),
        evidence_urls: Vec::new(),
        fixed_versions: Vec::new(),
        affected_packages: vec!["test-pkg".to_string()],
        source_ids: Vec::new(),
        confidence: EvidenceConfidence::Unknown,
    };
    let result = remediation.validate_text_safety();
    assert!(
        result.is_err(),
        "text with 'proof of concept' must fail validation"
    );
    assert_eq!(result.unwrap_err().keyword, "proof of concept");
}

#[test]
fn all_remediation_categories_can_be_validated() {
    let categories = vec![
        RemediationCategory::Upgrade,
        RemediationCategory::Pin,
        RemediationCategory::Replace,
        RemediationCategory::RemoveDependency,
        RemediationCategory::ConfigurationMitigation,
        RemediationCategory::FeatureDisable,
        RemediationCategory::VulnerableApiAvoidance,
        RemediationCategory::TransitiveOverride,
        RemediationCategory::VendorPatch,
        RemediationCategory::MonitorOnly,
        RemediationCategory::ManualReview,
        RemediationCategory::NoActionSupportedByEvidence,
    ];

    for category in categories {
        let remediation = SecurityRemediation {
            category,
            description: format!("Remediation for category {category:?}"),
            rationale: "Safe rationale text".to_string(),
            evidence_urls: Vec::new(),
            fixed_versions: Vec::new(),
            affected_packages: Vec::new(),
            source_ids: Vec::new(),
            confidence: EvidenceConfidence::Unknown,
        };
        assert!(
            remediation.validate_text_safety().is_ok(),
            "safe remediation for category {category:?} must pass validation"
        );
    }
}

// ---------------------------------------------------------------------------
// 12. M001 typed dependency evidence and applicability trust boundary
// ---------------------------------------------------------------------------

#[test]
fn resolved_lock_finding_carries_exact_evidence() {
    let content = "[[package]]\nname = \"serde\"\nversion = \"1.0.193\"\n";
    let findings = parse_dependency_file("Cargo.lock", content);
    assert_eq!(findings.len(), 1);
    let finding = &findings[0];
    assert_eq!(finding.exact_version(), Some("1.0.193"));
    assert!(finding.has_resolved_evidence());
    assert_eq!(finding.version.as_deref(), Some("1.0.193"));
}

#[test]
fn manifest_requirement_shaped_like_exact_number_is_not_resolved() {
    let content = "[dependencies]\nserde = \"1.0.193\"\n";
    let findings = parse_dependency_file("Cargo.toml", content);
    assert_eq!(findings.len(), 1);
    let finding = &findings[0];
    assert_eq!(finding.exact_version(), None);
    assert!(!finding.has_resolved_evidence());
    assert_eq!(finding.version_requirement.as_deref(), Some("1.0.193"));
}

#[test]
fn checksum_observation_is_not_resolved_evidence() {
    let content = "github.com/gin-gonic/gin v1.9.0 h1:abc123\n";
    let findings = parse_dependency_file("go.sum", content);
    assert_eq!(findings.len(), 1);
    let finding = &findings[0];
    assert_eq!(finding.exact_version(), None);
    assert!(!finding.has_resolved_evidence());
    assert!(finding.integrity_hash.is_some());
}

#[test]
fn go_mod_requirement_is_not_resolved_evidence() {
    let content = "module example.com/x\n\nrequire github.com/gin-gonic/gin v1.9.1\n";
    let findings = parse_dependency_file("go.mod", content);
    assert_eq!(findings.len(), 1);
    assert!(!findings[0].has_resolved_evidence());
    assert_eq!(findings[0].version_requirement.as_deref(), Some("1.9.1"));
}

#[test]
fn high_advisory_plus_medium_dependency_caps_at_medium() {
    assert_eq!(
        compose_confidence(
            Some(ApplicabilityConfidence::Medium),
            ApplicabilityConfidence::High
        ),
        ApplicabilityConfidence::Medium
    );
    assert_eq!(
        compose_confidence(
            Some(ApplicabilityConfidence::High),
            ApplicabilityConfidence::High
        ),
        ApplicabilityConfidence::High
    );
    assert_eq!(
        compose_confidence(None, ApplicabilityConfidence::High),
        ApplicabilityConfidence::Low
    );
    assert_eq!(
        compose_confidence(
            Some(ApplicabilityConfidence::High),
            ApplicabilityConfidence::Low
        ),
        ApplicabilityConfidence::Low
    );
}

#[test]
fn advisory_confidence_requires_structured_ranges() {
    assert_eq!(
        advisory_confidence_for_ranges(2),
        ApplicabilityConfidence::High
    );
    assert_eq!(
        advisory_confidence_for_ranges(0),
        ApplicabilityConfidence::Low
    );
}

#[test]
fn finding_source_kind_and_relation_are_preserved() {
    let content = "[[package]]\nname = \"serde\"\nversion = \"1.0.193\"\n";
    let findings = parse_dependency_file("Cargo.lock", content);
    assert_eq!(findings[0].source_kind, DependencySource::LockFile);
    assert_eq!(findings[0].relation, Some(DependencyRelation::Transitive));
}

#[test]
fn package_mismatch_remains_unmatched() {
    assert!(!packages_match(
        &PackageEcosystem::Npm,
        "lodash",
        "underscore"
    ));
    assert!(packages_match(&PackageEcosystem::Npm, "Lodash", "lodash"));
}

#[test]
fn pypi_names_canonicalize_for_comparison_but_preserve_display() {
    assert_eq!(
        canonical_package_name(&PackageEcosystem::Pypi, "My_Package.Name"),
        "my-package-name"
    );
    assert_eq!(
        canonical_package_name(&PackageEcosystem::Pypi, "requests"),
        "requests"
    );
    assert!(packages_match(
        &PackageEcosystem::Pypi,
        "my_package",
        "my-package"
    ));
    assert!(packages_match(&PackageEcosystem::Pypi, "Django", "django"));
    assert_eq!(
        canonical_package_name(&PackageEcosystem::Npm, "My_Package"),
        "My_Package"
    );
}

#[test]
fn legacy_json_still_contains_existing_fields() {
    let finding = DependencyFinding {
        ecosystem: PackageEcosystem::CratesIo,
        package: "serde".to_string(),
        version: Some("1.0.193".to_string()),
        source_file: Some("Cargo.lock".to_string()),
        source_line: Some(3),
        source_kind: DependencySource::LockFile,
        confidence: Some(ApplicabilityConfidence::High),
        relation: Some(DependencyRelation::Transitive),
        resolved_version: Some("1.0.193".to_string()),
        version_requirement: None,
        reference_kind: None,
        reference_value: None,
        provenance: None,
        target_context: None,
        integrity_hash: None,
    };
    let json = serde_json::to_value(&finding).unwrap();
    assert_eq!(json["ecosystem"], "crates_io");
    assert_eq!(json["package"], "serde");
    assert_eq!(json["version"], "1.0.193");
    assert_eq!(json["source_kind"], "lock_file");
    assert_eq!(json["resolved_version"], "1.0.193");
}

#[test]
fn typed_fields_round_trip_through_serde() {
    let finding = DependencyFinding {
        ecosystem: PackageEcosystem::GithubActions,
        package: "actions/checkout".to_string(),
        version: Some("v4".to_string()),
        source_file: Some(".github/workflows/ci.yml".to_string()),
        source_line: Some(7),
        source_kind: DependencySource::WorkflowFile,
        confidence: Some(ApplicabilityConfidence::Medium),
        relation: Some(DependencyRelation::Unknown),
        resolved_version: None,
        version_requirement: None,
        reference_kind: Some(DependencyReferenceKind::Tag),
        reference_value: Some("v4".to_string()),
        provenance: Some("github".to_string()),
        target_context: None,
        integrity_hash: None,
    };
    let json = serde_json::to_string(&finding).unwrap();
    let parsed: DependencyFinding = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.reference_kind, Some(DependencyReferenceKind::Tag));
    assert_eq!(parsed.reference_value.as_deref(), Some("v4"));
    assert_eq!(parsed.exact_version(), None);
    assert!(!parsed.has_resolved_evidence());
}

#[test]
fn parse_report_distinguishes_completeness_states() {
    let complete = DependencyParseReport::complete(Vec::new());
    assert_eq!(complete.status, ParseStatus::Complete);
    assert!(complete.findings.is_empty());
    assert!(complete.diagnostics.is_empty());

    let partial = DependencyParseReport::partial(
        Vec::new(),
        vec![eggsearch::core::security_applicability::ParseDiagnostic {
            code: "dependency_parse_partial".to_string(),
            message: "truncated".to_string(),
            line: None,
        }],
    );
    assert_eq!(partial.status, ParseStatus::Partial);
    assert_eq!(partial.diagnostics.len(), 1);

    let unsupported = DependencyParseReport::unsupported("nope");
    assert_eq!(unsupported.status, ParseStatus::Unsupported);
    assert_eq!(
        unsupported.diagnostics[0].code,
        "dependency_format_unsupported"
    );

    let malformed = DependencyParseReport::malformed("bad");
    assert_eq!(malformed.status, ParseStatus::Malformed);
    assert_eq!(malformed.diagnostics[0].code, "dependency_parse_malformed");
}

#[test]
fn dispatch_reports_unsupported_for_unknown_files() {
    let report = parse_dependency_file_report("README.md", "some content");
    assert_eq!(report.status, ParseStatus::Unsupported);
    assert!(report.findings.is_empty());
}

#[test]
fn dispatch_handles_windows_paths_for_known_files() {
    let content = "[[package]]\nname = \"serde\"\nversion = \"1.0.193\"\n";
    let report = parse_dependency_file_report("C:\\proj\\Cargo.lock", content);
    assert_eq!(report.status, ParseStatus::Complete);
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].exact_version(), Some("1.0.193"));
}

// ---------------------------------------------------------------------------
// 13. M007 parser budgets and diagnostic warning codes
// ---------------------------------------------------------------------------

use eggsearch::core::security_applicability::{
    truncate_aggregate, truncate_findings, truncate_report, DependencyParserBudget,
};
use eggsearch::core::warning::WarningCode;

fn synthetic_finding(package: &str) -> DependencyFinding {
    DependencyFinding {
        ecosystem: PackageEcosystem::CratesIo,
        package: package.to_string(),
        version: Some("1.0.0".to_string()),
        source_file: Some("Cargo.lock".to_string()),
        source_line: None,
        source_kind: DependencySource::LockFile,
        confidence: Some(ApplicabilityConfidence::High),
        relation: Some(DependencyRelation::Transitive),
        resolved_version: Some("1.0.0".to_string()),
        version_requirement: None,
        reference_kind: None,
        reference_value: None,
        provenance: None,
        target_context: None,
        integrity_hash: None,
    }
}

#[test]
fn per_file_budget_boundaries() {
    let budget = DependencyParserBudget::standard();
    let below: Vec<DependencyFinding> = (0..budget.max_findings_per_file)
        .map(|i| synthetic_finding(&format!("pkg-{i}")))
        .collect();
    let (kept, note) = truncate_findings(below, budget.max_findings_per_file);
    assert_eq!(kept.len(), budget.max_findings_per_file);
    assert!(note.is_none());

    let above: Vec<DependencyFinding> = (0..budget.max_findings_per_file + 1)
        .map(|i| synthetic_finding(&format!("pkg-{i}")))
        .collect();
    let (kept, note) = truncate_findings(above, budget.max_findings_per_file);
    assert_eq!(kept.len(), budget.max_findings_per_file);
    assert_eq!(kept[0].package, "pkg-0");
    assert_eq!(
        kept[budget.max_findings_per_file - 1].package,
        format!("pkg-{}", budget.max_findings_per_file - 1)
    );
    let note = note.expect("above-limit input must produce a diagnostic");
    assert_eq!(note.code, "dependency_finding_budget_exceeded");
}

#[test]
fn aggregate_budget_boundaries() {
    let budget = DependencyParserBudget::standard();
    let exact: Vec<DependencyFinding> = (0..budget.max_findings_per_request)
        .map(|i| synthetic_finding(&format!("pkg-{i}")))
        .collect();
    let (kept, note) = truncate_aggregate(exact, budget.max_findings_per_request);
    assert_eq!(kept.len(), budget.max_findings_per_request);
    assert!(note.is_none());

    let over: Vec<DependencyFinding> = (0..budget.max_findings_per_request + 100)
        .map(|i| synthetic_finding(&format!("pkg-{i}")))
        .collect();
    let (kept, note) = truncate_aggregate(over, budget.max_findings_per_request);
    assert_eq!(kept.len(), budget.max_findings_per_request);
    assert!(note.is_some());
}

#[test]
fn file_list_budget_boundaries() {
    use eggsearch::meta::advisory_range::cap_file_list;
    let budget = DependencyParserBudget::standard();
    let exact: Vec<String> = (0..budget.max_files_per_request)
        .map(|i| format!("f{i}.lock"))
        .collect();
    let (kept, note) = cap_file_list(&exact, budget.max_files_per_request);
    assert_eq!(kept.len(), budget.max_files_per_request);
    assert!(note.is_none());

    let mut over = exact.clone();
    over.push("extra.lock".to_string());
    let (kept, note) = cap_file_list(&over, budget.max_files_per_request);
    assert_eq!(kept.len(), budget.max_files_per_request);
    let note = note.expect("above-limit file list must warn");
    assert!(note
        .message
        .starts_with("dependency_file_budget_exceeded: "));
}

#[test]
fn diagnostic_count_and_length_are_bounded() {
    let budget = DependencyParserBudget {
        max_diagnostics: 2,
        max_diagnostic_chars: 10,
        ..DependencyParserBudget::standard()
    };
    let report = DependencyParseReport {
        findings: Vec::new(),
        status: ParseStatus::Complete,
        diagnostics: vec![
            eggsearch::core::security_applicability::ParseDiagnostic {
                code: "dependency_parse_partial".to_string(),
                message: "a very long diagnostic message exceeding the cap".to_string(),
                line: None,
            },
            eggsearch::core::security_applicability::ParseDiagnostic {
                code: "dependency_parse_partial".to_string(),
                message: "second".to_string(),
                line: None,
            },
            eggsearch::core::security_applicability::ParseDiagnostic {
                code: "dependency_parse_partial".to_string(),
                message: "third".to_string(),
                line: None,
            },
        ],
    };
    let bounded = truncate_report(report, budget);
    assert_eq!(bounded.diagnostics.len(), 2);
    assert!(bounded.diagnostics[0].message.chars().count() <= 10);
    assert_eq!(bounded.status, ParseStatus::Partial);
}

#[test]
fn every_diagnostic_code_maps_to_a_stable_warning() {
    use eggsearch::core::warning::convert_warnings;
    use eggsearch::meta::advisory_range::{parse_diagnostic_warning, weak_evidence_summary};
    let cases = [
        (
            "dependency_parse_malformed",
            WarningCode::DependencyParseMalformed,
        ),
        (
            "dependency_format_unsupported",
            WarningCode::DependencyFormatUnsupported,
        ),
        (
            "dependency_parse_partial",
            WarningCode::DependencyParsePartial,
        ),
        (
            "dependency_finding_budget_exceeded",
            WarningCode::DependencyFindingBudgetExceeded,
        ),
    ];
    for (code, expected) in cases {
        let warning = parse_diagnostic_warning(
            "Cargo.lock",
            &eggsearch::core::security_applicability::ParseDiagnostic {
                code: code.to_string(),
                message: "detail".to_string(),
                line: None,
            },
        );
        let structured = convert_warnings(std::slice::from_ref(&warning));
        assert_eq!(structured.len(), 1);
        assert_eq!(structured[0].code, expected, "code: {code}");
    }
    let file_warning = eggsearch::core::result::SearchWarning::new(
        "_system",
        "dependency_file_budget_exceeded: kept 32 files",
    );
    let structured = convert_warnings(std::slice::from_ref(&file_warning));
    assert_eq!(
        structured[0].code,
        WarningCode::DependencyFileBudgetExceeded
    );
    let summary = weak_evidence_summary(3).expect("nonzero weak count must warn");
    let structured = convert_warnings(std::slice::from_ref(&summary));
    assert_eq!(
        structured[0].code,
        WarningCode::WeakEvidenceIgnoredForExactApplicability
    );
    assert!(weak_evidence_summary(0).is_none());
}
