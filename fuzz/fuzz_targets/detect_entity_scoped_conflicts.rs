#![no_main]

use libfuzzer_sys::fuzz_target;
use eggsearch::core::conflict::detect_entity_scoped_conflicts;
use eggsearch::core::source_card::SourceCard;

fuzz_target!(|data: &[u8]| {
    if let Ok(cards) = serde_json::from_slice::<Vec<SourceCard>>(data) {
        let _ = detect_entity_scoped_conflicts(&cards);
    }
});
