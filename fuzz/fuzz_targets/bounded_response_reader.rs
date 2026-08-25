#![no_main]

use libfuzzer_sys::fuzz_target;
use eggsearch::meta::engines::push_bounded_chunk;

fuzz_target!(|data: &[u8]| {
    let per_response_cap: usize = 64 * 1024;

    let mut total_bytes: usize = 0;
    let mut body: Vec<u8> = Vec::new();

    // Feed the input as a stream of chunks to exercise accumulation
    // across calls to the real bounded-read implementation.
    let chunk_size = (data.len() / 4).max(1);
    for chunk in data.chunks(chunk_size) {
        if push_bounded_chunk(&mut body, chunk, per_response_cap, "fuzz").is_err() {
            break;
        }
        assert!(total_bytes <= per_response_cap);
        assert_eq!(body.len(), total_bytes + chunk.len());
        assert!(body.len() <= per_response_cap);
        total_bytes += chunk.len();
    }
});
