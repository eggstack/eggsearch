use eggsearch::mcp::cursor::{
    encode_next, locate_start, page_window, CursorError, CursorTool, PlanKey, RetrievalCursor,
    MAX_CURSOR_BYTES,
};
use proptest::prelude::*;

fn sample() -> RetrievalCursor {
    RetrievalCursor::new(
        CursorTool::WebFetch,
        &PlanKey::new(CursorTool::WebFetch)
            .str("https://example.com/docs")
            .usize(12000)
            .finish(),
        7,
        "chunk_0123456789abcdef",
        12000,
    )
}

fn tool_strategy() -> impl Strategy<Value = CursorTool> {
    prop_oneof![
        Just(CursorTool::WebFetch),
        Just(CursorTool::BatchFetch),
        Just(CursorTool::ResearchSearch),
    ]
}

proptest! {
    #[test]
    fn round_trip_is_lossless(
        tool in tool_strategy(),
        ord in 0u32..100_000,
        timeout in 1u64..3_600_000,
        suffix in "[a-z0-9_]{1,24}",
    ) {
        let key = PlanKey::new(tool).str(&suffix).finish();
        let id = format!("chunk_{suffix}");
        let cursor = RetrievalCursor::new(tool, &key, ord as usize, &id, timeout);
        let encoded = cursor.encode().expect("encodes within the cap");
        prop_assert!(encoded.len() <= MAX_CURSOR_BYTES);
        let decoded = RetrievalCursor::decode(&encoded).expect("decodes");
        prop_assert_eq!(&decoded, &cursor);
    }

    #[test]
    fn arbitrary_bytes_never_panic(raw in "\\PC{0,700}") {
        let _ = RetrievalCursor::decode(&raw);
    }

    #[test]
    fn oversize_is_always_rejected(padding in 0usize..64) {
        let encoded = sample().encode().unwrap();
        let filler = "A".repeat(MAX_CURSOR_BYTES + padding);
        prop_assert_eq!(
            RetrievalCursor::decode(&filler),
            Err(CursorError::Invalid)
        );
        prop_assert!(RetrievalCursor::decode(&encoded).is_ok());
    }

    #[test]
    fn a_cursor_never_decodes_for_another_tool(other in tool_strategy()) {
        let cursor = sample();
        let decoded = RetrievalCursor::decode(&cursor.encode().unwrap()).unwrap();
        if other == CursorTool::WebFetch {
            prop_assert!(decoded.require_tool(other).is_ok());
        } else {
            prop_assert_eq!(decoded.require_tool(other), Err(CursorError::Mismatch));
        }
    }

    #[test]
    fn page_chain_covers_every_unit_once(
        unit_count in 1usize..24,
        capacity in 1usize..8,
    ) {
        let ids: Vec<String> = (0..unit_count).map(|i| format!("u_{i}")).collect();
        let key = PlanKey::new(CursorTool::BatchFetch).finish();
        let mut start = 0usize;
        let mut seen: Vec<usize> = Vec::new();
        let mut encoded: Option<String> = None;
        loop {
            let window = page_window(ids.len(), start, capacity, |_| 1);
            prop_assert!(!window.is_empty());
            seen.extend(window.start..window.end);
            let next = encode_next(CursorTool::BatchFetch, &key, &ids, window, 1000);
            if next.is_none() {
                break;
            }
            let cursor = RetrievalCursor::decode(next.as_ref().unwrap()).unwrap();
            start = locate_start(&ids, &cursor).expect("cursor id is always present");
            encoded = next;
            prop_assert!(start <= ids.len());
        }
        prop_assert_eq!(seen, (0..unit_count).collect::<Vec<_>>());
        prop_assert!(encoded.is_some() || unit_count <= capacity);
    }
}
