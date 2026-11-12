#![cfg(feature = "mock")]
//! Captured-fixture conformance for the five HTML search scrapers.
//!
//! Each engine must distinguish four outcomes from captured markup:
//! parsed rows, an explicit empty state, a blocked page, and a structural
//! parse failure.
//! A seeded drift case renames the result container and requires
//! `EngineError::ParseFailed` instead of a bare empty result.

use std::path::PathBuf;

use eggsearch::engine_parse_test::parse_html;
use eggsearch::meta::engines::error::EngineError;

struct Case {
    provider: &'static str,
    first_title: &'static str,
    first_url: &'static str,
    drift_hint: &'static str,
    drift_from: &'static str,
    drift_to: &'static str,
}

const CASES: [Case; 5] = [
    Case {
        provider: "brave",
        first_title: "Rust Programming Language",
        first_url: "https://rust-lang.org/en-US/",
        drift_hint: "div[data-type='web']",
        drift_from: "data-type=\"web\"",
        drift_to: "data-type=\"webs\"",
    },
    Case {
        provider: "duckduckgo",
        first_title: "Rust Programming Language",
        first_url: "https://www.rust-lang.org/",
        drift_hint: "div.result",
        drift_from: "class=\"result ",
        drift_to: "class=\"result-renamed ",
    },
    Case {
        provider: "mojeek",
        first_title: "Rust Programming Language",
        first_url: "https://www.rust-lang.org/",
        drift_hint: "results-standard",
        drift_from: "class=\"results-standard\"",
        drift_to: "class=\"results-standard-renamed\"",
    },
    Case {
        provider: "startpage",
        first_title: "Rust Programming Language",
        first_url: "https://www.rust-lang.org/en-US",
        drift_hint: "payload",
        drift_from: "\"clickUrl\"",
        drift_to: "\"destinationUrl\"",
    },
    Case {
        provider: "yahoo",
        first_title: "Rust Programming Language",
        first_url: "https://www.rust-lang.org/",
        drift_hint: "div.algo-sr",
        drift_from: "class=\"algo-sr\"",
        drift_to: "class=\"algo-sr-renamed\"",
    },
];

fn fixture(provider: &str, name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/engines")
        .join(provider)
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[test]
fn results_fixture_parses_rows() {
    for case in &CASES {
        let html = fixture(case.provider, "results.html");
        let rows = parse_html(case.provider, &html, 10)
            .unwrap_or_else(|error| panic!("{} results.html: {error}", case.provider));
        assert!(
            !rows.is_empty(),
            "{} results.html must yield rows",
            case.provider
        );
        assert_eq!(
            rows[0].title, case.first_title,
            "{} first title",
            case.provider
        );
        assert_eq!(rows[0].url, case.first_url, "{} first url", case.provider);
        for row in &rows {
            assert!(!row.title.is_empty(), "{} empty title", case.provider);
            assert!(
                row.url.starts_with("http"),
                "{} non-http url: {}",
                case.provider,
                row.url
            );
        }
    }
}

#[test]
fn results_fixture_respects_max_results() {
    for case in &CASES {
        let html = fixture(case.provider, "results.html");
        let rows = parse_html(case.provider, &html, 1)
            .unwrap_or_else(|error| panic!("{} results.html: {error}", case.provider));
        assert_eq!(rows.len(), 1, "{} max_results", case.provider);
    }
}

#[test]
fn empty_fixture_reports_explicit_empty_state() {
    for case in &CASES {
        let html = fixture(case.provider, "empty.html");
        let rows = parse_html(case.provider, &html, 10).unwrap_or_else(|error| {
            panic!(
                "{} empty.html must be an explicit empty state, got: {error}",
                case.provider
            )
        });
        assert!(
            rows.is_empty(),
            "{} empty.html must yield zero rows",
            case.provider
        );
    }
}

#[test]
fn blocked_fixture_reports_blocked() {
    for case in &CASES {
        let html = fixture(case.provider, "blocked.html");
        let error = parse_html(case.provider, &html, 10).expect_err("blocked page must not parse");
        match error {
            EngineError::Blocked { engine, reason } => {
                assert_eq!(engine, case.provider, "blocked engine id");
                assert!(!reason.is_empty(), "blocked reason must not be empty");
            }
            other => panic!(
                "{} blocked.html must yield Blocked, got: {other:?}",
                case.provider
            ),
        }
    }
}

#[test]
fn seeded_drift_reports_parse_failed() {
    for case in &CASES {
        let html = fixture(case.provider, "results.html");
        assert!(
            html.contains(case.drift_from),
            "{} drift seed {:?} missing from fixture",
            case.provider,
            case.drift_from
        );
        let drifted = html.replace(case.drift_from, case.drift_to);
        assert!(
            !drifted.contains(case.drift_from),
            "{} drift seed still present after rename",
            case.provider
        );
        let error = parse_html(case.provider, &drifted, 10)
            .expect_err("a renamed result container must not parse as empty");
        match error {
            EngineError::ParseFailed { engine, reason } => {
                assert_eq!(engine, case.provider, "drift engine id");
                assert!(
                    reason.contains(case.drift_hint),
                    "{} drift reason must name the drifted shape {:?}, got: {reason}",
                    case.provider,
                    case.drift_hint
                );
            }
            other => panic!(
                "{} drifted results.html must yield ParseFailed, got: {other:?}",
                case.provider
            ),
        }
    }
}

#[test]
fn unknown_provider_is_rejected() {
    let error = parse_html("not_a_scraper", "<html></html>", 10).expect_err("unknown id");
    assert!(matches!(error, EngineError::Unsupported { .. }));
}
