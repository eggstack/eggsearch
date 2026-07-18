#![no_main]

use libfuzzer_sys::fuzz_target;
use eggsearch::fetch::extract_content;

fuzz_target!(|data: &str| {
    let ct = if data.len() % 3 == 0 {
        "text/html"
    } else if data.len() % 3 == 1 {
        "text/plain"
    } else {
        "application/json"
    };
    let _ = extract_content(data.as_bytes(), ct, 10000, false);
});
