use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::{ResultMetadata, SearchResult};

const ENGINE: &str = "semantic_scholar";
const BASE_URL: &str = "https://api.semanticscholar.org/graph/v1";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const SNIPPET_MAX_CHARS: usize = 500;

#[derive(Debug, Deserialize)]
struct SemanticScholarResponse {
    #[serde(default)]
    data: Vec<SemanticScholarPaper>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct SemanticScholarPaper {
    #[serde(default)]
    #[allow(dead_code)]
    paper_id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    external_ids: Option<SemanticScholarExternalIds>,
    #[serde(default)]
    authors: Vec<SemanticScholarAuthor>,
    #[serde(default)]
    year: Option<u32>,
    #[serde(default)]
    venue: Option<String>,
    #[serde(default)]
    abstract_text: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    citation_count: Option<u64>,
    #[serde(default)]
    openAccessPdf: Option<SemanticScholarOaPdf>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct SemanticScholarExternalIds {
    #[serde(default)]
    DOI: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SemanticScholarAuthor {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SemanticScholarOaPdf {
    #[serde(default)]
    url: Option<String>,
}

pub async fn search(
    client: &Client,
    query: &str,
    max_results: usize,
    timeout: Duration,
    api_key: Option<&str>,
) -> Result<Vec<SearchResult>, EngineError> {
    if max_results == 0 {
        return Ok(Vec::new());
    }

    let fields =
        "paperId,title,externalIds,authors,year,venue,abstract,url,citationCount,openAccessPdf";
    let url = format!(
        "{}/paper/search?query={}&fields={}&limit={}",
        BASE_URL,
        urlencoding::encode(query),
        fields,
        max_results,
    );

    let response = tokio::time::timeout(timeout, {
        let mut req = client.get(&url).header("Accept", "application/json");
        if let Some(key) = api_key {
            req = req.header("x-api-key", key);
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

    let parsed: SemanticScholarResponse =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;

    let mut out = Vec::with_capacity(max_results.min(parsed.data.len()));
    for paper in parsed.data {
        if out.len() >= max_results {
            break;
        }
        if let Some(card) = convert_paper(paper) {
            out.push(card);
        }
    }

    Ok(out)
}

fn convert_paper(paper: SemanticScholarPaper) -> Option<SearchResult> {
    let title = paper.title?.trim().to_string();
    if title.is_empty() {
        return None;
    }

    let authors: Vec<String> = paper
        .authors
        .iter()
        .filter_map(|a| a.name.as_deref())
        .map(|s| s.to_string())
        .collect();

    let venue = paper.venue.as_deref().unwrap_or("");
    let year = paper.year.map(|y| y.to_string()).unwrap_or_default();

    let doi_url = paper
        .external_ids
        .as_ref()
        .and_then(|ids| ids.DOI.as_ref())
        .map(|doi| format!("https://doi.org/{doi}"))
        .unwrap_or_default();

    let oa_url = paper
        .openAccessPdf
        .as_ref()
        .and_then(|pdf| pdf.url.as_deref())
        .unwrap_or("");

    let paper_url = paper.url.as_deref().unwrap_or("");

    let url = if !oa_url.is_empty() {
        oa_url.to_string()
    } else if !doi_url.is_empty() {
        doi_url
    } else if !paper_url.is_empty() {
        paper_url.to_string()
    } else {
        return None;
    };

    let snippet = paper
        .abstract_text
        .as_ref()
        .map(|s| truncate(s, SNIPPET_MAX_CHARS));

    let citation_count = paper.citation_count.unwrap_or(0);
    let year_label = if !year.is_empty() {
        format!(" ({year})")
    } else {
        String::new()
    };
    let author_label = if !authors.is_empty() {
        if authors.len() > 3 {
            format!(", {} et al.", authors[0])
        } else {
            format!(", {}", authors.join(", "))
        }
    } else {
        String::new()
    };
    let venue_label = if !venue.is_empty() {
        format!(", {venue}")
    } else {
        String::new()
    };
    let citation_label = if citation_count > 0 {
        format!(", cited by {citation_count}")
    } else {
        String::new()
    };

    let enriched_title = format!("{title}{author_label}{year_label}{venue_label}{citation_label}");

    Some(SearchResult {
        title: enriched_title,
        url,
        snippet,
        source_engine: ENGINE.to_string(),
        metadata: ResultMetadata::None,
    })
}

fn truncate(s: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let char_len = s.chars().count();
    if char_len <= max_chars {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max_chars).collect();
    match truncated.rfind(char::is_whitespace) {
        Some(pos) if pos > 0 => truncated[..pos].to_string(),
        _ => truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate("hello", 100), "hello");
    }

    #[test]
    fn test_truncate_at_word_boundary() {
        assert_eq!(truncate("hello world foo bar", 11), "hello");
    }

    #[test]
    fn test_truncate_zero_max() {
        assert_eq!(truncate("anything", 0), "");
    }

    #[test]
    fn test_convert_paper_returns_none_for_missing_title() {
        let paper = SemanticScholarPaper {
            paper_id: None,
            title: None,
            external_ids: None,
            authors: vec![],
            year: None,
            venue: None,
            abstract_text: None,
            url: None,
            citation_count: None,
            openAccessPdf: None,
        };
        assert!(convert_paper(paper).is_none());
    }

    #[test]
    fn test_convert_paper_enriched_title() {
        let paper = SemanticScholarPaper {
            paper_id: Some("abc123".to_string()),
            title: Some("Test Paper".to_string()),
            external_ids: Some(SemanticScholarExternalIds {
                DOI: Some("10.1234/test".to_string()),
            }),
            authors: vec![SemanticScholarAuthor {
                name: Some("Alice Smith".to_string()),
            }],
            year: Some(2024),
            venue: Some("Nature".to_string()),
            abstract_text: Some("This is an abstract.".to_string()),
            url: Some("https://www.semanticscholar.org/paper/abc123".to_string()),
            citation_count: Some(42),
            openAccessPdf: Some(SemanticScholarOaPdf {
                url: Some("https://example.com/paper.pdf".to_string()),
            }),
        };
        let result = convert_paper(paper).unwrap();
        assert!(result.title.contains("Test Paper"));
        assert!(result.title.contains("Alice Smith"));
        assert!(result.title.contains("2024"));
        assert!(result.title.contains("Nature"));
        assert!(result.title.contains("42"));
        assert_eq!(result.url, "https://example.com/paper.pdf");
        assert_eq!(result.source_engine, "semantic_scholar");
    }

    #[test]
    fn test_convert_paper_many_authors_truncates() {
        let authors: Vec<SemanticScholarAuthor> = (0..5)
            .map(|i| SemanticScholarAuthor {
                name: Some(format!("Author {i}")),
            })
            .collect();
        let paper = SemanticScholarPaper {
            paper_id: None,
            title: Some("Paper".to_string()),
            external_ids: None,
            authors,
            year: None,
            venue: None,
            abstract_text: None,
            url: Some("https://example.com".to_string()),
            citation_count: None,
            openAccessPdf: None,
        };
        let result = convert_paper(paper).unwrap();
        assert!(result.title.contains("et al."));
    }

    #[test]
    fn test_convert_paper_doi_fallback() {
        let paper = SemanticScholarPaper {
            paper_id: None,
            title: Some("Paper".to_string()),
            external_ids: Some(SemanticScholarExternalIds {
                DOI: Some("10.1234/test".to_string()),
            }),
            authors: vec![],
            year: None,
            venue: None,
            abstract_text: None,
            url: Some("https://www.semanticscholar.org/paper/xyz".to_string()),
            citation_count: None,
            openAccessPdf: None,
        };
        let result = convert_paper(paper).unwrap();
        assert_eq!(result.url, "https://doi.org/10.1234/test");
    }

    #[test]
    fn test_convert_paper_falls_back_to_semanticscholar_url() {
        let paper = SemanticScholarPaper {
            paper_id: None,
            title: Some("Paper".to_string()),
            external_ids: None,
            authors: vec![],
            year: None,
            venue: None,
            abstract_text: None,
            url: Some("https://www.semanticscholar.org/paper/xyz".to_string()),
            citation_count: None,
            openAccessPdf: None,
        };
        let result = convert_paper(paper).unwrap();
        assert_eq!(result.url, "https://www.semanticscholar.org/paper/xyz");
    }

    #[test]
    fn test_semantic_scholar_provider_descriptor() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc =
            built_in_provider_descriptor("semantic_scholar", true, false, true, false, None, None)
                .unwrap();
        assert_eq!(desc.id, "semantic_scholar");
        assert_eq!(desc.display_name, "Semantic Scholar");
        assert_eq!(desc.kind, crate::core::provider::ProviderKind::ApiKey);
        assert!(desc.requires_api_key);
        assert!(desc.configured);
        assert!(desc.enabled);
        assert!(!desc.default);
        assert!(desc.capabilities.supports_scholarly_search);
        assert!(desc.capabilities.supports_doi_lookup);
    }
}
