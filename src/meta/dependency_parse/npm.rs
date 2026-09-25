use std::collections::{HashMap, HashSet};

use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyParseReport, DependencyReferenceKind,
    DependencyRelation, DependencySource,
};

pub(crate) fn parse_package_lock(content: &str, path: &str) -> DependencyParseReport {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(content) else {
        return DependencyParseReport::malformed("package-lock.json is not valid JSON");
    };
    let version = json.get("lockfileVersion");
    match version {
        Some(version) if version.as_u64() == Some(1) => {
            report_v1_dependencies(&json, path, Vec::new())
        }
        Some(version) if matches!(version.as_u64(), Some(2) | Some(3)) => {
            report_packages(&json, path)
        }
        Some(version) => {
            DependencyParseReport::unsupported(format!("unsupported npm lockfileVersion {version}"))
        }
        None => {
            if json.get("packages").and_then(|p| p.as_object()).is_some() {
                report_packages(&json, path)
            } else if json
                .get("dependencies")
                .and_then(|d| d.as_object())
                .is_some()
            {
                report_v1_dependencies(&json, path, Vec::new())
            } else {
                DependencyParseReport::unsupported("unrecognized npm lock shape")
            }
        }
    }
}

fn report_packages(json: &serde_json::Value, path: &str) -> DependencyParseReport {
    let Some(packages) = json.get("packages").and_then(|p| p.as_object()) else {
        return DependencyParseReport::partial(
            Vec::new(),
            vec![crate::core::security_applicability::ParseDiagnostic {
                code: "dependency_parse_partial".to_string(),
                message: "npm lockfileVersion 2/3 without a packages map".to_string(),
                line: None,
            }],
        );
    };
    let direct = root_direct_names(packages.get(""));
    let mut findings = Vec::new();
    for (key, entry) in packages {
        if key.is_empty() {
            continue;
        }
        if let Some(finding) = packages_entry(key, entry, &direct, path) {
            findings.push(finding);
        }
    }
    DependencyParseReport::complete(findings)
}

fn root_direct_names(root: Option<&serde_json::Value>) -> HashSet<String> {
    let mut names = HashSet::new();
    let Some(root) = root else {
        return names;
    };
    for section in ["dependencies", "devDependencies", "optionalDependencies"] {
        if let Some(deps) = root.get(section).and_then(|d| d.as_object()) {
            names.extend(deps.keys().cloned());
        }
    }
    names
}

fn packages_entry(
    key: &str,
    entry: &serde_json::Value,
    direct: &HashSet<String>,
    path: &str,
) -> Option<DependencyFinding> {
    let name = key
        .rsplit_once("node_modules/")
        .map(|(_, name)| name)
        .unwrap_or(key);
    if name.is_empty() || name.contains(char::is_whitespace) {
        return None;
    }
    let version = entry
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let linked = entry.get("link").and_then(|l| l.as_bool()).unwrap_or(false);
    let resolved_url = entry.get("resolved").and_then(|r| r.as_str()).unwrap_or("");
    let mut context: Vec<String> = Vec::new();
    for flag in ["dev", "optional", "peer"] {
        if entry.get(flag).and_then(|f| f.as_bool()).unwrap_or(false) {
            context.push(flag.to_string());
        }
    }
    let target_context = if context.is_empty() {
        None
    } else {
        Some(context.join(", "))
    };
    let relation = if direct.contains(name) {
        DependencyRelation::Direct
    } else {
        DependencyRelation::Transitive
    };
    if linked || !key.contains("node_modules/") {
        let target = if resolved_url.is_empty() {
            None
        } else {
            Some(resolved_url.to_string())
        };
        return Some(DependencyFinding {
            ecosystem: PackageEcosystem::Npm,
            package: name.to_string(),
            version: if version.is_empty() {
                None
            } else {
                Some(version.clone())
            },
            resolved_version: None,
            version_requirement: if version.is_empty() {
                None
            } else {
                Some(version)
            },
            reference_kind: Some(DependencyReferenceKind::Workspace),
            reference_value: target,
            provenance: Some("workspace".to_string()),
            target_context,
            integrity_hash: None,
            source_file: Some(path.to_string()),
            source_line: None,
            source_kind: DependencySource::LockFile,
            confidence: Some(ApplicabilityConfidence::Medium),
            relation: Some(relation),
        });
    }
    if version.is_empty() {
        return None;
    }
    let (reference_kind, reference_value, provenance) = non_registry_provenance(resolved_url);
    Some(DependencyFinding {
        ecosystem: PackageEcosystem::Npm,
        package: name.to_string(),
        version: Some(version.clone()),
        resolved_version: Some(version),
        version_requirement: None,
        reference_kind,
        reference_value,
        provenance,
        target_context,
        integrity_hash: None,
        source_file: Some(path.to_string()),
        source_line: None,
        source_kind: DependencySource::LockFile,
        confidence: Some(ApplicabilityConfidence::High),
        relation: Some(relation),
    })
}

fn non_registry_provenance(
    url: &str,
) -> (
    Option<DependencyReferenceKind>,
    Option<String>,
    Option<String>,
) {
    use DependencyReferenceKind as Kind;
    if url.is_empty() {
        return (None, None, None);
    }
    if url.starts_with("git+") || url.starts_with("git:") || url.starts_with("github:") {
        return (
            Some(Kind::Git),
            Some(url.to_string()),
            Some(format!("git: {url}")),
        );
    }
    if url.starts_with("file:") {
        return (
            Some(Kind::Path),
            Some(url.to_string()),
            Some(format!("file: {url}")),
        );
    }
    if url.starts_with("http:") && !url.contains("registry") {
        return (
            Some(Kind::Url),
            Some(url.to_string()),
            Some(format!("tarball: {url}")),
        );
    }
    (None, None, None)
}

fn report_v1_dependencies(
    json: &serde_json::Value,
    path: &str,
    diagnostics: Vec<crate::core::security_applicability::ParseDiagnostic>,
) -> DependencyParseReport {
    let Some(top) = json.get("dependencies").and_then(|d| d.as_object()) else {
        return DependencyParseReport::partial(
            Vec::new(),
            vec![crate::core::security_applicability::ParseDiagnostic {
                code: "dependency_parse_partial".to_string(),
                message: "npm lockfileVersion 1 without a dependencies map".to_string(),
                line: None,
            }],
        );
    };
    let mut findings = Vec::new();
    let mut stack: Vec<(&serde_json::Map<String, serde_json::Value>, usize)> = vec![(top, 0)];
    let mut seen: HashMap<String, usize> = HashMap::new();
    while let Some((deps, depth)) = stack.pop() {
        let mut names: Vec<&String> = deps.keys().collect();
        names.sort();
        for name in names {
            let Some(entry) = deps.get(name) else {
                continue;
            };
            let version = entry.get("version").and_then(|v| v.as_str()).unwrap_or("");
            if !version.is_empty() {
                let key = format!("{name}@{version}");
                if seen.insert(key, findings.len()).is_none() {
                    let dev = entry.get("dev").and_then(|d| d.as_bool()).unwrap_or(false);
                    let resolved_url = entry.get("resolved").and_then(|r| r.as_str()).unwrap_or("");
                    let (reference_kind, reference_value, provenance) =
                        non_registry_provenance(resolved_url);
                    findings.push(DependencyFinding {
                        ecosystem: PackageEcosystem::Npm,
                        package: name.clone(),
                        version: Some(version.to_string()),
                        resolved_version: Some(version.to_string()),
                        version_requirement: None,
                        reference_kind,
                        reference_value,
                        provenance,
                        target_context: if dev { Some("dev".to_string()) } else { None },
                        integrity_hash: None,
                        source_file: Some(path.to_string()),
                        source_line: None,
                        source_kind: DependencySource::LockFile,
                        confidence: Some(ApplicabilityConfidence::High),
                        relation: Some(DependencyRelation::Transitive),
                    });
                }
            }
            if depth < 64 {
                if let Some(nested) = entry.get("dependencies").and_then(|d| d.as_object()) {
                    stack.push((nested, depth + 1));
                }
            }
        }
    }
    if diagnostics.is_empty() {
        DependencyParseReport::complete(findings)
    } else {
        DependencyParseReport::partial(findings, diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::security_applicability::ParseStatus;

    #[test]
    fn v1_nested_dependencies_walk_recursively() {
        let content = r#"{"lockfileVersion": 1, "dependencies": {"a": {"version": "1.0.0", "dependencies": {"b": {"version": "2.0.0"}}}}}"#;
        let report = parse_package_lock(content, "package-lock.json");
        assert_eq!(report.status, ParseStatus::Complete);
        assert_eq!(report.findings.len(), 2);
        assert!(report.findings.iter().all(|f| f.has_resolved_evidence()));
    }

    #[test]
    fn v2_root_entry_excluded_and_direct_marked() {
        let content = r#"{"name": "app", "lockfileVersion": 3, "packages": {"": {"name": "app", "version": "1.0.0", "dependencies": {"a": "^1.0.0"}}, "node_modules/a": {"version": "1.2.3"}, "node_modules/@s/b": {"version": "2.0.0", "dev": true}}}"#;
        let report = parse_package_lock(content, "package-lock.json");
        assert_eq!(report.status, ParseStatus::Complete);
        assert_eq!(report.findings.len(), 2);
        assert!(report.findings.iter().all(|f| f.package != "app"));
        let direct = report.findings.iter().find(|f| f.package == "a").unwrap();
        assert_eq!(direct.relation, Some(DependencyRelation::Direct));
        let scoped = report
            .findings
            .iter()
            .find(|f| f.package == "@s/b")
            .unwrap();
        assert_eq!(scoped.target_context.as_deref(), Some("dev"));
    }

    #[test]
    fn workspace_and_git_sources_keep_provenance() {
        let content = r#"{"lockfileVersion": 3, "packages": {"": {}, "node_modules/w": {"version": "1.0.0", "link": true, "resolved": "packages/w"}, "node_modules/g": {"version": "1.0.0", "resolved": "git+https://example.com/g.git#abc"}}}"#;
        let report = parse_package_lock(content, "package-lock.json");
        let workspace = report.findings.iter().find(|f| f.package == "w").unwrap();
        assert_eq!(workspace.exact_version(), None);
        assert_eq!(workspace.provenance.as_deref(), Some("workspace"));
        let git = report.findings.iter().find(|f| f.package == "g").unwrap();
        assert_eq!(git.exact_version(), Some("1.0.0"));
        assert!(git
            .provenance
            .as_deref()
            .unwrap_or("")
            .contains("example.com"));
    }

    #[test]
    fn unsupported_future_lock_version_is_explicit() {
        let report = parse_package_lock(
            r#"{"lockfileVersion": 99, "packages": {}}"#,
            "package-lock.json",
        );
        assert_eq!(report.status, ParseStatus::Unsupported);
        let malformed = parse_package_lock("{invalid", "package-lock.json");
        assert_eq!(malformed.status, ParseStatus::Malformed);
    }
}
