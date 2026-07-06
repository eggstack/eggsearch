use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;

use super::error::EngineError;
use super::models::{ResultMetadata, SearchResult};
use crate::core::package::{PackageCoordinate, PackageEcosystem};

pub struct MavenCentralRegistryEngine {
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

    let (group, artifact, version) = parse_maven_query(query);
    if artifact.is_empty() {
        return Ok(Vec::new());
    }

    let coord = PackageCoordinate {
        ecosystem: PackageEcosystem::Maven,
        name: artifact,
        namespace: if group.is_empty() { None } else { Some(group) },
        version,
        version_requirement: None,
    };

    let resolution =
        crate::meta::package_resolver::resolve_package(client, &coord, Some(timeout)).await;

    Ok(vec![resolution_to_result(&resolution)])
}

fn parse_maven_query(query: &str) -> (String, String, Option<String>) {
    let trimmed = query.trim();

    let (coord_part, version) = if let Some(pos) = trimmed.rfind('@') {
        let c = trimmed[..pos].trim().to_string();
        let v = trimmed[pos + 1..].trim().to_string();
        if v.is_empty() {
            (c, None)
        } else {
            (c, Some(v))
        }
    } else if let Some(pos) = trimmed.rfind(" version:") {
        let c = trimmed[..pos].trim().to_string();
        let v = trimmed[pos + 9..].trim().to_string();
        if v.is_empty() {
            (c, None)
        } else {
            (c, Some(v))
        }
    } else if let Some(pos) = trimmed.rfind(':') {
        let maybe_version = &trimmed[pos + 1..];
        if maybe_version
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
        {
            let c = trimmed[..pos].trim().to_string();
            let v = maybe_version.trim().to_string();
            (c, if v.is_empty() { None } else { Some(v) })
        } else {
            (trimmed.to_string(), None)
        }
    } else {
        (trimmed.to_string(), None)
    };

    if let Some(colon_pos) = coord_part.find(':') {
        let group = coord_part[..colon_pos].trim().to_string();
        let artifact = coord_part[colon_pos + 1..].trim().to_string();
        (group, artifact, version)
    } else {
        (String::new(), coord_part, version)
    }
}

fn resolution_to_result(resolution: &crate::core::package::PackageResolution) -> SearchResult {
    let name = &resolution.coordinate.name;
    let group_display = resolution
        .coordinate
        .namespace
        .as_deref()
        .unwrap_or("unknown");
    let version_display = resolution
        .resolved_version
        .as_deref()
        .or(resolution.latest_version.as_deref())
        .unwrap_or("unknown");

    let title = format!("{group_display}:{name} {version_display}");

    let mut snippet_parts = Vec::new();
    if let Some(ref repo) = resolution.source_repository_url {
        snippet_parts.push(format!("Repository: {repo}"));
    }
    if let Some(ref docs) = resolution.docs_url {
        snippet_parts.push(format!("Docs: {docs}"));
    }
    for w in &resolution.warnings {
        snippet_parts.push(format!("Warning: {w}"));
    }

    let url = resolution
        .registry_url
        .clone()
        .unwrap_or_else(|| format!("https://central.sonatype.com/artifact/{group_display}/{name}"));

    SearchResult {
        title,
        url,
        snippet: if snippet_parts.is_empty() {
            None
        } else {
            Some(snippet_parts.join(" | "))
        },
        source_engine: "maven_central".to_string(),
        metadata: ResultMetadata::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_maven_query_with_colon() {
        let (group, artifact, version) = parse_maven_query("org.apache:commons-lang3");
        assert_eq!(group, "org.apache");
        assert_eq!(artifact, "commons-lang3");
        assert!(version.is_none());
    }

    #[test]
    fn parse_maven_query_with_version_keyword() {
        let (group, artifact, version) =
            parse_maven_query("org.apache:commons-lang3 version:3.14.0");
        assert_eq!(group, "org.apache");
        assert_eq!(artifact, "commons-lang3");
        assert_eq!(version.as_deref(), Some("3.14.0"));
    }

    #[test]
    fn parse_maven_query_simple_artifact() {
        let (group, artifact, version) = parse_maven_query("commons-lang3");
        assert!(group.is_empty());
        assert_eq!(artifact, "commons-lang3");
        assert!(version.is_none());
    }

    #[test]
    fn parse_maven_query_empty() {
        let (group, artifact, version) = parse_maven_query("");
        assert!(group.is_empty());
        assert!(artifact.is_empty());
        assert!(version.is_none());
    }
}
