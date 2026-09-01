use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::{ResultMetadata, SearchResult};

const ENGINE: &str = "openalex";
const BASE_URL: &str = "https://api.openalex.org";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const SNIPPET_MAX_CHARS: usize = 500;
const EMAIL: &str = "eggsearch@example.com";

#[derive(Debug, Deserialize)]
struct OpenAlexResponse {
    results: Vec<OpenAlexWork>,
}

#[derive(Debug, Deserialize)]
struct OpenAlexWork {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    doi: Option<String>,
    #[serde(default)]
    authorships: Vec<OpenAlexAuthorship>,
    #[serde(default)]
    publication_year: Option<u32>,
    #[serde(default)]
    primary_location: Option<OpenAlexPrimaryLocation>,
    #[serde(default)]
    abstract_inverted_index: Option<serde_json::Value>,
    #[serde(default)]
    open_access: Option<OpenAlexOpenAccess>,
    #[serde(default)]
    cited_by_count: Option<u64>,
    #[serde(default)]
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAlexAuthorship {
    #[serde(default)]
    author: Option<OpenAlexAuthor>,
}

#[derive(Debug, Deserialize)]
struct OpenAlexAuthor {
    #[serde(default)]
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAlexPrimaryLocation {
    #[serde(default)]
    source: Option<OpenAlexSource>,
}

#[derive(Debug, Deserialize)]
struct OpenAlexSource {
    #[serde(default)]
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAlexOpenAccess {
    #[serde(default)]
    #[allow(dead_code)]
    is_oa: Option<bool>,
    #[serde(default)]
    oa_url: Option<String>,
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

    let url = format!(
        "{}/works?search={}&per-page={}",
        BASE_URL,
        urlencoding::encode(query),
        max_results,
    );

    let bytes = tokio::time::timeout(timeout, async {
        let resp = client
            .get(&url)
            .header("User-Agent", format!("eggsearch/1.0 (mailto:{EMAIL})"))
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| EngineError::Http {
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

    let parsed: OpenAlexResponse =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;

    let mut out = Vec::with_capacity(max_results.min(parsed.results.len()));
    for work in parsed.results {
        if out.len() >= max_results {
            break;
        }
        if let Some(card) = convert_work(work) {
            out.push(card);
        }
    }

    Ok(out)
}

fn convert_work(work: OpenAlexWork) -> Option<SearchResult> {
    let title = work.title?.trim().to_string();
    if title.is_empty() {
        return None;
    }

    let authors: Vec<String> = work
        .authorships
        .iter()
        .filter_map(|a| a.author.as_ref()?.display_name.as_deref())
        .map(|s| s.to_string())
        .collect();

    let venue = work
        .primary_location
        .as_ref()
        .and_then(|loc| loc.source.as_ref()?.display_name.as_deref())
        .unwrap_or("");

    let year = work
        .publication_year
        .map(|y| y.to_string())
        .unwrap_or_default();

    let doi_url = work.doi.as_deref().unwrap_or("");

    let oa_url = work
        .open_access
        .as_ref()
        .and_then(|oa| oa.oa_url.as_deref())
        .unwrap_or("");

    let url = if !oa_url.is_empty() {
        oa_url.to_string()
    } else if !doi_url.is_empty() {
        doi_url.to_string()
    } else {
        work.id.as_deref().unwrap_or("").to_string()
    };

    if url.is_empty() {
        return None;
    }

    let snippet = work
        .abstract_inverted_index
        .as_ref()
        .and_then(|idx| reconstruct_abstract(idx).map(|s| truncate(&s, SNIPPET_MAX_CHARS)));

    let citation_count = work.cited_by_count.unwrap_or(0);
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

fn reconstruct_abstract(inverted_index: &serde_json::Value) -> Option<String> {
    let map = inverted_index.as_object()?;
    let mut word_positions: Vec<(u64, String)> = Vec::new();
    for (word, positions) in map {
        let pos_arr = positions.as_array()?;
        for pos in pos_arr {
            let p = pos.as_u64()?;
            word_positions.push((p, word.clone()));
        }
    }
    word_positions.sort_by_key(|(p, _)| *p);
    let abstract_text: Vec<&str> = word_positions.iter().map(|(_, w)| w.as_str()).collect();
    Some(abstract_text.join(" "))
}

fn truncate(s: &str, max_chars: usize) -> String {
    crate::core::sanitize::truncate_at_word(s, max_chars)
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
    fn test_reconstruct_abstract() {
        let json = serde_json::json!({
            "the": [0],
            "cat": [1],
            "sat": [2]
        });
        assert_eq!(reconstruct_abstract(&json).unwrap(), "the cat sat");
    }

    #[test]
    fn test_reconstruct_abstract_out_of_order() {
        let json = serde_json::json!({
            "cat": [1],
            "the": [0],
            "sat": [2]
        });
        assert_eq!(reconstruct_abstract(&json).unwrap(), "the cat sat");
    }

    #[test]
    fn test_convert_work_returns_none_for_missing_title() {
        let work = OpenAlexWork {
            title: None,
            doi: None,
            authorships: vec![],
            publication_year: None,
            primary_location: None,
            abstract_inverted_index: None,
            open_access: None,
            cited_by_count: None,
            id: None,
        };
        assert!(convert_work(work).is_none());
    }

    #[test]
    fn test_convert_work_enriched_title() {
        let work = OpenAlexWork {
            title: Some("Test Paper".to_string()),
            doi: Some("https://doi.org/10.1234/test".to_string()),
            authorships: vec![OpenAlexAuthorship {
                author: Some(OpenAlexAuthor {
                    display_name: Some("Alice Smith".to_string()),
                }),
            }],
            publication_year: Some(2024),
            primary_location: Some(OpenAlexPrimaryLocation {
                source: Some(OpenAlexSource {
                    display_name: Some("Nature".to_string()),
                }),
            }),
            abstract_inverted_index: None,
            open_access: Some(OpenAlexOpenAccess {
                is_oa: Some(true),
                oa_url: Some("https://example.com/paper".to_string()),
            }),
            cited_by_count: Some(42),
            id: Some("W1234".to_string()),
        };
        let result = convert_work(work).unwrap();
        assert!(result.title.contains("Test Paper"));
        assert!(result.title.contains("Alice Smith"));
        assert!(result.title.contains("2024"));
        assert!(result.title.contains("Nature"));
        assert!(result.title.contains("42"));
        assert_eq!(result.url, "https://example.com/paper");
        assert_eq!(result.source_engine, "openalex");
    }

    #[test]
    fn test_convert_work_falls_back_to_doi_url() {
        let work = OpenAlexWork {
            title: Some("Test".to_string()),
            doi: Some("https://doi.org/10.1234/test".to_string()),
            authorships: vec![],
            publication_year: None,
            primary_location: None,
            abstract_inverted_index: None,
            open_access: Some(OpenAlexOpenAccess {
                is_oa: Some(false),
                oa_url: None,
            }),
            cited_by_count: None,
            id: Some("W1234".to_string()),
        };
        let result = convert_work(work).unwrap();
        assert_eq!(result.url, "https://doi.org/10.1234/test");
    }

    #[test]
    fn test_convert_work_many_authors_truncates() {
        let authors: Vec<OpenAlexAuthorship> = (0..5)
            .map(|i| OpenAlexAuthorship {
                author: Some(OpenAlexAuthor {
                    display_name: Some(format!("Author {i}")),
                }),
            })
            .collect();
        let work = OpenAlexWork {
            title: Some("Paper".to_string()),
            doi: None,
            authorships: authors,
            publication_year: None,
            primary_location: None,
            abstract_inverted_index: None,
            open_access: None,
            cited_by_count: None,
            id: Some("W1234".to_string()),
        };
        let result = convert_work(work).unwrap();
        assert!(result.title.contains("et al."));
    }

    #[test]
    fn test_openalex_provider_descriptor() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc =
            built_in_provider_descriptor("openalex", true, false, true, false, None, None).unwrap();
        assert_eq!(desc.id, "openalex");
        assert_eq!(desc.display_name, "OpenAlex");
        assert_eq!(desc.kind, crate::core::provider::ProviderKind::JsonApi);
        assert!(!desc.requires_api_key);
        assert!(desc.configured);
        assert!(desc.enabled);
        assert!(!desc.default);
        assert!(desc.capabilities.supports_scholarly_search);
        assert!(desc.capabilities.supports_doi_lookup);
        assert!(desc.capabilities.supports_result_timestamps);
    }
}
