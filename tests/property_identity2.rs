use eggsearch::core::identity::{batch_fetch_id, doc_id, fetch_id, suggested_fetch_id};
use proptest::prelude::*;

fn url_strategy() -> impl Strategy<Value = String> {
    "https?://[a-z][a-z0-9-]*\\.[a-z]{2,}(:[0-9]{2,5})?(/[a-zA-Z0-9/_.~-]*)?"
}

fn safe_string() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 _./-]{1,50}"
}

proptest! {
    #[test]
    fn fetch_id_deterministic(url in url_strategy()) {
        let a = fetch_id(Some(&url), None, Some(1), Some(10), Some("prefix"));
        let b = fetch_id(Some(&url), None, Some(1), Some(10), Some("prefix"));
        prop_assert_eq!(a, b);
    }

    #[test]
    fn fetch_id_starts_with_prefix(url in url_strategy()) {
        let id = fetch_id(Some(&url), None, None, None, None);
        prop_assert!(id.starts_with("fetch_"), "id doesn't start with fetch_: {}", id);
    }

    #[test]
    fn fetch_id_length(url in url_strategy()) {
        let id = fetch_id(Some(&url), None, None, None, None);
        prop_assert_eq!(id.len(), 22);
    }

    #[test]
    fn different_inputs_different_fetch_ids(url in url_strategy()) {
        let base = fetch_id(Some(&url), None, Some(1), Some(10), None);
        let changed = fetch_id(Some(&url), None, Some(5), Some(15), None);
        prop_assert_ne!(base, changed);
    }

    #[test]
    fn suggested_fetch_id_deterministic(url in url_strategy(), group in "[a-zA-Z]+", priority in 1u8..255u8) {
        let a = suggested_fetch_id(&url, &group, priority);
        let b = suggested_fetch_id(&url, &group, priority);
        prop_assert_eq!(a, b);
    }

    #[test]
    fn suggested_fetch_id_starts_with_prefix(url in url_strategy()) {
        let id = suggested_fetch_id(&url, "group", 1);
        prop_assert!(id.starts_with("suggested_"), "id doesn't start with suggested_: {}", id);
    }

    #[test]
    fn suggested_fetch_id_length(url in url_strategy()) {
        let id = suggested_fetch_id(&url, "group", 1);
        prop_assert_eq!(id.len(), 26);
    }
}

proptest! {
    #[test]
    fn batch_fetch_id_deterministic(label in safe_string(), index in 0usize..1000usize) {
        let a = batch_fetch_id(&label, index);
        let b = batch_fetch_id(&label, index);
        prop_assert_eq!(a, b);
    }

    #[test]
    fn batch_fetch_id_starts_with_prefix(label in safe_string()) {
        let id = batch_fetch_id(&label, 0);
        prop_assert!(id.starts_with("batch_"), "id doesn't start with batch_: {}", id);
    }

    #[test]
    fn batch_fetch_id_length(label in safe_string()) {
        let id = batch_fetch_id(&label, 0);
        prop_assert_eq!(id.len(), 22);
    }

    #[test]
    fn different_inputs_different_batch_ids(label in safe_string()) {
        let a = batch_fetch_id(&label, 0);
        let b = batch_fetch_id(&label, 1);
        prop_assert_ne!(a, b);
    }

    #[test]
    fn doc_id_deterministic(url in url_strategy(), title in "[a-zA-Z0-9 ]{1,50}", kind in "[a-z]{1,20}") {
        let a = doc_id(Some(&url), Some(&title), Some(&kind));
        let b = doc_id(Some(&url), Some(&title), Some(&kind));
        prop_assert_eq!(a, b);
    }

    #[test]
    fn doc_id_starts_with_prefix(url in url_strategy()) {
        let id = doc_id(Some(&url), None, None);
        prop_assert!(id.starts_with("doc_"), "id doesn't start with doc_: {}", id);
    }

    #[test]
    fn doc_id_length(url in url_strategy()) {
        let id = doc_id(Some(&url), None, None);
        prop_assert_eq!(id.len(), 20);
    }

    #[test]
    fn different_inputs_different_doc_ids(url in url_strategy()) {
        let a = doc_id(Some(&url), Some("title1"), None);
        let b = doc_id(Some(&url), Some("title2"), None);
        prop_assert_ne!(a, b);
    }
}
