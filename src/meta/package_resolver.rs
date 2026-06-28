//! Package registry resolver: bounded HTTP lookups for crates.io, PyPI, and npm.
//!
//! Resolves package coordinates to registry URLs, documentation URLs,
//! source repository URLs, and version information. Falls back to
//! deterministic URLs when registry APIs fail.

use crate::core::package::{PackageCoordinate, PackageEcosystem, PackageResolution};
use reqwest::Client;
use std::time::Duration;

/// Default timeout for registry API lookups.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// Resolve package metadata from a registry API.
///
/// Makes bounded HTTP calls to the package registry and returns a
/// `PackageResolution` with URLs, version info, and warnings.
/// On failure, returns deterministic fallback URLs with a warning.
pub async fn resolve_package(
    client: &Client,
    coordinate: &PackageCoordinate,
    timeout: Option<Duration>,
) -> PackageResolution {
    let timeout = timeout.unwrap_or(DEFAULT_TIMEOUT);

    match coordinate.ecosystem {
        PackageEcosystem::CratesIo => resolve_crates_io(client, coordinate, timeout).await,
        PackageEcosystem::Pypi => resolve_pypi(client, coordinate, timeout).await,
        PackageEcosystem::Npm => resolve_npm(client, coordinate, timeout).await,
    }
}

/// Resolve a crates.io package.
async fn resolve_crates_io(
    client: &Client,
    coord: &PackageCoordinate,
    timeout: Duration,
) -> PackageResolution {
    let api_url = PackageEcosystem::CratesIo.registry_api_url(&coord.name);

    match client.get(&api_url).timeout(timeout).send().await {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<serde_json::Value>().await {
                Ok(val) => parse_crates_io_response(coord, &val),
                Err(e) => fallback_with_warning(coord, &format!("crates.io JSON parse error: {e}")),
            }
        }
        Ok(resp) => fallback_with_warning(
            coord,
            &format!("crates.io API returned status {}", resp.status()),
        ),
        Err(e) => fallback_with_warning(coord, &format!("crates.io API error: {e}")),
    }
}

/// Parse a crates.io API response into PackageResolution.
fn parse_crates_io_response(coord: &PackageCoordinate, val: &serde_json::Value) -> PackageResolution {
    let krate = val.get("crate").unwrap_or(val);

    let latest_version = krate
        .get("newest_version")
        .or_else(|| krate.get("max_version"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let resolved_version = coord
        .version
        .clone()
        .or_else(|| latest_version.clone());

    let registry_url = Some(format!(
        "https://crates.io/crates/{}",
        coord.name
    ));

    let docs_url = resolved_version.as_ref().map(|v| {
        format!("https://docs.rs/{}/{}", coord.name, v)
    });

    let source_repository_url = krate
        .get("repository")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            // crates.io puts repo in the "links" field sometimes
            krate
                .get("links")
                .and_then(|v| v.get("repository"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        });

    let homepage_url = krate
        .get("homepage")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let license = krate
        .get("license")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    PackageResolution {
        coordinate: coord.clone(),
        registry_url,
        docs_url,
        source_repository_url,
        homepage_url,
        changelog_url: None,
        license,
        latest_version,
        resolved_version,
        published_at: None,
        verified: true,
        warnings: vec![],
    }
}

/// Resolve a PyPI package.
async fn resolve_pypi(
    client: &Client,
    coord: &PackageCoordinate,
    timeout: Duration,
) -> PackageResolution {
    let api_url = PackageEcosystem::Pypi.registry_api_url(&coord.name);

    match client.get(&api_url).timeout(timeout).send().await {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<serde_json::Value>().await {
                Ok(val) => parse_pypi_response(coord, &val),
                Err(e) => fallback_with_warning(coord, &format!("PyPI JSON parse error: {e}")),
            }
        }
        Ok(resp) => fallback_with_warning(
            coord,
            &format!("PyPI API returned status {}", resp.status()),
        ),
        Err(e) => fallback_with_warning(coord, &format!("PyPI API error: {e}")),
    }
}

/// Parse a PyPI API response into PackageResolution.
fn parse_pypi_response(coord: &PackageCoordinate, val: &serde_json::Value) -> PackageResolution {
    let info = val.get("info").unwrap_or(val);

    let latest_version = info
        .get("version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let resolved_version = coord
        .version
        .clone()
        .or_else(|| latest_version.clone());

    let registry_url = Some(format!(
        "https://pypi.org/project/{}/",
        coord.name
    ));

    // PyPI project_urls is a map of label -> url
    let project_urls = info.get("project_urls").and_then(|v| v.as_object());

    let docs_url = project_urls
        .and_then(|urls| {
            urls.get("Documentation")
                .or_else(|| urls.get("Docs"))
                .or_else(|| urls.get("docs"))
                .or_else(|| urls.get("documentation"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        })
        .or_else(|| {
            // Fallback: readthedocs URL pattern
            resolved_version.as_ref().map(|_| {
                format!(
                    "https://{}-readthedocs-io.readthedocs.io/en/stable/",
                    coord.name.replace('_', "-")
                )
            })
        });

    let source_repository_url = project_urls
        .and_then(|urls| {
            urls.get("Source")
                .or_else(|| urls.get("source"))
                .or_else(|| urls.get("Repository"))
                .or_else(|| urls.get("repository"))
                .or_else(|| urls.get("GitHub"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        });

    let homepage_url = info
        .get("home_page")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let changelog_url = project_urls.and_then(|urls| {
        urls.get("Changelog")
            .or_else(|| urls.get("changelog"))
            .or_else(|| urls.get("Changes"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    });

    let license = info
        .get("license")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    // Get published_at from releases if version is specified
    let published_at = resolved_version.as_ref().and_then(|ver| {
        val.get("releases")
            .and_then(|r| r.get(ver))
            .and_then(|r| r.as_array())
            .and_then(|files| files.first())
            .and_then(|f| f.get("upload_time_iso_8601"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
    });

    PackageResolution {
        coordinate: coord.clone(),
        registry_url,
        docs_url,
        source_repository_url,
        homepage_url,
        changelog_url,
        license,
        latest_version,
        resolved_version,
        published_at,
        verified: true,
        warnings: vec![],
    }
}

/// Resolve an npm package.
async fn resolve_npm(
    client: &Client,
    coord: &PackageCoordinate,
    timeout: Duration,
) -> PackageResolution {
    let api_url = PackageEcosystem::Npm.registry_api_url(&coord.name);

    match client.get(&api_url).timeout(timeout).send().await {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<serde_json::Value>().await {
                Ok(val) => parse_npm_response(coord, &val),
                Err(e) => fallback_with_warning(coord, &format!("npm JSON parse error: {e}")),
            }
        }
        Ok(resp) => fallback_with_warning(
            coord,
            &format!("npm API returned status {}", resp.status()),
        ),
        Err(e) => fallback_with_warning(coord, &format!("npm API error: {e}")),
    }
}

/// Parse an npm registry response into PackageResolution.
fn parse_npm_response(coord: &PackageCoordinate, val: &serde_json::Value) -> PackageResolution {
    let latest_version = val
        .get("dist-tags")
        .and_then(|d| d.get("latest"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let resolved_version = coord
        .version
        .clone()
        .or_else(|| latest_version.clone());

    let registry_url = Some(format!(
        "https://www.npmjs.com/package/{}",
        coord.name
    ));

    let repository_url = val
        .get("repository")
        .and_then(|r| r.get("url"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(normalize_npm_repo_url);

    let homepage_url = val
        .get("homepage")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let docs_url = Some(format!(
        "https://www.npmjs.com/package/{}",
        coord.name
    ));

    let license = val
        .get("license")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    // Get published_at from the specific version
    let published_at = resolved_version.as_ref().and_then(|ver| {
        val.get("time")
            .and_then(|t| t.get(ver))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
    });

    PackageResolution {
        coordinate: coord.clone(),
        registry_url,
        docs_url,
        source_repository_url: repository_url,
        homepage_url,
        changelog_url: None,
        license,
        latest_version,
        resolved_version,
        published_at,
        verified: true,
        warnings: vec![],
    }
}

/// Normalize npm repository URLs (remove git+ prefix, .git suffix).
fn normalize_npm_repo_url(url: &str) -> String {
    let url = url
        .strip_prefix("git+")
        .unwrap_or(url)
        .strip_suffix(".git")
        .unwrap_or(url);
    // Also handle github: shorthand
    if let Some(path) = url.strip_prefix("github:") {
        return format!("https://github.com/{}", path);
    }
    url.to_string()
}

/// Create a PackageResolution with deterministic fallback URLs and a warning.
fn fallback_with_warning(coord: &PackageCoordinate, warning: &str) -> PackageResolution {
    let registry_url = Some(coord.ecosystem.registry_base_url().to_string());
    let docs_url = match coord.ecosystem {
        PackageEcosystem::CratesIo => coord.version.as_ref().map(|v| {
            format!("https://docs.rs/{}/{}", coord.name, v)
        }),
        PackageEcosystem::Pypi => Some(format!(
            "https://pypi.org/project/{}/",
            coord.name
        )),
        PackageEcosystem::Npm => Some(format!(
            "https://www.npmjs.com/package/{}",
            coord.name
        )),
    };

    PackageResolution {
        coordinate: coord.clone(),
        registry_url,
        docs_url,
        source_repository_url: None,
        homepage_url: None,
        changelog_url: None,
        license: None,
        latest_version: None,
        resolved_version: coord.version.clone(),
        published_at: None,
        verified: false,
        warnings: vec![warning.to_string()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::package::{PackageCoordinate, PackageEcosystem};

    fn test_client() -> Client {
        Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap()
    }

    #[test]
    fn normalize_npm_repo_url_strips_git_prefix() {
        assert_eq!(
            normalize_npm_repo_url("git+https://github.com/user/repo.git"),
            "https://github.com/user/repo"
        );
    }

    #[test]
    fn normalize_npm_repo_url_github_shorthand() {
        assert_eq!(
            normalize_npm_repo_url("github:user/repo"),
            "https://github.com/user/repo"
        );
    }

    #[test]
    fn normalize_npm_repo_url_plain_https() {
        assert_eq!(
            normalize_npm_repo_url("https://github.com/user/repo"),
            "https://github.com/user/repo"
        );
    }

    #[test]
    fn fallback_with_warning_creates_valid_resolution() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::CratesIo,
            name: "axum".to_string(),
            version: Some("0.7.0".to_string()),
            version_requirement: None,
        };
        let res = fallback_with_warning(&coord, "API timeout");
        assert!(!res.verified);
        assert_eq!(res.warnings.len(), 1);
        assert_eq!(res.warnings[0], "API timeout");
        assert!(res.registry_url.is_some());
        assert!(res.docs_url.is_some());
        assert_eq!(res.resolved_version.as_deref(), Some("0.7.0"));
    }

    #[test]
    fn fallback_with_warning_pypi() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Pypi,
            name: "requests".to_string(),
            version: None,
            version_requirement: None,
        };
        let res = fallback_with_warning(&coord, "network error");
        assert!(!res.verified);
        assert!(res.registry_url.unwrap().contains("pypi.org"));
        assert!(res.docs_url.unwrap().contains("pypi.org"));
    }

    #[test]
    fn fallback_with_warning_npm() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Npm,
            name: "express".to_string(),
            version: None,
            version_requirement: None,
        };
        let res = fallback_with_warning(&coord, "404 not found");
        assert!(!res.verified);
        assert!(res.registry_url.unwrap().contains("npmjs.com"));
        assert!(res.docs_url.unwrap().contains("npmjs.com"));
    }

    #[test]
    fn parse_crates_io_response_basic() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::CratesIo,
            name: "axum".to_string(),
            version: None,
            version_requirement: None,
        };
        let val: serde_json::Value = serde_json::json!({
            "crate": {
                "newest_version": "0.8.1",
                "repository": "https://github.com/tokio-rs/axum",
                "homepage": "https://github.com/tokio-rs/axum",
                "license": "MIT"
            }
        });
        let res = parse_crates_io_response(&coord, &val);
        assert!(res.verified);
        assert_eq!(res.latest_version.as_deref(), Some("0.8.1"));
        assert_eq!(res.resolved_version.as_deref(), Some("0.8.1"));
        assert_eq!(
            res.source_repository_url.as_deref(),
            Some("https://github.com/tokio-rs/axum")
        );
        assert_eq!(res.license.as_deref(), Some("MIT"));
        assert!(res.warnings.is_empty());
    }

    #[test]
    fn parse_crates_io_response_with_explicit_version() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::CratesIo,
            name: "axum".to_string(),
            version: Some("0.7.0".to_string()),
            version_requirement: None,
        };
        let val: serde_json::Value = serde_json::json!({
            "crate": {
                "newest_version": "0.8.1",
                "repository": "https://github.com/tokio-rs/axum"
            }
        });
        let res = parse_crates_io_response(&coord, &val);
        assert_eq!(res.resolved_version.as_deref(), Some("0.7.0"));
        assert_eq!(res.latest_version.as_deref(), Some("0.8.1"));
        assert!(res.docs_url.unwrap().contains("0.7.0"));
    }

    #[test]
    fn parse_pypi_response_basic() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Pypi,
            name: "requests".to_string(),
            version: None,
            version_requirement: None,
        };
        let val: serde_json::Value = serde_json::json!({
            "info": {
                "version": "2.31.0",
                "home_page": "https://requests.readthedocs.io",
                "license": "Apache-2.0",
                "project_urls": {
                    "Documentation": "https://requests.readthedocs.io",
                    "Source": "https://github.com/psf/requests",
                    "Changelog": "https://github.com/psf/requests/blob/main/HISTORY.md"
                }
            },
            "releases": {
                "2.31.0": [{
                    "upload_time_iso_8601": "2023-05-22T15:00:00Z"
                }]
            }
        });
        let res = parse_pypi_response(&coord, &val);
        assert!(res.verified);
        assert_eq!(res.latest_version.as_deref(), Some("2.31.0"));
        assert_eq!(res.resolved_version.as_deref(), Some("2.31.0"));
        assert_eq!(
            res.source_repository_url.as_deref(),
            Some("https://github.com/psf/requests")
        );
        assert!(res.docs_url.unwrap().contains("requests.readthedocs.io"));
        assert!(res.changelog_url.unwrap().contains("HISTORY.md"));
        assert_eq!(res.published_at.as_deref(), Some("2023-05-22T15:00:00Z"));
    }

    #[test]
    fn parse_npm_response_basic() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Npm,
            name: "express".to_string(),
            version: None,
            version_requirement: None,
        };
        let val: serde_json::Value = serde_json::json!({
            "dist-tags": {
                "latest": "4.18.2"
            },
            "repository": {
                "url": "git+https://github.com/expressjs/express.git"
            },
            "homepage": "https://expressjs.com",
            "license": "MIT",
            "time": {
                "4.18.2": "2023-06-20T12:00:00.000Z"
            }
        });
        let res = parse_npm_response(&coord, &val);
        assert!(res.verified);
        assert_eq!(res.latest_version.as_deref(), Some("4.18.2"));
        assert_eq!(res.resolved_version.as_deref(), Some("4.18.2"));
        assert_eq!(
            res.source_repository_url.as_deref(),
            Some("https://github.com/expressjs/express")
        );
        assert_eq!(res.homepage_url.as_deref(), Some("https://expressjs.com"));
        assert_eq!(res.license.as_deref(), Some("MIT"));
    }

    #[test]
    fn parse_npm_response_with_explicit_version() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Npm,
            name: "express".to_string(),
            version: Some("4.18.0".to_string()),
            version_requirement: None,
        };
        let val: serde_json::Value = serde_json::json!({
            "dist-tags": {
                "latest": "4.18.2"
            },
            "time": {
                "4.18.0": "2023-03-01T00:00:00.000Z",
                "4.18.2": "2023-06-20T12:00:00.000Z"
            }
        });
        let res = parse_npm_response(&coord, &val);
        assert_eq!(res.resolved_version.as_deref(), Some("4.18.0"));
        assert_eq!(res.latest_version.as_deref(), Some("4.18.2"));
        assert_eq!(
            res.published_at.as_deref(),
            Some("2023-03-01T00:00:00.000Z")
        );
    }

    #[tokio::test]
    async fn resolve_package_falls_back_on_network_error() {
        let client = test_client();
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::CratesIo,
            name: "axum".to_string(),
            version: Some("0.7.0".to_string()),
            version_requirement: None,
        };
        // Use an invalid URL to trigger a network error
        let res = resolve_package(&client, &coord, Some(Duration::from_millis(1))).await;
        // Should get fallback, not a panic
        assert!(!res.warnings.is_empty() || !res.verified);
    }
}
