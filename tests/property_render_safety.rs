use eggsearch::core::sanitize::{bound_text, frame, scan_injection_markers, strip_control_chars};
use proptest::prelude::*;

proptest! {
    #[test]
    fn strip_control_chars_never_panics_on_any_input(s in "\\P{Cc}*") {
        let _ = strip_control_chars(&s);
    }

    #[test]
    fn strip_control_chars_never_panics_on_all_controls(
        chars in proptest::collection::vec(prop_oneof![
            Just('\0'),
            Just('\r'),
            Just('\x01'),
            Just('\x08'),
            Just('\x0B'),
            Just('\x0C'),
            Just('\x0E'),
            Just('\x1F'),
            Just('\x7F'),
            Just('\u{202A}'),
            Just('\u{202E}'),
            Just('\u{2066}'),
            Just('\u{2069}'),
            Just('\u{200B}'),
            Just('\u{200D}'),
            Just('\u{FEFF}'),
        ], 0..100)
    ) {
        let s: String = chars.into_iter().collect();
        let (cleaned, removed) = strip_control_chars(&s);
        prop_assert!(removed <= s.chars().count(), "removed count cannot exceed total chars");
        prop_assert_eq!(cleaned.chars().count(), s.chars().count() - removed);
    }

    #[test]
    fn bound_text_output_never_exceeds_max_chars(
        s in "\\PC{0,500}",
        max_chars in 1usize..200usize
    ) {
        let (result, truncated) = bound_text(&s, max_chars);
        let char_count = result.chars().count();
        prop_assert!(char_count <= max_chars, "output {} chars exceeds max {}", char_count, max_chars);
        if s.chars().count() > max_chars {
            prop_assert!(truncated, "should be marked truncated");
        }
    }

    #[test]
    fn bound_text_with_zero_returns_empty(s in "\\PC{0,100}") {
        let (result, truncated) = bound_text(&s, 0);
        prop_assert!(result.is_empty(), "zero max_chars should return empty");
        prop_assert!(truncated, "zero max_chars should be truncated");
    }

    #[test]
    fn bound_text_appends_ellipsis_when_truncated(
        s in "[a-zA-Z]{2,100}",
        max_chars in 1usize..50usize
    ) {
        if s.chars().count() > max_chars {
            let (result, truncated) = bound_text(&s, max_chars);
            prop_assert!(truncated);
            prop_assert!(result.ends_with('…'), "truncated text should end with ellipsis");
            prop_assert_eq!(result.chars().count(), max_chars, "output should be exactly max_chars");
        }
    }

    #[test]
    fn frame_output_starts_and_ends_with_delimiters(
        content in "\\PC{0,100}",
        field in "[a-z]{1,20}",
        id in "[a-z0-9]{1,20}"
    ) {
        let framed = frame(&content, &field, &id);
        prop_assert!(framed.starts_with("<<<EXTERNAL_UNTRUSTED"), "should start with delimiter");
        prop_assert!(framed.ends_with("<<<END>>>"), "should end with delimiter");
        prop_assert!(framed.contains(&format!("field={}", field)), "should contain field name");
        prop_assert!(framed.contains(&format!("id={}", id)), "should contain id");
    }

    #[test]
    fn frame_preserves_content(
        content in "[a-zA-Z0-9 ]{1,100}"
    ) {
        let framed = frame(&content, "test", "123");
        prop_assert!(framed.contains(&content), "framed output should contain original content");
    }

    #[test]
    fn scan_injection_markers_never_panics_on_any_input(s in "\\PC{0,200}") {
        let _ = scan_injection_markers(&s);
    }

    #[test]
    fn scan_injection_markers_returns_offsets_within_bounds(
        s in "\\PC{0,200}"
    ) {
        let hits = scan_injection_markers(&s);
        for hit in &hits {
            prop_assert!(hit.byte_offset < s.len(), "offset {} beyond string length {}", hit.byte_offset, s.len());
        }
    }

    #[test]
    fn scan_injection_markers_deterministic(
        s in "\\PC{0,200}"
    ) {
        let h1 = scan_injection_markers(&s);
        let h2 = scan_injection_markers(&s);
        prop_assert_eq!(h1, h2, "injection scan should be deterministic");
    }

    #[test]
    fn strip_control_chars_idempotent(
        s in "\\PC{0,200}"
    ) {
        let (c1, _) = strip_control_chars(&s);
        let (c2, _) = strip_control_chars(&c1);
        prop_assert_eq!(c1, c2, "strip_control_chars should be idempotent");
    }

    #[test]
    fn bound_text_idempotent(
        s in "\\PC{0,200}",
        max_chars in 1usize..100usize
    ) {
        let (b1, _) = bound_text(&s, max_chars);
        let (b2, _) = bound_text(&b1, max_chars);
        prop_assert_eq!(b1, b2, "bound_text should be idempotent");
    }

    #[test]
    fn strip_control_preserves_safe_chars(
        safe_chars in proptest::collection::vec(prop_oneof![
            Just('a'), Just('z'), Just('0'), Just('9'),
            Just(' '), Just('\n'), Just('\t'),
            Just('!'), Just('@'), Just('#'), Just('$'),
            Just('中'), Just('É'), Just('ñ'), Just('α'),
            Just('🚀'), Just('€'), Just('£'),
        ], 0..100)
    ) {
        let s: String = safe_chars.into_iter().collect();
        let (cleaned, removed) = strip_control_chars(&s);
        prop_assert_eq!(removed, 0, "safe chars should not be removed");
        prop_assert_eq!(cleaned, s, "safe chars should be preserved exactly");
    }

    #[test]
    fn bound_text_preserves_prefix(
        s in "[a-zA-Z]{10,200}",
        max_chars in 5usize..15usize
    ) {
        let (result, _) = bound_text(&s, max_chars);
        let prefix: String = s.chars().take(max_chars - 1).collect();
        prop_assert!(result.starts_with(&prefix), "should preserve prefix up to max_chars-1");
    }

    #[test]
    fn unsafe_elements_never_appear_in_framed_output(
        content in "[a-zA-Z0-9 ]{1,100}",
        field in "[a-z_]+",
        id in "[a-zA-Z0-9_-]+"
    ) {
        let (cleaned, _) = strip_control_chars(&content);
        let framed = frame(&cleaned, &field, &id);
        prop_assert!(!framed.contains("<script>"), "script tag in framed output");
        prop_assert!(!framed.contains("<iframe>"), "iframe tag in framed output");
        prop_assert!(!framed.contains("<object>"), "object tag in framed output");
        prop_assert!(!framed.contains("<embed>"), "embed tag in framed output");
    }

    #[test]
    fn bound_text_never_splits_utf8_invalid_boundary(
        s in "\\PC{0,500}",
        max_chars in 1usize..200usize
    ) {
        let (result, _truncated) = bound_text(&s, max_chars);
        prop_assert!(
            std::str::from_utf8(result.as_bytes()).is_ok(),
            "bounded text must be valid UTF-8"
        );
    }
}
