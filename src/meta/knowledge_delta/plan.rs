use chrono::NaiveDate;

use crate::core::knowledge_delta::{
    next_day, CalendarDate, DatePrecision, DEFAULT_SUBQUERIES, MAX_SUBQUERIES,
};

pub const ENGLISH_HINTS: &[&str] = &["release", "compatibility", "deprecation"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedQuery {
    pub query: String,
    pub topic_unchanged: bool,
    pub english_hint: Option<&'static str>,
}

pub fn plan_subqueries(topic: &str, requested: Option<usize>) -> Vec<PlannedQuery> {
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
        planned.push(PlannedQuery {
            query: format!("{topic} {hint}"),
            topic_unchanged: false,
            english_hint: Some(hint),
        });
    }
    planned
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
        let planned = plan_subqueries("free-threaded Python", None);
        assert_eq!(planned.len(), 4);
        assert_eq!(planned[0].query, "free-threaded Python");
        assert!(planned[0].topic_unchanged);
        assert!(planned[1].query.ends_with(" release"));
        assert_eq!(plan_subqueries("topic", Some(1)).len(), 1);
        assert_eq!(plan_subqueries("topic", Some(99)).len(), 4);
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
