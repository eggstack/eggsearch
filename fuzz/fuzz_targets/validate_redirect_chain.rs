#![no_main]

use libfuzzer_sys::fuzz_target;
use eggsearch::fetch::limits::{validate_url, FetchLimits};

fuzz_target!(|data: &str| {
    let limits = FetchLimits {
        allow_private_network: true,
        allow_localhost: true,
        ..Default::default()
    };
    let mut last_result = None;
    for line in data.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let result = validate_url(line, &limits);
        if result.is_err() {
            break;
        }
        last_result = Some(result);
    }
    let _ = last_result;
});
