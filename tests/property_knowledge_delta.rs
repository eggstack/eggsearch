use eggsearch::core::knowledge_delta::{
    classify, compare_cutoff, parse_calendar_date, reported_after_cutoff, CalendarDate, DateBasis,
    DatePrecision, DatedEvidence, TemporalRelationship,
};
use proptest::prelude::*;

fn date(raw: &str, precision: DatePrecision) -> CalendarDate {
    parse_calendar_date(raw, precision).unwrap_or_else(|| panic!("unparsed {raw}"))
}

fn day(raw: &str) -> CalendarDate {
    date(raw, DatePrecision::Day)
}

fn month(raw: &str) -> CalendarDate {
    date(raw, DatePrecision::Month)
}

fn year(raw: &str) -> CalendarDate {
    date(raw, DatePrecision::Year)
}

fn at_precision(year: i32, month: u8, day: u8, precision: u8) -> CalendarDate {
    match precision {
        0 => CalendarDate {
            year,
            month: Some(month),
            day: Some(day),
            precision: DatePrecision::Day,
        },
        1 => CalendarDate {
            year,
            month: Some(month),
            day: None,
            precision: DatePrecision::Month,
        },
        _ => CalendarDate {
            year,
            month: None,
            day: None,
            precision: DatePrecision::Year,
        },
    }
}

#[test]
fn comparison_fixtures_match_adr_0020() {
    let cases = [
        (
            day("2024-06-02"),
            day("2024-06-01"),
            TemporalRelationship::AfterCutoff,
        ),
        (
            day("2024-06-01"),
            day("2024-06-01"),
            TemporalRelationship::OnOrBeforeCutoff,
        ),
        (
            day("2024-07-01"),
            month("2024-06"),
            TemporalRelationship::AfterCutoff,
        ),
        (
            day("2024-06-15"),
            month("2024-06"),
            TemporalRelationship::Indeterminate,
        ),
        (
            month("2024-06"),
            month("2024-06"),
            TemporalRelationship::Indeterminate,
        ),
        (
            month("2024-06"),
            day("2024-06-15"),
            TemporalRelationship::Indeterminate,
        ),
        (
            month("2024-06"),
            year("2024"),
            TemporalRelationship::Indeterminate,
        ),
    ];
    for (evidence, cutoff, expected) in cases {
        assert_eq!(
            compare_cutoff(&evidence, DateBasis::Publication, &cutoff),
            expected,
            "{evidence:?} vs {cutoff:?}"
        );
    }
}

#[test]
fn unknown_cutoff_or_basis_stays_indeterminate() {
    let evidence = day("2024-07-01");
    let cutoff = day("2024-06-01");
    assert_eq!(
        compare_cutoff(&evidence, DateBasis::Unknown, &cutoff),
        TemporalRelationship::Indeterminate
    );
    let unknown = CalendarDate {
        year: 2024,
        month: None,
        day: None,
        precision: DatePrecision::Unknown,
    };
    assert_eq!(
        compare_cutoff(&evidence, DateBasis::Publication, &unknown),
        TemporalRelationship::Indeterminate
    );
    assert_eq!(
        compare_cutoff(&unknown, DateBasis::Event, &cutoff),
        TemporalRelationship::Indeterminate
    );
}

#[test]
fn coarser_precision_is_not_folded_to_midnight() {
    let parsed = month("2024-06");
    assert_eq!(parsed.day, None);
    assert_eq!(parsed.normalized().as_deref(), Some("2024-06"));
    let yearly = year("2024");
    assert_eq!(yearly.month, None);
    assert_eq!(yearly.day, None);
    assert!(parse_calendar_date("2024-06-01", DatePrecision::Month).is_none());
    assert!(parse_calendar_date("2024-06", DatePrecision::Day).is_none());
}

#[test]
fn conflicting_bases_do_not_pick_a_winner() {
    let cutoff = day("2024-06-01");
    let classified = classify(
        &[
            DatedEvidence {
                date: day("2024-05-01"),
                basis: DateBasis::Event,
            },
            DatedEvidence {
                date: day("2024-08-01"),
                basis: DateBasis::Event,
            },
            DatedEvidence {
                date: day("2024-07-01"),
                basis: DateBasis::Publication,
            },
        ],
        &cutoff,
    );
    assert!(classified.event_conflicted);
    assert_eq!(
        classified.event_relationship,
        TemporalRelationship::Indeterminate
    );
    assert!(reported_after_cutoff(&classified).is_none());
}

#[test]
fn publication_only_after_cutoff_is_labeled_publication() {
    let cutoff = day("2024-06-01");
    let classified = classify(
        &[DatedEvidence {
            date: day("2024-07-02"),
            basis: DateBasis::Publication,
        }],
        &cutoff,
    );
    assert!(classified.event_missing);
    assert_eq!(
        classified.event_relationship,
        TemporalRelationship::Indeterminate
    );
    let (basis, date) = reported_after_cutoff(&classified).unwrap();
    assert_eq!(basis, DateBasis::Publication);
    assert_eq!(date, day("2024-07-02"));
}

#[test]
fn event_on_or_before_is_not_emitted() {
    let cutoff = day("2024-06-01");
    let classified = classify(
        &[
            DatedEvidence {
                date: day("2024-06-01"),
                basis: DateBasis::Event,
            },
            DatedEvidence {
                date: day("2024-09-01"),
                basis: DateBasis::Publication,
            },
        ],
        &cutoff,
    );
    assert_eq!(
        classified.event_relationship,
        TemporalRelationship::OnOrBeforeCutoff
    );
    assert!(reported_after_cutoff(&classified).is_none());
}

proptest! {
    #[test]
    fn equal_day_is_on_or_before(
        year in 1970i32..2100,
        month in 1u32..13,
        day_of_month in 1u32..29
    ) {
        let raw = format!("{year:04}-{month:02}-{day_of_month:02}");
        let parsed = day(&raw);
        prop_assert_eq!(
            compare_cutoff(&parsed, DateBasis::Event, &parsed),
            TemporalRelationship::OnOrBeforeCutoff
        );
    }

    #[test]
    fn equal_month_or_year_is_indeterminate(year in 1970i32..2100, month in 1u8..13) {
        let monthly = CalendarDate {
            year,
            month: Some(month),
            day: None,
            precision: DatePrecision::Month,
        };
        let yearly = CalendarDate {
            year,
            month: None,
            day: None,
            precision: DatePrecision::Year,
        };
        prop_assert_eq!(
            compare_cutoff(&monthly, DateBasis::Publication, &monthly),
            TemporalRelationship::Indeterminate
        );
        prop_assert_eq!(
            compare_cutoff(&yearly, DateBasis::Publication, &yearly),
            TemporalRelationship::Indeterminate
        );
    }

    #[test]
    fn date_accounting_relations_partition_dated_sources(
        dates in prop::collection::vec((1970i32..2100, 1u8..13, 1u8..29, 0u8..3), 0..30),
        cutoff_year in 1970i32..2100,
        cutoff_month in 1u8..13,
        cutoff_day in 1u8..29,
        cutoff_precision in 0u8..3
    ) {
        let cutoff = at_precision(cutoff_year, cutoff_month, cutoff_day, cutoff_precision);
        let mut dated = 0usize;
        let mut at_or_before_cutoff = 0usize;
        let mut not_after_cutoff = 0usize;
        let mut after_cutoff = 0usize;
        for &(year, month, day, precision) in &dates {
            let evidence = at_precision(year, month, day, precision);
            let relation = compare_cutoff(&evidence, DateBasis::Publication, &cutoff);
            dated += 1;
            if relation == TemporalRelationship::OnOrBeforeCutoff {
                at_or_before_cutoff += 1;
            }
            if relation == TemporalRelationship::AfterCutoff {
                after_cutoff += 1;
            } else {
                not_after_cutoff += 1;
            }
        }
        prop_assert_eq!(dated, dates.len());
        prop_assert_eq!(dated, not_after_cutoff + after_cutoff);
        prop_assert!(at_or_before_cutoff <= not_after_cutoff);
    }
}
