use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;

use super::error::EngineError;
use super::models::{ResultMetadata, SearchResult};
use crate::core::package::{PackageCoordinate, PackageEcosystem};

pub struct PypiRegistryEngine {
    pub client: Arc<Client>,
}

pub async fn search(
    client: &Client,
    query: &str,
    max_results: usize,
    timeout: Duration,
) -> Result<Vec<SearchResult>, EngineError> {
    if max_results == 0 {
        return Ok(Vec::new());
    }

    let (name, version) = parse_pypi_query(query);
    if name.is_empty() {
        return Ok(Vec::new());
    }

    let coord = PackageCoordinate {
        ecosystem: PackageEcosystem::Pypi,
        name,
        namespace: None,
        version,
        version_requirement: None,
    };

    let resolution =
        crate::meta::package_resolver::resolve_package(client, &coord, Some(timeout)).await;

    Ok(vec![resolution_to_result(&resolution)])
}

fn parse_pypi_query(query: &str) -> (String, Option<String>) {
    let trimmed = query.trim();

    if let Some(pos) = trimmed.find('@') {
        let name = trimmed[..pos].trim().to_string();
        let version = trimmed[pos + 1..].trim().to_string();
        if !version.is_empty() {
            return (name, Some(version));
        }
        return (name, None);
    }

    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.is_empty() {
        return (String::new(), None);
    }

    if parts.len() >= 2 {
        let maybe_ver = parts[1];
        if maybe_ver.starts_with("version:")
            || maybe_ver.starts_with("v:")
            || maybe_ver
                .chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
        {
            let ver = if maybe_ver.contains(':') {
                maybe_ver
                    .split_once(':')
                    .map(|x| x.1)
                    .unwrap_or("")
                    .to_string()
            } else {
                maybe_ver.to_string()
            };
            if !ver.is_empty() {
                return (parts[0].to_string(), Some(ver));
            }
        }
    }

    (parts[0].to_string(), None)
}

fn resolution_to_result(resolution: &crate::core::package::PackageResolution) -> SearchResult {
    let name = &resolution.coordinate.name;
    let version_display = resolution
        .resolved_version
        .as_deref()
        .or(resolution.latest_version.as_deref())
        .unwrap_or("unknown");

    let title = format!("{name} {version_display}");

    let mut snippet_parts = Vec::new();
    if let Some(ref lic) = resolution.license {
        snippet_parts.push(format!("License: {lic}"));
    }
    if let Some(ref repo) = resolution.source_repository_url {
        snippet_parts.push(format!("Repository: {repo}"));
    }
    if let Some(ref home) = resolution.homepage_url {
        snippet_parts.push(format!("Homepage: {home}"));
    }
    if let Some(ref docs) = resolution.docs_url {
        snippet_parts.push(format!("Docs: {docs}"));
    }
    if let Some(ref changelog) = resolution.changelog_url {
        snippet_parts.push(format!("Changelog: {changelog}"));
    }
    for w in &resolution.warnings {
        snippet_parts.push(format!("Warning: {w}"));
    }

    let url = resolution
        .registry_url
        .clone()
        .unwrap_or_else(|| format!("https://pypi.org/project/{name}/"));

    SearchResult {
        title,
        url,
        snippet: if snippet_parts.is_empty() {
            None
        } else {
            Some(snippet_parts.join(" | "))
        },
        source_engine: "pypi".to_string(),
        excerpts: Vec::new(),
        published_at: None,
        metadata: ResultMetadata::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pypi_query_simple() {
        let (name, version) = parse_pypi_query("requests");
        assert_eq!(name, "requests");
        assert!(version.is_none());
    }

    #[test]
    fn parse_pypi_query_with_version_keyword() {
        let (name, version) = parse_pypi_query("requests version:2.31.0");
        assert_eq!(name, "requests");
        assert_eq!(version.as_deref(), Some("2.31.0"));
    }

    #[test]
    fn parse_pypi_query_with_at_sign() {
        let (name, version) = parse_pypi_query("requests@2.31.0");
        assert_eq!(name, "requests");
        assert_eq!(version.as_deref(), Some("2.31.0"));
    }

    #[test]
    fn parse_pypi_query_with_bare_version() {
        let (name, version) = parse_pypi_query("requests 2.31.0");
        assert_eq!(name, "requests");
        assert_eq!(version.as_deref(), Some("2.31.0"));
    }

    #[test]
    fn parse_pypi_query_empty() {
        let (name, version) = parse_pypi_query("");
        assert!(name.is_empty());
        assert!(version.is_none());
    }
}
