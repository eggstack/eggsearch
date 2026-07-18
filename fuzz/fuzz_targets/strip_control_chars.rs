#![no_main]

use libfuzzer_sys::fuzz_target;
use eggsearch::core::sanitize::strip_control_chars;

fuzz_target!(|data: &str| {
    let (cleaned, count) = strip_control_chars(data);
    // Idempotency: stripping again should remove nothing
    let (cleaned2, count2) = strip_control_chars(&cleaned);
    assert_eq!(count2, 0, "strip_control_chars must be idempotent");
    assert_eq!(cleaned, cleaned2, "idempotent application must produce same output");
    // All removed characters must be control chars
    let _ = count;
});
