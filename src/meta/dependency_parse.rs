use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{ApplicabilityConfidence, DependencyFinding, DependencySource};

/// Parse a dependency file and extract dependency findings.
/// Returns empty vec with no panics for malformed files.
pub fn parse_dependency_file(
    path: &str,
    content: &str,
) -> Vec<DependencyFinding> {
    let filename = path.rsplit('/').next().unwrap_or(path);
    
    match filename {
        "Cargo.lock" => parse_cargo_lock(content, path),
        "Cargo.toml" => parse_cargo_toml(content, path),
        "package-lock.json" => parse_package_lock(content, path),
        "npm-shrinkwrap.json" => parse_package_lock(content, path),
        "go.mod" => parse_go_mod(content, path),
        "requirements.txt" | "requirements.in" => parse_requirements_txt(content, path),
        "Gemfile.lock" => parse_gemfile_lock(content, path),
        "composer.lock" => parse_composer_lock(content, path),
        "pom.xml" => parse_pom_xml(content, path),
        name if name.ends_with(".csproj") => parse_csproj(content, path),
        name if name.ends_with(".yml") || name.ends_with(".yaml") => {
            if path.contains(".github/workflows/") || path.contains(".github\\workflows\\") {
                parse_workflow_yml(content, path)
            } else {
                Vec::new()
            }
        }
        "Dockerfile" | "docker-compose.yml" | "docker-compose.yaml" => {
            parse_dockerfile(content, path)
        }
        _ => Vec::new(),
    }
}

/// Parse Cargo.lock (TOML-based, [package] sections with name/version)
fn parse_cargo_lock(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut in_package = false;
    let mut name = String::new();
    let mut version = String::new();
    let mut line_num = 0u32;
    
    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();
        
        if trimmed == "[[package]]" {
            // Flush previous entry
            if !name.is_empty() {
                findings.push(DependencyFinding {
                    ecosystem: PackageEcosystem::CratesIo,
                    package: name.clone(),
                    version: if version.is_empty() { None } else { Some(version.clone()) },
                    source_file: Some(path.to_string()),
                    source_line: Some(line_num.saturating_sub(2)),
                    source_kind: DependencySource::LockFile,
                    confidence: Some(ApplicabilityConfidence::High),
                });
            }
            in_package = true;
            name.clear();
            version.clear();
        } else if trimmed.starts_with('[') && trimmed != "[[package]]" {
            in_package = false;
        } else if in_package {
            if let Some(val) = trimmed.strip_prefix("name = ") {
                name = val.trim_matches('"').to_string();
            } else if let Some(val) = trimmed.strip_prefix("version = ") {
                version = val.trim_matches('"').to_string();
            }
        }
    }
    
    // Flush last entry
    if !name.is_empty() {
        findings.push(DependencyFinding {
            ecosystem: PackageEcosystem::CratesIo,
            package: name,
            version: if version.is_empty() { None } else { Some(version) },
            source_file: Some(path.to_string()),
            source_line: Some(line_num.saturating_sub(1)),
            source_kind: DependencySource::LockFile,
            confidence: Some(ApplicabilityConfidence::High),
        });
    }
    
    findings
}

/// Parse Cargo.toml for direct [dependencies] and [dev-dependencies]
fn parse_cargo_toml(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut in_deps = false;
    let mut line_num = 0u32;
    
    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();
        
        if trimmed == "[dependencies]" || trimmed == "[dev-dependencies]" 
            || trimmed == "[build-dependencies]" {
            in_deps = true;
        } else if trimmed.starts_with('[') {
            in_deps = false;
        } else if in_deps {
            // Parse: name = "version" or name = { version = "..." }
            if let Some(name) = trimmed.strip_suffix(" = ") {
                let name = name.trim();
                if !name.is_empty() && !name.starts_with('#') {
                    findings.push(DependencyFinding {
                        ecosystem: PackageEcosystem::CratesIo,
                        package: name.to_string(),
                        version: None,
                        source_file: Some(path.to_string()),
                        source_line: Some(line_num),
                        source_kind: DependencySource::Manifest,
                        confidence: Some(ApplicabilityConfidence::Medium),
                    });
                }
            } else if let Some(name) = trimmed.split_once(" = ") {
                let name = name.0.trim();
                // Try to extract inline version
                let rest = &trimmed[name.len()..];
                let version = if let Some(vstart) = rest.find("version") {
                    let after_ver = &rest[vstart..];
                    after_ver.split_once('"').and_then(|(_, v)| v.split_once('"').map(|(v, _)| v)).map(|v| v.to_string())
                } else {
                    rest.trim().strip_prefix('"').and_then(|v| v.strip_suffix('"')).map(|v| v.to_string())
                };
                
                if !name.is_empty() && !name.starts_with('#') {
                    findings.push(DependencyFinding {
                        ecosystem: PackageEcosystem::CratesIo,
                        package: name.to_string(),
                        version,
                        source_file: Some(path.to_string()),
                        source_line: Some(line_num),
                        source_kind: DependencySource::Manifest,
                        confidence: Some(ApplicabilityConfidence::Medium),
                    });
                }
            }
        }
    }
    
    findings
}

/// Parse package-lock.json (npm)
fn parse_package_lock(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    
    let parsed: serde_json::Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(_) => return findings,
    };
    
    // npm lockfile v2+: "packages" key with "" as root
    if let Some(packages) = parsed.get("packages").and_then(|p| p.as_object()) {
        for (key, val) in packages {
            if let Some(name) = val.get("version").and_then(|v| v.as_str()) {
                let pkg_name = if key.is_empty() {
                    parsed.get("name").and_then(|n| n.as_str()).unwrap_or("root")
                } else {
                    // packages key is "node_modules/pkg" or "node_modules/@scope/pkg"
                    key.rsplit_once("node_modules/").map(|(_, n)| n).unwrap_or(key)
                };
                findings.push(DependencyFinding {
                    ecosystem: PackageEcosystem::Npm,
                    package: pkg_name.to_string(),
                    version: Some(name.to_string()),
                    source_file: Some(path.to_string()),
                    source_line: None,
                    source_kind: DependencySource::LockFile,
                    confidence: Some(ApplicabilityConfidence::High),
                });
            }
        }
    }
    // npm lockfile v1: "dependencies" key
    else if let Some(deps) = parsed.get("dependencies").and_then(|d| d.as_object()) {
        for (name, val) in deps {
            if let Some(version) = val.get("version").and_then(|v| v.as_str()) {
                findings.push(DependencyFinding {
                    ecosystem: PackageEcosystem::Npm,
                    package: name.to_string(),
                    version: Some(version.to_string()),
                    source_file: Some(path.to_string()),
                    source_line: None,
                    source_kind: DependencySource::LockFile,
                    confidence: Some(ApplicabilityConfidence::High),
                });
            }
        }
    }
    
    findings
}

/// Parse go.mod for module requirements
fn parse_go_mod(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut in_require = false;
    let mut line_num = 0u32;
    
    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();
        
        if trimmed.starts_with("require (") || trimmed == "require" {
            in_require = true;
            continue;
        }
        
        if trimmed == ")" && in_require {
            in_require = false;
            continue;
        }
        
        if in_require || trimmed.starts_with("require ") {
            let rest = if in_require { trimmed } else { trimmed.trim_start_matches("require ") };
            let rest = rest.trim();
            
            // Skip comments
            if rest.starts_with("//") {
                continue;
            }
            
            // Format: module version [+incompatible]
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() >= 2 {
                let module = parts[0];
                let version = parts[1].trim_start_matches('v');
                
                findings.push(DependencyFinding {
                    ecosystem: PackageEcosystem::Go,
                    package: module.to_string(),
                    version: Some(version.to_string()),
                    source_file: Some(path.to_string()),
                    source_line: Some(line_num),
                    source_kind: DependencySource::LockFile,
                    confidence: Some(ApplicabilityConfidence::High),
                });
            }
        }
    }
    
    findings
}

/// Parse requirements.txt / requirements.in
fn parse_requirements_txt(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut line_num = 0u32;
    
    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();
        
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
            continue;
        }
        
        // Format: package[extras]>=version,==version
        let pkg = trimmed.split(['>', '<', '=', '!', '[', ';'])
            .next()
            .unwrap_or(trimmed)
            .trim();
        
        if pkg.is_empty() || !pkg.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_') {
            continue;
        }
        
        // Try to extract pinned version
        let version = trimmed.find("==").map(|idx| trimmed[idx+2..].split([';', '#']).next().unwrap_or("").trim().to_string())
            .or_else(|| trimmed.find(">=").map(|idx| trimmed[idx+2..].split([',', ';', '#']).next().unwrap_or("").trim().to_string()));
        
        let confidence = if version.is_some() {
            ApplicabilityConfidence::Medium
        } else {
            ApplicabilityConfidence::Low
        };
        
        findings.push(DependencyFinding {
            ecosystem: PackageEcosystem::Pypi,
            package: pkg.to_string(),
            version,
            source_file: Some(path.to_string()),
            source_line: Some(line_num),
            source_kind: DependencySource::Manifest,
            confidence: Some(confidence),
        });
    }
    
    findings
}

/// Parse Gemfile.lock
fn parse_gemfile_lock(content: &str, path: &str) -> Vec<DependencyFinding> {
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
        
        if trimmed.is_empty() || (trimmed.starts_with(|c: char| c.is_uppercase()) && trimmed.ends_with(':')) {
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
                    let version = name_ver[paren_start+1..].trim_end_matches(')').trim();
                    if !name.is_empty() {
                        findings.push(DependencyFinding {
                            ecosystem: PackageEcosystem::Rubygems,
                            package: name.to_string(),
                            version: Some(version.to_string()),
                            source_file: Some(path.to_string()),
                            source_line: Some(line_num),
                            source_kind: DependencySource::LockFile,
                            confidence: Some(ApplicabilityConfidence::High),
                        });
                    }
                }
            }
        }
    }
    
    findings
}

/// Parse composer.lock
fn parse_composer_lock(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    
    let parsed: serde_json::Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(_) => return findings,
    };
    
    // composer.lock has "packages" and "packages-dev" arrays
    for key in &["packages", "packages-dev"] {
        if let Some(packages) = parsed.get(*key).and_then(|p| p.as_array()) {
            for pkg in packages {
                let name = pkg.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let version = pkg.get("version").and_then(|v| v.as_str()).unwrap_or("");
                if !name.is_empty() {
                    findings.push(DependencyFinding {
                        ecosystem: PackageEcosystem::Packagist,
                        package: name.to_string(),
                        version: Some(version.trim_start_matches('v').to_string()),
                        source_file: Some(path.to_string()),
                        source_line: None,
                        source_kind: DependencySource::LockFile,
                        confidence: Some(ApplicabilityConfidence::High),
                    });
                }
            }
        }
    }
    
    findings
}

/// Parse pom.xml (Maven) - best-effort extraction of groupId/artifactId/version
fn parse_pom_xml(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut line_num = 0u32;
    let mut in_deps = false;
    let mut artifact_id = String::new();
    let mut group_id = String::new();
    let mut version = String::new();
    
    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();
        
        if trimmed.contains("<dependencies>") {
            in_deps = true;
        } else if trimmed.contains("</dependencies>") {
            in_deps = false;
            // Flush last
            if !artifact_id.is_empty() {
                findings.push(DependencyFinding {
                    ecosystem: PackageEcosystem::Maven,
                    package: if group_id.is_empty() { artifact_id.clone() } else { format!("{}:{}", group_id, artifact_id) },
                    version: if version.is_empty() { None } else { Some(version.clone()) },
                    source_file: Some(path.to_string()),
                    source_line: Some(line_num),
                    source_kind: DependencySource::Manifest,
                    confidence: Some(ApplicabilityConfidence::Medium),
                });
            }
            artifact_id.clear();
            group_id.clear();
            version.clear();
        } else if in_deps {
            if let Some(val) = extract_xml_tag(trimmed, "groupId") {
                group_id = val;
            } else if let Some(val) = extract_xml_tag(trimmed, "artifactId") {
                artifact_id = val;
            } else if let Some(val) = extract_xml_tag(trimmed, "version") {
                version = val;
            }
            
            // Detect closing </dependency>
            if trimmed.contains("</dependency>") {
                if !artifact_id.is_empty() {
                    findings.push(DependencyFinding {
                        ecosystem: PackageEcosystem::Maven,
                        package: if group_id.is_empty() { artifact_id.clone() } else { format!("{}:{}", group_id, artifact_id) },
                        version: if version.is_empty() { None } else { Some(version.clone()) },
                        source_file: Some(path.to_string()),
                        source_line: Some(line_num),
                        source_kind: DependencySource::Manifest,
                        confidence: Some(ApplicabilityConfidence::Medium),
                    });
                }
                artifact_id.clear();
                group_id.clear();
                version.clear();
            }
        }
    }
    
    findings
}

fn extract_xml_tag(line: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    if let Some(start) = line.find(&open) {
        let rest = &line[start + open.len()..];
        if let Some(end) = rest.find(&close) {
            let val = rest[..end].trim();
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    // Self-closing: <version>${...}</version> or <version>1.0</version>
    None
}

/// Parse .csproj for PackageReference elements
fn parse_csproj(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut line_num = 0u32;
    
    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();
        
        if trimmed.contains("PackageReference") {
            let include = extract_xml_attr(trimmed, "Include");
            let version = extract_xml_attr(trimmed, "Version");
            
            if let Some(name) = include {
                findings.push(DependencyFinding {
                    ecosystem: PackageEcosystem::Nuget,
                    package: name,
                    version,
                    source_file: Some(path.to_string()),
                    source_line: Some(line_num),
                    source_kind: DependencySource::Manifest,
                    confidence: Some(ApplicabilityConfidence::Medium),
                });
            }
        }
    }
    
    findings
}

fn extract_xml_attr(line: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr);
    let start = line.find(&pattern)? + pattern.len();
    let end = line[start..].find('"')? + start;
    let val = line[start..end].trim();
    if val.is_empty() { None } else { Some(val.to_string()) }
}

/// Parse GitHub Actions workflow files for `uses:` entries
fn parse_workflow_yml(content: &str, path: &str) -> Vec<DependencyFinding> {
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
                let ref_name = val[at_idx+1..].trim();
                
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
                    });
                }
            }
        }
    }
    
    findings
}

/// Parse Dockerfile and docker-compose for image references
fn parse_dockerfile(content: &str, path: &str) -> Vec<DependencyFinding> {
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
                    });
                }
            }
        }
    }
    
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    const CARGO_LOCK: &str = r#"
[[package]]
name = "serde"
version = "1.0.193"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "..."

[[package]]
name = "tokio"
version = "1.35.1"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "..."
"#;

    #[test]
    fn parse_cargo_lock_basic() {
        let findings = parse_dependency_file("Cargo.lock", CARGO_LOCK);
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].package, "serde");
        assert_eq!(findings[0].version.as_deref(), Some("1.0.193"));
        assert_eq!(findings[0].ecosystem, PackageEcosystem::CratesIo);
        assert_eq!(findings[0].confidence, Some(ApplicabilityConfidence::High));
    }

    const PACKAGE_LOCK_V2: &str = r#"{
  "name": "my-app",
  "packages": {
    "": {
      "name": "my-app",
      "version": "1.0.0",
      "dependencies": {}
    },
    "node_modules/lodash": {
      "version": "4.17.21"
    },
    "node_modules/@scope/pkg": {
      "version": "2.0.0"
    }
  }
}"#;

    #[test]
    fn parse_package_lock_v2() {
        let findings = parse_dependency_file("package-lock.json", PACKAGE_LOCK_V2);
        assert!(findings.len() >= 2);
        let lodash = findings.iter().find(|f| f.package == "lodash").unwrap();
        assert_eq!(lodash.version.as_deref(), Some("4.17.21"));
        assert_eq!(lodash.ecosystem, PackageEcosystem::Npm);
    }

    const GO_MOD: &str = r#"module example.com/myproject

go 1.21

require (
    github.com/gin-gonic/gin v1.9.1
    github.com/stretchr/testify v1.8.4
    golang.org/x/crypto v0.16.0
)
"#;

    #[test]
    fn parse_go_mod_basic() {
        let findings = parse_dependency_file("go.mod", GO_MOD);
        assert_eq!(findings.len(), 3);
        let gin = findings.iter().find(|f| f.package.contains("gin")).unwrap();
        assert_eq!(gin.version.as_deref(), Some("1.9.1"));
        assert_eq!(gin.ecosystem, PackageEcosystem::Go);
    }

    const REQUIREMENTS_TXT: &str = r#"requests>=2.28.0
flask==2.3.2
django>=4.2,<5.0
pytest
"#;

    #[test]
    fn parse_requirements_txt_basic() {
        let findings = parse_dependency_file("requirements.txt", REQUIREMENTS_TXT);
        assert_eq!(findings.len(), 4);
        let flask = findings.iter().find(|f| f.package == "flask").unwrap();
        assert_eq!(flask.version.as_deref(), Some("2.3.2"));
        assert_eq!(flask.ecosystem, PackageEcosystem::Pypi);
    }

    const GEMFILE_LOCK: &str = r#"
GEM
  remote: https://rubygems.org/
  specs:
    activesupport (7.1.0)
      base64
      benchmark (>= 0.3)
    rails (7.1.0)
      activesupport (= 7.1.0)
"#;

    #[test]
    fn parse_gemfile_lock_basic() {
        let findings = parse_dependency_file("Gemfile.lock", GEMFILE_LOCK);
        assert!(findings.len() >= 2);
        let asp = findings.iter().find(|f| f.package == "activesupport").unwrap();
        assert_eq!(asp.version.as_deref(), Some("7.1.0"));
        assert_eq!(asp.ecosystem, PackageEcosystem::Rubygems);
    }

    const COMPOSER_LOCK: &str = r#"{
    "packages": [
        {
            "name": "laravel/framework",
            "version": "v10.48.4"
        }
    ],
    "packages-dev": []
}"#;

    #[test]
    fn parse_composer_lock_basic() {
        let findings = parse_dependency_file("composer.lock", COMPOSER_LOCK);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].package, "laravel/framework");
        assert_eq!(findings[0].version.as_deref(), Some("10.48.4"));
        assert_eq!(findings[0].ecosystem, PackageEcosystem::Packagist);
    }

    const WORKFLOW_YML: &str = r#"name: CI
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
"#;

    #[test]
    fn parse_workflow_yml() {
        let findings = parse_dependency_file(".github/workflows/ci.yml", WORKFLOW_YML);
        assert_eq!(findings.len(), 2);
        let checkout = findings.iter().find(|f| f.package == "actions/checkout").unwrap();
        assert_eq!(checkout.version.as_deref(), Some("v4"));
        assert_eq!(checkout.ecosystem, PackageEcosystem::GithubActions);
    }

    const DOCKERFILE: &str = r#"FROM node:20-alpine AS builder
RUN npm install
FROM nginx:1.25-alpine
"#;

    #[test]
    fn parse_dockerfile() {
        let findings = parse_dependency_file("Dockerfile", DOCKERFILE);
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].package, "node");
        assert_eq!(findings[0].version.as_deref(), Some("20-alpine"));
        assert_eq!(findings[0].ecosystem, PackageEcosystem::Oci);
    }

    #[test]
    fn malformed_file_no_panic() {
        let findings = parse_dependency_file("Cargo.lock", "not valid toml {{{");
        assert!(findings.is_empty());
    }

    #[test]
    fn empty_file() {
        let findings = parse_dependency_file("Cargo.lock", "");
        assert!(findings.is_empty());
    }

    #[test]
    fn unknown_file_type() {
        let findings = parse_dependency_file("README.md", "some content");
        assert!(findings.is_empty());
    }
}
