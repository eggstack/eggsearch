use std::collections::HashSet;

use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyRelation, DependencySource,
};

pub(crate) fn parse_gemfile_lock(content: &str, path: &str) -> Vec<DependencyFinding> {
    let direct = collect_direct_names(content);
    let mut findings = Vec::new();
    let mut section = SourceSection::None;
    let mut in_specs = false;
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        if line.starts_with(' ') || line.starts_with('\t') {
            if line.trim() == "specs:" {
                in_specs = matches!(
                    section,
                    SourceSection::Gem { .. }
                        | SourceSection::Git { .. }
                        | SourceSection::Path { .. }
                );
            } else if in_specs {
                if let Some(spec) = parse_spec_row(line, &section, path, line_num, &direct) {
                    findings.push(spec);
                }
            } else {
                section.subkey(line);
            }
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(name) = trimmed.strip_suffix(':') {
            section = SourceSection::named(name);
            in_specs = false;
            continue;
        }
        if matches!(
            trimmed,
            "GEM" | "GIT" | "PATH" | "PLATFORMS" | "DEPENDENCIES" | "BUNDLED WITH" | "RUBY VERSION"
        ) {
            if trimmed == "DEPENDENCIES" {
                section = SourceSection::None;
            } else {
                section =
                    SourceSection::named(trimmed.split_whitespace().next().unwrap_or(trimmed));
            }
            in_specs = false;
            continue;
        }
        in_specs = false;
    }

    findings
}

fn collect_direct_names(content: &str) -> HashSet<String> {
    let mut names = HashSet::new();
    let mut in_dependencies = false;
    for line in content.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            if in_dependencies {
                let entry = line.trim().trim_end_matches('!');
                let name = entry.split('(').next().unwrap_or(entry).trim();
                if !name.is_empty() {
                    names.insert(name.to_lowercase());
                }
            }
            continue;
        }
        let trimmed = line.trim();
        in_dependencies = trimmed == "DEPENDENCIES";
    }
    names
}

enum SourceSection {
    None,
    Gem { remote: String },
    Git { remote: String, detail: Vec<String> },
    Path { remote: String },
    Other,
}

impl SourceSection {
    fn named(name: &str) -> Self {
        match name {
            "GEM" => SourceSection::Gem {
                remote: String::new(),
            },
            "GIT" => SourceSection::Git {
                remote: String::new(),
                detail: Vec::new(),
            },
            "PATH" => SourceSection::Path {
                remote: String::new(),
            },
            _ => SourceSection::Other,
        }
    }

    fn subkey(&mut self, line: &str) {
        let trimmed = line.trim();
        let Some((key, value)) = trimmed.split_once(':') else {
            return;
        };
        let value = value.trim().to_string();
        if value.is_empty() {
            return;
        }
        match self {
            SourceSection::Gem { remote } => {
                if key.trim() == "remote" && remote.is_empty() {
                    *remote = value;
                }
            }
            SourceSection::Git { remote, detail } => match key.trim() {
                "remote" if remote.is_empty() => *remote = value,
                "revision" | "branch" | "ref" | "tag" => {
                    detail.push(format!("{}: {value}", key.trim()));
                }
                _ => {}
            },
            SourceSection::Path { remote } => {
                if key.trim() == "remote" && remote.is_empty() {
                    *remote = value;
                }
            }
            SourceSection::None | SourceSection::Other => {}
        }
    }

    fn provenance(&self) -> Option<String> {
        match self {
            SourceSection::Gem { remote } => {
                if remote.is_empty() {
                    None
                } else {
                    Some(format!("gem: {remote}"))
                }
            }
            SourceSection::Git { remote, detail } => {
                let mut parts = Vec::new();
                if remote.is_empty() {
                    parts.push("git".to_string());
                } else {
                    parts.push(format!("git: {remote}"));
                }
                parts.extend(detail.iter().cloned());
                Some(parts.join("; "))
            }
            SourceSection::Path { remote } => {
                if remote.is_empty() {
                    Some("path".to_string())
                } else {
                    Some(format!("path: {remote}"))
                }
            }
            SourceSection::None | SourceSection::Other => None,
        }
    }
}

fn indent_of(line: &str) -> usize {
    line.bytes().take_while(|byte| *byte == b' ').count()
}

fn parse_spec_row(
    line: &str,
    section: &SourceSection,
    path: &str,
    line_num: u32,
    direct: &HashSet<String>,
) -> Option<DependencyFinding> {
    if indent_of(line) != 4 {
        return None;
    }
    let body = line.trim();
    let paren = body.find('(')?;
    let name = body[..paren].trim();
    if name.is_empty() || name.contains(' ') {
        return None;
    }
    let version = body[paren + 1..]
        .trim_end_matches(')')
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_end_matches(',')
        .to_string();
    if version.is_empty() {
        return None;
    }
    let provenance = section.provenance();
    let relation = if direct.contains(&name.to_lowercase()) {
        DependencyRelation::Direct
    } else {
        DependencyRelation::Transitive
    };
    Some(DependencyFinding {
        ecosystem: PackageEcosystem::Rubygems,
        package: name.to_string(),
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
        relation: Some(relation),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCK: &str = "GEM\n  remote: https://rubygems.org/\n  specs:\n    rails (7.1.0)\n      activesupport (= 7.1.0)\n    activesupport (7.1.0)\n      base64\n      benchmark (>= 0.3)\n\nGIT\n  remote: https://example.com/foo.git\n  revision: abc123\n  branch: main\n  specs:\n    foo (0.2.0)\n\nPATH\n  remote: vendor/bar\n  specs:\n    bar (0.1.0)\n\nPLATFORMS\n  ruby\n\nDEPENDENCIES\n  rails\n  foo!\n";

    #[test]
    fn child_constraints_never_become_resolved_findings() {
        let findings = parse_gemfile_lock(LOCK, "Gemfile.lock");
        assert!(findings.iter().all(|f| f.has_resolved_evidence()));
        assert!(findings.iter().all(|f| f.package != "benchmark"));
        assert!(findings.iter().all(|f| f.package != "base64"));
        let support = findings
            .iter()
            .find(|f| f.package == "activesupport")
            .unwrap();
        assert_eq!(support.exact_version(), Some("7.1.0"));
    }

    #[test]
    fn direct_marking_and_source_provenance() {
        let findings = parse_gemfile_lock(LOCK, "Gemfile.lock");
        assert_eq!(findings.len(), 4);
        let rails = findings.iter().find(|f| f.package == "rails").unwrap();
        assert_eq!(rails.relation, Some(DependencyRelation::Direct));
        assert!(rails
            .provenance
            .as_deref()
            .unwrap_or("")
            .contains("rubygems.org"));
        let foo = findings.iter().find(|f| f.package == "foo").unwrap();
        assert_eq!(foo.relation, Some(DependencyRelation::Direct));
        let provenance = foo.provenance.as_deref().unwrap_or("");
        assert!(provenance.contains("example.com"));
        assert!(provenance.contains("abc123"));
        let bar = findings.iter().find(|f| f.package == "bar").unwrap();
        assert_eq!(bar.relation, Some(DependencyRelation::Transitive));
        assert!(bar
            .provenance
            .as_deref()
            .unwrap_or("")
            .contains("vendor/bar"));
        let transitive = findings
            .iter()
            .find(|f| f.package == "activesupport")
            .unwrap();
        assert_eq!(transitive.relation, Some(DependencyRelation::Transitive));
    }

    #[test]
    fn same_package_from_two_sources_kept_separate() {
        let content = "GEM\n  remote: https://rubygems.org/\n  specs:\n    dup (1.0.0)\n\nGIT\n  remote: https://example.com/dup.git\n  revision: r1\n  specs:\n    dup (1.0.0)\n";
        let findings = parse_gemfile_lock(content, "Gemfile.lock");
        assert_eq!(findings.len(), 2);
        assert!(findings.iter().any(|f| f
            .provenance
            .as_deref()
            .unwrap_or("")
            .contains("rubygems")));
        assert!(findings.iter().any(|f| f
            .provenance
            .as_deref()
            .unwrap_or("")
            .contains("example.com")));
    }

    #[test]
    fn malformed_sections_do_not_panic() {
        assert!(parse_gemfile_lock(":::{{{", "Gemfile.lock").is_empty());
        assert!(parse_gemfile_lock("GEM\n  specs:\n      broken (((\n", "Gemfile.lock").is_empty());
    }
}
