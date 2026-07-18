#![no_main]

use libfuzzer_sys::fuzz_target;
use eggsearch::core::identity::source_id;

fuzz_target!(|data: &str| {
    let _ = source_id(Some("fuzz"), Some(data), None, None);
});
