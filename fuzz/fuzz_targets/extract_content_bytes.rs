#![no_main]

use libfuzzer_sys::fuzz_target;
use eggsearch::fetch::extract_content;

fuzz_target!(|data: &[u8]| {
    let _ = extract_content(data, "http://fuzz", 10000, false);
});
