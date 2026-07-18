#![no_main]

use libfuzzer_sys::fuzz_target;
use eggsearch::core::sanitize::{strip_control_chars, bound_text, scan_injection_markers};

fuzz_target!(|data: &str| {
    let (cleaned, _) = strip_control_chars(data);
    let (bounded, _) = bound_text(&cleaned, 500);
    let _ = scan_injection_markers(&bounded);
});
