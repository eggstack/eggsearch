use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::{CodeSearchMetadata, ResultMetadata, SearchResult};

const ENGINE: &str = "sourcegraph";
const DEFAULT_BASE_URL: &str = "https://sourcegraph.com/.api/search";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct SourcegraphResponse {
    #[serde(default)]
    results: Vec<SourcegraphResult>,
}

#[derive(Debug, Deserialize)]
struct SourcegraphResult {
    #[serde(default)]
    repository: Option<SourcegraphRepository>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    file_name: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    branches: Option<Vec<String>>,
    #[serde(default)]
    line_matches: Vec<SourcegraphLineMatch>,
}

#[derive(Debug, Deserialize)]
struct SourcegraphRepository {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SourcegraphLineMatch {
    #[serde(default)]
    line: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    range: Option<SourcegraphRange>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SourcegraphRange {
    #[serde(default)]
    start: Option<SourcegraphPosition>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SourcegraphPosition {
    #[serde(default)]
    offset: Option<u64>,
    #[serde(default)]
    line: Option<u64>,
}

pub async fn search(
    client: &Client,
    api_key: Option<&str>,
    query: &str,
    max_results: usize,
    timeout: Duration,
) -> Result<Vec<SearchResult>, EngineError> {
    if max_results == 0 {
        return Ok(Vec::new());
    }

    let count = max_results.clamp(1, 100);
    let url = format!(
        "{}/json?query={}&display={}",
        DEFAULT_BASE_URL,
        urlencoding::encode(query),
        count,
    );

    let response = tokio::time::timeout(timeout, {
        let mut req = client.get(&url).header("Accept", "application/json");
        if let Some(key) = api_key {
            req = req.header("Authorization", format!("token {key}"));
        }
        req.send()
    })
    .await
    .map_err(|_| EngineError::Timeout { engine: ENGINE })?
    .map_err(|e| EngineError::Http {
        engine: ENGINE,
        source: e,
    })?;

    let status = response.status();
    if !status.is_success() {
        return Err(EngineError::BadStatus {
            engine: ENGINE,
            status: status.as_u16(),
        });
    }

    let bytes = response.bytes().await.map_err(|e| EngineError::Http {
        engine: ENGINE,
        source: e,
    })?;
    if bytes.len() > MAX_BODY_BYTES {
        return Err(EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("response body too large: {} bytes", bytes.len()),
        });
    }

    let parsed: SourcegraphResponse =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;

    Ok(convert(parsed.results, max_results))
}

fn convert(results: Vec<SourcegraphResult>, max_results: usize) -> Vec<SearchResult> {
    let mut out = Vec::with_capacity(max_results.min(results.len()));
    for result in results {
        if out.len() >= max_results {
            break;
        }
        if let Some(card) = convert_result(result) {
            out.push(card);
        }
    }
    out
}

fn convert_result(result: SourcegraphResult) -> Option<SearchResult> {
    let repo = result.repository?;
    let repo_name = repo.name?.trim().to_string();
    if repo_name.is_empty() {
        return None;
    }

    let path = result.path?.trim().to_string();
    if path.is_empty() {
        return None;
    }

    let url = repo
        .url
        .as_deref()
        .map(|u| {
            let mut base = u.trim_end_matches('/').to_string();
            base.push_str("/blob/main/");
            base.push_str(&path);
            base
        })
        .unwrap_or_else(|| format!("https://sourcegraph.com/{repo_name}/-/blob/{path}"));

    let title = format!("{path} - {repo_name}");

    let snippet = result
        .line_matches
        .first()
        .and_then(|lm| lm.line.as_deref())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let matched_symbol = result.line_matches.iter().find_map(|lm| {
        lm.line.as_deref().and_then(|line| {
            let line = line.trim();
            if line.starts_with("fn ")
                || line.starts_with("pub fn ")
                || line.starts_with("pub(crate) fn ")
                || line.starts_with("struct ")
                || line.starts_with("pub struct ")
                || line.starts_with("enum ")
                || line.starts_with("pub enum ")
                || line.starts_with("trait ")
                || line.starts_with("pub trait ")
                || line.starts_with("type ")
                || line.starts_with("pub type ")
                || line.starts_with("impl ")
            {
                extract_symbol_name(line)
            } else {
                None
            }
        })
    });

    let text_fragment = result
        .line_matches
        .first()
        .and_then(|lm| lm.line.as_ref().map(|l| l.trim().to_string()));

    let metadata = if matched_symbol.is_some() || text_fragment.is_some() {
        ResultMetadata::CodeSearch(CodeSearchMetadata {
            matched_symbol,
            text_fragment,
        })
    } else {
        ResultMetadata::None
    };

    Some(SearchResult {
        title,
        url,
        snippet,
        source_engine: ENGINE.to_string(),
        metadata,
    })
}

fn extract_symbol_name(line: &str) -> Option<String> {
    let line = line.trim();
    for prefix in &[
        "pub(crate) fn ",
        "pub fn ",
        "fn ",
        "pub struct ",
        "struct ",
        "pub enum ",
        "enum ",
        "pub trait ",
        "trait ",
        "pub type ",
        "type ",
        "impl ",
    ] {
        if let Some(rest) = line.strip_prefix(prefix) {
            let name = rest
                .split(['(', '{', '<', ':', ' '])
                .next()
                .unwrap_or("")
                .trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_result_returns_none_for_missing_repo() {
        let result = SourcegraphResult {
            repository: None,
            path: Some("src/lib.rs".to_string()),
            file_name: Some("lib.rs".to_string()),
            branches: None,
            line_matches: vec![],
        };
        assert!(convert_result(result).is_none());
    }

    #[test]
    fn test_convert_result_returns_none_for_missing_path() {
        let result = SourcegraphResult {
            repository: Some(SourcegraphRepository {
                name: Some("test/repo".to_string()),
                url: None,
            }),
            path: None,
            file_name: Some("lib.rs".to_string()),
            branches: None,
            line_matches: vec![],
        };
        assert!(convert_result(result).is_none());
    }

    #[test]
    fn test_convert_result_returns_none_for_empty_repo_name() {
        let result = SourcegraphResult {
            repository: Some(SourcegraphRepository {
                name: Some(String::new()),
                url: None,
            }),
            path: Some("src/lib.rs".to_string()),
            file_name: Some("lib.rs".to_string()),
            branches: None,
            line_matches: vec![],
        };
        assert!(convert_result(result).is_none());
    }

    #[test]
    fn test_convert_result_returns_none_for_empty_path() {
        let result = SourcegraphResult {
            repository: Some(SourcegraphRepository {
                name: Some("test/repo".to_string()),
                url: None,
            }),
            path: Some(String::new()),
            file_name: Some("lib.rs".to_string()),
            branches: None,
            line_matches: vec![],
        };
        assert!(convert_result(result).is_none());
    }

    #[test]
    fn test_convert_result_with_valid_data() {
        let result = SourcegraphResult {
            repository: Some(SourcegraphRepository {
                name: Some("tokio-rs/axum".to_string()),
                url: Some("https://github.com/tokio-rs/axum".to_string()),
            }),
            path: Some("src/lib.rs".to_string()),
            file_name: Some("lib.rs".to_string()),
            branches: None,
            line_matches: vec![],
        };
        let card = convert_result(result).unwrap();
        assert_eq!(card.title, "src/lib.rs - tokio-rs/axum");
        assert_eq!(
            card.url,
            "https://github.com/tokio-rs/axum/blob/main/src/lib.rs"
        );
        assert!(card.snippet.is_none());
        assert_eq!(card.source_engine, "sourcegraph");
    }

    #[test]
    fn test_convert_result_fallback_url_when_no_repo_url() {
        let result = SourcegraphResult {
            repository: Some(SourcegraphRepository {
                name: Some("test/repo".to_string()),
                url: None,
            }),
            path: Some("src/main.rs".to_string()),
            file_name: Some("main.rs".to_string()),
            branches: None,
            line_matches: vec![],
        };
        let card = convert_result(result).unwrap();
        assert_eq!(
            card.url,
            "https://sourcegraph.com/test/repo/-/blob/src/main.rs"
        );
    }

    #[test]
    fn test_convert_result_with_line_match() {
        let result = SourcegraphResult {
            repository: Some(SourcegraphRepository {
                name: Some("test/repo".to_string()),
                url: None,
            }),
            path: Some("src/lib.rs".to_string()),
            file_name: Some("lib.rs".to_string()),
            branches: None,
            line_matches: vec![SourcegraphLineMatch {
                line: Some("pub fn main() {}".to_string()),
                range: None,
            }],
        };
        let card = convert_result(result).unwrap();
        assert_eq!(card.snippet.as_deref(), Some("pub fn main() {}"));
    }

    #[test]
    fn test_convert_result_with_symbol_extraction() {
        let result = SourcegraphResult {
            repository: Some(SourcegraphRepository {
                name: Some("test/repo".to_string()),
                url: None,
            }),
            path: Some("src/lib.rs".to_string()),
            file_name: Some("lib.rs".to_string()),
            branches: None,
            line_matches: vec![SourcegraphLineMatch {
                line: Some("pub fn my_function(arg: u32) -> bool {}".to_string()),
                range: None,
            }],
        };
        let card = convert_result(result).unwrap();
        match &card.metadata {
            ResultMetadata::CodeSearch(m) => {
                assert_eq!(m.matched_symbol.as_deref(), Some("my_function"));
            }
            other => panic!("expected CodeSearch metadata, got {other:?}"),
        }
    }

    #[test]
    fn test_extract_symbol_name_fn() {
        assert_eq!(
            extract_symbol_name("pub fn main() {}"),
            Some("main".to_string())
        );
    }

    #[test]
    fn test_extract_symbol_name_struct() {
        assert_eq!(
            extract_symbol_name("pub struct Foo {}"),
            Some("Foo".to_string())
        );
    }

    #[test]
    fn test_extract_symbol_name_enum() {
        assert_eq!(
            extract_symbol_name("enum Bar { A, B }"),
            Some("Bar".to_string())
        );
    }

    #[test]
    fn test_extract_symbol_name_trait() {
        assert_eq!(
            extract_symbol_name("pub trait MyTrait {}"),
            Some("MyTrait".to_string())
        );
    }

    #[test]
    fn test_extract_symbol_name_impl() {
        assert_eq!(extract_symbol_name("impl Foo {"), Some("Foo".to_string()));
    }

    #[test]
    fn test_extract_symbol_name_type() {
        assert_eq!(
            extract_symbol_name("type MyType = u32;"),
            Some("MyType".to_string())
        );
    }

    #[test]
    fn test_extract_symbol_name_crate_fn() {
        assert_eq!(
            extract_symbol_name("pub(crate) fn helper() {}"),
            Some("helper".to_string())
        );
    }

    #[test]
    fn test_extract_symbol_name_no_match() {
        assert_eq!(extract_symbol_name("let x = 5;"), None);
    }

    #[test]
    fn test_convert_respects_max_results() {
        let results: Vec<SourcegraphResult> = (0..5)
            .map(|i| SourcegraphResult {
                repository: Some(SourcegraphRepository {
                    name: Some("test/repo".to_string()),
                    url: None,
                }),
                path: Some(format!("src/f{i}.rs")),
                file_name: Some(format!("f{i}.rs")),
                branches: None,
                line_matches: vec![],
            })
            .collect();
        let out = convert(results, 2);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn test_max_results_zero_returns_empty() {
        let out = convert(vec![], 0);
        assert!(out.is_empty());
    }

    #[test]
    fn test_provider_descriptor_for_sourcegraph() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc =
            built_in_provider_descriptor("sourcegraph", true, false, true, false, None, None)
                .unwrap();
        assert_eq!(desc.id, "sourcegraph");
        assert_eq!(desc.display_name, "Sourcegraph");
        assert_eq!(desc.kind, crate::core::provider::ProviderKind::ApiKey);
        assert!(desc.requires_api_key);
        assert!(desc.configured);
        assert!(desc.enabled);
        assert!(!desc.default);
        assert!(desc.capabilities.supports_code_search);
        assert!(desc.capabilities.supports_repo_indexing);
        assert!(desc.capabilities.supports_path_filter);
        assert!(desc.capabilities.supports_language_filter);
        assert!(!desc.capabilities.supports_structured_changelog);
    }
}
