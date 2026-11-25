use std::collections::BTreeMap;

use serde::Serialize;
use sha2::{Digest, Sha256};
use similar::{Algorithm, ChangeTag, TextDiff};

pub(crate) const MAX_SNAPSHOT_BYTES: usize = 1_048_576;
pub(crate) const MAX_SNAPSHOT_LINES: usize = 4_096;
const MAX_DIFF_LINES: usize = 20_000;
const MAX_DIFF_CELLS: usize = 4_000_000;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct Change {
    pub status: &'static str,
    pub previous_fetched_at: Option<String>,
    pub current_fetched_at: String,
    pub chars_added: usize,
    pub chars_removed: usize,
    pub hunks: Vec<ChangeHunk>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct ChangeHunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub lines: Vec<String>,
}

pub(crate) fn normalize_text(text: &str) -> String {
    let lf = text.replace("\r\n", "\n").replace('\r', "\n");
    let (sanitized, _) = crate::core::sanitize::strip_control_chars(&lf);
    let mut lines: Vec<&str> = sanitized.lines().map(str::trim_end).collect();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

pub(crate) fn sha256(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

pub(crate) fn build_change(
    previous: Option<(&str, &str)>,
    current: Option<&str>,
    current_fetched_at: String,
    max_hunks: usize,
    max_diff_chars: usize,
) -> Change {
    let previous_text = previous.map(|(text, _)| text).unwrap_or("");
    let status = match (previous, current) {
        (None, Some(_)) => "new",
        (Some((old, _)), Some(new)) if sha256(old) == sha256(new) => "same",
        (Some(_), Some(_)) => "changed",
        (Some(_), None) => "removed",
        (None, None) => "new",
    };
    let previous_fetched_at = previous.map(|(_, fetched_at)| fetched_at.to_string());
    let current_text = current.unwrap_or("");
    if status == "same" || status == "removed" {
        return Change {
            status,
            previous_fetched_at,
            current_fetched_at,
            chars_added: 0,
            chars_removed: 0,
            hunks: Vec::new(),
            truncated: false,
        };
    }
    let old_lines = previous_text.lines().count();
    let new_lines = current_text.lines().count();
    if old_lines.saturating_add(new_lines) > MAX_DIFF_LINES
        || old_lines.saturating_mul(new_lines) > MAX_DIFF_CELLS
    {
        let (chars_added, chars_removed) = changed_line_char_counts(previous_text, current_text);
        return Change {
            status,
            previous_fetched_at,
            current_fetched_at,
            chars_added,
            chars_removed,
            hunks: Vec::new(),
            truncated: true,
        };
    }
    let diff = TextDiff::configure()
        .algorithm(Algorithm::Patience)
        .diff_lines(previous_text, current_text);
    let groups = diff.grouped_ops(3);
    let mut hunks = Vec::new();
    let mut chars_added = 0usize;
    let mut chars_removed = 0usize;
    let mut used_chars = 0usize;
    let mut truncated = false;
    for group in groups {
        if hunks.len() >= max_hunks {
            truncated = true;
            break;
        }
        let Some(first) = group.first() else {
            continue;
        };
        let Some(last) = group.last() else {
            continue;
        };
        let old_range = first.old_range().start..last.old_range().end;
        let new_range = first.new_range().start..last.new_range().end;
        let mut lines = Vec::new();
        let mut hunk_added = 0usize;
        let mut hunk_removed = 0usize;
        for op in &group {
            for change in diff.iter_changes(op) {
                let (prefix, counter) = match change.tag() {
                    ChangeTag::Equal => (' ', None),
                    ChangeTag::Insert => ('+', Some(&mut hunk_added)),
                    ChangeTag::Delete => ('-', Some(&mut hunk_removed)),
                };
                let value = change.value().strip_suffix('\n').unwrap_or(change.value());
                if let Some(counter) = counter {
                    *counter = counter.saturating_add(value.chars().count());
                }
                lines.push(format!("{prefix}{value}"));
            }
        }
        let hunk_chars = lines.iter().map(|line| line.chars().count()).sum::<usize>();
        if used_chars.saturating_add(hunk_chars) > max_diff_chars {
            truncated = true;
            break;
        }
        used_chars += hunk_chars;
        chars_added = chars_added.saturating_add(hunk_added);
        chars_removed = chars_removed.saturating_add(hunk_removed);
        hunks.push(ChangeHunk {
            old_start: old_range.start.saturating_add(1),
            old_lines: old_range.len(),
            new_start: new_range.start.saturating_add(1),
            new_lines: new_range.len(),
            lines,
        });
    }
    Change {
        status,
        previous_fetched_at,
        current_fetched_at,
        chars_added,
        chars_removed,
        hunks,
        truncated,
    }
}

fn changed_line_char_counts(old: &str, new: &str) -> (usize, usize) {
    let mut old_lines = BTreeMap::<&str, usize>::new();
    let mut new_lines = BTreeMap::<&str, usize>::new();
    for line in old.lines() {
        *old_lines.entry(line).or_default() += 1;
    }
    for line in new.lines() {
        *new_lines.entry(line).or_default() += 1;
    }
    let mut added = 0usize;
    let mut removed = 0usize;
    for (line, count) in &new_lines {
        let difference = count.saturating_sub(old_lines.get(line).copied().unwrap_or(0));
        added = added.saturating_add(difference.saturating_mul(line.chars().count()));
    }
    for (line, count) in &old_lines {
        let difference = count.saturating_sub(new_lines.get(line).copied().unwrap_or(0));
        removed = removed.saturating_add(difference.saturating_mul(line.chars().count()));
    }
    (added, removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_preserves_unicode_and_removes_only_trailing_line_whitespace() {
        assert_eq!(normalize_text("a  \r\n雪\t\r\n\n"), "a\n雪");
        assert_eq!(normalize_text("a\u{200b}b"), "ab");
    }

    #[test]
    fn same_text_has_no_hunks_after_normalization() {
        let old = normalize_text("alpha  \n");
        let new = normalize_text("alpha\n\n");
        let change = build_change(
            Some((&old, "2026-01-01T00:00:00Z")),
            Some(&new),
            "2026-01-02T00:00:00Z".into(),
            20,
            8192,
        );
        assert_eq!(change.status, "same");
        assert!(change.hunks.is_empty());
        assert_eq!(change.chars_added, 0);
        assert_eq!(change.chars_removed, 0);
    }

    #[test]
    fn removal_reports_no_hunks() {
        let change = build_change(
            Some(("alpha\nbeta", "2026-01-01T00:00:00Z")),
            None,
            "2026-01-02T00:00:00Z".into(),
            20,
            8192,
        );
        assert_eq!(change.status, "removed");
        assert!(change.hunks.is_empty());
        assert_eq!(change.chars_added, 0);
        assert_eq!(change.chars_removed, 0);
        assert_eq!(
            change.previous_fetched_at.as_deref(),
            Some("2026-01-01T00:00:00Z")
        );
    }

    #[test]
    fn changed_line_reports_stable_hunk_and_character_counts() {
        let a = "one\ntwo\nthree\nfour";
        let b = "one\nTWO\nthree\nfour";
        let first = build_change(
            Some((a, "2026-01-01T00:00:00Z")),
            Some(b),
            "2026-01-02T00:00:00Z".into(),
            20,
            8192,
        );
        let second = build_change(
            Some((a, "2026-01-01T00:00:00Z")),
            Some(b),
            "2026-01-02T00:00:00Z".into(),
            20,
            8192,
        );
        assert_eq!(
            serde_json::to_vec(&first).unwrap(),
            serde_json::to_vec(&second).unwrap()
        );
        assert_eq!(first.status, "changed");
        assert_eq!(first.chars_added, 3);
        assert_eq!(first.chars_removed, 3);
        assert_eq!(first.hunks.len(), 1);
        assert_eq!(
            first.hunks[0].lines,
            [" one", "-two", "+TWO", " three", " four"]
        );
    }

    #[test]
    fn insertion_deletion_and_unicode_report_exact_character_counts() {
        for (old, new, added, removed) in [
            ("a\nb", "a\nx\nb", 1, 0),
            ("a\nx\nb", "a\nb", 0, 1),
            ("雪", "雪🙂", 2, 1),
        ] {
            let result = build_change(Some((old, "before")), Some(new), "now".into(), 20, 8192);
            assert_eq!(result.chars_added, added, "{old:?} -> {new:?}");
            assert_eq!(result.chars_removed, removed, "{old:?} -> {new:?}");
        }
    }

    #[test]
    fn reorder_is_a_deterministic_change() {
        let first = build_change(
            Some(("a\nb", "before")),
            Some("b\na"),
            "now".into(),
            20,
            8192,
        );
        let second = build_change(
            Some(("a\nb", "before")),
            Some("b\na"),
            "now".into(),
            20,
            8192,
        );
        assert_eq!(
            serde_json::to_vec(&first).unwrap(),
            serde_json::to_vec(&second).unwrap()
        );
        assert_eq!(first.status, "changed");
        assert!(!first.hunks.is_empty());
        assert!(first.chars_added > 0);
        assert!(first.chars_removed > 0);
    }

    #[test]
    fn size_guard_returns_counts_without_hunks() {
        let old = (0..2_001)
            .map(|i| format!("old{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let new = (0..2_001)
            .map(|i| format!("new{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let result = build_change(Some((&old, "before")), Some(&new), "now".into(), 20, 8192);
        assert!(result.truncated);
        assert!(result.hunks.is_empty());
        assert!(result.chars_added > 0);
        assert!(result.chars_removed > 0);
    }

    #[test]
    fn hunk_and_character_limits_truncate_in_order() {
        let result = build_change(
            Some(("a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl", "before")),
            Some("A\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nL"),
            "now".into(),
            1,
            8192,
        );
        assert_eq!(result.hunks.len(), 1);
        assert!(result.truncated);
        let limited = build_change(
            Some(("old line", "before")),
            Some("new line"),
            "now".into(),
            20,
            2,
        );
        assert!(limited.hunks.is_empty());
        assert!(limited.truncated);
        assert_eq!(limited.chars_added, 0);
        assert_eq!(limited.chars_removed, 0);
    }
}
