#![no_main]

use libfuzzer_sys::fuzz_target;
use eggsearch::fetch::limits::{validate_url, FetchLimits};

fuzz_target!(|data: &str| {
    let limits = FetchLimits {
        allow_private_network: true,
        allow_localhost: true,
        ..Default::default()
    };
    let _ = validate_url(data, &limits);
});
