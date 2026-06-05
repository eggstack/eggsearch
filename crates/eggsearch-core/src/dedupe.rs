//! Deduplication of search results across providers.

use std::collections::HashMap;

use url::Url;

use crate::normalize::canonicalize;
use crate::result::SearchResult;

/// Deduplicate results by canonical URL.
///
/// The first occurrence (in input order) wins. When duplicates are found,
/// we keep the highest-ranked entry but merge any non-empty snippet from
/// later entries that have a better (longer) snippet.
pub fn dedupe_by_url(results: Vec<SearchResult>) -> Vec<SearchResult> {
    let mut by_key: HashMap<String, usize> = HashMap::new();
    let mut out: Vec<SearchResult> = Vec::with_capacity(results.len());

    for r in results {
        let key = canonical_key(&r.url);
        if let Some(&idx) = by_key.get(&key) {
            if let Some(existing) = out.get_mut(idx) {
                // Merge snippet if existing is empty or shorter.
                match (&existing.snippet, &r.snippet) {
                    (None, Some(s)) => existing.snippet = Some(s.clone()),
                    (Some(e), Some(s)) if s.len() > e.len() => {
                        existing.snippet = Some(s.clone());
                    }
                    _ => {}
                }
                // Prefer the earlier provider's source_kind (first wins).
            }
        } else {
            by_key.insert(key, out.len());
            out.push(r);
        }
    }

    out
}

/// Similarity-based dedupe using simple title Jaccard over lowercased word
/// tokens. Two results are considered duplicates if their title token sets
/// overlap by >= the given threshold (0.0..=1.0).
pub fn dedupe_by_similar_title(results: Vec<SearchResult>, threshold: f32) -> Vec<SearchResult> {
    let mut out: Vec<SearchResult> = Vec::with_capacity(results.len());
    let mut token_sets: Vec<std::collections::HashSet<String>> = Vec::new();

    for r in results {
        let tokens = title_tokens(&r.title);
        let mut is_dup = false;
        for (i, prev) in token_sets.iter().enumerate() {
            if tokens.is_empty() || prev.is_empty() {
                continue;
            }
            let intersection = tokens.intersection(prev).count();
            let union = tokens.union(prev).count();
            if union == 0 {
                continue;
            }
            let jaccard = intersection as f32 / union as f32;
            if jaccard >= threshold {
                is_dup = true;
                // If the new one has a better snippet, swap it in.
                if let Some(existing) = out.get_mut(i) {
                    match (&existing.snippet, &r.snippet) {
                        (None, Some(s)) => existing.snippet = Some(s.clone()),
                        (Some(e), Some(s)) if s.len() > e.len() => {
                            existing.snippet = Some(s.clone());
                        }
                        _ => {}
                    }
                }
                break;
            }
        }
        if !is_dup {
            token_sets.push(tokens);
            out.push(r);
        }
    }
    out
}

/// Cap the number of results from any single domain unless the query
/// explicitly scopes to that domain.
pub fn cap_per_domain(results: Vec<SearchResult>, cap: usize) -> Vec<SearchResult> {
    if cap == 0 {
        return results;
    }
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut out: Vec<SearchResult> = Vec::with_capacity(results.len());
    for r in results {
        let key = r.domain().unwrap_or_else(|| "<no-domain>".to_string());
        let c = counts.entry(key).or_insert(0);
        if *c < cap {
            out.push(r);
            *c += 1;
        }
    }
    out
}

fn canonical_key(url: &Url) -> String {
    canonicalize(url.as_str())
        .map(|u| u.to_string())
        .unwrap_or_else(|| url.to_string())
}

fn title_tokens(title: &str) -> std::collections::HashSet<String> {
    title
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 3)
        .map(|t| t.to_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::result::{SourceKind, TrustLevel};
    use chrono::Utc;

    fn r(title: &str, url: &str, snippet: Option<&str>) -> SearchResult {
        SearchResult {
            title: title.to_string(),
            url: Url::parse(url).unwrap(),
            snippet: snippet.map(String::from),
            published_at: None,
            rank: 0,
            score: None,
            provider_id: "test".to_string(),
            source_kind: SourceKind::Web,
            trust_level: TrustLevel::ExternalUntrusted,
        }
    }

    #[test]
    fn dedupe_merges_equivalent_urls() {
        let v = vec![
            r("A", "https://Example.com/x?utm_source=t", Some("short")),
            r("A", "https://example.com/x", Some("a longer snippet here")),
        ];
        let out = dedupe_by_url(v);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].snippet.as_deref(), Some("a longer snippet here"));
    }

    #[test]
    fn dedupe_title_works() {
        let _ = Utc::now();
        let v = vec![
            r("Axum Middleware Tutorial", "https://a.com/x", None),
            r("Axum Middleware Tutorial!", "https://b.com/y", None),
        ];
        let out = dedupe_by_similar_title(v, 0.5);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn cap_per_domain_limits() {
        let v = vec![
            r("1", "https://a.com/x", None),
            r("2", "https://a.com/y", None),
            r("3", "https://a.com/z", None),
            r("4", "https://b.com/x", None),
        ];
        let out = cap_per_domain(v, 2);
        assert_eq!(out.len(), 3);
        assert_eq!(out.iter().filter(|r| r.url.host_str() == Some("a.com")).count(), 2);
    }
}
