use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyReferenceKind, DependencyRelation,
    DependencySource,
};

pub(crate) fn parse_requirements_txt(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut line_num = 0u32;
    let mut continued = String::new();

    for line in content.lines() {
        line_num += 1;
        let line = line.trim_end();
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if let Some(stripped) = line.strip_suffix('\\') {
            continued.push_str(stripped);
            continued.push(' ');
            continue;
        }
        let full = if continued.is_empty() {
            line.to_string()
        } else {
            continued.push_str(line);
            std::mem::take(&mut continued)
        };
        let full = strip_inline_comment(&full);
        let trimmed = full.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
            continue;
        }
        if let Some(finding) = parse_requirement_line(trimmed, path, line_num) {
            findings.push(finding);
        }
    }

    findings
}

fn strip_inline_comment(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'#' && (i == 0 || bytes[i - 1].is_ascii_whitespace()) {
            return line[..i].to_string();
        }
        i += 1;
    }
    line.to_string()
}

fn parse_requirement_line(line: &str, path: &str, line_num: u32) -> Option<DependencyFinding> {
    let (before_marker, marker) = match line.split_once(';') {
        Some((before, after)) => (before.trim(), Some(after.trim().to_string())),
        None => (line.trim(), None),
    };
    if before_marker.is_empty() {
        return None;
    }
    let (name_part, extras_from_spec, requirement) = match before_marker.split_once('@') {
        Some((name, url)) => {
            let url = url.trim();
            if url.is_empty() {
                return None;
            }
            (name.trim(), None, Some(format!("@ {url}")))
        }
        None => {
            let name_end = before_marker
                .find(|c: char| {
                    c == '>' || c == '<' || c == '=' || c == '!' || c == '~' || c == '[' || c == ' '
                })
                .unwrap_or(before_marker.len());
            let (name, rest) = before_marker.split_at(name_end);
            let rest = rest.trim();
            if rest.starts_with('[') {
                let end = rest.find(']').unwrap_or(rest.len());
                let extras = rest[1..end.min(rest.len())].trim().to_string();
                let after = rest[end.min(rest.len())..].trim_start_matches(']').trim();
                let requirement = if after.is_empty() {
                    None
                } else {
                    Some(after.to_string())
                };
                let extras = if extras.is_empty() {
                    None
                } else {
                    Some(extras)
                };
                (name.trim(), extras, requirement)
            } else {
                let requirement = if rest.is_empty() {
                    None
                } else {
                    Some(rest.to_string())
                };
                (name.trim(), None, requirement)
            }
        }
    };
    let (name, extras) = match name_part.split_once('[') {
        Some((name, rest)) => {
            let inline = rest.trim_end_matches(']').trim();
            let extras = if inline.is_empty() {
                extras_from_spec
            } else {
                Some(inline.to_string())
            };
            (name.trim(), extras)
        }
        None => (name_part, extras_from_spec),
    };
    if name.is_empty()
        || !name
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.')
    {
        return None;
    }
    let is_direct_url = requirement
        .as_deref()
        .is_some_and(|req| req.starts_with('@'));
    let exact_version = match requirement.as_deref() {
        Some(req) if !is_direct_url => extract_exact_pin(req),
        _ => None,
    };
    let mut target_parts: Vec<String> = Vec::new();
    if let Some(extras) = extras {
        if !extras.is_empty() {
            target_parts.push(format!("extras: {extras}"));
        }
    }
    if let Some(marker) = marker {
        if !marker.is_empty() {
            target_parts.push(format!("marker: {marker}"));
        }
    }
    let target_context = if target_parts.is_empty() {
        None
    } else {
        Some(target_parts.join("; "))
    };
    let (reference_kind, reference_value) = match requirement.as_deref() {
        Some(req) if is_direct_url => (
            Some(DependencyReferenceKind::Url),
            Some(req.trim_start_matches('@').trim().to_string()),
        ),
        _ => (None, None),
    };
    let confidence = if requirement.is_some() || reference_value.is_some() {
        ApplicabilityConfidence::Medium
    } else {
        ApplicabilityConfidence::Low
    };
    Some(DependencyFinding {
        ecosystem: PackageEcosystem::Pypi,
        package: name.to_string(),
        version: exact_version.clone(),
        resolved_version: None,
        version_requirement: requirement,
        reference_kind,
        reference_value,
        provenance: None,
        target_context,
        integrity_hash: None,
        source_file: Some(path.to_string()),
        source_line: Some(line_num),
        source_kind: DependencySource::Manifest,
        confidence: Some(confidence),
        relation: Some(DependencyRelation::Direct),
    })
}

fn extract_exact_pin(specifier: &str) -> Option<String> {
    let specifier = specifier.trim();
    let pinned = specifier.strip_prefix("==")?;
    if pinned.starts_with('=') {
        return None;
    }
    let token = pinned.split([',', ' ', '\t']).next().unwrap_or("").trim();
    if token.is_empty() || token.contains('*') || token.contains('!') || specifier.contains(',') {
        return None;
    }
    Some(token.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::package::PackageEcosystem;
    use crate::core::security_applicability::{canonical_package_name, packages_match};

    #[test]
    fn direct_url_reference_is_not_resolved() {
        let content = "mypkg @ https://example.com/mypkg-1.0.tar.gz\n";
        let findings = parse_requirements_txt(content, "requirements.txt");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].package, "mypkg");
        assert!(!findings[0].has_resolved_evidence());
        assert_eq!(
            findings[0].reference_kind,
            Some(DependencyReferenceKind::Url)
        );
        assert!(findings[0]
            .reference_value
            .as_deref()
            .unwrap_or("")
            .contains("example.com"));
    }

    #[test]
    fn extras_and_markers_become_target_context() {
        let content = "requests[security,socks]>=2.28.0; python_version > \"2.7\"\n";
        let findings = parse_requirements_txt(content, "requirements.txt");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].package, "requests");
        assert!(!findings[0].has_resolved_evidence());
        let context = findings[0].target_context.as_deref().unwrap_or("");
        assert!(context.contains("extras: security,socks"));
        assert!(context.contains("python_version"));
    }

    #[test]
    fn wildcard_and_arbitrary_equality_are_not_exact_pins() {
        for line in [
            "pkg==1.2.*\n",
            "pkg===some-token\n",
            "pkg>=1.0,<2.0\n",
            "pkg~=1.4.2\n",
        ] {
            let findings = parse_requirements_txt(line, "requirements.txt");
            assert_eq!(findings.len(), 1, "line: {line}");
            assert_eq!(findings[0].version, None, "line: {line}");
            assert!(!findings[0].has_resolved_evidence(), "line: {line}");
        }
        let findings = parse_requirements_txt("pkg==1.2.3\n", "requirements.txt");
        assert_eq!(findings[0].version.as_deref(), Some("1.2.3"));
        assert_eq!(findings[0].version_requirement.as_deref(), Some("==1.2.3"));
    }

    #[test]
    fn display_name_preserved_while_comparison_canonicalizes() {
        let findings = parse_requirements_txt("My_Package==1.0\n", "requirements.txt");
        assert_eq!(findings[0].package, "My_Package");
        assert_eq!(
            canonical_package_name(&PackageEcosystem::Pypi, &findings[0].package),
            "my-package"
        );
        assert!(packages_match(
            &PackageEcosystem::Pypi,
            &findings[0].package,
            "my-package"
        ));
    }
}
