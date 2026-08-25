#![no_main]

use libfuzzer_sys::fuzz_target;
use eggsearch::core::document::DocumentKind;
use eggsearch::fetch::detect::classify;
use eggsearch::fetch::extract::{extract_content, MAX_LINKS};

const BASE_URL: &str = "https://example.com/page";

fuzz_target!(|data: &[u8]| {
    let content_type = if data.len() % 3 == 0 {
        "text/html"
    } else if data.len() % 3 == 1 {
        "text/plain"
    } else {
        "application/json"
    };

    // Exercise real content-type classification with deterministic
    // expectations from the classifier contract.
    let detected = classify(Some(content_type), BASE_URL, data);
    if content_type == "application/json" {
        assert!(
            matches!(detected.kind, DocumentKind::Json),
            "application/json must classify as JSON, got {:?}",
            detected.kind
        );
        assert_eq!(detected.language.as_deref(), Some("json"));
        assert!(detected.line_preserving);
    }

    // Exercise HTML extraction with a realistic base URL for link
    // resolution (the second parameter of extract_content).
    let (_, _, _, links, warnings, _text_truncated, links_seen, links_truncated) =
        extract_content(data, BASE_URL, 10000, false);
    assert!(
        links.len() <= links_seen,
        "extracted links cannot exceed the number of anchor elements seen"
    );
    if !links_truncated {
        assert!(
            links_seen <= MAX_LINKS,
            "without truncation the link count must be under the cap"
        );
    }
    let _ = warnings;
});
