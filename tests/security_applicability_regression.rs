//! Targeted regression tests for security applicability version/range evaluation.
//!
//! These tests directly exercise `assess_version_applicability` and
//! `evaluate_range_expression` from `eggsearch::meta::advisory_range`
//! to guard against the inverted `>=` bug (Workstream 8 of the
//! corrective plan) and validate conservative `Unknown` behavior.
//!
//! They would have caught the original inverted comparison because
//! they assert that versions below the `>=` floor are `NotAffected`,
//! not `Affected`.
//!
//! Run via:
//! ```bash
//! cargo test --all-features --test security_applicability_regression
//! ```

use eggsearch::core::package::PackageEcosystem;
use eggsearch::core::security_applicability::{AdvisoryRange, ApplicabilityStatus};
use eggsearch::meta::advisory_range::{
    assess_version_applicability, evaluate_clause, evaluate_range_expression,
};

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

// ---------------------------------------------------------------------------
// 1. Advisory range boundary: >= 2.0.0, < 3.0.0
// ---------------------------------------------------------------------------

#[test]
fn range_boundary_below_floor_is_not_affected() {
    let ranges = vec![make_range(
        PackageEcosystem::Npm,
        Some(">= 2.0.0, < 3.0.0"),
        vec!["3.0.0"],
        vec![],
    )];
    let outcome = assess_version_applicability("1.9.9", &ranges, &PackageEcosystem::Npm);
    assert_eq!(
        outcome.status,
        ApplicabilityStatus::NotAffected,
        "version 1.9.9 below >= 2.0.0 floor must be NotAffected (not Affected)"
    );
}

#[test]
fn range_boundary_inside_is_affected() {
    let ranges = vec![make_range(
        PackageEcosystem::Npm,
        Some(">= 2.0.0, < 3.0.0"),
        vec!["3.0.0"],
        vec![],
    )];
    let outcome = assess_version_applicability("2.5.0", &ranges, &PackageEcosystem::Npm);
    assert_eq!(
        outcome.status,
        ApplicabilityStatus::Affected,
        "version 2.5.0 inside >= 2.0.0, < 3.0.0 must be Affected"
    );
}

#[test]
fn range_boundary_at_fixed_is_not_affected() {
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
        "version 3.0.0 matches fixed version, must be NotAffected"
    );
}

// ---------------------------------------------------------------------------
// 2. Unknown range syntax returns Unknown
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
        "unparseable range 'banana' must return Unknown, not Affected or NotAffected"
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

// ---------------------------------------------------------------------------
// 3. Unsupported OSV GIT range returns Unknown
// ---------------------------------------------------------------------------

#[test]
fn unsupported_osv_git_range_returns_unknown() {
    let ranges = vec![make_range(
        PackageEcosystem::CratesIo,
        Some("GIT:abc123def"),
        vec![],
        vec![],
    )];
    let outcome = assess_version_applicability("1.0.0", &ranges, &PackageEcosystem::CratesIo);
    assert_eq!(
        outcome.status,
        ApplicabilityStatus::Unknown,
        "unsupported GIT range must return Unknown"
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

// ---------------------------------------------------------------------------
// 4. Multiple ranges: one Affected dominates
// ---------------------------------------------------------------------------

#[test]
fn multiple_ranges_affected_dominates_not_affected() {
    let ranges = vec![
        make_range(PackageEcosystem::CratesIo, Some(">= 5.0.0"), vec![], vec![]),
        make_range(
            PackageEcosystem::CratesIo,
            Some(">= 1.0.0, < 2.0.0"),
            vec![],
            vec![],
        ),
    ];
    // 1.5.0 is outside >= 5.0.0 (NotAffected) but inside >= 1.0.0, < 2.0.0 (Affected)
    let outcome = assess_version_applicability("1.5.0", &ranges, &PackageEcosystem::CratesIo);
    assert_eq!(
        outcome.status,
        ApplicabilityStatus::Affected,
        "Affected must dominate NotAffected across multiple ranges"
    );
}

// ---------------------------------------------------------------------------
// 5. Multiple ranges: one Unknown collapses
// ---------------------------------------------------------------------------

#[test]
fn multiple_ranges_unknown_collapses_with_not_affected() {
    let ranges = vec![
        make_range(PackageEcosystem::CratesIo, Some(">= 5.0.0"), vec![], vec![]),
        make_range(PackageEcosystem::CratesIo, Some("banana"), vec![], vec![]),
    ];
    // 1.5.0 is outside >= 5.0.0 (NotAffected) but 'banana' is unparseable (Unknown)
    let outcome = assess_version_applicability("1.5.0", &ranges, &PackageEcosystem::CratesIo);
    assert_eq!(
        outcome.status,
        ApplicabilityStatus::Unknown,
        "Unknown + NotAffected must collapse to Unknown (conservative)"
    );
}

// ---------------------------------------------------------------------------
// Table-driven operator tests (would have caught inverted >=)
// ---------------------------------------------------------------------------

fn assert_clause(version: &str, clause: &str, expected: Option<bool>) {
    let actual = evaluate_clause(version, clause, &PackageEcosystem::CratesIo);
    assert_eq!(
        actual, expected,
        "evaluate_clause(version={version}, clause=\"{clause}\") = {actual:?}, expected {expected:?}"
    );
}

#[test]
fn table_driven_ge_operator() {
    assert_clause("1.9.9", ">= 2.0.0", Some(false));
    assert_clause("2.0.0", ">= 2.0.0", Some(true));
    assert_clause("2.1.0", ">= 2.0.0", Some(true));
}

#[test]
fn table_driven_gt_operator() {
    assert_clause("2.0.0", "> 2.0.0", Some(false));
    assert_clause("2.0.1", "> 2.0.0", Some(true));
}

#[test]
fn table_driven_le_operator() {
    assert_clause("1.9.9", "<= 2.0.0", Some(true));
    assert_clause("2.0.0", "<= 2.0.0", Some(true));
    assert_clause("2.0.1", "<= 2.0.0", Some(false));
}

#[test]
fn table_driven_lt_operator() {
    assert_clause("1.9.9", "< 2.0.0", Some(true));
    assert_clause("2.0.0", "< 2.0.0", Some(false));
}

#[test]
fn table_driven_eq_operator() {
    assert_clause("2.0.0", "= 2.0.0", Some(true));
    assert_clause("2.0.1", "= 2.0.0", Some(false));
}

#[test]
fn table_driven_intersection() {
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

// ---------------------------------------------------------------------------
// Empty ranges and fixed-version exact match
// ---------------------------------------------------------------------------

#[test]
fn empty_ranges_return_unknown() {
    let outcome = assess_version_applicability("1.0.0", &[], &PackageEcosystem::CratesIo);
    assert_eq!(
        outcome.status,
        ApplicabilityStatus::Unknown,
        "empty ranges must return Unknown"
    );
}

#[test]
fn fixed_version_exact_match_is_not_affected() {
    let ranges = vec![make_range(
        PackageEcosystem::CratesIo,
        Some(">= 1.0.0, < 2.0.0"),
        vec!["2.0.0"],
        vec![],
    )];
    let outcome = assess_version_applicability("2.0.0", &ranges, &PackageEcosystem::CratesIo);
    assert_eq!(
        outcome.status,
        ApplicabilityStatus::NotAffected,
        "version matching fixed version must be NotAffected"
    );
}

// ---------------------------------------------------------------------------
// Edge case: all unknown ranges stay unknown
// ---------------------------------------------------------------------------

#[test]
fn multiple_unknown_ranges_all_stay_unknown() {
    let ranges = vec![
        make_range(PackageEcosystem::CratesIo, Some("banana"), vec![], vec![]),
        make_range(PackageEcosystem::CratesIo, Some("apple"), vec![], vec![]),
    ];
    let outcome = assess_version_applicability("1.0.0", &ranges, &PackageEcosystem::CratesIo);
    assert_eq!(
        outcome.status,
        ApplicabilityStatus::Unknown,
        "all unknown ranges must stay Unknown"
    );
}

// ---------------------------------------------------------------------------
// Edge case: all evaluated NotAffected stays NotAffected
// ---------------------------------------------------------------------------

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
    assert_eq!(
        outcome.status,
        ApplicabilityStatus::NotAffected,
        "all ranges evaluated as NotAffected must stay NotAffected"
    );
}

// ---------------------------------------------------------------------------
// Edge case: mixed known-false then unknown stays NotAffected
// (the known-false clause short-circuits before the unknown clause)
// ---------------------------------------------------------------------------

#[test]
fn known_false_then_unknown_stays_not_affected() {
    let eco = PackageEcosystem::CratesIo;
    // >= 1.0.0 fails for version 0.5.0, so banana clause is never reached
    assert_eq!(
        evaluate_range_expression("0.5.0", ">= 1.0.0, banana", &eco),
        Some(false),
        "known-false clause must short-circuit before unknown clause"
    );
}

// ---------------------------------------------------------------------------
// Edge case: known-true then unknown collapses to Unknown
// ---------------------------------------------------------------------------

#[test]
fn known_true_then_unknown_collapses_to_unknown() {
    let eco = PackageEcosystem::CratesIo;
    // >= 1.0.0 passes for 1.5.0, but banana is unknown -> whole range unknown
    assert_eq!(
        evaluate_range_expression("1.5.0", ">= 1.0.0, banana", &eco),
        None,
        "known-true then unknown clause must collapse to Unknown"
    );
}
