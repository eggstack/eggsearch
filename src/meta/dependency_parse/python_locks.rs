use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyReferenceKind, DependencyRelation,
    DependencySource,
};

pub(crate) fn parse_poetry_lock(content: &str, path: &str) -> Vec<DependencyFinding> {
    let value: toml::Value = match content.parse() {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    let Some(packages) = value
        .get("package")
        .and_then(|packages| packages.as_array())
    else {
        return Vec::new();
    };
    let mut findings = Vec::new();
    for entry in packages {
        if let Some(finding) = poetry_package(entry, path) {
            findings.push(finding);
        }
    }
    findings
}

fn poetry_package(entry: &toml::Value, path: &str) -> Option<DependencyFinding> {
    let table = entry.as_table()?;
    let name = table.get("name").and_then(|name| name.as_str())?;
    if name.is_empty() {
        return None;
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
    let mut provenance_parts: Vec<String> = Vec::new();
    if let Some(source) = table.get("source").and_then(|source| source.as_table()) {
        for key in ["type", "url", "reference"] {
            if let Some(value) = source.get(key).and_then(|value| value.as_str()) {
                if !value.is_empty() {
                    provenance_parts.push(format!("{key}: {value}"));
                }
            }
        }
    }
    let provenance = if provenance_parts.is_empty() {
        None
    } else {
        Some(provenance_parts.join("; "))
    };
    Some(DependencyFinding {
        ecosystem: PackageEcosystem::Pypi,
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
    })
}

pub(crate) fn parse_pipfile_lock(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let Ok(json) = serde_json::from_str::<serde_json::Value>(content) else {
        return findings;
    };
    for section in ["default", "develop"] {
        let Some(deps) = json.get(section).and_then(|deps| deps.as_object()) else {
            continue;
        };
        let mut names: Vec<&String> = deps.keys().collect();
        names.sort();
        for name in names {
            let Some(entry) = deps.get(name) else {
                continue;
            };
            let version = entry
                .get("version")
                .and_then(|version| version.as_str())
                .map(|version| version.trim_start_matches("==").to_string())
                .filter(|version| !version.is_empty());
            let mut provenance_parts: Vec<String> = Vec::new();
            let mut reference_kind: Option<DependencyReferenceKind> = None;
            let mut reference_value: Option<String> = None;
            if let Some(url) = entry.get("git").and_then(|url| url.as_str()) {
                if !url.is_empty() {
                    let reference = match entry.get("ref").and_then(|r| r.as_str()) {
                        Some(rev) if !rev.is_empty() => format!("{url} (ref: {rev})"),
                        _ => url.to_string(),
                    };
                    reference_kind = Some(DependencyReferenceKind::Git);
                    reference_value = Some(reference);
                    provenance_parts.push(format!("git: {url}"));
                }
            }
            for key in ["path", "file"] {
                if let Some(value) = entry.get(key).and_then(|value| value.as_str()) {
                    if !value.is_empty() {
                        reference_kind = Some(DependencyReferenceKind::Path);
                        reference_value = Some(value.to_string());
                        provenance_parts.push(format!("{key}: {value}"));
                    }
                }
            }
            if let Some(index) = entry.get("index").and_then(|index| index.as_str()) {
                if !index.is_empty() {
                    provenance_parts.push(format!("index: {index}"));
                }
            }
            let provenance = if provenance_parts.is_empty() {
                None
            } else {
                Some(provenance_parts.join("; "))
            };
            findings.push(DependencyFinding {
                ecosystem: PackageEcosystem::Pypi,
                package: name.clone(),
                version: version.clone(),
                resolved_version: version,
                version_requirement: None,
                reference_kind,
                reference_value,
                provenance,
                target_context: if section == "develop" {
                    Some("develop".to_string())
                } else {
                    None
                },
                integrity_hash: None,
                source_file: Some(path.to_string()),
                source_line: None,
                source_kind: DependencySource::LockFile,
                confidence: Some(ApplicabilityConfidence::High),
                relation: Some(DependencyRelation::Transitive),
            });
        }
    }
    findings
}

pub(crate) fn parse_uv_lock(content: &str, path: &str) -> Vec<DependencyFinding> {
    let value: toml::Value = match content.parse() {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    let Some(packages) = value
        .get("package")
        .and_then(|packages| packages.as_array())
    else {
        return Vec::new();
    };
    let mut findings = Vec::new();
    for entry in packages {
        if let Some(finding) = uv_package(entry, path) {
            findings.push(finding);
        }
    }
    findings
}

fn uv_package(entry: &toml::Value, path: &str) -> Option<DependencyFinding> {
    let table = entry.as_table()?;
    let name = table.get("name").and_then(|name| name.as_str())?;
    if name.is_empty() {
        return None;
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
    let mut provenance_parts: Vec<String> = Vec::new();
    if let Some(source) = table.get("source") {
        collect_toml_source(source, &mut provenance_parts);
    }
    let provenance = if provenance_parts.is_empty() {
        None
    } else {
        Some(provenance_parts.join("; "))
    };
    Some(DependencyFinding {
        ecosystem: PackageEcosystem::Pypi,
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
    })
}

fn collect_toml_source(source: &toml::Value, parts: &mut Vec<String>) {
    if let Some(table) = source.as_table() {
        let mut keys: Vec<&String> = table.keys().collect();
        keys.sort();
        for key in keys {
            let Some(value) = table.get(key) else {
                continue;
            };
            if let Some(text) = value.as_str() {
                if !text.is_empty() {
                    parts.push(format!("{key}: {text}"));
                }
            } else if value.as_bool().is_some() || value.as_integer().is_some() {
                parts.push(format!("{key}: {value}"));
            }
        }
    } else if let Some(text) = source.as_str() {
        if !text.is_empty() {
            parts.push(text.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poetry_git_source_keeps_provenance() {
        let content = "[[package]]\nname = \"my-pkg\"\nversion = \"1.0.0\"\n\n[package.source]\ntype = \"git\"\nurl = \"https://example.com/my-pkg.git\"\nreference = \"abc123\"\n";
        let findings = parse_poetry_lock(content, "poetry.lock");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].exact_version(), Some("1.0.0"));
        let provenance = findings[0].provenance.as_deref().unwrap_or("");
        assert!(provenance.contains("git"));
        assert!(provenance.contains("example.com"));
    }

    #[test]
    fn uv_non_registry_source_keeps_provenance() {
        let content = "[[package]]\nname = \"my-pkg\"\nversion = \"1.0.0\"\nsource = { git = \"https://example.com/my-pkg.git\", rev = \"abc123\" }\n";
        let findings = parse_uv_lock(content, "uv.lock");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].exact_version(), Some("1.0.0"));
        assert!(findings[0]
            .provenance
            .as_deref()
            .unwrap_or("")
            .contains("example.com"));
    }

    #[test]
    fn pipfile_git_path_and_develop_context() {
        let content = "{\"default\": {\"req\": {\"version\": \"==2.28.0\"}, \"g\": {\"git\": \"https://example.com/g.git\", \"ref\": \"v1.0\"}}, \"develop\": {\"p\": {\"path\": \"./local\"}}}";
        let findings = parse_pipfile_lock(content, "Pipfile.lock");
        assert_eq!(findings.len(), 3);
        let git = findings.iter().find(|f| f.package == "g").unwrap();
        assert_eq!(git.reference_kind, Some(DependencyReferenceKind::Git));
        assert_eq!(git.target_context, None);
        let dev = findings.iter().find(|f| f.package == "p").unwrap();
        assert_eq!(dev.reference_kind, Some(DependencyReferenceKind::Path));
        assert_eq!(dev.target_context.as_deref(), Some("develop"));
    }
}
