use eggsearch::fetch::render::{render_code, render_csv, render_diff, render_plaintext};
use proptest::prelude::*;

fn language_strategy() -> impl Strategy<Value = Option<String>> {
    prop_oneof![
        Just(None),
        "rust".prop_map(Some),
        "python".prop_map(Some),
        "javascript".prop_map(Some),
        "go".prop_map(Some),
        "bash".prop_map(Some),
    ]
}

proptest! {
    #[test]
    fn render_code_never_panics(
        text in "\\PC{0,5000}",
        lang in language_strategy(),
        max_chars in 100usize..50000usize
    ) {
        let _ = render_code(&text, lang.as_deref(), max_chars);
    }

    #[test]
    fn render_diff_never_panics(
        text in "\\PC{0,5000}",
        max_chars in 100usize..50000usize
    ) {
        let _ = render_diff(&text, max_chars);
    }

    #[test]
    fn render_plaintext_never_panics(
        text in "\\PC{0,5000}",
        max_chars in 100usize..50000usize
    ) {
        let _ = render_plaintext(&text, max_chars);
    }

    #[test]
    fn render_csv_never_panics(
        text in "\\PC{0,5000}",
        max_chars in 100usize..50000usize
    ) {
        let _ = render_csv(&text, max_chars);
    }

    #[test]
    fn render_code_output_bounded(
        text in "[a-zA-Z0-9 \\n]{100,5000}",
        max_chars in 100usize..5000usize
    ) {
        let result = render_code(&text, None, max_chars);
        let total_chars: usize = result.blocks.iter().map(|b| b.text.chars().count()).sum();
        prop_assert!(
            total_chars <= max_chars + 200,
            "total block text {} exceeds max_chars {} significantly",
            total_chars, max_chars
        );
    }

    #[test]
    fn render_diff_output_bounded(
        text in "[a-zA-Z0-9 +\\-\\n]{100,5000}",
        max_chars in 100usize..5000usize
    ) {
        let result = render_diff(&text, max_chars);
        let total_chars: usize = result.blocks.iter().map(|b| b.text.chars().count()).sum();
        prop_assert!(
            total_chars <= max_chars + 200,
            "total block text {} exceeds max_chars {}",
            total_chars, max_chars
        );
    }

    #[test]
    fn render_code_deterministic(
        text in "[a-zA-Z0-9 \\n]{10,500}",
        max_chars in 100usize..2000usize
    ) {
        let r1 = render_code(&text, None, max_chars);
        let r2 = render_code(&text, None, max_chars);
        prop_assert_eq!(r1.blocks.len(), r2.blocks.len());
        for (b1, b2) in r1.blocks.iter().zip(r2.blocks.iter()) {
            prop_assert_eq!(&b1.text, &b2.text, "block text should be deterministic");
        }
    }

    #[test]
    fn render_code_with_empty_input(max_chars in 100usize..5000usize) {
        let result = render_code("", None, max_chars);
        prop_assert!(result.blocks.is_empty(), "empty input should produce no blocks");
    }

    #[test]
    fn render_diff_with_empty_input(max_chars in 100usize..5000usize) {
        let result = render_diff("", max_chars);
        prop_assert!(result.blocks.is_empty(), "empty input should produce no blocks");
    }

    #[test]
    fn render_code_line_numbers_monotonic(
        text in "[a-zA-Z0-9 \\n]{100,2000}",
        max_chars in 500usize..5000usize
    ) {
        let result = render_code(&text, None, max_chars);
        let mut prev_end = 0;
        for block in &result.blocks {
            if let (Some(start), Some(end)) = (block.line_start, block.line_end) {
                prop_assert!(start <= end, "line_start {} > line_end {}", start, end);
                prop_assert!(start > prev_end || prev_end == 0, "line ranges should not overlap");
                prev_end = end;
            }
        }
    }

    #[test]
    fn render_code_language_set_when_provided(
        text in "[a-zA-Z0-9 \\n]{10,200}",
        lang in "[a-z]{2,10}"
    ) {
        let result = render_code(&text, Some(&lang), 5000);
        for block in &result.blocks {
            if block.kind == eggsearch::core::document::BlockKind::Code {
                prop_assert!(
                    block.language.is_some(),
                    "code block should have language when provided"
                );
            }
        }
    }

    #[test]
    fn render_plaintext_preserves_newlines(
        text in "[a-zA-Z0-9]{2,50}",
        count in 2usize..10usize
    ) {
        let repeated = std::iter::repeat_n(&text, count).map(|s| s.as_str()).collect::<Vec<_>>().join("\n");
        let result = render_plaintext(&repeated, 50000);
        let all_text: String = result.blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join("\n");
        prop_assert!(
            all_text.contains(&text),
            "plaintext renderer should preserve content"
        );
    }
}
