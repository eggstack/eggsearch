#![no_main]

use libfuzzer_sys::fuzz_target;
use eggsearch::core::document::{
    build_document_chunks, BlockKind, DocumentOutlineEntry, RenderedBlock,
};
use std::collections::HashSet;

fuzz_target!(|data: &str| {
    // Build blocks from the fuzzed data by splitting on newlines
    let blocks: Vec<RenderedBlock> = data
        .lines()
        .enumerate()
        .map(|(i, line)| RenderedBlock {
            kind: if line.starts_with("# ") {
                BlockKind::Heading
            } else {
                BlockKind::RawText
            },
            text: line.to_string(),
            level: if line.starts_with("# ") {
                Some(1)
            } else {
                None
            },
            anchor: None,
            language: None,
            line_start: Some(i + 1),
            line_end: Some(i + 1),
            page: None,
        })
        .collect();

    let outline = vec![DocumentOutlineEntry {
        level: 1,
        title: "Fuzzed".to_string(),
        anchor: None,
        block_index: Some(0),
        page: None,
    }];

    let chunks = build_document_chunks("fuzz-doc", &outline, &blocks, 4096);

    // All chunk IDs must be unique
    let mut ids = HashSet::new();
    for chunk in &chunks {
        assert!(ids.insert(chunk.chunk_id.clone()), "duplicate chunk_id: {}", chunk.chunk_id);
    }
    // Block ranges must be contiguous and non-overlapping
    let mut prev_end = None;
    for chunk in &chunks {
        if let Some(end) = prev_end {
            assert_eq!(chunk.block_start, end + 1, "non-contiguous block ranges");
        }
        assert!(chunk.block_start <= chunk.block_end);
        prev_end = Some(chunk.block_end);
    }
    // All block indices must be within bounds
    for chunk in &chunks {
        assert!(chunk.block_start < blocks.len());
        assert!(chunk.block_end < blocks.len());
    }
});
