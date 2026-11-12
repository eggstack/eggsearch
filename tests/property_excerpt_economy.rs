use std::collections::HashSet;

use eggsearch::mcp::projection::{project, project_with_limits, ProjectionLimits, ResponseDetail};
use proptest::prelude::*;
use serde_json::{json, Value};

const WORDS: &[&str] = &[
    "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel", "india", "juliet",
    "kilo", "lima", "mike", "november", "oscar", "papa",
];

fn word() -> impl Strategy<Value = String> {
    prop::sample::select(WORDS).prop_map(str::to_owned)
}

fn passage() -> impl Strategy<Value = String> {
    (prop::collection::vec(word(), 0..14), any::<bool>()).prop_map(|(tokens, framed)| {
        let body = tokens.join(" ");
        if framed {
            format!("<<<EXTERNAL_UNTRUSTED field=snippet id=src_prop>>>\n{body}\n<<<END>>>")
        } else {
            body
        }
    })
}

fn limits(excerpt_budget_chars: usize) -> ProjectionLimits {
    ProjectionLimits {
        brief_max_chars: 8_000,
        excerpt_budget_chars,
    }
}

fn payload_from(cards: Vec<(String, Vec<String>)>) -> Value {
    let results: Vec<Value> = cards
        .into_iter()
        .enumerate()
        .map(|(index, (snippet, excerpts))| {
            json!({
                "id": format!("card_{index}"),
                "stable_id": format!("src_{index:016x}"),
                "title": format!("Title {index}"),
                "url": format!("https://example.com/{index}"),
                "trust": "external_untrusted",
                "trust_markers": {"injection_hits": 0},
                "snippet": snippet,
                "excerpts": excerpts
                    .into_iter()
                    .enumerate()
                    .map(|(position, text)| json!({"text": text, "provenance": "snippet", "score": position}))
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    json!({
        "query": "proptest payload",
        "results": results,
        "warnings": [],
        "structured_warnings": []
    })
}

fn payload_and_budget() -> impl Strategy<Value = (Value, usize)> {
    (
        prop::collection::vec((passage(), prop::collection::vec(passage(), 0..3)), 0..10),
        prop::sample::select(vec![256usize, 320, 512, 1_024, 4_000]),
    )
        .prop_map(|(cards, budget)| (payload_from(cards), budget))
}

fn framed_body(text: &str) -> &str {
    if !text.starts_with("<<<EXTERNAL_UNTRUSTED") {
        return text;
    }
    let Some(close) = text.find(">>>") else {
        return text;
    };
    let body = &text[close + 3..];
    match body.rfind("<<<END>>>") {
        Some(end) => body[..end].trim_matches('\n'),
        None => body,
    }
}

fn passage_chars(value: &Value) -> usize {
    let mut total = 0usize;
    let mut visit = |card: &Value| {
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
            visit(card);
        }
    }
    if let Some(groups) = value.get("groups").and_then(Value::as_array) {
        for group in groups {
            if let Some(results) = group.get("results").and_then(Value::as_array) {
                for card in results {
                    visit(card);
                }
            }
        }
    }
    total
}

fn report_counter(projected: &Value, key: &str) -> u64 {
    projected["excerpt_economy"][key]
        .as_u64()
        .unwrap_or_else(|| panic!("excerpt_economy.{key} must be an integer: {projected}"))
}

fn assert_counter_identity(projected: &Value, budget: usize) {
    let considered = report_counter(projected, "considered_count");
    let admitted = report_counter(projected, "admitted_count");
    let redundant = report_counter(projected, "redundant_count");
    let over_budget = report_counter(projected, "over_budget_count");
    assert_eq!(
        admitted + redundant + over_budget,
        considered,
        "counter identity must hold: {projected}"
    );
    assert_eq!(report_counter(projected, "budget_chars"), budget as u64);
    assert!(
        admitted + redundant <= 256,
        "similarity work is capped at 256 passages: {projected}"
    );
    assert!(
        passage_chars(projected) <= budget,
        "excerpt characters {} exceed budget {budget}: {projected}",
        passage_chars(projected)
    );
}

fn assert_frames_balanced(projected: &Value) {
    let cards = projected["results"].as_array().cloned().unwrap_or_else(|| {
        projected["groups"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|group| group["results"].as_array().cloned().unwrap_or_default())
            .collect()
    });
    for card in cards {
        let mut texts: Vec<&str> = Vec::new();
        if let Some(snippet) = card.get("snippet").and_then(Value::as_str) {
            texts.push(snippet);
        }
        if let Some(excerpts) = card.get("excerpts").and_then(Value::as_array) {
            for excerpt in excerpts {
                if let Some(text) = excerpt.get("text").and_then(Value::as_str) {
                    texts.push(text);
                }
            }
        }
        for text in texts {
            if text.starts_with("<<<EXTERNAL_UNTRUSTED") {
                assert_eq!(
                    text.matches("<<<EXTERNAL_UNTRUSTED").count(),
                    1,
                    "exactly one frame header: {text:?}"
                );
                assert_eq!(
                    text.matches("<<<END>>>").count(),
                    1,
                    "exactly one frame footer: {text:?}"
                );
                assert!(
                    text.ends_with("<<<END>>>"),
                    "frame footer must close the passage: {text:?}"
                );
            }
        }
    }
}

fn stable_ids(projected: &Value) -> Vec<String> {
    projected["results"]
        .as_array()
        .map(|results| {
            results
                .iter()
                .map(|card| card["stable_id"].as_str().unwrap_or_default().to_owned())
                .collect()
        })
        .unwrap_or_default()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 96, ..ProptestConfig::default() })]

    #[test]
    fn projection_is_deterministic_and_preserves_cards((payload, budget) in payload_and_budget()) {
        let first = project_with_limits("web_search", payload.clone(), ResponseDetail::Compact, limits(budget));
        let second = project_with_limits("web_search", payload.clone(), ResponseDetail::Compact, limits(budget));
        prop_assert_eq!(&first, &second);

        let diagnostic = project("web_search", payload.clone(), ResponseDetail::Diagnostic);
        prop_assert_eq!(first["results"].as_array().map(Vec::len), diagnostic["results"].as_array().map(Vec::len));
        prop_assert_eq!(stable_ids(&first), stable_ids(&diagnostic));
        for (index, card) in first["results"].as_array().unwrap().iter().enumerate() {
            prop_assert_eq!(&card["id"], &diagnostic["results"][index]["id"]);
            prop_assert_eq!(&card["url"], &diagnostic["results"][index]["url"]);
            prop_assert_eq!(&card["title"], &diagnostic["results"][index]["title"]);
            prop_assert_eq!(&card["trust_markers"], &diagnostic["results"][index]["trust_markers"]);
        }

        assert_counter_identity(&first, budget);
        assert_frames_balanced(&first);
    }

    #[test]
    fn surviving_text_is_a_prefix_of_its_source((payload, budget) in payload_and_budget()) {
        let projected = project_with_limits("web_search", payload.clone(), ResponseDetail::Compact, limits(budget));
        let original = payload["results"].as_array().unwrap();
        for (index, card) in projected["results"].as_array().unwrap().iter().enumerate() {
            let source = &original[index];
            if let Some(snippet) = card.get("snippet").and_then(Value::as_str) {
                let source_body = framed_body(source["snippet"].as_str().unwrap());
                prop_assert!(source_body.starts_with(framed_body(snippet)), "clipped snippet must be a body prefix");
            }
            let source_texts: Vec<&str> = source["excerpts"]
                .as_array()
                .map(|excerpts| excerpts.iter().filter_map(|e| e["text"].as_str()).collect())
                .unwrap_or_default();
            let kept = card["excerpts"].as_array().map(Vec::len).unwrap_or(0);
            prop_assert!(kept <= source_texts.len());
            for excerpt in card["excerpts"].as_array().into_iter().flatten() {
                let text = framed_body(excerpt["text"].as_str().unwrap());
                prop_assert!(
                    source_texts.iter().any(|candidate| framed_body(candidate).starts_with(text)),
                    "surviving excerpt must derive from a source passage"
                );
            }
        }
    }

    #[test]
    fn suppression_is_monotone_in_budget((payload, low, extra) in (
        prop::collection::vec((passage(), prop::collection::vec(passage(), 0..3)), 0..8),
        prop::sample::select(vec![256usize, 384, 512, 768]),
        0usize..2_000,
    ).prop_map(|(cards, low, extra)| (payload_from(cards), low, extra))) {
        let high = low + extra;
        let smaller = project_with_limits("web_search", payload.clone(), ResponseDetail::Compact, limits(low));
        let larger = project_with_limits("web_search", payload, ResponseDetail::Compact, limits(high));
        let smaller_redundant = report_counter(&smaller, "redundant_count");
        let larger_redundant = report_counter(&larger, "redundant_count");
        prop_assert!(smaller_redundant <= larger_redundant, "suppression must not grow as the budget shrinks: {smaller_redundant} > {larger_redundant}");
        prop_assert!(report_counter(&smaller, "admitted_count") <= report_counter(&larger, "admitted_count"));
        assert_counter_identity(&smaller, low);
        assert_counter_identity(&larger, high);
    }
}

#[test]
fn saturation_256_passages_stays_inside_the_budget() {
    let cards: Vec<(String, Vec<String>)> = (0..256)
        .map(|index| {
            let body = format!("{index:03} {}", "distinct retrieval evidence ".repeat(20));
            (
                format!("<<<EXTERNAL_UNTRUSTED field=snippet id=src_{index}>>>\n{body}\n<<<END>>>"),
                Vec::new(),
            )
        })
        .collect();
    let payload = payload_from(cards);
    let projected = project_with_limits(
        "web_search",
        payload,
        ResponseDetail::Compact,
        limits(4_000),
    );
    assert_counter_identity(&projected, 4_000);
    assert!(report_counter(&projected, "considered_count") == 256);
    assert!(report_counter(&projected, "over_budget_count") > 0);
    assert!(report_counter(&projected, "clipped_count") > 0);
    let ids: HashSet<String> = stable_ids(&projected).into_iter().collect();
    assert_eq!(ids.len(), 256);
    println!(
        "excerpt-economy saturation passages=256 budget=4000 considered={} admitted={} redundant={} over_budget={} clipped={} chars_before={} chars_after={}",
        report_counter(&projected, "considered_count"),
        report_counter(&projected, "admitted_count"),
        report_counter(&projected, "redundant_count"),
        report_counter(&projected, "over_budget_count"),
        report_counter(&projected, "clipped_count"),
        report_counter(&projected, "chars_before"),
        report_counter(&projected, "chars_after")
    );
}

#[test]
fn saturation_beyond_the_passage_cap_is_reported_and_trimmed() {
    let cards: Vec<(String, Vec<String>)> = (0..300)
        .map(|index| {
            (
                format!("unique passage number {index} with several words"),
                Vec::new(),
            )
        })
        .collect();
    let payload = payload_from(cards);
    let projected = project_with_limits(
        "web_search",
        payload,
        ResponseDetail::Compact,
        limits(32_000),
    );
    assert_counter_identity(&projected, 32_000);
    assert_eq!(report_counter(&projected, "considered_count"), 300);
    assert!(
        report_counter(&projected, "admitted_count")
            + report_counter(&projected, "redundant_count")
            <= 256
    );
    assert!(report_counter(&projected, "over_budget_count") >= 44);
    assert_eq!(projected["results"].as_array().unwrap().len(), 300);
    let retained_with_text = projected["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|card| card.get("snippet").is_some())
        .count();
    assert_eq!(
        retained_with_text as u64,
        report_counter(&projected, "admitted_count")
    );
    assert!(passage_chars(&projected) <= 32_000);
}
