#![no_main]

use libfuzzer_sys::fuzz_target;
use eggsearch::fetch::extract_content;

fuzz_target!(|data: &[u8]| {
    let _ = extract_content(data, "http://fuzz", 5000, false);
    let lossy = String::from_utf8_lossy(data);
    let _ = extract_content(lossy.as_bytes(), "http://fuzz", 5000, true);
});
