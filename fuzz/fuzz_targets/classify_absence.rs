#![no_main]

use libfuzzer_sys::fuzz_target;
use eggsearch::core::retrieval_status::{classify_absence, EvidenceAbsenceKind};

fuzz_target!(|data: &[u8]| {
    if let Ok(kind) = serde_json::from_slice::<EvidenceAbsenceKind>(data) {
        let _ = classify_absence(kind);
    }
});
