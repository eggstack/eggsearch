use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyParseReport, DependencyRelation,
    DependencySource, ParseDiagnostic,
};

pub(crate) fn parse_pom_xml(content: &str, path: &str) -> DependencyParseReport {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);
    reader.config_mut().check_end_names = true;
    let mut buf = Vec::new();
    let mut findings = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut current: Option<PomDependency> = None;
    let mut field: Option<PomField> = None;
    let mut text = String::new();
    let mut exclusion_depth = 0usize;
    let mut malformed = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => {
                if !stack.is_empty() {
                    malformed = true;
                }
                break;
            }
            Ok(Event::Start(element)) => {
                let name = local_name(&element.name());
                stack.push(name.clone());
                if name == "exclusions" || name == "exclusion" {
                    exclusion_depth += 1;
                }
                if name == "dependency" && is_project_dependency(&stack) {
                    if current.is_none() {
                        current = Some(PomDependency::new(line_of(content, &reader)));
                    }
                } else if current.is_some() && exclusion_depth == 0 {
                    field = match name.as_str() {
                        "groupId" => Some(PomField::Group),
                        "artifactId" => Some(PomField::Artifact),
                        "version" => Some(PomField::Version),
                        "scope" => Some(PomField::Scope),
                        "optional" => Some(PomField::Optional),
                        _ => None,
                    };
                    text.clear();
                } else {
                    field = None;
                }
            }
            Ok(Event::Empty(element)) => {
                let name = local_name(&element.name());
                if name == "dependency" && is_empty_project_dependency(&stack) {
                    let dependency = PomDependency::new(line_of(content, &reader));
                    findings.push(dependency.finding(path));
                }
            }
            Ok(Event::Text(content)) => {
                if field.is_some() {
                    if let Ok(decoded) = content.decode() {
                        text.push_str(&decoded);
                    }
                }
            }
            Ok(Event::End(element)) => {
                let name = local_name(&element.name());
                if name == "exclusions" || name == "exclusion" {
                    exclusion_depth = exclusion_depth.saturating_sub(1);
                }
                if name == "dependency" {
                    if let Some(dependency) = current.take() {
                        if is_project_dependency(&stack) {
                            findings.push(dependency.finding(path));
                        }
                    }
                    field = None;
                } else if let Some(dependency) = current.as_mut() {
                    if let Some(active) = field.take() {
                        let value = text.trim().to_string();
                        if !value.is_empty() {
                            match active {
                                PomField::Group => dependency.group = value,
                                PomField::Artifact => dependency.artifact = value,
                                PomField::Version => dependency.version = value,
                                PomField::Scope => dependency.scope = value,
                                PomField::Optional => dependency.optional = value,
                            }
                        }
                        text.clear();
                    }
                }
                stack.pop();
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
        DependencyParseReport::malformed("pom.xml could not be parsed")
    } else if malformed {
        DependencyParseReport::partial(
            findings,
            vec![ParseDiagnostic {
                code: "dependency_parse_partial".to_string(),
                message: "pom.xml truncated or malformed after partial parse".to_string(),
                line: None,
            }],
        )
    } else {
        DependencyParseReport::complete(findings)
    }
}

#[derive(Clone, Copy)]
enum PomField {
    Group,
    Artifact,
    Version,
    Scope,
    Optional,
}

struct PomDependency {
    group: String,
    artifact: String,
    version: String,
    scope: String,
    optional: String,
    line: u32,
}

impl PomDependency {
    fn new(line: u32) -> Self {
        Self {
            group: String::new(),
            artifact: String::new(),
            version: String::new(),
            scope: String::new(),
            optional: String::new(),
            line,
        }
    }

    fn finding(self, path: &str) -> DependencyFinding {
        let package = if self.group.is_empty() {
            self.artifact.clone()
        } else {
            format!("{}:{}", self.group, self.artifact)
        };
        let requirement = if self.version.is_empty() {
            None
        } else {
            Some(self.version.clone())
        };
        let mut context: Vec<String> = Vec::new();
        if !self.scope.is_empty() {
            context.push(format!("scope: {}", self.scope));
        }
        if !self.optional.is_empty() {
            context.push(format!("optional: {}", self.optional));
        }
        let confidence = if requirement.is_some() {
            ApplicabilityConfidence::Medium
        } else {
            ApplicabilityConfidence::Low
        };
        DependencyFinding {
            ecosystem: PackageEcosystem::Maven,
            package,
            version: requirement.clone(),
            resolved_version: None,
            version_requirement: requirement,
            reference_kind: None,
            reference_value: None,
            provenance: None,
            target_context: if context.is_empty() {
                None
            } else {
                Some(context.join("; "))
            },
            integrity_hash: None,
            source_file: Some(path.to_string()),
            source_line: Some(self.line),
            source_kind: DependencySource::Manifest,
            confidence: Some(confidence),
            relation: Some(DependencyRelation::Direct),
        }
    }
}

fn is_project_dependency(stack: &[String]) -> bool {
    let Some(parent) = stack.iter().rev().nth(1) else {
        return false;
    };
    if parent != "dependencies" {
        return false;
    }
    !is_excluded_scope(stack)
}

fn is_empty_project_dependency(stack: &[String]) -> bool {
    if stack.last().is_none_or(|parent| parent != "dependencies") {
        return false;
    }
    !is_excluded_scope(stack)
}

fn is_excluded_scope(stack: &[String]) -> bool {
    stack.iter().any(|element| {
        element == "dependencyManagement"
            || element == "plugins"
            || element == "plugin"
            || element == "exclusions"
            || element == "parent"
            || element == "profiles"
            || element == "profile"
    })
}

fn local_name(name: &quick_xml::name::QName<'_>) -> String {
    String::from_utf8_lossy(name.local_name().as_ref()).into_owned()
}

fn line_of(content: &str, reader: &Reader<&[u8]>) -> u32 {
    let end = (reader.buffer_position() as usize).min(content.len());
    (content[..end].bytes().filter(|byte| *byte == b'\n').count() + 1) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::security_applicability::ParseStatus;

    const POM: &str = r#"<project xmlns="http://maven.apache.org/POM/4.0.0">
  <modelVersion>4.0.0</modelVersion>
  <groupId>com.example</groupId>
  <artifactId>app</artifactId>
  <version>1.0</version>
  <dependencyManagement>
    <dependencies>
      <dependency>
        <groupId>org.managed</groupId>
        <artifactId>managed</artifactId>
        <version>9.9.9</version>
      </dependency>
    </dependencies>
  </dependencyManagement>
  <dependencies>
    <dependency>
      <groupId>org.real</groupId>
      <artifactId>lib</artifactId>
      <version>1.2.3</version>
      <scope>test</scope>
      <optional>true</optional>
    </dependency>
    <dependency>
      <groupId>org.excl</groupId>
      <artifactId>with-exclusion</artifactId>
      <version>${revision}</version>
      <exclusions>
        <exclusion>
          <groupId>org.hidden</groupId>
          <artifactId>hidden</artifactId>
        </exclusion>
      </exclusions>
    </dependency>
  </dependencies>
</project>"#;

    #[test]
    fn management_and_exclusions_never_become_direct_findings() {
        let report = parse_pom_xml(POM, "pom.xml");
        assert_eq!(report.status, ParseStatus::Complete);
        assert_eq!(report.findings.len(), 2);
        assert!(report.findings.iter().all(|f| f.exact_version().is_none()));
        assert!(report
            .findings
            .iter()
            .all(|f| !f.package.contains("managed") && !f.package.contains("hidden")));
        let scoped = report
            .findings
            .iter()
            .find(|f| f.package == "org.real:lib")
            .unwrap();
        let context = scoped.target_context.as_deref().unwrap_or("");
        assert!(context.contains("scope: test"));
        assert!(context.contains("optional: true"));
        let property = report
            .findings
            .iter()
            .find(|f| f.package == "org.excl:with-exclusion")
            .unwrap();
        assert_eq!(property.version_requirement.as_deref(), Some("${revision}"));
    }

    #[test]
    fn malformed_pom_stays_distinct_from_empty() {
        let malformed = parse_pom_xml("<project><dependencies>", "pom.xml");
        assert_eq!(malformed.status, ParseStatus::Malformed);
        let empty = parse_pom_xml("<project></project>", "pom.xml");
        assert_eq!(empty.status, ParseStatus::Complete);
        assert!(empty.findings.is_empty());
    }
}
