use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyRelation, DependencySource,
};

use super::extract_xml_attr;

/// Parse .csproj for PackageReference elements
pub(crate) fn parse_csproj(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();

        if trimmed.contains("PackageReference") {
            let include = extract_xml_attr(trimmed, "Include");
            let version = extract_xml_attr(trimmed, "Version");

            if let Some(name) = include {
                findings.push(DependencyFinding {
                    ecosystem: PackageEcosystem::Nuget,
                    package: name,
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

    findings
}

/// Parse NuGet packages.lock.json (JSON format)
pub(crate) fn parse_packages_lock_json(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();

    if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
        if let Some(deps) = json.get("libraries").and_then(|v| v.as_object()) {
            for (key, _val) in deps {
                // Key format: "PackageName/1.2.3"
                if let Some((name, version)) = key.rsplit_once('/') {
                    findings.push(DependencyFinding {
                        ecosystem: PackageEcosystem::Nuget,
                        package: name.to_string(),
                        version: Some(version.to_string()),
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
