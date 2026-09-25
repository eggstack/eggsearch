use std::collections::BTreeMap;

use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyParseReport, DependencyReferenceKind,
    DependencyRelation, DependencySource,
};

pub(crate) struct YarnEntry {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) requirement: Option<String>,
    pub(crate) provenance: Option<String>,
    pub(crate) reference_kind: Option<DependencyReferenceKind>,
    pub(crate) reference_value: Option<String>,
    pub(crate) integrity: Option<String>,
    pub(crate) line: u32,
}

pub(crate) fn push_entry(
    findings: &mut Vec<DependencyFinding>,
    index: &mut BTreeMap<(String, String, String), usize>,
    entry: YarnEntry,
    path: &str,
) {
    let key = (
        entry.name.clone(),
        entry.version.clone(),
        entry.provenance.clone().unwrap_or_default(),
    );
    if let Some(existing) = index.get(&key).and_then(|i| findings.get_mut(*i)) {
        merge_requirement(existing, entry.requirement);
        return;
    }
    index.insert(key, findings.len());
    findings.push(DependencyFinding {
        ecosystem: PackageEcosystem::Npm,
        package: entry.name,
        version: Some(entry.version.clone()),
        resolved_version: Some(entry.version),
        version_requirement: entry.requirement,
        reference_kind: entry.reference_kind,
        reference_value: entry.reference_value,
        provenance: entry.provenance,
        target_context: None,
        integrity_hash: entry.integrity,
        source_file: Some(path.to_string()),
        source_line: Some(entry.line),
        source_kind: DependencySource::LockFile,
        confidence: Some(ApplicabilityConfidence::High),
        relation: Some(DependencyRelation::Transitive),
    });
}
pub(crate) fn merge_requirement(finding: &mut DependencyFinding, requirement: Option<String>) {
    let Some(requirement) = requirement.filter(|r| !r.is_empty()) else {
        return;
    };
    match finding.version_requirement.as_mut() {
        Some(existing) if !existing.split(", ").any(|r| r == requirement) => {
            existing.push_str(", ");
            existing.push_str(&requirement);
        }
        Some(_) => {}
        None => finding.version_requirement = Some(requirement),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_workspace_entry_helper(
    findings: &mut Vec<DependencyFinding>,
    index: &mut BTreeMap<(String, String, String), usize>,
    package: String,
    version: &str,
    requirement: Option<String>,
    provenance: Option<String>,
    reference: (Option<DependencyReferenceKind>, Option<String>),
    line: u32,
    path: &str,
) {
    let key = (
        package.clone(),
        String::new(),
        provenance.clone().unwrap_or_default(),
    );
    if let Some(existing) = index.get(&key).and_then(|i| findings.get_mut(*i)) {
        merge_requirement(existing, requirement);
        return;
    }
    index.insert(key, findings.len());
    findings.push(DependencyFinding {
        ecosystem: PackageEcosystem::Npm,
        package,
        version: Some(version.to_string()),
        resolved_version: None,
        version_requirement: requirement,
        reference_kind: reference.0,
        reference_value: reference.1,
        provenance,
        target_context: None,
        integrity_hash: None,
        source_file: Some(path.to_string()),
        source_line: Some(line),
        source_kind: DependencySource::LockFile,
        confidence: Some(ApplicabilityConfidence::Medium),
        relation: Some(DependencyRelation::Transitive),
    });
}

pub(crate) fn extend_selectors(selectors: &mut Vec<String>, header: &str) {
    for selector in header.split(',') {
        let selector = selector.trim().trim_matches('"').trim().to_string();
        if !selector.is_empty() {
            selectors.push(selector);
        }
    }
}

pub(crate) fn take_quoted(value: &str, slot: &mut Option<String>) {
    let value = value.trim().trim_matches('"').to_string();
    if !value.is_empty() {
        *slot = Some(value);
    }
}

pub(crate) fn entry_name(selectors: &[String]) -> Option<String> {
    selectors
        .iter()
        .find_map(|selector| selector_name(selector))
}

pub(crate) fn selector_name(selector: &str) -> Option<String> {
    let selector = selector.trim().trim_matches('"').trim();
    if selector.is_empty() || selector.contains(char::is_whitespace) {
        return None;
    }
    if let Some(at) = selector.rfind('@') {
        let candidate = selector[..at].trim();
        if candidate.is_empty() {
            Some(selector.to_string())
        } else {
            Some(candidate.to_string())
        }
    } else {
        Some(selector.to_string())
    }
}

pub(crate) fn entry_requirement(selectors: &[String]) -> Option<String> {
    let mut ranges: Vec<String> = Vec::new();
    for selector in selectors {
        if let Some(at) = selector.rfind('@') {
            let range = selector[at + 1..].trim();
            if !range.is_empty() && !ranges.contains(&range.to_string()) {
                ranges.push(range.to_string());
            }
        }
    }
    ranges.sort();
    if ranges.is_empty() {
        None
    } else {
        Some(ranges.join(", "))
    }
}

pub(crate) fn parse_yarn_lock(content: &str, path: &str) -> DependencyParseReport {
    if super::yarn_berry::is_berry_lock(content) {
        super::yarn_berry::parse_berry_lock(content, path)
    } else {
        parse_classic_lock(content, path)
    }
}

fn parse_classic_lock(content: &str, path: &str) -> DependencyParseReport {
    let mut findings: Vec<DependencyFinding> = Vec::new();
    let mut index: BTreeMap<(String, String, String), usize> = BTreeMap::new();
    let mut selectors: Vec<String> = Vec::new();
    let mut version: Option<String> = None;
    let mut resolved: Option<String> = None;
    let mut integrity: Option<String> = None;
    let mut line_num = 0u32;
    let mut entry_line = 0u32;
    let mut entries_seen = 0usize;

    for line in content.lines() {
        line_num += 1;
        if line.starts_with('"')
            || (!line.starts_with(' ') && !line.starts_with('\t') && line.trim().ends_with(':'))
        {
            flush_classic(
                &selectors,
                &mut version,
                &mut resolved,
                &mut integrity,
                &mut findings,
                &mut index,
                &mut entries_seen,
                entry_line,
                path,
            );
            selectors.clear();
            version = None;
            resolved = None;
            integrity = None;
            if let Some(header) = line.trim().strip_suffix(':') {
                entry_line = line_num;
                extend_selectors(&mut selectors, header);
            }
            continue;
        }
        if selectors.is_empty() {
            continue;
        }
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("version ") {
            take_quoted(value, &mut version);
        } else if let Some(value) = trimmed.strip_prefix("resolved ") {
            take_quoted(value, &mut resolved);
        } else if let Some(value) = trimmed.strip_prefix("integrity ") {
            let value = value.trim().to_string();
            if !value.is_empty() {
                integrity = Some(value);
            }
        }
    }
    flush_classic(
        &selectors,
        &mut version,
        &mut resolved,
        &mut integrity,
        &mut findings,
        &mut index,
        &mut entries_seen,
        entry_line,
        path,
    );

    if entries_seen == 0 {
        DependencyParseReport::unsupported("unrecognized yarn.lock shape")
    } else {
        DependencyParseReport::complete(findings)
    }
}

#[allow(clippy::too_many_arguments)]
fn flush_classic(
    selectors: &[String],
    version: &mut Option<String>,
    resolved: &mut Option<String>,
    integrity: &mut Option<String>,
    findings: &mut Vec<DependencyFinding>,
    index: &mut BTreeMap<(String, String, String), usize>,
    entries_seen: &mut usize,
    entry_line: u32,
    path: &str,
) {
    if selectors.is_empty() {
        return;
    }
    *entries_seen += 1;
    let Some(name) = entry_name(selectors) else {
        return;
    };
    let version = version.take().unwrap_or_default();
    if version.is_empty() {
        return;
    }
    let (reference_kind, reference_value, provenance) =
        classic_provenance(resolved.take().as_deref());
    push_entry(
        findings,
        index,
        YarnEntry {
            name,
            version,
            requirement: entry_requirement(selectors),
            provenance,
            reference_kind,
            reference_value,
            integrity: integrity.take(),
            line: entry_line,
        },
        path,
    );
}

fn classic_provenance(
    url: Option<&str>,
) -> (
    Option<DependencyReferenceKind>,
    Option<String>,
    Option<String>,
) {
    let Some(url) = url.filter(|u| !u.is_empty()) else {
        return (None, None, None);
    };
    if url.starts_with("git:") || url.starts_with("git+") {
        return (
            Some(DependencyReferenceKind::Git),
            Some(url.to_string()),
            Some(format!("git: {url}")),
        );
    }
    if url.starts_with("file:") {
        return (
            Some(DependencyReferenceKind::Path),
            Some(url.to_string()),
            Some(format!("file: {url}")),
        );
    }
    (None, None, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::security_applicability::ParseStatus;

    const CLASSIC: &str = "# yarn lockfile v1\n\n\"@babel/core@^7.20.0, @babel/core@^7.21.0\":\n  version \"7.20.12\"\n  resolved \"https://registry.yarnpkg.com/@babel/core/-/core-7.20.12.tgz\"\n  integrity sha512-x\ndash@^4.17.21, lodash@~4.17.20:\n  version \"4.17.21\"\n  resolved \"https://registry.yarnpkg.com/lodash/-/lodash-4.17.21.tgz\"\n";

    #[test]
    fn classic_grouped_and_scoped_selectors() {
        let report = parse_yarn_lock(CLASSIC, "yarn.lock");
        assert_eq!(report.status, ParseStatus::Complete);
        assert_eq!(report.findings.len(), 2);
        let core = report
            .findings
            .iter()
            .find(|f| f.package == "@babel/core")
            .unwrap();
        assert_eq!(core.exact_version(), Some("7.20.12"));
        assert_eq!(core.integrity_hash.as_deref(), Some("sha512-x"));
        assert!(report.findings.iter().all(|f| !f.package.contains('"')));
    }
}
