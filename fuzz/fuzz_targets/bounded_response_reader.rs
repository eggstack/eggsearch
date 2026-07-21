#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let per_response_cap: usize = 64 * 1024;

    let mut total_bytes: usize = 0;
    let mut body = String::new();

    let chunk = data;
    total_bytes += chunk.len();

    if total_bytes > per_response_cap {
        return;
    }

    if let Ok(s) = std::str::from_utf8(chunk) {
        body.push_str(s);
    }

    assert!(total_bytes <= per_response_cap + chunk.len());
    assert!(body.len() <= per_response_cap + chunk.len());
});
