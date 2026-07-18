#![no_main]

use libfuzzer_sys::fuzz_target;
use eggsearch::core::sanitize::scan_injection_markers;

fuzz_target!(|data: &str| {
    let hits = scan_injection_markers(data);
    // All byte offsets must be valid UTF-8 boundaries
    for hit in &hits {
        assert!(hit.byte_offset <= data.len());
        assert!(data.is_char_boundary(hit.byte_offset));
    }
    // Pattern names must be non-empty
    for hit in &hits {
        assert!(!hit.pattern.is_empty());
    }
});
