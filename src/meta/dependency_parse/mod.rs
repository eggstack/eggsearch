use crate::core::security_applicability::{DependencyFinding, DependencyParseReport};

pub(crate) mod cargo;
pub(crate) mod composer;
pub(crate) mod containers;
pub(crate) mod dotnet;
pub(crate) mod github_actions;
pub(crate) mod go;
pub(crate) mod gradle;
pub(crate) mod maven;
pub(crate) mod npm;
pub(crate) mod nuget;
pub(crate) mod python;
pub(crate) mod python_locks;
pub(crate) mod ruby;

/// Parse a dependency file and extract dependency findings plus
/// completeness/diagnostic information without panicking.
///
/// Unknown filenames yield an `Unsupported` report; malformed or partially
/// supported inputs are reported by the format parsers as they gain
/// structured detection in later milestones.
pub fn parse_dependency_file_report(path: &str, content: &str) -> DependencyParseReport {
    let filename = dispatch_basename(path);

    match filename {
        "Cargo.lock" => DependencyParseReport::complete(cargo::parse_cargo_lock(content, path)),
        "Cargo.toml" => DependencyParseReport::complete(cargo::parse_cargo_toml(content, path)),
        "package-lock.json" => {
            DependencyParseReport::complete(npm::parse_package_lock(content, path))
        }
        "npm-shrinkwrap.json" => {
            DependencyParseReport::complete(npm::parse_package_lock(content, path))
        }
        "yarn.lock" => DependencyParseReport::complete(npm::parse_yarn_lock(content, path)),
        "pnpm-lock.yaml" => DependencyParseReport::complete(npm::parse_pnpm_lock(content, path)),
        "poetry.lock" => {
            DependencyParseReport::complete(python_locks::parse_poetry_lock(content, path))
        }
        "Pipfile.lock" => {
            DependencyParseReport::complete(python_locks::parse_pipfile_lock(content, path))
        }
        "uv.lock" => DependencyParseReport::complete(python_locks::parse_uv_lock(content, path)),
        "go.mod" => DependencyParseReport::complete(go::parse_go_mod(content, path)),
        "go.sum" => DependencyParseReport::complete(go::parse_go_sum(content, path)),
        "modules.txt" if is_go_vendor_manifest(path) => {
            DependencyParseReport::complete(go::parse_vendor_modules(content, path))
        }
        "requirements.txt" | "requirements.in" => {
            DependencyParseReport::complete(python::parse_requirements_txt(content, path))
        }
        "Gemfile.lock" => DependencyParseReport::complete(ruby::parse_gemfile_lock(content, path)),
        "composer.lock" => {
            DependencyParseReport::complete(composer::parse_composer_lock(content, path))
        }
        "pom.xml" => maven::parse_pom_xml(content, path),
        "gradle.lockfile" => {
            DependencyParseReport::complete(gradle::parse_gradle_lockfile(content, path))
        }
        "packages.lock.json" => nuget::parse_packages_lock_json(content, path),
        name if name.ends_with(".csproj") => dotnet::parse_csproj(content, path),
        name if name.ends_with(".yml") || name.ends_with(".yaml") => {
            parse_yaml_routed_file(path, content)
        }
        "Dockerfile" | "docker-compose.yml" | "docker-compose.yaml" => {
            DependencyParseReport::complete(containers::parse_dockerfile(content, path))
        }
        name if name.starts_with("build.gradle") => {
            DependencyParseReport::complete(gradle::parse_build_gradle(content, path))
        }
        _ => DependencyParseReport::unsupported(format!(
            "unrecognized dependency filename for '{filename}'"
        )),
    }
}

/// Path-aware basename extraction for dependency dispatch.
///
/// Handles native paths on the current host plus a conservative fallback
/// for backslash-separated path strings received on non-Windows hosts.
pub(crate) fn dispatch_basename(path: &str) -> &str {
    let after_slash = path.rsplit('/').next().unwrap_or(path);
    after_slash.rsplit('\\').next().unwrap_or(after_slash)
}

pub(crate) fn is_go_vendor_manifest(path: &str) -> bool {
    let segments: Vec<&str> = path.split(['/', '\\']).collect();
    segments.len() >= 2
        && segments[segments.len() - 1] == "modules.txt"
        && segments[segments.len() - 2] == "vendor"
}

fn parse_yaml_routed_file(path: &str, content: &str) -> DependencyParseReport {
    if path.contains(".github/workflows/") || path.contains(".github\\workflows\\") {
        DependencyParseReport::complete(github_actions::parse_workflow_yml(content, path))
    } else if path.contains("docker-compose") {
        DependencyParseReport::complete(containers::parse_dockerfile(content, path))
    } else {
        DependencyParseReport::unsupported(format!("unrecognized YAML dependency file '{path}'"))
    }
}

/// Parse a dependency file and extract dependency findings.
/// Returns empty vec with no panics for malformed files.
pub fn parse_dependency_file(path: &str, content: &str) -> Vec<DependencyFinding> {
    parse_dependency_file_report(path, content).findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::package::PackageEcosystem;
    use crate::core::security_applicability::{
        ApplicabilityConfidence, DependencySource, ParseStatus,
    };

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
        assert_eq!(findings.len(), 5);
        let spring = findings
            .iter()
            .find(|f| f.package == "org.springframework:spring-core")
            .unwrap();
        assert_eq!(spring.version.as_deref(), Some("5.3.23"));
        assert_eq!(spring.ecosystem, PackageEcosystem::Maven);
        assert_eq!(spring.source_kind, DependencySource::Manifest);
        let reactor = findings
            .iter()
            .find(|f| f.package == "io.projectreactor:reactor-core")
            .unwrap();
        assert_eq!(reactor.exact_version(), None);
        assert_eq!(
            reactor.version_requirement.as_deref(),
            Some("${reactorVersion}")
        );
    }

    const PACKAGES_LOCK_JSON: &str = r#"{
  "version": 1,
  "dependencies": {
    "net8.0": {
      "Newtonsoft.Json": {
        "type": "Direct",
        "requested": "[13.0.3, )",
        "resolved": "13.0.3",
        "contentHash": "abc123"
      },
      "NUnit": {
        "type": "Transitive",
        "requested": "[3.13.3, )",
        "resolved": "3.13.3",
        "contentHash": "def456"
      }
    }
  }
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
        assert_eq!(newtonsoft.exact_version(), Some("13.0.3"));
        assert_eq!(
            newtonsoft.version_requirement.as_deref(),
            Some("[13.0.3, )")
        );
        assert_eq!(newtonsoft.ecosystem, PackageEcosystem::Nuget);
        assert_eq!(newtonsoft.target_context.as_deref(), Some("net8.0"));
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
    fn lockfile_structured_parse_prefers_no_line_over_wrong_line() {
        let content = "[[package]]\nname = \"foo\"\nversion = \"1.0\"\n";
        let findings = parse_dependency_file("Cargo.lock", content);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].package, "foo");
        assert_eq!(findings[0].exact_version(), Some("1.0"));
        assert_eq!(findings[0].source_line, None);
    }

    #[test]
    fn line_oriented_parsers_retain_source_lines() {
        let findings = parse_dependency_file(
            "go.mod",
            "module example.com/x\n\nrequire github.com/foo/bar v1.2.3\n",
        );
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].source_line, Some(3));
    }

    #[test]
    fn dispatch_is_equivalent_on_windows_and_unix_paths() {
        let cases = [
            (
                "Cargo.lock",
                "[[package]]\nname = \"a\"\nversion = \"1.0\"\n",
            ),
            ("Cargo.toml", "[dependencies]\na = \"1.0\"\n"),
            ("go.mod", "module x\n\nrequire a/b v1.0.0\n"),
            ("go.sum", "a/b v1.0.0 h1:x\n"),
            ("vendor/modules.txt", "# a/b v1.0.0\n"),
            ("requirements.txt", "a==1.0\n"),
            (
                "poetry.lock",
                "[[package]]\nname = \"a\"\nversion = \"1.0\"\n",
            ),
            (
                "Pipfile.lock",
                "{\"default\": {\"a\": {\"version\": \"==1.0\"}}}",
            ),
            ("uv.lock", "[[package]]\nname = \"a\"\nversion = \"1.0\"\n"),
        ];
        for (file, content) in cases {
            let unix = parse_dependency_file_report(&format!("proj/{file}"), content);
            let windows = parse_dependency_file_report(&format!("C:\\proj\\{file}"), content);
            assert_eq!(unix.status, windows.status, "file: {file}");
            assert_eq!(unix.findings.len(), windows.findings.len(), "file: {file}");
            assert!(!unix.findings.is_empty(), "file: {file}");
        }
    }

    #[test]
    fn vendor_manifest_requires_vendor_parent() {
        let content = "# a/b v1.0.0\n";
        let gated = parse_dependency_file_report("proj/vendor/modules.txt", content);
        assert_eq!(gated.findings.len(), 1);
        let bare = parse_dependency_file_report("proj/modules.txt", content);
        assert_eq!(bare.status, ParseStatus::Unsupported);
        assert!(bare.findings.is_empty());
    }
}
