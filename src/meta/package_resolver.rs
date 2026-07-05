//! Package registry resolver: bounded HTTP lookups for CratesIo, PyPI,
//! npm, Go, Maven, NuGet, RubyGems, Packagist, OCI, and GitHub Actions.
//!
//! Resolves package coordinates to registry URLs, documentation URLs,
//! source repository URLs, and version information. Falls back to
//! deterministic URLs when registry APIs fail.
//!
//! This is metadata lookup only — it does not solve dependencies or
//! download artifacts. OCI and GitHub Actions use exact version matching
//! only (no semver range).

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
        PackageEcosystem::Go => resolve_go(client, coordinate, timeout).await,
        PackageEcosystem::Maven => resolve_maven(client, coordinate, timeout).await,
        PackageEcosystem::Nuget => resolve_nuget(client, coordinate, timeout).await,
        PackageEcosystem::Rubygems => resolve_rubygems(client, coordinate, timeout).await,
        PackageEcosystem::Packagist => resolve_packagist(client, coordinate, timeout).await,
        PackageEcosystem::Oci => resolve_oci(client, coordinate, timeout).await,
        PackageEcosystem::GithubActions => {
            resolve_github_actions(client, coordinate, timeout).await
        }
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
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
            Ok(val) => parse_crates_io_response(coord, &val),
            Err(e) => fallback_with_warning(coord, &format!("crates.io JSON parse error: {e}")),
        },
        Ok(resp) => fallback_with_warning(
            coord,
            &format!("crates.io API returned status {}", resp.status()),
        ),
        Err(e) => fallback_with_warning(coord, &format!("crates.io API error: {e}")),
    }
}

/// Parse a crates.io API response into PackageResolution.
fn parse_crates_io_response(
    coord: &PackageCoordinate,
    val: &serde_json::Value,
) -> PackageResolution {
    let krate = val.get("crate").unwrap_or(val);

    let latest_version = krate
        .get("newest_version")
        .or_else(|| krate.get("max_version"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let resolved_version = coord.version.clone().or_else(|| latest_version.clone());

    let registry_url = Some(format!("https://crates.io/crates/{}", coord.name));

    let docs_url = resolved_version
        .as_ref()
        .map(|v| format!("https://docs.rs/{}/{}", coord.name, v));

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
        release_url: None,
        advisory_urls: vec![],
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
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
            Ok(val) => parse_pypi_response(coord, &val),
            Err(e) => fallback_with_warning(coord, &format!("PyPI JSON parse error: {e}")),
        },
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

    let resolved_version = coord.version.clone().or_else(|| latest_version.clone());

    let registry_url = Some(format!("https://pypi.org/project/{}/", coord.name));

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

    let source_repository_url = project_urls.and_then(|urls| {
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
        release_url: None,
        advisory_urls: vec![],
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
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
            Ok(val) => parse_npm_response(coord, &val),
            Err(e) => fallback_with_warning(coord, &format!("npm JSON parse error: {e}")),
        },
        Ok(resp) => {
            fallback_with_warning(coord, &format!("npm API returned status {}", resp.status()))
        }
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

    let resolved_version = coord.version.clone().or_else(|| latest_version.clone());

    let registry_url = Some(format!("https://www.npmjs.com/package/{}", coord.name));

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

    let docs_url = Some(format!("https://www.npmjs.com/package/{}", coord.name));

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
        release_url: None,
        advisory_urls: vec![],
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
        return format!("https://github.com/{path}");
    }
    url.to_string()
}

/// Derive a GitHub source repo URL from a Go module path if it starts with github.com/.
fn infer_go_source_repo(module: &str) -> Option<String> {
    let path = module.strip_prefix("github.com/")?;
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() >= 2 {
        Some(format!("https://github.com/{}/{}", parts[0], parts[1]))
    } else {
        None
    }
}

/// Resolve a Go module.
async fn resolve_go(
    client: &Client,
    coord: &PackageCoordinate,
    timeout: Duration,
) -> PackageResolution {
    let api_url = PackageEcosystem::Go.registry_api_url(&coord.name);

    match client.get(&api_url).timeout(timeout).send().await {
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
            Ok(val) => parse_go_response(coord, &val),
            Err(e) => fallback_with_warning(coord, &format!("Go proxy JSON parse error: {e}")),
        },
        Ok(resp) => fallback_with_warning(
            coord,
            &format!("Go proxy returned status {}", resp.status()),
        ),
        Err(e) => fallback_with_warning(coord, &format!("Go proxy error: {e}")),
    }
}

fn parse_go_response(coord: &PackageCoordinate, val: &serde_json::Value) -> PackageResolution {
    let latest_version = val
        .get("Version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let resolved_version = coord.version.clone().or_else(|| latest_version.clone());

    let registry_url = Some(format!("https://pkg.go.dev/{}", coord.name));

    let docs_url = resolved_version
        .as_ref()
        .map(|v| format!("https://pkg.go.dev/{}@{}", coord.name, v));

    let source_repository_url = infer_go_source_repo(&coord.name);

    let published_at = val
        .get("Time")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    PackageResolution {
        coordinate: coord.clone(),
        registry_url,
        docs_url,
        source_repository_url,
        homepage_url: None,
        changelog_url: None,
        release_url: None,
        advisory_urls: vec![],
        license: None,
        latest_version,
        resolved_version,
        published_at,
        verified: true,
        warnings: vec![],
    }
}

/// Resolve a Maven package.
async fn resolve_maven(
    client: &Client,
    coord: &PackageCoordinate,
    timeout: Duration,
) -> PackageResolution {
    let group = coord.namespace.as_deref().unwrap_or("");
    let artifact = &coord.name;
    let query = format!("g:\"{group}\"+AND+a:\"{artifact}\"");
    let api_url = format!("https://search.maven.org/solrsearch/select?q={query}&rows=1&wt=json");

    match client.get(&api_url).timeout(timeout).send().await {
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
            Ok(val) => parse_maven_response(coord, &val),
            Err(e) => fallback_with_warning(coord, &format!("Maven search JSON parse error: {e}")),
        },
        Ok(resp) => fallback_with_warning(
            coord,
            &format!("Maven search returned status {}", resp.status()),
        ),
        Err(e) => fallback_with_warning(coord, &format!("Maven search error: {e}")),
    }
}

fn parse_maven_response(coord: &PackageCoordinate, val: &serde_json::Value) -> PackageResolution {
    let docs = val.get("response").and_then(|r| r.get("docs"));
    let first = docs.and_then(|d| d.as_array()).and_then(|arr| arr.first());

    let latest_version = first
        .and_then(|doc| doc.get("latestVersion"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let resolved_version = coord.version.clone().or_else(|| latest_version.clone());

    let group = coord.namespace.as_deref().unwrap_or("group");
    let registry_url = Some(format!(
        "https://central.sonatype.com/artifact/{}/{}",
        group, coord.name
    ));

    let docs_url = resolved_version
        .as_ref()
        .map(|v| format!("https://javadoc.io/doc/{}/{}/{}", group, coord.name, v));

    let source_repository_url = first
        .and_then(|doc| doc.get("repository"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    PackageResolution {
        coordinate: coord.clone(),
        registry_url,
        docs_url,
        source_repository_url,
        homepage_url: None,
        changelog_url: None,
        release_url: None,
        advisory_urls: vec![],
        license: None,
        latest_version,
        resolved_version,
        published_at: None,
        verified: true,
        warnings: vec![],
    }
}

/// Resolve a NuGet package.
async fn resolve_nuget(
    client: &Client,
    coord: &PackageCoordinate,
    timeout: Duration,
) -> PackageResolution {
    let api_url = PackageEcosystem::Nuget.registry_api_url(&coord.name);

    match client.get(&api_url).timeout(timeout).send().await {
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
            Ok(val) => parse_nuget_response(coord, &val),
            Err(e) => fallback_with_warning(coord, &format!("NuGet JSON parse error: {e}")),
        },
        Ok(resp) => fallback_with_warning(
            coord,
            &format!("NuGet API returned status {}", resp.status()),
        ),
        Err(e) => fallback_with_warning(coord, &format!("NuGet API error: {e}")),
    }
}

fn parse_nuget_response(coord: &PackageCoordinate, val: &serde_json::Value) -> PackageResolution {
    let versions = val.get("versions").and_then(|v| v.as_array());

    let latest_version = versions
        .and_then(|arr| arr.last())
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let resolved_version = coord.version.clone().or_else(|| latest_version.clone());

    let registry_url = Some(format!("https://www.nuget.org/packages/{}", coord.name));

    let docs_url = Some(format!(
        "https://learn.microsoft.com/en-us/dotnet/api/{}",
        coord.name.to_lowercase().replace('-', "")
    ));

    PackageResolution {
        coordinate: coord.clone(),
        registry_url,
        docs_url,
        source_repository_url: None,
        homepage_url: None,
        changelog_url: None,
        release_url: None,
        advisory_urls: vec![],
        license: None,
        latest_version,
        resolved_version,
        published_at: None,
        verified: true,
        warnings: vec![],
    }
}

/// Resolve a RubyGems package.
async fn resolve_rubygems(
    client: &Client,
    coord: &PackageCoordinate,
    timeout: Duration,
) -> PackageResolution {
    let api_url = PackageEcosystem::Rubygems.registry_api_url(&coord.name);

    match client.get(&api_url).timeout(timeout).send().await {
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
            Ok(val) => parse_rubygems_response(coord, &val),
            Err(e) => fallback_with_warning(coord, &format!("RubyGems JSON parse error: {e}")),
        },
        Ok(resp) => fallback_with_warning(
            coord,
            &format!("RubyGems API returned status {}", resp.status()),
        ),
        Err(e) => fallback_with_warning(coord, &format!("RubyGems API error: {e}")),
    }
}

fn parse_rubygems_response(
    coord: &PackageCoordinate,
    val: &serde_json::Value,
) -> PackageResolution {
    let latest_version = val
        .get("version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let resolved_version = coord.version.clone().or_else(|| latest_version.clone());

    let registry_url = Some(format!("https://rubygems.org/gems/{}", coord.name));

    let docs_url = None;

    let source_repository_url = val
        .get("source_code_uri")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let homepage_url = val
        .get("homepage_uri")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let changelog_url = val
        .get("changelog_uri")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let license = val
        .get("licenses")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    PackageResolution {
        coordinate: coord.clone(),
        registry_url,
        docs_url,
        source_repository_url,
        homepage_url,
        changelog_url,
        release_url: None,
        advisory_urls: vec![],
        license,
        latest_version,
        resolved_version,
        published_at: None,
        verified: true,
        warnings: vec![],
    }
}

/// Resolve a Packagist/Composer package.
async fn resolve_packagist(
    client: &Client,
    coord: &PackageCoordinate,
    timeout: Duration,
) -> PackageResolution {
    let api_url = PackageEcosystem::Packagist.registry_api_url(&coord.name);

    match client.get(&api_url).timeout(timeout).send().await {
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
            Ok(val) => parse_packagist_response(coord, &val),
            Err(e) => fallback_with_warning(coord, &format!("Packagist JSON parse error: {e}")),
        },
        Ok(resp) => fallback_with_warning(
            coord,
            &format!("Packagist API returned status {}", resp.status()),
        ),
        Err(e) => fallback_with_warning(coord, &format!("Packagist API error: {e}")),
    }
}

fn parse_packagist_response(
    coord: &PackageCoordinate,
    val: &serde_json::Value,
) -> PackageResolution {
    let pkg = val.get("package").unwrap_or(val);

    let versions = pkg.get("versions").and_then(|v| v.as_object());

    let latest_version = versions.and_then(|map| {
        map.keys()
            .filter(|k| !k.starts_with("dev-") && !k.contains("alpha") && !k.contains("beta"))
            .max_by_key(|k| k.as_str())
            .cloned()
    });

    let resolved_version = coord.version.clone().or_else(|| latest_version.clone());

    let registry_url = Some(format!("https://packagist.org/packages/{}", coord.name));

    let docs_url = None;

    let source_repository_url = pkg
        .get("source")
        .and_then(|s| s.get("url"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let homepage_url = pkg
        .get("homepage")
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
        release_url: None,
        advisory_urls: vec![],
        license: None,
        latest_version,
        resolved_version,
        published_at: None,
        verified: true,
        warnings: vec![],
    }
}

/// Resolve a Docker/OCI image.
async fn resolve_oci(
    client: &Client,
    coord: &PackageCoordinate,
    timeout: Duration,
) -> PackageResolution {
    let api_url = PackageEcosystem::Oci.registry_api_url(&coord.name);

    match client.get(&api_url).timeout(timeout).send().await {
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
            Ok(val) => parse_oci_response(coord, &val),
            Err(e) => fallback_with_warning(coord, &format!("Docker Hub JSON parse error: {e}")),
        },
        Ok(resp) => fallback_with_warning(
            coord,
            &format!("Docker Hub API returned status {}", resp.status()),
        ),
        Err(e) => fallback_with_warning(coord, &format!("Docker Hub API error: {e}")),
    }
}

fn parse_oci_response(coord: &PackageCoordinate, val: &serde_json::Value) -> PackageResolution {
    let registry_url = Some(format!("https://hub.docker.com/r/{}", coord.name));

    let docs_url = None;

    let homepage_url = val
        .get("homepage")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let source_repository_url = val
        .get("source_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let published_at = val
        .get("last_updated")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    PackageResolution {
        coordinate: coord.clone(),
        registry_url,
        docs_url,
        source_repository_url,
        homepage_url,
        changelog_url: None,
        release_url: None,
        advisory_urls: vec![],
        license: None,
        latest_version: None,
        resolved_version: coord.version.clone(),
        published_at,
        verified: true,
        warnings: vec![],
    }
}

/// Resolve a GitHub Actions action (fully deterministic, no API call needed).
async fn resolve_github_actions(
    _client: &Client,
    coord: &PackageCoordinate,
    _timeout: Duration,
) -> PackageResolution {
    let registry_url = Some(format!("https://github.com/{}", coord.name));

    let docs_url = Some(format!(
        "https://github.com/{}/blob/main/README.md",
        coord.name
    ));

    let source_repository_url = Some(format!("https://github.com/{}", coord.name));

    let release_url = Some(format!("https://github.com/{}/releases", coord.name));

    PackageResolution {
        coordinate: coord.clone(),
        registry_url,
        docs_url,
        source_repository_url,
        homepage_url: None,
        changelog_url: None,
        release_url,
        advisory_urls: vec![],
        license: None,
        latest_version: None,
        resolved_version: coord.version.clone(),
        published_at: None,
        verified: false,
        warnings: vec![
            "GitHub Actions resolution is deterministic (no API verification)".to_string(),
        ],
    }
}

/// Create a PackageResolution with deterministic fallback URLs and a warning.
fn fallback_with_warning(coord: &PackageCoordinate, warning: &str) -> PackageResolution {
    let registry_url = Some(coord.ecosystem.registry_base_url().to_string());
    let docs_url = match coord.ecosystem {
        PackageEcosystem::CratesIo => coord
            .version
            .as_ref()
            .map(|v| format!("https://docs.rs/{}/{}", coord.name, v)),
        PackageEcosystem::Pypi => Some(format!("https://pypi.org/project/{}/", coord.name)),
        PackageEcosystem::Npm => Some(format!("https://www.npmjs.com/package/{}", coord.name)),
        PackageEcosystem::Go => Some(format!("https://pkg.go.dev/{}", coord.name)),
        PackageEcosystem::Maven => {
            let group = coord.namespace.as_deref().unwrap_or("group");
            Some(format!(
                "https://central.sonatype.com/artifact/{group}/{}",
                coord.name
            ))
        }
        PackageEcosystem::Nuget => Some(format!("https://www.nuget.org/packages/{}", coord.name)),
        PackageEcosystem::Rubygems => Some(format!("https://rubygems.org/gems/{}", coord.name)),
        PackageEcosystem::Packagist => {
            Some(format!("https://packagist.org/packages/{}", coord.name))
        }
        PackageEcosystem::Oci => Some(format!("https://hub.docker.com/r/{}", coord.name)),
        PackageEcosystem::GithubActions => Some(format!("https://github.com/{}", coord.name)),
    };

    PackageResolution {
        coordinate: coord.clone(),
        registry_url,
        docs_url,
        source_repository_url: None,
        homepage_url: None,
        changelog_url: None,
        release_url: None,
        advisory_urls: vec![],
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
            namespace: None,
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
            namespace: None,
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
            namespace: None,
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
            namespace: None,
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
            namespace: None,
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
            namespace: None,
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
            namespace: None,
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
            namespace: None,
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
            namespace: None,
        };
        // Use an invalid URL to trigger a network error
        let res = resolve_package(&client, &coord, Some(Duration::from_millis(1))).await;
        // Should get fallback, not a panic
        assert!(!res.warnings.is_empty() || !res.verified);
    }

    #[test]
    fn infer_go_source_repo_github() {
        assert_eq!(
            infer_go_source_repo("github.com/tokio-rs/axum"),
            Some("https://github.com/tokio-rs/axum".to_string())
        );
    }

    #[test]
    fn infer_go_source_repo_not_github() {
        assert_eq!(infer_go_source_repo("golang.org/x/text"), None);
    }

    #[test]
    fn infer_go_source_repo_single_component() {
        assert_eq!(infer_go_source_repo("github.com/foo"), None);
    }

    #[test]
    fn parse_go_response_basic() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Go,
            name: "github.com/tokio-rs/axum".to_string(),
            namespace: None,
            version: None,
            version_requirement: None,
        };
        let val: serde_json::Value = serde_json::json!({
            "Version": "v0.7.0",
            "Time": "2024-01-15T10:00:00Z"
        });
        let res = parse_go_response(&coord, &val);
        assert!(res.verified);
        assert_eq!(res.latest_version.as_deref(), Some("v0.7.0"));
        assert_eq!(res.resolved_version.as_deref(), Some("v0.7.0"));
        assert_eq!(
            res.source_repository_url.as_deref(),
            Some("https://github.com/tokio-rs/axum")
        );
        assert_eq!(res.published_at.as_deref(), Some("2024-01-15T10:00:00Z"));
        assert!(res.docs_url.unwrap().contains("@v0.7.0"));
    }

    #[test]
    fn parse_go_response_with_explicit_version() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Go,
            name: "github.com/tokio-rs/axum".to_string(),
            namespace: None,
            version: Some("v0.6.0".to_string()),
            version_requirement: None,
        };
        let val: serde_json::Value = serde_json::json!({
            "Version": "v0.7.0",
            "Time": "2024-01-15T10:00:00Z"
        });
        let res = parse_go_response(&coord, &val);
        assert_eq!(res.resolved_version.as_deref(), Some("v0.6.0"));
        assert_eq!(res.latest_version.as_deref(), Some("v0.7.0"));
        assert!(res.docs_url.unwrap().contains("@v0.6.0"));
    }

    #[test]
    fn parse_maven_response_basic() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Maven,
            name: "commons-lang3".to_string(),
            namespace: Some("org.apache".to_string()),
            version: None,
            version_requirement: None,
        };
        let val: serde_json::Value = serde_json::json!({
            "response": {
                "docs": [{
                    "latestVersion": "3.14.0",
                    "repository": "https://github.com/apache/commons-lang"
                }]
            }
        });
        let res = parse_maven_response(&coord, &val);
        assert!(res.verified);
        assert_eq!(res.latest_version.as_deref(), Some("3.14.0"));
        assert_eq!(res.resolved_version.as_deref(), Some("3.14.0"));
        assert_eq!(
            res.source_repository_url.as_deref(),
            Some("https://github.com/apache/commons-lang")
        );
        assert!(res.docs_url.unwrap().contains("javadoc.io"));
        assert!(res
            .registry_url
            .unwrap()
            .contains("org.apache/commons-lang3"));
    }

    #[test]
    fn parse_maven_response_with_explicit_version() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Maven,
            name: "commons-lang3".to_string(),
            namespace: Some("org.apache".to_string()),
            version: Some("3.12.0".to_string()),
            version_requirement: None,
        };
        let val: serde_json::Value = serde_json::json!({
            "response": {
                "docs": [{
                    "latestVersion": "3.14.0"
                }]
            }
        });
        let res = parse_maven_response(&coord, &val);
        assert_eq!(res.resolved_version.as_deref(), Some("3.12.0"));
        assert_eq!(res.latest_version.as_deref(), Some("3.14.0"));
    }

    #[test]
    fn parse_nuget_response_basic() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Nuget,
            name: "Newtonsoft.Json".to_string(),
            namespace: None,
            version: None,
            version_requirement: None,
        };
        let val: serde_json::Value = serde_json::json!({
            "versions": ["12.0.0", "13.0.1", "13.0.3"]
        });
        let res = parse_nuget_response(&coord, &val);
        assert!(res.verified);
        assert_eq!(res.latest_version.as_deref(), Some("13.0.3"));
        assert_eq!(res.resolved_version.as_deref(), Some("13.0.3"));
        assert!(res.registry_url.unwrap().contains("nuget.org"));
    }

    #[test]
    fn parse_nuget_response_with_explicit_version() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Nuget,
            name: "Newtonsoft.Json".to_string(),
            namespace: None,
            version: Some("12.0.0".to_string()),
            version_requirement: None,
        };
        let val: serde_json::Value = serde_json::json!({
            "versions": ["12.0.0", "13.0.1", "13.0.3"]
        });
        let res = parse_nuget_response(&coord, &val);
        assert_eq!(res.resolved_version.as_deref(), Some("12.0.0"));
        assert_eq!(res.latest_version.as_deref(), Some("13.0.3"));
    }

    #[test]
    fn parse_nuget_response_empty_versions() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Nuget,
            name: "NoSuchPackage".to_string(),
            namespace: None,
            version: None,
            version_requirement: None,
        };
        let val: serde_json::Value = serde_json::json!({
            "versions": []
        });
        let res = parse_nuget_response(&coord, &val);
        assert!(res.verified);
        assert!(res.latest_version.is_none());
    }

    #[test]
    fn parse_rubygems_response_basic() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Rubygems,
            name: "rails".to_string(),
            namespace: None,
            version: None,
            version_requirement: None,
        };
        let val: serde_json::Value = serde_json::json!({
            "version": "7.1.2",
            "homepage_uri": "https://rubyonrails.org",
            "source_code_uri": "https://github.com/rails/rails",
            "changelog_uri": "https://github.com/rails/rails/blob/main/CHANGELOG.md",
            "licenses": ["MIT"]
        });
        let res = parse_rubygems_response(&coord, &val);
        assert!(res.verified);
        assert_eq!(res.latest_version.as_deref(), Some("7.1.2"));
        assert_eq!(res.resolved_version.as_deref(), Some("7.1.2"));
        assert_eq!(
            res.source_repository_url.as_deref(),
            Some("https://github.com/rails/rails")
        );
        assert_eq!(res.homepage_url.as_deref(), Some("https://rubyonrails.org"));
        assert!(res.changelog_url.unwrap().contains("CHANGELOG.md"));
        assert_eq!(res.license.as_deref(), Some("MIT"));
        assert!(res.registry_url.unwrap().contains("rubygems.org"));
    }

    #[test]
    fn parse_rubygems_response_with_explicit_version() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Rubygems,
            name: "rails".to_string(),
            namespace: None,
            version: Some("7.0.8".to_string()),
            version_requirement: None,
        };
        let val: serde_json::Value = serde_json::json!({
            "version": "7.1.2"
        });
        let res = parse_rubygems_response(&coord, &val);
        assert_eq!(res.resolved_version.as_deref(), Some("7.0.8"));
        assert_eq!(res.latest_version.as_deref(), Some("7.1.2"));
    }

    #[test]
    fn parse_packagist_response_basic() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Packagist,
            name: "monolog/monolog".to_string(),
            namespace: None,
            version: None,
            version_requirement: None,
        };
        let val: serde_json::Value = serde_json::json!({
            "package": {
                "versions": {
                    "3.5.0": {},
                    "3.4.1": {},
                    "dev-main": {}
                },
                "source": {
                    "url": "https://github.com/Seldaek/monolog",
                    "type": "git"
                },
                "homepage": "https://github.com/Seldaek/monolog"
            }
        });
        let res = parse_packagist_response(&coord, &val);
        assert!(res.verified);
        assert_eq!(res.latest_version.as_deref(), Some("3.5.0"));
        assert_eq!(res.resolved_version.as_deref(), Some("3.5.0"));
        assert_eq!(
            res.source_repository_url.as_deref(),
            Some("https://github.com/Seldaek/monolog")
        );
        assert!(res.registry_url.unwrap().contains("packagist.org"));
    }

    #[test]
    fn parse_packagist_response_filters_dev_versions() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Packagist,
            name: "monolog/monolog".to_string(),
            namespace: None,
            version: None,
            version_requirement: None,
        };
        let val: serde_json::Value = serde_json::json!({
            "package": {
                "versions": {
                    "dev-main": {},
                    "dev-feature-x": {}
                }
            }
        });
        let res = parse_packagist_response(&coord, &val);
        assert!(res.latest_version.is_none());
    }

    #[test]
    fn parse_oci_response_basic() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Oci,
            name: "library/nginx".to_string(),
            namespace: None,
            version: None,
            version_requirement: None,
        };
        let val: serde_json::Value = serde_json::json!({
            "homepage": "https://github.com/nginxinc/docker-nginx",
            "source_url": "https://github.com/nginxinc/docker-nginx",
            "last_updated": "2024-02-01T12:00:00Z"
        });
        let res = parse_oci_response(&coord, &val);
        assert!(res.verified);
        assert_eq!(
            res.source_repository_url.as_deref(),
            Some("https://github.com/nginxinc/docker-nginx")
        );
        assert_eq!(res.published_at.as_deref(), Some("2024-02-01T12:00:00Z"));
        assert!(res.registry_url.unwrap().contains("hub.docker.com"));
    }

    #[tokio::test]
    async fn resolve_github_actions_deterministic() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::GithubActions,
            name: "actions/checkout".to_string(),
            namespace: None,
            version: Some("v4".to_string()),
            version_requirement: None,
        };
        let client = test_client();
        let res = resolve_github_actions(&client, &coord, Duration::from_secs(5)).await;
        assert!(!res.verified);
        assert_eq!(res.resolved_version.as_deref(), Some("v4"));
        assert_eq!(
            res.registry_url.as_deref(),
            Some("https://github.com/actions/checkout")
        );
        assert!(res.docs_url.unwrap().contains("README.md"));
        assert!(res.release_url.unwrap().contains("/releases"));
        assert_eq!(res.warnings.len(), 1);
        assert!(res.warnings[0].contains("deterministic"));
    }

    #[test]
    fn fallback_with_warning_go() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Go,
            name: "github.com/tokio-rs/axum".to_string(),
            namespace: None,
            version: Some("v0.7.0".to_string()),
            version_requirement: None,
        };
        let res = fallback_with_warning(&coord, "Go proxy timeout");
        assert!(!res.verified);
        assert!(res.registry_url.unwrap().contains("pkg.go.dev"));
        assert!(res.docs_url.unwrap().contains("pkg.go.dev"));
    }

    #[test]
    fn fallback_with_warning_maven() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Maven,
            name: "commons-lang3".to_string(),
            namespace: Some("org.apache".to_string()),
            version: None,
            version_requirement: None,
        };
        let res = fallback_with_warning(&coord, "Maven search failed");
        assert!(!res.verified);
        assert!(res.registry_url.unwrap().contains("sonatype.com"));
        assert!(res.docs_url.unwrap().contains("org.apache"));
    }

    #[test]
    fn fallback_with_warning_nuget() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Nuget,
            name: "Newtonsoft.Json".to_string(),
            namespace: None,
            version: None,
            version_requirement: None,
        };
        let res = fallback_with_warning(&coord, "NuGet API error");
        assert!(!res.verified);
        assert!(res.registry_url.unwrap().contains("nuget.org"));
        assert!(res.docs_url.unwrap().contains("nuget"));
    }

    #[test]
    fn fallback_with_warning_rubygems() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Rubygems,
            name: "rails".to_string(),
            namespace: None,
            version: None,
            version_requirement: None,
        };
        let res = fallback_with_warning(&coord, "RubyGems timeout");
        assert!(!res.verified);
        assert!(res.registry_url.unwrap().contains("rubygems.org"));
        assert!(res.docs_url.unwrap().contains("rubygems.org"));
    }

    #[test]
    fn fallback_with_warning_packagist() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Packagist,
            name: "monolog/monolog".to_string(),
            namespace: None,
            version: None,
            version_requirement: None,
        };
        let res = fallback_with_warning(&coord, "Packagist API error");
        assert!(!res.verified);
        assert!(res.registry_url.unwrap().contains("packagist.org"));
        assert!(res.docs_url.unwrap().contains("packagist.org"));
    }

    #[test]
    fn fallback_with_warning_oci() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::Oci,
            name: "library/nginx".to_string(),
            namespace: None,
            version: None,
            version_requirement: None,
        };
        let res = fallback_with_warning(&coord, "Docker Hub error");
        assert!(!res.verified);
        assert!(res.registry_url.unwrap().contains("hub.docker.com"));
        assert!(res.docs_url.unwrap().contains("hub.docker.com"));
    }

    #[test]
    fn fallback_with_warning_github_actions() {
        let coord = PackageCoordinate {
            ecosystem: PackageEcosystem::GithubActions,
            name: "actions/checkout".to_string(),
            namespace: None,
            version: None,
            version_requirement: None,
        };
        let res = fallback_with_warning(&coord, "GitHub Actions fallback");
        assert!(!res.verified);
        assert!(res.registry_url.unwrap().contains("github.com"));
        assert!(res.docs_url.unwrap().contains("github.com"));
    }

    #[tokio::test]
    async fn resolve_package_all_new_ecosystems_fallback() {
        let client = test_client();
        let ecosystems = vec![
            (PackageEcosystem::Go, "github.com/foo/bar"),
            (PackageEcosystem::Maven, "commons-lang3"),
            (PackageEcosystem::Nuget, "Newtonsoft.Json"),
            (PackageEcosystem::Rubygems, "rails"),
            (PackageEcosystem::Packagist, "monolog/monolog"),
            (PackageEcosystem::Oci, "library/nginx"),
            (PackageEcosystem::GithubActions, "actions/checkout"),
        ];
        for (eco, name) in ecosystems {
            let coord = PackageCoordinate {
                ecosystem: eco.clone(),
                name: name.to_string(),
                namespace: None,
                version: None,
                version_requirement: None,
            };
            let res = resolve_package(&client, &coord, Some(Duration::from_millis(1))).await;
            assert!(
                !res.verified || !res.warnings.is_empty(),
                "Expected fallback for {eco}: {name}"
            );
        }
    }
}
