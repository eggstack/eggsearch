use std::collections::BTreeMap;

use super::yarn::{
    entry_name, entry_requirement, extend_selectors, push_entry, push_workspace_entry_helper,
    take_quoted, YarnEntry,
};
use crate::core::security_applicability::{
    DependencyFinding, DependencyParseReport, DependencyReferenceKind,
};

const SUPPORTED_BERRY_METADATA: &[u64] = &[5, 6, 7, 8];

type BerryIdentity = (
    String,
    Option<String>,
    (Option<DependencyReferenceKind>, Option<String>),
    Option<String>,
);

pub(crate) fn is_berry_lock(content: &str) -> bool {
    content.lines().any(|line| line.trim() == "__metadata:")
}

pub(crate) fn parse_berry_lock(content: &str, path: &str) -> DependencyParseReport {
    match berry_metadata_version(content) {
        Some(version) if !SUPPORTED_BERRY_METADATA.contains(&version) => {
            return DependencyParseReport::unsupported(format!(
                "unsupported yarn __metadata version {version}"
            ));
        }
        _ => {}
    }
    let mut findings: Vec<DependencyFinding> = Vec::new();
    let mut index: BTreeMap<(String, String, String), usize> = BTreeMap::new();
    let mut selectors: Vec<String> = Vec::new();
    let mut version: Option<String> = None;
    let mut resolution: Option<String> = None;
    let mut line_num = 0u32;
    let mut entry_line = 0u32;
    let mut entries_seen = 0usize;

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();
        if trimmed == "__metadata:" || trimmed.starts_with("__metadata:") {
            flush_berry(
                &selectors,
                &mut version,
                &mut resolution,
                &mut findings,
                &mut index,
                &mut entries_seen,
                entry_line,
                path,
            );
            selectors.clear();
            version = None;
            resolution = None;
            continue;
        }
        if !line.starts_with(' ') && !line.starts_with('\t') && trimmed.ends_with(':') {
            flush_berry(
                &selectors,
                &mut version,
                &mut resolution,
                &mut findings,
                &mut index,
                &mut entries_seen,
                entry_line,
                path,
            );
            selectors.clear();
            version = None;
            resolution = None;
            if let Some(header) = trimmed.strip_suffix(':') {
                entry_line = line_num;
                extend_selectors(&mut selectors, header);
            }
            continue;
        }
        if selectors.is_empty() {
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("version:") {
            take_quoted(value, &mut version);
        } else if let Some(value) = trimmed.strip_prefix("resolution:") {
            take_quoted(value, &mut resolution);
        }
    }
    flush_berry(
        &selectors,
        &mut version,
        &mut resolution,
        &mut findings,
        &mut index,
        &mut entries_seen,
        entry_line,
        path,
    );

    if entries_seen == 0 {
        DependencyParseReport::unsupported("unrecognized yarn Berry lock shape")
    } else {
        DependencyParseReport::complete(findings)
    }
}

#[allow(clippy::too_many_arguments)]
fn flush_berry(
    selectors: &[String],
    version: &mut Option<String>,
    resolution: &mut Option<String>,
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
    let resolution = resolution.take().unwrap_or_default();
    let (package, provenance, reference, resolved) = berry_identity(&name, &version, &resolution);
    match resolved {
        Some(resolved) => push_entry(
            findings,
            index,
            YarnEntry {
                name: package,
                version: resolved,
                requirement: entry_requirement(selectors),
                provenance,
                reference_kind: reference.0,
                reference_value: reference.1,
                integrity: None,
                line: entry_line,
            },
            path,
        ),
        None => push_workspace_entry_helper(
            findings,
            index,
            package,
            &version,
            entry_requirement(selectors),
            provenance,
            reference,
            entry_line,
            path,
        ),
    }
}

fn berry_metadata_version(content: &str) -> Option<u64> {
    let mut in_metadata = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "__metadata:" {
            in_metadata = true;
            continue;
        }
        if in_metadata {
            if !line.starts_with(' ') && !line.starts_with('\t') {
                break;
            }
            if let Some(value) = trimmed.strip_prefix("version:") {
                return value.trim().trim_matches('"').parse().ok();
            }
        }
    }
    None
}

fn berry_identity(header_name: &str, version: &str, resolution: &str) -> BerryIdentity {
    let (name, protocol, selector) = split_resolution(resolution).unwrap_or_else(|| {
        (
            header_name.to_string(),
            "unknown".to_string(),
            String::new(),
        )
    });
    let package = if name.is_empty() {
        header_name.to_string()
    } else {
        name
    };
    match protocol.as_str() {
        "npm" => (package, None, (None, None), Some(version.to_string())),
        "workspace" | "portal" | "link" => (
            package,
            Some(format!("{protocol}: {selector}")),
            (Some(DependencyReferenceKind::Workspace), Some(selector)),
            None,
        ),
        "file" => (
            package,
            Some(format!("file: {selector}")),
            (Some(DependencyReferenceKind::Path), Some(selector)),
            None,
        ),
        "patch" => (
            package,
            Some(format!("patch: {selector}")),
            (None, None),
            Some(version.to_string()),
        ),
        "git" | "http" | "https" | "exec" => (
            package,
            Some(format!("{protocol}: {selector}")),
            (
                Some(DependencyReferenceKind::Url),
                Some(format!("{protocol}:{selector}")),
            ),
            None,
        ),
        _ => (
            package,
            Some(format!("{protocol}: {selector}")),
            (None, None),
            Some(version.to_string()),
        ),
    }
}

fn split_resolution(resolution: &str) -> Option<(String, String, String)> {
    let resolution = resolution.trim().trim_matches('"').trim();
    if resolution.is_empty() {
        return None;
    }
    let at = resolution.rfind('@')?;
    let name = resolution[..at].trim().to_string();
    let rest = resolution[at + 1..].trim();
    let colon = rest.find(':')?;
    Some((
        name,
        rest[..colon].trim().to_string(),
        rest[colon + 1..].trim().to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::package::PackageEcosystem;
    use crate::core::security_applicability::ParseStatus;
    use crate::core::security_applicability::{DependencyRelation, DependencySource};

    const BERRY: &str = "__metadata:\n  version: 8\n  cacheKey: 8\n\n\"lodash@npm:^4.17.21\":\n  version: 4.17.21\n  resolution: \"lodash@npm:4.17.21\"\n  checksum: c1\n  languageName: node\n  linkType: hard\n\n\"mypkg@workspace:.\":\n  version: 0.0.0-use.local\n  resolution: \"mypkg@workspace:.\"\n  languageName: unknown\n  linkType: soft\n";

    #[test]
    fn berry_metadata_resolution_and_workspace() {
        let report = parse_berry_lock(BERRY, "yarn.lock");
        assert_eq!(report.status, ParseStatus::Complete);
        assert_eq!(report.findings.len(), 2);
        let lodash = report
            .findings
            .iter()
            .find(|f| f.package == "lodash")
            .unwrap();
        assert_eq!(lodash.exact_version(), Some("4.17.21"));
        assert_eq!(lodash.ecosystem, PackageEcosystem::Npm);
        assert_eq!(lodash.source_kind, DependencySource::LockFile);
        assert_eq!(lodash.relation, Some(DependencyRelation::Transitive));
        let workspace = report
            .findings
            .iter()
            .find(|f| f.package == "mypkg")
            .unwrap();
        assert_eq!(workspace.exact_version(), None);
        assert_eq!(workspace.provenance.as_deref(), Some("workspace: ."));
    }

    #[test]
    fn unsupported_berry_metadata_is_explicit() {
        let content = "__metadata:\n  version: 99\n\n\"a@npm:^1.0.0\":\n  version: 1.0.0\n  resolution: \"a@npm:1.0.0\"\n";
        let report = parse_berry_lock(content, "yarn.lock");
        assert_eq!(report.status, ParseStatus::Unsupported);
    }
}
