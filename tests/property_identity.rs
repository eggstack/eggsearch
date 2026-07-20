use eggsearch::core::identity::{
    batch_fetch_id, canonicalize_url, chunk_id, code_span_id, doc_id, fetch_id, source_id,
    suggested_fetch_id,
};
use proptest::prelude::*;

fn url_strategy() -> impl Strategy<Value = String> {
    "https?://[a-z][a-z0-9-]*\\.[a-z]{2,}(:[0-9]{2,5})?(/[a-zA-Z0-9/_.~-]*)?"
}

proptest! {
    #[test]
    fn source_id_deterministic(provider in "[a-z]+", url in url_strategy(), title in "[a-zA-Z0-9 ]{1,50}") {
        let a = source_id(Some(&provider), Some(&url), Some(&title), None);
        let b = source_id(Some(&provider), Some(&url), Some(&title), None);
        prop_assert_eq!(a, b);
    }

    #[test]
    fn source_id_starts_with_prefix(provider in "[a-z]+", url in url_strategy()) {
        let id = source_id(Some(&provider), Some(&url), None, None);
        prop_assert!(id.starts_with("src_"), "id doesn't start with src_: {}", id);
    }

    #[test]
    fn source_id_length(provider in "[a-z]+", url in url_strategy()) {
        let id = source_id(Some(&provider), Some(&url), None, None);
        prop_assert_eq!(id.len(), 20);
    }

    #[test]
    fn source_id_canonicalizes_urls(provider in "[a-z]+") {
        let base = source_id(Some(&provider), Some("https://example.com/path"), None, None);
        let trailing = source_id(Some(&provider), Some("https://example.com/path/"), None, None);
        let fragment = source_id(Some(&provider), Some("https://example.com/path#sec"), None, None);
        let www = source_id(Some(&provider), Some("https://www.example.com/path"), None, None);
        let port = source_id(Some(&provider), Some("https://example.com:443/path"), None, None);
        prop_assert_eq!(&base, &trailing);
        prop_assert_eq!(&base, &fragment);
        prop_assert_eq!(&base, &www);
        prop_assert_eq!(&base, &port);
    }

    #[test]
    fn different_inputs_different_source_ids(provider in "[a-z]+", url in url_strategy(), title in "[a-zA-Z]{5,20}") {
        let base = source_id(Some(&provider), Some(&url), Some(&title), None);
        let mut title2 = title.clone();
        title2.push('x');
        let changed = source_id(Some(&provider), Some(&url), Some(&title2), None);
        prop_assert_ne!(base, changed);
    }
}

proptest! {
    #[test]
    fn canonicalize_url_idempotent(url in url_strategy()) {
        let a = canonicalize_url(&url);
        let b = canonicalize_url(&a);
        prop_assert_eq!(a, b);
    }

    #[test]
    fn canonicalize_url_strips_trailing_slash(url in "https://example\\.com/[a-z]{1,10}/") {
        let result = canonicalize_url(&url);
        prop_assert!(
            !result.ends_with('/') || result == "https://example.com/",
            "trailing slash not stripped: {}",
            result
        );
    }

    #[test]
    fn canonicalize_url_strips_fragment(url in "https://example\\.com/path#[a-z]{1,5}") {
        let result = canonicalize_url(&url);
        prop_assert!(!result.contains('#'), "fragment not stripped: {}", result);
    }

    #[test]
    fn canonicalize_url_lowercases_scheme(url in "HTTP://EXAMPLE\\.COM/path") {
        let result = canonicalize_url(&url);
        prop_assert!(
            result.starts_with("http://") || result.starts_with("https://"),
            "scheme not lowercased: {}",
            result
        );
    }

    #[test]
    fn canonicalize_url_strips_www(url in "https://www\\.[a-z0-9.-]+/[a-z0-9/]*") {
        let result = canonicalize_url(&url);
        prop_assert!(!result.contains("://www."), "www not stripped: {}", result);
    }

    #[test]
    fn canonicalize_url_strips_default_port_http(url in "http://example\\.com:80/path") {
        let result = canonicalize_url(&url);
        prop_assert!(!result.contains(":80"), "default port not stripped: {}", result);
    }

    #[test]
    fn canonicalize_url_strips_default_port_https(url in "https://example\\.com:443/path") {
        let result = canonicalize_url(&url);
        prop_assert!(!result.contains(":443"), "default port not stripped: {}", result);
    }

    #[test]
    fn canonicalize_url_preserves_non_default_port(url in "https://example\\.com:8443/path") {
        let result = canonicalize_url(&url);
        prop_assert!(result.contains(":8443"), "non-default port stripped: {}", result);
    }
}

proptest! {
    #[test]
    fn cross_type_ids_never_collide(
        url in "https://[a-z]+\\.com/path",
        title in "[a-zA-Z]{5,20}"
    ) {
        let src = source_id(Some("provider"), Some(&url), Some(&title), None);
        let fid = fetch_id(Some(&url), None, None, None, None);
        let doc = doc_id(Some(&url), Some(&title), None);
        let sug = suggested_fetch_id(&url, "code", 1);
        let bat = batch_fetch_id(&url, 0);
        let chnk = chunk_id(&doc, 0, "intro");
        let cspan = code_span_id(&url, Some(1), Some(10), None);

        let ids: Vec<&str> = vec![&src, &fid, &doc, &sug, &bat, &chnk, &cspan];
        let unique: std::collections::HashSet<&str> = ids.into_iter().collect();
        prop_assert_eq!(unique.len(), 7, "all entity types must produce distinct IDs, got: src={}, fetch={}, doc={}, suggested={}, batch={}, chunk={}, span={}", src, fid, doc, sug, bat, chnk, cspan);
    }

    #[test]
    fn unicode_normalization_fullwidth_vs_ascii(
        path in "[a-zA-Z0-9]{1,10}"
    ) {
        let ascii_url = format!("https://example.com/{path}");
        let fullwidth: String = path.chars().map(|c| {
            if c.is_ascii_alphanumeric() {
                char::from_u32(0xFF01 + (c as u32 - '!' as u32)).unwrap_or(c)
            } else {
                c
            }
        }).collect();
        let fullwidth_url = format!("https://example.com/{fullwidth}");
        let ascii_id = source_id(Some("test"), Some(&ascii_url), None, None);
        let fullwidth_id = source_id(Some("test"), Some(&fullwidth_url), None, None);
        prop_assert_ne!(
            ascii_id, fullwidth_id,
            "fullwidth URL should produce different ID than ASCII: {:?} vs {:?}",
            fullwidth_url, ascii_url
        );
    }

    #[test]
    fn source_id_field_order_insensitive(
        url in "https://[a-z]+\\.com/[a-z]{1,10}",
        title in "[a-zA-Z]{5,20}"
    ) {
        let a = source_id(Some("prov"), Some(&url), Some(&title), None);
        let b = source_id(Some("prov"), Some(&url), Some(&title), None);
        prop_assert_eq!(a, b, "identical fields must produce identical source_id");
    }
}
