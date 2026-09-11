use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyRelation, DependencySource,
};

/// Parse requirements.txt / requirements.in
pub(crate) fn parse_requirements_txt(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
            continue;
        }

        // Format: package[extras]>=version,==version
        let pkg = trimmed
            .split(['>', '<', '=', '!', '[', ';'])
            .next()
            .unwrap_or(trimmed)
            .trim();

        if pkg.is_empty()
            || !pkg
                .chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_')
        {
            continue;
        }

        // Only extract exact pinned versions (==), not ranges (>=, <=, ~=).
        // Version ranges are not resolved to single versions — callers
        // should treat version as None for range specifiers.
        let version = trimmed
            .find("==")
            .map(|idx| {
                trimmed[idx + 2..]
                    .split([';', '#'])
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string()
            })
            .filter(|v| !v.is_empty());

        let confidence = if version.is_some() {
            ApplicabilityConfidence::Medium
        } else {
            ApplicabilityConfidence::Low
        };

        findings.push(DependencyFinding {
            ecosystem: PackageEcosystem::Pypi,
            package: pkg.to_string(),
            version,
            source_file: Some(path.to_string()),
            source_line: Some(line_num),
            source_kind: DependencySource::Manifest,
            confidence: Some(confidence),
            relation: Some(DependencyRelation::Direct),
        });
    }

    findings
}

/// Parse poetry.lock (TOML-based, similar to Cargo.lock)
pub(crate) fn parse_poetry_lock(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut in_package = false;
    let mut name = String::new();
    let mut version = String::new();
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();

        if trimmed == "[[package]]" {
            if !name.is_empty() {
                findings.push(DependencyFinding {
                    ecosystem: PackageEcosystem::Pypi,
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

    if !name.is_empty() {
        findings.push(DependencyFinding {
            ecosystem: PackageEcosystem::Pypi,
            package: name,
            version: if version.is_empty() {
                None
            } else {
                Some(version)
            },
            source_file: Some(path.to_string()),
            source_line: Some(line_num),
            source_kind: DependencySource::LockFile,
            confidence: Some(ApplicabilityConfidence::High),
            relation: Some(DependencyRelation::Transitive),
        });
    }

    findings
}

/// Parse Pipfile.lock (JSON format)
pub(crate) fn parse_pipfile_lock(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();

    if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
        for section in &["default", "develop"] {
            if let Some(deps) = json.get(*section).and_then(|v| v.as_object()) {
                for (name, val) in deps {
                    let version = val
                        .get("version")
                        .and_then(|v| v.as_str())
                        .map(|s| s.trim_start_matches("==").to_string());
                    findings.push(DependencyFinding {
                        ecosystem: PackageEcosystem::Pypi,
                        package: name.clone(),
                        version,
                        source_file: Some(path.to_string()),
                        source_line: None,
                        source_kind: DependencySource::LockFile,
                        confidence: Some(ApplicabilityConfidence::High),
                        relation: Some(DependencyRelation::Transitive),
                    });
                }
            }
        }
    }

    findings
}

/// Parse uv.lock (TOML-based, similar to Cargo.lock)
pub(crate) fn parse_uv_lock(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut in_package = false;
    let mut name = String::new();
    let mut version = String::new();
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();

        if trimmed == "[[package]]" {
            if !name.is_empty() {
                findings.push(DependencyFinding {
                    ecosystem: PackageEcosystem::Pypi,
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

    if !name.is_empty() {
        findings.push(DependencyFinding {
            ecosystem: PackageEcosystem::Pypi,
            package: name,
            version: if version.is_empty() {
                None
            } else {
                Some(version)
            },
            source_file: Some(path.to_string()),
            source_line: Some(line_num),
            source_kind: DependencySource::LockFile,
            confidence: Some(ApplicabilityConfidence::High),
            relation: Some(DependencyRelation::Transitive),
        });
    }

    findings
}
