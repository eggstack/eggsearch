use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyReferenceKind, DependencyRelation,
    DependencySource,
};

pub(crate) fn parse_cargo_lock(content: &str, path: &str) -> Vec<DependencyFinding> {
    let value: toml::Value = match content.parse() {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    let packages = value
        .get("package")
        .and_then(|packages| packages.as_array());
    let Some(packages) = packages else {
        return Vec::new();
    };
    let mut findings = Vec::new();
    for entry in packages {
        let Some(table) = entry.as_table() else {
            continue;
        };
        let name = table
            .get("name")
            .and_then(|name| name.as_str())
            .unwrap_or("");
        if name.is_empty() {
            continue;
        }
        let version = table
            .get("version")
            .and_then(|version| version.as_str())
            .unwrap_or("");
        let version = if version.is_empty() {
            None
        } else {
            Some(version.to_string())
        };
        let source = table
            .get("source")
            .and_then(|source| source.as_str())
            .unwrap_or("");
        let provenance = if source.is_empty() {
            None
        } else {
            Some(source.to_string())
        };
        findings.push(DependencyFinding {
            ecosystem: PackageEcosystem::CratesIo,
            package: name.to_string(),
            version: version.clone(),
            resolved_version: version,
            version_requirement: None,
            reference_kind: None,
            reference_value: None,
            provenance,
            target_context: None,
            integrity_hash: None,
            source_file: Some(path.to_string()),
            source_line: None,
            source_kind: DependencySource::LockFile,
            confidence: Some(ApplicabilityConfidence::High),
            relation: Some(DependencyRelation::Transitive),
        });
    }
    findings
}

pub(crate) fn parse_cargo_toml(content: &str, path: &str) -> Vec<DependencyFinding> {
    let value: toml::Value = match content.parse() {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    let mut findings = Vec::new();
    collect_dependency_table(&value, &["dependencies"], None, path, &mut findings);
    collect_dependency_table(&value, &["dev-dependencies"], None, path, &mut findings);
    collect_dependency_table(&value, &["build-dependencies"], None, path, &mut findings);
    if let Some(target) = value.get("target").and_then(|target| target.as_table()) {
        let mut target_names: Vec<&String> = target.keys().collect();
        target_names.sort();
        for target_name in target_names {
            let Some(target_table) = target.get(target_name) else {
                continue;
            };
            let target_context = Some(target_name.clone());
            collect_dependency_table(
                target_table,
                &["dependencies"],
                target_context.clone(),
                path,
                &mut findings,
            );
            collect_dependency_table(
                target_table,
                &["dev-dependencies"],
                target_context.clone(),
                path,
                &mut findings,
            );
            collect_dependency_table(
                target_table,
                &["build-dependencies"],
                target_context,
                path,
                &mut findings,
            );
        }
    }
    findings
}

fn collect_dependency_table(
    value: &toml::Value,
    keys: &[&str],
    target_context: Option<String>,
    path: &str,
    findings: &mut Vec<DependencyFinding>,
) {
    let mut current = value;
    for key in keys {
        let Some(next) = current.get(*key) else {
            return;
        };
        current = next;
    }
    let Some(table) = current.as_table() else {
        return;
    };
    let mut names: Vec<&String> = table.keys().collect();
    names.sort();
    for alias in names {
        let Some(spec) = table.get(alias) else {
            continue;
        };
        if let Some(finding) = cargo_requirement(alias, spec, target_context.clone(), path) {
            findings.push(finding);
        }
    }
}

fn cargo_requirement(
    alias: &str,
    spec: &toml::Value,
    target_context: Option<String>,
    path: &str,
) -> Option<DependencyFinding> {
    if alias.is_empty() || alias.starts_with('#') {
        return None;
    }
    if let Some(requirement) = spec.as_str() {
        return Some(DependencyFinding {
            ecosystem: PackageEcosystem::CratesIo,
            package: alias.to_string(),
            version: Some(requirement.to_string()),
            resolved_version: None,
            version_requirement: Some(requirement.to_string()),
            reference_kind: None,
            reference_value: None,
            provenance: None,
            target_context,
            integrity_hash: None,
            source_file: Some(path.to_string()),
            source_line: None,
            source_kind: DependencySource::Manifest,
            confidence: Some(ApplicabilityConfidence::Medium),
            relation: Some(DependencyRelation::Direct),
        });
    }
    let table = spec.as_table()?;
    let package = table
        .get("package")
        .and_then(|package| package.as_str())
        .unwrap_or(alias);
    if package.is_empty() {
        return None;
    }
    let requirement = table
        .get("version")
        .and_then(|version| version.as_str())
        .map(|version| version.to_string());
    let workspace = table
        .get("workspace")
        .and_then(|workspace| workspace.as_bool())
        .unwrap_or(false);
    let mut provenance_parts: Vec<String> = Vec::new();
    if package != alias {
        provenance_parts.push(format!("alias: {alias}"));
    }
    if workspace {
        provenance_parts.push("workspace inheritance".to_string());
    }
    for key in ["registry", "path"] {
        if let Some(value) = table.get(key).and_then(|value| value.as_str()) {
            if !value.is_empty() {
                provenance_parts.push(format!("{key}: {value}"));
            }
        }
    }
    let mut reference_kind: Option<DependencyReferenceKind> = None;
    let mut reference_value: Option<String> = None;
    if let Some(url) = table.get("git").and_then(|url| url.as_str()) {
        if !url.is_empty() {
            reference_kind = Some(DependencyReferenceKind::Git);
            let mut reference = url.to_string();
            for key in ["rev", "tag", "branch"] {
                if let Some(value) = table.get(key).and_then(|value| value.as_str()) {
                    if !value.is_empty() {
                        reference.push_str(&format!(" ({key}: {value})"));
                    }
                }
            }
            reference_value = Some(reference);
            provenance_parts.push(format!("git: {url}"));
        }
    } else if let Some(path_value) = table.get("path").and_then(|value| value.as_str()) {
        if !path_value.is_empty() {
            reference_kind = Some(DependencyReferenceKind::Path);
            reference_value = Some(path_value.to_string());
        }
    }
    let provenance = if provenance_parts.is_empty() {
        None
    } else {
        Some(provenance_parts.join("; "))
    };
    let confidence = if requirement.is_some() || reference_value.is_some() {
        ApplicabilityConfidence::Medium
    } else {
        ApplicabilityConfidence::Low
    };
    Some(DependencyFinding {
        ecosystem: PackageEcosystem::CratesIo,
        package: package.to_string(),
        version: requirement.clone(),
        resolved_version: None,
        version_requirement: requirement,
        reference_kind,
        reference_value,
        provenance,
        target_context,
        integrity_hash: None,
        source_file: Some(path.to_string()),
        source_line: None,
        source_kind: DependencySource::Manifest,
        confidence: Some(confidence),
        relation: Some(DependencyRelation::Direct),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::security_applicability::DependencyReferenceKind;

    #[test]
    fn renamed_dependency_keeps_registry_identity() {
        let content = "[dependencies]\nfoo = { package = \"real-crate\", version = \"1.2.3\" }\n";
        let findings = parse_cargo_toml(content, "Cargo.toml");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].package, "real-crate");
        assert_eq!(findings[0].exact_version(), None);
        assert_eq!(findings[0].version_requirement.as_deref(), Some("1.2.3"));
        assert!(findings[0]
            .provenance
            .as_deref()
            .unwrap_or("")
            .contains("alias: foo"));
    }

    #[test]
    fn target_dependency_carries_target_context() {
        let content = "[target.'cfg(windows)'.dependencies]\nwinapi = \"0.3\"\n";
        let findings = parse_cargo_toml(content, "Cargo.toml");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].package, "winapi");
        assert_eq!(findings[0].target_context.as_deref(), Some("cfg(windows)"));
        assert_eq!(findings[0].exact_version(), None);
    }

    #[test]
    fn workspace_inherited_dependency_is_not_resolved() {
        let content = "[dependencies]\nserde = { workspace = true }\n";
        let findings = parse_cargo_toml(content, "Cargo.toml");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].exact_version(), None);
        assert_eq!(findings[0].version_requirement, None);
        assert!(findings[0]
            .provenance
            .as_deref()
            .unwrap_or("")
            .contains("workspace"));
    }

    #[test]
    fn git_and_path_dependencies_keep_provenance() {
        let content = "[dependencies]\nfoo = { git = \"https://example.com/foo\", rev = \"abc123\" }\nbar = { path = \"../bar\" }\n";
        let findings = parse_cargo_toml(content, "Cargo.toml");
        assert_eq!(findings.len(), 2);
        let foo = findings.iter().find(|f| f.package == "foo").unwrap();
        assert_eq!(foo.reference_kind, Some(DependencyReferenceKind::Git));
        assert!(foo
            .reference_value
            .as_deref()
            .unwrap_or("")
            .contains("abc123"));
        let bar = findings.iter().find(|f| f.package == "bar").unwrap();
        assert_eq!(bar.reference_kind, Some(DependencyReferenceKind::Path));
        assert_eq!(bar.exact_version(), None);
    }

    #[test]
    fn lock_preserves_non_registry_source() {
        let content = "[[package]]\nname = \"foo\"\nversion = \"0.1.0\"\nsource = \"git+https://example.com/foo#abc123\"\n\n[[package]]\nname = \"serde\"\nversion = \"1.0.193\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n";
        let findings = parse_cargo_lock(content, "Cargo.lock");
        assert_eq!(findings.len(), 2);
        let foo = findings.iter().find(|f| f.package == "foo").unwrap();
        assert_eq!(foo.exact_version(), Some("0.1.0"));
        assert!(foo
            .provenance
            .as_deref()
            .unwrap_or("")
            .contains("git+https"));
        let serde = findings.iter().find(|f| f.package == "serde").unwrap();
        assert_eq!(serde.exact_version(), Some("1.0.193"));
    }

    #[test]
    fn malformed_toml_yields_no_findings_without_panic() {
        assert!(parse_cargo_toml("not valid toml {{{", "Cargo.toml").is_empty());
        assert!(parse_cargo_lock("not valid toml {{{", "Cargo.lock").is_empty());
    }
}
