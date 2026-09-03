use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;

use super::error::EngineError;
use super::models::{ResultMetadata, SearchResult};
use crate::core::package::{PackageCoordinate, PackageEcosystem};

pub struct NugetRegistryEngine {
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

    let (name, version) = parse_nuget_query(query);
    if name.is_empty() {
        return Ok(Vec::new());
    }

    let coord = PackageCoordinate {
        ecosystem: PackageEcosystem::Nuget,
        name,
        namespace: None,
        version,
        version_requirement: None,
    };

    let resolution =
        crate::meta::package_resolver::resolve_package(client, &coord, Some(timeout)).await;

    Ok(vec![resolution_to_result(&resolution)])
}

fn parse_nuget_query(query: &str) -> (String, Option<String>) {
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
    if let Some(ref docs) = resolution.docs_url {
        snippet_parts.push(format!("Docs: {docs}"));
    }
    for w in &resolution.warnings {
        snippet_parts.push(format!("Warning: {w}"));
    }

    let url = resolution
        .registry_url
        .clone()
        .unwrap_or_else(|| format!("https://www.nuget.org/packages/{name}"));

    SearchResult {
        title,
        url,
        snippet: if snippet_parts.is_empty() {
            None
        } else {
            Some(snippet_parts.join(" | "))
        },
        source_engine: "nuget".to_string(),
        excerpts: Vec::new(),
        published_at: None,
        metadata: ResultMetadata::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_nuget_query_simple() {
        let (name, version) = parse_nuget_query("Newtonsoft.Json");
        assert_eq!(name, "Newtonsoft.Json");
        assert!(version.is_none());
    }

    #[test]
    fn parse_nuget_query_with_version() {
        let (name, version) = parse_nuget_query("Newtonsoft.Json version:13.0.3");
        assert_eq!(name, "Newtonsoft.Json");
        assert_eq!(version.as_deref(), Some("13.0.3"));
    }

    #[test]
    fn parse_nuget_query_with_at_sign() {
        let (name, version) = parse_nuget_query("Newtonsoft.Json@13.0.3");
        assert_eq!(name, "Newtonsoft.Json");
        assert_eq!(version.as_deref(), Some("13.0.3"));
    }

    #[test]
    fn parse_nuget_query_empty() {
        let (name, version) = parse_nuget_query("");
        assert!(name.is_empty());
        assert!(version.is_none());
    }
}
