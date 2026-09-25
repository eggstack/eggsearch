use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyReferenceKind, DependencyRelation,
    DependencySource,
};

pub(crate) fn parse_workflow_yml(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let Some(value) = uses_value(line) else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        if let Some(image) = value.strip_prefix("docker://") {
            let image = image.trim();
            if !image.is_empty() {
                if let Some(finding) =
                    super::containers::parse_image_reference(image, path, Some(line_num))
                {
                    findings.push(finding);
                }
            }
            continue;
        }
        if value.starts_with("./") || value.starts_with("../") || value.starts_with("$/") {
            findings.push(DependencyFinding {
                ecosystem: PackageEcosystem::GithubActions,
                package: value.to_string(),
                version: None,
                resolved_version: None,
                version_requirement: None,
                reference_kind: Some(DependencyReferenceKind::Local),
                reference_value: Some(value.to_string()),
                provenance: Some("local action".to_string()),
                target_context: None,
                integrity_hash: None,
                source_file: Some(path.to_string()),
                source_line: Some(line_num),
                source_kind: DependencySource::WorkflowFile,
                confidence: Some(ApplicabilityConfidence::Low),
                relation: Some(DependencyRelation::Unknown),
            });
            continue;
        }
        let Some((action, reference)) = value.rsplit_once('@') else {
            if is_expression(&value) {
                findings.push(DependencyFinding {
                    ecosystem: PackageEcosystem::GithubActions,
                    package: value.clone(),
                    version: None,
                    resolved_version: None,
                    version_requirement: None,
                    reference_kind: Some(DependencyReferenceKind::Expression),
                    reference_value: Some(value.clone()),
                    provenance: None,
                    target_context: None,
                    integrity_hash: None,
                    source_file: Some(path.to_string()),
                    source_line: Some(line_num),
                    source_kind: DependencySource::WorkflowFile,
                    confidence: Some(ApplicabilityConfidence::Low),
                    relation: Some(DependencyRelation::Unknown),
                });
            }
            continue;
        };
        let action = action.trim().to_string();
        let reference = reference.trim().to_string();
        if action.is_empty() || reference.is_empty() {
            continue;
        }
        if !action.contains('/') || action.starts_with('.') || action.starts_with('/') {
            continue;
        }
        let (kind, confidence) = classify_reference(&reference);
        let provenance = if action.ends_with(".yml") || action.ends_with(".yaml") {
            Some("reusable workflow".to_string())
        } else {
            None
        };
        findings.push(DependencyFinding {
            ecosystem: PackageEcosystem::GithubActions,
            package: action,
            version: Some(reference.clone()),
            resolved_version: None,
            version_requirement: None,
            reference_kind: Some(kind),
            reference_value: Some(reference),
            provenance,
            target_context: None,
            integrity_hash: None,
            source_file: Some(path.to_string()),
            source_line: Some(line_num),
            source_kind: DependencySource::WorkflowFile,
            confidence: Some(confidence),
            relation: Some(DependencyRelation::Unknown),
        });
    }

    findings
}

fn uses_value(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let stripped = trimmed.strip_prefix("- ").unwrap_or(trimmed);
    let value = stripped.strip_prefix("uses:")?.trim();
    if value.is_empty() {
        return None;
    }
    Some(strip_trailing_comment(value))
}

fn strip_trailing_comment(value: &str) -> String {
    let stripped = super::containers::strip_yaml_comment(value);
    stripped
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

fn classify_reference(reference: &str) -> (DependencyReferenceKind, ApplicabilityConfidence) {
    if is_expression(reference) {
        return (
            DependencyReferenceKind::Expression,
            ApplicabilityConfidence::Low,
        );
    }
    if is_full_sha(reference) {
        return (
            DependencyReferenceKind::Commit,
            ApplicabilityConfidence::High,
        );
    }
    if is_tag_like(reference) {
        return (
            DependencyReferenceKind::Tag,
            ApplicabilityConfidence::Medium,
        );
    }
    (
        DependencyReferenceKind::Branch,
        ApplicabilityConfidence::Low,
    )
}

fn is_expression(reference: &str) -> bool {
    reference.contains("${{") || reference.contains('$')
}

fn is_full_sha(reference: &str) -> bool {
    (reference.len() == 40 || reference.len() == 64)
        && reference.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_tag_like(reference: &str) -> bool {
    let candidate = reference.strip_prefix('v').unwrap_or(reference);
    if candidate.is_empty() {
        return false;
    }
    candidate.bytes().all(|b| b.is_ascii_digit())
        || (candidate.contains('.') && candidate.bytes().all(|b| b.is_ascii_digit() || b == b'.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORKFLOW: &str = "jobs:\n  build:\n    steps:\n      - uses: actions/checkout@8f9c05e2e0a2e548c0e4b8b9b9b9b9b9b9b9b9b9\n      - uses: actions/setup-node@v4\n      - uses: actions/deploy@main\n      - uses: owner/repo/path/to/workflow.yml@v1.2.3\n      - uses: ./local/action\n      - uses: $/same/repo/action\n      - uses: docker://example.com/img:1.0\n      - uses: ${{ matrix.action }}\n      - uses: \"actions/quoted@v2\" # pinned\n";

    #[test]
    fn reference_kinds_are_typed() {
        let findings = parse_workflow_yml(WORKFLOW, ".github/workflows/ci.yml");
        let sha = findings
            .iter()
            .find(|f| f.package == "actions/checkout")
            .unwrap();
        assert_eq!(sha.reference_kind, Some(DependencyReferenceKind::Commit));
        assert_eq!(sha.confidence, Some(ApplicabilityConfidence::High));
        assert_eq!(sha.exact_version(), None);
        let tag = findings
            .iter()
            .find(|f| f.package == "actions/setup-node")
            .unwrap();
        assert_eq!(tag.reference_kind, Some(DependencyReferenceKind::Tag));
        let branch = findings
            .iter()
            .find(|f| f.package == "actions/deploy")
            .unwrap();
        assert_eq!(branch.reference_kind, Some(DependencyReferenceKind::Branch));
        let reusable = findings
            .iter()
            .find(|f| f.package == "owner/repo/path/to/workflow.yml")
            .unwrap();
        assert_eq!(reusable.provenance.as_deref(), Some("reusable workflow"));
        let quoted = findings
            .iter()
            .find(|f| f.package == "actions/quoted")
            .unwrap();
        assert_eq!(quoted.reference_value.as_deref(), Some("v2"));
    }

    #[test]
    fn local_docker_and_expression_refs() {
        let findings = parse_workflow_yml(WORKFLOW, ".github/workflows/ci.yml");
        assert!(findings.iter().any(|f| f.package == "./local/action"
            && f.reference_kind == Some(DependencyReferenceKind::Local)));
        assert!(findings.iter().any(|f| f.package == "$/same/repo/action"
            && f.reference_kind == Some(DependencyReferenceKind::Local)));
        let docker = findings
            .iter()
            .find(|f| f.package == "example.com/img")
            .unwrap();
        assert_eq!(docker.ecosystem, PackageEcosystem::Oci);
        let expression = findings
            .iter()
            .find(|f| f.reference_kind == Some(DependencyReferenceKind::Expression))
            .unwrap();
        assert_eq!(expression.exact_version(), None);
        assert!(findings.iter().all(|f| !f.has_resolved_evidence()));
    }
}
