use crate::core::package::PackageEcosystem;

/// Compare two version strings for the given ecosystem.
/// Returns None if versions cannot be compared reliably.
pub fn compare_versions_for_ecosystem(
    ecosystem: &PackageEcosystem,
    a: &str,
    b: &str,
) -> Option<std::cmp::Ordering> {
    match ecosystem {
        PackageEcosystem::CratesIo
        | PackageEcosystem::Npm
        | PackageEcosystem::Go
        | PackageEcosystem::Nuget
        | PackageEcosystem::Rubygems
        | PackageEcosystem::Packagist
        | PackageEcosystem::Pypi => compare_semver_like(a, b),

        PackageEcosystem::Maven => compare_maven(a, b),

        PackageEcosystem::Oci | PackageEcosystem::GithubActions => {
            if a == b {
                Some(std::cmp::Ordering::Equal)
            } else {
                None
            }
        }
    }
}

/// Check if a version satisfies a range expression.
/// Returns None if the range cannot be evaluated.
pub fn version_satisfies_range(
    ecosystem: &PackageEcosystem,
    version: &str,
    range: &str,
) -> Option<bool> {
    match ecosystem {
        PackageEcosystem::CratesIo
        | PackageEcosystem::Npm
        | PackageEcosystem::Go
        | PackageEcosystem::Nuget
        | PackageEcosystem::Rubygems
        | PackageEcosystem::Packagist
        | PackageEcosystem::Pypi => evaluate_semver_range(version, range),

        PackageEcosystem::Maven => evaluate_maven_range(version, range),

        PackageEcosystem::Oci | PackageEcosystem::GithubActions => Some(range.trim() == version),
    }
}

/// Parse a semver-like version string into numeric segments.
/// Strips pre-release suffixes for comparison.
fn parse_version_segments(version: &str) -> Option<Vec<u64>> {
    let v = version.trim().trim_start_matches('v');
    let segments: Vec<u64> = v
        .split(['.', '-', '+'])
        .filter_map(|s| s.parse().ok())
        .collect();
    if segments.is_empty() {
        None
    } else {
        Some(segments)
    }
}

/// Compare two semver-like version strings.
fn compare_semver_like(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    let a_parts = parse_version_segments(a)?;
    let b_parts = parse_version_segments(b)?;

    for (a_val, b_val) in a_parts.iter().zip(b_parts.iter()) {
        match a_val.cmp(b_val) {
            std::cmp::Ordering::Equal => continue,
            other => return Some(other),
        }
    }

    Some(a_parts.len().cmp(&b_parts.len()))
}

/// Compare Maven versions with qualifier support.
/// Simple lexical comparison for qualifier-bearing versions.
fn compare_maven(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    if let Some(ordering) = compare_semver_like(a, b) {
        return Some(ordering);
    }
    Some(a.cmp(b))
}

/// Evaluate a semver range expression.
/// Supports: exact, >=, >, <=, <, !=, and comma-separated intersections.
fn evaluate_semver_range(version: &str, range: &str) -> Option<bool> {
    let range = range.trim();
    if range.is_empty() {
        return None;
    }

    if !range.contains(['>', '<', '=', '!', ',']) {
        return compare_semver_like(version, range).map(|ord| ord == std::cmp::Ordering::Equal);
    }

    for part in range.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        #[allow(clippy::question_mark)]
        let satisfied = if let Some(ver) = part.strip_prefix(">=") {
            compare_semver_like(version, ver.trim()).map(|ord| ord != std::cmp::Ordering::Less)
        } else if let Some(ver) = part.strip_prefix('>') {
            compare_semver_like(version, ver.trim()).map(|ord| ord == std::cmp::Ordering::Greater)
        } else if let Some(ver) = part.strip_prefix("<=") {
            compare_semver_like(version, ver.trim()).map(|ord| ord != std::cmp::Ordering::Greater)
        } else if let Some(ver) = part.strip_prefix('<') {
            compare_semver_like(version, ver.trim()).map(|ord| ord == std::cmp::Ordering::Less)
        } else if let Some(ver) = part.strip_prefix("!=") {
            compare_semver_like(version, ver.trim()).map(|ord| ord != std::cmp::Ordering::Equal)
        } else if let Some(ver) = part.strip_prefix('=') {
            compare_semver_like(version, ver.trim()).map(|ord| ord == std::cmp::Ordering::Equal)
        } else {
            return None;
        };

        match satisfied {
            Some(false) => return Some(false),
            None => return None,
            Some(true) => continue,
        }
    }

    Some(true)
}

/// Evaluate a Maven range expression.
fn evaluate_maven_range(version: &str, range: &str) -> Option<bool> {
    evaluate_semver_range(version, range)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_equal() {
        assert_eq!(
            compare_versions_for_ecosystem(&PackageEcosystem::CratesIo, "1.0.0", "1.0.0"),
            Some(std::cmp::Ordering::Equal)
        );
    }

    #[test]
    fn semver_greater() {
        assert_eq!(
            compare_versions_for_ecosystem(&PackageEcosystem::Npm, "2.0.0", "1.0.0"),
            Some(std::cmp::Ordering::Greater)
        );
    }

    #[test]
    fn semver_less() {
        assert_eq!(
            compare_versions_for_ecosystem(&PackageEcosystem::Go, "1.0.0", "2.0.0"),
            Some(std::cmp::Ordering::Less)
        );
    }

    #[test]
    fn oci_exact_match() {
        assert_eq!(
            compare_versions_for_ecosystem(&PackageEcosystem::Oci, "v1.2.3", "v1.2.3"),
            Some(std::cmp::Ordering::Equal)
        );
    }

    #[test]
    fn oci_no_partial() {
        assert_eq!(
            compare_versions_for_ecosystem(&PackageEcosystem::Oci, "v1.2.3", "v1.2.4"),
            None
        );
    }

    #[test]
    fn range_exact() {
        assert_eq!(
            version_satisfies_range(&PackageEcosystem::CratesIo, "1.5.0", "1.5.0"),
            Some(true)
        );
    }

    #[test]
    fn range_gte() {
        assert_eq!(
            version_satisfies_range(&PackageEcosystem::CratesIo, "1.5.0", ">= 1.0.0"),
            Some(true)
        );
        assert_eq!(
            version_satisfies_range(&PackageEcosystem::CratesIo, "0.9.0", ">= 1.0.0"),
            Some(false)
        );
    }

    #[test]
    fn range_lt() {
        assert_eq!(
            version_satisfies_range(&PackageEcosystem::CratesIo, "1.5.0", "< 2.0.0"),
            Some(true)
        );
        assert_eq!(
            version_satisfies_range(&PackageEcosystem::CratesIo, "2.0.0", "< 2.0.0"),
            Some(false)
        );
    }

    #[test]
    fn range_intersection() {
        assert_eq!(
            version_satisfies_range(&PackageEcosystem::Npm, "1.5.0", ">= 1.0.0, < 2.0.0"),
            Some(true)
        );
        assert_eq!(
            version_satisfies_range(&PackageEcosystem::Npm, "2.5.0", ">= 1.0.0, < 2.0.0"),
            Some(false)
        );
    }

    #[test]
    fn range_not_equal() {
        assert_eq!(
            version_satisfies_range(&PackageEcosystem::CratesIo, "1.5.0", "!= 1.5.0"),
            Some(false)
        );
        assert_eq!(
            version_satisfies_range(&PackageEcosystem::CratesIo, "1.5.1", "!= 1.5.0"),
            Some(true)
        );
    }

    #[test]
    fn unparseable_version_returns_none() {
        assert_eq!(
            compare_versions_for_ecosystem(&PackageEcosystem::CratesIo, "not-a-version", "1.0.0"),
            None
        );
    }
}
