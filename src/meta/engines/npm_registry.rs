use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;

use super::error::EngineError;
use super::models::{ResultMetadata, SearchResult};
use crate::core::package::{PackageCoordinate, PackageEcosystem};

pub struct NpmRegistryEngine {
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

    let (name, version) = parse_npm_query(query);
    if name.is_empty() {
        return Ok(Vec::new());
    }

    let coord = PackageCoordinate {
        ecosystem: PackageEcosystem::Npm,
        name,
        namespace: None,
        version,
        version_requirement: None,
    };

    let resolution =
        crate::meta::package_resolver::resolve_package(client, &coord, Some(timeout)).await;

    Ok(vec![resolution_to_result(&resolution)])
}

fn parse_npm_query(query: &str) -> (String, Option<String>) {
    let trimmed = query.trim();

    if let Some(rest) = trimmed.strip_prefix('@') {
        if let Some(pos) = rest.find('@') {
            let name = trimmed[..=pos].trim().to_string();
            let version = trimmed[pos + 2..].trim().to_string();
            if !version.is_empty() {
                return (name, Some(version));
            }
            return (name, None);
        }
        return (trimmed.to_string(), None);
    }

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

    if parts.len() >= 2 && (parts[1].starts_with("version:") || parts[1].starts_with("v:")) {
        let ver = parts[1]
            .split_once(':')
            .map(|x| x.1)
            .unwrap_or("")
            .to_string();
        if !ver.is_empty() {
            return (parts[0].to_string(), Some(ver));
        }
        return (parts[0].to_string(), None);
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
    for w in &resolution.warnings {
        snippet_parts.push(format!("Warning: {w}"));
    }

    let url = resolution
        .registry_url
        .clone()
        .unwrap_or_else(|| format!("https://www.npmjs.com/package/{name}"));

    SearchResult {
        title,
        url,
        snippet: if snippet_parts.is_empty() {
            None
        } else {
            Some(snippet_parts.join(" | "))
        },
        source_engine: "npm_registry".to_string(),
        metadata: ResultMetadata::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_npm_query_simple() {
        let (name, version) = parse_npm_query("express");
        assert_eq!(name, "express");
        assert!(version.is_none());
    }

    #[test]
    fn parse_npm_query_with_version() {
        let (name, version) = parse_npm_query("express version:4.18.2");
        assert_eq!(name, "express");
        assert_eq!(version.as_deref(), Some("4.18.2"));
    }

    #[test]
    fn parse_npm_query_with_at_sign() {
        let (name, version) = parse_npm_query("express@4.18.2");
        assert_eq!(name, "express");
        assert_eq!(version.as_deref(), Some("4.18.2"));
    }

    #[test]
    fn parse_npm_query_empty() {
        let (name, version) = parse_npm_query("");
        assert!(name.is_empty());
        assert!(version.is_none());
    }

    #[test]
    fn parse_npm_query_at_beginning() {
        let (name, version) = parse_npm_query("@scope/package@1.0.0");
        assert_eq!(name, "@scope/package");
        assert_eq!(version.as_deref(), Some("1.0.0"));
    }
}
