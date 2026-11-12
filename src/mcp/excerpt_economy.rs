use crate::core::identity::{write_str, FnvHasher};
use serde_json::Value;
use std::cmp::Ordering;

pub(crate) use crate::core::config::{
    DEFAULT_EXCERPT_BUDGET_CHARS, MAX_EXCERPT_BUDGET_CHARS, MIN_EXCERPT_BUDGET_CHARS,
};

pub(crate) const MAX_ECONOMY_PASSAGES: usize = 256;
pub(crate) const MIN_SHINGLES_FOR_SIMILARITY: usize = 4;
pub(crate) const MIN_PARTIAL_CHARS: usize = 200;
pub(crate) const SHINGLE_LEN: usize = 4;
pub(crate) const DUPLICATE_OVERLAP: f64 = 0.80;

const FRAME_HEADER: &str = "<<<EXTERNAL_UNTRUSTED";
const FRAME_FOOTER: &str = "<<<END>>>";
const FRAME_CLOSE: &str = ">>>";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ExcerptEconomyReport {
    pub budget_chars: usize,
    pub considered_count: usize,
    pub admitted_count: usize,
    pub redundant_count: usize,
    pub over_budget_count: usize,
    pub clipped_count: usize,
    pub chars_before: usize,
    pub chars_after: usize,
}

impl ExcerptEconomyReport {
    #[cfg(test)]
    fn counts_balance(&self) -> bool {
        self.admitted_count + self.redundant_count + self.over_budget_count == self.considered_count
    }

    fn into_value(self) -> Value {
        serde_json::json!({
            "budget_chars": self.budget_chars,
            "considered_count": self.considered_count,
            "admitted_count": self.admitted_count,
            "redundant_count": self.redundant_count,
            "over_budget_count": self.over_budget_count,
            "clipped_count": self.clipped_count,
            "chars_before": self.chars_before,
            "chars_after": self.chars_after,
        })
    }
}

struct ShingleIndex {
    tokens: Vec<String>,
    shingles: Vec<(u64, u32)>,
}

struct EconomyState {
    report: ExcerptEconomyReport,
    admitted: Vec<ShingleIndex>,
    remaining: usize,
    examined: usize,
}

impl EconomyState {
    fn new(budget_chars: usize) -> Self {
        Self {
            report: ExcerptEconomyReport {
                budget_chars,
                ..ExcerptEconomyReport::default()
            },
            admitted: Vec::new(),
            remaining: budget_chars,
            examined: 0,
        }
    }
}

enum TextDecision {
    Keep,
    Replace(String),
    Drop,
}

pub(crate) fn apply(value: &mut Value, budget_chars: usize) -> Option<ExcerptEconomyReport> {
    let budget_chars = budget_chars.clamp(MIN_EXCERPT_BUDGET_CHARS, MAX_EXCERPT_BUDGET_CHARS);
    let mut state = EconomyState::new(budget_chars);
    let mut walked = false;
    if let Some(results) = value.get_mut("results").and_then(Value::as_array_mut) {
        for card in results.iter_mut() {
            process_card(card, &mut state);
        }
        walked = true;
    } else if let Some(groups) = value.get_mut("groups").and_then(Value::as_array_mut) {
        for group in groups.iter_mut() {
            if let Some(results) = group.get_mut("results").and_then(Value::as_array_mut) {
                for card in results.iter_mut() {
                    process_card(card, &mut state);
                }
            }
        }
        walked = true;
    }
    if !walked {
        return None;
    }
    let report = state.report;
    if let Some(object) = value.as_object_mut() {
        object.insert("excerpt_economy".to_owned(), report.into_value());
    }
    Some(report)
}

fn process_card(card: &mut Value, state: &mut EconomyState) {
    let Some(object) = card.as_object_mut() else {
        return;
    };
    let mut suppressed = 0usize;
    let snippet = object
        .get("snippet")
        .and_then(Value::as_str)
        .map(str::to_owned);
    if let Some(snippet) = snippet {
        match consider(&snippet, state) {
            TextDecision::Keep => {}
            TextDecision::Replace(text) => {
                object.insert("snippet".to_owned(), Value::String(text));
            }
            TextDecision::Drop => {
                object.remove("snippet");
                suppressed += 1;
            }
        }
    }
    if let Some(excerpts) = object.get_mut("excerpts").and_then(Value::as_array_mut) {
        let mut retained: Vec<Value> = Vec::with_capacity(excerpts.len());
        for mut excerpt in std::mem::take(excerpts) {
            let text = excerpt
                .get("text")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let Some(text) = text else {
                retained.push(excerpt);
                continue;
            };
            match consider(&text, state) {
                TextDecision::Keep => retained.push(excerpt),
                TextDecision::Replace(text) => {
                    if let Some(target) = excerpt.get_mut("text") {
                        *target = Value::String(text);
                    }
                    retained.push(excerpt);
                }
                TextDecision::Drop => suppressed += 1,
            }
        }
        *excerpts = retained;
    }
    if suppressed > 0 {
        object.insert("excerpts_suppressed".to_owned(), suppressed.into());
    }
}

fn consider(text: &str, state: &mut EconomyState) -> TextDecision {
    let chars = text.chars().count();
    state.report.considered_count += 1;
    state.report.chars_before += chars;
    if state.remaining == 0 || state.examined >= MAX_ECONOMY_PASSAGES {
        state.report.over_budget_count += 1;
        return TextDecision::Drop;
    }
    state.examined += 1;
    let candidate = shingle_index(normalize_tokens(text));
    if state
        .admitted
        .iter()
        .any(|admitted| is_duplicate(&candidate, admitted))
    {
        state.report.redundant_count += 1;
        return TextDecision::Drop;
    }
    if chars <= state.remaining {
        state.remaining -= chars;
        state.report.admitted_count += 1;
        state.report.chars_after += chars;
        state.admitted.push(candidate);
        return TextDecision::Keep;
    }
    if state.remaining >= MIN_PARTIAL_CHARS {
        let clipped = clip_passage(text, state.remaining);
        let clipped_chars = clipped.chars().count();
        if clipped_chars <= state.remaining {
            state.remaining -= clipped_chars;
            state.report.admitted_count += 1;
            state.report.clipped_count += 1;
            state.report.chars_after += clipped_chars;
            state.admitted.push(candidate);
            return TextDecision::Replace(clipped);
        }
    }
    state.report.over_budget_count += 1;
    TextDecision::Drop
}

fn clip_passage(text: &str, max_chars: usize) -> String {
    crate::mcp::projection::clip_framed_text(text, max_chars)
}

fn normalize_tokens(text: &str) -> Vec<String> {
    strip_frame(text)
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
        .collect()
}

fn strip_frame(text: &str) -> &str {
    if !text.starts_with(FRAME_HEADER) {
        return text;
    }
    let Some(close) = text.find(FRAME_CLOSE) else {
        return text;
    };
    let body = &text[close + FRAME_CLOSE.len()..];
    match body.rfind(FRAME_FOOTER) {
        Some(end) => &body[..end],
        None => body,
    }
}

fn shingle_index(tokens: Vec<String>) -> ShingleIndex {
    let mut shingles: Vec<(u64, u32)> = tokens
        .windows(SHINGLE_LEN)
        .enumerate()
        .map(|(start, window)| (window_fingerprint(window), start as u32))
        .collect();
    shingles.sort_unstable();
    shingles.dedup_by_key(|entry| entry.0);
    ShingleIndex { tokens, shingles }
}

fn window_fingerprint(window: &[String]) -> u64 {
    let mut hasher = FnvHasher::new();
    for token in window {
        write_str(&mut hasher, token);
    }
    hasher.finish()
}

fn window_matches(
    left: &[String],
    left_start: usize,
    right: &[String],
    right_start: usize,
) -> bool {
    let end = left_start.saturating_add(SHINGLE_LEN);
    let other_end = right_start.saturating_add(SHINGLE_LEN);
    end <= left.len()
        && other_end <= right.len()
        && left[left_start..end] == right[right_start..other_end]
}

fn confirmed_overlap(candidate: &ShingleIndex, admitted: &ShingleIndex) -> usize {
    let mut matched = 0usize;
    let mut left = 0usize;
    let mut right = 0usize;
    while left < candidate.shingles.len() && right < admitted.shingles.len() {
        match candidate.shingles[left].0.cmp(&admitted.shingles[right].0) {
            Ordering::Less => left += 1,
            Ordering::Greater => right += 1,
            Ordering::Equal => {
                let left_start = candidate.shingles[left].1 as usize;
                let right_start = admitted.shingles[right].1 as usize;
                if window_matches(&candidate.tokens, left_start, &admitted.tokens, right_start) {
                    matched += 1;
                }
                left += 1;
                right += 1;
            }
        }
    }
    matched
}

fn is_duplicate(candidate: &ShingleIndex, admitted: &ShingleIndex) -> bool {
    if candidate.tokens.is_empty() || admitted.tokens.is_empty() {
        return false;
    }
    if candidate.tokens == admitted.tokens {
        return true;
    }
    let floor = candidate.shingles.len().min(admitted.shingles.len());
    if floor < MIN_SHINGLES_FOR_SIMILARITY {
        return false;
    }
    let matched = confirmed_overlap(candidate, admitted);
    matched as f64 / floor as f64 >= DUPLICATE_OVERLAP
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn framed(field: &str, id: &str, body: &str) -> String {
        format!("<<<EXTERNAL_UNTRUSTED field={field} id={id}>>>\n{body}\n<<<END>>>")
    }

    fn passage_chars(value: &Value) -> usize {
        let mut total = 0usize;
        let mut walk = |card: &Value| {
            if let Some(snippet) = card.get("snippet").and_then(Value::as_str) {
                total += snippet.chars().count();
            }
            if let Some(excerpts) = card.get("excerpts").and_then(Value::as_array) {
                for excerpt in excerpts {
                    if let Some(text) = excerpt.get("text").and_then(Value::as_str) {
                        total += text.chars().count();
                    }
                }
            }
        };
        if let Some(results) = value.get("results").and_then(Value::as_array) {
            for card in results {
                walk(card);
            }
        }
        if let Some(groups) = value.get("groups").and_then(Value::as_array) {
            for group in groups {
                if let Some(results) = group.get("results").and_then(Value::as_array) {
                    for card in results {
                        walk(card);
                    }
                }
            }
        }
        total
    }

    fn duplicate(left: &str, right: &str) -> bool {
        is_duplicate(
            &shingle_index(normalize_tokens(left)),
            &shingle_index(normalize_tokens(right)),
        )
    }

    #[test]
    fn frame_envelope_is_stripped_before_tokenizing() {
        let body = "the quick brown fox jumps over the lazy dog";
        assert_eq!(
            normalize_tokens(&framed("snippet", "src_a", body)),
            normalize_tokens(body)
        );
        assert_eq!(normalize_tokens("Read more..."), vec!["read", "more"]);
    }

    #[test]
    fn frame_tokens_cannot_inflate_similarity() {
        let shared = "alpha beta gamma delta epsilon zeta eta theta iota kappa";
        let left = framed("snippet", "src_a", shared);
        let right = framed("title", "src_b", "lambda mu nu xi omicron pi rho sigma tau");
        assert!(!duplicate(&left, &right));
        assert!(duplicate(&left, &framed("excerpt", "src_c", shared)));
    }

    #[test]
    fn exact_duplicates_collapse_first_wins() {
        let mut value = json!({
            "results": [
                {"stable_id": "src_a", "snippet": framed("snippet", "src_a", "identical passage text")},
                {"stable_id": "src_b", "snippet": framed("snippet", "src_b", "identical passage text")},
                {"stable_id": "src_c", "snippet": framed("snippet", "src_c", "a wholly different statement")}
            ]
        });
        let report = apply(&mut value, DEFAULT_EXCERPT_BUDGET_CHARS).expect("report");
        assert!(report.counts_balance());
        assert_eq!(report.considered_count, 3);
        assert_eq!(report.redundant_count, 1);
        assert_eq!(report.admitted_count, 2);
        assert_eq!(
            value["results"][0]["snippet"],
            framed("snippet", "src_a", "identical passage text")
        );
        assert!(value["results"][1].get("snippet").is_none());
        assert_eq!(value["results"][1]["excerpts_suppressed"], 1);
        assert!(value["results"][0].get("excerpts_suppressed").is_none());
        assert_eq!(value["results"][0]["stable_id"], "src_a");
    }

    #[test]
    fn shingle_overlap_at_threshold_collapses() {
        let admitted = "alpha bravo charlie delta echo foxtrot golf hotel";
        let candidate = "alpha bravo charlie delta echo foxtrot golf india";
        assert!(duplicate(admitted, candidate));
    }

    #[test]
    fn shingle_overlap_below_threshold_is_retained() {
        let admitted = "alpha bravo charlie delta echo foxtrot golf hotel";
        let candidate = "hotel india juliet alpha bravo charlie delta echo";
        assert!(!duplicate(admitted, candidate));
    }

    #[test]
    fn differing_identifier_is_retained() {
        let admitted = "GHSA-2024-1111 affects libfoo before 1.2.3";
        let candidate = "GHSA-2024-2222 affects libfoo before 1.2.3";
        assert!(!duplicate(admitted, candidate));
    }

    #[test]
    fn short_passages_compare_by_equality_only() {
        let admitted = "rust async runtime";
        let candidate = "rust async runtime guide";
        assert!(duplicate(admitted, "rust async runtime"));
        assert!(!duplicate(admitted, candidate));
    }

    #[test]
    fn empty_passages_are_never_duplicates() {
        assert!(!duplicate("", ""));
        assert!(!duplicate(&framed("snippet", "src_a", ""), ""));
    }

    #[test]
    fn budget_clips_when_partial_room_and_drops_below_floor() {
        let mut value = json!({
            "results": [
                {"snippet": framed("snippet", "src_a", &"a".repeat(300))},
                {"snippet": framed("snippet", "src_b", &"b".repeat(300))},
                {"snippet": "tail"}
            ]
        });
        let report = apply(&mut value, 256).expect("report");
        assert_eq!(report.budget_chars, 256);
        assert!(report.counts_balance());
        assert_eq!(report.admitted_count, 1);
        assert_eq!(report.clipped_count, 1);
        assert_eq!(report.over_budget_count, 2);
        assert_eq!(passage_chars(&value), 256);
        let clipped = value["results"][0]["snippet"].as_str().expect("clipped");
        assert!(clipped.starts_with("<<<EXTERNAL_UNTRUSTED"));
        assert!(clipped.ends_with("<<<END>>>"));
        assert_eq!(clipped.matches("<<<END>>>").count(), 1);
    }

    #[test]
    fn budget_below_minimum_is_clamped() {
        let mut value = json!({"results": [{"snippet": "hello"}]});
        let report = apply(&mut value, 4).expect("report");
        assert_eq!(report.budget_chars, MIN_EXCERPT_BUDGET_CHARS);
    }

    #[test]
    fn saturation_terminates_and_respects_the_hard_budget() {
        let results: Vec<Value> = (0..MAX_ECONOMY_PASSAGES + 24)
            .map(|index| {
                let body: String = (0..40).map(|word| format!("t{index}w{word} ")).collect();
                json!({
                    "stable_id": format!("src_{index}"),
                    "snippet": framed("snippet", &format!("src_{index}"), &body)
                })
            })
            .collect();
        let mut value = json!({"results": results});
        let report = apply(&mut value, DEFAULT_EXCERPT_BUDGET_CHARS).expect("report");
        assert_eq!(report.considered_count, MAX_ECONOMY_PASSAGES + 24);
        assert_eq!(report.redundant_count, 0);
        assert!(report.admitted_count + report.redundant_count <= MAX_ECONOMY_PASSAGES);
        assert!(report.over_budget_count >= 24);
        assert!(report.counts_balance());
        assert!(
            passage_chars(&value) <= DEFAULT_EXCERPT_BUDGET_CHARS,
            "{}",
            passage_chars(&value)
        );
        assert_eq!(
            value["results"].as_array().unwrap().len(),
            MAX_ECONOMY_PASSAGES + 24
        );
    }

    #[test]
    fn groups_are_walked_in_response_order() {
        let text = framed("snippet", "src_a", "shared evidence statement");
        let mut value = json!({
            "groups": [
                {"group": "one", "results": [{"stable_id": "src_a", "snippet": text.clone()}]},
                {"group": "two", "results": [{"stable_id": "src_b", "snippet": text}]}
            ]
        });
        let report = apply(&mut value, DEFAULT_EXCERPT_BUDGET_CHARS).expect("report");
        assert!(report.counts_balance());
        assert_eq!(report.redundant_count, 1);
        assert_eq!(value["groups"][0]["results"][0]["stable_id"], "src_a");
        assert!(value["groups"][0]["results"][0].get("snippet").is_some());
        assert!(value["groups"][1]["results"][0].get("snippet").is_none());
        assert_eq!(value["groups"][1]["results"][0]["excerpts_suppressed"], 1);
    }

    #[test]
    fn absent_card_array_returns_no_report() {
        let mut value = json!({"vulnerabilities": [{"id": "CVE-2024-1"}]});
        assert!(apply(&mut value, DEFAULT_EXCERPT_BUDGET_CHARS).is_none());
        assert!(value.get("excerpt_economy").is_none());
    }

    #[test]
    fn apply_is_deterministic() {
        let text = framed("snippet", "src_a", "repeated evidence text for determinism");
        let payload = json!({
            "results": [
                {"stable_id": "src_a", "snippet": text.clone()},
                {"stable_id": "src_b", "snippet": text},
                {"stable_id": "src_c", "snippet": framed("snippet", "src_c", &"z".repeat(600))}
            ]
        });
        let mut first = payload.clone();
        let mut second = payload;
        let first_report = apply(&mut first, 512).expect("report");
        let second_report = apply(&mut second, 512).expect("report");
        assert_eq!(first_report, second_report);
        assert_eq!(first, second);
    }
}
