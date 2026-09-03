use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::error::EngineError;
use super::models::{EngineRetrievalMetadata, EngineSearchBatch, ScopeIndexStatus, SearchResult};
use super::request::EngineSearchRequest;
use crate::core::query::SearchIntent;

const ENGINE: &str = "firecrawl_developer";
const DEFAULT_URL: &str = "https://api.firecrawl.dev/v2/search/developer";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const FIRECRAWL_MAX_K: usize = 20;
const DEFAULT_PASSAGES: usize = 2;
const MAX_PASSAGES: usize = 3;

#[derive(Debug, Serialize)]
struct DeveloperRequest {
    query: String,
    k: usize,
    passages: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    types: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repos: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct DeveloperResponse {
    #[serde(default)]
    results: Vec<DeveloperResult>,
    #[serde(default)]
    repos: Vec<RepoEcho>,
    #[serde(default)]
    sources: Vec<SourceEcho>,
}

#[derive(Debug, Deserialize)]
struct DeveloperResult {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    passages: Vec<Passage>,
}

#[derive(Debug, Deserialize)]
struct Passage {
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RepoEcho {
    #[serde(default)]
    repo: Option<String>,
    #[serde(default)]
    indexed: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct SourceEcho {
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    indexed: Option<bool>,
}

fn resolve_url(base_url: Option<&str>) -> String {
    match base_url {
        Some(u) if !u.is_empty() => u.to_string(),
        _ => DEFAULT_URL.to_string(),
    }
}

fn clamp_k(max_results: usize) -> usize {
    max_results.clamp(1, FIRECRAWL_MAX_K)
}

fn resolve_passages(excerpt_count: usize) -> usize {
    if excerpt_count == 0 {
        DEFAULT_PASSAGES
    } else {
        excerpt_count.clamp(1, MAX_PASSAGES)
    }
}

fn map_types(intent: SearchIntent) -> Option<Vec<String>> {
    match intent {
        SearchIntent::Docs => Some(vec!["doc".to_string(), "readme".to_string()]),
        SearchIntent::Issues => Some(vec!["issue".to_string(), "pull_request".to_string()]),
        _ => None,
    }
}

fn fallback_title(url: &str) -> String {
    let normalized = crate::core::sanitize::normalize_whitespace(url);
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        url.to_string()
    } else {
        trimmed.to_string()
    }
}

fn parse_owner_repo_number(url: &str) -> Option<(String, String, Option<u64>)> {
    let parsed = url::Url::parse(url).ok()?;
    let segments: Vec<&str> = parsed
        .path()
        .trim_start_matches('/')
        .trim_end_matches('/')
        .split('/')
        .collect();
    if segments.len() < 2 {
        return None;
    }
    let owner = segments[0].to_string();
    let repo = segments[1].to_string();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    let number = segments
        .iter()
        .skip(2)
        .filter_map(|s| {
            if *s == "issues" || *s == "pull" {
                None
            } else {
                s.parse::<u64>().ok()
            }
        })
        .next();
    Some((owner, repo, number))
}

fn artifact_kind(id: Option<&str>, kind: Option<&str>) -> String {
    if let Some(k) = kind {
        let k = k.trim();
        if !k.is_empty() {
            return k.to_string();
        }
    }
    if let Some(raw) = id {
        if let Some((prefix, _)) = raw.split_once(':') {
            let prefix = prefix.trim();
            if !prefix.is_empty() {
                return prefix.to_string();
            }
        }
    }
    String::new()
}

fn convert(
    raw: Vec<DeveloperResult>,
    max_results: usize,
    excerpt_count: usize,
) -> Vec<SearchResult> {
    let mut out = Vec::with_capacity(max_results.min(raw.len()));
    for r in raw {
        if out.len() >= max_results {
            break;
        }
        let Some(url) = r.url else { continue };
        if !super::is_http_url(&url) {
            continue;
        }
        let title = r
            .title
            .map(|t| crate::core::sanitize::normalize_whitespace(&t))
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| fallback_title(&url));
        if title.is_empty() {
            continue;
        }
        let kind = artifact_kind(r.id.as_deref(), r.kind.as_deref());
        let mut passage_texts: Vec<String> = Vec::new();
        if excerpt_count > 0 {
            for p in r.passages {
                if passage_texts.len() >= excerpt_count {
                    break;
                }
                let Some(text) = p.text else { continue };
                let text = crate::core::sanitize::normalize_whitespace(&text)
                    .trim()
                    .to_string();
                if text.is_empty() {
                    continue;
                }
                passage_texts.push(text);
            }
        } else {
            for p in &r.passages {
                if let Some(text) = &p.text {
                    let t = crate::core::sanitize::normalize_whitespace(text)
                        .trim()
                        .to_string();
                    if !t.is_empty() {
                        passage_texts.push(t);
                        break;
                    }
                }
            }
        }
        let snippet = passage_texts.first().cloned();
        let mut excerpts = Vec::new();
        if excerpt_count > 0 {
            for text in passage_texts.iter().take(excerpt_count) {
                excerpts.push(crate::core::source_card::SourceExcerpt {
                    text: text.clone(),
                    score: None,
                    provenance: crate::core::source_card::ExcerptProvenance::ProviderPassage,
                });
            }
        }
        let metadata = match kind.as_str() {
            "issue" => {
                let (owner, repo, number) = parse_owner_repo_number(&url)
                    .map(|(o, r, n)| (Some(o), Some(r), n))
                    .unwrap_or((None, None, None));
                super::models::ResultMetadata::Issue(crate::core::source_card::IssueMetadata {
                    owner,
                    repo,
                    number,
                    is_pull_request: Some(false),
                    ..Default::default()
                })
            }
            "pull_request" => {
                let (owner, repo, number) = parse_owner_repo_number(&url)
                    .map(|(o, r, n)| (Some(o), Some(r), n))
                    .unwrap_or((None, None, None));
                super::models::ResultMetadata::Issue(crate::core::source_card::IssueMetadata {
                    owner,
                    repo,
                    number,
                    is_pull_request: Some(true),
                    ..Default::default()
                })
            }
            _ => super::models::ResultMetadata::None,
        };
        out.push(SearchResult {
            title,
            url,
            snippet,
            source_engine: ENGINE.to_string(),
            metadata,
            excerpts,
            published_at: None,
        });
    }
    out
}

fn convert_metadata(resp: &DeveloperResponse) -> EngineRetrievalMetadata {
    let mut scope_index = Vec::new();
    for echo in &resp.repos {
        if let Some(repo) = &echo.repo {
            let repo = repo.trim();
            if repo.is_empty() {
                continue;
            }
            scope_index.push(ScopeIndexStatus {
                scope: repo.to_string(),
                indexed: echo.indexed.unwrap_or(true),
            });
        }
    }
    for echo in &resp.sources {
        if let Some(source) = &echo.source {
            let source = source.trim();
            if source.is_empty() {
                continue;
            }
            scope_index.push(ScopeIndexStatus {
                scope: source.to_string(),
                indexed: echo.indexed.unwrap_or(true),
            });
        }
    }
    EngineRetrievalMetadata { scope_index }
}

pub async fn search(
    client: &Client,
    api_key: Option<&str>,
    base_url: Option<&str>,
    request: &EngineSearchRequest,
) -> Result<EngineSearchBatch, EngineError> {
    if request.max_results == 0 {
        return Ok(EngineSearchBatch::from_results(Vec::new()));
    }
    let url = resolve_url(base_url);
    let k = clamp_k(request.max_results);
    let passages = resolve_passages(request.excerpt_count);
    let excerpt_cap = request
        .excerpt_count
        .min(crate::core::source_card::MAX_EXCERPT_REQUEST_COUNT);
    let effective_excerpts = if excerpt_cap == 0 {
        DEFAULT_PASSAGES.min(crate::core::source_card::MAX_EXCERPT_REQUEST_COUNT)
    } else {
        excerpt_cap
    };
    let types = map_types(request.intent);
    let repos = request.repo_scope.as_ref().map(|s| vec![s.slug()]);
    if let Some(ref repos) = repos {
        if let Some(ref types) = types {
            let has_repo_type = types
                .iter()
                .any(|t| t == "issue" || t == "pull_request" || t == "readme");
            if !has_repo_type {
                return Err(EngineError::Unsupported {
                    engine: ENGINE,
                    reason: "repos scope requires a repository result type".to_string(),
                });
            }
        }
        for slug in repos {
            if slug.trim().is_empty() || !slug.contains('/') {
                return Err(EngineError::Unsupported {
                    engine: ENGINE,
                    reason: "invalid repository scope".to_string(),
                });
            }
        }
    }
    let body = DeveloperRequest {
        query: request.query.clone(),
        k,
        passages,
        types,
        repos,
    };
    let timeout = request.timeout;
    let max_results = request.max_results;
    let bytes = tokio::time::timeout(timeout, async {
        let mut req = client
            .post(&url)
            .json(&body)
            .header("Accept", "application/json");
        if let Some(key) = api_key.filter(|k| !k.is_empty()) {
            req = req.header("Authorization", format!("Bearer {key}"));
        }
        let resp = req.send().await.map_err(|e| EngineError::Http {
            engine: ENGINE,
            source: e,
        })?;
        let status = resp.status();
        if !status.is_success() {
            return Err(EngineError::BadStatus {
                engine: ENGINE,
                status: status.as_u16(),
            });
        }
        super::read_bounded_body(resp, ENGINE, MAX_BODY_BYTES).await
    })
    .await
    .map_err(|_| EngineError::Timeout { engine: ENGINE })??;

    let parsed: DeveloperResponse =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;
    let metadata = convert_metadata(&parsed);
    Ok(EngineSearchBatch {
        results: convert(parsed.results, max_results, effective_excerpts),
        retrieval_metadata: metadata,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_k_bounds() {
        assert_eq!(clamp_k(0), 1);
        assert_eq!(clamp_k(5), 5);
        assert_eq!(clamp_k(100), FIRECRAWL_MAX_K);
    }

    #[test]
    fn resolve_passages_defaults_and_caps() {
        assert_eq!(resolve_passages(0), DEFAULT_PASSAGES);
        assert_eq!(resolve_passages(1), 1);
        assert_eq!(resolve_passages(3), 3);
        assert_eq!(resolve_passages(10), MAX_PASSAGES);
    }

    #[test]
    fn map_types_restricts_only_where_safe() {
        assert_eq!(
            map_types(SearchIntent::Docs),
            Some(vec!["doc".to_string(), "readme".to_string()])
        );
        assert_eq!(
            map_types(SearchIntent::Issues),
            Some(vec!["issue".to_string(), "pull_request".to_string()])
        );
        assert_eq!(map_types(SearchIntent::Web), None);
        assert_eq!(map_types(SearchIntent::Code), None);
        assert_eq!(map_types(SearchIntent::News), None);
    }

    #[test]
    fn fallback_title_uses_url() {
        assert_eq!(
            fallback_title("https://example.com/docs"),
            "https://example.com/docs"
        );
    }

    #[test]
    fn convert_handles_all_four_artifact_prefixes() {
        let raw = vec![
            DeveloperResult {
                id: Some("issue:tokio-rs/axum#123".to_string()),
                kind: Some("issue".to_string()),
                url: Some("https://github.com/tokio-rs/axum/issues/123".to_string()),
                title: Some("Bug report".to_string()),
                passages: vec![Passage {
                    text: Some("first passage".to_string()),
                }],
            },
            DeveloperResult {
                id: Some("pull_request:tokio-rs/axum#124".to_string()),
                kind: Some("pull_request".to_string()),
                url: Some("https://github.com/tokio-rs/axum/pull/124".to_string()),
                title: Some("Fix".to_string()),
                passages: vec![Passage {
                    text: Some("pr passage".to_string()),
                }],
            },
            DeveloperResult {
                id: Some("readme:tokio-rs/axum".to_string()),
                kind: Some("readme".to_string()),
                url: Some("https://github.com/tokio-rs/axum".to_string()),
                title: Some("axum".to_string()),
                passages: vec![Passage {
                    text: Some("readme passage".to_string()),
                }],
            },
            DeveloperResult {
                id: Some("doc:example".to_string()),
                kind: Some("doc".to_string()),
                url: Some("https://example.com/docs".to_string()),
                title: None,
                passages: vec![Passage {
                    text: Some("doc passage".to_string()),
                }],
            },
        ];
        let out = convert(raw, 10, 2);
        assert_eq!(out.len(), 4);
        assert!(matches!(
            out[0].metadata,
            super::super::models::ResultMetadata::Issue(_)
        ));
        assert!(matches!(
            out[1].metadata,
            super::super::models::ResultMetadata::Issue(_)
        ));
        assert_eq!(out[3].title, "https://example.com/docs");
        for card in &out {
            assert!(!card.excerpts.is_empty());
            assert!(matches!(
                card.excerpts[0].provenance,
                crate::core::source_card::ExcerptProvenance::ProviderPassage
            ));
        }
    }

    #[test]
    fn convert_missing_doc_title_uses_deterministic_fallback() {
        let raw = vec![DeveloperResult {
            id: Some("doc:example".to_string()),
            kind: Some("doc".to_string()),
            url: Some("https://example.com/a".to_string()),
            title: None,
            passages: vec![],
        }];
        let a = convert(raw, 10, 0);
        let raw2 = vec![DeveloperResult {
            id: Some("doc:example".to_string()),
            kind: Some("doc".to_string()),
            url: Some("https://example.com/a".to_string()),
            title: Some(String::new()),
            passages: vec![],
        }];
        let b = convert(raw2, 10, 0);
        assert_eq!(a[0].title, b[0].title);
        assert_eq!(a[0].title, "https://example.com/a");
    }

    #[test]
    fn convert_respects_excerpt_bounds() {
        let raw = vec![DeveloperResult {
            id: Some("issue:o/r#1".to_string()),
            kind: Some("issue".to_string()),
            url: Some("https://github.com/o/r/issues/1".to_string()),
            title: Some("T".to_string()),
            passages: (0..10)
                .map(|i| Passage {
                    text: Some(format!("passage {i}")),
                })
                .collect(),
        }];
        let out = convert(raw, 10, 3);
        assert_eq!(out[0].excerpts.len(), 3);
    }

    #[test]
    fn convert_metadata_preserves_indexed_false() {
        let resp = DeveloperResponse {
            results: vec![],
            repos: vec![RepoEcho {
                repo: Some("tokio-rs/axum".to_string()),
                indexed: Some(false),
            }],
            sources: vec![],
        };
        let meta = convert_metadata(&resp);
        assert!(meta.has_unindexed());
        assert_eq!(meta.unindexed_scopes(), vec!["tokio-rs/axum"]);
    }

    #[test]
    fn descriptor_flags_do_not_claim_code_search() {
        let desc = crate::core::provider::built_in_provider_descriptor(
            "firecrawl_developer",
            true,
            false,
            true,
            true,
            None,
            None,
        )
        .expect("descriptor");
        assert!(!desc.requires_api_key);
        assert!(desc.capabilities.supports_issue_search);
        assert!(desc.capabilities.supports_repo_filter);
        assert!(!desc.capabilities.supports_code_search);
        assert!(!desc.capabilities.supports_release_search);
        assert!(!desc.capabilities.supports_scholarly_search);
        assert!(!desc.capabilities.supports_repo_indexing);
    }
}
