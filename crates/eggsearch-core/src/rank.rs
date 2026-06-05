//! Reciprocal rank fusion across multiple providers.

use std::collections::HashMap;

use crate::result::SearchResult;
use crate::source_card::{make_source_card, SourceCard};

/// Default reciprocal rank fusion constant.
pub const DEFAULT_RRF_K: f32 = 60.0;

/// Combine ranked result lists from multiple providers using reciprocal
/// rank fusion. Returns a merged, re-ranked vector of source cards.
///
/// Inputs are ordered: index in the outer vec is provider index, and
/// within each list, index is rank.
///
/// Final order is by descending fusion score; ties are broken by the
/// average input rank to keep results stable.
pub fn reciprocal_rank_fusion(
    ranked_lists: &[Vec<SearchResult>],
    k: f32,
    max_results: usize,
) -> Vec<SourceCard> {
    let mut score: HashMap<String, f32> = HashMap::new();
    let mut rank_sum: HashMap<String, u32> = HashMap::new();
    let mut by_key: HashMap<String, SearchResult> = HashMap::new();

    for list in ranked_lists {
        for (rank, r) in list.iter().enumerate() {
            let key = r.url.to_string();
            let weight = 1.0 / (k + rank as f32 + 1.0);
            *score.entry(key.clone()).or_insert(0.0) += weight;
            *rank_sum.entry(key.clone()).or_insert(0) += rank as u32;
            by_key
                .entry(key)
                .and_modify(|existing| {
                    // Promote trust if a more trusted variant exists.
                    use crate::result::TrustLevel::*;
                    if matches!(
                        (existing.trust_level, r.trust_level),
                        (ExternalUntrusted, LocalCachedExternal)
                            | (ExternalUntrusted, LocalTrusted)
                            | (LocalCachedExternal, LocalTrusted)
                    ) {
                        existing.trust_level = r.trust_level;
                    }
                })
                .or_insert_with(|| r.clone());
        }
    }

    let mut scored: Vec<(String, f32, u32)> = score
        .into_iter()
        .map(|(k, s)| {
            let rs = *rank_sum.get(&k).unwrap_or(&0);
            (k, s, rs)
        })
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.2.cmp(&b.2)));

    scored
        .into_iter()
        .take(max_results)
        .filter_map(|(key, sc, _)| by_key.remove(&key).map(|r| (r, sc)))
        .map(|(mut r, sc)| {
            r.score = Some(sc);
            make_source_card(&r)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::result::{SourceKind, TrustLevel};
    use url::Url;

    fn r(title: &str, url: &str, provider: &str) -> SearchResult {
        SearchResult {
            title: title.to_string(),
            url: Url::parse(url).unwrap(),
            snippet: None,
            published_at: None,
            rank: 0,
            score: None,
            provider_id: provider.to_string(),
            source_kind: SourceKind::Web,
            trust_level: TrustLevel::ExternalUntrusted,
        }
    }

    #[test]
    fn fusion_merges_and_ranks() {
        let a = vec![
            r("A1", "https://a.com/1", "p1"),
            r("A2", "https://a.com/2", "p1"),
        ];
        let b = vec![
            r("A1", "https://a.com/1", "p2"),
            r("A3", "https://a.com/3", "p2"),
        ];
        let out = reciprocal_rank_fusion(&[a, b], DEFAULT_RRF_K, 10);
        // A1 appears in both lists and should win.
        assert!(!out.is_empty());
        assert_eq!(out[0].title, "A1");
    }

    #[test]
    fn fusion_caps_results() {
        let a = (0..20)
            .map(|i| r(&format!("T{i}"), &format!("https://a.com/{i}"), "p1"))
            .collect();
        let out = reciprocal_rank_fusion(&[a], DEFAULT_RRF_K, 5);
        assert_eq!(out.len(), 5);
    }
}
