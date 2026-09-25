use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyParseReport, DependencyRelation,
    DependencySource,
};

pub(crate) fn parse_csproj(content: &str, path: &str) -> DependencyParseReport {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);
    reader.config_mut().check_end_names = true;
    let mut buf = Vec::new();
    let mut findings = Vec::new();
    let mut group_conditions: Vec<Option<String>> = Vec::new();
    let mut frameworks: Vec<String> = Vec::new();
    let mut pending: Option<PendingReference> = None;
    let mut in_version = false;
    let mut version_text = String::new();
    let mut in_framework = false;
    let mut framework_text = String::new();
    let mut depth = 0usize;
    let mut malformed = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => {
                if depth > 0 {
                    malformed = true;
                }
                break;
            }
            Ok(Event::Start(element)) => {
                depth += 1;
                let name = local_name(&element.name());
                if name == "PackageReference" {
                    if let Some(reference) = pending_reference(&reader, &element, content) {
                        pending = Some(reference);
                    }
                } else if name == "Version" && pending.is_some() {
                    in_version = true;
                    version_text.clear();
                } else if name == "TargetFramework" || name == "TargetFrameworks" {
                    in_framework = true;
                    framework_text.clear();
                } else if name == "ItemGroup" {
                    group_conditions.push(condition_attr(&reader, &element));
                }
            }
            Ok(Event::Empty(element)) => {
                let name = local_name(&element.name());
                if name == "PackageReference" {
                    if let Some(reference) = pending_reference(&reader, &element, content) {
                        let context = target_context(
                            &frameworks,
                            &group_conditions,
                            reference.condition.as_deref(),
                        );
                        findings.push(requirement_finding(
                            reference.include,
                            reference.version,
                            context,
                            reference.line,
                            path,
                        ));
                    }
                }
            }
            Ok(Event::Text(text)) => {
                if in_version {
                    if let Ok(decoded) = text.decode() {
                        version_text.push_str(&decoded);
                    }
                } else if in_framework {
                    if let Ok(decoded) = text.decode() {
                        framework_text.push_str(&decoded);
                    }
                }
            }
            Ok(Event::End(element)) => {
                depth = depth.saturating_sub(1);
                let name = local_name(&element.name());
                if name == "Version" {
                    in_version = false;
                } else if name == "TargetFramework" || name == "TargetFrameworks" {
                    in_framework = false;
                    let framework = framework_text.trim().to_string();
                    if !framework.is_empty() {
                        frameworks.push(framework);
                    }
                } else if name == "ItemGroup" {
                    group_conditions.pop();
                } else if name == "PackageReference" {
                    if let Some(reference) = pending.take() {
                        let version = reference.version.or_else(|| {
                            let version = version_text.trim().to_string();
                            if version.is_empty() {
                                None
                            } else {
                                Some(version)
                            }
                        });
                        let context = target_context(
                            &frameworks,
                            &group_conditions,
                            reference.condition.as_deref(),
                        );
                        findings.push(requirement_finding(
                            reference.include,
                            version,
                            context,
                            reference.line,
                            path,
                        ));
                    }
                    in_version = false;
                }
            }
            Err(_) => {
                malformed = true;
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    if malformed && findings.is_empty() {
        DependencyParseReport::malformed("csproj XML could not be parsed")
    } else if malformed {
        DependencyParseReport::partial(
            findings,
            vec![crate::core::security_applicability::ParseDiagnostic {
                code: "dependency_parse_partial".to_string(),
                message: "csproj XML truncated or malformed after partial parse".to_string(),
                line: None,
            }],
        )
    } else {
        DependencyParseReport::complete(findings)
    }
}

struct PendingReference {
    include: String,
    version: Option<String>,
    condition: Option<String>,
    line: Option<u32>,
}

fn pending_reference(
    reader: &Reader<&[u8]>,
    element: &quick_xml::events::BytesStart<'_>,
    content: &str,
) -> Option<PendingReference> {
    let mut include: Option<String> = None;
    let mut version: Option<String> = None;
    let mut condition: Option<String> = None;
    for attr in element.attributes().flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
        let Ok(value) = attr.decode_and_unescape_value(reader.decoder()) else {
            continue;
        };
        match key.as_str() {
            "Include" => include = Some(value.into_owned()),
            "Version" => version = Some(value.into_owned()),
            "Condition" => condition = Some(value.into_owned()),
            _ => {}
        }
    }
    let include = include.filter(|include| !include.is_empty())?;
    let line = Some(byte_offset_line(content, reader.buffer_position() as usize));
    Some(PendingReference {
        include,
        version: version.filter(|version| !version.is_empty()),
        condition: condition.filter(|condition| !condition.is_empty()),
        line,
    })
}

fn condition_attr(
    reader: &Reader<&[u8]>,
    element: &quick_xml::events::BytesStart<'_>,
) -> Option<String> {
    for attr in element.attributes().flatten() {
        if attr.key.as_ref() == b"Condition" {
            if let Ok(value) = attr.decode_and_unescape_value(reader.decoder()) {
                let value = value.trim().to_string();
                if !value.is_empty() {
                    return Some(value);
                }
            }
        }
    }
    None
}

fn target_context(
    frameworks: &[String],
    groups: &[Option<String>],
    condition: Option<&str>,
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if !frameworks.is_empty() {
        parts.push(frameworks.join(", "));
    }
    for group in groups.iter().flatten() {
        parts.push(format!("condition: {group}"));
    }
    if let Some(condition) = condition {
        parts.push(format!("condition: {condition}"));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("; "))
    }
}

fn requirement_finding(
    package: String,
    version: Option<String>,
    context: Option<String>,
    line: Option<u32>,
    path: &str,
) -> DependencyFinding {
    let confidence = if version.is_some() {
        ApplicabilityConfidence::Medium
    } else {
        ApplicabilityConfidence::Low
    };
    DependencyFinding {
        ecosystem: PackageEcosystem::Nuget,
        package,
        version: version.clone(),
        resolved_version: None,
        version_requirement: version,
        reference_kind: None,
        reference_value: None,
        provenance: None,
        target_context: context,
        integrity_hash: None,
        source_file: Some(path.to_string()),
        source_line: line,
        source_kind: DependencySource::Manifest,
        confidence: Some(confidence),
        relation: Some(DependencyRelation::Direct),
    }
}

fn local_name(name: &quick_xml::name::QName<'_>) -> String {
    String::from_utf8_lossy(name.local_name().as_ref()).into_owned()
}

fn byte_offset_line(content: &str, offset: usize) -> u32 {
    let end = offset.min(content.len());
    (content[..end].bytes().filter(|byte| *byte == b'\n').count() + 1) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::security_applicability::ParseStatus;

    #[test]
    fn multiline_and_child_version_forms() {
        let content = "<Project Sdk=\"Microsoft.NET.Sdk\">\n  <ItemGroup>\n    <PackageReference\n      Include=\"A\"\n      Version=\"1.0.0\" />\n    <PackageReference Include=\"B\">\n      <Version>2.0.0</Version>\n    </PackageReference>\n  </ItemGroup>\n</Project>\n";
        let report = parse_csproj(content, "A.csproj");
        assert_eq!(report.status, ParseStatus::Complete);
        assert_eq!(report.findings.len(), 2);
        let a = report.findings.iter().find(|f| f.package == "A").unwrap();
        assert_eq!(a.version_requirement.as_deref(), Some("1.0.0"));
        assert_eq!(a.exact_version(), None);
        let b = report.findings.iter().find(|f| f.package == "B").unwrap();
        assert_eq!(b.version_requirement.as_deref(), Some("2.0.0"));
    }

    #[test]
    fn conditions_and_namespaces_preserved() {
        let content = "<Project xmlns=\"http://schemas.microsoft.com/developer/msbuild/2003\">\n  <ItemGroup Condition=\"'$(TargetFramework)' == 'net8.0'\">\n    <PackageReference Include=\"C\" Version=\"3.0.0\" Condition=\"'$(OS)' == 'Windows'\" />\n  </ItemGroup>\n</Project>\n";
        let report = parse_csproj(content, "C.csproj");
        assert_eq!(report.findings.len(), 1);
        let context = report.findings[0].target_context.as_deref().unwrap_or("");
        assert!(context.contains("Windows"));
        assert!(context.contains("net8.0"));
    }

    #[test]
    fn malformed_xml_stays_distinct_from_empty() {
        let malformed = parse_csproj("<Project><PackageReference", "A.csproj");
        assert_eq!(malformed.status, ParseStatus::Malformed);
        let empty = parse_csproj("<Project></Project>", "A.csproj");
        assert_eq!(empty.status, ParseStatus::Complete);
        assert!(empty.findings.is_empty());
    }
}
