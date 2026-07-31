#![no_main]

use libfuzzer_sys::fuzz_target;
use eggsearch::fetch::pdf::extract_pdf_text;
use eggsearch::fetch::pdf::PdfLimits;

fuzz_target!(|data: &[u8]| {
    let limits = PdfLimits {
        max_pages: 5,
        max_chars_per_page: 1000,
        max_total_chars: 5000,
    };
    let _ = extract_pdf_text(data, 5000, &limits, None);
});
