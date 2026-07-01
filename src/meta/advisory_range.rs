use crate::core::package::PackageEcosystem;
use crate::core::security::VulnerabilityMetadata;
use crate::core::security_applicability::AdvisoryRange;

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
pub fn version_in_ranges(version: &str, ranges: &[AdvisoryRange]) -> (bool, Vec<String>) {
    let mut reasons = Vec::new();

    for range in ranges {
        if range.fixed_versions.iter().any(|fixed| fixed == version) {
            reasons.push(format!(
                "version {} matches fixed version in advisory from {}",
                version, range.source
            ));
            return (false, reasons);
        }

        if !range.last_affected_versions.is_empty() {
            if range.last_affected_versions.iter().any(|la| la == version) {
                reasons.push(format!(
                    "version {} matches last affected version in advisory from {}",
                    version, range.source
                ));
                return (true, reasons);
            }
            reasons.push(format!(
                "version {} not in affected version list from {}",
                version, range.source
            ));
            continue;
        }

        if let Some(ref affected_range) = range.affected_range {
            if evaluate_range_string(version, affected_range) {
                reasons.push(format!(
                    "version {} matches affected range '{}' from {}",
                    version, affected_range, range.source
                ));
                return (true, reasons);
            }
        }
    }

    if reasons.is_empty() {
        reasons.push("no structured advisory ranges available for comparison".to_string());
    }

    (false, reasons)
}

/// Simple range string evaluator for common patterns.
/// This is intentionally conservative — returns false for unparseable ranges.
fn evaluate_range_string(version: &str, range: &str) -> bool {
    let parts: Vec<&str> = range.split(',').map(|s| s.trim()).collect();

    for part in parts {
        let part = part.trim();
        if let Some(ver) = part.strip_prefix(">=") {
            let ver = ver.trim();
            if compare_versions(version, ver) == std::cmp::Ordering::Less {
                return false;
            }
        } else if let Some(ver) = part.strip_prefix('>') {
            let ver = ver.trim();
            if compare_versions(version, ver) != std::cmp::Ordering::Greater {
                return false;
            }
        } else if let Some(ver) = part.strip_prefix("<=") {
            let ver = ver.trim();
            if compare_versions(version, ver) == std::cmp::Ordering::Greater {
                return false;
            }
        } else if let Some(ver) = part.strip_prefix('<') {
            let ver = ver.trim();
            if compare_versions(version, ver) != std::cmp::Ordering::Less {
                return false;
            }
        } else if let Some(ver) = part.strip_prefix('=') {
            let ver = ver.trim();
            if version != ver {
                return false;
            }
        }
    }

    true
}

/// Compare two version strings using simple numeric segment comparison.
/// Returns ordering for segments that can be compared, Equal if identical.
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let a_parts: Vec<u64> = a
        .split('.')
        .filter_map(|s| s.split('-').next()?.parse().ok())
        .collect();
    let b_parts: Vec<u64> = b
        .split('.')
        .filter_map(|s| s.split('-').next()?.parse().ok())
        .collect();

    for (a_val, b_val) in a_parts.iter().zip(b_parts.iter()) {
        match a_val.cmp(b_val) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }

    a_parts.len().cmp(&b_parts.len())
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
        let (affected, reasons) = version_in_ranges("1.5.0", &ranges);
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
        let (affected, _) = version_in_ranges("1.5.0", &ranges);
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
        let (affected, _) = version_in_ranges("2.0.0", &ranges);
        assert!(!affected);
    }

    #[test]
    fn compare_versions_basic() {
        assert_eq!(
            compare_versions("1.0.0", "1.0.0"),
            std::cmp::Ordering::Equal
        );
        assert_eq!(
            compare_versions("2.0.0", "1.0.0"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            compare_versions("1.0.0", "2.0.0"),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            compare_versions("1.2.3", "1.2.4"),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn empty_ranges_return_unknown() {
        let (affected, reasons) = version_in_ranges("1.0.0", &[]);
        assert!(!affected);
        assert!(reasons
            .iter()
            .any(|r| r.contains("no structured advisory ranges")));
    }
}
