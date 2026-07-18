#![no_main]

use libfuzzer_sys::fuzz_target;
use eggsearch::fetch::extract::extract_content;

fuzz_target!(|data: &[u8]| {
    let (title, desc, text, links, warnings, truncated, links_seen, links_truncated) =
        extract_content(data, "https://example.com/", 5000, true);
    // Title and desc are Optional but always valid UTF-8 when present
    if let Some(ref t) = title {
        assert!(t.chars().all(|c| !c.is_control() || c == '\n'));
    }
    if let Some(ref d) = desc {
        assert!(!d.is_empty());
    }
    // Links must not exceed MAX_LINKS
    assert!(links.len() <= 100);
    // Truncation flag must be consistent with text length
    if !truncated {
        assert!(text.chars().count() <= 5000);
    }
    let _ = warnings;
    let _ = links_seen;
    let _ = links_truncated;
});
