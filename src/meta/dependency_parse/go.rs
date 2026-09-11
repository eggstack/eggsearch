use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyRelation, DependencySource,
};

/// Parse go.mod for module requirements
pub(crate) fn parse_go_mod(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut in_require = false;
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();

        if trimmed.starts_with("require (") || trimmed == "require" {
            in_require = true;
            continue;
        }

        if trimmed == ")" && in_require {
            in_require = false;
            continue;
        }

        if in_require || trimmed.starts_with("require ") {
            let rest = if in_require {
                trimmed
            } else {
                trimmed.trim_start_matches("require ")
            };
            let rest = rest.trim();

            // Skip comments
            if rest.starts_with("//") {
                continue;
            }

            // Format: module version [+incompatible]
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() >= 2 {
                let module = parts[0];
                let version = parts[1].trim_start_matches('v');

                findings.push(DependencyFinding {
                    ecosystem: PackageEcosystem::Go,
                    package: module.to_string(),
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

    findings
}

/// Parse go.sum (text format with module/version lines)
pub(crate) fn parse_go_sum(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let module = parts[0];
            let version = parts[1];
            // go.sum has entries like: module version/go.mod hash
            let clean_version = version.split("/").next().unwrap_or(version);
            let key = (module.to_string(), clean_version.to_string());
            if seen.insert(key) {
                findings.push(DependencyFinding {
                    ecosystem: PackageEcosystem::Go,
                    package: module.to_string(),
                    version: Some(clean_version.to_string()),
                    source_file: Some(path.to_string()),
                    source_line: Some(line_num),
                    source_kind: DependencySource::LockFile,
                    confidence: Some(ApplicabilityConfidence::High),
                    relation: Some(DependencyRelation::Transitive),
                });
            }
        }
    }

    findings
}
