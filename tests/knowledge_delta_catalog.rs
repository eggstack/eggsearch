use std::collections::BTreeMap;
use std::fmt::Write;

use eggsearch::core::knowledge_delta::{
    parse_calendar_date, resolve_cutoff, CutoffOrigin, DatePrecision, OperatorCutoff, SnapshotRow,
};
use eggsearch::meta::knowledge_delta::{Catalog, CatalogEntry, BUNDLED_CUTOFF_SNAPSHOT};

fn parse_catalog(raw: &str) -> Catalog {
    serde_json::from_str(raw).expect("inline catalog JSON should parse")
}

fn problems_contain(
    result: Result<(), Vec<eggsearch::meta::knowledge_delta::CatalogProblem>>,
    needle: &str,
) {
    let problems = result.expect_err("catalog operation should reject the fixture");
    assert!(problems
        .iter()
        .any(|problem| problem.message().contains(needle)));
}

#[test]
fn embedded_catalog_parses_and_passes_validation() {
    assert!(Catalog::embedded().validate().is_ok());
}

#[test]
fn embedded_catalog_matches_bundled_snapshot() {
    assert!(Catalog::embedded()
        .verify_snapshot(BUNDLED_CUTOFF_SNAPSHOT)
        .is_ok());
}

#[test]
fn every_verified_model_resolves_from_the_bundled_snapshot() {
    let operator = BTreeMap::new();
    for entry in Catalog::embedded().verified_rows() {
        let resolved = resolve_cutoff(
            Some(entry.model_id.as_str()),
            None,
            &operator,
            BUNDLED_CUTOFF_SNAPSHOT,
        )
        .expect("verified model resolves");
        let precision = DatePrecision::parse(&entry.precision).unwrap();
        assert_eq!(
            resolved.date,
            parse_calendar_date(&entry.date, precision).unwrap(),
            "{}",
            entry.model_id
        );
        assert_eq!(resolved.date.precision, precision, "{}", entry.model_id);
    }
}

#[test]
fn every_alias_resolves_to_its_row_cutoff() {
    let operator = BTreeMap::new();
    for entry in Catalog::embedded().verified_rows() {
        let expected =
            parse_calendar_date(&entry.date, DatePrecision::parse(&entry.precision).unwrap())
                .unwrap();
        for alias in &entry.aliases {
            let resolved = resolve_cutoff(Some(alias), None, &operator, BUNDLED_CUTOFF_SNAPSHOT)
                .expect("verified alias resolves");
            assert_eq!(resolved.date, expected, "{alias}");
        }
    }
}

#[test]
fn unknown_model_returns_none() {
    assert!(resolve_cutoff(
        Some("unlisted-model"),
        None,
        &BTreeMap::new(),
        BUNDLED_CUTOFF_SNAPSHOT,
    )
    .is_none());
}

#[test]
fn operator_override_precedes_the_bundled_snapshot() {
    let mut operator = BTreeMap::new();
    operator.insert(
        "gpt-6-astra".to_string(),
        OperatorCutoff {
            date: "2025-01-02".to_string(),
            precision: "day".to_string(),
            source: "operator config".to_string(),
            verified_at: Some("2026-11-20".to_string()),
        },
    );
    let resolved = resolve_cutoff(
        Some("gpt-6-astra"),
        None,
        &operator,
        BUNDLED_CUTOFF_SNAPSHOT,
    )
    .unwrap();
    assert_eq!(resolved.origin, CutoffOrigin::Operator);
    assert_eq!(resolved.date.normalized().as_deref(), Some("2025-01-02"));
}

#[test]
fn duplicate_alias_across_entries_is_rejected() {
    let catalog = parse_catalog(
        r#"[
          {"model_id":"alpha","aliases":["shared"],"date":"2026-01-02","precision":"day","source":"Fixture","source_url":"https://example.org/alpha","verified_at":"2026-11-20","origin":"provider_official","status":"verified","note":"fixture"},
          {"model_id":"beta","aliases":["shared"],"date":"2026-01-02","precision":"day","source":"Fixture","source_url":"https://example.org/beta","verified_at":"2026-11-20","origin":"provider_official","status":"verified","note":"fixture"}
        ]"#,
    );
    problems_contain(catalog.validate(), "beta");
}

#[test]
fn alias_shadowing_another_model_id_is_rejected() {
    let catalog = parse_catalog(
        r#"[
          {"model_id":"alpha","aliases":["beta"],"date":"2026-01-02","precision":"day","source":"Fixture","source_url":"https://example.org/alpha","verified_at":"2026-11-20","origin":"provider_official","status":"verified","note":"fixture"},
          {"model_id":"beta","aliases":[],"date":"2026-01-02","precision":"day","source":"Fixture","source_url":"https://example.org/beta","verified_at":"2026-11-20","origin":"provider_official","status":"verified","note":"fixture"}
        ]"#,
    );
    problems_contain(catalog.validate(), "alpha");
}

#[test]
fn verified_entry_missing_from_snapshot_is_rejected() {
    let catalog = parse_catalog(
        r#"[{"model_id":"alpha","aliases":[],"date":"2026-01-02","precision":"day","source":"Fixture","source_url":"https://example.org/alpha","verified_at":"2026-11-20","origin":"provider_official","status":"verified","note":"fixture"}]"#,
    );
    problems_contain(catalog.verify_snapshot(&[]), "alpha");
}

#[test]
fn snapshot_row_missing_from_catalog_is_rejected() {
    let catalog = parse_catalog("[]");
    let snapshot = [SnapshotRow {
        model_id: "unlisted-model",
        aliases: &[],
        date: "2026-01-02",
        precision: DatePrecision::Day,
        source: "Fixture",
        source_url: "https://example.org/unlisted-model",
        verified_at: "2026-11-20",
        origin: CutoffOrigin::ProviderOfficial,
    }];
    problems_contain(catalog.verify_snapshot(&snapshot), "unlisted-model");
}

#[test]
fn date_and_precision_mismatch_is_rejected() {
    let invalid_date = parse_catalog(
        r#"[{"model_id":"alpha","aliases":[],"date":"2026-01","precision":"day","source":"Fixture","source_url":"https://example.org/alpha","verified_at":"2026-11-20","origin":"provider_official","status":"verified","note":"fixture"}]"#,
    );
    problems_contain(invalid_date.validate(), "alpha");

    let snapshot = [SnapshotRow {
        model_id: "gpt-6-astra",
        aliases: &["openai/gpt-6-astra"],
        date: "2026-04",
        precision: DatePrecision::Month,
        source: "OpenAI API model documentation",
        source_url: "https://platform.openai.com/docs/models/gpt-6-astra.md",
        verified_at: "2026-11-20",
        origin: CutoffOrigin::ProviderOfficial,
    }];
    problems_contain(
        Catalog::embedded().verify_snapshot(&snapshot),
        "gpt-6-astra",
    );
}

#[test]
fn verified_entry_requires_source_url_and_verified_at() {
    let catalog = parse_catalog(
        r#"[{"model_id":"alpha","aliases":[],"date":"2026-01-02","precision":"day","source":"","source_url":"","verified_at":"2026-11","origin":"provider_official","status":"verified","note":"fixture"}]"#,
    );
    let problems = catalog
        .validate()
        .expect_err("missing verification metadata is rejected");
    assert!(problems
        .iter()
        .any(|problem| problem.message().contains("source")));
    assert!(problems
        .iter()
        .any(|problem| problem.message().contains("source_url")));
    assert!(problems
        .iter()
        .any(|problem| problem.message().contains("verified_at")));
}

#[test]
fn catalog_size_is_bounded() {
    let mut raw = String::from("[");
    for index in 0..129 {
        if index > 0 {
            raw.push(',');
        }
        write!(
            raw,
            r#"{{"model_id":"model-{index:03}","aliases":[],"date":"","precision":"unknown","source":"","source_url":"","verified_at":"","origin":"provider_official","status":"no_published_cutoff","note":""}}"#
        )
        .unwrap();
    }
    raw.push(']');
    let catalog = parse_catalog(&raw);
    problems_contain(catalog.validate(), "maximum is 128");
}

#[test]
fn verified_rows_exclude_unpublished_cutoffs() {
    assert_eq!(Catalog::embedded().verified_rows().count(), 11);
    assert!(Catalog::embedded()
        .0
        .iter()
        .filter(|entry: &&CatalogEntry| entry.status != "verified")
        .all(|entry| entry.date.is_empty() && entry.precision == "unknown"));
}
