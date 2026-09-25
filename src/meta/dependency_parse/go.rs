use std::collections::HashMap;

use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyReferenceKind, DependencyRelation,
    DependencySource,
};

pub(crate) fn parse_go_mod(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut replacements: HashMap<String, String> = HashMap::new();
    let mut in_replace_block = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("replace (") || trimmed == "replace" {
            in_replace_block = true;
            continue;
        }
        if trimmed == ")" {
            in_replace_block = false;
            continue;
        }
        let directive = if in_replace_block {
            Some(trimmed)
        } else {
            trimmed.strip_prefix("replace ").map(str::trim)
        };
        if let Some(directive) = directive {
            if let Some((old, new)) = directive.split_once("=>") {
                let old_module = old.split_whitespace().next().unwrap_or("").to_string();
                let new = new.trim().to_string();
                if !old_module.is_empty() && !new.is_empty() {
                    replacements.insert(old_module, new);
                }
            }
        }
    }

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
        if trimmed == ")" {
            in_require = false;
            continue;
        }
        if trimmed.starts_with("replace") {
            continue;
        }
        if in_require || trimmed.starts_with("require ") {
            let rest = if in_require {
                trimmed
            } else {
                trimmed.trim_start_matches("require ").trim()
            };
            if rest.starts_with("//") || rest.is_empty() {
                continue;
            }
            let indirect = rest.contains("// indirect");
            let code = rest.split("//").next().unwrap_or(rest);
            let parts: Vec<&str> = code.split_whitespace().collect();
            if parts.len() >= 2 {
                let module = parts[0].to_string();
                let version = parts[1].trim_start_matches('v').to_string();
                let mut provenance: Option<String> = None;
                let mut reference_kind: Option<DependencyReferenceKind> = None;
                let mut reference_value: Option<String> = None;
                if let Some(replacement) = replacements.get(&module) {
                    provenance = Some(format!("replace => {replacement}"));
                    let replacement_path =
                        replacement.split_whitespace().next().unwrap_or(replacement);
                    let is_path = replacement_path.starts_with('.')
                        || replacement_path.starts_with('/')
                        || replacement_path.starts_with('\\')
                        || (replacement_path.len() >= 2 && replacement_path.as_bytes()[1] == b':');
                    if is_path {
                        reference_kind = Some(DependencyReferenceKind::Path);
                        reference_value = Some(replacement.clone());
                    }
                }
                findings.push(DependencyFinding {
                    ecosystem: PackageEcosystem::Go,
                    package: module,
                    version: Some(version.clone()),
                    resolved_version: None,
                    version_requirement: Some(version),
                    reference_kind,
                    reference_value,
                    provenance,
                    target_context: None,
                    integrity_hash: None,
                    source_file: Some(path.to_string()),
                    source_line: Some(line_num),
                    source_kind: DependencySource::Manifest,
                    confidence: Some(ApplicabilityConfidence::Medium),
                    relation: Some(if indirect {
                        DependencyRelation::Transitive
                    } else {
                        DependencyRelation::Direct
                    }),
                });
            }
        }
    }
    findings
}

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
            let clean_version = version.split('/').next().unwrap_or(version);
            let key = (module.to_string(), clean_version.to_string());
            if seen.insert(key) {
                findings.push(DependencyFinding {
                    ecosystem: PackageEcosystem::Go,
                    package: module.to_string(),
                    version: Some(clean_version.to_string()),
                    resolved_version: None,
                    version_requirement: None,
                    reference_kind: None,
                    reference_value: None,
                    provenance: Some("go.sum checksum history".to_string()),
                    target_context: None,
                    integrity_hash: Some(parts.get(2).unwrap_or(&"").to_string()),
                    source_file: Some(path.to_string()),
                    source_line: Some(line_num),
                    source_kind: DependencySource::LockFile,
                    confidence: Some(ApplicabilityConfidence::Medium),
                    relation: Some(DependencyRelation::Transitive),
                });
            }
        }
    }

    findings
}

pub(crate) fn parse_vendor_modules(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings: Vec<DependencyFinding> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut line_num = 0u32;
    let mut last_index: Option<usize> = None;

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();
        if trimmed.starts_with("## explicit") {
            if let Some(index) = last_index {
                if let Some(finding) = findings.get_mut(index) {
                    finding.relation = Some(DependencyRelation::Direct);
                }
            }
            continue;
        }
        if trimmed.starts_with("##") {
            continue;
        }
        let Some(header) = trimmed.strip_prefix("# ") else {
            last_index = None;
            continue;
        };
        let mut parts = header.split_whitespace();
        let Some(module) = parts.next() else {
            continue;
        };
        let Some(raw_version) = parts.next() else {
            continue;
        };
        if module.is_empty() || raw_version.is_empty() {
            continue;
        }
        let version = raw_version.trim_start_matches('v').to_string();
        let rest: Vec<&str> = parts.collect();
        let mut provenance: Option<String> = None;
        if let Some(arrow) = rest.iter().position(|token| *token == "=>") {
            let replacement = rest[arrow + 1..].join(" ");
            if !replacement.is_empty() {
                provenance = Some(format!("replace => {replacement}"));
            }
        }
        if seen.insert(module.to_string()) {
            findings.push(DependencyFinding {
                ecosystem: PackageEcosystem::Go,
                package: module.to_string(),
                version: Some(version.clone()),
                resolved_version: Some(version),
                version_requirement: None,
                reference_kind: None,
                reference_value: None,
                provenance,
                target_context: None,
                integrity_hash: None,
                source_file: Some(path.to_string()),
                source_line: Some(line_num),
                source_kind: DependencySource::LockFile,
                confidence: Some(ApplicabilityConfidence::High),
                relation: Some(DependencyRelation::Transitive),
            });
            last_index = Some(findings.len() - 1);
        } else {
            last_index = None;
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indirect_requirements_are_transitive() {
        let content = "module example.com/x\n\ngo 1.21\n\nrequire (\n\tgithub.com/direct/mod v1.0.0\n\tgithub.com/indirect/mod v2.0.0 // indirect\n)\n";
        let findings = parse_go_mod(content, "go.mod");
        assert_eq!(findings.len(), 2);
        let direct = findings
            .iter()
            .find(|f| f.package == "github.com/direct/mod")
            .unwrap();
        assert_eq!(direct.relation, Some(DependencyRelation::Direct));
        assert_eq!(direct.source_kind, DependencySource::Manifest);
        assert!(!direct.has_resolved_evidence());
        let indirect = findings
            .iter()
            .find(|f| f.package == "github.com/indirect/mod")
            .unwrap();
        assert_eq!(indirect.relation, Some(DependencyRelation::Transitive));
    }

    #[test]
    fn replace_to_module_and_path_keep_provenance() {
        let content = "module example.com/x\n\ngo 1.21\n\nrequire (\n\tgithub.com/a/mod v1.0.0\n\tgithub.com/b/mod v1.0.0\n)\n\nreplace github.com/a/mod => github.com/fork/mod v1.2.3\n\nreplace github.com/b/mod => ../local/b\n";
        let findings = parse_go_mod(content, "go.mod");
        assert_eq!(findings.len(), 2);
        let forked = findings
            .iter()
            .find(|f| f.package == "github.com/a/mod")
            .unwrap();
        assert!(forked
            .provenance
            .as_deref()
            .unwrap_or("")
            .contains("github.com/fork/mod"));
        assert_eq!(forked.reference_kind, None);
        let local = findings
            .iter()
            .find(|f| f.package == "github.com/b/mod")
            .unwrap();
        assert_eq!(local.reference_kind, Some(DependencyReferenceKind::Path));
        assert!(!local.has_resolved_evidence());
    }

    #[test]
    fn stale_go_sum_versions_are_integrity_only() {
        let content = "example.com/stale v9.9.9 h1:abc\nexample.com/stale v9.9.9/go.mod h1:def\n";
        let findings = parse_go_sum(content, "go.sum");
        assert_eq!(findings.len(), 1);
        assert!(!findings[0].has_resolved_evidence());
        assert!(findings[0].integrity_hash.is_some());
    }

    #[test]
    fn vendor_manifest_yields_resolved_versions() {
        let content = "# github.com/vendored/mod v1.4.2\n## explicit; go 1.21\n# github.com/indirect/dep v0.9.0\n";
        let findings = parse_vendor_modules(content, "vendor/modules.txt");
        assert_eq!(findings.len(), 2);
        let direct = findings
            .iter()
            .find(|f| f.package == "github.com/vendored/mod")
            .unwrap();
        assert_eq!(direct.exact_version(), Some("1.4.2"));
        assert_eq!(direct.relation, Some(DependencyRelation::Direct));
        let transitive = findings
            .iter()
            .find(|f| f.package == "github.com/indirect/dep")
            .unwrap();
        assert_eq!(transitive.relation, Some(DependencyRelation::Transitive));
    }
}
