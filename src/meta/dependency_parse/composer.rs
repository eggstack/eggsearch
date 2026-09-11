use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyRelation, DependencySource,
};

/// Parse composer.lock
pub(crate) fn parse_composer_lock(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();

    let parsed: serde_json::Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(_) => return findings,
    };

    // composer.lock has "packages" and "packages-dev" arrays
    for key in &["packages", "packages-dev"] {
        if let Some(packages) = parsed.get(*key).and_then(|p| p.as_array()) {
            for pkg in packages {
                let name = pkg.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let version = pkg.get("version").and_then(|v| v.as_str()).unwrap_or("");
                if !name.is_empty() {
                    findings.push(DependencyFinding {
                        ecosystem: PackageEcosystem::Packagist,
                        package: name.to_string(),
                        version: Some(version.trim_start_matches('v').to_string()),
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
