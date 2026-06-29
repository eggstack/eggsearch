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
}

impl PackageEcosystem {
    /// Parse an ecosystem string, accepting common aliases.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "crates_io" | "crates.io" | "cargo" | "rust" => Some(Self::CratesIo),
            "pypi" | "python" => Some(Self::Pypi),
            "npm" | "javascript" | "node" => Some(Self::Npm),
            _ => None,
        }
    }

    /// Stable snake_case string form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CratesIo => "crates_io",
            Self::Pypi => "pypi",
            Self::Npm => "npm",
        }
    }

    /// OSV ecosystem string for advisory lookups.
    pub fn osv_ecosystem(&self) -> &'static str {
        match self {
            Self::CratesIo => "crates.io",
            Self::Pypi => "PyPI",
            Self::Npm => "npm",
        }
    }

    /// Registry base URL for this ecosystem.
    pub fn registry_base_url(&self) -> &'static str {
        match self {
            Self::CratesIo => "https://crates.io",
            Self::Pypi => "https://pypi.org",
            Self::Npm => "https://www.npmjs.com",
        }
    }

    /// Registry API URL for package metadata.
    pub fn registry_api_url(&self, package: &str) -> String {
        match self {
            Self::CratesIo => format!("https://crates.io/api/v1/crates/{package}"),
            Self::Pypi => format!("https://pypi.org/pypi/{package}/json"),
            Self::Npm => format!("https://registry.npmjs.org/{package}"),
        }
    }
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
    /// The package ecosystem (crates.io, pypi, npm).
    pub ecosystem: PackageEcosystem,
    /// The package name (e.g. "axum", "requests", "express").
    pub name: String,
    /// Optional specific version (e.g. "0.7.0").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Optional version requirement (e.g. ">=0.6, <0.8").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_requirement: Option<String>,
}

impl PackageCoordinate {
    /// Validate the coordinate, returning an error if invalid.
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("package name must not be empty".to_string());
        }
        if self.name.contains(' ') {
            return Err("package name must not contain spaces".to_string());
        }
        Ok(())
    }
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
    /// Registry listing URL (e.g. https://crates.io/crates/axum).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry_url: Option<String>,
    /// Documentation URL (e.g. https://docs.rs/axum/0.7.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docs_url: Option<String>,
    /// Source repository URL (e.g. https://github.com/tokio-rs/axum).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_repository_url: Option<String>,
    /// Homepage URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage_url: Option<String>,
    /// Changelog URL if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changelog_url: Option<String>,
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
        assert_eq!(PackageEcosystem::parse("maven"), None);
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
            version: None,
            version_requirement: None,
        };
        assert!(coord.validate().is_ok());
    }

    #[test]
    fn serde_roundtrip_coordinate() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::CratesIo,
            name: "axum".to_string(),
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
}
