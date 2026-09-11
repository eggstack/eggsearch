use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyRelation, DependencySource,
};

/// Parse package-lock.json (npm)
pub(crate) fn parse_package_lock(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();

    let parsed: serde_json::Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(_) => return findings,
    };

    // npm lockfile v2+: "packages" key with "" as root
    if let Some(packages) = parsed.get("packages").and_then(|p| p.as_object()) {
        for (key, val) in packages {
            if let Some(name) = val.get("version").and_then(|v| v.as_str()) {
                let pkg_name = if key.is_empty() {
                    parsed
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("root")
                } else {
                    // packages key is "node_modules/pkg" or "node_modules/@scope/pkg"
                    key.rsplit_once("node_modules/")
                        .map(|(_, n)| n)
                        .unwrap_or(key)
                };
                findings.push(DependencyFinding {
                    ecosystem: PackageEcosystem::Npm,
                    package: pkg_name.to_string(),
                    version: Some(name.to_string()),
                    source_file: Some(path.to_string()),
                    source_line: None,
                    source_kind: DependencySource::LockFile,
                    confidence: Some(ApplicabilityConfidence::High),
                    relation: Some(DependencyRelation::Transitive),
                });
            }
        }
    }
    // npm lockfile v1: "dependencies" key
    else if let Some(deps) = parsed.get("dependencies").and_then(|d| d.as_object()) {
        for (name, val) in deps {
            if let Some(version) = val.get("version").and_then(|v| v.as_str()) {
                findings.push(DependencyFinding {
                    ecosystem: PackageEcosystem::Npm,
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

    findings
}

/// Parse yarn.lock (YAML-like format with indented version)
pub(crate) fn parse_yarn_lock(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut current_name: Option<String> = None;
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();

        // yarn.lock entries start with quoted or unquoted package names at the top level
        let is_entry_header = line.starts_with('"')
            || (!line.starts_with(' ') && !line.starts_with('\t') && trimmed.ends_with(':'));

        if is_entry_header {
            // New entry — flush previous
            current_name.take();
            // Extract the first package name from the entry header
            if let Some(name_part) = trimmed.strip_suffix(':') {
                let clean = name_part.trim_matches('"').trim();
                // Handle scoped packages: @scope/name@version -> @scope/name
                // Handle plain packages: name@version -> name
                let pkg_name = if let Some(at_pos) = clean.rfind('@') {
                    let candidate = &clean[..at_pos];
                    if candidate.is_empty() {
                        // Scoped package starting with @: @scope/name
                        clean.to_string()
                    } else {
                        candidate.to_string()
                    }
                } else {
                    clean.to_string()
                };
                if !pkg_name.is_empty() && !pkg_name.contains(' ') {
                    current_name = Some(pkg_name);
                }
            }
        } else if trimmed.starts_with("version ") {
            if let Some(name) = current_name.take() {
                let version = trimmed
                    .trim_start_matches("version")
                    .trim()
                    .trim_matches('"')
                    .to_string();
                findings.push(DependencyFinding {
                    ecosystem: PackageEcosystem::Npm,
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
        }
    }

    findings
}

/// Parse pnpm-lock.yaml (YAML format with packages map)
pub(crate) fn parse_pnpm_lock(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut in_packages = false;
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();

        if trimmed == "packages:" {
            in_packages = true;
            continue;
        }

        if in_packages {
            // Top-level keys (not indented) exit the packages section
            if !line.starts_with(' ') && !line.starts_with('\t') && !trimmed.is_empty() {
                in_packages = false;
                continue;
            }

            // Package entries look like: /lodash@4.17.21:
            if let Some(entry) = trimmed.strip_suffix(':') {
                if entry.starts_with('/') || entry.contains('@') {
                    // Parse /name@version or /@scope/name@version
                    let pkg_part = entry.trim_start_matches('/');
                    if let Some((name, version)) = pkg_part.rsplit_once('@') {
                        if !name.is_empty() && !version.is_empty() {
                            findings.push(DependencyFinding {
                                ecosystem: PackageEcosystem::Npm,
                                package: name.to_string(),
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
        }
    }

    findings
}
