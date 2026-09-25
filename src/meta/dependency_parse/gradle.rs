use std::collections::HashSet;

use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyRelation, DependencySource,
};

pub(crate) fn parse_gradle_lockfile(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut seen = HashSet::new();
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some(equals_pos) = trimmed.find('=') else {
            continue;
        };
        let dep_part = &trimmed[..equals_pos];
        let parts: Vec<&str> = dep_part.split(':').collect();
        if parts.len() < 3 {
            continue;
        }
        let (group, artifact, version) = (parts[0].trim(), parts[1].trim(), parts[2].trim());
        if group.is_empty() || artifact.is_empty() || version.is_empty() || version.starts_with('{')
        {
            continue;
        }
        if seen.insert(format!("{group}:{artifact}:{version}")) {
            findings.push(DependencyFinding {
                ecosystem: PackageEcosystem::Maven,
                package: format!("{group}:{artifact}"),
                version: Some(version.to_string()),
                resolved_version: Some(version.to_string()),
                version_requirement: None,
                reference_kind: None,
                reference_value: None,
                provenance: None,
                target_context: None,
                integrity_hash: None,
                source_file: Some(path.to_string()),
                source_line: Some(line_num),
                source_kind: DependencySource::LockFile,
                confidence: Some(ApplicabilityConfidence::High),
                relation: Some(DependencyRelation::Transitive),
            });
        }
    }

    findings
}

const GRADLE_CONFIGURATIONS: &[&str] = &[
    "implementation",
    "api",
    "compileOnly",
    "runtimeOnly",
    "testImplementation",
    "testCompileOnly",
    "testRuntimeOnly",
    "compile",
];

pub(crate) fn parse_build_gradle(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('#') {
            continue;
        }

        for prefix in GRADLE_CONFIGURATIONS {
            let Some(rest) = trimmed.strip_prefix(prefix) else {
                continue;
            };
            let rest = rest.trim();
            let Some(coordinate) = gradle_coordinate(rest) else {
                continue;
            };
            let parts: Vec<&str> = coordinate.split(':').collect();
            if parts.len() < 3 {
                continue;
            }
            let (group, artifact, version) = (parts[0].trim(), parts[1].trim(), parts[2].trim());
            if group.is_empty() || artifact.is_empty() {
                continue;
            }
            let dynamic = version.is_empty()
                || version.starts_with('$')
                || version.starts_with('{')
                || version == "+"
                || version.ends_with('+');
            let requirement = if version.is_empty() {
                None
            } else {
                Some(version.to_string())
            };
            findings.push(DependencyFinding {
                ecosystem: PackageEcosystem::Maven,
                package: format!("{group}:{artifact}"),
                version: requirement.clone(),
                resolved_version: None,
                version_requirement: requirement,
                reference_kind: None,
                reference_value: None,
                provenance: if dynamic {
                    Some("dynamic or property version: unresolved".to_string())
                } else {
                    None
                },
                target_context: Some(prefix.to_string()),
                integrity_hash: None,
                source_file: Some(path.to_string()),
                source_line: Some(line_num),
                source_kind: DependencySource::Manifest,
                confidence: Some(ApplicabilityConfidence::Medium),
                relation: Some(DependencyRelation::Direct),
            });
            break;
        }
    }

    findings
}

fn gradle_coordinate(rest: &str) -> Option<String> {
    let rest = rest.trim();
    let inner = if (rest.starts_with('\'') || rest.starts_with('"')) && rest.len() >= 2 {
        let quote = rest.as_bytes()[0];
        let body = &rest[1..];
        let end = body.find(quote as char)?;
        body[..end].to_string()
    } else if rest.starts_with('(') {
        let body = rest.strip_prefix('(')?.trim();
        let quote = body.as_bytes().first()?;
        if *quote != b'\'' && *quote != b'"' {
            return None;
        }
        let body = &body[1..];
        let end = body.find(*quote as char)?;
        body[..end].to_string()
    } else {
        return None;
    };
    let inner = inner.trim();
    for wrapper in ["platform(", "enforcedPlatform("] {
        if let Some(wrapped) = inner.strip_prefix(wrapper) {
            let wrapped = wrapped.trim_end_matches(')').trim();
            return Some(wrapped.to_string());
        }
    }
    Some(inner.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_entries_are_exact_and_deduplicated() {
        let content = "org.a:b:1.0=compileClasspath\norg.a:b:1.0=runtimeClasspath\nincomplete::{strictly}=x\n";
        let findings = parse_gradle_lockfile(content, "gradle.lockfile");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].exact_version(), Some("1.0"));
    }

    #[test]
    fn kotlin_paren_and_dynamic_versions() {
        let content = "dependencies {\n    implementation(\"org.a:b:1.0\")\n    api('org.c:d:${ver}')\n    testImplementation 'org.e:f:2.+ '\n}\n";
        let findings = parse_build_gradle(content, "build.gradle.kts");
        assert_eq!(findings.len(), 3);
        let literal = findings.iter().find(|f| f.package == "org.a:b").unwrap();
        assert_eq!(literal.version_requirement.as_deref(), Some("1.0"));
        assert_eq!(literal.exact_version(), None);
        for package in ["org.c:d", "org.e:f"] {
            let dynamic = findings.iter().find(|f| f.package == package).unwrap();
            assert_eq!(dynamic.exact_version(), None);
            assert!(dynamic
                .provenance
                .as_deref()
                .unwrap_or("")
                .contains("unresolved"));
        }
    }
}
