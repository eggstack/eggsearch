use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyRelation, DependencySource,
};

/// Parse Dockerfile and docker-compose for image references
pub(crate) fn parse_dockerfile(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();

        // FROM instruction: FROM image:tag AS name
        if trimmed.starts_with("FROM ") || trimmed.starts_with("from ") {
            let rest = &trimmed[5..];
            let image = rest.split_whitespace().next().unwrap_or("");
            if let Some((name, tag)) = image.rsplit_once(':') {
                if !name.is_empty() && !tag.is_empty() && tag != "latest" {
                    findings.push(DependencyFinding {
                        ecosystem: PackageEcosystem::Oci,
                        package: name.to_string(),
                        version: Some(tag.to_string()),
                        source_file: Some(path.to_string()),
                        source_line: Some(line_num),
                        source_kind: DependencySource::LockFile,
                        confidence: Some(ApplicabilityConfidence::Medium),
                        relation: Some(DependencyRelation::Transitive),
                    });
                }
            }
        }

        // docker-compose image: "image: owner/name:tag"
        if let Some(val) = trimmed.strip_prefix("image:") {
            let image = val.trim().trim_matches('"').trim_matches('\'');
            if let Some((name, tag)) = image.rsplit_once(':') {
                if !name.is_empty() && !tag.is_empty() && tag != "latest" {
                    findings.push(DependencyFinding {
                        ecosystem: PackageEcosystem::Oci,
                        package: name.to_string(),
                        version: Some(tag.to_string()),
                        source_file: Some(path.to_string()),
                        source_line: Some(line_num),
                        source_kind: DependencySource::LockFile,
                        confidence: Some(ApplicabilityConfidence::Medium),
                        relation: Some(DependencyRelation::Transitive),
                    });
                }
            }
        }
    }

    findings
}
