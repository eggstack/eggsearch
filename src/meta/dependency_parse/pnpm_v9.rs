use std::collections::BTreeMap;

use super::pnpm::{
    collect_importers, flush_package, push_workspace, split_name_version, strip_peer_suffix,
    unquote,
};
use crate::core::security_applicability::{
    DependencyFinding, DependencyParseReport, ParseDiagnostic,
};

pub(crate) fn parse_v9_lock(content: &str, path: &str) -> DependencyParseReport {
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
                true,
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
                true,
                &mut findings,
                &mut index,
                path,
            );
            let key = strip_peer_suffix(&unquote(trimmed.trim_end_matches(':')));
            if key.starts_with("link:") || key.starts_with("file:") || key.starts_with("portal:") {
                current = None;
                continue;
            }
            if let Some((name, version)) = split_name_version(&key) {
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
        true,
        &mut findings,
        &mut index,
        path,
    );

    for (name, entry) in &importers {
        let version = entry.version.as_deref().unwrap_or("");
        if version.starts_with("link:") || version.starts_with("file:") {
            push_workspace(
                &mut findings,
                &mut index,
                name.clone(),
                version.to_string(),
                entry.specifier.clone(),
                path,
            );
        }
    }

    if findings.is_empty() {
        DependencyParseReport::partial(
            findings,
            vec![ParseDiagnostic {
                code: "dependency_parse_partial".to_string(),
                message: "pnpm v9 lockfile has no recognizable package entries".to_string(),
                line: None,
            }],
        )
    } else {
        DependencyParseReport::complete(findings)
    }
}

#[cfg(test)]
mod tests {
    use super::super::pnpm::parse_pnpm_lock;
    use crate::core::security_applicability::DependencyRelation;
    use crate::core::security_applicability::ParseStatus;

    const V9: &str = "lockfileVersion: 9.0\n\npackages:\n\n  lodash@4.17.21:\n    resolution: {integrity: sha512-x}\n\n  '@babel/core@7.20.12':\n    resolution: {integrity: sha512-y}\n\nsnapshots:\n\n  lodash@4.17.21:\n    dependencies:\n      foo: 1.0.0\n\nimporters:\n\n  .:\n    dependencies:\n      lodash:\n        specifier: ^4.17.21\n        version: 4.17.21\n";

    #[test]
    fn v9_ids_importers_and_snapshot_isolation() {
        let report = parse_pnpm_lock(V9, "pnpm-lock.yaml");
        assert_eq!(report.status, ParseStatus::Complete);
        assert_eq!(report.findings.len(), 2);
        let lodash = report
            .findings
            .iter()
            .find(|f| f.package == "lodash")
            .unwrap();
        assert_eq!(lodash.exact_version(), Some("4.17.21"));
        assert_eq!(lodash.relation, Some(DependencyRelation::Direct));
        assert!(report.findings.iter().all(|f| !f.package.contains('(')));
    }

    #[test]
    fn unsupported_major_is_explicit() {
        let report = parse_pnpm_lock("lockfileVersion: '5.4'\n", "pnpm-lock.yaml");
        assert_eq!(report.status, ParseStatus::Unsupported);
        let missing = parse_pnpm_lock("packages:\n", "pnpm-lock.yaml");
        assert_eq!(missing.status, ParseStatus::Unsupported);
    }
}
