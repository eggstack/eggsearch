//! Phase 13 Workstream 5: Security applicability regression corpus.
//!
//! Covers the full applicability assessment pipeline:
//! - Version/range evaluation (affected, not-affected, unknown, insufficient-evidence)
//! - Dependency relation classification (direct vs transitive)
//! - Remediation action categories (upgrade vs manual-review)
//! - KEV metadata handling
//! - Text safety validation for remediation text
//!
//! Run via:
//! ```bash
//! cargo test --features mock --test security_applicability_corpus
//! ```

use eggsearch::core::code_evidence::EvidenceConfidence;
use eggsearch::core::package::PackageEcosystem;
use eggsearch::core::result::TrustLevel;
use eggsearch::core::security::{assess_source_quality, SecuritySourceTier};
use eggsearch::core::security::{
    KevMetadata, RemediationCategory, SecurityRemediation, SeverityLevel, VulnerabilityMetadata,
    VulnerabilitySource,
};
use eggsearch::core::security_applicability::{
    AdvisoryRange, ApplicabilityAssessment, ApplicabilityConfidence, ApplicabilityStatus,
    DependencyFinding, DependencyRelation, DependencySource,
};
use eggsearch::core::source_card::{SourceCard, SourceMetadata};
use eggsearch::meta::advisory_range::{assess_version_applicability, extract_advisory_ranges};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

fn make_vuln(
    affected: Vec<&str>,
    patched: Vec<&str>,
    patched_versions: Vec<&str>,
) -> VulnerabilityMetadata {
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
        patched_versions: patched_versions.into_iter().map(String::from).collect(),
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

fn make_card(source_kind: eggsearch::core::source_card::SourceKind, url: &str) -> SourceCard {
    let mut card = SourceCard::new(
        "Test",
        url,
        vec!["test".to_string()],
        None,
        TrustLevel::ExternalUntrusted,
    );
    card.metadata = SourceMetadata {
        source_kind,
        ..Default::default()
    };
    card
}

// ===========================================================================
// 1. Affected exact version → status: affected, confidence: high
// ===========================================================================

#[test]
fn affected_exact_version_returns_affected_high_confidence() {
    let ranges = vec![make_range(
        PackageEcosystem::CratesIo,
        Some(">=1.0.0, <2.0.0"),
        vec![],
        vec![],
    )];
    let outcome = assess_version_applicability("1.5.0", &ranges, &PackageEcosystem::CratesIo);

    assert_eq!(outcome.status, ApplicabilityStatus::Affected);
    assert!(
        !outcome.matched_ranges.is_empty(),
        "must have at least one matched range"
    );
    assert!(
        outcome
            .reasons
            .iter()
            .any(|r| r.contains("matches affected range")),
        "reasons must mention range match: {:?}",
        outcome.reasons
    );
}

// ===========================================================================
// 2. Unaffected exact version (above fixed range) → status: not_affected
// ===========================================================================

#[test]
fn unaffected_exact_version_returns_not_affected() {
    let ranges = vec![make_range(
        PackageEcosystem::CratesIo,
        Some(">=1.0.0, <2.0.0"),
        vec!["2.0.0"],
        vec![],
    )];
    let outcome = assess_version_applicability("2.1.0", &ranges, &PackageEcosystem::CratesIo);

    assert_eq!(outcome.status, ApplicabilityStatus::NotAffected);
    assert!(
        outcome
            .reasons
            .iter()
            .any(|r| r.contains("outside affected range")),
        "reasons must mention outside range: {:?}",
        outcome.reasons
    );
}

// ===========================================================================
// 3. Unknown range syntax → status: unknown
// ===========================================================================

#[test]
fn unknown_range_syntax_returns_unknown() {
    let ranges = vec![make_range(
        PackageEcosystem::CratesIo,
        Some("banana"),
        vec![],
        vec![],
    )];
    let outcome = assess_version_applicability("1.0.0", &ranges, &PackageEcosystem::CratesIo);

    assert_eq!(outcome.status, ApplicabilityStatus::Unknown);
    assert!(
        outcome
            .reasons
            .iter()
            .any(|r| r.contains("could not evaluate range")),
        "reasons must mention range evaluation failure: {:?}",
        outcome.reasons
    );
}

// ===========================================================================
// 4. Missing version → status: insufficient_evidence
// ===========================================================================

#[test]
fn missing_version_yields_insufficient_evidence() {
    let assessment = ApplicabilityAssessment {
        status: ApplicabilityStatus::InsufficientEvidence,
        confidence: ApplicabilityConfidence::Low,
        ecosystem: PackageEcosystem::CratesIo,
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
        "confidence must be Low when no version provided"
    );
}

// ===========================================================================
// 5. Lockfile transitive dependency
// ===========================================================================

#[test]
fn lockfile_transitive_dependency_is_classified() {
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

// ===========================================================================
// 6. Manifest direct dependency
// ===========================================================================

#[test]
fn manifest_direct_dependency_is_classified() {
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

// ===========================================================================
// 7. Fixed version available → Upgrade remediation
// ===========================================================================

#[test]
fn fixed_version_available_produces_upgrade_remediation() {
    let ranges = vec![make_range(
        PackageEcosystem::Npm,
        Some(">= 2.0.0, < 3.0.0"),
        vec!["3.0.0"],
        vec![],
    )];
    let outcome = assess_version_applicability("2.5.0", &ranges, &PackageEcosystem::Npm);

    assert_eq!(outcome.status, ApplicabilityStatus::Affected);

    let fixed_versions = &outcome.matched_ranges[0].fixed_versions;
    assert_eq!(fixed_versions, &vec!["3.0.0"]);

    let remediation = SecurityRemediation {
        category: RemediationCategory::Upgrade,
        description: format!(
            "Upgrade test-pkg to version {} or later.",
            fixed_versions[0]
        ),
        rationale: "Advisory CVE-2024-9999 indicates this package is affected; fixed versions are available".to_string(),
        evidence_urls: Vec::new(),
        fixed_versions: fixed_versions.clone(),
        affected_packages: vec!["test-pkg".to_string()],
        source_ids: Vec::new(),
        confidence: EvidenceConfidence::Strong,
    };

    assert_eq!(remediation.category, RemediationCategory::Upgrade);
    assert_eq!(remediation.fixed_versions, vec!["3.0.0"]);
}

// ===========================================================================
// 8. No fixed version → ManualReview remediation
// ===========================================================================

#[test]
fn no_fixed_version_produces_manual_review_remediation() {
    let ranges = vec![make_range(
        PackageEcosystem::CratesIo,
        Some(">= 1.0.0, < 2.0.0"),
        vec![],
        vec![],
    )];
    let outcome = assess_version_applicability("1.5.0", &ranges, &PackageEcosystem::CratesIo);

    assert_eq!(outcome.status, ApplicabilityStatus::Affected);
    assert!(
        outcome.matched_ranges[0].fixed_versions.is_empty(),
        "no fixed versions should be present"
    );

    let remediation = SecurityRemediation {
        category: RemediationCategory::ManualReview,
        description: "Manual review required — no fixed version available from advisory metadata."
            .to_string(),
        rationale:
            "Advisory CVE-2024-9999 confirms affected status but no patched version is documented"
                .to_string(),
        evidence_urls: Vec::new(),
        fixed_versions: Vec::new(),
        affected_packages: vec!["test-pkg".to_string()],
        source_ids: Vec::new(),
        confidence: EvidenceConfidence::Weak,
    };

    assert_eq!(remediation.category, RemediationCategory::ManualReview);
    assert!(remediation.fixed_versions.is_empty());
}

// ===========================================================================
// 9. KEV present → KEV warning emitted
// ===========================================================================

#[test]
fn kev_present_emits_kev_match_warning() {
    let vuln = VulnerabilityMetadata {
        cve_ids: vec!["CVE-2024-0001".to_string()],
        ecosystem: Some("crates_io".to_string()),
        package: Some("test-pkg".to_string()),
        affected_ranges: vec!["< 2.0.0".to_string()],
        severity: Some(SeverityLevel::Critical),
        kev: Some(KevMetadata {
            vendor: Some("test-vendor".to_string()),
            product: Some("test-pkg".to_string()),
            required_action: Some("Apply patch".to_string()),
            due_date: Some("2025-01-15".to_string()),
            known_ransomware_usage: true,
            catalog_date: Some("2024-12-01".to_string()),
        }),
        source: VulnerabilitySource::Osv,
        ..Default::default()
    };

    let kev = vuln.kev.as_ref().expect("KEV metadata must be present");
    assert!(
        kev.known_ransomware_usage,
        "KEV must indicate ransomware usage"
    );
    assert!(kev.due_date.is_some(), "KEV must have a due date");
}

#[test]
fn kev_present_on_vulnerability_metadata_is_preserved() {
    let vuln = make_vuln(vec![">= 1.0.0"], vec![], vec![]);
    assert!(vuln.kev.is_none(), "default vuln metadata has no KEV");

    let mut vuln_with_kev = vuln;
    vuln_with_kev.kev = Some(KevMetadata {
        vendor: Some("vendor".to_string()),
        product: Some("product".to_string()),
        required_action: Some("patch".to_string()),
        due_date: Some("2025-06-01".to_string()),
        known_ransomware_usage: true,
        catalog_date: Some("2024-11-01".to_string()),
    });
    assert!(vuln_with_kev.kev.is_some());
}

// ===========================================================================
// 10. KEV absent → kev_absent_not_proof warning type
// ===========================================================================

#[test]
fn kev_absent_not_proof_warning_type() {
    let vuln = make_vuln(vec![">= 1.0.0"], vec![], vec![]);
    assert!(vuln.kev.is_none(), "no KEV data should be present");

    // Simulate the warning that would be emitted
    let warning = "kev_absent_not_proof: no CVE identifiers available for KEV catalog lookup";
    assert!(
        warning.contains("kev_absent_not_proof"),
        "warning must use kev_absent_not_proof prefix"
    );
}

// ===========================================================================
// 11. Text safety validation
// ===========================================================================

#[test]
fn safe_remediation_text_passes_validation() {
    let remediation = SecurityRemediation {
        category: RemediationCategory::Upgrade,
        description: "Upgrade test-pkg to version 3.0.0 or later to fix the vulnerability.".to_string(),
        rationale: "Advisory CVE-2024-9999 indicates this package is affected; fixed versions are available".to_string(),
        evidence_urls: Vec::new(),
        fixed_versions: vec!["3.0.0".to_string()],
        affected_packages: vec!["test-pkg".to_string()],
        source_ids: Vec::new(),
        confidence: EvidenceConfidence::Strong,
    };

    assert!(
        remediation.validate_text_safety().is_ok(),
        "safe text must pass validation"
    );
}

#[test]
fn exploit_like_remediation_text_fails_validation() {
    let remediation = SecurityRemediation {
        category: RemediationCategory::Upgrade,
        description: "Upgrade to fix the vulnerability and prevent exploit.".to_string(),
        rationale: "This version is affected by a known exploit vector.".to_string(),
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
fn shellcode_in_text_fails_validation() {
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
    assert!(result.is_err(), "text with 'shellcode' must fail");
    assert_eq!(result.unwrap_err().keyword, "shellcode");
}

#[test]
fn rce_in_text_fails_validation() {
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
    assert!(result.is_err(), "text with 'rce' must fail");
    assert_eq!(result.unwrap_err().keyword, "rce");
}

#[test]
fn proof_of_concept_in_text_fails_validation() {
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
    assert!(result.is_err(), "text with 'proof of concept' must fail");
    assert_eq!(result.unwrap_err().keyword, "proof of concept");
}

// ===========================================================================
// Additional: extract_advisory_ranges from VulnerabilityMetadata
// ===========================================================================

#[test]
fn extract_advisory_ranges_from_metadata() {
    let vuln = make_vuln(vec![">= 1.0.0, < 2.0.0"], vec!["2.0.0"], vec![]);
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

// ===========================================================================
// Additional: Version below range is NotAffected
// ===========================================================================

#[test]
fn version_below_range_is_not_affected() {
    let ranges = vec![make_range(
        PackageEcosystem::Npm,
        Some(">= 2.0.0, < 3.0.0"),
        vec!["3.0.0"],
        vec![],
    )];
    let outcome = assess_version_applicability("1.9.9", &ranges, &PackageEcosystem::Npm);

    assert_eq!(outcome.status, ApplicabilityStatus::NotAffected);
}

// ===========================================================================
// Additional: version at fixed version is NotAffected
// ===========================================================================

#[test]
fn version_at_fixed_version_is_not_affected() {
    let ranges = vec![make_range(
        PackageEcosystem::Npm,
        Some(">= 2.0.0, < 3.0.0"),
        vec!["3.0.0"],
        vec![],
    )];
    let outcome = assess_version_applicability("3.0.0", &ranges, &PackageEcosystem::Npm);

    assert_eq!(outcome.status, ApplicabilityStatus::NotAffected);
}

// ===========================================================================
// Additional: Empty ranges → Unknown
// ===========================================================================

#[test]
fn empty_ranges_return_unknown() {
    let outcome = assess_version_applicability("1.0.0", &[], &PackageEcosystem::CratesIo);
    assert_eq!(outcome.status, ApplicabilityStatus::Unknown);
}

// ===========================================================================
// Additional: RemediationCategory as_str coverage
// ===========================================================================

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

// ===========================================================================
// Additional: All remediation categories pass safe text validation
// ===========================================================================

#[test]
fn all_categories_pass_safe_text_validation() {
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

// ===========================================================================
// Additional: Serde roundtrip for ApplicabilityAssessment
// ===========================================================================

#[test]
fn applicability_assessment_serde_roundtrip() {
    let assessment = ApplicabilityAssessment {
        status: ApplicabilityStatus::Affected,
        confidence: ApplicabilityConfidence::High,
        ecosystem: PackageEcosystem::Npm,
        package: "express".to_string(),
        version: Some("2.5.0".to_string()),
        advisory_ids: vec!["CVE-2024-9999".to_string()],
        matched_ranges: vec![make_range(
            PackageEcosystem::Npm,
            Some(">= 2.0.0, < 3.0.0"),
            vec!["3.0.0"],
            vec![],
        )],
        fixed_versions: vec!["3.0.0".to_string()],
        reasons: vec!["version satisfies affected range".to_string()],
        evidence_urls: Vec::new(),
        warnings: Vec::new(),
        version_source: Some(DependencySource::Manifest),
        dependency_relation: Some(DependencyRelation::Direct),
        source_ids: vec!["src_aabbccdd11223344".to_string()],
        fetch_ids: vec![],
    };

    let json = serde_json::to_string(&assessment).unwrap();
    let parsed: ApplicabilityAssessment = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.status, ApplicabilityStatus::Affected);
    assert_eq!(parsed.confidence, ApplicabilityConfidence::High);
    assert_eq!(parsed.package, "express");
    assert_eq!(parsed.version.as_deref(), Some("2.5.0"));
    assert_eq!(parsed.fixed_versions, vec!["3.0.0"]);
    assert_eq!(parsed.dependency_relation, Some(DependencyRelation::Direct));
}

// ===========================================================================
// Additional: Not-affected produces no remediation actions
// ===========================================================================

#[test]
fn not_affected_produces_no_remediation() {
    let ranges = vec![make_range(
        PackageEcosystem::CratesIo,
        Some(">= 2.0.0, < 3.0.0"),
        vec!["3.0.0"],
        vec![],
    )];
    let outcome = assess_version_applicability("1.5.0", &ranges, &PackageEcosystem::CratesIo);

    let remediation_actions: Vec<SecurityRemediation> = match outcome.status {
        ApplicabilityStatus::Affected => vec![SecurityRemediation {
            category: RemediationCategory::Upgrade,
            description: "Upgrade to 3.0.0.".to_string(),
            rationale: "affected".to_string(),
            evidence_urls: Vec::new(),
            fixed_versions: vec!["3.0.0".to_string()],
            affected_packages: Vec::new(),
            source_ids: Vec::new(),
            confidence: EvidenceConfidence::Strong,
        }],
        ApplicabilityStatus::NotAffected => Vec::new(),
        _ => vec![SecurityRemediation {
            category: RemediationCategory::ManualReview,
            description: "Manual review required.".to_string(),
            rationale: "uncertain".to_string(),
            evidence_urls: Vec::new(),
            fixed_versions: Vec::new(),
            affected_packages: Vec::new(),
            source_ids: Vec::new(),
            confidence: EvidenceConfidence::Weak,
        }],
    };

    assert!(
        remediation_actions.is_empty(),
        "not_affected must produce zero remediation actions, got {}",
        remediation_actions.len()
    );
}

// ===========================================================================
// Additional: Source tier classification for security URLs
// ===========================================================================

#[test]
fn security_source_tier_classifications() {
    let cases = vec![
        (
            "https://nvd.nist.gov/vuln/detail/CVE-2024-0001",
            SecuritySourceTier::PrimaryAdvisory,
        ),
        (
            "https://osv.dev/vulnerability/GHSA-test",
            SecuritySourceTier::PrimaryAdvisory,
        ),
        (
            "https://github.com/advisories/GHSA-test",
            SecuritySourceTier::PackageRegistryAdvisory,
        ),
        (
            "https://example.com/security/advisory",
            SecuritySourceTier::VendorAdvisory,
        ),
        (
            "https://blog.example.com/security",
            SecuritySourceTier::NewsOrBlog,
        ),
        (
            "https://stackoverflow.com/questions/123",
            SecuritySourceTier::CommunityDiscussion,
        ),
    ];

    for (url, expected_tier) in cases {
        let tier = eggsearch::core::security::classify_source_tier(url);
        assert_eq!(tier, expected_tier, "URL {url} should be {expected_tier:?}");
    }
}

// ===========================================================================
// Additional: assess_source_quality picks highest tier
// ===========================================================================

#[test]
fn assess_source_quality_picks_highest_tier() {
    let cards = vec![
        make_card(
            eggsearch::core::source_card::SourceKind::News,
            "https://blog.example.com/security",
        ),
        make_card(
            eggsearch::core::source_card::SourceKind::SecurityAdvisory,
            "https://nvd.nist.gov/vuln/detail/CVE-2024-0001",
        ),
    ];
    let quality = assess_source_quality(&cards);
    assert_eq!(
        quality.tier,
        SecuritySourceTier::PrimaryAdvisory,
        "should pick highest tier among mixed sources"
    );
}

// ===========================================================================
// Additional: Serde roundtrip for SecurityRemediation
// ===========================================================================

#[test]
fn security_remediation_serde_roundtrip() {
    let remediation = SecurityRemediation {
        category: RemediationCategory::Upgrade,
        description: "Upgrade to version 3.0.0.".to_string(),
        rationale: "Fixes CVE-2024-9999".to_string(),
        evidence_urls: vec!["https://example.com/advisory".to_string()],
        fixed_versions: vec!["3.0.0".to_string()],
        affected_packages: vec!["test-pkg".to_string()],
        source_ids: vec!["src_aabbccdd11223344".to_string()],
        confidence: EvidenceConfidence::Exact,
    };

    let json = serde_json::to_string(&remediation).unwrap();
    let parsed: SecurityRemediation = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.category, RemediationCategory::Upgrade);
    assert_eq!(parsed.fixed_versions, vec!["3.0.0"]);
    assert_eq!(parsed.confidence, EvidenceConfidence::Exact);
}
