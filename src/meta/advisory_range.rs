use crate::core::package::PackageEcosystem;
use crate::core::security::VulnerabilityMetadata;
use crate::core::security_applicability::AdvisoryRange;
use crate::meta::version_compare::{compare_versions_for_ecosystem, version_satisfies_range};

/// Extract advisory ranges from vulnerability metadata.
/// Returns empty vec if no structured ranges can be extracted.
pub fn extract_advisory_ranges(vuln: &VulnerabilityMetadata) -> Vec<AdvisoryRange> {
    let mut ranges = Vec::new();

    let ecosystem = vuln
        .ecosystem
        .as_deref()
        .and_then(PackageEcosystem::parse)
        .unwrap_or(PackageEcosystem::CratesIo);
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
            source: vuln
                .source
                .as_str()
                .to_string(),
        });
    }

    ranges
}

/// Check if a version string falls within advisory ranges.
/// Returns (is_affected, reasons) tuple.
pub fn version_in_ranges(
    version: &str,
    ranges: &[AdvisoryRange],
    ecosystem: &PackageEcosystem,
) -> (bool, Vec<String>) {
    let mut reasons = Vec::new();

    for range in ranges {
        // Check fixed versions first — if the version matches a fix, it's not affected
        if range.fixed_versions.iter().any(|fixed| fixed == version) {
            reasons.push(format!(
                "version {version} matches fixed version in advisory from {}",
                range.source
            ));
            return (false, reasons);
        }

        // Check last_affected versions — if listed, version is affected
        if !range.last_affected_versions.is_empty() {
            if range
                .last_affected_versions
                .iter()
                .any(|la| la == version)
            {
                reasons.push(format!(
                    "version {version} matches last affected version in advisory from {}",
                    range.source
                ));
                return (true, reasons);
            }
            reasons.push(format!(
                "version {version} not in affected version list from {}",
                range.source
            ));
            continue;
        }

        // Check affected range expression using ecosystem-aware comparison
        if let Some(ref affected_range) = range.affected_range {
            match evaluate_range(version, affected_range, ecosystem) {
                Some(true) => {
                    reasons.push(format!(
                        "version {version} matches affected range '{affected_range}' from {}",
                        range.source
                    ));
                    return (true, reasons);
                }
                Some(false) => {}
                None => {
                    reasons.push(format!(
                        "could not evaluate range '{affected_range}' for version {version} from {}",
                        range.source
                    ));
                }
            }
        }
    }

    if reasons.is_empty() {
        reasons.push("no structured advisory ranges available for comparison".to_string());
    }

    (false, reasons)
}

/// Evaluate a version against a range expression using ecosystem-aware comparison.
/// Returns Some(true) if the version satisfies the range, Some(false) if not,
/// or None if the range cannot be evaluated.
fn evaluate_range(
    version: &str,
    range: &str,
    ecosystem: &PackageEcosystem,
) -> Option<bool> {
    // Try ecosystem-aware range evaluation first
    if let Some(result) = version_satisfies_range(ecosystem, version, range) {
        return Some(result);
    }

    // Fall back to simple range evaluation for unsupported range syntax
    let parts: Vec<&str> = range.split(',').map(|s| s.trim()).collect();

    for part in parts {
        let part = part.trim();
        if let Some(ver) = part.strip_prefix(">=") {
            let ver = ver.trim();
            match compare_versions_for_ecosystem(ecosystem, version, ver) {
                Some(std::cmp::Ordering::Less) | Some(std::cmp::Ordering::Equal) => {}
                _ => return Some(false),
            }
        } else if let Some(ver) = part.strip_prefix('>') {
            let ver = ver.trim();
            if compare_versions_for_ecosystem(ecosystem, version, ver)
                != Some(std::cmp::Ordering::Greater)
            {
                return Some(false);
            }
        } else if let Some(ver) = part.strip_prefix("<=") {
            let ver = ver.trim();
            if let Some(std::cmp::Ordering::Greater) =
                compare_versions_for_ecosystem(ecosystem, version, ver)
            {
                return Some(false);
            }
        } else if let Some(ver) = part.strip_prefix('<') {
            let ver = ver.trim();
            if compare_versions_for_ecosystem(ecosystem, version, ver)
                != Some(std::cmp::Ordering::Less)
            {
                return Some(false);
            }
        } else if let Some(ver) = part.strip_prefix('=') {
            let ver = ver.trim();
            if compare_versions_for_ecosystem(ecosystem, version, ver)
                != Some(std::cmp::Ordering::Equal)
            {
                return Some(false);
            }
        }
    }

    Some(true)
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

    #[test]
    fn extract_ranges_from_metadata() {
        let vuln = make_vuln(vec![" >= 1.0.0"], vec!["1.2.3"]);
        let ranges = extract_advisory_ranges(&vuln);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].fixed_versions, vec!["1.2.3"]);
    }

    #[test]
    fn version_below_introduced_not_affected() {
        let ranges = vec![AdvisoryRange {
            ecosystem: PackageEcosystem::CratesIo,
            package: "test".to_string(),
            affected_range: Some(">= 2.0.0, < 3.0.0".to_string()),
            fixed_versions: vec!["3.0.0".to_string()],
            introduced_versions: vec!["2.0.0".to_string()],
            last_affected_versions: Vec::new(),
            source: "test".to_string(),
        }];
        let (affected, reasons) =
            version_in_ranges("1.5.0", &ranges, &PackageEcosystem::CratesIo);
        assert!(!affected);
        assert!(!reasons.is_empty());
    }

    #[test]
    fn version_in_affected_range() {
        let ranges = vec![AdvisoryRange {
            ecosystem: PackageEcosystem::CratesIo,
            package: "test".to_string(),
            affected_range: Some(">= 1.0.0, < 2.0.0".to_string()),
            fixed_versions: vec!["2.0.0".to_string()],
            introduced_versions: vec!["1.0.0".to_string()],
            last_affected_versions: Vec::new(),
            source: "test".to_string(),
        }];
        let (affected, _) = version_in_ranges("1.5.0", &ranges, &PackageEcosystem::CratesIo);
        assert!(affected);
    }

    #[test]
    fn version_equal_to_fixed_not_affected() {
        let ranges = vec![AdvisoryRange {
            ecosystem: PackageEcosystem::CratesIo,
            package: "test".to_string(),
            affected_range: None,
            fixed_versions: vec!["2.0.0".to_string()],
            introduced_versions: Vec::new(),
            last_affected_versions: Vec::new(),
            source: "test".to_string(),
        }];
        let (affected, _) = version_in_ranges("2.0.0", &ranges, &PackageEcosystem::CratesIo);
        assert!(!affected);
    }

    #[test]
    fn empty_ranges_return_unknown() {
        let (affected, reasons) =
            version_in_ranges("1.0.0", &[], &PackageEcosystem::CratesIo);
        assert!(!affected);
        assert!(reasons
            .iter()
            .any(|r| r.contains("no structured advisory ranges")));
    }

    #[test]
    fn last_affected_version_is_affected() {
        let ranges = vec![AdvisoryRange {
            ecosystem: PackageEcosystem::Npm,
            package: "test".to_string(),
            affected_range: None,
            fixed_versions: Vec::new(),
            introduced_versions: Vec::new(),
            last_affected_versions: vec!["1.2.3".to_string()],
            source: "test".to_string(),
        }];
        let (affected, _) = version_in_ranges("1.2.3", &ranges, &PackageEcosystem::Npm);
        assert!(affected);
    }

    #[test]
    fn last_affected_version_not_matching() {
        let ranges = vec![AdvisoryRange {
            ecosystem: PackageEcosystem::Npm,
            package: "test".to_string(),
            affected_range: None,
            fixed_versions: Vec::new(),
            introduced_versions: Vec::new(),
            last_affected_versions: vec!["1.2.3".to_string()],
            source: "test".to_string(),
        }];
        let (affected, _) = version_in_ranges("1.2.4", &ranges, &PackageEcosystem::Npm);
        assert!(!affected);
    }

    #[test]
    fn maven_range_evaluation() {
        let ranges = vec![AdvisoryRange {
            ecosystem: PackageEcosystem::Maven,
            package: "test".to_string(),
            affected_range: Some(">= 2.0.0".to_string()),
            fixed_versions: vec!["2.5.0".to_string()],
            introduced_versions: Vec::new(),
            last_affected_versions: Vec::new(),
            source: "test".to_string(),
        }];
        let (affected, _) = version_in_ranges("2.3.0", &ranges, &PackageEcosystem::Maven);
        assert!(affected);
    }
}
