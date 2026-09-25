use std::collections::HashSet;

use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyReferenceKind, DependencyRelation,
    DependencySource,
};

pub(crate) fn is_dockerfile_name(basename: &str) -> bool {
    let lower = basename.to_lowercase();
    lower == "dockerfile" || lower.starts_with("dockerfile.") || lower.ends_with(".dockerfile")
}

pub(crate) fn is_compose_file(basename: &str) -> bool {
    let lower = basename.to_lowercase();
    matches!(
        lower.as_str(),
        "docker-compose.yml" | "docker-compose.yaml" | "compose.yml" | "compose.yaml"
    ) || ((lower.starts_with("docker-compose.") || lower.starts_with("compose."))
        && (lower.ends_with(".yml") || lower.ends_with(".yaml")))
}

pub(crate) fn parse_dockerfile(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut stages: HashSet<String> = HashSet::new();
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();
        if let Some(rest) = from_instruction(trimmed) {
            let tokens = split_instruction(rest);
            if tokens.is_empty() {
                continue;
            }
            let mut image: Option<&str> = None;
            let mut alias: Option<String> = None;
            let mut index = 0;
            while index < tokens.len() {
                let token = tokens[index];
                if token.starts_with("--") {
                    if !token.contains('=') {
                        index += 1;
                    }
                } else if image.is_none() {
                    image = Some(token);
                } else if token.eq_ignore_ascii_case("AS") {
                    alias = tokens.get(index + 1).map(|alias| alias.to_lowercase());
                    break;
                }
                index += 1;
            }
            if let Some(image) = image {
                if stages.contains(&image.to_lowercase()) {
                    continue;
                }
                if let Some(finding) = parse_image_reference(image, path, Some(line_num)) {
                    findings.push(finding);
                }
            }
            if let Some(alias) = alias {
                stages.insert(alias);
            }
            continue;
        }
        if let Some(value) = compose_image_value(trimmed) {
            if let Some(finding) = parse_image_reference(&value, path, Some(line_num)) {
                findings.push(finding);
            }
        }
    }

    findings
}

fn from_instruction(trimmed: &str) -> Option<&str> {
    let (keyword, rest) = trimmed.split_once(char::is_whitespace)?;
    if keyword.eq_ignore_ascii_case("FROM") {
        Some(rest.trim())
    } else {
        None
    }
}

fn split_instruction(rest: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut current: Option<usize> = None;
    let mut in_single = false;
    let mut in_double = false;
    for (index, byte) in rest.bytes().enumerate() {
        match byte {
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            b' ' | b'\t' if !in_single && !in_double => {
                if let Some(start) = current.take() {
                    tokens.push(&rest[start..index]);
                }
            }
            _ => {
                if current.is_none() {
                    current = Some(index);
                }
            }
        }
    }
    if let Some(start) = current {
        tokens.push(&rest[start..]);
    }
    tokens
}

fn compose_image_value(trimmed: &str) -> Option<String> {
    let value = trimmed.strip_prefix("image:")?.trim();
    if value.is_empty() || value.starts_with('$') {
        return None;
    }
    let value = strip_yaml_comment(value);
    let value = value.trim_matches('"').trim_matches('\'').trim();
    if value.is_empty() || value.contains(char::is_whitespace) {
        return None;
    }
    Some(value.to_string())
}

pub(crate) fn strip_yaml_comment(value: &str) -> String {
    let mut in_single = false;
    let mut in_double = false;
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            b'#' if !in_single
                && !in_double
                && index > 0
                && bytes[index - 1].is_ascii_whitespace() =>
            {
                return value[..index].trim().to_string();
            }
            _ => {}
        }
        index += 1;
    }
    value.trim().to_string()
}

pub(crate) fn parse_image_reference(
    image: &str,
    path: &str,
    line: Option<u32>,
) -> Option<DependencyFinding> {
    let image = image.trim().trim_matches('"').trim_matches('\'').trim();
    if image.is_empty()
        || image.contains(char::is_whitespace)
        || image.ends_with(':')
        || image.ends_with('@')
    {
        return None;
    }
    if image.eq_ignore_ascii_case("scratch") {
        return None;
    }
    if image.contains('$') {
        return Some(DependencyFinding {
            ecosystem: PackageEcosystem::Oci,
            package: image.to_string(),
            version: None,
            resolved_version: None,
            version_requirement: None,
            reference_kind: Some(DependencyReferenceKind::Expression),
            reference_value: Some(image.to_string()),
            provenance: None,
            target_context: None,
            integrity_hash: None,
            source_file: Some(path.to_string()),
            source_line: line,
            source_kind: DependencySource::Dockerfile,
            confidence: Some(ApplicabilityConfidence::Low),
            relation: Some(DependencyRelation::Transitive),
        });
    }
    let (name, reference) = match image.split_once('@') {
        Some((name, digest)) => {
            if name.is_empty() || digest.is_empty() {
                return None;
            }
            let (package, tag) = split_tag(name);
            (
                package.to_string(),
                ImageReference::Digest {
                    tag: tag
                        .filter(|tag| !tag.is_empty() && *tag != "latest")
                        .map(str::to_string),
                    digest: digest.to_string(),
                    raw: image.to_string(),
                },
            )
        }
        None => {
            let (package, tag) = split_tag(image);
            if package.is_empty() {
                return None;
            }
            (
                package.to_string(),
                match tag {
                    Some(tag) if !tag.is_empty() && tag != "latest" => ImageReference::Tag {
                        tag: tag.to_string(),
                        raw: image.to_string(),
                    },
                    _ => ImageReference::Latest {
                        raw: image.to_string(),
                    },
                },
            )
        }
    };
    Some(image_finding(name, reference, path, line))
}

fn split_tag(name: &str) -> (&str, Option<&str>) {
    let slash = name.rfind('/');
    let colon = name.rfind(':');
    match colon {
        Some(position) if slash.is_none_or(|slash| position > slash) => {
            (&name[..position], Some(&name[position + 1..]))
        }
        _ => (name, None),
    }
}

enum ImageReference {
    Tag {
        tag: String,
        raw: String,
    },
    Digest {
        tag: Option<String>,
        digest: String,
        raw: String,
    },
    Latest {
        raw: String,
    },
}

fn image_finding(
    package: String,
    reference: ImageReference,
    path: &str,
    line: Option<u32>,
) -> DependencyFinding {
    let base = DependencyFinding {
        ecosystem: PackageEcosystem::Oci,
        package,
        version: None,
        resolved_version: None,
        version_requirement: None,
        reference_kind: None,
        reference_value: None,
        provenance: None,
        target_context: None,
        integrity_hash: None,
        source_file: Some(path.to_string()),
        source_line: line,
        source_kind: DependencySource::Dockerfile,
        confidence: None,
        relation: Some(DependencyRelation::Transitive),
    };
    match reference {
        ImageReference::Tag { tag, raw } => DependencyFinding {
            version: Some(tag),
            reference_kind: Some(DependencyReferenceKind::Tag),
            reference_value: Some(raw),
            confidence: Some(ApplicabilityConfidence::Medium),
            ..base
        },
        ImageReference::Digest { tag, digest, raw } => DependencyFinding {
            version: tag,
            reference_kind: Some(DependencyReferenceKind::Digest),
            reference_value: Some(raw),
            integrity_hash: Some(digest),
            confidence: Some(ApplicabilityConfidence::High),
            ..base
        },
        ImageReference::Latest { raw } => DependencyFinding {
            version: Some("latest".to_string()),
            reference_kind: Some(DependencyReferenceKind::Tag),
            reference_value: Some(raw),
            confidence: Some(ApplicabilityConfidence::Low),
            ..base
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_options_stages_and_digests() {
        let content = "FROM --platform=$BUILDPLATFORM rust:1.89 AS builder\nRUN make\nFROM builder AS final\nFROM ubuntu@sha256:abc123\nFROM scratch\n";
        let findings = parse_dockerfile(content, "Dockerfile");
        assert_eq!(findings.len(), 2);
        let rust = findings.iter().find(|f| f.package == "rust").unwrap();
        assert_eq!(rust.version.as_deref(), Some("1.89"));
        assert_eq!(rust.source_kind, DependencySource::Dockerfile);
        let ubuntu = findings.iter().find(|f| f.package == "ubuntu").unwrap();
        assert_eq!(ubuntu.reference_kind, Some(DependencyReferenceKind::Digest));
        assert_eq!(ubuntu.integrity_hash.as_deref(), Some("sha256:abc123"));
        assert!(findings
            .iter()
            .all(|f| f.package != "builder" && f.package != "scratch"));
    }

    #[test]
    fn registry_ports_tags_and_compose() {
        let content = "FROM registry.example:5000/name:1.2\nFROM nginx\nimage: \"registry.example:5000/other:2.0@sha256:def\"\nimage: ${IMAGE}\n";
        let findings = parse_dockerfile(content, "docker-compose.yml");
        assert_eq!(findings.len(), 3);
        let port = findings
            .iter()
            .find(|f| f.package == "registry.example:5000/name")
            .unwrap();
        assert_eq!(port.version.as_deref(), Some("1.2"));
        let latest = findings.iter().find(|f| f.package == "nginx").unwrap();
        assert_eq!(latest.version.as_deref(), Some("latest"));
        let both = findings
            .iter()
            .find(|f| f.package == "registry.example:5000/other")
            .unwrap();
        assert_eq!(both.version.as_deref(), Some("2.0"));
        assert_eq!(both.reference_kind, Some(DependencyReferenceKind::Digest));
    }

    #[test]
    fn routing_helpers_stay_bounded() {
        assert!(is_dockerfile_name("Dockerfile"));
        assert!(is_dockerfile_name("Dockerfile.dev"));
        assert!(is_dockerfile_name("dev.Dockerfile"));
        assert!(is_dockerfile_name("Dockerfile.txt"));
        assert!(!is_dockerfile_name("my-dockerfile"));
        assert!(!is_dockerfile_name("dockerfile-backup"));
        assert!(is_compose_file("docker-compose.yml"));
        assert!(is_compose_file("compose.yaml"));
        assert!(is_compose_file("docker-compose.override.yml"));
        assert!(!is_compose_file("my-docker-compose-stuff.yml"));
        assert!(!is_compose_file("docker-compose.txt"));
    }
}
