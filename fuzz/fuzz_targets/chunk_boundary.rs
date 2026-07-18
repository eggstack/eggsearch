#![no_main]

use libfuzzer_sys::fuzz_target;
use eggsearch::core::sanitize::{strip_control_chars, bound_text};

fuzz_target!(|data: &str| {
    let (cleaned, _) = strip_control_chars(data);
    for boundary in [1, 2, 3, 4, 7, 8, 15, 16, 31, 32, 63, 64, 127, 128, 255, 256] {
        let (bounded, _) = bound_text(&cleaned, boundary);
        let _ = bounded;
    }
});
