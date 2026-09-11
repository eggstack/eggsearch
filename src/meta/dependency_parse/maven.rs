use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyRelation, DependencySource,
};

use super::extract_xml_tag;

/// Parse pom.xml (Maven) - best-effort extraction of groupId/artifactId/version
pub(crate) fn parse_pom_xml(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut line_num = 0u32;
    let mut in_deps = false;
    let mut artifact_id = String::new();
    let mut group_id = String::new();
    let mut version = String::new();

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();

        if trimmed.contains("<dependencies>") {
            in_deps = true;
        } else if trimmed.contains("</dependencies>") {
            in_deps = false;
            // Flush last
            if !artifact_id.is_empty() {
                findings.push(DependencyFinding {
                    ecosystem: PackageEcosystem::Maven,
                    package: if group_id.is_empty() {
                        artifact_id.clone()
                    } else {
                        format!("{group_id}:{artifact_id}")
                    },
                    version: if version.is_empty() {
                        None
                    } else {
                        Some(version.clone())
                    },
                    source_file: Some(path.to_string()),
                    source_line: Some(line_num),
                    source_kind: DependencySource::Manifest,
                    confidence: Some(ApplicabilityConfidence::Medium),
                    relation: Some(DependencyRelation::Direct),
                });
            }
            artifact_id.clear();
            group_id.clear();
            version.clear();
        } else if in_deps {
            if let Some(val) = extract_xml_tag(trimmed, "groupId") {
                group_id = val;
            } else if let Some(val) = extract_xml_tag(trimmed, "artifactId") {
                artifact_id = val;
            } else if let Some(val) = extract_xml_tag(trimmed, "version") {
                version = val;
            }

            // Detect closing </dependency>
            if trimmed.contains("</dependency>") {
                if !artifact_id.is_empty() {
                    findings.push(DependencyFinding {
                        ecosystem: PackageEcosystem::Maven,
                        package: if group_id.is_empty() {
                            artifact_id.clone()
                        } else {
                            format!("{group_id}:{artifact_id}")
                        },
                        version: if version.is_empty() {
                            None
                        } else {
                            Some(version.clone())
                        },
                        source_file: Some(path.to_string()),
                        source_line: Some(line_num),
                        source_kind: DependencySource::Manifest,
                        confidence: Some(ApplicabilityConfidence::Medium),
                        relation: Some(DependencyRelation::Direct),
                    });
                }
                artifact_id.clear();
                group_id.clear();
                version.clear();
            }
        }
    }

    findings
}

/// Parse gradle.lockfile (properties-like format)
pub(crate) fn parse_gradle_lockfile(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();
        // Format: "group:artifact:version=configuration" or "group:artifact:version -> configuration"
        if let Some(equals_pos) = trimmed.find('=') {
            let dep_part = &trimmed[..equals_pos];
            let parts: Vec<&str> = dep_part.split(':').collect();
            if parts.len() >= 3 {
                let group = parts[0];
                let artifact = parts[1];
                let version = parts[2];
                if !version.is_empty() && !version.starts_with('{') {
                    findings.push(DependencyFinding {
                        ecosystem: PackageEcosystem::Maven,
                        package: format!("{group}:{artifact}"),
                        version: Some(version.to_string()),
                        source_file: Some(path.to_string()),
                        source_line: Some(line_num),
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

/// Parse build.gradle (best-effort: extract implementation/api dependency blocks)
pub(crate) fn parse_build_gradle(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();

        // Match: implementation 'group:artifact:version'
        // Match: implementation "group:artifact:version"
        for prefix in &[
            "implementation",
            "api",
            "compile",
            "runtimeOnly",
            "testImplementation",
        ] {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                let rest = rest.trim();
                // Expect: 'group:artifact:version' or "group:artifact:version"
                let dep_str = if (rest.starts_with('\'') && rest.ends_with('\''))
                    || (rest.starts_with('"') && rest.ends_with('"'))
                {
                    &rest[1..rest.len() - 1]
                } else {
                    continue;
                };

                let parts: Vec<&str> = dep_str.split(':').collect();
                if parts.len() >= 3 {
                    let group = parts[0];
                    let artifact = parts[1];
                    let version = parts[2];
                    // Skip variable references like ${versions.spring}
                    if !version.starts_with('$') && !version.starts_with('{') {
                        findings.push(DependencyFinding {
                            ecosystem: PackageEcosystem::Maven,
                            package: format!("{group}:{artifact}"),
                            version: Some(version.to_string()),
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
    }

    findings
}
