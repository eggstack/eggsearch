use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyRelation, DependencySource,
};

/// Parse Gemfile.lock
pub(crate) fn parse_gemfile_lock(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut in_specs = false;
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();

        if trimmed == "specs:" || trimmed == "DEPENDENCIES" {
            in_specs = true;
            continue;
        }

        if trimmed.is_empty()
            || (trimmed.starts_with(|c: char| c.is_uppercase()) && trimmed.ends_with(':'))
        {
            in_specs = false;
            continue;
        }

        if in_specs {
            // Lines like: "    activesupport (7.1.0)"
            // Use the raw line to detect the 4-space indent
            if let Some(name_ver) = line.strip_prefix("    ") {
                let name_ver = name_ver.trim();
                if let Some(paren_start) = name_ver.find('(') {
                    let name = name_ver[..paren_start].trim();
                    let version = name_ver[paren_start + 1..].trim_end_matches(')').trim();
                    if !name.is_empty() {
                        findings.push(DependencyFinding {
                            ecosystem: PackageEcosystem::Rubygems,
                            package: name.to_string(),
                            version: Some(version.to_string()),
                            source_file: Some(path.to_string()),
                            source_line: Some(line_num),
                            source_kind: DependencySource::LockFile,
                            confidence: Some(ApplicabilityConfidence::High),
                            relation: Some(DependencyRelation::Transitive),
                        });
                    }
                }
            }
        }
    }

    findings
}
