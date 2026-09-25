use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyParseReport, DependencyRelation,
    DependencySource,
};

pub(crate) fn parse_packages_lock_json(content: &str, path: &str) -> DependencyParseReport {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(content) else {
        return DependencyParseReport::malformed("packages.lock.json is not valid JSON");
    };
    let Some(targets) = json.get("dependencies").and_then(|deps| deps.as_object()) else {
        return DependencyParseReport::unsupported(
            "unrecognized NuGet lock shape: missing versioned dependencies graph",
        );
    };
    let mut diagnostics = Vec::new();
    if !json
        .get("version")
        .is_some_and(|version| version.is_number())
    {
        diagnostics.push(crate::core::security_applicability::ParseDiagnostic {
            code: "dependency_format_unsupported".to_string(),
            message: "packages.lock.json has missing or non-numeric version; parsed best-effort"
                .to_string(),
            line: None,
        });
    }
    let mut findings = Vec::new();
    for (target, packages) in targets {
        let Some(packages) = packages.as_object() else {
            diagnostics.push(crate::core::security_applicability::ParseDiagnostic {
                code: "dependency_parse_partial".to_string(),
                message: format!("NuGet target '{target}' is not a package map; skipped"),
                line: None,
            });
            continue;
        };
        for (package, entry) in packages {
            if package.is_empty() {
                continue;
            }
            let entry_type = entry
                .get("type")
                .and_then(|entry_type| entry_type.as_str())
                .unwrap_or("");
            if entry_type.eq_ignore_ascii_case("project") {
                let requested = entry
                    .get("requested")
                    .and_then(|requested| requested.as_str())
                    .map(|requested| requested.to_string())
                    .filter(|requested| !requested.is_empty());
                findings.push(DependencyFinding {
                    ecosystem: PackageEcosystem::Nuget,
                    package: package.clone(),
                    version: requested.clone(),
                    resolved_version: None,
                    version_requirement: requested,
                    reference_kind: None,
                    reference_value: None,
                    provenance: Some("project reference".to_string()),
                    target_context: Some(target.clone()),
                    integrity_hash: None,
                    source_file: Some(path.to_string()),
                    source_line: None,
                    source_kind: DependencySource::LockFile,
                    confidence: Some(ApplicabilityConfidence::Medium),
                    relation: Some(DependencyRelation::Direct),
                });
                continue;
            }
            let resolved = entry
                .get("resolved")
                .and_then(|resolved| resolved.as_str())
                .map(|resolved| resolved.to_string())
                .filter(|resolved| !resolved.is_empty());
            let requested = entry
                .get("requested")
                .and_then(|requested| requested.as_str())
                .map(|requested| requested.to_string())
                .filter(|requested| !requested.is_empty());
            let integrity = entry
                .get("contentHash")
                .and_then(|hash| hash.as_str())
                .map(|hash| hash.to_string())
                .filter(|hash| !hash.is_empty());
            let relation = if entry_type.eq_ignore_ascii_case("direct") {
                DependencyRelation::Direct
            } else if entry_type.to_lowercase().contains("transitive") {
                DependencyRelation::Transitive
            } else {
                DependencyRelation::Unknown
            };
            let confidence = if resolved.is_some() {
                ApplicabilityConfidence::High
            } else {
                ApplicabilityConfidence::Medium
            };
            findings.push(DependencyFinding {
                ecosystem: PackageEcosystem::Nuget,
                package: package.clone(),
                version: resolved.clone(),
                resolved_version: resolved,
                version_requirement: requested,
                reference_kind: None,
                reference_value: None,
                provenance: None,
                target_context: Some(target.clone()),
                integrity_hash: integrity,
                source_file: Some(path.to_string()),
                source_line: None,
                source_kind: DependencySource::LockFile,
                confidence: Some(confidence),
                relation: Some(relation),
            });
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

    const TWO_TFMS: &str = r#"{
  "version": 1,
  "dependencies": {
    "net8.0": {
      "A": {"type": "Direct", "requested": "[1.0.0, )", "resolved": "1.0.0", "contentHash": "h1"},
      "B": {"type": "Transitive", "requested": "[2.0.0, )", "resolved": "2.1.0", "contentHash": "h2"}
    },
    "net8.0/win-x64": {
      "A": {"type": "Direct", "requested": "[1.0.0, )", "resolved": "1.0.0", "contentHash": "h1"},
      "C": {"type": "CentralTransitive", "requested": "[3.0.0, )", "resolved": "3.0.0", "contentHash": "h3"},
      "P": {"type": "Project", "requested": "[1.0.0, )", "resolved": "1.0.0"},
      "F": {"type": "FutureKind", "requested": "[4.0.0, )", "resolved": "4.0.0", "contentHash": "h4"}
    }
  }
}"#;

    #[test]
    fn target_graphs_relations_and_provenance() {
        let report = parse_packages_lock_json(TWO_TFMS, "packages.lock.json");
        assert_eq!(report.status, ParseStatus::Complete);
        assert_eq!(report.findings.len(), 6);
        let direct = report
            .findings
            .iter()
            .find(|f| f.package == "A" && f.target_context.as_deref() == Some("net8.0"))
            .unwrap();
        assert_eq!(direct.exact_version(), Some("1.0.0"));
        assert_eq!(direct.relation, Some(DependencyRelation::Direct));
        assert_eq!(direct.version_requirement.as_deref(), Some("[1.0.0, )"));
        assert_eq!(direct.integrity_hash.as_deref(), Some("h1"));
        let transitive = report.findings.iter().find(|f| f.package == "B").unwrap();
        assert_eq!(transitive.relation, Some(DependencyRelation::Transitive));
        assert_eq!(transitive.exact_version(), Some("2.1.0"));
        let central = report.findings.iter().find(|f| f.package == "C").unwrap();
        assert_eq!(central.relation, Some(DependencyRelation::Transitive));
        assert_eq!(central.target_context.as_deref(), Some("net8.0/win-x64"));
        let project = report.findings.iter().find(|f| f.package == "P").unwrap();
        assert_eq!(project.exact_version(), None);
        assert_eq!(project.provenance.as_deref(), Some("project reference"));
        let future = report.findings.iter().find(|f| f.package == "F").unwrap();
        assert_eq!(future.relation, Some(DependencyRelation::Unknown));
        assert_eq!(future.exact_version(), Some("4.0.0"));
    }

    #[test]
    fn legacy_libraries_shape_is_unsupported() {
        let content = "{\"version\": 2, \"libraries\": {\"A/1.0\": {}}}";
        let report = parse_packages_lock_json(content, "packages.lock.json");
        assert_eq!(report.status, ParseStatus::Unsupported);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn malformed_and_unversioned_inputs_stay_distinct() {
        let malformed = parse_packages_lock_json("{invalid", "packages.lock.json");
        assert_eq!(malformed.status, ParseStatus::Malformed);
        let unversioned =
            parse_packages_lock_json("{\"dependencies\": {\"net8.0\": {}}}", "packages.lock.json");
        assert_eq!(unversioned.status, ParseStatus::Partial);
        assert!(unversioned.findings.is_empty());
    }
}
