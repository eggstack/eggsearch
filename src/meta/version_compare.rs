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

/// Parse a semver-like version string into numeric core segments plus
/// an optional pre-release tag. Build metadata (`+...`) is ignored.
/// Returns None when the version has no numeric core segments.
fn parse_version_parts(version: &str) -> Option<(Vec<u64>, Option<String>)> {
    let v = version.trim().trim_start_matches('v');
    let v = v.split('+').next().unwrap_or(v);
    let (core, pre) = match v.split_once('-') {
        Some((core, pre)) => (core, Some(pre.to_string())),
        None => (v, None),
    };
    let segments: Vec<u64> = core.split('.').filter_map(|s| s.parse().ok()).collect();
    if segments.is_empty() {
        None
    } else {
        Some((segments, pre))
    }
}

/// Compare two semver-like version strings.
///
/// Missing segments are padded with zeros so `1.2 == 1.2.0`. A
/// pre-release version orders below the corresponding release
/// (`2.0.0-beta < 2.0.0`); two pre-release tags compare lexically.
fn compare_semver_like(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    let (a_parts, a_pre) = parse_version_parts(a)?;
    let (b_parts, b_pre) = parse_version_parts(b)?;

    let len = a_parts.len().max(b_parts.len());
    for i in 0..len {
        let a_val = a_parts.get(i).copied().unwrap_or(0);
        let b_val = b_parts.get(i).copied().unwrap_or(0);
        match a_val.cmp(&b_val) {
            std::cmp::Ordering::Equal => continue,
            other => return Some(other),
        }
    }

    Some(match (&a_pre, &b_pre) {
        (None, None) => std::cmp::Ordering::Equal,
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(x), Some(y)) => x.cmp(y),
    })
}

/// Canonicalize a Maven qualifier: `final`/`ga`/`release` all collapse
/// to the empty (release) qualifier; others lowercase.
fn normalize_maven_qualifier(qualifier: &str) -> String {
    let lower = qualifier.to_ascii_lowercase();
    match lower.as_str() {
        "" | "final" | "ga" | "release" => String::new(),
        _ => lower,
    }
}

/// Rank a normalized Maven qualifier using Maven's well-known order:
/// `alpha < beta < milestone < rc < snapshot < release < sp`.
/// Unknown qualifiers rank with release and tie-break lexically.
fn maven_qualifier_rank(qualifier: &str) -> u8 {
    match qualifier {
        "alpha" | "a" => 0,
        "beta" | "b" => 1,
        "milestone" | "m" => 2,
        "rc" | "cr" => 3,
        "snapshot" => 4,
        "sp" => 6,
        _ => 5,
    }
}

/// Parse a Maven version into numeric segments plus a normalized
/// trailing qualifier. Tokens are split on `.`, `-`, `_`, and `+`;
/// leading numeric tokens form the segments and the first non-numeric
/// token starts the qualifier. Returns None when there is no numeric
/// segment.
fn parse_maven_version(version: &str) -> Option<(Vec<u64>, String)> {
    let v = version.trim().trim_start_matches('v');
    if v.is_empty() {
        return None;
    }
    let mut segments = Vec::new();
    let mut qualifier_tokens: Vec<&str> = Vec::new();
    for token in v.split(['.', '-', '_', '+']) {
        if qualifier_tokens.is_empty() {
            if let Ok(n) = token.parse::<u64>() {
                segments.push(n);
                continue;
            }
        }
        qualifier_tokens.push(token);
    }
    if segments.is_empty() {
        return None;
    }
    Some((
        segments,
        normalize_maven_qualifier(&qualifier_tokens.join("-")),
    ))
}

/// Compare Maven versions with qualifier support.
///
/// Numeric cores compare first (zero-padded to equal length); ties
/// break on the qualifier using Maven's well-known ordering
/// (`alpha < beta < milestone < rc < snapshot < release/final <
/// sp`). Unparseable versions fall back to lexical comparison.
fn compare_maven(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    let (a_parts, a_q) = parse_maven_version(a)?;
    let (b_parts, b_q) = parse_maven_version(b)?;

    let len = a_parts.len().max(b_parts.len());
    for i in 0..len {
        let a_val = a_parts.get(i).copied().unwrap_or(0);
        let b_val = b_parts.get(i).copied().unwrap_or(0);
        match a_val.cmp(&b_val) {
            std::cmp::Ordering::Equal => continue,
            other => return Some(other),
        }
    }

    let ord = maven_qualifier_rank(&a_q).cmp(&maven_qualifier_rank(&b_q));
    Some(if ord == std::cmp::Ordering::Equal {
        a_q.cmp(&b_q)
    } else {
        ord
    })
}

/// Evaluate a range expression against the given comparator.
/// Supports: exact, >=, >, <=, <, !=, and comma-separated intersections.
fn evaluate_range<F>(version: &str, range: &str, compare: F) -> Option<bool>
where
    F: Fn(&str, &str) -> Option<std::cmp::Ordering>,
{
    let range = range.trim();
    if range.is_empty() {
        return None;
    }

    if !range.contains(['>', '<', '=', '!', ',']) {
        return compare(version, range).map(|ord| ord == std::cmp::Ordering::Equal);
    }

    for part in range.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        #[allow(clippy::question_mark)]
        let satisfied = if let Some(ver) = part.strip_prefix(">=") {
            compare(version, ver.trim()).map(|ord| ord != std::cmp::Ordering::Less)
        } else if let Some(ver) = part.strip_prefix('>') {
            compare(version, ver.trim()).map(|ord| ord == std::cmp::Ordering::Greater)
        } else if let Some(ver) = part.strip_prefix("<=") {
            compare(version, ver.trim()).map(|ord| ord != std::cmp::Ordering::Greater)
        } else if let Some(ver) = part.strip_prefix('<') {
            compare(version, ver.trim()).map(|ord| ord == std::cmp::Ordering::Less)
        } else if let Some(ver) = part.strip_prefix("!=") {
            compare(version, ver.trim()).map(|ord| ord != std::cmp::Ordering::Equal)
        } else if let Some(ver) = part.strip_prefix('=') {
            compare(version, ver.trim()).map(|ord| ord == std::cmp::Ordering::Equal)
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

/// Evaluate a semver range expression.
fn evaluate_semver_range(version: &str, range: &str) -> Option<bool> {
    evaluate_range(version, range, compare_semver_like)
}

/// Evaluate a Maven range expression.
fn evaluate_maven_range(version: &str, range: &str) -> Option<bool> {
    evaluate_range(version, range, compare_maven)
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

    #[test]
    fn missing_segments_padded_with_zeros() {
        assert_eq!(
            compare_versions_for_ecosystem(&PackageEcosystem::Npm, "1.2", "1.2.0"),
            Some(std::cmp::Ordering::Equal)
        );
        assert_eq!(
            compare_versions_for_ecosystem(&PackageEcosystem::Pypi, "1.2", "1.2.1"),
            Some(std::cmp::Ordering::Less)
        );
        assert_eq!(
            compare_versions_for_ecosystem(&PackageEcosystem::CratesIo, "1.10", "1.9"),
            Some(std::cmp::Ordering::Greater)
        );
    }

    #[test]
    fn prerelease_orders_below_release() {
        assert_eq!(
            compare_versions_for_ecosystem(&PackageEcosystem::Npm, "2.0.0-beta", "2.0.0"),
            Some(std::cmp::Ordering::Less)
        );
        assert_eq!(
            compare_versions_for_ecosystem(&PackageEcosystem::Npm, "2.0.0", "2.0.0-beta"),
            Some(std::cmp::Ordering::Greater)
        );
        assert_eq!(
            version_satisfies_range(&PackageEcosystem::Npm, "2.0.0-beta", "< 2.0.0"),
            Some(true)
        );
    }

    #[test]
    fn prerelease_tags_compare_lexically() {
        assert_eq!(
            compare_versions_for_ecosystem(
                &PackageEcosystem::CratesIo,
                "2.0.0-alpha",
                "2.0.0-beta"
            ),
            Some(std::cmp::Ordering::Less)
        );
    }

    #[test]
    fn maven_qualifier_ordering() {
        let maven = PackageEcosystem::Maven;
        assert_eq!(
            compare_versions_for_ecosystem(&maven, "1.0-alpha", "1.0-beta"),
            Some(std::cmp::Ordering::Less)
        );
        assert_eq!(
            compare_versions_for_ecosystem(&maven, "1.0-beta", "1.0-rc"),
            Some(std::cmp::Ordering::Less)
        );
        assert_eq!(
            compare_versions_for_ecosystem(&maven, "1.0-rc", "1.0"),
            Some(std::cmp::Ordering::Less)
        );
        assert_eq!(
            compare_versions_for_ecosystem(&maven, "1.0-snapshot", "1.0"),
            Some(std::cmp::Ordering::Less)
        );
        assert_eq!(
            compare_versions_for_ecosystem(&maven, "1.0-final", "1.0"),
            Some(std::cmp::Ordering::Equal)
        );
        assert_eq!(
            version_satisfies_range(&maven, "1.0-alpha", "< 1.0"),
            Some(true)
        );
    }
}
