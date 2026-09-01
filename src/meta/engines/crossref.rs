use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::{ResultMetadata, SearchResult};

const ENGINE: &str = "crossref";
const BASE_URL: &str = "https://api.crossref.org";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const SNIPPET_MAX_CHARS: usize = 500;
const EMAIL: &str = "eggsearch@example.com";

#[derive(Debug, Deserialize)]
struct CrossrefResponse {
    message: CrossrefMessage,
}

#[derive(Debug, Deserialize)]
struct CrossrefMessage {
    #[serde(default)]
    items: Vec<CrossrefWork>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct CrossrefWork {
    #[serde(default)]
    title: Vec<String>,
    #[serde(default)]
    DOI: Option<String>,
    #[serde(default)]
    author: Vec<CrossrefAuthor>,
    #[serde(default)]
    published: Option<CrossrefDate>,
    #[serde(default)]
    container_title: Vec<String>,
    #[serde(default)]
    abstract_html: Option<String>,
    #[serde(default)]
    URL: Option<String>,
    #[serde(default)]
    link: Vec<CrossrefLink>,
}

#[derive(Debug, Deserialize)]
struct CrossrefAuthor {
    #[serde(default)]
    given: Option<String>,
    #[serde(default)]
    family: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CrossrefDate {
    #[serde(default)]
    #[allow(dead_code)]
    date_parts: Vec<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct CrossrefLink {
    #[serde(default)]
    URL: Option<String>,
    #[serde(default)]
    content_type: Option<String>,
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
        "{}/works?query={}&rows={}",
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

    let parsed: CrossrefResponse =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;

    let mut out = Vec::with_capacity(max_results.min(parsed.message.items.len()));
    for work in parsed.message.items {
        if out.len() >= max_results {
            break;
        }
        if let Some(card) = convert_work(work) {
            out.push(card);
        }
    }

    Ok(out)
}

fn convert_work(work: CrossrefWork) -> Option<SearchResult> {
    let title = work.title.first()?.trim().to_string();
    if title.is_empty() {
        return None;
    }

    let authors: Vec<String> = work
        .author
        .iter()
        .map(|a| {
            let given = a.given.as_deref().unwrap_or("");
            let family = a.family.as_deref().unwrap_or("");
            match (given, family) {
                (g, f) if !g.is_empty() && !f.is_empty() => format!("{g} {f}"),
                (g, _) if !g.is_empty() => g.to_string(),
                (_, f) => f.to_string(),
            }
        })
        .collect();

    let venue = work
        .container_title
        .first()
        .map(|s| s.as_str())
        .unwrap_or("");

    let year = work
        .published
        .as_ref()
        .and_then(|d| d.date_parts.first()?.first())
        .and_then(|v| v.as_u64())
        .map(|y| y.to_string())
        .unwrap_or_default();

    let doi_url = work.DOI.as_deref().unwrap_or("");

    let oa_url = work
        .link
        .iter()
        .find(|l| l.content_type.as_deref() == Some("application/pdf"))
        .and_then(|l| l.URL.as_deref())
        .or(work.URL.as_deref());

    let url = if let Some(oa) = oa_url {
        oa.to_string()
    } else if !doi_url.is_empty() {
        format!("https://doi.org/{doi_url}")
    } else {
        return None;
    };

    let snippet = work.abstract_html.as_ref().map(|s| {
        let cleaned = strip_html_tags(s);
        truncate(&cleaned, SNIPPET_MAX_CHARS)
    });

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

    let enriched_title = format!("{title}{author_label}{year_label}{venue_label}");

    Some(SearchResult {
        title: enriched_title,
        url,
        snippet,
        source_engine: ENGINE.to_string(),
        metadata: ResultMetadata::None,
    })
}

fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut inside_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => inside_tag = true,
            '>' => inside_tag = false,
            _ if !inside_tag => result.push(ch),
            _ => {}
        }
    }
    result
}

fn truncate(s: &str, max_chars: usize) -> String {
    crate::core::sanitize::truncate_at_word(s, max_chars)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html_tags() {
        assert_eq!(
            strip_html_tags("<p>Some <b>bold</b> text</p>"),
            "Some bold text"
        );
    }

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
    fn test_convert_work_returns_none_for_missing_title() {
        let work = CrossrefWork {
            title: vec![],
            DOI: None,
            author: vec![],
            published: None,
            container_title: vec![],
            abstract_html: None,
            URL: None,
            link: vec![],
        };
        assert!(convert_work(work).is_none());
    }

    #[test]
    fn test_convert_work_enriched_title() {
        let work = CrossrefWork {
            title: vec!["Test Paper".to_string()],
            DOI: Some("10.1234/test".to_string()),
            author: vec![CrossrefAuthor {
                given: Some("Alice".to_string()),
                family: Some("Smith".to_string()),
            }],
            published: Some(CrossrefDate {
                date_parts: vec![vec![serde_json::json!(2024)]],
            }),
            container_title: vec!["Nature".to_string()],
            abstract_html: Some("<p>Some abstract</p>".to_string()),
            URL: Some("https://example.com/paper".to_string()),
            link: vec![],
        };
        let result = convert_work(work).unwrap();
        assert!(result.title.contains("Test Paper"));
        assert!(result.title.contains("Alice Smith"));
        assert!(result.title.contains("2024"));
        assert!(result.title.contains("Nature"));
        assert_eq!(result.url, "https://example.com/paper");
        assert_eq!(result.source_engine, "crossref");
    }

    #[test]
    fn test_convert_work_many_authors_truncates() {
        let authors: Vec<CrossrefAuthor> = (0..5)
            .map(|i| CrossrefAuthor {
                given: Some(format!("A{i}")),
                family: Some(format!("B{i}")),
            })
            .collect();
        let work = CrossrefWork {
            title: vec!["Paper".to_string()],
            DOI: Some("10.1234/test".to_string()),
            author: authors,
            published: None,
            container_title: vec![],
            abstract_html: None,
            URL: None,
            link: vec![],
        };
        let result = convert_work(work).unwrap();
        assert!(result.title.contains("et al."));
    }

    #[test]
    fn test_convert_work_doi_fallback_url() {
        let work = CrossrefWork {
            title: vec!["Paper".to_string()],
            DOI: Some("10.1234/test".to_string()),
            author: vec![],
            published: None,
            container_title: vec![],
            abstract_html: None,
            URL: None,
            link: vec![],
        };
        let result = convert_work(work).unwrap();
        assert_eq!(result.url, "https://doi.org/10.1234/test");
    }

    #[test]
    fn test_crossref_provider_descriptor() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc =
            built_in_provider_descriptor("crossref", true, false, true, false, None, None).unwrap();
        assert_eq!(desc.id, "crossref");
        assert_eq!(desc.display_name, "Crossref");
        assert_eq!(desc.kind, crate::core::provider::ProviderKind::JsonApi);
        assert!(!desc.requires_api_key);
        assert!(desc.configured);
        assert!(desc.enabled);
        assert!(!desc.default);
        assert!(desc.capabilities.supports_scholarly_search);
        assert!(desc.capabilities.supports_doi_lookup);
    }
}
