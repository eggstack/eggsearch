#![no_main]

use libfuzzer_sys::fuzz_target;
use eggsearch::fetch::pdf::extract_pdf_text;
use eggsearch::fetch::pdf::PdfLimits;
use eggsearch::fetch::pdf_stream::scan_stream_budget;
use eggsearch::fetch::FetchError;

const BUDGET: usize = 1_048_576;

fuzz_target!(|data: &[u8]| {
    let limits = PdfLimits {
        max_pages: 5,
        max_chars_per_page: 1000,
        max_total_chars: 5000,
        max_decompressed_bytes: BUDGET,
    };
    match scan_stream_budget(data, BUDGET) {
        Ok(total) => assert!(total <= BUDGET),
        Err(FetchError::PdfDecompressionBoundExceeded { measured, budget }) => {
            assert!(budget == BUDGET);
            assert!(measured > budget);
        }
        Err(other) => panic!("stream scan returned an unexpected error: {other:?}"),
    }
    let _ = extract_pdf_text(data, 5000, &limits, None);
});
