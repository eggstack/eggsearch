//! Deterministic query-focused chunk selection for `web_fetch`.
//!
//! Ranks the already-extracted [`FetchDocument`](crate::core::document::FetchDocument)
//! chunks against a caller focus query using dependency-free lexical
//! scoring. No embeddings, no model calls, no extra URL traversal.

use std::collections::HashSet;

use crate::core::document::{DocumentChunk, FetchDocument};
use crate::core::fetch::FocusedFetchSelection;

/// Tokenize text into lowercase alphanumeric tokens.
fn tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_ascii_lowercase())
        .collect()
}

/// Normalize text for phrase comparison: lowercase, single spaces.
fn normalized(text: &str) -> String {
    crate::core::sanitize::normalize_whitespace(text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

/// Score one chunk against the focus query.
///
/// Combines normalized token overlap, exact-phrase boost, heading-path
/// overlap, and case-sensitive exact-symbol boost for code-like
/// queries. Returns `0.0` when nothing matches.
fn score_chunk(
    query_tokens: &[String],
    query_norm: &str,
    query_raw: &str,
    chunk: &DocumentChunk,
) -> f64 {
    if query_tokens.is_empty() {
        return 0.0;
    }
    let chunk_tokens: HashSet<String> = tokens(&chunk.text).into_iter().collect();
    if chunk_tokens.is_empty() {
        return 0.0;
    }
    let query_set: HashSet<String> = query_tokens.iter().cloned().collect();
    let overlap = query_set.intersection(&chunk_tokens).count();
    if overlap == 0 {
        return 0.0;
    }
    let mut score = 10.0 * overlap as f64 / query_tokens.len() as f64;
    if !query_norm.is_empty() && normalized(&chunk.text).contains(query_norm) {
        score += 8.0;
    }
    if query_raw.len() >= 3 && chunk.text.contains(query_raw) {
        score += 6.0;
    }
    if !chunk.heading_path.is_empty() {
        let heading_tokens: HashSet<String> =
            chunk.heading_path.iter().flat_map(|h| tokens(h)).collect();
        let heading_overlap = query_set.intersection(&heading_tokens).count();
        score += 3.0 * heading_overlap as f64;
    }
    score
}

/// Select focused chunks from an already-extracted document.
///
/// `max_chunks` caps the returned chunk count; `max_chars` caps the
/// total returned characters. Selection starts from the highest-scoring
/// chunks (ties broken by original document order) and expands each
/// pick to its immediately neighboring chunks while budget allows, so
/// a high-scoring chunk retains adjacent context. Output is in
/// original document order with stable chunk IDs.
pub fn select_focus_chunks(
    document: &FetchDocument,
    query: &str,
    max_chunks: usize,
    max_chars: usize,
) -> FocusedFetchSelection {
    let empty = FocusedFetchSelection {
        chunks: Vec::new(),
        truncated: false,
        total_chars: 0,
    };
    if max_chunks == 0 || max_chars == 0 || document.chunks.is_empty() {
        return empty;
    }
    let query_raw = query.trim();
    if query_raw.is_empty() {
        return empty;
    }
    let query_tokens = tokens(query_raw);
    let query_norm = normalized(query_raw);
    if query_tokens.is_empty() {
        return empty;
    }
    let scores: Vec<f64> = document
        .chunks
        .iter()
        .map(|c| score_chunk(&query_tokens, &query_norm, query_raw, c))
        .collect();
    let candidate_count = scores.iter().filter(|s| **s > 0.0).count();
    if candidate_count == 0 {
        return empty;
    }
    let mut ranked: Vec<usize> = (0..document.chunks.len()).collect();
    ranked.sort_by(|a, b| {
        scores[*b]
            .partial_cmp(&scores[*a])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.cmp(b))
    });
    let mut selected: Vec<usize> = Vec::new();
    let mut selected_set: HashSet<usize> = HashSet::new();
    for idx in ranked {
        if selected.len() >= max_chunks {
            break;
        }
        if scores[idx] <= 0.0 {
            break;
        }
        if selected_set.insert(idx) {
            selected.push(idx);
        }
        for neighbor in [idx.checked_sub(1), idx.checked_add(1)] {
            if selected.len() >= max_chunks {
                break;
            }
            if let Some(nb) = neighbor {
                if nb < document.chunks.len() && scores[nb] > 0.0 && selected_set.insert(nb) {
                    selected.push(nb);
                }
            }
        }
    }
    selected.sort_unstable();
    let mut chunks = Vec::new();
    let mut total_chars = 0usize;
    let mut truncated = selected.len() < candidate_count;
    for idx in selected {
        let chunk = &document.chunks[idx];
        let chars = chunk.text.chars().count();
        if chunks.is_empty() && chars > max_chars {
            let cut: String = chunk.text.chars().take(max_chars).collect();
            total_chars = cut.chars().count();
            chunks.push(DocumentChunk {
                text: cut,
                ..chunk.clone()
            });
            truncated = true;
            break;
        }
        if total_chars + chars > max_chars {
            truncated = true;
            break;
        }
        total_chars += chars;
        chunks.push(chunk.clone());
    }
    FocusedFetchSelection {
        chunks,
        truncated,
        total_chars,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::document::{DocumentOutlineEntry, RenderedBlock};
    use crate::core::identity::chunk_id;

    fn chunk(index: usize, text: &str, heading: &[&str]) -> DocumentChunk {
        DocumentChunk {
            chunk_id: chunk_id("doc_test", index, &heading.join("/")),
            text: text.to_string(),
            heading_path: heading.iter().map(|s| s.to_string()).collect(),
            block_start: index,
            block_end: index,
            page_start: None,
            page_end: None,
        }
    }

    fn doc(chunks: Vec<DocumentChunk>) -> FetchDocument {
        FetchDocument {
            kind: crate::core::document::DocumentKind::Html,
            render_format: crate::core::document::RenderFormat::AgentBlocksV1,
            text_format: "plain".to_string(),
            text_chars_returned: 0,
            text_truncated: false,
            block_truncated: false,
            link_truncated: false,
            metadata: None,
            outline: Vec::<DocumentOutlineEntry>::new(),
            blocks: Vec::<RenderedBlock>::new(),
            chunks,
        }
    }

    #[test]
    fn empty_query_or_caps_return_empty() {
        let d = doc(vec![chunk(0, "hello world", &[])]);
        let out = select_focus_chunks(&d, "   ", 5, 1000);
        assert!(out.chunks.is_empty());
        assert!(!out.truncated);
        let out = select_focus_chunks(&d, "hello", 0, 1000);
        assert!(out.chunks.is_empty());
        let out = select_focus_chunks(&d, "hello", 5, 0);
        assert!(out.chunks.is_empty());
        let out = select_focus_chunks(&d, "nomatchxyz", 5, 1000);
        assert!(out.chunks.is_empty());
        assert!(!out.truncated);
    }

    #[test]
    fn token_overlap_ranks_best_chunk_first() {
        let d = doc(vec![
            chunk(0, "the weather is nice today", &[]),
            chunk(1, "rust async tokio runtime executor", &[]),
            chunk(2, "cooking recipes for dinner", &[]),
        ]);
        let out = select_focus_chunks(&d, "tokio runtime", 5, 10_000);
        assert!(!out.chunks.is_empty());
        assert_eq!(out.chunks[0].text, "rust async tokio runtime executor");
        assert!(!out.truncated);
    }

    #[test]
    fn exact_phrase_beats_partial_overlap() {
        let d = doc(vec![
            chunk(0, "tokio provides async utilities", &[]),
            chunk(1, "the tokio runtime drives tasks", &[]),
        ]);
        let out = select_focus_chunks(&d, "tokio runtime", 1, 10_000);
        assert_eq!(out.chunks.len(), 1);
        assert_eq!(out.chunks[0].text, "the tokio runtime drives tasks");
        assert!(out.truncated);
    }

    #[test]
    fn code_symbol_exact_match_wins() {
        let d = doc(vec![
            chunk(0, "use the router to handle requests", &[]),
            chunk(
                1,
                "call Router::new() to construct a Router::new instance",
                &[],
            ),
        ]);
        let out = select_focus_chunks(&d, "Router::new", 1, 10_000);
        assert_eq!(out.chunks.len(), 1);
        assert!(out.chunks[0].text.contains("Router::new"));
        assert!(out.truncated);
    }

    #[test]
    fn heading_overlap_boosts_matching_section() {
        let d = doc(vec![
            chunk(0, "generic introduction paragraph here", &["Introduction"]),
            chunk(
                1,
                "generic configuration paragraph here",
                &["Configuration"],
            ),
        ]);
        let out = select_focus_chunks(&d, "configuration", 5, 10_000);
        assert_eq!(
            out.chunks[0].heading_path,
            vec!["Configuration".to_string()]
        );
    }

    #[test]
    fn tie_break_is_document_order() {
        let d = doc(vec![
            chunk(0, "alpha shared", &[]),
            chunk(1, "beta shared", &[]),
        ]);
        let out = select_focus_chunks(&d, "shared", 5, 10_000);
        assert_eq!(out.chunks.len(), 2);
        assert!(out.chunks[0].text.starts_with("alpha"));
    }

    #[test]
    fn adjacency_expansion_keeps_neighbors_within_cap() {
        let d = doc(vec![
            chunk(0, "tokio overview preamble", &[]),
            chunk(1, "tokio runtime internals deep dive", &[]),
            chunk(2, "more tokio runtime details follow", &[]),
            chunk(3, "runtime appendix notes", &[]),
        ]);
        let out = select_focus_chunks(&d, "tokio runtime", 3, 10_000);
        let texts: Vec<&str> = out.chunks.iter().map(|c| c.text.as_str()).collect();
        assert!(texts.contains(&"tokio runtime internals deep dive"));
        assert!(texts.contains(&"tokio overview preamble"));
        assert_eq!(out.chunks.len(), 3);
        assert!(out.truncated);
    }

    #[test]
    fn zero_score_neighbors_are_not_pulled_in() {
        let d = doc(vec![
            chunk(0, "unrelated preamble", &[]),
            chunk(1, "tokio runtime internals deep dive", &[]),
            chunk(2, "unrelated appendix", &[]),
        ]);
        let out = select_focus_chunks(&d, "tokio runtime", 5, 10_000);
        assert_eq!(out.chunks.len(), 1);
        assert_eq!(out.chunks[0].text, "tokio runtime internals deep dive");
        assert!(!out.truncated);
    }

    #[test]
    fn chunk_and_char_caps_set_truncated() {
        let d = doc(vec![
            chunk(0, "shared one", &[]),
            chunk(1, "shared two", &[]),
            chunk(2, "shared three", &[]),
        ]);
        let out = select_focus_chunks(&d, "shared", 2, 10_000);
        assert_eq!(out.chunks.len(), 2);
        assert!(out.truncated);
        assert_eq!(
            out.total_chars,
            out.chunks
                .iter()
                .map(|c| c.text.chars().count())
                .sum::<usize>()
        );
        let out = select_focus_chunks(&d, "shared", 5, 12);
        assert!(out.total_chars <= 12);
        assert!(out.truncated);
    }

    #[test]
    fn output_is_document_order_with_stable_ids() {
        let d = doc(vec![
            chunk(0, "shared zero", &[]),
            chunk(1, "nothing relevant", &[]),
            chunk(2, "shared two", &[]),
        ]);
        let out = select_focus_chunks(&d, "shared", 5, 10_000);
        assert_eq!(out.chunks.len(), 2);
        assert_eq!(out.chunks[0].chunk_id, chunk_id("doc_test", 0, ""));
        assert_eq!(out.chunks[1].chunk_id, chunk_id("doc_test", 2, ""));
        assert!(out.chunks[0].block_start < out.chunks[1].block_start);
    }
}
