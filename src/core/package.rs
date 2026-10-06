//! Package coordinate types and ecosystem resolution for repo_search.
//!
//! This module provides typed package metadata that can enrich
//! repo-oriented searches with registry, docs, source repository,
//! and version context.

use serde::{Deserialize, Serialize};

/// Supported package ecosystems.
///
/// Each variant maps to a specific registry API and URL scheme.
/// Aliases are accepted during parsing (e.g. "cargo" -> `CratesIo`).
#[derive(
    Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PackageEcosystem {
    /// crates.io / docs.rs for Rust crates.
    #[default]
    #[serde(alias = "crates.io", alias = "cargo", alias = "rust")]
    CratesIo,
    /// PyPI for Python packages.
    #[serde(alias = "python")]
    Pypi,
    /// npm for JavaScript/Node packages.
    #[serde(alias = "javascript", alias = "node")]
    Npm,
    /// Go modules (pkg.go.dev).
    #[serde(alias = "go_modules")]
    Go,
    /// Maven/Gradle JVM packages (Central/Sonatype).
    #[serde(alias = "gradle", alias = "jvm")]
    Maven,
    /// NuGet .NET packages.
    #[serde(alias = "dotnet", alias = ".net")]
    Nuget,
    /// RubyGems packages.
    #[serde(alias = "ruby", alias = "gem")]
    Rubygems,
    /// Packagist/Composer PHP packages.
    #[serde(alias = "composer", alias = "php")]
    Packagist,
    /// Docker/OCI container images.
    #[serde(alias = "docker", alias = "container")]
    Oci,
    /// GitHub Actions.
    #[serde(alias = "gh_actions", alias = "actions")]
    GithubActions,
}

impl PackageEcosystem {
    /// Parse an ecosystem string, accepting common aliases.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "crates_io" | "crates.io" | "cargo" | "rust" => Some(Self::CratesIo),
            "pypi" | "python" => Some(Self::Pypi),
            "npm" | "javascript" | "node" => Some(Self::Npm),
            "go" | "go_modules" => Some(Self::Go),
            "maven" | "gradle" | "jvm" => Some(Self::Maven),
            "nuget" | "dotnet" | ".net" => Some(Self::Nuget),
            "rubygems" | "ruby" | "gem" => Some(Self::Rubygems),
            "packagist" | "composer" | "php" => Some(Self::Packagist),
            "oci" | "docker" | "container" => Some(Self::Oci),
            "github_actions" | "gh_actions" | "actions" => Some(Self::GithubActions),
            _ => None,
        }
    }

    /// Stable snake_case string form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CratesIo => "crates_io",
            Self::Pypi => "pypi",
            Self::Npm => "npm",
            Self::Go => "go",
            Self::Maven => "maven",
            Self::Nuget => "nuget",
            Self::Rubygems => "rubygems",
            Self::Packagist => "packagist",
            Self::Oci => "oci",
            Self::GithubActions => "github_actions",
        }
    }

    /// OSV ecosystem string for advisory lookups.
    pub fn osv_ecosystem(&self) -> &'static str {
        match self {
            Self::CratesIo => "crates.io",
            Self::Pypi => "PyPI",
            Self::Npm => "npm",
            Self::Go => "Go",
            Self::Maven => "Maven",
            Self::Nuget => "NuGet",
            Self::Rubygems => "RubyGems",
            Self::Packagist => "Packagist",
            Self::Oci => "",
            Self::GithubActions => "",
        }
    }

    /// Registry base URL for this ecosystem.
    pub fn registry_base_url(&self) -> &'static str {
        match self {
            Self::CratesIo => "https://crates.io",
            Self::Pypi => "https://pypi.org",
            Self::Npm => "https://www.npmjs.com",
            Self::Go => "https://pkg.go.dev",
            Self::Maven => "https://central.sonatype.com",
            Self::Nuget => "https://www.nuget.org",
            Self::Rubygems => "https://rubygems.org",
            Self::Packagist => "https://packagist.org",
            Self::Oci => "https://hub.docker.com",
            Self::GithubActions => "https://github.com",
        }
    }

    /// Registry API URL for package metadata.
    ///
    /// The package name is percent-encoded segment by segment, so a caller-supplied
    /// name can never introduce a path, query, or fragment delimiter into the URL.
    pub fn registry_api_url(&self, package: &str) -> String {
        let path = encode_package_path(package);
        match self {
            Self::CratesIo => format!("https://crates.io/api/v1/crates/{path}"),
            Self::Pypi => format!("https://pypi.org/pypi/{path}/json"),
            Self::Npm => format!("https://registry.npmjs.org/{path}"),
            Self::Go => format!("https://proxy.golang.org/{path}/@latest"),
            Self::Maven => {
                let encoded = urlencoding::encode(package);
                format!("https://search.maven.org/solrsearch/select?q=g%3A%22{encoded}%22&rows=1&wt=json")
            }
            Self::Nuget => format!("https://api.nuget.org/v3-flatcontainer/{path}/index.json"),
            Self::Rubygems => format!("https://rubygems.org/api/v1/gems/{path}.json"),
            Self::Packagist => format!("https://packagist.org/packages/{path}.json"),
            Self::Oci => format!("https://hub.docker.com/v2/repositories/{path}/"),
            Self::GithubActions => format!("https://api.github.com/repos/{path}"),
        }
    }
}

/// Percent-encode a package name for use as a URL path, preserving `/` separators.
///
/// Each `/`-delimited segment is encoded independently so multi-segment names
/// (`github.com/foo/bar`, `monolog/monolog`, `library/nginx`, `actions/checkout`)
/// stay intact. Dot-only segments are re-encoded so they can never act as path
/// traversal after a server-side decode.
fn encode_package_path(package: &str) -> String {
    package
        .split('/')
        .map(|segment| {
            let encoded = urlencoding::encode(segment).into_owned();
            if segment == "." || segment == ".." {
                encoded.replace('.', "%2E")
            } else {
                encoded
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

impl std::fmt::Display for PackageEcosystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Typed package coordinates for package-aware repo searches.
#[derive(
    Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct PackageCoordinate {
    /// The package ecosystem (crates.io, pypi, npm, etc.).
    pub ecosystem: PackageEcosystem,
    /// The package name (e.g. "axum", "requests", "express").
    pub name: String,
    /// Optional namespace (e.g. Maven group_id, OCI registry namespace).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    /// Optional specific version (e.g. "0.7.0").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Optional version requirement (e.g. ">=0.6, <0.8").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_requirement: Option<String>,
}

/// Characters that must never appear in a package name or namespace.
///
/// `/` and `:` are excluded on purpose: several ecosystems use them as
/// meaningful separators (`github.com/foo/bar`, `monolog/monolog`). Everything
/// listed here is either a URL delimiter or has no place in a registry
/// identifier, so rejecting it keeps registry requests unambiguous.
const FORBIDDEN_COORDINATE_CHARS: [char; 9] =
    ['?', '#', '&', '"', '<', '>', '\\', '\u{0}', '\u{7f}'];

impl PackageCoordinate {
    /// Validate the coordinate, returning an error if invalid.
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("package name must not be empty".to_string());
        }
        if self.name.contains(' ') {
            return Err("package name must not contain spaces".to_string());
        }
        if let Some(c) = self
            .name
            .chars()
            .find(|c| FORBIDDEN_COORDINATE_CHARS.contains(c) || c.is_control())
        {
            return Err(format!(
                "package name must not contain URL delimiter or control character {c:?}"
            ));
        }
        if has_dot_only_segment(&self.name) {
            return Err(
                "package name must not contain empty or dot-only path segments".to_string(),
            );
        }
        if let Some(ref ns) = self.namespace {
            if ns.contains("..") || ns.starts_with('/') || ns.ends_with('/') {
                return Err("namespace must not contain path traversal".to_string());
            }
            if let Some(c) = ns
                .chars()
                .find(|c| FORBIDDEN_COORDINATE_CHARS.contains(c) || c.is_control())
            {
                return Err(format!(
                    "namespace must not contain URL delimiter or control character {c:?}"
                ));
            }
            if has_dot_only_segment(ns) {
                return Err(
                    "namespace must not contain empty or dot-only path segments".to_string()
                );
            }
        }
        if self.ecosystem == PackageEcosystem::Maven && self.name.contains(':') {
            return Err(
                "Maven package name must not contain ':'; use the namespace field for group_id"
                    .to_string(),
            );
        }
        if self.ecosystem == PackageEcosystem::Oci
            && (self.name.contains("..") || self.name.contains(' '))
        {
            return Err("OCI image name must not contain path traversal or spaces".to_string());
        }
        Ok(())
    }
}

/// True when a `/`-delimited coordinate contains an empty, `.`, or `..` segment.
fn has_dot_only_segment(value: &str) -> bool {
    value
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
}

/// Resolved package metadata from a registry API lookup.
///
/// Contains URLs, version info, and warnings from the resolution process.
/// Resolution metadata is separate from `SourceCard` — it lives at the
/// response level as advisory context.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PackageResolution {
    /// The original coordinate that was resolved.
    pub coordinate: PackageCoordinate,
    /// Registry listing URL (e.g. <https://crates.io/crates/axum>).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry_url: Option<String>,
    /// Documentation URL (e.g. <https://docs.rs/axum/0.7.0>).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docs_url: Option<String>,
    /// Source repository URL (e.g. <https://github.com/tokio-rs/axum>).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_repository_url: Option<String>,
    /// Homepage URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage_url: Option<String>,
    /// Changelog URL if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changelog_url: Option<String>,
    /// Release notes URL if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_url: Option<String>,
    /// Advisory/security URLs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub advisory_urls: Vec<String>,
    /// License string (e.g. "MIT", "Apache-2.0").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// Latest version known from the registry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_version: Option<String>,
    /// The resolved version (either the requested version or the latest).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_version: Option<String>,
    /// Published timestamp for the resolved version, if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    /// Whether the registry API was successfully queried.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub verified: bool,
    /// Warnings from the resolution process (e.g. fallback URLs, API errors).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// Map between PackageEcosystem and OSV ecosystem strings.
pub fn ecosystem_to_osv(ecosystem: &PackageEcosystem) -> &'static str {
    ecosystem.osv_ecosystem()
}

/// Map between a user-supplied ecosystem string and OSV ecosystem string.
/// Returns None if the ecosystem is not recognized.
pub fn user_ecosystem_to_osv(ecosystem: &str) -> Option<&'static str> {
    PackageEcosystem::parse(ecosystem).map(|e| e.osv_ecosystem())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ecosystem_crates_io_aliases() {
        assert_eq!(
            PackageEcosystem::parse("crates_io"),
            Some(PackageEcosystem::CratesIo)
        );
        assert_eq!(
            PackageEcosystem::parse("crates.io"),
            Some(PackageEcosystem::CratesIo)
        );
        assert_eq!(
            PackageEcosystem::parse("cargo"),
            Some(PackageEcosystem::CratesIo)
        );
        assert_eq!(
            PackageEcosystem::parse("rust"),
            Some(PackageEcosystem::CratesIo)
        );
    }

    #[test]
    fn parse_ecosystem_pypi_aliases() {
        assert_eq!(
            PackageEcosystem::parse("pypi"),
            Some(PackageEcosystem::Pypi)
        );
        assert_eq!(
            PackageEcosystem::parse("python"),
            Some(PackageEcosystem::Pypi)
        );
    }

    #[test]
    fn parse_ecosystem_npm_aliases() {
        assert_eq!(PackageEcosystem::parse("npm"), Some(PackageEcosystem::Npm));
        assert_eq!(
            PackageEcosystem::parse("javascript"),
            Some(PackageEcosystem::Npm)
        );
        assert_eq!(PackageEcosystem::parse("node"), Some(PackageEcosystem::Npm));
    }

    #[test]
    fn parse_ecosystem_unknown() {
        assert_eq!(PackageEcosystem::parse("unknown"), None);
        assert_eq!(PackageEcosystem::parse(""), None);
        assert_eq!(PackageEcosystem::parse("terraform"), None);
    }

    #[test]
    fn parse_ecosystem_case_insensitive() {
        assert_eq!(
            PackageEcosystem::parse("CRATES_IO"),
            Some(PackageEcosystem::CratesIo)
        );
        assert_eq!(
            PackageEcosystem::parse("PyPI"),
            Some(PackageEcosystem::Pypi)
        );
        assert_eq!(PackageEcosystem::parse("NPM"), Some(PackageEcosystem::Npm));
    }

    #[test]
    fn ecosystem_as_str() {
        assert_eq!(PackageEcosystem::CratesIo.as_str(), "crates_io");
        assert_eq!(PackageEcosystem::Pypi.as_str(), "pypi");
        assert_eq!(PackageEcosystem::Npm.as_str(), "npm");
    }

    #[test]
    fn ecosystem_display() {
        assert_eq!(PackageEcosystem::CratesIo.to_string(), "crates_io");
        assert_eq!(PackageEcosystem::Pypi.to_string(), "pypi");
        assert_eq!(PackageEcosystem::Npm.to_string(), "npm");
    }

    #[test]
    fn ecosystem_osv_ecosystem() {
        assert_eq!(PackageEcosystem::CratesIo.osv_ecosystem(), "crates.io");
        assert_eq!(PackageEcosystem::Pypi.osv_ecosystem(), "PyPI");
        assert_eq!(PackageEcosystem::Npm.osv_ecosystem(), "npm");
    }

    #[test]
    fn ecosystem_registry_base_url() {
        assert_eq!(
            PackageEcosystem::CratesIo.registry_base_url(),
            "https://crates.io"
        );
        assert_eq!(
            PackageEcosystem::Pypi.registry_base_url(),
            "https://pypi.org"
        );
        assert_eq!(
            PackageEcosystem::Npm.registry_base_url(),
            "https://www.npmjs.com"
        );
    }

    #[test]
    fn ecosystem_registry_api_url() {
        assert_eq!(
            PackageEcosystem::CratesIo.registry_api_url("axum"),
            "https://crates.io/api/v1/crates/axum"
        );
        assert_eq!(
            PackageEcosystem::Pypi.registry_api_url("requests"),
            "https://pypi.org/pypi/requests/json"
        );
        assert_eq!(
            PackageEcosystem::Npm.registry_api_url("express"),
            "https://registry.npmjs.org/express"
        );
    }

    #[test]
    fn ecosystem_default_is_crates_io() {
        assert_eq!(PackageEcosystem::default(), PackageEcosystem::CratesIo);
    }

    #[test]
    fn validate_rejects_empty_name() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::CratesIo,
            name: "  ".to_string(),
            namespace: None,
            version: None,
            version_requirement: None,
        };
        assert!(coord.validate().is_err());
    }

    #[test]
    fn validate_rejects_name_with_spaces() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Npm,
            name: "my package".to_string(),
            namespace: None,
            version: None,
            version_requirement: None,
        };
        assert!(coord.validate().is_err());
    }

    #[test]
    fn validate_accepts_valid_coordinate() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::CratesIo,
            name: "axum".to_string(),
            namespace: None,
            version: Some("0.7.0".to_string()),
            version_requirement: None,
        };
        assert!(coord.validate().is_ok());
    }

    #[test]
    fn validate_accepts_hyphenated_name() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Npm,
            name: "my-package".to_string(),
            namespace: None,
            version: None,
            version_requirement: None,
        };
        assert!(coord.validate().is_ok());
    }

    #[test]
    fn registry_api_url_percent_encodes_injection_characters() {
        // Maven Solr parameter injection: a quote must not break out of `q`.
        let url = PackageEcosystem::Maven.registry_api_url("x\"&rows=5000&wt=xml");
        assert!(
            !url.contains("wt=xml\"") && !url.contains("&rows=5000"),
            "Maven query must not carry an injected parameter: {url}"
        );
        assert!(
            url.ends_with("&rows=1&wt=json"),
            "caller rows must win: {url}"
        );
        assert!(!url.contains('"'), "raw quote leaked into URL: {url}");

        // Path injection: traversal must not escape the API prefix.
        let url = PackageEcosystem::CratesIo.registry_api_url("../../../../v1/users/me");
        assert!(
            url.starts_with("https://crates.io/api/v1/crates/"),
            "path traversal escaped the crates API prefix: {url}"
        );
        assert!(
            !url.contains(".."),
            "literal traversal segments must not survive: {url}"
        );
        assert!(
            url.contains("%2E%2E"),
            "dot-only segments must be percent-encoded: {url}"
        );
    }

    #[test]
    fn registry_api_url_preserves_multi_segment_names() {
        assert_eq!(
            PackageEcosystem::Go.registry_api_url("github.com/foo/bar"),
            "https://proxy.golang.org/github.com/foo/bar/@latest"
        );
        assert_eq!(
            PackageEcosystem::Packagist.registry_api_url("monolog/monolog"),
            "https://packagist.org/packages/monolog/monolog.json"
        );
        assert_eq!(
            PackageEcosystem::GithubActions.registry_api_url("actions/checkout"),
            "https://api.github.com/repos/actions/checkout"
        );
        // A name that is only dots must not re-form as a traversal segment.
        assert_eq!(
            PackageEcosystem::CratesIo.registry_api_url(".."),
            "https://crates.io/api/v1/crates/%2E%2E"
        );
    }

    #[test]
    fn validate_rejects_url_delimiters_and_traversal() {
        for (ecosystem, name) in [
            (PackageEcosystem::CratesIo, "axum?rows=5"),
            (PackageEcosystem::CratesIo, "axum#frag"),
            (PackageEcosystem::CratesIo, "a&b"),
            (PackageEcosystem::CratesIo, "a\u{0}b"),
            (PackageEcosystem::CratesIo, "a\\b"),
            (PackageEcosystem::CratesIo, "../../v1/users"),
            (PackageEcosystem::CratesIo, "axum/../.."),
            (PackageEcosystem::Go, "/leading/slash"),
            (PackageEcosystem::Packagist, "vendor//pkg"),
            (PackageEcosystem::Packagist, "vendor/./pkg"),
        ] {
            let coord = PackageCoordinate {
                ecosystem: ecosystem.clone(),
                name: name.to_string(),
                ..Default::default()
            };
            assert!(
                coord.validate().is_err(),
                "{ecosystem} name {name:?} should be rejected"
            );
        }
    }

    #[test]
    fn validate_rejects_url_delimiters_in_namespace() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Maven,
            name: "commons-lang3".to_string(),
            namespace: Some("org.apache?x=1".to_string()),
            ..Default::default()
        };
        assert!(coord.validate().is_err());
    }

    #[test]
    fn validate_still_accepts_real_ecosystem_names() {
        for (ecosystem, name) in [
            (PackageEcosystem::Go, "github.com/foo/bar"),
            (PackageEcosystem::Packagist, "monolog/monolog"),
            (PackageEcosystem::Oci, "library/nginx"),
            (PackageEcosystem::GithubActions, "actions/checkout"),
            (PackageEcosystem::Npm, "my-package"),
            (PackageEcosystem::Nuget, "Newtonsoft.Json"),
        ] {
            let coord = PackageCoordinate {
                ecosystem: ecosystem.clone(),
                name: name.to_string(),
                ..Default::default()
            };
            assert!(
                coord.validate().is_ok(),
                "{ecosystem} name {name:?} should be accepted"
            );
        }
    }

    #[test]
    fn serde_roundtrip_coordinate() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::CratesIo,
            name: "axum".to_string(),
            namespace: None,
            version: Some("0.7.0".to_string()),
            version_requirement: None,
        };
        let json = serde_json::to_string(&coord).unwrap();
        let parsed: PackageCoordinate = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, coord);
    }

    #[test]
    fn serde_roundtrip_resolution() {
        let resolution = PackageResolution {
            coordinate: PackageCoordinate {
                ecosystem: PackageEcosystem::CratesIo,
                name: "axum".to_string(),
                namespace: None,
                version: Some("0.7.0".to_string()),
                version_requirement: None,
            },
            registry_url: Some("https://crates.io/crates/axum".to_string()),
            docs_url: Some("https://docs.rs/axum/0.7.0".to_string()),
            source_repository_url: Some("https://github.com/tokio-rs/axum".to_string()),
            verified: true,
            warnings: vec![],
            ..Default::default()
        };
        let json = serde_json::to_string(&resolution).unwrap();
        let parsed: PackageResolution = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.coordinate.name, "axum");
        assert!(parsed.verified);
    }

    #[test]
    fn serde_skips_none_fields() {
        let resolution = PackageResolution {
            coordinate: PackageCoordinate {
                ecosystem: PackageEcosystem::Npm,
                name: "express".to_string(),
                namespace: None,
                version: None,
                version_requirement: None,
            },
            ..Default::default()
        };
        let json = serde_json::to_string(&resolution).unwrap();
        assert!(!json.contains("registry_url"));
        assert!(!json.contains("docs_url"));
        assert!(!json.contains("source_repository_url"));
    }

    #[test]
    fn serde_skips_empty_warnings() {
        let resolution = PackageResolution {
            coordinate: PackageCoordinate {
                ecosystem: PackageEcosystem::Pypi,
                name: "requests".to_string(),
                namespace: None,
                version: None,
                version_requirement: None,
            },
            warnings: vec![],
            ..Default::default()
        };
        let json = serde_json::to_string(&resolution).unwrap();
        assert!(!json.contains("warnings"));
    }

    #[test]
    fn ecosystem_to_osv_mapping() {
        assert_eq!(ecosystem_to_osv(&PackageEcosystem::CratesIo), "crates.io");
        assert_eq!(ecosystem_to_osv(&PackageEcosystem::Pypi), "PyPI");
        assert_eq!(ecosystem_to_osv(&PackageEcosystem::Npm), "npm");
    }

    #[test]
    fn user_ecosystem_to_osv_mapping() {
        assert_eq!(user_ecosystem_to_osv("crates.io"), Some("crates.io"));
        assert_eq!(user_ecosystem_to_osv("pypi"), Some("PyPI"));
        assert_eq!(user_ecosystem_to_osv("npm"), Some("npm"));
        assert_eq!(user_ecosystem_to_osv("unknown"), None);
    }

    #[test]
    fn parse_ecosystem_go_aliases() {
        assert_eq!(PackageEcosystem::parse("go"), Some(PackageEcosystem::Go));
        assert_eq!(
            PackageEcosystem::parse("go_modules"),
            Some(PackageEcosystem::Go)
        );
        assert_eq!(PackageEcosystem::parse("GO"), Some(PackageEcosystem::Go));
    }

    #[test]
    fn parse_ecosystem_maven_aliases() {
        assert_eq!(
            PackageEcosystem::parse("maven"),
            Some(PackageEcosystem::Maven)
        );
        assert_eq!(
            PackageEcosystem::parse("gradle"),
            Some(PackageEcosystem::Maven)
        );
        assert_eq!(
            PackageEcosystem::parse("jvm"),
            Some(PackageEcosystem::Maven)
        );
        assert_eq!(
            PackageEcosystem::parse("MAVEN"),
            Some(PackageEcosystem::Maven)
        );
    }

    #[test]
    fn parse_ecosystem_nuget_aliases() {
        assert_eq!(
            PackageEcosystem::parse("nuget"),
            Some(PackageEcosystem::Nuget)
        );
        assert_eq!(
            PackageEcosystem::parse("dotnet"),
            Some(PackageEcosystem::Nuget)
        );
        assert_eq!(
            PackageEcosystem::parse(".net"),
            Some(PackageEcosystem::Nuget)
        );
        assert_eq!(
            PackageEcosystem::parse("NUGET"),
            Some(PackageEcosystem::Nuget)
        );
    }

    #[test]
    fn parse_ecosystem_rubygems_aliases() {
        assert_eq!(
            PackageEcosystem::parse("rubygems"),
            Some(PackageEcosystem::Rubygems)
        );
        assert_eq!(
            PackageEcosystem::parse("ruby"),
            Some(PackageEcosystem::Rubygems)
        );
        assert_eq!(
            PackageEcosystem::parse("gem"),
            Some(PackageEcosystem::Rubygems)
        );
        assert_eq!(
            PackageEcosystem::parse("RUBYGEMS"),
            Some(PackageEcosystem::Rubygems)
        );
    }

    #[test]
    fn parse_ecosystem_packagist_aliases() {
        assert_eq!(
            PackageEcosystem::parse("packagist"),
            Some(PackageEcosystem::Packagist)
        );
        assert_eq!(
            PackageEcosystem::parse("composer"),
            Some(PackageEcosystem::Packagist)
        );
        assert_eq!(
            PackageEcosystem::parse("php"),
            Some(PackageEcosystem::Packagist)
        );
        assert_eq!(
            PackageEcosystem::parse("PACKAGIST"),
            Some(PackageEcosystem::Packagist)
        );
    }

    #[test]
    fn parse_ecosystem_oci_aliases() {
        assert_eq!(PackageEcosystem::parse("oci"), Some(PackageEcosystem::Oci));
        assert_eq!(
            PackageEcosystem::parse("docker"),
            Some(PackageEcosystem::Oci)
        );
        assert_eq!(
            PackageEcosystem::parse("container"),
            Some(PackageEcosystem::Oci)
        );
        assert_eq!(PackageEcosystem::parse("OCI"), Some(PackageEcosystem::Oci));
    }

    #[test]
    fn parse_ecosystem_github_actions_aliases() {
        assert_eq!(
            PackageEcosystem::parse("github_actions"),
            Some(PackageEcosystem::GithubActions)
        );
        assert_eq!(
            PackageEcosystem::parse("gh_actions"),
            Some(PackageEcosystem::GithubActions)
        );
        assert_eq!(
            PackageEcosystem::parse("actions"),
            Some(PackageEcosystem::GithubActions)
        );
        assert_eq!(
            PackageEcosystem::parse("GITHUB_ACTIONS"),
            Some(PackageEcosystem::GithubActions)
        );
    }

    #[test]
    fn ecosystem_as_str_all_variants() {
        assert_eq!(PackageEcosystem::Go.as_str(), "go");
        assert_eq!(PackageEcosystem::Maven.as_str(), "maven");
        assert_eq!(PackageEcosystem::Nuget.as_str(), "nuget");
        assert_eq!(PackageEcosystem::Rubygems.as_str(), "rubygems");
        assert_eq!(PackageEcosystem::Packagist.as_str(), "packagist");
        assert_eq!(PackageEcosystem::Oci.as_str(), "oci");
        assert_eq!(PackageEcosystem::GithubActions.as_str(), "github_actions");
    }

    #[test]
    fn ecosystem_display_all_variants() {
        assert_eq!(PackageEcosystem::Go.to_string(), "go");
        assert_eq!(PackageEcosystem::Maven.to_string(), "maven");
        assert_eq!(PackageEcosystem::Nuget.to_string(), "nuget");
        assert_eq!(PackageEcosystem::Rubygems.to_string(), "rubygems");
        assert_eq!(PackageEcosystem::Packagist.to_string(), "packagist");
        assert_eq!(PackageEcosystem::Oci.to_string(), "oci");
        assert_eq!(
            PackageEcosystem::GithubActions.to_string(),
            "github_actions"
        );
    }

    #[test]
    fn ecosystem_osv_ecosystem_all_variants() {
        assert_eq!(PackageEcosystem::Go.osv_ecosystem(), "Go");
        assert_eq!(PackageEcosystem::Maven.osv_ecosystem(), "Maven");
        assert_eq!(PackageEcosystem::Nuget.osv_ecosystem(), "NuGet");
        assert_eq!(PackageEcosystem::Rubygems.osv_ecosystem(), "RubyGems");
        assert_eq!(PackageEcosystem::Packagist.osv_ecosystem(), "Packagist");
        assert_eq!(PackageEcosystem::Oci.osv_ecosystem(), "");
        assert_eq!(PackageEcosystem::GithubActions.osv_ecosystem(), "");
    }

    #[test]
    fn ecosystem_registry_base_url_all_variants() {
        assert_eq!(
            PackageEcosystem::Go.registry_base_url(),
            "https://pkg.go.dev"
        );
        assert_eq!(
            PackageEcosystem::Maven.registry_base_url(),
            "https://central.sonatype.com"
        );
        assert_eq!(
            PackageEcosystem::Nuget.registry_base_url(),
            "https://www.nuget.org"
        );
        assert_eq!(
            PackageEcosystem::Rubygems.registry_base_url(),
            "https://rubygems.org"
        );
        assert_eq!(
            PackageEcosystem::Packagist.registry_base_url(),
            "https://packagist.org"
        );
        assert_eq!(
            PackageEcosystem::Oci.registry_base_url(),
            "https://hub.docker.com"
        );
        assert_eq!(
            PackageEcosystem::GithubActions.registry_base_url(),
            "https://github.com"
        );
    }

    #[test]
    fn ecosystem_registry_api_url_all_variants() {
        assert_eq!(
            PackageEcosystem::Go.registry_api_url("github.com/foo/bar"),
            "https://proxy.golang.org/github.com/foo/bar/@latest"
        );
        assert!(PackageEcosystem::Maven
            .registry_api_url("org.apache.commons:commons-lang3")
            .contains("search.maven.org"));
        assert!(PackageEcosystem::Nuget
            .registry_api_url("Newtonsoft.Json")
            .contains("api.nuget.org"));
        assert!(PackageEcosystem::Rubygems
            .registry_api_url("rails")
            .contains("rubygems.org"));
        assert!(PackageEcosystem::Packagist
            .registry_api_url("monolog/monolog")
            .contains("packagist.org"));
        assert!(PackageEcosystem::Oci
            .registry_api_url("library/nginx")
            .contains("hub.docker.com"));
        assert!(PackageEcosystem::GithubActions
            .registry_api_url("actions/checkout")
            .contains("api.github.com"));
    }

    #[test]
    fn validate_rejects_path_traversal_in_namespace() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Maven,
            name: "commons-lang3".to_string(),
            namespace: Some("org/apache/..".to_string()),
            version: None,
            version_requirement: None,
        };
        assert!(coord.validate().is_err());
    }

    #[test]
    fn validate_rejects_leading_slash_in_namespace() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Maven,
            name: "commons-lang3".to_string(),
            namespace: Some("/org.apache".to_string()),
            version: None,
            version_requirement: None,
        };
        assert!(coord.validate().is_err());
    }

    #[test]
    fn validate_rejects_trailing_slash_in_namespace() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Maven,
            name: "commons-lang3".to_string(),
            namespace: Some("org.apache/".to_string()),
            version: None,
            version_requirement: None,
        };
        assert!(coord.validate().is_err());
    }

    #[test]
    fn validate_rejects_colon_in_maven_name() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Maven,
            name: "org.apache:commons-lang3".to_string(),
            namespace: None,
            version: None,
            version_requirement: None,
        };
        assert!(coord.validate().is_err());
    }

    #[test]
    fn validate_accepts_maven_with_separate_namespace() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Maven,
            name: "commons-lang3".to_string(),
            namespace: Some("org.apache".to_string()),
            version: None,
            version_requirement: None,
        };
        assert!(coord.validate().is_ok());
    }

    #[test]
    fn validate_rejects_oci_name_with_dots_traversal() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Oci,
            name: "image..name".to_string(),
            namespace: None,
            version: None,
            version_requirement: None,
        };
        assert!(coord.validate().is_err());
    }

    #[test]
    fn validate_rejects_oci_name_with_spaces() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Oci,
            name: "my image".to_string(),
            namespace: None,
            version: None,
            version_requirement: None,
        };
        assert!(coord.validate().is_err());
    }

    #[test]
    fn validate_accepts_oci_valid_name() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Oci,
            name: "library/nginx".to_string(),
            namespace: None,
            version: None,
            version_requirement: None,
        };
        assert!(coord.validate().is_ok());
    }

    #[test]
    fn validate_accepts_empty_namespace() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Maven,
            name: "commons-lang3".to_string(),
            namespace: None,
            version: None,
            version_requirement: None,
        };
        assert!(coord.validate().is_ok());
    }

    #[test]
    fn serde_roundtrip_coordinate_with_namespace() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Maven,
            name: "commons-lang3".to_string(),
            namespace: Some("org.apache".to_string()),
            version: Some("3.12.0".to_string()),
            version_requirement: None,
        };
        let json = serde_json::to_string(&coord).unwrap();
        let parsed: PackageCoordinate = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, coord);
        assert_eq!(parsed.namespace.as_deref(), Some("org.apache"));
    }

    #[test]
    fn serde_roundtrip_coordinate_without_namespace() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Go,
            name: "github.com/foo/bar".to_string(),
            namespace: None,
            version: Some("v1.2.3".to_string()),
            version_requirement: None,
        };
        let json = serde_json::to_string(&coord).unwrap();
        assert!(!json.contains("namespace"));
        let parsed: PackageCoordinate = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, coord);
    }

    #[test]
    fn serde_roundtrip_resolution_with_new_fields() {
        let resolution = PackageResolution {
            coordinate: PackageCoordinate {
                ecosystem: PackageEcosystem::Nuget,
                name: "Newtonsoft.Json".to_string(),
                namespace: None,
                version: Some("13.0.3".to_string()),
                version_requirement: None,
            },
            registry_url: Some("https://www.nuget.org/packages/Newtonsoft.Json".to_string()),
            docs_url: None,
            source_repository_url: None,
            homepage_url: None,
            changelog_url: None,
            release_url: Some("https://github.com/JamesNK/Newtonsoft.Json/releases".to_string()),
            advisory_urls: vec!["https://github.com/advisories/GHSA-xxxx".to_string()],
            license: Some("MIT".to_string()),
            latest_version: Some("13.0.3".to_string()),
            resolved_version: Some("13.0.3".to_string()),
            published_at: None,
            verified: true,
            warnings: vec![],
        };
        let json = serde_json::to_string(&resolution).unwrap();
        assert!(json.contains("release_url"));
        assert!(json.contains("advisory_urls"));
        let parsed: PackageResolution = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed.release_url.as_deref(),
            Some("https://github.com/JamesNK/Newtonsoft.Json/releases")
        );
        assert_eq!(parsed.advisory_urls.len(), 1);
    }

    #[test]
    fn serde_skips_empty_advisory_urls() {
        let resolution = PackageResolution {
            coordinate: PackageCoordinate {
                ecosystem: PackageEcosystem::Oci,
                name: "library/nginx".to_string(),
                namespace: None,
                version: None,
                version_requirement: None,
            },
            advisory_urls: vec![],
            ..Default::default()
        };
        let json = serde_json::to_string(&resolution).unwrap();
        assert!(!json.contains("advisory_urls"));
    }

    #[test]
    fn serde_skips_none_release_url() {
        let resolution = PackageResolution {
            coordinate: PackageCoordinate {
                ecosystem: PackageEcosystem::GithubActions,
                name: "actions/checkout".to_string(),
                namespace: None,
                version: None,
                version_requirement: None,
            },
            release_url: None,
            ..Default::default()
        };
        let json = serde_json::to_string(&resolution).unwrap();
        assert!(!json.contains("release_url"));
    }

    #[test]
    fn ecosystem_to_osv_new_variants() {
        assert_eq!(ecosystem_to_osv(&PackageEcosystem::Go), "Go");
        assert_eq!(ecosystem_to_osv(&PackageEcosystem::Maven), "Maven");
        assert_eq!(ecosystem_to_osv(&PackageEcosystem::Nuget), "NuGet");
        assert_eq!(ecosystem_to_osv(&PackageEcosystem::Rubygems), "RubyGems");
        assert_eq!(ecosystem_to_osv(&PackageEcosystem::Packagist), "Packagist");
        assert_eq!(ecosystem_to_osv(&PackageEcosystem::Oci), "");
        assert_eq!(ecosystem_to_osv(&PackageEcosystem::GithubActions), "");
    }

    #[test]
    fn user_ecosystem_to_osv_new_variants() {
        assert_eq!(user_ecosystem_to_osv("go"), Some("Go"));
        assert_eq!(user_ecosystem_to_osv("maven"), Some("Maven"));
        assert_eq!(user_ecosystem_to_osv("nuget"), Some("NuGet"));
        assert_eq!(user_ecosystem_to_osv("rubygems"), Some("RubyGems"));
        assert_eq!(user_ecosystem_to_osv("packagist"), Some("Packagist"));
        assert_eq!(user_ecosystem_to_osv("oci"), Some(""));
        assert_eq!(user_ecosystem_to_osv("github_actions"), Some(""));
    }
}
