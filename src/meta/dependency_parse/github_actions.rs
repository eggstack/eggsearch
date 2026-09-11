use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyRelation, DependencySource,
};

/// Parse GitHub Actions workflow files for `uses:` entries
pub(crate) fn parse_workflow_yml(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();

        // Match "uses: <value>" or "- uses: <value>"
        let val = trimmed
            .strip_prefix("- ")
            .unwrap_or(trimmed)
            .strip_prefix("uses:")
            .map(|v| v.trim());

        if let Some(val) = val {
            // Format: owner/repo@ref or owner/repo/path@ref
            if let Some(at_idx) = val.rfind('@') {
                let action = val[..at_idx].trim();
                let ref_name = val[at_idx + 1..].trim();

                // Only track actions (owner/repo format)
                if action.contains('/') && !action.starts_with('.') && !action.starts_with('/') {
                    findings.push(DependencyFinding {
                        ecosystem: PackageEcosystem::GithubActions,
                        package: action.to_string(),
                        version: Some(ref_name.to_string()),
                        source_file: Some(path.to_string()),
                        source_line: Some(line_num),
                        source_kind: DependencySource::WorkflowFile,
                        confidence: Some(ApplicabilityConfidence::High),
                        relation: Some(DependencyRelation::Unknown),
                    });
                }
            }
        }
    }

    findings
}
