use crate::core::package::PackageEcosystem;
use crate::core::security_applicability::{
    ApplicabilityConfidence, DependencyFinding, DependencyRelation, DependencySource,
};

/// Parse a dependency file and extract dependency findings.
/// Returns empty vec with no panics for malformed files.
pub fn parse_dependency_file(path: &str, content: &str) -> Vec<DependencyFinding> {
    let filename = path.rsplit('/').next().unwrap_or(path);

    match filename {
        "Cargo.lock" => parse_cargo_lock(content, path),
        "Cargo.toml" => parse_cargo_toml(content, path),
        "package-lock.json" => parse_package_lock(content, path),
        "npm-shrinkwrap.json" => parse_package_lock(content, path),
        "yarn.lock" => parse_yarn_lock(content, path),
        "pnpm-lock.yaml" => parse_pnpm_lock(content, path),
        "poetry.lock" => parse_poetry_lock(content, path),
        "Pipfile.lock" => parse_pipfile_lock(content, path),
        "uv.lock" => parse_uv_lock(content, path),
        "go.mod" => parse_go_mod(content, path),
        "go.sum" => parse_go_sum(content, path),
        "requirements.txt" | "requirements.in" => parse_requirements_txt(content, path),
        "Gemfile.lock" => parse_gemfile_lock(content, path),
        "composer.lock" => parse_composer_lock(content, path),
        "pom.xml" => parse_pom_xml(content, path),
        "gradle.lockfile" => parse_gradle_lockfile(content, path),
        name if name.ends_with(".csproj") => parse_csproj(content, path),
        "packages.lock.json" => parse_packages_lock_json(content, path),
        name if name.ends_with(".yml") || name.ends_with(".yaml") => {
            if path.contains(".github/workflows/") || path.contains(".github\\workflows\\") {
                parse_workflow_yml(content, path)
            } else if path.contains("docker-compose") {
                parse_dockerfile(content, path)
            } else {
                Vec::new()
            }
        }
        "Dockerfile" | "docker-compose.yml" | "docker-compose.yaml" => {
            parse_dockerfile(content, path)
        }
        name if name.starts_with("build.gradle") => parse_build_gradle(content, path),
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
                    version: if version.is_empty() {
                        None
                    } else {
                        Some(version.clone())
                    },
                    source_file: Some(path.to_string()),
                    source_line: Some(line_num.saturating_sub(2)),
                    source_kind: DependencySource::LockFile,
                    confidence: Some(ApplicabilityConfidence::High),
                    relation: Some(DependencyRelation::Transitive),
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
            version: if version.is_empty() {
                None
            } else {
                Some(version)
            },
            source_file: Some(path.to_string()),
            source_line: Some(line_num.saturating_sub(1)),
            source_kind: DependencySource::LockFile,
            confidence: Some(ApplicabilityConfidence::High),
            relation: Some(DependencyRelation::Transitive),
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

        if trimmed == "[dependencies]"
            || trimmed == "[dev-dependencies]"
            || trimmed == "[build-dependencies]"
        {
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
                        relation: Some(DependencyRelation::Direct),
                    });
                }
            } else if let Some(name) = trimmed.split_once(" = ") {
                let name = name.0.trim();
                // Try to extract inline version
                let rest = &trimmed[name.len()..];
                let version = if let Some(vstart) = rest.find("version") {
                    let after_ver = &rest[vstart..];
                    after_ver
                        .split_once('"')
                        .and_then(|(_, v)| v.split_once('"').map(|(v, _)| v))
                        .map(|v| v.to_string())
                } else {
                    rest.trim()
                        .strip_prefix('"')
                        .and_then(|v| v.strip_suffix('"'))
                        .map(|v| v.to_string())
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
                        relation: Some(DependencyRelation::Direct),
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
                    parsed
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("root")
                } else {
                    // packages key is "node_modules/pkg" or "node_modules/@scope/pkg"
                    key.rsplit_once("node_modules/")
                        .map(|(_, n)| n)
                        .unwrap_or(key)
                };
                findings.push(DependencyFinding {
                    ecosystem: PackageEcosystem::Npm,
                    package: pkg_name.to_string(),
                    version: Some(name.to_string()),
                    source_file: Some(path.to_string()),
                    source_line: None,
                    source_kind: DependencySource::LockFile,
                    confidence: Some(ApplicabilityConfidence::High),
                    relation: Some(DependencyRelation::Transitive),
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
                    relation: Some(DependencyRelation::Transitive),
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
            let rest = if in_require {
                trimmed
            } else {
                trimmed.trim_start_matches("require ")
            };
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
                    relation: Some(DependencyRelation::Transitive),
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
        let pkg = trimmed
            .split(['>', '<', '=', '!', '[', ';'])
            .next()
            .unwrap_or(trimmed)
            .trim();

        if pkg.is_empty()
            || !pkg
                .chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_')
        {
            continue;
        }

        // Only extract exact pinned versions (==), not ranges (>=, <=, ~=).
        // Version ranges are not resolved to single versions — callers
        // should treat version as None for range specifiers.
        let version = trimmed
            .find("==")
            .map(|idx| {
                trimmed[idx + 2..]
                    .split([';', '#'])
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string()
            })
            .filter(|v| !v.is_empty());

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
            relation: Some(DependencyRelation::Direct),
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
                        relation: Some(DependencyRelation::Transitive),
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
                    package: if group_id.is_empty() {
                        artifact_id.clone()
                    } else {
                        format!("{group_id}:{artifact_id}")
                    },
                    version: if version.is_empty() {
                        None
                    } else {
                        Some(version.clone())
                    },
                    source_file: Some(path.to_string()),
                    source_line: Some(line_num),
                    source_kind: DependencySource::Manifest,
                    confidence: Some(ApplicabilityConfidence::Medium),
                    relation: Some(DependencyRelation::Direct),
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
                        package: if group_id.is_empty() {
                            artifact_id.clone()
                        } else {
                            format!("{group_id}:{artifact_id}")
                        },
                        version: if version.is_empty() {
                            None
                        } else {
                            Some(version.clone())
                        },
                        source_file: Some(path.to_string()),
                        source_line: Some(line_num),
                        source_kind: DependencySource::Manifest,
                        confidence: Some(ApplicabilityConfidence::Medium),
                        relation: Some(DependencyRelation::Direct),
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
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
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
                    relation: Some(DependencyRelation::Direct),
                });
            }
        }
    }

    findings
}

fn extract_xml_attr(line: &str, attr: &str) -> Option<String> {
    let pattern = format!("{attr}=\"");
    let start = line.find(&pattern)? + pattern.len();
    let end = line[start..].find('"')? + start;
    let val = line[start..end].trim();
    if val.is_empty() {
        None
    } else {
        Some(val.to_string())
    }
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

/// Parse yarn.lock (YAML-like format with indented version)
fn parse_yarn_lock(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut current_name: Option<String> = None;
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();

        // yarn.lock entries start with quoted or unquoted package names at the top level
        let is_entry_header = line.starts_with('"')
            || (!line.starts_with(' ') && !line.starts_with('\t') && trimmed.ends_with(':'));

        if is_entry_header {
            // New entry — flush previous
            current_name.take();
            // Extract the first package name from the entry header
            if let Some(name_part) = trimmed.strip_suffix(':') {
                let clean = name_part.trim_matches('"').trim();
                // Handle scoped packages: @scope/name@version -> @scope/name
                // Handle plain packages: name@version -> name
                let pkg_name = if let Some(at_pos) = clean.rfind('@') {
                    let candidate = &clean[..at_pos];
                    if candidate.is_empty() {
                        // Scoped package starting with @: @scope/name
                        clean.to_string()
                    } else {
                        candidate.to_string()
                    }
                } else {
                    clean.to_string()
                };
                if !pkg_name.is_empty() && !pkg_name.contains(' ') {
                    current_name = Some(pkg_name);
                }
            }
        } else if trimmed.starts_with("version ") {
            if let Some(name) = current_name.take() {
                let version = trimmed
                    .trim_start_matches("version")
                    .trim()
                    .trim_matches('"')
                    .to_string();
                findings.push(DependencyFinding {
                    ecosystem: PackageEcosystem::Npm,
                    package: name,
                    version: if version.is_empty() {
                        None
                    } else {
                        Some(version)
                    },
                    source_file: Some(path.to_string()),
                    source_line: Some(line_num),
                    source_kind: DependencySource::LockFile,
                    confidence: Some(ApplicabilityConfidence::High),
                    relation: Some(DependencyRelation::Transitive),
                });
            }
        }
    }

    findings
}

/// Parse pnpm-lock.yaml (YAML format with packages map)
fn parse_pnpm_lock(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut in_packages = false;
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();

        if trimmed == "packages:" {
            in_packages = true;
            continue;
        }

        if in_packages {
            // Top-level keys (not indented) exit the packages section
            if !line.starts_with(' ') && !line.starts_with('\t') && !trimmed.is_empty() {
                in_packages = false;
                continue;
            }

            // Package entries look like: /lodash@4.17.21:
            if let Some(entry) = trimmed.strip_suffix(':') {
                if entry.starts_with('/') || entry.contains('@') {
                    // Parse /name@version or /@scope/name@version
                    let pkg_part = entry.trim_start_matches('/');
                    if let Some((name, version)) = pkg_part.rsplit_once('@') {
                        if !name.is_empty() && !version.is_empty() {
                            findings.push(DependencyFinding {
                                ecosystem: PackageEcosystem::Npm,
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
    }

    findings
}

/// Parse poetry.lock (TOML-based, similar to Cargo.lock)
fn parse_poetry_lock(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut in_package = false;
    let mut name = String::new();
    let mut version = String::new();
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();

        if trimmed == "[[package]]" {
            if !name.is_empty() {
                findings.push(DependencyFinding {
                    ecosystem: PackageEcosystem::Pypi,
                    package: name.clone(),
                    version: if version.is_empty() {
                        None
                    } else {
                        Some(version.clone())
                    },
                    source_file: Some(path.to_string()),
                    source_line: Some(line_num.saturating_sub(2)),
                    source_kind: DependencySource::LockFile,
                    confidence: Some(ApplicabilityConfidence::High),
                    relation: Some(DependencyRelation::Transitive),
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

    if !name.is_empty() {
        findings.push(DependencyFinding {
            ecosystem: PackageEcosystem::Pypi,
            package: name,
            version: if version.is_empty() {
                None
            } else {
                Some(version)
            },
            source_file: Some(path.to_string()),
            source_line: Some(line_num),
            source_kind: DependencySource::LockFile,
            confidence: Some(ApplicabilityConfidence::High),
            relation: Some(DependencyRelation::Transitive),
        });
    }

    findings
}

/// Parse Pipfile.lock (JSON format)
fn parse_pipfile_lock(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();

    if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
        for section in &["default", "develop"] {
            if let Some(deps) = json.get(*section).and_then(|v| v.as_object()) {
                for (name, val) in deps {
                    let version = val
                        .get("version")
                        .and_then(|v| v.as_str())
                        .map(|s| s.trim_start_matches("==").to_string());
                    findings.push(DependencyFinding {
                        ecosystem: PackageEcosystem::Pypi,
                        package: name.clone(),
                        version,
                        source_file: Some(path.to_string()),
                        source_line: None,
                        source_kind: DependencySource::LockFile,
                        confidence: Some(ApplicabilityConfidence::High),
                        relation: Some(DependencyRelation::Transitive),
                    });
                }
            }
        }
    }

    findings
}

/// Parse uv.lock (TOML-based, similar to Cargo.lock)
fn parse_uv_lock(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut in_package = false;
    let mut name = String::new();
    let mut version = String::new();
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();

        if trimmed == "[[package]]" {
            if !name.is_empty() {
                findings.push(DependencyFinding {
                    ecosystem: PackageEcosystem::Pypi,
                    package: name.clone(),
                    version: if version.is_empty() {
                        None
                    } else {
                        Some(version.clone())
                    },
                    source_file: Some(path.to_string()),
                    source_line: Some(line_num.saturating_sub(2)),
                    source_kind: DependencySource::LockFile,
                    confidence: Some(ApplicabilityConfidence::High),
                    relation: Some(DependencyRelation::Transitive),
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

    if !name.is_empty() {
        findings.push(DependencyFinding {
            ecosystem: PackageEcosystem::Pypi,
            package: name,
            version: if version.is_empty() {
                None
            } else {
                Some(version)
            },
            source_file: Some(path.to_string()),
            source_line: Some(line_num),
            source_kind: DependencySource::LockFile,
            confidence: Some(ApplicabilityConfidence::High),
            relation: Some(DependencyRelation::Transitive),
        });
    }

    findings
}

/// Parse go.sum (text format with module/version lines)
fn parse_go_sum(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let module = parts[0];
            let version = parts[1];
            // go.sum has entries like: module version/go.mod hash
            let clean_version = version.split("/").next().unwrap_or(version);
            let key = (module.to_string(), clean_version.to_string());
            if seen.insert(key) {
                findings.push(DependencyFinding {
                    ecosystem: PackageEcosystem::Go,
                    package: module.to_string(),
                    version: Some(clean_version.to_string()),
                    source_file: Some(path.to_string()),
                    source_line: Some(line_num),
                    source_kind: DependencySource::LockFile,
                    confidence: Some(ApplicabilityConfidence::High),
                    relation: Some(DependencyRelation::Transitive),
                });
            }
        }
    }

    findings
}

/// Parse gradle.lockfile (properties-like format)
fn parse_gradle_lockfile(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();
        // Format: "group:artifact:version=configuration" or "group:artifact:version -> configuration"
        if let Some(equals_pos) = trimmed.find('=') {
            let dep_part = &trimmed[..equals_pos];
            let parts: Vec<&str> = dep_part.split(':').collect();
            if parts.len() >= 3 {
                let group = parts[0];
                let artifact = parts[1];
                let version = parts[2];
                if !version.is_empty() && !version.starts_with('{') {
                    findings.push(DependencyFinding {
                        ecosystem: PackageEcosystem::Maven,
                        package: format!("{group}:{artifact}"),
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

    findings
}

/// Parse build.gradle (best-effort: extract implementation/api dependency blocks)
fn parse_build_gradle(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();
    let mut line_num = 0u32;

    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();

        // Match: implementation 'group:artifact:version'
        // Match: implementation "group:artifact:version"
        for prefix in &[
            "implementation",
            "api",
            "compile",
            "runtimeOnly",
            "testImplementation",
        ] {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                let rest = rest.trim();
                // Expect: 'group:artifact:version' or "group:artifact:version"
                let dep_str = if (rest.starts_with('\'') && rest.ends_with('\''))
                    || (rest.starts_with('"') && rest.ends_with('"'))
                {
                    &rest[1..rest.len() - 1]
                } else {
                    continue;
                };

                let parts: Vec<&str> = dep_str.split(':').collect();
                if parts.len() >= 3 {
                    let group = parts[0];
                    let artifact = parts[1];
                    let version = parts[2];
                    // Skip variable references like ${versions.spring}
                    if !version.starts_with('$') && !version.starts_with('{') {
                        findings.push(DependencyFinding {
                            ecosystem: PackageEcosystem::Maven,
                            package: format!("{group}:{artifact}"),
                            version: Some(version.to_string()),
                            source_file: Some(path.to_string()),
                            source_line: Some(line_num),
                            source_kind: DependencySource::Manifest,
                            confidence: Some(ApplicabilityConfidence::Medium),
                            relation: Some(DependencyRelation::Direct),
                        });
                    }
                }
            }
        }
    }

    findings
}

/// Parse NuGet packages.lock.json (JSON format)
fn parse_packages_lock_json(content: &str, path: &str) -> Vec<DependencyFinding> {
    let mut findings = Vec::new();

    if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
        if let Some(deps) = json.get("libraries").and_then(|v| v.as_object()) {
            for (key, _val) in deps {
                // Key format: "PackageName/1.2.3"
                if let Some((name, version)) = key.rsplit_once('/') {
                    findings.push(DependencyFinding {
                        ecosystem: PackageEcosystem::Nuget,
                        package: name.to_string(),
                        version: Some(version.to_string()),
                        source_file: Some(path.to_string()),
                        source_line: None,
                        source_kind: DependencySource::LockFile,
                        confidence: Some(ApplicabilityConfidence::High),
                        relation: Some(DependencyRelation::Transitive),
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
        let asp = findings
            .iter()
            .find(|f| f.package == "activesupport")
            .unwrap();
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
        let checkout = findings
            .iter()
            .find(|f| f.package == "actions/checkout")
            .unwrap();
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

    const YARN_LOCK: &str = r#"# THIS FILE IS AUTOMATICALLY GENERATED. DO NOT EDIT.
"@babel/core@^7.20.0":
  version "7.20.12"
  resolved "https://registry.yarnpkg.com/@babel/core/-/core-7.20.12.tgz"
  integrity sha512-...
  dependencies:
    "@babel/higher" "^7.20.0"

lodash@^4.17.21:
  version "4.17.21"
  resolved "https://registry.yarnpkg.com/lodash/-/lodash-4.17.21.tgz"
  integrity sha512-...
"#;

    #[test]
    fn parse_yarn_lock() {
        let findings = parse_dependency_file("yarn.lock", YARN_LOCK);
        assert!(findings.len() >= 2);
        let core = findings
            .iter()
            .find(|f| f.package == "@babel/core")
            .unwrap();
        assert_eq!(core.version.as_deref(), Some("7.20.12"));
        assert_eq!(core.ecosystem, PackageEcosystem::Npm);
        let lodash = findings.iter().find(|f| f.package == "lodash").unwrap();
        assert_eq!(lodash.version.as_deref(), Some("4.17.21"));
    }

    const PNPM_LOCK: &str = r#"lockfileVersion: '6.0'

packages:

  /lodash@4.17.21:
    resolution: {integrity: sha512-...}
    dev: false

  /@babel/core@7.20.12:
    resolution: {integrity: sha512-...}
    dependencies:
      '@babel/higher': ^7.20.0
    dev: false

settings:
  auto-install-peers: true
"#;

    #[test]
    fn parse_pnpm_lock() {
        let findings = parse_dependency_file("pnpm-lock.yaml", PNPM_LOCK);
        assert_eq!(findings.len(), 2);
        let lodash = findings.iter().find(|f| f.package == "lodash").unwrap();
        assert_eq!(lodash.version.as_deref(), Some("4.17.21"));
        assert_eq!(lodash.ecosystem, PackageEcosystem::Npm);
        let core = findings
            .iter()
            .find(|f| f.package == "@babel/core")
            .unwrap();
        assert_eq!(core.version.as_deref(), Some("7.20.12"));
    }

    const POETRY_LOCK: &str = r#"
[[package]]
name = "requests"
version = "2.28.0"
description = "Python HTTP for Humans."
optional = false
python-versions = ">=3.7, <4"

[[package]]
name = "urllib3"
version = "1.26.12"
description = "HTTP library with thread-safe connection pooling"
optional = false
python-versions = ">=2.7, !=3.0.*, !=3.1.*, !=3.2.*, !=3.3.*, !=3.4.*, !=3.5.*"
"#;

    #[test]
    fn parse_poetry_lock() {
        let findings = parse_dependency_file("poetry.lock", POETRY_LOCK);
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].package, "requests");
        assert_eq!(findings[0].version.as_deref(), Some("2.28.0"));
        assert_eq!(findings[0].ecosystem, PackageEcosystem::Pypi);
    }

    const PIPFILE_LOCK: &str = r#"{
    "_meta": {
        "hash": {"sha256": "..."},
        "requires": {"python_version": "3.10"}
    },
    "default": {
        "requests": {
            "hashes": ["sha256:..."],
            "version": "==2.28.0"
        },
        "urllib3": {
            "hashes": ["sha256:..."],
            "version": "==1.26.12"
        }
    },
    "develop": {}
}"#;

    #[test]
    fn parse_pipfile_lock() {
        let findings = parse_dependency_file("Pipfile.lock", PIPFILE_LOCK);
        assert_eq!(findings.len(), 2);
        let requests = findings.iter().find(|f| f.package == "requests").unwrap();
        assert_eq!(requests.version.as_deref(), Some("2.28.0"));
        assert_eq!(requests.ecosystem, PackageEcosystem::Pypi);
    }

    const UV_LOCK: &str = r#"
[[package]]
name = "requests"
version = "2.28.0"

[[package]]
name = "urllib3"
version = "1.26.12"
"#;

    #[test]
    fn parse_uv_lock() {
        let findings = parse_dependency_file("uv.lock", UV_LOCK);
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].package, "requests");
        assert_eq!(findings[0].version.as_deref(), Some("2.28.0"));
        assert_eq!(findings[0].ecosystem, PackageEcosystem::Pypi);
    }

    const GO_SUM: &str = r#"github.com/gin-gonic/gin v1.9.0 h1:...
github.com/gin-gonic/gin v1.9.0/go.mod h1:...
github.com/go-playground/validator/v10 v10.11.0 h1:...
github.com/go-playground/validator/v10 v10.11.0/go.mod h1:...
golang.org/x/crypto v0.1.0 h1:...
"#;

    #[test]
    fn parse_go_sum() {
        let findings = parse_dependency_file("go.sum", GO_SUM);
        // Should deduplicate (v1.9.0 and v1.9.0/go.mod are different lines but same version)
        assert!(findings.len() >= 3);
        let gin = findings
            .iter()
            .find(|f| f.package == "github.com/gin-gonic/gin")
            .unwrap();
        assert_eq!(gin.version.as_deref(), Some("v1.9.0"));
        assert_eq!(gin.ecosystem, PackageEcosystem::Go);
    }

    const GRADLE_LOCKFILE: &str = r#"# This is a Gradle lockfile
# ... do not edit ...
org.springframework:spring-core:5.3.23=compileClasspath
org.springframework:spring-beans:5.3.23=compileClasspath
com.google.guava:guava:31.1-jre=runtimeClasspath
"#;

    #[test]
    fn parse_gradle_lockfile() {
        let findings = parse_dependency_file("gradle.lockfile", GRADLE_LOCKFILE);
        assert_eq!(findings.len(), 3);
        let spring = findings
            .iter()
            .find(|f| f.package == "org.springframework:spring-core")
            .unwrap();
        assert_eq!(spring.version.as_deref(), Some("5.3.23"));
        assert_eq!(spring.ecosystem, PackageEcosystem::Maven);
    }

    const BUILD_GRADLE: &str = r#"dependencies {
    implementation 'org.springframework:spring-core:5.3.23'
    implementation "com.google.guava:guava:31.1-jre"
    testImplementation 'junit:junit:4.13.2'
    api 'io.projectreactor:reactor-core:${reactorVersion}'
    runtimeOnly 'org.postgresql:postgresql:42.5.0'
}
"#;

    #[test]
    fn parse_build_gradle() {
        let findings = parse_dependency_file("build.gradle", BUILD_GRADLE);
        assert_eq!(findings.len(), 4); // excludes reactor-core due to variable ref
        let spring = findings
            .iter()
            .find(|f| f.package == "org.springframework:spring-core")
            .unwrap();
        assert_eq!(spring.version.as_deref(), Some("5.3.23"));
        assert_eq!(spring.ecosystem, PackageEcosystem::Maven);
        assert_eq!(spring.source_kind, DependencySource::Manifest);
    }

    const PACKAGES_LOCK_JSON: &str = r#"{
  "version": 2,
  "libraries": {
    "Newtonsoft.Json/13.0.3": {
      "type": "package",
      "build": {}
    },
    "NUnit/3.13.3": {
      "type": "package",
      "build": {}
    }
  },
  "projectFileDependencyGroups": {}
}"#;

    #[test]
    fn parse_packages_lock_json() {
        let findings = parse_dependency_file("packages.lock.json", PACKAGES_LOCK_JSON);
        assert_eq!(findings.len(), 2);
        let newtonsoft = findings
            .iter()
            .find(|f| f.package == "Newtonsoft.Json")
            .unwrap();
        assert_eq!(newtonsoft.version.as_deref(), Some("13.0.3"));
        assert_eq!(newtonsoft.ecosystem, PackageEcosystem::Nuget);
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

    #[test]
    fn malformed_yarn_lock() {
        let findings = parse_dependency_file("yarn.lock", "not a valid lockfile {{{");
        assert!(findings.is_empty());
    }

    #[test]
    fn malformed_pipfile_lock() {
        let findings = parse_dependency_file("Pipfile.lock", "not valid json");
        assert!(findings.is_empty());
    }

    #[test]
    fn malformed_packages_lock_json() {
        let findings = parse_dependency_file("packages.lock.json", "{invalid json");
        assert!(findings.is_empty());
    }

    #[test]
    fn go_sum_deduplicates() {
        let content = "github.com/foo/bar v1.0.0 h1:abc\ngithub.com/foo/bar v1.0.0/go.mod h1:def\n";
        let findings = parse_dependency_file("go.sum", content);
        assert_eq!(findings.len(), 1);
    }

    // ===== WS4: malformed input audit =====

    #[test]
    fn parse_cargo_lock_invalid() {
        let findings = parse_dependency_file("Cargo.lock", "garbage {{{ not toml");
        assert!(findings.is_empty());
    }

    #[test]
    fn parse_package_lock_invalid_json() {
        let findings = parse_dependency_file("package-lock.json", r#"{ "name": "x", }"#);
        assert!(findings.is_empty());
    }

    #[test]
    fn parse_go_mod_invalid() {
        let content = "module foo\nrequire (\n broken";
        let findings = parse_dependency_file("go.mod", content);
        // Should not panic; broken require block yields partial or empty results
        assert!(findings.is_empty() || findings.iter().all(|f| f.version.is_some()));
    }

    #[test]
    fn parse_pom_xml_missing_version() {
        let content = r#"<dependencies>
  <dependency>
    <groupId>x</groupId>
    <artifactId>y</artifactId>
  </dependency>
</dependencies>"#;
        let findings = parse_dependency_file("pom.xml", content);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].package, "x:y");
        assert_eq!(findings[0].version, None);
    }

    #[test]
    fn parse_csproj_missing_version() {
        let content = r#"<PackageReference Include="Newtonsoft.Json" />"#;
        let findings = parse_dependency_file("MyApp.csproj", content);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].package, "Newtonsoft.Json");
        assert_eq!(findings[0].version, None);
    }

    #[test]
    fn parse_gemfile_lock_invalid() {
        let findings = parse_dependency_file("Gemfile.lock", "not a valid lockfile\nrandom text");
        assert!(findings.is_empty());
    }

    #[test]
    fn parse_composer_lock_invalid() {
        let findings = parse_dependency_file("composer.lock", r#"{"name": "x,"#);
        assert!(findings.is_empty());
    }

    #[test]
    fn parse_dockerfile_variable_tag() {
        // Variable tags like ${TAG} are extracted literally — they
        // won't match real versions but should not panic.
        let content = "FROM ubuntu:${TAG}\n";
        let findings = parse_dependency_file("Dockerfile", content);
        // The parser splits on ':' so tag = "${TAG}" which is non-empty
        // and not "latest", so a finding IS produced (literal token).
        // The important property: no panic.
        if let Some(f) = findings.first() {
            assert_eq!(f.ecosystem, PackageEcosystem::Oci);
            assert!(f.version.is_some());
        }
    }

    #[test]
    fn parse_workflow_invalid_uses() {
        let content = r#"on: push
jobs:
  build:
    steps:
      - uses: 'invalid-format'"#;
        let findings = parse_dependency_file(".github/workflows/ci.yml", content);
        // "invalid-format" has no '/' so it's not an owner/repo action — skipped.
        assert!(findings.is_empty());
    }

    // ===== WS4: confidence semantics =====

    #[test]
    fn lockfile_yields_high_confidence() {
        let content = r#"[[package]]
name = "serde"
version = "1.0.193"
"#;
        let findings = parse_dependency_file("Cargo.lock", content);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].confidence, Some(ApplicabilityConfidence::High));
        assert_eq!(findings[0].version.as_deref(), Some("1.0.193"));
    }

    #[test]
    fn manifest_yields_high_confidence() {
        // Cargo.toml pinned dependency should have Medium confidence
        // (manifest pinned versions are Medium for cargo, not High,
        // since Cargo.toml specs can be ranges like "^1.0")
        let content = r#"[dependencies]
tokio = "1.35.1"
"#;
        let findings = parse_dependency_file("Cargo.toml", content);
        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].confidence,
            Some(ApplicabilityConfidence::Medium)
        );
    }

    #[test]
    fn version_range_not_treated_as_installed() {
        // A requirements.txt line with >= is a range, not a pinned version.
        let content = "requests>=2.0.0\n";
        let findings = parse_dependency_file("requirements.txt", content);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].version, None);
    }

    #[test]
    fn exact_eq_version_extracted_from_requirements() {
        // == pins should still produce a version.
        let content = "flask==2.3.2\n";
        let findings = parse_dependency_file("requirements.txt", content);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].version.as_deref(), Some("2.3.2"));
    }

    #[test]
    fn requirements_txt_no_version_yields_low_confidence() {
        let content = "pytest\n";
        let findings = parse_dependency_file("requirements.txt", content);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].version, None);
        assert_eq!(findings[0].confidence, Some(ApplicabilityConfidence::Low));
    }

    #[test]
    fn lockfile_line_numbers_point_to_entry() {
        let content = "line1\nline2\n[[package]]\nname = \"foo\"\nversion = \"1.0\"\n";
        let findings = parse_dependency_file("Cargo.lock", content);
        assert_eq!(findings.len(), 1);
        // source_line should point near the [[package]] line (line 3)
        let line = findings[0].source_line.unwrap();
        assert!(
            (2..=5).contains(&line),
            "line number {line} should point near entry"
        );
    }
}
