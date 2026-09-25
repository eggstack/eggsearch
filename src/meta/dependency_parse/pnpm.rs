use std::collections::{BTreeMap, HashMap};

use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyParseReport, DependencyReferenceKind,
    DependencyRelation, DependencySource, ParseDiagnostic,
};

pub(crate) fn parse_pnpm_lock(content: &str, path: &str) -> DependencyParseReport {
    match lockfile_major(content).as_deref() {
        Some("6") => parse_v6_lock(content, path),
        Some("9") => super::pnpm_v9::parse_v9_lock(content, path),
        Some(other) => {
            DependencyParseReport::unsupported(format!("unsupported pnpm lockfileVersion {other}"))
        }
        None => DependencyParseReport::unsupported(
            "unrecognized pnpm lock shape: missing lockfileVersion",
        ),
    }
}

fn lockfile_major(content: &str) -> Option<String> {
    for line in content.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            continue;
        }
        if let Some(value) = line.trim().strip_prefix("lockfileVersion:") {
            let value = value.trim().trim_matches('\'').trim_matches('"').trim();
            let major = value.split('.').next().unwrap_or(value).trim();
            if major.is_empty() || !major.chars().all(|c| c.is_ascii_digit()) {
                return Some(value.to_string());
            }
            return Some(major.to_string());
        }
    }
    None
}

pub(crate) fn strip_peer_suffix(key: &str) -> String {
    match key.find('(') {
        Some(index) => key[..index].trim().to_string(),
        None => key.trim().to_string(),
    }
}

pub(crate) fn unquote(key: &str) -> String {
    let key = key.trim();
    if key.len() >= 2
        && ((key.starts_with('\'') && key.ends_with('\''))
            || (key.starts_with('"') && key.ends_with('"')))
    {
        key[1..key.len() - 1].to_string()
    } else {
        key.to_string()
    }
}

pub(crate) fn split_name_version(id: &str) -> Option<(String, String)> {
    let id = id.trim();
    let at = id.rfind('@')?;
    let name = id[..at].trim();
    let version = id[at + 1..].trim();
    if name.is_empty() || version.is_empty() || version.contains(char::is_whitespace) {
        return None;
    }
    Some((name.to_string(), version.to_string()))
}

pub(crate) struct ImporterEntry {
    pub(crate) specifier: Option<String>,
    pub(crate) version: Option<String>,
}

pub(crate) fn collect_importers(content: &str) -> HashMap<String, ImporterEntry> {
    let mut direct: HashMap<String, ImporterEntry> = HashMap::new();
    let mut in_importers = false;
    let mut importer_depth = false;
    let mut in_deps = false;
    let mut current_name: Option<String> = None;
    let mut specifier: Option<String> = None;
    let mut version: Option<String> = None;

    for line in content.lines() {
        let indent = line.bytes().take_while(|b| *b == b' ').count();
        let trimmed = line.trim();
        if indent == 0 && !trimmed.is_empty() {
            flush_importer(&mut current_name, &mut specifier, &mut version, &mut direct);
            in_importers = trimmed == "importers:";
            importer_depth = false;
            in_deps = false;
            continue;
        }
        if !in_importers {
            continue;
        }
        if indent == 2 && trimmed.ends_with(':') {
            flush_importer(&mut current_name, &mut specifier, &mut version, &mut direct);
            importer_depth = true;
            in_deps = false;
            continue;
        }
        if !importer_depth {
            continue;
        }
        if indent == 4 && trimmed.ends_with(':') {
            flush_importer(&mut current_name, &mut specifier, &mut version, &mut direct);
            in_deps = matches!(
                trimmed.trim_end_matches(':'),
                "dependencies" | "devDependencies" | "optionalDependencies"
            );
            continue;
        }
        if !in_deps {
            continue;
        }
        if indent == 6 && trimmed.ends_with(':') {
            flush_importer(&mut current_name, &mut specifier, &mut version, &mut direct);
            current_name = Some(unquote(trimmed.trim_end_matches(':')));
            specifier = None;
            version = None;
            continue;
        }
        if current_name.is_some() && indent > 6 {
            if let Some(value) = trimmed.strip_prefix("specifier:") {
                specifier = Some(unquote(value));
            } else if let Some(value) = trimmed.strip_prefix("version:") {
                version = Some(unquote(value));
            }
        }
    }
    flush_importer(&mut current_name, &mut specifier, &mut version, &mut direct);
    direct
}

fn flush_importer(
    current_name: &mut Option<String>,
    specifier: &mut Option<String>,
    version: &mut Option<String>,
    direct: &mut HashMap<String, ImporterEntry>,
) {
    if let Some(name) = current_name.take() {
        direct.insert(
            name,
            ImporterEntry {
                specifier: specifier.take(),
                version: version.take(),
            },
        );
    }
}

pub(crate) struct PackageEntry {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) requirement: Option<String>,
    pub(crate) direct: bool,
    pub(crate) dev: bool,
    pub(crate) integrity: Option<String>,
    pub(crate) line: u32,
}

pub(crate) fn push_package(
    findings: &mut Vec<DependencyFinding>,
    index: &mut BTreeMap<(String, String), usize>,
    entry: PackageEntry,
    path: &str,
) {
    let key = (entry.name.clone(), entry.version.clone());
    if index.contains_key(&key) {
        return;
    }
    index.insert(key, findings.len());
    findings.push(DependencyFinding {
        ecosystem: PackageEcosystem::Npm,
        package: entry.name,
        version: Some(entry.version.clone()),
        resolved_version: Some(entry.version),
        version_requirement: entry.requirement,
        reference_kind: None,
        reference_value: None,
        provenance: None,
        target_context: if entry.dev {
            Some("dev".to_string())
        } else {
            None
        },
        integrity_hash: entry.integrity,
        source_file: Some(path.to_string()),
        source_line: Some(entry.line),
        source_kind: DependencySource::LockFile,
        confidence: Some(ApplicabilityConfidence::High),
        relation: Some(if entry.direct {
            DependencyRelation::Direct
        } else {
            DependencyRelation::Transitive
        }),
    });
}

pub(crate) fn push_workspace(
    findings: &mut Vec<DependencyFinding>,
    index: &mut BTreeMap<(String, String), usize>,
    name: String,
    target: String,
    specifier: Option<String>,
    path: &str,
) {
    let key = (name.clone(), String::new());
    if index.contains_key(&key) {
        return;
    }
    index.insert(key, findings.len());
    findings.push(DependencyFinding {
        ecosystem: PackageEcosystem::Npm,
        package: name,
        version: None,
        resolved_version: None,
        version_requirement: specifier,
        reference_kind: Some(DependencyReferenceKind::Workspace),
        reference_value: Some(target.clone()),
        provenance: Some(format!("workspace: {target}")),
        target_context: None,
        integrity_hash: None,
        source_file: Some(path.to_string()),
        source_line: None,
        source_kind: DependencySource::LockFile,
        confidence: Some(ApplicabilityConfidence::Medium),
        relation: Some(DependencyRelation::Direct),
    });
}

fn parse_v6_lock(content: &str, path: &str) -> DependencyParseReport {
    let importers = collect_importers(content);
    let mut findings: Vec<DependencyFinding> = Vec::new();
    let mut index: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut in_packages = false;
    let mut current: Option<(String, String, u32)> = None;
    let mut integrity: Option<String> = None;
    let mut dev = false;
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let indent = line.bytes().take_while(|b| *b == b' ').count();
        let trimmed = line.trim();
        if indent == 0 && !trimmed.is_empty() {
            flush_package(
                &mut current,
                &mut integrity,
                &mut dev,
                &importers,
                false,
                &mut findings,
                &mut index,
                path,
            );
            in_packages = trimmed == "packages:";
            continue;
        }
        if !in_packages {
            continue;
        }
        if indent == 2 && trimmed.ends_with(':') {
            flush_package(
                &mut current,
                &mut integrity,
                &mut dev,
                &importers,
                false,
                &mut findings,
                &mut index,
                path,
            );
            let key = strip_peer_suffix(&unquote(trimmed.trim_end_matches(':')));
            if key.starts_with("file:") || key.starts_with("link:") {
                current = None;
                continue;
            }
            if let Some((name, version)) = split_name_version(key.trim_start_matches('/')) {
                current = Some((name, version, line_num));
                integrity = None;
                dev = false;
            }
            continue;
        }
        if current.is_some() && indent > 2 {
            if let Some(value) = trimmed.strip_prefix("resolution:") {
                if let Some(value) = unquote(value).strip_prefix("{integrity:") {
                    integrity = Some(value.trim_end_matches('}').trim().to_string());
                }
            } else if trimmed == "dev: true" {
                dev = true;
            }
        }
    }
    flush_package(
        &mut current,
        &mut integrity,
        &mut dev,
        &importers,
        false,
        &mut findings,
        &mut index,
        path,
    );

    if findings.is_empty() {
        DependencyParseReport::partial(
            findings,
            vec![ParseDiagnostic {
                code: "dependency_parse_partial".to_string(),
                message: "pnpm v6 lockfile has no recognizable package entries".to_string(),
                line: None,
            }],
        )
    } else {
        DependencyParseReport::complete(findings)
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn flush_package(
    current: &mut Option<(String, String, u32)>,
    integrity: &mut Option<String>,
    dev: &mut bool,
    importers: &HashMap<String, ImporterEntry>,
    match_version: bool,
    findings: &mut Vec<DependencyFinding>,
    index: &mut BTreeMap<(String, String), usize>,
    path: &str,
) {
    let Some((name, version, line)) = current.take() else {
        return;
    };
    let integrity = integrity.take();
    let dev = std::mem::replace(dev, false);
    let direct_entry = importers.get(&name);
    let requirement = match (direct_entry, match_version) {
        (Some(entry), true) => {
            if entry.version.as_deref().is_some_and(|v| v == version) {
                entry.specifier.clone()
            } else {
                None
            }
        }
        (Some(entry), false) => entry.specifier.clone(),
        (None, _) => None,
    };
    push_package(
        findings,
        index,
        PackageEntry {
            name,
            version,
            requirement,
            direct: direct_entry.is_some(),
            dev,
            integrity,
            line,
        },
        path,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::security_applicability::ParseStatus;

    const V6: &str = "lockfileVersion: '6.0'\n\npackages:\n\n  /lodash@4.17.21:\n    resolution: {integrity: sha512-x}\n    dev: false\n\n  /@babel/core@7.20.12:\n    resolution: {integrity: sha512-y}\n    dev: true\n\nimporters:\n\n  .:\n    dependencies:\n      lodash:\n        specifier: ^4.17.21\n        version: 4.17.21\n";

    #[test]
    fn v6_scoped_and_direct_marking() {
        let report = parse_pnpm_lock(V6, "pnpm-lock.yaml");
        assert_eq!(report.status, ParseStatus::Complete);
        assert_eq!(report.findings.len(), 2);
        let lodash = report
            .findings
            .iter()
            .find(|f| f.package == "lodash")
            .unwrap();
        assert_eq!(lodash.exact_version(), Some("4.17.21"));
        assert_eq!(lodash.relation, Some(DependencyRelation::Direct));
        assert_eq!(lodash.version_requirement.as_deref(), Some("^4.17.21"));
        let core = report
            .findings
            .iter()
            .find(|f| f.package == "@babel/core")
            .unwrap();
        assert_eq!(core.target_context.as_deref(), Some("dev"));
        assert!(report.findings.iter().all(|f| !f.package.contains('(')
            && !f.package.contains('\'')
            && !f.package.contains('"')));
    }
}
