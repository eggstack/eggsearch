use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyRelation, DependencySource,
};

/// Parse Cargo.lock (TOML-based, [package] sections with name/version)
pub(crate) fn parse_cargo_lock(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut in_package = false;
    let mut name = String::new();
    let mut version = String::new();
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();

        if trimmed == "[[package]]" {
            // Flush previous entry
            if !name.is_empty() {
                findings.push(DependencyFinding {
                    ecosystem: PackageEcosystem::CratesIo,
                    package: name.clone(),
                    version: if version.is_empty() {
                        None
                    } else {
                        Some(version.clone())
                    },
                    source_file: Some(path.to_string()),
                    source_line: Some(line_num.saturating_sub(2)),
                    source_kind: DependencySource::LockFile,
                    confidence: Some(ApplicabilityConfidence::High),
                    relation: Some(DependencyRelation::Transitive),
                });
            }
            in_package = true;
            name.clear();
            version.clear();
        } else if trimmed.starts_with('[') && trimmed != "[[package]]" {
            in_package = false;
        } else if in_package {
            if let Some(val) = trimmed.strip_prefix("name = ") {
                name = val.trim_matches('"').to_string();
            } else if let Some(val) = trimmed.strip_prefix("version = ") {
                version = val.trim_matches('"').to_string();
            }
        }
    }

    // Flush last entry
    if !name.is_empty() {
        findings.push(DependencyFinding {
            ecosystem: PackageEcosystem::CratesIo,
            package: name,
            version: if version.is_empty() {
                None
            } else {
                Some(version)
            },
            source_file: Some(path.to_string()),
            source_line: Some(line_num.saturating_sub(1)),
            source_kind: DependencySource::LockFile,
            confidence: Some(ApplicabilityConfidence::High),
            relation: Some(DependencyRelation::Transitive),
        });
    }

    findings
}

/// Parse Cargo.toml for direct [dependencies] and [dev-dependencies]
pub(crate) fn parse_cargo_toml(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut in_deps = false;
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();

        if trimmed == "[dependencies]"
            || trimmed == "[dev-dependencies]"
            || trimmed == "[build-dependencies]"
        {
            in_deps = true;
        } else if trimmed.starts_with('[') {
            in_deps = false;
        } else if in_deps {
            // Parse: name = "version" or name = { version = "..." }
            if let Some(name) = trimmed.strip_suffix(" = ") {
                let name = name.trim();
                if !name.is_empty() && !name.starts_with('#') {
                    findings.push(DependencyFinding {
                        ecosystem: PackageEcosystem::CratesIo,
                        package: name.to_string(),
                        version: None,
                        source_file: Some(path.to_string()),
                        source_line: Some(line_num),
                        source_kind: DependencySource::Manifest,
                        confidence: Some(ApplicabilityConfidence::Medium),
                        relation: Some(DependencyRelation::Direct),
                    });
                }
            } else if let Some(name) = trimmed.split_once(" = ") {
                let name = name.0.trim();
                // Try to extract inline version
                let rest = &trimmed[name.len()..];
                let version = if let Some(vstart) = rest.find("version") {
                    let after_ver = &rest[vstart..];
                    after_ver
                        .split_once('"')
                        .and_then(|(_, v)| v.split_once('"').map(|(v, _)| v))
                        .map(|v| v.to_string())
                } else {
                    rest.trim()
                        .strip_prefix('"')
                        .and_then(|v| v.strip_suffix('"'))
                        .map(|v| v.to_string())
                };

                if !name.is_empty() && !name.starts_with('#') {
                    findings.push(DependencyFinding {
                        ecosystem: PackageEcosystem::CratesIo,
                        package: name.to_string(),
                        version,
                        source_file: Some(path.to_string()),
                        source_line: Some(line_num),
                        source_kind: DependencySource::Manifest,
                        confidence: Some(ApplicabilityConfidence::Medium),
                        relation: Some(DependencyRelation::Direct),
                    });
                }
            }
        }
    }

    findings
}
