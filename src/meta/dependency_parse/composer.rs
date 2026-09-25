use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyReferenceKind, DependencyRelation,
    DependencySource,
};

pub(crate) fn parse_composer_lock(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();

    let parsed: serde_json::Value = match serde_json::from_str(content) {
        Ok(parsed) => parsed,
        Err(_) => return findings,
    };

    for key in ["packages", "packages-dev"] {
        let Some(packages) = parsed.get(key).and_then(|packages| packages.as_array()) else {
            continue;
        };
        for package in packages {
            if let Some(finding) = composer_package(package, key, path) {
                findings.push(finding);
            }
        }
    }

    findings
}

fn composer_package(
    package: &serde_json::Value,
    section: &str,
    path: &str,
) -> Option<DependencyFinding> {
    let name = package.get("name").and_then(|name| name.as_str())?;
    if name.is_empty() {
        return None;
    }
    let raw_version = package
        .get("version")
        .and_then(|version| version.as_str())
        .unwrap_or("");
    let version = raw_version.trim_start_matches('v').to_string();
    let version = if version.is_empty() {
        None
    } else {
        Some(version)
    };
    let mut provenance_parts: Vec<String> = Vec::new();
    let mut reference_kind: Option<DependencyReferenceKind> = None;
    let mut reference_value: Option<String> = None;
    if is_dev_version(raw_version) {
        provenance_parts.push(format!("dev version: {raw_version}"));
    }
    if let Some(source) = package.get("source") {
        let source_type = source
            .get("type")
            .and_then(|source_type| source_type.as_str())
            .unwrap_or("");
        let url = source.get("url").and_then(|url| url.as_str()).unwrap_or("");
        let reference = source
            .get("reference")
            .and_then(|reference| reference.as_str())
            .unwrap_or("");
        if !source_type.is_empty() || !url.is_empty() {
            let mut part = String::from("source");
            if !source_type.is_empty() {
                part.push_str(&format!(" {source_type}"));
            }
            if !url.is_empty() {
                part.push_str(&format!(": {url}"));
            }
            provenance_parts.push(part);
        }
        if !reference.is_empty() {
            reference_kind = match source_type {
                "git" => Some(DependencyReferenceKind::Git),
                "path" | "filesystem" => Some(DependencyReferenceKind::Path),
                "url" | "file" | "zip" | "tar" => Some(DependencyReferenceKind::Url),
                _ => Some(DependencyReferenceKind::Commit),
            };
            reference_value = Some(reference.to_string());
        } else if !url.is_empty() {
            reference_kind = match source_type {
                "git" => Some(DependencyReferenceKind::Git),
                "path" | "filesystem" => Some(DependencyReferenceKind::Path),
                _ => Some(DependencyReferenceKind::Url),
            };
            reference_value = Some(url.to_string());
        }
    }
    if let Some(dist) = package.get("dist") {
        let dist_type = dist
            .get("type")
            .and_then(|dist_type| dist_type.as_str())
            .unwrap_or("");
        let url = dist.get("url").and_then(|url| url.as_str()).unwrap_or("");
        let reference = dist
            .get("reference")
            .and_then(|reference| reference.as_str())
            .unwrap_or("");
        if !dist_type.is_empty() || !url.is_empty() || !reference.is_empty() {
            let mut part = String::from("dist");
            if !dist_type.is_empty() {
                part.push_str(&format!(" {dist_type}"));
            }
            if !url.is_empty() {
                part.push_str(&format!(": {url}"));
            }
            if !reference.is_empty() {
                part.push_str(&format!(" (reference: {reference})"));
            }
            provenance_parts.push(part);
        }
        if reference_value.is_none() && !reference.is_empty() {
            reference_kind = Some(DependencyReferenceKind::Commit);
            reference_value = Some(reference.to_string());
        }
    }
    if let Some(shasum) = package
        .get("dist")
        .and_then(|dist| dist.get("shasum"))
        .and_then(|shasum| shasum.as_str())
    {
        if !shasum.is_empty() {
            provenance_parts.push(format!("dist shasum: {shasum}"));
        }
    }
    let provenance = if provenance_parts.is_empty() {
        None
    } else {
        Some(provenance_parts.join("; "))
    };
    let confidence = if version.is_some() {
        ApplicabilityConfidence::High
    } else {
        ApplicabilityConfidence::Low
    };
    Some(DependencyFinding {
        ecosystem: PackageEcosystem::Packagist,
        package: name.to_string(),
        version: version.clone(),
        resolved_version: version,
        version_requirement: None,
        reference_kind,
        reference_value,
        provenance,
        target_context: if section == "packages-dev" {
            Some("packages-dev".to_string())
        } else {
            None
        },
        integrity_hash: None,
        source_file: Some(path.to_string()),
        source_line: None,
        source_kind: DependencySource::LockFile,
        confidence: Some(confidence),
        relation: Some(DependencyRelation::Transitive),
    })
}

fn is_dev_version(version: &str) -> bool {
    version.starts_with("dev-")
        || version.ends_with("-dev")
        || version.contains("x-dev")
        || version.contains(".x-dev")
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCK: &str = r#"{
  "packages": [
    {"name": "laravel/framework", "version": "v10.48.4"},
    {"name": "acme/dev-pkg", "version": "dev-main",
     "source": {"type": "git", "url": "https://example.com/dev-pkg.git", "reference": "abc123def456"},
     "dist": {"type": "zip", "url": "https://example.com/dev-pkg.zip", "reference": "abc123def456", "shasum": "s1"}},
    {"name": "acme/custom", "version": "1.2.0",
     "source": {"type": "git", "url": "https://git.example.org/custom.git", "reference": "r2"}},
    {"name": "acme/empty", "version": ""}
  ],
  "packages-dev": [
    {"name": "phpunit/phpunit", "version": "10.5.0"}
  ]
}"#;

    #[test]
    fn lock_versions_sources_and_dev_context() {
        let findings = parse_composer_lock(LOCK, "composer.lock");
        assert_eq!(findings.len(), 5);
        let stable = findings
            .iter()
            .find(|f| f.package == "laravel/framework")
            .unwrap();
        assert_eq!(stable.exact_version(), Some("10.48.4"));
        assert_eq!(stable.target_context, None);
        let dev = findings
            .iter()
            .find(|f| f.package == "acme/dev-pkg")
            .unwrap();
        assert_eq!(dev.exact_version(), Some("dev-main"));
        assert!(dev
            .provenance
            .as_deref()
            .unwrap_or("")
            .contains("dev version"));
        assert_eq!(dev.reference_value.as_deref(), Some("abc123def456"));
        let custom = findings
            .iter()
            .find(|f| f.package == "acme/custom")
            .unwrap();
        assert!(custom
            .provenance
            .as_deref()
            .unwrap_or("")
            .contains("git.example.org"));
        let empty = findings.iter().find(|f| f.package == "acme/empty").unwrap();
        assert_eq!(empty.version, None);
        assert_eq!(empty.exact_version(), None);
        let dev_dep = findings
            .iter()
            .find(|f| f.package == "phpunit/phpunit")
            .unwrap();
        assert_eq!(dev_dep.target_context.as_deref(), Some("packages-dev"));
    }
}
