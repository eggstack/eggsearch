use chrono::NaiveDate;

use crate::core::knowledge_delta::{
    next_day, CalendarDate, DatePrecision, DEFAULT_SUBQUERIES, MAX_SUBQUERIES,
};

pub const ENGLISH_HINTS: &[&str] = &["release", "compatibility", "deprecation"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmptyStateReason {
    NoCutoffResolved,
    NoDatedSources,
    AllSourcesAtOrBeforeCutoff,
    NoMatchingIndexHits,
    IndexNotSeeded,
    IndexStale,
    DeltasEmitted,
}

impl EmptyStateReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoCutoffResolved => "no_cutoff_resolved",
            Self::NoDatedSources => "no_dated_sources",
            Self::AllSourcesAtOrBeforeCutoff => "all_sources_at_or_before_cutoff",
            Self::NoMatchingIndexHits => "no_matching_index_hits",
            Self::IndexNotSeeded => "index_not_seeded",
            Self::IndexStale => "index_stale",
            Self::DeltasEmitted => "deltas_emitted",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedQuery {
    pub query: String,
    pub topic_unchanged: bool,
    pub english_hint: Option<&'static str>,
}

pub fn plan_subqueries(
    topic: &str,
    requested: Option<usize>,
    max_query_chars: usize,
) -> Result<Vec<PlannedQuery>, String> {
    if topic.trim().is_empty() {
        return Err("topic must be non-empty".to_string());
    }
    let topic_chars = topic.chars().count();
    if topic_chars > max_query_chars {
        return Err(format!("topic must be <= {max_query_chars} characters"));
    }
    let cap = requested
        .unwrap_or(DEFAULT_SUBQUERIES)
        .clamp(1, MAX_SUBQUERIES);
    let mut planned = vec![PlannedQuery {
        query: topic.to_string(),
        topic_unchanged: true,
        english_hint: None,
    }];
    for hint in ENGLISH_HINTS {
        if planned.len() >= cap {
            break;
        }
        let query = format!("{topic} {hint}");
        if query.chars().count() > max_query_chars {
            continue;
        }
        planned.push(PlannedQuery {
            query,
            topic_unchanged: false,
            english_hint: Some(hint),
        });
    }
    Ok(planned)
}

pub fn day_search_window(
    cutoff: &CalendarDate,
    today: NaiveDate,
) -> Option<(NaiveDate, NaiveDate)> {
    if cutoff.precision != DatePrecision::Day {
        return None;
    }
    let month = cutoff.month?;
    let day = cutoff.day?;
    let cutoff_day = NaiveDate::from_ymd_opt(cutoff.year, u32::from(month), u32::from(day))?;
    let start = next_day(cutoff_day)?;
    if start > today {
        return None;
    }
    Some((start, today))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topic_is_first_and_unchanged() {
        let planned = plan_subqueries("free-threaded Python", None, 512).unwrap();
        assert_eq!(planned.len(), 4);
        assert_eq!(planned[0].query, "free-threaded Python");
        assert!(planned[0].topic_unchanged);
        assert!(planned[1].query.ends_with(" release"));
        assert_eq!(plan_subqueries("topic", Some(1), 512).unwrap().len(), 1);
        assert_eq!(plan_subqueries("topic", Some(99), 512).unwrap().len(), 4);
    }

    #[test]
    fn topic_and_generated_hints_respect_the_configured_query_bound() {
        let planned = plan_subqueries("topic", None, 15).unwrap();
        assert_eq!(planned.len(), 2);
        assert_eq!(planned[0].query, "topic");
        assert!(planned
            .iter()
            .all(|query| query.query.chars().count() <= 15));
        assert!(plan_subqueries("0123456789abcdef", None, 15).is_err());
    }

    #[test]
    fn coarser_cutoff_has_no_provider_window() {
        let month = CalendarDate {
            year: 2024,
            month: Some(6),
            day: None,
            precision: DatePrecision::Month,
        };
        let today = NaiveDate::from_ymd_opt(2026, 11, 20).unwrap();
        assert!(day_search_window(&month, today).is_none());
        let day = CalendarDate {
            year: 2024,
            month: Some(6),
            day: Some(1),
            precision: DatePrecision::Day,
        };
        assert_eq!(
            day_search_window(&day, today),
            Some((NaiveDate::from_ymd_opt(2024, 6, 2).unwrap(), today))
        );
    }
}
