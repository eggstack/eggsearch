use eggsearch::core::sanitize::{bound_text, frame, scan_injection_markers, strip_control_chars};
use proptest::prelude::*;

const ELLIPSIS: char = '\u{2026}';

fn is_unsafe_char(c: char) -> bool {
    matches!(c,
        '\0' | '\r'
        | '\x01'..='\x08' | '\x0B' | '\x0C' | '\x0E'..='\x1F' | '\x7F'
        | '\u{200E}' | '\u{200F}'
        | '\u{202A}'..='\u{202E}'
        | '\u{2066}'..='\u{2069}'
        | '\u{200B}'..='\u{200D}'
        | '\u{FEFF}'
    )
}

fn printable_ascii_with_newline_tab() -> impl Strategy<Value = String> {
    "[\\x20-\\x7e\\n\\t]*"
}

fn arbitrary_string_with_known_unsafe() -> impl Strategy<Value = String> {
    (
        "[\\x20-\\x7e]*",
        prop::collection::vec(
            prop_oneof![
                Just('\0'),
                Just('\r'),
                Just('\x01'),
                Just('\x08'),
                Just('\x0B'),
                Just('\x0C'),
                Just('\x0E'),
                Just('\x1F'),
                Just('\x7F'),
                Just('\u{200E}'),
                Just('\u{200F}'),
                Just('\u{202A}'),
                Just('\u{202E}'),
                Just('\u{2066}'),
                Just('\u{2069}'),
                Just('\u{200B}'),
                Just('\u{200D}'),
                Just('\u{FEFF}'),
            ],
            0..5,
        ),
    )
        .prop_map(|(safe, unsafe_chars)| {
            let mut s = safe;
            for c in unsafe_chars {
                s.push(c);
            }
            s
        })
}

proptest! {
    #[test]
    fn strip_output_has_no_unsafe(s in "[\\x00-\\x7f\\u{200B}-\\u{200D}\\u{200E}\\u{200F}\\u{202A}-\\u{202E}\\u{2066}-\\u{2069}\\u{FEFF}]*") {
        let (out, _removed) = strip_control_chars(&s);
        for c in out.chars() {
            prop_assert!(!is_unsafe_char(c), "unsafe char found in output: {c:?}");
        }
    }

    #[test]
    fn strip_idempotent(s in arbitrary_string_with_known_unsafe()) {
        let (once, _) = strip_control_chars(&s);
        let (twice, _) = strip_control_chars(&once);
        prop_assert_eq!(once, twice);
    }

    #[test]
    fn strip_removal_count_correct(s in "[\\x00-\\x7f\\u{200B}-\\u{200D}\\u{200E}\\u{200F}\\u{202A}-\\u{202E}\\u{2066}-\\u{2069}\\u{FEFF}]*") {
        let (out, removed) = strip_control_chars(&s);
        let input_chars = s.chars().count();
        let output_chars = out.chars().count();
        prop_assert_eq!(input_chars - output_chars, removed);
    }

    #[test]
    fn strip_preserves_newline_and_tab(s in printable_ascii_with_newline_tab()) {
        let (out, _) = strip_control_chars(&s);
        let original_n = s.matches('\n').count();
        let original_t = s.matches('\t').count();
        let out_n = out.matches('\n').count();
        let out_t = out.matches('\t').count();
        prop_assert_eq!(original_n, out_n);
        prop_assert_eq!(original_t, out_t);
    }

    #[test]
    fn bound_output_length_within_limit(s in "[a-zA-Z0-9 ]*", max_chars in 1usize..200) {
        let (out, _truncated) = bound_text(&s, max_chars);
        prop_assert!(out.chars().count() <= max_chars);
    }

    #[test]
    fn bound_truncated_flag_matches(s in "[a-zA-Z0-9 ]*", max_chars in 1usize..200) {
        let (out, truncated) = bound_text(&s, max_chars);
        let input_len = s.chars().count();
        if truncated {
            prop_assert!(input_len > max_chars);
            prop_assert!(out.ends_with(ELLIPSIS));
        } else {
            prop_assert!(input_len <= max_chars);
            prop_assert_eq!(out, s);
        }
    }
}

proptest! {
    #[test]
    fn bound_truncated_ends_with_ellipsis(s in ".{10,200}", max_chars in 1usize..10) {
        let (out, truncated) = bound_text(&s, max_chars);
        if truncated {
            prop_assert!(out.ends_with(ELLIPSIS), "truncated output should end with ellipsis");
        }
    }

    #[test]
    fn bound_zero_max_returns_empty(s in "[a-zA-Z0-9]*") {
        let (out, truncated) = bound_text(&s, 0);
        prop_assert_eq!(out, "");
        prop_assert!(truncated);
    }

    #[test]
    fn scan_never_panics(s in "[\\x00-\\x7f\\u{200B}-\\u{200D}\\u{200E}\\u{200F}\\u{202A}-\\u{202E}\\u{2066}-\\u{2069}\\u{FEFF}]*") {
        let _hits = scan_injection_markers(&s);
    }

    #[test]
    fn scan_byte_offsets_valid(s in "[a-zA-Z0-9 ]*") {
        let hits = scan_injection_markers(&s);
        let byte_len = s.len();
        for hit in &hits {
            prop_assert!(hit.byte_offset < byte_len,
                "byte_offset {} >= input length {}", hit.byte_offset, byte_len);
        }
    }

    #[test]
    fn scan_valid_pattern_names(s in "[a-zA-Z0-9 ]*") {
        let valid = ["ignore_previous", "disregard_all", "system_colon",
            "assistant_colon", "im_start", "im_end", "chatml_tag"];
        let hits = scan_injection_markers(&s);
        for hit in &hits {
            prop_assert!(valid.contains(&hit.pattern),
                "invalid pattern name: {}", hit.pattern);
        }
    }
}

proptest! {
    #[test]
    fn bound_text_unicode_boundary_never_panics(s in "\\PC{0,500}", max_chars in 1usize..200) {
        let (out, _truncated) = bound_text(&s, max_chars);
        prop_assert!(out.chars().count() <= max_chars);
    }

    #[test]
    fn bound_text_framing_overhead_cannot_exceed_cap(
        s in "[a-zA-Z0-9 ]{10,500}",
        max_chars in 10usize..200usize,
        field in "[a-z_]+",
        id in "[a-zA-Z0-9_-]+"
    ) {
        let (bounded, _) = bound_text(&s, max_chars);
        let framed = frame(&bounded, &field, &id);
        let inner_chars = framed
            .lines()
            .skip(1)
            .take(framed.lines().count().saturating_sub(2))
            .collect::<String>()
            .chars()
            .count();
        prop_assert!(
            inner_chars <= max_chars,
            "framed inner content {} chars exceeds max_chars {}",
            inner_chars,
            max_chars
        );
    }

    #[test]
    fn frame_starts_and_ends_correctly(s in "[a-zA-Z0-9 ]*", field in "[a-z_]*", id in "[a-zA-Z0-9_-]*") {
        let out = frame(&s, &field, &id);
        prop_assert!(out.starts_with("<<<EXTERNAL_UNTRUSTED field="),
            "output doesn't start correctly: {:?}", &out[..40.min(out.len())]);
        prop_assert!(out.ends_with("<<<END>>>"),
            "output doesn't end correctly: {:?}", &out[out.len().saturating_sub(20)..]);
    }

    #[test]
    fn frame_contains_field_and_id(s in "[a-zA-Z0-9 ]*", field in "[a-z_]+", id in "[a-zA-Z0-9_-]+") {
        let out = frame(&s, &field, &id);
        let expected_field = format!("field={field}");
        let expected_id = format!("id={id}");
        prop_assert!(out.contains(&expected_field),
            "output missing field marker: {:?}", &out[..60.min(out.len())]);
        prop_assert!(out.contains(&expected_id),
            "output missing id marker: {:?}", &out[..60.min(out.len())]);
    }
}
