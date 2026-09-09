//! Shared fetch execution policy for web and batch fetches.
//!
//! Consolidates timeout resolution, max-char clamping, trust
//! propagation inputs, cache controls, and focus projection where
//! semantics are identical across tools. Repo-specific revision
//! resolution and local safe-open logic remain separate.

use crate::core::document::FetchDocument;
use crate::core::fetch::{
    ExtractMode, FetchCachePolicy, FocusedFetchSelection, MAX_CACHE_AGE_SECONDS, MAX_FOCUS_CHUNKS,
    MAX_FOCUS_QUERY_CHARS,
};

/// Validate an optional focus query.
pub fn validate_focus_query(query: Option<&str>) -> Result<Option<String>, String> {
    match query {
        None => Ok(None),
        Some(q) if q.trim().is_empty() => Err("focus must not be empty".to_string()),
        Some(q) if q.chars().count() > MAX_FOCUS_QUERY_CHARS => Err(format!(
            "focus must be <= {MAX_FOCUS_QUERY_CHARS} characters"
        )),
        Some(q) => Ok(Some(q.trim().to_string())),
    }
}

/// Validate an optional focus chunk cap.
pub fn validate_focus_max_chunks(v: Option<usize>) -> Result<Option<usize>, String> {
    match v {
        Some(0) => Err("focus_max_chunks must be > 0".to_string()),
        Some(n) if n > MAX_FOCUS_CHUNKS => {
            Err(format!("focus_max_chunks must be <= {MAX_FOCUS_CHUNKS}"))
        }
        other => Ok(other),
    }
}

/// Validate an optional focus character cap.
pub fn validate_focus_max_chars(v: Option<usize>) -> Result<Option<usize>, String> {
    match v {
        Some(0) => Err("focus_max_chars must be > 0".to_string()),
        other => Ok(other),
    }
}

/// Reject focus with metadata-only extraction.
pub fn validate_focus_for_extract_mode(
    focus: Option<&str>,
    mode: ExtractMode,
) -> Result<(), String> {
    if focus.is_some() && mode == ExtractMode::MetadataOnly {
        return Err(
            "focus requires extracted content; it is not valid with extract_mode = \"metadata_only\""
                .to_string(),
        );
    }
    Ok(())
}

/// Validate an optional caller cache-age bound.
pub fn validate_cache_age(age: Option<u64>) -> Result<Option<u64>, String> {
    if let Some(a) = age {
        if a > MAX_CACHE_AGE_SECONDS {
            return Err(format!(
                "max_cache_age_seconds must be <= {MAX_CACHE_AGE_SECONDS}"
            ));
        }
    }
    Ok(age)
}

/// Resolve the effective per-item character budget.
pub fn resolve_effective_max_chars(requested: Option<usize>, default: usize, cap: usize) -> usize {
    requested.unwrap_or(default).min(cap).max(1)
}

/// Resolve the effective focus chunk budget.
pub fn focus_max_chunks_or_default(v: Option<usize>) -> usize {
    v.unwrap_or(MAX_FOCUS_CHUNKS).clamp(1, MAX_FOCUS_CHUNKS)
}

/// Resolve the effective focus character budget.
pub fn focus_max_chars_or_default(
    v: Option<usize>,
    effective_max_chars: usize,
    cap: usize,
) -> usize {
    v.unwrap_or(effective_max_chars).min(cap).max(1)
}

/// Apply deterministic focus projection to an extracted document.
pub fn apply_focus_to_document(
    document: Option<&FetchDocument>,
    fetched: bool,
    focus_query: Option<&str>,
    focus_max_chunks: Option<usize>,
    focus_max_chars: Option<usize>,
    effective_max_chars: usize,
    max_chars_cap: usize,
) -> Option<FocusedFetchSelection> {
    let (query, document) = match (focus_query, document) {
        (Some(q), Some(d)) if fetched => (q, d),
        _ => return None,
    };
    let max_chunks = focus_max_chunks_or_default(focus_max_chunks);
    let max_chars = focus_max_chars_or_default(focus_max_chars, effective_max_chars, max_chars_cap);
    Some(crate::core::focus::select_focus_chunks(
        document, query, max_chunks, max_chars,
    ))
}

/// Resolve the effective cache policy.
pub fn cache_policy_or_default(v: Option<FetchCachePolicy>) -> FetchCachePolicy {
    v.unwrap_or(FetchCachePolicy::Default)
}

/// Truncate text on UTF-8 character boundaries.
pub fn truncate_utf8_safe(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    text.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_query_validation() {
        assert!(validate_focus_query(None).unwrap().is_none());
        assert!(validate_focus_query(Some("  ")).is_err());
        assert!(validate_focus_query(Some("tokio")).unwrap().is_some());
        let long = "x".repeat(MAX_FOCUS_QUERY_CHARS + 1);
        assert!(validate_focus_query(Some(&long)).is_err());
    }

    #[test]
    fn focus_chunks_validation() {
        assert!(validate_focus_max_chunks(Some(0)).is_err());
        assert!(validate_focus_max_chunks(Some(MAX_FOCUS_CHUNKS + 1)).is_err());
        assert_eq!(validate_focus_max_chunks(Some(2)).unwrap(), Some(2));
    }

    #[test]
    fn focus_chars_validation() {
        assert!(validate_focus_max_chars(Some(0)).is_err());
        assert_eq!(validate_focus_max_chars(Some(10)).unwrap(), Some(10));
    }

    #[test]
    fn metadata_only_rejects_focus() {
        assert!(validate_focus_for_extract_mode(Some("q"), ExtractMode::MetadataOnly).is_err());
        assert!(validate_focus_for_extract_mode(Some("q"), ExtractMode::Text).is_ok());
        assert!(validate_focus_for_extract_mode(None, ExtractMode::MetadataOnly).is_ok());
    }

    #[test]
    fn truncate_is_char_boundary_safe() {
        let s = "héllo wörld";
        let out = truncate_utf8_safe(s, 5);
        assert_eq!(out.chars().count(), 5);
        let emoji = "a🦀b🦀c";
        let out = truncate_utf8_safe(emoji, 2);
        assert_eq!(out, "a🦀");
    }

    #[test]
    fn effective_max_chars_clamps() {
        assert_eq!(resolve_effective_max_chars(None, 12000, 50000), 12000);
        assert_eq!(
            resolve_effective_max_chars(Some(100000), 12000, 50000),
            50000
        );
        assert_eq!(resolve_effective_max_chars(Some(5), 12000, 50000), 5);
    }
}
